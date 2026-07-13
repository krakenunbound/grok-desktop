//! Concurrent named-agent runs used by the Agents workspace.

use crate::{config, grok_cli, grok_process};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use uuid::Uuid;

const MAX_AGENT_PROMPT_CHARS: usize = 400_000;
const MAX_AGENT_TEXT_CHARS: usize = 100_000;
const MAX_CONCURRENT_AGENT_RUNS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub name: String,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentRunStarted {
    pub id: String,
    pub agent: String,
    pub prompt: String,
    pub cwd: String,
    pub started_at: String,
}

pub struct StartAgentRunRequest {
    pub cwd: String,
    pub agent: String,
    pub prompt: String,
    pub model: String,
    pub yolo: bool,
}

#[derive(Debug, Clone, Serialize)]
struct AgentRunEvent {
    run_id: String,
    kind: String,
    status: String,
    chunk: String,
    exit_code: Option<i32>,
}

struct AgentRuntime {
    child: Mutex<Option<Child>>,
    cancel: AtomicBool,
    job: Mutex<Option<isize>>,
}

pub struct AgentRunManager {
    runs: Arc<Mutex<HashMap<String, Arc<AgentRuntime>>>>,
}

impl AgentRunManager {
    pub fn new() -> Self {
        Self {
            runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for AgentRunManager {
    fn default() -> Self {
        Self::new()
    }
}

fn validated_cwd(cwd: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(cwd.trim())
        .canonicalize()
        .map_err(|e| format!("Working directory does not exist: {cwd} ({e})"))?;
    if !path.is_dir() {
        return Err("Agent working directory must be a folder".into());
    }
    Ok(path)
}

fn valid_agent_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn source_label(source: Option<&Value>) -> String {
    let Some(source) = source else {
        return "unknown".into();
    };
    let kind = source
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if let Some(path) = source.get("path").and_then(Value::as_str) {
        format!("{kind} · {path}")
    } else if let Some(plugin) = source.get("plugin_name").and_then(Value::as_str) {
        format!("plugin · {plugin}")
    } else {
        kind.to_string()
    }
}

pub fn list_definitions(binary_override: &str, cwd: &str) -> Result<Vec<AgentDefinition>, String> {
    let cwd = validated_cwd(cwd)?;
    let output = grok_cli::run_grok(binary_override, Some(&cwd), &["inspect", "--json"])?;
    if !output.status.success() {
        return Err("Grok could not inspect agent definitions for this project".into());
    }
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Could not parse Grok agent inventory: {e}"))?;
    let mut definitions = value
        .get("agents")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.to_string();
            if !valid_agent_name(&name) {
                return None;
            }
            Some(AgentDefinition {
                name,
                description: item
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("Named Grok agent")
                    .to_string(),
                source: source_label(item.get("source")),
            })
        })
        .collect::<Vec<_>>();
    definitions.sort_by(|a, b| a.name.cmp(&b.name));
    definitions.dedup_by(|a, b| a.name == b.name);
    Ok(definitions)
}

pub fn create_definition(
    cwd: &str,
    scope: &str,
    name: &str,
    description: &str,
    instructions: &str,
) -> Result<PathBuf, String> {
    let name = name.trim().to_ascii_lowercase();
    if !valid_agent_name(&name) {
        return Err(
            "Agent names may contain only letters, numbers, hyphens, and underscores".into(),
        );
    }
    let description = description.trim();
    let instructions = instructions.trim();
    if description.is_empty() || instructions.is_empty() {
        return Err("Description and instructions are required".into());
    }
    if description.len() > 2_000 || instructions.len() > MAX_AGENT_TEXT_CHARS {
        return Err("Agent definition is too large".into());
    }

    let directory = match scope {
        "project" => validated_cwd(cwd)?.join(".grok").join("agents"),
        "user" => dirs::home_dir()
            .ok_or_else(|| "Could not locate the user home folder".to_string())?
            .join(".grok")
            .join("agents"),
        _ => return Err("Agent scope must be project or user".into()),
    };
    std::fs::create_dir_all(&directory)
        .map_err(|e| format!("Could not create agents folder: {e}"))?;
    let destination = directory.join(format!("{name}.md"));
    let folded_description = description
        .lines()
        .map(|line| format!("  {}", line.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    let content = format!(
        "---\nname: {name}\ndescription: >\n{folded_description}\nprompt_mode: full\nmodel: inherit\npermission_mode: default\nagents_md: true\n---\n\n{instructions}\n"
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                format!("An agent named '{name}' already exists in this scope")
            } else {
                format!("Could not save agent definition: {e}")
            }
        })?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Could not save agent definition: {e}"))?;
    Ok(destination)
}

fn push_value(args: &mut Vec<String>, flag: &str, value: &str) {
    if !value.trim().is_empty() {
        args.push(flag.into());
        args.push(value.trim().into());
    }
}

fn emit(app: &AppHandle, run_id: &str, kind: &str, status: &str, chunk: String, exit: Option<i32>) {
    let _ = app.emit(
        "agent-run-event",
        AgentRunEvent {
            run_id: run_id.into(),
            kind: kind.into(),
            status: status.into(),
            chunk,
            exit_code: exit,
        },
    );
}

pub async fn start_run(
    app: AppHandle,
    manager: &AgentRunManager,
    binary_override: &str,
    request: StartAgentRunRequest,
) -> Result<AgentRunStarted, String> {
    let StartAgentRunRequest {
        cwd,
        agent,
        prompt,
        model,
        yolo,
    } = request;
    if prompt.trim().is_empty() || prompt.len() > MAX_AGENT_PROMPT_CHARS {
        return Err("Enter an agent task (maximum 400,000 characters)".into());
    }
    if manager.runs.lock().await.len() >= MAX_CONCURRENT_AGENT_RUNS {
        return Err(format!(
            "Up to {MAX_CONCURRENT_AGENT_RUNS} agents can run at once"
        ));
    }
    let cwd = validated_cwd(&cwd)?;
    if !agent.is_empty() {
        if !valid_agent_name(&agent) {
            return Err("Invalid agent name".into());
        }
        let known = list_definitions(binary_override, cwd.to_string_lossy().as_ref())?;
        if !known.iter().any(|item| item.name == agent) {
            return Err(format!("Grok did not discover an agent named '{agent}'"));
        }
    }
    let binary = grok_process::resolve_grok_binary(binary_override)?;
    let id = Uuid::new_v4().to_string();
    let settings = config::load_settings();
    let mut args = vec![
        "-p".into(),
        prompt.clone(),
        "-m".into(),
        model,
        "--cwd".into(),
        cwd.to_string_lossy().to_string(),
        "--output-format".into(),
        "plain".into(),
        "--session-id".into(),
        id.clone(),
    ];
    if !agent.is_empty() {
        args.push("--agent".into());
        args.push(agent.clone());
    }
    push_value(&mut args, "--reasoning-effort", &settings.reasoning_effort);
    if yolo {
        args.push("--always-approve".into());
    } else if settings.permission_mode != "default" {
        args.push("--permission-mode".into());
        args.push(settings.permission_mode);
    }
    if !settings.plan_mode {
        args.push("--no-plan".into());
    }
    if settings.disable_web_search {
        args.push("--disable-web-search".into());
    }
    if !settings.subagents_enabled {
        args.push("--no-subagents".into());
    }
    if settings.memory_enabled {
        args.push("--experimental-memory".into());
    } else {
        args.push("--no-memory".into());
    }

    let mut command = Command::new(binary);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Could not start Grok agent: {e}"))?;
    let process_id = child
        .id()
        .ok_or_else(|| "Agent process has no id".to_string())?;
    let job = grok_process::create_kill_on_close_job(process_id)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Could not capture agent output".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Could not capture agent errors".to_string())?;
    let runtime = Arc::new(AgentRuntime {
        child: Mutex::new(Some(child)),
        cancel: AtomicBool::new(false),
        job: Mutex::new(Some(job)),
    });
    manager
        .runs
        .lock()
        .await
        .insert(id.clone(), runtime.clone());

