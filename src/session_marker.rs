use serde_json::Value;
use std::path::Path;

const NOTICE: &str = "The previous session was interrupted before it finalized. Its last turn may be incomplete; verify its state before continuing.";

/// Return a recovery notice only when the newest prior session lacks a clean finalizer.
pub fn previous_session_notice(payload: &Value) -> Option<&'static str> {
    let current = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    if current.is_empty() {
        return None;
    }
    let home = std::env::var_os("HERMES_HOME")?;
    let path = Path::new(&home).join("plugin-data/foundation/session-markers.jsonl");
    let contents = std::fs::read_to_string(path).ok()?;
    let records: Vec<Value> = contents
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    let prior = records.iter().rev().find(|record| {
        let id = record
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        !id.is_empty() && id != current
    })?;
    let id = prior.get("session_id")?.as_str()?;
    let session_records: Vec<&Value> = records
        .iter()
        .filter(|record| record.get("session_id").and_then(Value::as_str) == Some(id))
        .collect();
    let start = session_records.iter().rposition(|record| {
        record.get("event").and_then(Value::as_str) == Some("session:start:marker")
    })?;
    let events = &session_records[start..];
    if let Some(turn_end) = events
        .iter()
        .rev()
        .find(|record| record.get("event").and_then(Value::as_str) == Some("session:turn:end"))
    {
        if turn_end.get("interrupted").and_then(Value::as_bool) == Some(true) {
            return Some(NOTICE);
        }
    }
    let finalized = events
        .iter()
        .any(|record| record.get("event").and_then(Value::as_str) == Some("session:end:marker"));
    (!finalized).then_some(NOTICE)
}
