//! Local privacy audit and safeguards for Grok Build processes.

use crate::config;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use toml_edit::{value, DocumentMut};

const UPLOAD_START: &str = "repo_state.upload.start";
const UPLOAD_ENQUEUED: &str = "repo_state.upload.enqueued";

#[derive(Debug, Clone, Serialize, Default)]
pub struct RepositoryUploadSummary {
    pub path: String,
    pub events: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PrivacyAudit {
    pub guard_enabled: bool,
    pub account_retention_opt_out: Option<bool>,
    pub telemetry_disabled_in_config: bool,
    pub trace_upload_disabled_in_config: bool,
    pub log_exists: bool,
    pub log_bytes: u64,
    pub upload_start_events: u64,
    pub upload_enqueued_events: u64,
    pub upload_bytes: u64,
    pub largest_upload_bytes: u64,
    pub first_upload_at: Option<String>,
    pub last_upload_at: Option<String>,
    pub repositories: Vec<RepositoryUploadSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectPathRisk {
    pub risky: bool,
    pub severity: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
struct LogContext {
    repo_path: Option<String>,
    size_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct LogEvent {
    ts: Option<String>,
    msg: Option<String>,
    ctx: Option<LogContext>,
}

fn grok_home() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("GROK_HOME") {
        return Ok(PathBuf::from(home));
    }
    dirs::home_dir()
        .map(|home| home.join(".grok"))
        .ok_or_else(|| "Could not locate the Grok home folder".to_string())
}

pub fn log_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("GROK_LOG_FILE") {
        return Ok(PathBuf::from(path));
    }
    Ok(grok_home()?.join("logs").join("unified.jsonl"))
}

fn config_path() -> Result<PathBuf, String> {
    Ok(grok_home()?.join("config.toml"))
}

fn account_retention_opt_out() -> Option<bool> {
    let path = grok_home().ok()?.join("auth.json");
    let raw = fs::read_to_string(path).ok()?;
    let root: Value = serde_json::from_str(&raw).ok()?;
    root.as_object()?.values().find_map(|entry| {
        entry
            .get("coding_data_retention_opt_out")
            .and_then(Value::as_bool)
    })
}

fn expected_retention_confirmation(opt_out: bool) -> &'static str {
    if opt_out {
        "DELETE PREVIOUSLY SYNCED DATA"
    } else {
        "ALLOW FUTURE DATA RETENTION"
    }
}

pub async fn set_coding_data_retention(
    grok_binary_override: &str,
    opt_out: bool,
    confirmation: &str,
) -> Result<(), String> {
    if confirmation != expected_retention_confirmation(opt_out) {
        return Err("Data-retention confirmation did not match".into());
    }
    if account_retention_opt_out() == Some(opt_out) {
        return Ok(());
    }

    let binary = crate::grok_process::resolve_grok_binary(grok_binary_override)?;
    let direct = crate::grok_acp::request_extension(
        &binary,
        "x.ai/privacy/setCodingDataRetention",
        serde_json::json!({"codingDataRetentionOptOut": opt_out}),
        Duration::from_secs(20),
    )
    .await;
    if let Ok(response) = direct {
        let confirmed = response
            .get("codingDataRetentionOptOut")
            .and_then(Value::as_bool);
        if confirmed != Some(opt_out) {
            return Err("Grok returned an unexpected data-retention state".into());
        }
        // The extension persists the account cache after the server accepts
        // the change. Give that atomic save a short window to become visible.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while tokio::time::Instant::now() < deadline {
            if account_retention_opt_out() == Some(opt_out) {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(75)).await;
        }
        return Err(
            "Grok changed data retention but did not update its local account state".into(),
        );
    }

    // Compatibility path for Grok versions predating the privacy ACP
    // extension. Keep this until the minimum supported CLI version advances.
    set_coding_data_retention_legacy(&binary, opt_out).await
}

