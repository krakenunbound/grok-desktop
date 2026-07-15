use crate::grok_process;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct GrokCliSession {
    pub id: String,
    pub created: String,
    pub updated: String,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrokCliWorktree {
    pub id: String,
    pub name: String,
    pub path: String,
    pub branch: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrokCliCapability {
    pub name: String,
    pub detail: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrokCliOverview {
    pub version: String,
    pub commit: String,
    pub channel: String,
    pub compatibility: String,
    pub sessions: Vec<GrokCliSession>,
    pub worktrees: Vec<GrokCliWorktree>,
    pub capabilities: Vec<GrokCliCapability>,
    pub errors: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct GrokCliIdentity {
    version: String,
    commit: String,
    channel: String,
}

fn parse_cli_identity(text: &str) -> GrokCliIdentity {
    let mut parts = text.split_whitespace();
    let _name = parts.next();
    let version = parts.next().unwrap_or("unknown").to_string();
    let commit = parts
        .next()
        .unwrap_or_default()
        .trim_matches(['(', ')'])
        .to_string();
    let channel = parts
        .next()
        .unwrap_or_default()
        .trim_matches(['[', ']'])
        .to_string();
    GrokCliIdentity {
        version,
        commit,
        channel,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokUpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub installer: Option<String>,
    pub channel: String,
    pub auto_update: bool,
    pub error: Option<String>,
}

async fn run_update_command(
    grok_binary_override: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<Output, String> {
    let binary = grok_process::resolve_grok_binary(grok_binary_override)?;
    let mut command = tokio::process::Command::new(binary);
    command
        .args(args)
        .current_dir(dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    crate::privacy::apply_process_guard(&mut command);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let child = command
        .spawn()
        .map_err(|error| format!("Start Grok CLI updater: {error}"))?;
    let process_id = child
        .id()
        .ok_or_else(|| "Grok CLI updater did not expose a process ID".to_string())?;
    let job = grok_process::create_kill_on_close_job(process_id)?;
    let result = tokio::time::timeout(timeout, child.wait_with_output()).await;
    grok_process::close_job(job);

    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => Err(format!("Wait for Grok CLI updater: {error}")),
        Err(_) => Err(format!(
            "Grok CLI updater did not finish within {} seconds",
            timeout.as_secs()
        )),
    }
}

pub async fn check_update(grok_binary_override: &str) -> Result<GrokUpdateStatus, String> {
    let output = run_update_command(
        grok_binary_override,
        &["update", "--check", "--json"],
        Duration::from_secs(20),
    )
    .await?;
    if !output.status.success() {
        return Err(format!(
            "Grok CLI update check exited with {}",
            output.status
        ));
    }
    let status = parse_update_status(&output.stdout)?;
    if let Some(error) = status.error.as_deref().filter(|value| !value.is_empty()) {
        return Err(format!("Grok CLI update check: {error}"));
    }
    Ok(status)
}

pub async fn install_update(grok_binary_override: &str) -> Result<GrokUpdateStatus, String> {
    let before = check_update(grok_binary_override).await?;
    if !before.update_available {
        return Ok(before);
    }
    let output =
        run_update_command(grok_binary_override, &["update"], Duration::from_secs(180)).await?;
    if !output.status.success() {
        return Err(format!("Grok CLI update exited with {}", output.status));
    }
    let after = check_update(grok_binary_override).await?;
    if after.current_version == before.current_version {
        return Err("Grok CLI updater finished but the installed version did not change".into());
    }
    Ok(after)
}

fn parse_update_status(bytes: &[u8]) -> Result<GrokUpdateStatus, String> {
    serde_json::from_slice(bytes).map_err(|error| format!("Read Grok CLI update status: {error}"))
}

fn command_cwd(cwd: Option<&str>) -> Result<PathBuf, String> {
    match cwd.map(str::trim).filter(|v| !v.is_empty()) {
        Some(value) => {
            if value.contains('\0') {
                return Err("Invalid working directory".into());
            }
            let path = PathBuf::from(value)
                .canonicalize()
                .map_err(|e| format!("Working directory does not exist: {value} ({e})"))?;
            if !path.is_dir() {
                return Err(format!("Working directory is not a folder: {value}"));
            }
            Ok(path)
        }
        None => std::env::current_dir().map_err(|e| format!("resolve current directory: {e}")),
    }
}

pub(crate) fn run_grok(
    grok_binary_override: &str,
    cwd: Option<&PathBuf>,
    args: &[&str],
) -> Result<Output, String> {
    let binary = grok_process::resolve_grok_binary(grok_binary_override)?;
    let cwd = match cwd {
        Some(path) => command_cwd(path.to_str())?,
        None => command_cwd(None)?,
    };
    let mut command = Command::new(binary);
    command.args(args).current_dir(cwd);

    // Some Grok subcommands expect HOME even on Windows. Tauri-launched
    // children do not always inherit it, so set stable local defaults.
    if let Some(home) = dirs::home_dir() {
        if std::env::var_os("HOME").is_none() {
            command.env("HOME", &home);
        }
        if std::env::var_os("USERPROFILE").is_none() {
            command.env("USERPROFILE", &home);
        }
        if std::env::var_os("GROK_HOME").is_none() {
            command.env("GROK_HOME", home.join(".grok"));
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.output().map_err(|e| format!("run grok: {e}"))
}

fn output_text(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = format!("{}{}", stdout.trim(), stderr.trim());
    text.trim().to_string()
}

fn parse_sessions(text: &str) -> Vec<GrokCliSession> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("(no label)")
                && !line.starts_with("SESSION ID")
                && !line.starts_with("Error:")
        })
        .filter_map(|line| {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            if parts.len() < 5 {
                return None;
            }
            Some(GrokCliSession {
                id: parts[0].to_string(),
                created: parts[1].to_string(),
                updated: parts[2].to_string(),
                status: parts[3].to_string(),
                summary: parts[4..].join(" "),
            })
        })
        .collect()
}

fn value_string(item: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| item.get(*key).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn parse_worktrees(bytes: &[u8]) -> Result<Vec<GrokCliWorktree>, String> {
    let value =
        serde_json::from_slice::<Value>(bytes).map_err(|e| format!("parse worktrees: {e}"))?;
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };

    Ok(items
        .iter()
        .map(|item| {
            let id = value_string(item, &["id", "worktree_id", "uuid"]);
            let path = value_string(item, &["path", "worktree_path", "dir"]);
            let name = value_string(item, &["name", "label", "branch"]);
            GrokCliWorktree {
                id,
                name,
                path,
                branch: value_string(item, &["branch", "ref", "head"]),
                status: value_string(item, &["status", "state"]),
            }
        })
        .collect())
}

fn capabilities() -> Vec<GrokCliCapability> {
    [
        (
            "Persistent agent transports",
            "grok agent stdio / serve / leader",
        ),
        (
            "Sessions",
            "grok sessions list/search/delete plus resume/continue",
        ),
        (
            "Worktrees",
            "grok --worktree and grok worktree list/show/rm/gc",
        ),
        ("MCP", "grok mcp list/add/remove/doctor"),
        (
            "Plugins",
            "grok plugin install/update/enable/disable/details",
        ),
        (
            "Memory",
            "--experimental-memory / --no-memory / grok memory clear",
        ),
        (
            "Structured output",
            "--output-format json/streaming-json and --json-schema",
        ),
        ("Traces", "grok trace and grok export"),
        ("Dashboard", "grok dashboard"),
    ]
    .into_iter()
    .map(|(name, detail)| GrokCliCapability {
        name: name.into(),
        detail: detail.into(),
        available: true,
    })
    .collect()
}

pub fn overview(grok_binary_override: &str, cwd: Option<&str>) -> Result<GrokCliOverview, String> {
    let mut errors = Vec::new();

    let identity = run_grok(grok_binary_override, None, &["--version"])
        .ok()
        .filter(|output| output.status.success())
        .map(|output| parse_cli_identity(&output_text(&output)))
        .unwrap_or_default();

    let resolved_cwd = cwd.map(PathBuf::from);
    let sessions = match run_grok(
        grok_binary_override,
        resolved_cwd.as_ref(),
        &["sessions", "list", "-n", "8"],
    ) {
        Ok(output) if output.status.success() => parse_sessions(&output_text(&output)),
        Ok(output) => {
            errors.push(format!("sessions list failed: {}", output_text(&output)));
            Vec::new()
        }
        Err(e) => {
            errors.push(e);
            Vec::new()
        }
    };

    let worktrees = match run_grok(
        grok_binary_override,
        resolved_cwd.as_ref(),
        &["worktree", "list", "--json", "--all"],
    ) {
        Ok(output) if output.status.success() => match parse_worktrees(&output.stdout) {
            Ok(items) => items,
            Err(e) => {
                errors.push(e);
                Vec::new()
            }
        },
        Ok(output) => {
            errors.push(format!("worktree list failed: {}", output_text(&output)));
            Vec::new()
        }
        Err(e) => {
            errors.push(e);
            Vec::new()
        }
    };

    Ok(GrokCliOverview {
        version: identity.version,
        commit: identity.commit,
        channel: identity.channel,
        compatibility: "ACP v1 · direct billing/privacy · legacy fallbacks".into(),
        sessions,
        worktrees,
        capabilities: capabilities(),
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_cli_identity, parse_sessions, parse_update_status, parse_worktrees};

    #[test]
    fn parses_grok_cli_identity() {
        let identity = parse_cli_identity("grok 0.2.101 (5bc4b5dfad) [stable]");
        assert_eq!(identity.version, "0.2.101");
        assert_eq!(identity.commit, "5bc4b5dfad");
        assert_eq!(identity.channel, "stable");
    }

    #[test]
    fn parses_machine_readable_update_status() {
        let status = parse_update_status(
            br#"{"currentVersion":"0.2.99","latestVersion":"0.2.101","updateAvailable":true,"installer":"internal","channel":"stable","autoUpdate":true,"error":null}"#,
        )
        .expect("update status should parse");

        assert_eq!(status.current_version, "0.2.99");
        assert_eq!(status.latest_version.as_deref(), Some("0.2.101"));
        assert!(status.update_available);
        assert_eq!(status.channel, "stable");
        assert!(status.auto_update);
    }

    #[test]
    fn parses_upstream_update_failure_contract() {
        let status = parse_update_status(
            br#"{"currentVersion":"0.2.99","latestVersion":null,"updateAvailable":false,"installer":"npm","channel":"stable","autoUpdate":true,"error":"npm view @latest failed: E403"}"#,
        )
        .expect("nullable upstream fields should parse");

        assert!(status.latest_version.is_none());
        assert_eq!(status.installer.as_deref(), Some("npm"));
        assert!(status.error.as_deref().unwrap_or_default().contains("E403"));
    }

    #[test]
    fn parses_sessions_table_with_summary_spaces() {
        let text = r#"
(no label)
SESSION ID                            CREATED     UPDATED     STATUS      SUMMARY
019f4477-a355-78d2-8251-faa2a94b6f21  2026-07-09  2026-07-09  remote  Software Project Codebase Audit and Feedback
019f443a-620d-7a62-b64b-e738d69a06e8  2026-07-09  2026-07-09  local  Fix Tauri Release Build
"#;

        let sessions = parse_sessions(text);

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, "019f4477-a355-78d2-8251-faa2a94b6f21");
        assert_eq!(sessions[0].status, "remote");
        assert_eq!(
            sessions[0].summary,
            "Software Project Codebase Audit and Feedback"
        );
        assert_eq!(sessions[1].summary, "Fix Tauri Release Build");
    }

    #[test]
    fn parses_worktree_json_with_alternate_field_names() {
        let raw = br#"[
          {
            "worktree_id": "wt-1",
            "label": "feature-shell",
            "worktree_path": "F:\\repo\\.grok\\worktrees\\feature-shell",
            "ref": "main",
            "state": "ready"
          }
        ]"#;

        let worktrees = parse_worktrees(raw).expect("worktree JSON should parse");

        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].id, "wt-1");
        assert_eq!(worktrees[0].name, "feature-shell");
        assert_eq!(worktrees[0].branch, "main");
        assert_eq!(worktrees[0].status, "ready");
    }
}
