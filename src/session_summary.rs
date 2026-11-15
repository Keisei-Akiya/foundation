use serde_json::Value;

const MAX_SUMMARY_BYTES: usize = 512;

fn count_role(history: &[Value], role: &str) -> usize {
    history
        .iter()
        .filter(|item| {
            item.get("role").and_then(Value::as_str) == Some(role)
                || item.get("type").and_then(Value::as_str) == Some(role)
        })
        .count()
}

fn todo_counts(value: &Value) -> Option<(usize, usize)> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if matches!(key.as_str(), "todos" | "todo_list" | "items") {
                    if let Some(items) = value.as_array() {
                        let mut unfinished = 0;
                        let mut total = 0;
                        for item in items {
                            if let Some(status) = item.get("status").and_then(Value::as_str) {
                                total += 1;
                                if matches!(status, "pending" | "in_progress" | "blocked") {
                                    unfinished += 1;
                                }
                            }
                        }
                        if total > 0 {
                            return Some((unfinished, total));
                        }
                    }
                }
                if let Some(counts) = todo_counts(value) {
                    return Some(counts);
                }
            }
            None
        }
        Value::Array(values) => values.iter().find_map(todo_counts),
        _ => None,
    }
}

pub(super) fn build(history: &[Value]) -> String {
    let users = count_role(history, "user");
    let assistant = count_role(history, "assistant");
    let unfinished = todo_counts(&Value::Array(history.to_vec()));
    let open = match unfinished {
        Some((pending, total)) => {
            format!("{pending} of {total} structured todo items remain open.")
        }
        None => "No structured todo state was present in the captured history.".to_string(),
    };
    let summary = format!(
        "Purpose: Continue the work represented by this session.\nProgress: Captured {users} user and {assistant} assistant messages.\nUnfinished: {open}"
    );
    if summary.len() <= MAX_SUMMARY_BYTES {
        summary
    } else {
        "Purpose: Continue prior work.\nProgress: Session activity was recorded.\nUnfinished: Summary omitted because it exceeded the size limit.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::build;
    use serde_json::json;

    #[test]
    fn summary_contains_status_only_and_does_not_copy_sensitive_content() {
        let history = vec![
            json!({"role":"user","content":"private prompt /home/rocky/project export TOKEN=secret run cargo test"}),
            json!({"role":"assistant","content":"private response"}),
            json!({"type":"tool_result","tool_name":"todo","todos":[{"status":"pending","content":"secret path /home/private"},{"status":"completed","content":"done"}]}),
        ];
        let summary = build(&history);
        assert!(summary.contains("Purpose:"));
        assert!(summary.contains("Progress:"));
        assert!(summary.contains("Unfinished: 1 of 2"));
        for forbidden in [
            "private prompt",
            "/home/",
            "TOKEN=secret",
            "cargo test",
            "private response",
            "secret path",
        ] {
            assert!(!summary.contains(forbidden));
        }
        assert!(summary.len() <= 512);
    }
}
