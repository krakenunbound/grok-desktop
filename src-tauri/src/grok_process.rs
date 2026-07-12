//! Spawn and stream the Grok Build CLI for headless chat turns.
//!
//! Each user message runs:
//!   grok -p <prompt> -m <model> --cwd <dir> [--always-approve]
//!        [--session-id <chat-id> | --resume <chat-id>]
//!
//! Stdout/stderr are streamed to the frontend via Tauri events.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Live session parameters (not the OS process — process is per-turn).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub model: String,
    pub yolo: bool,
    pub cwd: String,
    pub chat_id: Option<String>,
    /// After the first successful turn, subsequent turns use --continue.
    pub has_prior_turn: bool,
}

/// Status payload emitted to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrokStatus {
    pub state: String,
    pub detail: String,
    pub yolo: bool,
    pub model: String,
    pub running: bool,
}

/// Managed shared process state.
pub struct GrokManager {
    pub config: Mutex<Option<SessionConfig>>,
    pub child: Mutex<Option<Child>>,
    pub cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// Windows Job Object handle. Closing it terminates the entire tool tree.
    pub job: Mutex<Option<isize>>,
}

impl GrokManager {
    pub fn new() -> Self {
        Self {
            config: Mutex::new(None),
            child: Mutex::new(None),
            cancel: Mutex::new(None),
            job: Mutex::new(None),
        }
    }
}

#[cfg(windows)]
fn create_kill_on_close_job(process_id: u32) -> Result<isize, String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
    };

    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(format!(
                "Create process containment job: {}",
                std::io::Error::last_os_error()
            ));
        }

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of_val(&info) as u32,
        ) == 0
        {
            let error = std::io::Error::last_os_error();
            CloseHandle(job);
            return Err(format!("Configure process containment job: {error}"));
        }

        let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, process_id);
        if process.is_null() {
            let error = std::io::Error::last_os_error();
            CloseHandle(job);
            return Err(format!("Open Grok process for containment: {error}"));
        }
        let assigned = AssignProcessToJobObject(job, process);
        let assignment_error = std::io::Error::last_os_error();
        CloseHandle(process);
        if assigned == 0 {
            CloseHandle(job);
            return Err(format!(
                "Assign Grok process to containment job: {assignment_error}"
            ));
        }
        Ok(job as isize)
    }
}

#[cfg(not(windows))]
fn create_kill_on_close_job(_process_id: u32) -> Result<isize, String> {
    Ok(0)
}

#[cfg(windows)]
fn close_job(handle: isize) {
    use windows_sys::Win32::Foundation::CloseHandle;
    unsafe {
        CloseHandle(handle as *mut std::ffi::c_void);
    }
}

#[cfg(not(windows))]
fn close_job(_handle: isize) {}

impl Default for GrokManager {
    fn default() -> Self {
        Self::new()
    }
}

const MAX_PROMPT_CHARS: usize = 400_000;
const MAX_IMAGE_PATHS: usize = 16;
const MAX_MODEL_CHARS: usize = 64;

fn is_allowed_grok_binary_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|n| {
            let lower = n.to_ascii_lowercase();
            lower == "grok" || lower == "grok.exe"
        })
        .unwrap_or(false)
}

/// Resolve the `grok` executable: settings override → PATH → common install paths.
pub fn resolve_grok_binary(override_path: &str) -> Result<PathBuf, String> {
    if !override_path.trim().is_empty() {
        let p = PathBuf::from(override_path.trim());
        if p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err("grok binary path must not contain '..'".into());
        }
        let p = p
            .canonicalize()
            .map_err(|e| format!("Configured grok binary not found: {e}"))?;
        if !p.is_file() {
            return Err(format!("Configured grok binary not found: {override_path}"));
        }
        if !is_allowed_grok_binary_name(&p) {
            return Err(
                "Grok binary override must point to an executable named 'grok' or 'grok.exe'"
                    .into(),
            );
        }
        return Ok(p);
    }

    if let Ok(p) = which::which("grok") {
        return Ok(p);
    }

    // Windows user install used by Grok Build installer.
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join(".grok").join("bin").join("grok.exe");
        if candidate.is_file() {
            return Ok(candidate);
        }
        let candidate = home.join(".grok").join("bin").join("grok");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(
        "Could not find `grok` on PATH. Install Grok Build CLI and ensure it is available, or set the binary path in Settings.".to_string(),
    )
}

