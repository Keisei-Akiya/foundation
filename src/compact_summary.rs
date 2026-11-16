use crate::state::{validate_session_id, StateStore};
use serde_json::{json, Value};
use std::path::Path;

#[allow(clippy::duplicate_mod)]
#[path = "session_summary.rs"]
mod session_summary;

const STORE: &str = "compact-summary";
const RECORDS: &str = "compact-summary";

fn enabled_home() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HERMES_HOME")?;
    if home.is_empty() {
        return None;
    }
    let home = Path::new(&home);
    let root = home.join("plugin-data/foundation");
    if root.join(format!("{STORE}.disabled")).is_file()
        || !root.join(format!("{STORE}.enabled")).is_file()
    {
        return None;
    }
    Some(home.to_path_buf())
}

fn session_id(payload: &Value) -> Option<&str> {
    let id = payload.get("session_id")?.as_str()?;
    validate_session_id(id).ok()?;
    Some(id)
}

fn last_record<'a>(records: &'a [Value], id: &str) -> Option<&'a Value> {
    records
        .iter()
        .rev()
        .find(|record| record.get("session_id").and_then(Value::as_str) == Some(id))
}

pub fn run(payload: &Value) {
    let Some(home) = enabled_home() else {
        return;
    };
    let Some(id) = session_id(payload) else {
        return;
    };
    let Ok(store) = StateStore::new(home) else {
        return;
    };
    let event = payload
        .get("hook_event_name")
        .or_else(|| payload.get("event"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let Ok(records) = store.read(RECORDS) else {
        return;
    };

    match event {
        "post_llm_call" => {
            let history = payload
                .get("conversation_history")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let summary = session_summary::build(&history);
            let _ = store.append(
                RECORDS,
                &json!({"event":"summary_saved","session_id":id,"summary":summary}),
            );
        }
        "post_compaction" => {
            if let Some(record) = last_record(&records, id) {
                if record.get("event").and_then(Value::as_str) == Some("summary_saved")
                    || record.get("event").and_then(Value::as_str) == Some("restored")
                {
                    let _ =
                        store.append(RECORDS, &json!({"event":"restore_pending","session_id":id}));
                }
            }
        }
        "pre_llm_call" => {
            if last_record(&records, id)
                .and_then(|r| r.get("event"))
                .and_then(Value::as_str)
                != Some("restore_pending")
            {
                return;
            }
            let Some(summary) = records.iter().rev().find_map(|record| {
                (record.get("session_id").and_then(Value::as_str) == Some(id)
                    && record.get("event").and_then(Value::as_str) == Some("summary_saved"))
                .then(|| record.get("summary").and_then(Value::as_str))
                .flatten()
            }) else {
                return;
            };
            if store
                .append(
                    RECORDS,
                    &json!({"event":"restored","session_id":id,"summary":summary}),
                )
                .is_ok()
            {
                println!(
                    "{}",
                    json!({"context":format!("Recent work summary after context compaction:\n{summary}")})
                );
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::last_record;
    use serde_json::json;

    #[test]
    fn latest_record_is_scoped_to_the_session() {
        let records = vec![
            json!({"event":"summary_saved","session_id":"a","summary":"first"}),
            json!({"event":"summary_saved","session_id":"b","summary":"other"}),
            json!({"event":"restore_pending","session_id":"a"}),
        ];
        assert_eq!(
            last_record(&records, "a").unwrap()["event"],
            "restore_pending"
        );
        assert_eq!(last_record(&records, "b").unwrap()["summary"], "other");
    }
}
