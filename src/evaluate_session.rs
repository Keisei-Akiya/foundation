//! Mark sessions eligible for ECC-style pattern review without auto-generating skills.
use crate::state::StateStore;
use serde_json::{json, Value};

const MIN_SESSION_LENGTH: usize = 10;

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
    let user_message_count = history
        .iter()
        .filter(|item| {
            item.get("type").and_then(Value::as_str) == Some("user")
                || item.get("role").and_then(Value::as_str) == Some("user")
        })
        .count();
    if user_message_count < MIN_SESSION_LENGTH {
        return;
    }
    let record = json!({
        "event": "stop:evaluate-session",
        "session_id": session_id,
        "user_message_count": user_message_count,
        "minimum_messages": MIN_SESSION_LENGTH,
        "status": "eligible_for_manual_review",
    });
    if let Ok(store) = StateStore::from_env() {
        if store
            .is_opted_in("session-evaluation", true)
            .unwrap_or(false)
        {
            let _ = store.append("session-evaluation", &record);
        }
    }
}