fn emit_status(app: &AppHandle, status: GrokStatus) {
    let _ = app.emit("grok-status", status);
}

fn push_arg_value(args: &mut Vec<String>, flag: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() {
        args.push(flag.into());
        args.push(value.into());
    }
}

fn push_repeated_lines(args: &mut Vec<String>, flag: &str, value: &str) {
    for line in value.lines().map(str::trim).filter(|line| !line.is_empty()) {
        args.push(flag.into());
        args.push(line.into());
    }
}

fn push_session_args(args: &mut Vec<String>, chat_id: Option<&str>, has_prior_turn: bool) {
    if let Some(chat_id) = chat_id {
        args.push(if has_prior_turn {
            "--resume".into()
        } else {
            "--session-id".into()
        });
        args.push(chat_id.into());
    } else if has_prior_turn {
        args.push("--continue".into());
    }
}

/// Map raw agent/CLI lines to high-level status labels (Agent Transparency Mode).
/// Prefer calm product language over tool/CLI jargon.
fn classify_status_line(line: &str) -> Option<&'static str> {
    let lower = line.to_lowercase();
    if lower.contains("always-approve") || lower.contains("yolo") {
        return Some("YOLO active");
    }
    if lower.contains("thinking") || lower.contains("reasoning") {
        return Some("Thinking…");
    }
    if lower.contains("plan mode") || lower.contains("planning") || lower.contains("plan:") {
        return Some("Planning…");
    }
    if lower.contains("diff")
        || lower.contains("applying")
        || lower.contains("writing file")
        || lower.contains("edited ")
        || lower.contains("search_replace")
        || lower.contains("write_file")
    {
        return Some("Executing changes…");
    }
    if lower.contains("tool call")
        || lower.contains("running tool")
        || lower.contains("invoking")
        || lower.contains("function call")
    {
        return Some("Running tools…");
    }
    if lower.contains("browser") || lower.contains("playwright") {
        return Some("Using browser…");
    }
    if (lower.contains("image") || lower.contains("imagine"))
        && (lower.contains("generat") || lower.contains("saved") || lower.contains("render"))
    {
        return Some("Generating image…");
    }
    if lower.contains("video") && lower.contains("generat") {
        return Some("Generating video…");
    }
    if lower.contains("generating") {
        return Some("Generating…");
    }
    if lower.contains("error") || lower.contains("failed") {
        return Some("Error");
    }
    None
}

/// Start (or reconfigure) a logical session. Does not spawn a long-lived process.
pub async fn start_session(
    app: AppHandle,
    manager: &GrokManager,
    model: String,
    yolo: bool,
    cwd: String,
    chat_id: Option<String>,
    grok_binary_override: &str,
) -> Result<SessionConfig, String> {
    // Validate binary early so the UI fails fast.
    let _ = resolve_grok_binary(grok_binary_override)?;

    if model.trim().is_empty() || model.len() > MAX_MODEL_CHARS {
        return Err("Invalid model id".into());
    }
    if model
        .chars()
        .any(|c| c.is_control() || c == ' ' || c == '"' || c == '\'')
    {
        return Err("Invalid model id characters".into());
    }

    let cwd_path = PathBuf::from(&cwd);
    let cwd_path = cwd_path
        .canonicalize()
        .map_err(|e| format!("Working directory does not exist: {cwd} ({e})"))?;
    if !cwd_path.is_dir() {
        return Err(format!("Working directory does not exist: {cwd}"));
    }
    let cwd = cwd_path.to_string_lossy().to_string();

    if let Some(ref id) = chat_id {
        crate::config::validate_id(id, "chat")?;
        uuid::Uuid::parse_str(id).map_err(|_| "Invalid chat id (expected UUID)".to_string())?;
    }

    // Preserve multi-turn continuity when cwd + chat stay the same.
    // Only cancel an in-flight child when the working directory changes.
    let prior = manager.config.lock().await.clone();
    let cwd_changed = prior.as_ref().map(|p| p.cwd != cwd).unwrap_or(true);
    let chat_changed = prior.as_ref().map(|p| p.chat_id != chat_id).unwrap_or(true);

    // Stop in-flight work when the workspace or chat thread changes.
    if cwd_changed || chat_changed {
        stop_session(manager).await?;
    }

    let has_prior_turn = match &prior {
        Some(p) if !cwd_changed && !chat_changed => p.has_prior_turn,
        _ => false,
    };

    let cfg = SessionConfig {
        model: model.clone(),
        yolo,
        cwd,
        chat_id,
        has_prior_turn,
    };
    *manager.config.lock().await = Some(cfg.clone());

    emit_status(
        &app,
        GrokStatus {
            state: "ready".into(),
            detail: if has_prior_turn {
                "Session ready (continuing)".into()
            } else {
                "Session ready".into()
            },
            yolo,
            model,
            running: false,
        },
    );

    Ok(cfg)
}

