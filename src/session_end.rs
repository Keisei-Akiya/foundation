//! Persist privacy-minimal per-turn metadata for cross-session state.
use crate::state::StateStore;
use serde_json::{json, Value};

#[path = "session_summary.rs"]
mod session_summary;

pub fn run(payload: &Value) {
    if payload.get("hook_event_name").and_then(Value::as_str) != Some("post_llm_call") {
        return;
    }
    let session_id = match payload.get("session_id").and_then(Value::as_str) {
        Some(id) if crate::state::validate_session_id(id).is_ok() => id,
        _ => return,
    };
    let history = match payload
        .get("conversation_history")
        .and_then(Value::as_array)
    {
        Some(history) => history,
        None => return,
    };
    let summary = session_summary::build(history);
    let record = json!({
        "event": "stop:session-end",
        "session_id": session_id,
        "summary": summary,
    });
    if let Ok(store) = StateStore::from_env() {
        if store.is_opted_in("session-summary", false).unwrap_or(false) {
            let _ = store.append("session-summary", &record);
        }
    }
}
