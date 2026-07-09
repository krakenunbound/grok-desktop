use crate::grok_process;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use toml_edit::{value, DocumentMut};

#[derive(Debug, Clone, Serialize)]
pub struct McpServerInfo {
    pub name: String,
    pub command: String,
    pub enabled: bool,
    pub source: String,
    pub can_toggle: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub enabled: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrokInventory {
    pub mcp_servers: Vec<McpServerInfo>,
    pub plugins: Vec<PluginInfo>,
    pub config_path: String,
}

fn grok_config_path() -> Result<std::path::PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not resolve home directory".to_string())?;
    Ok(home.join(".grok").join("config.toml"))
}

pub fn inventory(grok_binary_override: &str) -> Result<GrokInventory, String> {
    let config_path = grok_config_path()?;
    let mut config_path_label = config_path.to_string_lossy().to_string();
    let binary = grok_process::resolve_grok_binary(grok_binary_override).ok();
    let inspect = binary
        .as_ref()
        .and_then(|path| grok_json(path, &["inspect", "--json"]));
    let doctor = binary
        .as_ref()
        .and_then(|path| grok_json(path, &["mcp", "doctor", "--json"]));
    let mut servers: BTreeMap<String, McpServerInfo> = BTreeMap::new();

    add_config_mcp_servers(&mut servers, &config_path);
    if let Some(ref inspect_json) = inspect {
        if let Some(path) = user_config_path_from_inspect(inspect_json) {
            config_path_label = path;
        }
        add_inspect_mcp_servers(&mut servers, inspect_json);
    }
    if let Some(ref doctor_json) = doctor {
        add_doctor_mcp_servers(&mut servers, doctor_json);
    }

    Ok(GrokInventory {
        mcp_servers: servers.into_values().collect(),
        plugins: inspect
            .as_ref()
            .map(plugins_from_inspect)
            .unwrap_or_else(|| list_plugins(grok_binary_override)),
        config_path: config_path_label,
    })
}

fn add_config_mcp_servers(
    discovered: &mut BTreeMap<String, McpServerInfo>,
    config_path: &std::path::Path,
) {
    let raw = fs::read_to_string(config_path).unwrap_or_default();
    let doc = raw.parse::<DocumentMut>().unwrap_or_default();

    if let Some(config_servers) = doc.get("mcp_servers").and_then(|i| i.as_table()) {
        for (name, item) in config_servers.iter() {
            let Some(table) = item.as_table() else {
                continue;
            };
            let command = table
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let args = table
                .get("args")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            let enabled = table
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let command_line = if args.is_empty() {
                command
            } else {
                format!("{command} {args}")
            };
            let info = McpServerInfo {
                name: name.to_string(),
                command: command_line,
                enabled,
                source: config_path.to_string_lossy().to_string(),
                can_toggle: true,
                status: "Configured".into(),
            };
            discovered.insert(name.to_string(), info);
        }
    }
}

fn grok_json(binary: &std::path::Path, args: &[&str]) -> Option<Value> {
    let output = Command::new(binary).args(args).output().ok()?;
    serde_json::from_slice::<Value>(&output.stdout).ok()
}

fn user_config_path_from_inspect(inspect: &Value) -> Option<String> {
    inspect
        .get("configSources")?
        .get("layers")?
        .as_array()?
        .iter()
        .find(|layer| layer.get("role").and_then(Value::as_str) == Some("user"))
        .and_then(|layer| layer.get("path").and_then(Value::as_str))
        .map(ToString::to_string)
}

fn source_label(source: &Value) -> String {
    match source.get("type").and_then(Value::as_str).unwrap_or("") {
        "plugin" => source
            .get("plugin_name")
            .and_then(Value::as_str)
            .map(|name| format!("plugin: {name}"))
            .unwrap_or_else(|| "plugin".into()),
        "configToml" => source
            .get("path")
            .and_then(Value::as_str)
            .map(|path| format!("config: {path}"))
            .unwrap_or_else(|| "config".into()),
        other if !other.is_empty() => other.to_string(),
        _ => "discovered".into(),
    }
}

fn add_inspect_mcp_servers(servers: &mut BTreeMap<String, McpServerInfo>, inspect: &Value) {
    let Some(items) = inspect.get("mcpServers").and_then(Value::as_array) else {
        return;
    };
    for item in items {
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        let target = item.get("target").and_then(Value::as_str).unwrap_or("");
        let transport = item.get("transport").and_then(Value::as_str).unwrap_or("");
        let source = item
            .get("source")
            .map(source_label)
            .unwrap_or_else(|| "discovered".into());
        let can_toggle = source.starts_with("config:");
        let command = if transport.is_empty() {
            target.to_string()
        } else {
            format!("{transport}: {target}")
        };
        servers
            .entry(name.to_string())
            .and_modify(|existing| {
                existing.command = command.clone();
                existing.source = source.clone();
                existing.can_toggle = existing.can_toggle || can_toggle;
            })
            .or_insert_with(|| McpServerInfo {
                name: name.to_string(),
                command,
                enabled: true,
                source,
                can_toggle,
                status: "Discovered".into(),
            });
    }
}