/// Kill the active child process if any.
pub async fn stop_session(manager: &GrokManager) -> Result<(), String> {
    if let Some(flag) = manager.cancel.lock().await.take() {
        flag.store(true, Ordering::SeqCst);
    }
    let mut child_guard = manager.child.lock().await;
    if let Some(mut child) = child_guard.take() {
        if let Some(job) = manager.job.lock().await.take() {
            close_job(job);
        }
        let _ = child.kill().await;
    }
    Ok(())
}

/// Run one headless Grok turn and stream output to the frontend.
pub async fn send_message(
    app: AppHandle,
    manager: State<'_, GrokManager>,
    prompt: String,
    image_paths: Vec<String>,
    grok_binary_override: String,
) -> Result<(), String> {
    if prompt.trim().is_empty() && image_paths.is_empty() {
        return Err("Empty message".into());
    }
    if prompt.len() > MAX_PROMPT_CHARS {
        return Err(format!(
            "Prompt too long (max {MAX_PROMPT_CHARS} characters)"
        ));
    }
    if image_paths.len() > MAX_IMAGE_PATHS {
        return Err(format!("Too many images (max {MAX_IMAGE_PATHS})"));
    }

    let cfg = manager
        .config
        .lock()
        .await
        .clone()
        .ok_or_else(|| "No active session. Call start_session first.".to_string())?;

    let binary = resolve_grok_binary(&grok_binary_override)?;

    // Only attach images that live under the managed temp_images directory
    // (prevents IPC from pointing the agent at arbitrary secret files).
    let settings_for_images = crate::config::load_settings();
    let temp_root = crate::config::temp_images_dir(&settings_for_images)?;
    let temp_root = temp_root.canonicalize().unwrap_or(temp_root);
    let mut safe_images: Vec<String> = Vec::new();
    for path in &image_paths {
        if path.len() > 4096 || path.contains('\0') {
            return Err("Invalid image path".into());
        }
        let p = PathBuf::from(path);
        let canon = p
            .canonicalize()
            .map_err(|e| format!("Image path not found: {e}"))?;
        if !canon.starts_with(&temp_root) {
            return Err("Image path must be under the app temp_images directory".into());
        }
        if !canon.is_file() {
            return Err("Image path is not a file".into());
        }
        safe_images.push(canon.to_string_lossy().to_string());
    }

    // Build prompt with attached image paths for Grok Build file awareness.
    // Chat history is persisted separately for UI restore. The CLI session is
    // keyed to the chat UUID so another terminal/session in the same cwd can
    // never steal this chat's continuation target.
    let mut full_prompt = String::new();
    if !safe_images.is_empty() {
        for path in &safe_images {
            full_prompt.push_str(&format!("Attached image: {path}\n"));
        }
        full_prompt.push('\n');
    }
    full_prompt.push_str(&prompt);

    // Args as discrete argv entries (no shell) — avoids command injection.
    let mut args: Vec<String> = vec![
        "-p".into(),
        full_prompt,
        "-m".into(),
        cfg.model.clone(),
        "--cwd".into(),
        cfg.cwd.clone(),
        "--output-format".into(),
        "plain".into(),
    ];

    let advanced = crate::config::load_settings();

    if cfg.yolo {
        args.push("--always-approve".into());
    }
    if !advanced.plan_mode {
        args.push("--no-plan".into());
    }
    if advanced.disable_web_search {
        args.push("--disable-web-search".into());
    }
    if !advanced.subagents_enabled {
        args.push("--no-subagents".into());
    }
    if advanced.memory_enabled {
        args.push("--experimental-memory".into());
    } else {
        args.push("--no-memory".into());
    }
    if advanced.permission_mode != "default" {
        args.push("--permission-mode".into());
        args.push(advanced.permission_mode);
    }
    push_arg_value(&mut args, "--tools", &advanced.tools);
    push_arg_value(&mut args, "--disallowed-tools", &advanced.disallowed_tools);
    push_arg_value(&mut args, "--max-turns", &advanced.max_turns);
    push_arg_value(&mut args, "--rules", &advanced.extra_rules);
    push_repeated_lines(&mut args, "--allow", &advanced.allow_rules);
    push_repeated_lines(&mut args, "--deny", &advanced.deny_rules);
    push_session_args(&mut args, cfg.chat_id.as_deref(), cfg.has_prior_turn);

    emit_status(
        &app,
        GrokStatus {
            state: "running".into(),
            detail: "Thinking…".into(),
            yolo: cfg.yolo,
            model: cfg.model.clone(),
            running: true,
        },
    );

    let mut command = Command::new(&binary);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    // On Windows, avoid flashing a console window for the child.
    #[cfg(windows)]
    {
        #[allow(unused_imports)]
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    // Lock order must match stop_session: cancel then child (avoid deadlock).
    let mut cancel_guard = manager.cancel.lock().await;
    let mut child_guard = manager.child.lock().await;
    if child_guard.is_some() {
        return Err("A Grok turn is already running. Stop it first or wait.".into());
    }

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to spawn grok ({}): {e}", binary.display()))?;

    let process_id = child
        .id()
        .ok_or_else(|| "Grok process did not expose a process id".to_string())?;
    let job = match create_kill_on_close_job(process_id) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill().await;
            return Err(error);
        }
    };

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture grok stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture grok stderr".to_string())?;

    let cancel = Arc::new(AtomicBool::new(false));
    *cancel_guard = Some(cancel.clone());
    *child_guard = Some(child);
    *manager.job.lock().await = Some(job);
    drop(child_guard);
    drop(cancel_guard);

    let status_yolo = cfg.yolo;
    let status_model = cfg.model.clone();

    let app_out = app.clone();
    let cancel_out = cancel.clone();
    let yolo_out = status_yolo;
    let model_out = status_model.clone();
    let out_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if cancel_out.load(Ordering::SeqCst) {
                break;
            }
            if let Some(label) = classify_status_line(&line) {
                let _ = app_out.emit(
                    "grok-status",
                    GrokStatus {
                        state: "running".into(),
                        detail: label.into(),
                        yolo: yolo_out,
                        model: model_out.clone(),
                        running: true,
                    },
                );
            }
            let _ = app_out.emit("grok-stdout", line);
        }
    });

    let app_err = app.clone();
    let cancel_err = cancel.clone();
    let yolo_err = status_yolo;
    let model_err = status_model.clone();
    let err_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if cancel_err.load(Ordering::SeqCst) {
                break;
            }
            if let Some(label) = classify_status_line(&line) {
                let _ = app_err.emit(
                    "grok-status",
                    GrokStatus {
                        state: "running".into(),
                        detail: label.into(),
                        yolo: yolo_err,
                        model: model_err.clone(),
                        running: true,
                    },
                );
            }
            let _ = app_err.emit("grok-stderr", line);
        }
    });

    // Poll for exit so stop_session can acquire the child lock and kill.
    let exit_code = loop {
        if cancel.load(Ordering::SeqCst) {
            break -1;
        }
        let mut child_guard = manager.child.lock().await;
        if let Some(child) = child_guard.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => break status.code().unwrap_or(-1),
                Ok(None) => {
                    drop(child_guard);
                    tokio::time::sleep(std::time::Duration::from_millis(40)).await;
                }
                Err(e) => {
                    let _ = app.emit("grok-stderr", format!("Process wait error: {e}"));
                    break -1;
                }
            }
        } else {
            // Child was taken/killed by stop_session.
            break -1;
        }
    };

    let _ = out_handle.await;
    let _ = err_handle.await;

    *manager.child.lock().await = None;
    *manager.cancel.lock().await = None;
    if let Some(job) = manager.job.lock().await.take() {
        close_job(job);
    }

    let cancelled = cancel.load(Ordering::SeqCst);
    let success = !cancelled && exit_code == 0;

    // Only flip continuity flag — do not clobber concurrent YOLO/model updates.
    if success {
        if let Some(live) = manager.config.lock().await.as_mut() {
            live.has_prior_turn = true;
        }
    }

    let live = manager.config.lock().await.clone();
    let yolo_now = live.as_ref().map(|c| c.yolo).unwrap_or(cfg.yolo);
    let model_now = live
        .as_ref()
        .map(|c| c.model.clone())
        .unwrap_or_else(|| cfg.model.clone());

    let _ = app.emit(
        "grok-done",
        serde_json::json!({
            "exit_code": exit_code,
            "cancelled": cancelled,
            "success": success,
        }),
    );

    emit_status(
        &app,
        GrokStatus {
            state: if cancelled {
                "cancelled".into()
            } else if success {
                "ready".into()
            } else {
                "error".into()
            },
            detail: if cancelled {
                "Cancelled".into()
            } else if success {
                "Done".into()
            } else {
                format!("Exit code {exit_code}")
            },
            yolo: yolo_now,
            model: model_now,
            running: false,
        },
    );

    if cancelled {
        return Err("Cancelled".into());
    }
    if exit_code != 0 {
        return Err(format!("Grok exited with code {exit_code}"));
    }
    Ok(())
}