async fn set_coding_data_retention_legacy(binary: &Path, opt_out: bool) -> Result<(), String> {
    let slash_command = if opt_out {
        "/privacy opt-out"
    } else {
        "/privacy opt-in"
    };
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("--no-alt-screen")
        .arg(slash_command)
        .current_dir(dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    apply_process_guard(&mut command);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("Start Grok privacy command: {error}"))?;
    let process_id = child
        .id()
        .ok_or_else(|| "Grok privacy command did not expose a process ID".to_string())?;
    let job = crate::grok_process::create_kill_on_close_job(process_id)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let result = loop {
        if account_retention_opt_out() == Some(opt_out) {
            break Ok(());
        }
        match child.try_wait() {
            Ok(Some(status)) => break Err(format!("Grok privacy command exited with {status}")),
            Ok(None) => {}
            Err(error) => break Err(format!("Check Grok privacy command: {error}")),
        }
        if tokio::time::Instant::now() >= deadline {
            break Err("Grok did not confirm the data-retention change within 30 seconds".into());
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    };
    crate::grok_process::close_job(job);
    let _ = child.kill().await;
    let _ = child.wait().await;
    result
}

fn config_privacy_state() -> (bool, bool) {
    let Ok(path) = config_path() else {
        return (false, false);
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return (false, false);
    };
    let Ok(document) = raw.parse::<DocumentMut>() else {
        return (false, false);
    };
    let telemetry_disabled = document
        .get("features")
        .and_then(|item| item.get("telemetry"))
        .and_then(|item| item.as_bool())
        == Some(false);
    let trace_disabled = document
        .get("telemetry")
        .and_then(|item| item.get("trace_upload"))
        .and_then(|item| item.as_bool())
        == Some(false);
    (telemetry_disabled, trace_disabled)
}

pub fn audit(guard_enabled: bool) -> PrivacyAudit {
    let path = log_path().ok();
    let metadata = path.as_ref().and_then(|path| fs::metadata(path).ok());
    let (telemetry_disabled, trace_disabled) = config_privacy_state();
    let mut result = PrivacyAudit {
        guard_enabled,
        account_retention_opt_out: account_retention_opt_out(),
        telemetry_disabled_in_config: telemetry_disabled,
        trace_upload_disabled_in_config: trace_disabled,
        log_exists: metadata.is_some(),
        log_bytes: metadata.map(|value| value.len()).unwrap_or(0),
        ..PrivacyAudit::default()
    };
    let Some(path) = path.filter(|path| path.is_file()) else {
        return result;
    };
    let Ok(file) = File::open(path) else {
        return result;
    };
    let mut repositories: HashMap<String, RepositoryUploadSummary> = HashMap::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains("repo_state.upload") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<LogEvent>(&line) else {
            continue;
        };
        let message = event.msg.as_deref().unwrap_or_default();
        if message == UPLOAD_START {
            result.upload_start_events += 1;
        } else if message == UPLOAD_ENQUEUED {
            result.upload_enqueued_events += 1;
        } else {
            continue;
        }
        if let Some(timestamp) = event.ts {
            if result.first_upload_at.is_none() {
                result.first_upload_at = Some(timestamp.clone());
            }
            result.last_upload_at = Some(timestamp);
        }
        let Some(context) = event.ctx else {
            continue;
        };
        let bytes = if message == UPLOAD_ENQUEUED {
            context.size_bytes.unwrap_or(0)
        } else {
            0
        };
        result.upload_bytes = result.upload_bytes.saturating_add(bytes);
        result.largest_upload_bytes = result.largest_upload_bytes.max(bytes);
        if let Some(repo_path) = context.repo_path.filter(|value| !value.is_empty()) {
            let entry =
                repositories
                    .entry(repo_path.clone())
                    .or_insert_with(|| RepositoryUploadSummary {
                        path: repo_path,
                        ..RepositoryUploadSummary::default()
                    });
            entry.events += 1;
            entry.bytes = entry.bytes.saturating_add(bytes);
        }
    }
    result.repositories = repositories.into_values().collect();
    result.repositories.sort_by(|left, right| {
        right
            .bytes
            .cmp(&left.bytes)
            .then_with(|| left.path.cmp(&right.path))
    });
    result
}

