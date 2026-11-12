use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

const TTL_MS: u64 = 120_000;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
fn server(payload: &Value) -> Option<String> {
    let name = payload.get("tool_name")?.as_str()?;
    let rest = name.strip_prefix("mcp__")?;
    let server = rest.split("__").next()?;
    if server.is_empty()
        || !server
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    Some(server.to_owned())
}

pub fn run(payload: &Value) {
    let Some(server) = server(payload) else {
        return;
    };
    let Ok(store) = crate::state::StateStore::from_env() else {
        return;
    };
    match payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("")
    {
        "post_tool_call" => {
            let status = payload
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_ascii_lowercase();
            let failed = ["error", "failed", "failure", "timeout", "timed_out"]
                .contains(&status.as_str())
                || payload
                    .get("error_type")
                    .is_some_and(|v| !v.is_null() && v != "")
                || payload
                    .get("error_message")
                    .is_some_and(|v| !v.is_null() && v != "");
            if failed {
                let _ = store.append(
                    "mcp-health",
                    &json!({"server":server,"failed_at_ms":now_ms()}),
                );
            }
        }
        "pre_tool_call" => {
            let now = now_ms();
            let unhealthy = store
                .read("mcp-health")
                .unwrap_or_default()
                .iter()
                .rev()
                .find(|r| r.get("server").and_then(Value::as_str) == Some(&server))
                .and_then(|r| r.get("failed_at_ms").and_then(Value::as_u64))
                .is_some_and(|t| now.saturating_sub(t) <= TTL_MS);
            if unhealthy {
                println!(
                    "{}",
                    json!({"action":"block","message":format!("MCP server {server} recently failed; retry after the health-check TTL. Hermes exposes no reconnect API to this hook.")})
                );
            }
        }
        _ => {}
    }
}
