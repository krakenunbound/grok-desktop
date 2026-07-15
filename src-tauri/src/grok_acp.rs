//! Small, typed-enough ACP client for Grok Build extension requests.
//!
//! Grok Desktop deliberately keeps the official Grok CLI as its engine. This
//! module owns the JSON-RPC handshake used by non-chat features so billing and
//! privacy do not have to scrape terminal output or telemetry logs.

use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

struct AcpProcess {
    child: Child,
    job: Option<isize>,
    stdin: ChildStdin,
    reader: tokio::io::Lines<BufReader<ChildStdout>>,
}

impl AcpProcess {
    async fn start(binary: &Path) -> Result<Self, String> {
        let mut command = Command::new(binary);
        command
            .args(["agent", "stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        crate::privacy::apply_process_guard(&mut command);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command
            .spawn()
            .map_err(|error| format!("Start Grok ACP helper: {error}"))?;
        let process_id = child
            .id()
            .ok_or_else(|| "Grok ACP helper did not expose a process ID".to_string())?;
        let job = match crate::grok_process::create_kill_on_close_job(process_id) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill().await;
                return Err(error);
            }
        };
        let Some(stdin) = child.stdin.take() else {
            crate::grok_process::close_job(job);
            let _ = child.kill().await;
            return Err("Open Grok ACP helper input".into());
        };
        let Some(stdout) = child.stdout.take() else {
            crate::grok_process::close_job(job);
            let _ = child.kill().await;
            return Err("Open Grok ACP helper output".into());
        };

        // Drain stderr so a verbose CLI cannot fill the pipe and deadlock.
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(_line)) = lines.next_line().await {}
            });
        }

        Ok(Self {
            child,
            job: Some(job),
            stdin,
            reader: BufReader::new(stdout).lines(),
        })
    }

    async fn request(&mut self, id: i64, method: &str, params: Value) -> Result<Value, String> {
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut bytes = serde_json::to_vec(&message)
            .map_err(|error| format!("Encode Grok ACP request: {error}"))?;
        // The current Windows Grok reader requires CRLF framing. CRLF is also
        // accepted by the Unix line codec.
        bytes.extend_from_slice(b"\r\n");
        self.stdin
            .write_all(&bytes)
            .await
            .map_err(|error| format!("Write Grok ACP request: {error}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| format!("Flush Grok ACP request: {error}"))?;

        while let Some(line) = self
            .reader
            .next_line()
            .await
            .map_err(|error| format!("Read Grok ACP response: {error}"))?
        {
            let response: Value = serde_json::from_str(&line)
                .map_err(|error| format!("Decode Grok ACP response: {error}"))?;
            if response.get("id").and_then(Value::as_i64) != Some(id) {
                continue;
            }
            if let Some(error) = response.get("error") {
                return Err(format!("Grok ACP {method} failed: {error}"));
            }
            return Ok(response.get("result").cloned().unwrap_or(Value::Null));
        }
        Err(format!("Grok ACP closed before replying to {method}"))
    }

    async fn shutdown(mut self) {
        if let Some(job) = self.job.take() {
            crate::grok_process::close_job(job);
        }
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

impl Drop for AcpProcess {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            crate::grok_process::close_job(job);
        }
    }
}

pub(crate) fn select_auth_method(initialize: &Value) -> Option<String> {
    let methods = initialize.get("authMethods")?.as_array()?;
    let ids: Vec<&str> = methods
        .iter()
        .filter_map(|method| method.get("id").and_then(Value::as_str))
        .collect();
    let preferred = initialize
        .pointer("/_meta/defaultAuthMethodId")
        .and_then(Value::as_str);
    preferred
        .filter(|candidate| ids.contains(candidate))
        .or_else(|| ids.iter().copied().find(|id| *id == "cached_token"))
        .or_else(|| ids.first().copied())
        .map(str::to_string)
}

fn unwrap_extension_result(value: Value) -> Value {
    value.get("result").cloned().unwrap_or(value)
}

fn extension_wire_method(method: &str) -> Result<String, String> {
    if method.is_empty() || method.starts_with('_') || method.chars().any(char::is_whitespace) {
        return Err("Invalid ACP extension method".into());
    }
    Ok(format!("_{method}"))
}

/// Send an authenticated extension request to the official Grok Build agent.
pub async fn request_extension(
    binary: &Path,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, String> {
    let operation = async {
        let mut process = AcpProcess::start(binary).await?;
        let result = async {
            let initialize = process
                .request(
                    1,
                    "initialize",
                    serde_json::json!({
                        "protocolVersion": "1",
                        "clientCapabilities": {
                            "fs": {"readTextFile": false, "writeTextFile": false},
                            "terminal": false
                        },
                        "_meta": {"clientType": "desktop", "clientVersion": env!("CARGO_PKG_VERSION")}
                    }),
                )
                .await?;
            if let Some(method_id) = select_auth_method(&initialize) {
                process
                    .request(2, "authenticate", serde_json::json!({"methodId": method_id}))
                    .await?;
            }
            // ACP 0.10 encodes extension requests by prefixing the vendor
            // method with `_` on the JSON-RPC wire, then strips it before
            // calling Agent::ext_method.
            let wire_method = extension_wire_method(method)?;
            let response = process
                .request(3, &wire_method, params)
                .await?;
            Ok(unwrap_extension_result(response))
        }
        .await;
        process.shutdown().await;
        result
    };

    tokio::time::timeout(timeout, operation)
        .await
        .map_err(|_| format!("Grok ACP {method} timed out"))?
}

#[cfg(test)]
mod tests {
    use super::{extension_wire_method, select_auth_method, unwrap_extension_result};

    #[test]
    fn selects_agent_preferred_auth_method() {
        let response = serde_json::json!({
            "authMethods": [{"id": "cached_token"}, {"id": "xai_api_key"}],
            "_meta": {"defaultAuthMethodId": "xai_api_key"}
        });
        assert_eq!(
            select_auth_method(&response).as_deref(),
            Some("xai_api_key")
        );
    }

    #[test]
    fn falls_back_to_cached_token_for_legacy_agents() {
        let response = serde_json::json!({
            "authMethods": [{"id": "browser_login"}, {"id": "cached_token"}]
        });
        assert_eq!(
            select_auth_method(&response).as_deref(),
            Some("cached_token")
        );
    }

    #[test]
    fn unwraps_extension_payload() {
        let value = serde_json::json!({"result": {"config": {"creditUsagePercent": 42}}});
        assert_eq!(
            unwrap_extension_result(value)["config"]["creditUsagePercent"],
            42
        );
    }

    #[test]
    fn prefixes_vendor_extension_for_acp_wire() {
        assert_eq!(
            extension_wire_method("x.ai/billing").as_deref(),
            Ok("_x.ai/billing")
        );
        assert!(extension_wire_method("_x.ai/billing").is_err());
        assert!(extension_wire_method("x.ai/bad method").is_err());
    }

    #[tokio::test]
    #[ignore = "manual compatibility probe against the installed Grok CLI"]
    async fn installed_cli_billing_extension() {
        let binary = crate::grok_process::resolve_grok_binary("").expect("installed Grok CLI");
        let value = super::request_extension(
            &binary,
            "x.ai/billing",
            serde_json::json!({}),
            std::time::Duration::from_secs(25),
        )
        .await
        .expect("billing extension response");
        assert!(
            value.get("config").is_some(),
            "unexpected response: {value}"
        );
    }
}