fn add_doctor_mcp_servers(servers: &mut BTreeMap<String, McpServerInfo>, doctor: &Value) {
    let Some(items) = doctor.get("servers").and_then(Value::as_array) else {
        return;
    };
    for item in items {
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        let target = item.get("target").and_then(Value::as_str).unwrap_or("");
        let transport = item.get("transport").and_then(Value::as_str).unwrap_or("");
        let source = item
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("discovered")
            .to_string();
        let healthy = item
            .get("healthy")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let status = if healthy {
            "Ready"
        } else {
            "Needs auth or failed"
        }
        .to_string();
        let command = if transport.is_empty() {
            target.to_string()
        } else {
            format!("{transport}: {target}")
        };
        let can_toggle = source == "config";
        servers
            .entry(name.to_string())
            .and_modify(|existing| {
                existing.command = command.clone();
                existing.status = status.clone();
                if existing.source == "discovered" || existing.source.starts_with("config:") {
                    existing.source = source.clone();
                }
                existing.can_toggle = existing.can_toggle || can_toggle;
            })
            .or_insert_with(|| McpServerInfo {
                name: name.to_string(),
                command,
                enabled: true,
                source,
                can_toggle,
                status,
            });
    }
}

fn plugins_from_inspect(inspect: &Value) -> Vec<PluginInfo> {
    let Some(items) = inspect.get("plugins").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut plugins: Vec<PluginInfo> = items
        .iter()
        .filter_map(|item| {
            let name = item.get("name").and_then(Value::as_str)?.to_string();
            let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            let scope = item
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("plugin");
            let provides = item.get("provides").unwrap_or(&Value::Null);
            let skills = provides.get("skills").and_then(Value::as_u64).unwrap_or(0);
            let agents = provides.get("agents").and_then(Value::as_u64).unwrap_or(0);
            let mcp = provides
                .get("mcpServers")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let hooks = provides
                .get("hooks")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut parts = Vec::new();
            if skills > 0 {
                parts.push(format!("{skills} skills"));
            }
            if agents > 0 {
                parts.push(format!("{agents} agents"));
            }
            if mcp > 0 {
                parts.push(format!("{mcp} MCPs"));
            }
            if hooks {
                parts.push("hooks".into());
            }
            let detail = if parts.is_empty() {
                scope.to_string()
            } else {
                format!("{scope}, {}", parts.join(", "))
            };
            Some(PluginInfo {
                name,
                enabled,
                detail,
            })
        })
        .collect();
    plugins.sort_by(|a, b| a.name.cmp(&b.name));
    plugins
}

fn list_plugins(grok_binary_override: &str) -> Vec<PluginInfo> {
    let Ok(binary) = grok_process::resolve_grok_binary(grok_binary_override) else {
        return Vec::new();
    };
    let Ok(output) = Command::new(binary).args(["plugin", "list"]).output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.to_ascii_lowercase().contains("no plugins installed") {
        return Vec::new();
    }

    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.to_ascii_lowercase().contains("plugin") {
                return None;
            }
            let enabled = !trimmed.to_ascii_lowercase().contains("disabled");
            let name = trimmed
                .split_whitespace()
                .next()
                .unwrap_or(trimmed)
                .trim_matches(|c: char| c == '-' || c == '*' || c == '•')
                .to_string();
            if name.is_empty() {
                return None;
            }
            Some(PluginInfo {
                name,
                enabled,
                detail: trimmed.to_string(),
            })
        })
        .collect()
}

pub fn set_mcp_enabled(name: &str, enabled: bool) -> Result<GrokInventory, String> {
    if name.trim().is_empty() || name.contains(['\n', '\r', '[', ']']) {
        return Err("Invalid MCP server name".into());
    }

    let config_path = grok_config_path()?;
    let raw = fs::read_to_string(&config_path).map_err(|e| format!("read Grok config: {e}"))?;
    let mut doc = raw
        .parse::<DocumentMut>()
        .map_err(|e| format!("parse Grok config: {e}"))?;

    let server = doc["mcp_servers"]
        .as_table_mut()
        .and_then(|servers| servers.get_mut(name))
        .and_then(|item| item.as_table_mut())
        .ok_or_else(|| format!("MCP server not found: {name}"))?;
    server["enabled"] = value(enabled);
    fs::write(&config_path, doc.to_string()).map_err(|e| format!("write Grok config: {e}"))?;

    inventory("")
}

pub fn set_plugin_enabled(
    grok_binary_override: &str,
    name: &str,
    enabled: bool,
) -> Result<GrokInventory, String> {
    if name.trim().is_empty() || name.contains(['\n', '\r']) {
        return Err("Invalid plugin name".into());
    }
    let binary = grok_process::resolve_grok_binary(grok_binary_override)?;
    let action = if enabled { "enable" } else { "disable" };
    let output = Command::new(binary)
        .args(["plugin", action, name])
        .output()
        .map_err(|e| format!("run grok plugin {action}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "grok plugin {action} failed: {}{}",
            stdout.trim(),
            stderr.trim()
        ));
    }
    inventory(grok_binary_override)
}