    let started = AgentRunStarted {
        id: id.clone(),
        agent: if agent.is_empty() {
            "default".into()
        } else {
            agent
        },
        prompt,
        cwd: cwd.to_string_lossy().to_string(),
        started_at: chrono::Utc::now().to_rfc3339(),
    };
    emit(&app, &id, "status", "running", String::new(), None);

    let out_app = app.clone();
    let out_id = id.clone();
    let out_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            emit(
                &out_app,
                &out_id,
                "stdout",
                "running",
                format!("{line}\n"),
                None,
            );
        }
    });
    let err_app = app.clone();
    let err_id = id.clone();
    let err_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            emit(
                &err_app,
                &err_id,
                "stderr",
                "running",
                format!("{line}\n"),
                None,
            );
        }
    });
    let wait_app = app.clone();
    let wait_id = id.clone();
    let runs = manager.runs.clone();
    tokio::spawn(async move {
        let exit_code = loop {
            if runtime.cancel.load(Ordering::SeqCst) {
                break -1;
            }
            let mut child = runtime.child.lock().await;
            match child.as_mut().map(Child::try_wait) {
                Some(Ok(Some(status))) => break status.code().unwrap_or(-1),
                Some(Ok(None)) => {
                    drop(child);
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Some(Err(_)) | None => break -1,
            }
        };
        let _ = out_task.await;
        let _ = err_task.await;
        if let Some(job) = runtime.job.lock().await.take() {
            grok_process::close_job(job);
        }
        runtime.child.lock().await.take();
        runs.lock().await.remove(&wait_id);
        let cancelled = runtime.cancel.load(Ordering::SeqCst);
        emit(
            &wait_app,
            &wait_id,
            "done",
            if cancelled {
                "cancelled"
            } else if exit_code == 0 {
                "completed"
            } else {
                "failed"
            },
            String::new(),
            Some(exit_code),
        );
    });
    Ok(started)
}

pub async fn stop_run(manager: &AgentRunManager, run_id: &str) -> Result<(), String> {
    let runtime = manager
        .runs
        .lock()
        .await
        .get(run_id)
        .cloned()
        .ok_or_else(|| "Agent is no longer running".to_string())?;
    runtime.cancel.store(true, Ordering::SeqCst);
    if let Some(job) = runtime.job.lock().await.take() {
        grok_process::close_job(job);
    }
    if let Some(child) = runtime.child.lock().await.as_mut() {
        let _ = child.kill().await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::valid_agent_name;

    #[test]
    fn validates_agent_names() {
        assert!(valid_agent_name("security-review"));
        assert!(valid_agent_name("reviewer_2"));
        assert!(!valid_agent_name("../escape"));
        assert!(!valid_agent_name("agent name"));
    }
}
