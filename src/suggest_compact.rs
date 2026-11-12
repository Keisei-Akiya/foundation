use crate::state::StateStore;
use serde_json::{json, Value};

const THRESHOLD: u64 = 50;
const INTERVAL: u64 = 25;

pub fn run(payload: &Value) {
    let Ok(store) = StateStore::from_env() else {
        return;
    };
    if payload.get("hook_event_name").and_then(Value::as_str) != Some("pre_tool_call") {
        return;
    }
    let session = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if session.is_empty() {
        return;
    }
    let Ok(records) = store.read("suggest-compact-counts") else {
        return;
    };
    let count = records
        .iter()
        .filter(|r| r.get("session_id").and_then(Value::as_str) == Some(session))
        .count() as u64
        + 1;
    if store
        .append("suggest-compact-counts", &json!({"session_id":session}))
        .is_err()
    {
        return;
    }
    if count == THRESHOLD || (count > THRESHOLD && (count - THRESHOLD).is_multiple_of(INTERVAL)) {
        let message = if count == THRESHOLD {
            format!("[StrategicCompact] {count} tool calls reached - consider /compact if transitioning phases")
        } else {
            format!("[StrategicCompact] {count} tool calls - good checkpoint for /compact if context is stale")
        };
        if let Ok(items) = store.read("pending-context") {
            let id = items
                .iter()
                .filter_map(|item| item.get("id").and_then(Value::as_u64))
                .max()
                .unwrap_or(0)
                + 1;
            let _ = store.append(
                "pending-context",
                &json!({"id":id,"session_id":session,"context":message}),
            );
        }
    }
}
