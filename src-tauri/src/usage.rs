//! Read-only Grok subscription/credit usage from the CLI's local telemetry log.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

const MAX_LOG_TAIL_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub available: bool,
    pub usage_percent: f64,
    pub remaining_percent: f64,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub prepaid_balance_cents: i64,
    pub on_demand_used_cents: i64,
    pub on_demand_cap_cents: i64,
    pub subscription_tier: Option<String>,
    pub updated_at: Option<String>,
    pub source: String,
    pub detail: String,
}

impl Default for UsageSnapshot {
    fn default() -> Self {
        Self {
            available: false,
            usage_percent: 0.0,
            remaining_percent: 0.0,
            period_start: None,
            period_end: None,
            prepaid_balance_cents: 0,
            on_demand_used_cents: 0,
            on_demand_cap_cents: 0,
            subscription_tier: None,
            updated_at: None,
            source: "Grok CLI telemetry".into(),
            detail: "No Grok billing telemetry is available yet.".into(),
        }
    }
}

pub fn load_usage() -> UsageSnapshot {
    load_usage_from_path(usage_log_path())
}

pub async fn refresh_usage(grok_binary_override: &str) -> Result<UsageSnapshot, String> {
    let before = load_usage();
    let previous_update = before.updated_at.clone();
    let binary = crate::grok_process::resolve_grok_binary(grok_binary_override)?;
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("--no-alt-screen")
        .current_dir(dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    crate::privacy::apply_process_guard(&mut command);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("Start Grok usage refresh: {error}"))?;
    let process_id = child
        .id()
        .ok_or_else(|| "Grok usage refresh did not expose a process ID".to_string())?;
    let job = crate::grok_process::create_kill_on_close_job(process_id)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    let snapshot = loop {
        let current = load_usage();
        if current.available && current.updated_at != previous_update {
            break Some(current);
        }
        match child.try_wait() {
            Ok(Some(_)) => break None,
            Ok(None) => {}
            Err(error) => {
                crate::grok_process::close_job(job);
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(format!("Check Grok usage refresh: {error}"));
            }
        }
        if tokio::time::Instant::now() >= deadline {
            break None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };

    crate::grok_process::close_job(job);
    let _ = child.kill().await;
    let _ = child.wait().await;
    Ok(snapshot.unwrap_or_else(|| UsageSnapshot {
        detail: "Grok did not publish a fresh billing snapshot during startup.".into(),
        ..UsageSnapshot::default()
    }))
}

fn usage_log_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".grok").join("logs").join("unified.jsonl"))
}

fn load_usage_from_path(path: Option<PathBuf>) -> UsageSnapshot {
    let Some(path) = path else {
        return UsageSnapshot::default();
    };
    let Ok(mut file) = File::open(&path) else {
        return UsageSnapshot::default();
    };
    let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
        return UsageSnapshot::default();
    };
    let start = length.saturating_sub(MAX_LOG_TAIL_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return UsageSnapshot::default();
    }
    let mut raw = String::new();
    if file.read_to_string(&mut raw).is_err() {
        return UsageSnapshot::default();
    }

    for line in raw.lines().rev() {
        if !line.contains("billing: fetched credits config") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(config) = event.pointer("/ctx/config") else {
            continue;
        };
        let usage_percent = number(config.get("creditUsagePercent")).clamp(0.0, 100.0);
        return UsageSnapshot {
            available: true,
            usage_percent,
            remaining_percent: (100.0 - usage_percent).clamp(0.0, 100.0),
            period_start: string(config.pointer("/currentPeriod/start"))
                .or_else(|| string(config.get("billingPeriodStart"))),
            period_end: string(config.pointer("/currentPeriod/end"))
                .or_else(|| string(config.get("billingPeriodEnd"))),
            prepaid_balance_cents: integer(config.pointer("/prepaidBalance/val")),
            on_demand_used_cents: integer(config.pointer("/onDemandUsed/val")),
            on_demand_cap_cents: integer(config.pointer("/onDemandCap/val")),
            subscription_tier: string(config.get("subscriptionTier")),
            updated_at: string(event.get("ts")),
            source: "Grok CLI telemetry".into(),
            detail: "Read from Grok's local billing telemetry; refreshes when the CLI publishes a new snapshot.".into(),
        };
    }
    UsageSnapshot::default()
}

fn number(value: Option<&Value>) -> f64 {
    value
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str()?.parse::<f64>().ok())
        })
        .unwrap_or(0.0)
}

fn integer(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or(0)
}

fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::load_usage_from_path;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn parses_latest_billing_snapshot() {
        let path = std::env::temp_dir().join(format!("grok-usage-{}.jsonl", Uuid::new_v4()));
        let events = concat!(
            "{\"msg\":\"billing: fetched credits config\",\"ts\":\"old\",\"ctx\":{\"config\":{\"creditUsagePercent\":10}}}\n",
            "{\"msg\":\"billing: fetched credits config\",\"ts\":\"new\",\"ctx\":{\"config\":{\"creditUsagePercent\":82,\"currentPeriod\":{\"start\":\"start\",\"end\":\"end\"},\"prepaidBalance\":{\"val\":1250},\"onDemandUsed\":{\"val\":325},\"onDemandCap\":{\"val\":5000},\"subscriptionTier\":\"Premium\"}}}\n"
        );
        fs::write(&path, events).expect("write telemetry fixture");
        let usage = load_usage_from_path(Some(path.clone()));
        assert!(usage.available);
        assert_eq!(usage.usage_percent, 82.0);
        assert_eq!(usage.remaining_percent, 18.0);
        assert_eq!(usage.prepaid_balance_cents, 1250);
        assert_eq!(usage.on_demand_used_cents, 325);
        assert_eq!(usage.updated_at.as_deref(), Some("new"));
        fs::remove_file(path).expect("remove telemetry fixture");
    }
}