/// Update YOLO flag on the current session without dropping multi-turn continuity
/// unless the caller chooses to restart.
pub async fn set_yolo(manager: &GrokManager, yolo: bool) -> Result<SessionConfig, String> {
    let mut guard = manager.config.lock().await;
    let cfg = guard
        .as_mut()
        .ok_or_else(|| "No active session".to_string())?;
    cfg.yolo = yolo;
    Ok(cfg.clone())
}

/// Update model on the current session.
pub async fn set_model(manager: &GrokManager, model: String) -> Result<SessionConfig, String> {
    if model.trim().is_empty() || model.len() > MAX_MODEL_CHARS {
        return Err("Invalid model id".into());
    }
    if model
        .chars()
        .any(|c| c.is_control() || c == ' ' || c == '"' || c == '\'')
    {
        return Err("Invalid model id characters".into());
    }
    let mut guard = manager.config.lock().await;
    let cfg = guard
        .as_mut()
        .ok_or_else(|| "No active session".to_string())?;
    cfg.model = model;
    Ok(cfg.clone())
}

/// Read current session config if any.
pub async fn current_session(manager: &GrokManager) -> Option<SessionConfig> {
    manager.config.lock().await.clone()
}

/// Whether a turn is currently running.
pub async fn is_running(manager: &GrokManager) -> bool {
    manager.child.lock().await.is_some()
}

#[cfg(test)]
mod tests {
    use super::push_session_args;

    const CHAT_ID: &str = "019f5829-8992-7622-8c5d-72e56c32e489";

    #[test]
    fn new_chat_gets_deterministic_session_id() {
        let mut args = Vec::new();
        push_session_args(&mut args, Some(CHAT_ID), false);
        assert_eq!(args, ["--session-id", CHAT_ID]);
    }

    #[test]
    fn existing_chat_resumes_its_own_session() {
        let mut args = Vec::new();
        push_session_args(&mut args, Some(CHAT_ID), true);
        assert_eq!(args, ["--resume", CHAT_ID]);
    }

    #[test]
    fn id_less_legacy_session_uses_continue() {
        let mut args = Vec::new();
        push_session_args(&mut args, None, true);
        assert_eq!(args, ["--continue"]);
    }
}
