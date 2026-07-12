//! Read-only Grok subscription/credit usage from the CLI's local telemetry log.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

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