pub fn apply_grok_config_protection() -> Result<(), String> {
    let path = config_path()?;
    let raw = if path.is_file() {
        fs::read_to_string(&path).map_err(|error| format!("Read Grok config: {error}"))?
    } else {
        String::new()
    };
    let mut document = raw
        .parse::<DocumentMut>()
        .map_err(|error| format!("Parse Grok config: {error}"))?;
    let already_protected = document["features"]["telemetry"].as_bool() == Some(false)
        && document["telemetry"]["trace_upload"].as_bool() == Some(false)
        && document["telemetry"]["mixpanel_enabled"].as_bool() == Some(false);
    if already_protected {
        return Ok(());
    }
    if path.is_file() {
        let backup = path.with_file_name(format!(
            "config.privacy-backup-{}.toml",
            Utc::now().format("%Y%m%d-%H%M%S")
        ));
        fs::copy(&path, backup).map_err(|error| format!("Back up Grok config: {error}"))?;
    }
    document["features"]["telemetry"] = value(false);
    document["telemetry"]["trace_upload"] = value(false);
    document["telemetry"]["mixpanel_enabled"] = value(false);
    config::write_text_atomic(&path, &document.to_string())
        .map_err(|error| format!("Save Grok privacy settings: {error}"))
}

pub fn export_report(destination: &str, guard_enabled: bool) -> Result<(), String> {
    let destination = PathBuf::from(destination);
    if destination
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("Export path must not contain '..'".into());
    }
    let audit = audit(guard_enabled);
    let mut report = format!(
        "# Grok Desktop Privacy Audit\n\nGenerated: {}\n\n- Privacy Guard: {}\n- Account coding-data retention opt-out: {}\n- Telemetry disabled in config: {}\n- Trace upload disabled in config: {}\n- Upload start events: {}\n- Upload enqueued events: {}\n- Logged upload bytes: {}\n- Largest logged upload: {}\n- First upload event: {}\n- Last upload event: {}\n\n## Repositories\n\n",
        Utc::now().to_rfc3339(),
        audit.guard_enabled,
        audit
            .account_retention_opt_out
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".into()),
        audit.telemetry_disabled_in_config,
        audit.trace_upload_disabled_in_config,
        audit.upload_start_events,
        audit.upload_enqueued_events,
        audit.upload_bytes,
        audit.largest_upload_bytes,
        audit.first_upload_at.as_deref().unwrap_or("none"),
        audit.last_upload_at.as_deref().unwrap_or("none"),
    );
    if audit.repositories.is_empty() {
        report.push_str("No repository upload events found.\n");
    } else {
        for repository in audit.repositories {
            report.push_str(&format!(
                "- `{}` — {} events, {} bytes\n",
                repository.path.replace('`', "'"),
                repository.events,
                repository.bytes
            ));
        }
    }
    config::write_text_atomic(&destination, &report)
        .map_err(|error| format!("Export privacy report: {error}"))
}

pub fn archive_and_clear_logs(confirmation: &str) -> Result<PathBuf, String> {
    if confirmation != "ARCHIVE AND CLEAR" {
        return Err("Confirmation phrase did not match".into());
    }
    let path = log_path()?;
    if !path.is_file() {
        return Err("No Grok log file was found".into());
    }
    let archive_dir = config::app_data_dir()?.join("privacy_archives");
    fs::create_dir_all(&archive_dir)
        .map_err(|error| format!("Create privacy archive folder: {error}"))?;
    let archive = archive_dir.join(format!(
        "unified-{}.jsonl",
        Utc::now().format("%Y%m%d-%H%M%S")
    ));
    fs::copy(&path, &archive).map_err(|error| format!("Archive Grok logs: {error}"))?;
    OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .map_err(|error| format!("Clear Grok logs: {error}"))?;
    Ok(archive)
}

pub fn assess_project_path(path: &str) -> Result<ProjectPathRisk, String> {
    let path = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| format!("Project folder does not exist: {error}"))?;
    let home = dirs::home_dir().and_then(|home| home.canonicalize().ok());
    let root_like = path.parent().is_none() || path.parent().and_then(Path::parent).is_none();
    if home.as_ref().is_some_and(|home| home == &path) {
        return Ok(ProjectPathRisk {
            risky: true,
            severity: "critical".into(),
            reason: "This is your entire home folder. It may contain SSH keys, credentials, documents, and unrelated private data.".into(),
        });
    }
    if root_like
        || home
            .as_ref()
            .and_then(|home| home.parent())
            .is_some_and(|users| users == path)
    {
        return Ok(ProjectPathRisk {
            risky: true,
            severity: "critical".into(),
            reason: "This folder is unusually broad and may expose unrelated projects and private files to Grok.".into(),
        });
    }
    for sensitive in [".ssh", ".gnupg", "AppData"] {
        if path.join(sensitive).exists() {
            return Ok(ProjectPathRisk {
                risky: true,
                severity: "warning".into(),
                reason: format!("This folder contains a sensitive '{sensitive}' directory. Choose a narrower project folder when possible."),
            });
        }
    }
    Ok(ProjectPathRisk {
        risky: false,
        severity: "safe".into(),
        reason: String::new(),
    })
}

pub fn upload_log_cursor() -> u64 {
    log_path()
        .ok()
        .and_then(|path| fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

pub fn upload_started_since(cursor: &mut u64) -> bool {
    let Ok(path) = log_path() else {
        return false;
    };
    upload_started_in_file(&path, cursor)
}

fn upload_started_in_file(path: &Path, cursor: &mut u64) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let length = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    if length < *cursor {
        *cursor = 0;
    }
    if length <= *cursor {
        return false;
    }
    let previous_cursor = *cursor;
    // Keep a small overlap so an event name split across two writes cannot evade detection.
    let read_from = cursor.saturating_sub(UPLOAD_ENQUEUED.len() as u64);
    if file.seek(SeekFrom::Start(read_from)).is_err() {
        return false;
    }
    let mut new_text = String::new();
    if file.read_to_string(&mut new_text).is_err() {
        return false;
    }
    *cursor = length;
    [UPLOAD_START, UPLOAD_ENQUEUED].iter().any(|needle| {
        new_text
            .match_indices(needle)
            .any(|(offset, _)| read_from + offset as u64 + needle.len() as u64 > previous_cursor)
    })
}

pub fn apply_process_guard(command: &mut tokio::process::Command) {
    command
        .env("GROK_TELEMETRY_ENABLED", "0")
        .env("GROK_TELEMETRY_TRACE_UPLOAD", "0")
        .env("GROK_TELEMETRY_MIXPANEL_ENABLED", "0");
}

#[cfg(test)]
mod tests {
    use super::{assess_project_path, expected_retention_confirmation, upload_started_in_file};
    use std::fs;
    use std::io::Write;

    #[test]
    fn normal_workspace_is_not_broad() {
        let current = std::env::current_dir().expect("current directory");
        let risk = assess_project_path(current.to_string_lossy().as_ref()).expect("path risk");
        assert_ne!(risk.severity, "critical");
    }

    #[test]
    fn home_folder_is_critical() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let risk = assess_project_path(home.to_string_lossy().as_ref()).expect("home path risk");
        assert_eq!(risk.severity, "critical");
    }

    #[test]
    fn upload_monitor_detects_an_event_split_across_writes() {
        let path = std::env::temp_dir().join(format!(
            "grok-desktop-privacy-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        fs::write(&path, r#"{"msg":"repo_state.upload.enqu"#).expect("first log write");
        let mut cursor = fs::metadata(&path).expect("log metadata").len();
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open test log for append");
        file.write_all(b"eued\"}\n").expect("completed log write");
        drop(file);
        assert!(upload_started_in_file(&path, &mut cursor));
        fs::remove_file(path).expect("remove test log");
    }

    #[test]
    fn upload_monitor_does_not_redetect_a_completed_old_event() {
        let path = std::env::temp_dir().join(format!(
            "grok-desktop-privacy-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        fs::write(&path, r#"{"msg":"repo_state.upload.enqueued"}\n"#).expect("initial log");
        let mut cursor = fs::metadata(&path).expect("log metadata").len();
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open test log for append");
        file.write_all(b"{\"msg\":\"ordinary.event\"}\n")
            .expect("append unrelated event");
        drop(file);
        assert!(!upload_started_in_file(&path, &mut cursor));
        fs::remove_file(path).expect("remove test log");
    }

    #[test]
    fn retention_changes_require_explicit_directional_confirmation() {
        assert_eq!(
            expected_retention_confirmation(true),
            "DELETE PREVIOUSLY SYNCED DATA"
        );
        assert_eq!(
            expected_retention_confirmation(false),
            "ALLOW FUTURE DATA RETENTION"
        );
    }
}
