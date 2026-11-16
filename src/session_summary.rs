use serde_json::Value;

const MAX_SUMMARY_BYTES: usize = 512;

#[derive(Default)]
struct TodoCounts {
    completed: usize,
    open: usize,
    total: usize,
}

fn purpose(history: &[Value]) -> &'static str {
    let user_text = history
        .iter()
        .filter(|item| {
            item.get("role").and_then(Value::as_str) == Some("user")
                || item.get("type").and_then(Value::as_str) == Some("user")
        })
        .filter_map(|item| item.get("content").and_then(Value::as_str))
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();

    if user_text.iter().any(|text| {
        [
            "implement",
            "build",
            "create",
            "add ",
            "実装",
            "作成",
            "追加",
        ]
        .iter()
        .any(|keyword| text.contains(keyword))
    }) {
        "Software implementation"
    } else if user_text.iter().any(|text| {
        ["debug", "fix", "bug", "修正", "不具合", "障害"]
            .iter()
            .any(|keyword| text.contains(keyword))
    }) {
        "Debugging and repair"
    } else if user_text.iter().any(|text| {
        ["test", "verify", "check", "確認", "検証", "テスト"]
            .iter()
            .any(|keyword| text.contains(keyword))
    }) {
        "Testing and verification"
    } else if user_text.iter().any(|text| {
        ["research", "investigate", "analyze", "調査", "分析"]
            .iter()
            .any(|keyword| text.contains(keyword))
    }) {
        "Research and analysis"
    } else if user_text.iter().any(|text| {
        [
            "document",
            "documentation",
            "write",
            "docs",
            "文書",
            "ドキュメント",
            "執筆",
        ]
        .iter()
        .any(|keyword| text.contains(keyword))
    }) {
        "Documentation"
    } else {
        "General continuation"
    }
}

fn todo_counts(value: &Value) -> TodoCounts {
    match value {
        Value::Object(object) => {
            for key in ["todos", "todo_list"] {
                if let Some(items) = object.get(key).and_then(Value::as_array) {
                    let mut counts = TodoCounts::default();
                    for item in items {
                        match item.get("status").and_then(Value::as_str) {
                            Some("completed" | "done") => {
                                counts.completed += 1;
                                counts.total += 1;
                            }
                            Some("pending" | "in_progress" | "blocked") => {
                                counts.open += 1;
                                counts.total += 1;
                            }
                            _ => {}
                        }
                    }
                    if counts.total > 0 {
                        return counts;
                    }
                }
            }
            object
                .values()
                .map(todo_counts)
                .find(|counts| counts.total > 0)
                .unwrap_or_default()
        }
        Value::Array(values) => values
            .iter()
            .map(todo_counts)
            .find(|counts| counts.total > 0)
            .unwrap_or_default(),
        _ => TodoCounts::default(),
    }
}

fn tool_operation_count(history: &[Value]) -> usize {
    history
        .iter()
        .filter(|item| {
            item.get("tool_name").and_then(Value::as_str).is_some()
                || item.get("type").and_then(Value::as_str) == Some("tool_call")
                || item.get("type").and_then(Value::as_str) == Some("tool_result")
        })
        .count()
}

fn contains_secret_or_command(text: &str) -> bool {
    // Best-effort filtering only; summaries are local, but this is not a secret scanner.
    let lower = text.to_ascii_lowercase();
    if [
        "authorization: bearer",
        "bearer ",
        "token",
        "password",
        "secret",
        "key=",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
        || text.starts_with('$')
        || text.contains(['|', '`', ';'])
        || text.contains("&&")
        || text.contains("$(")
        || text.contains("```")
    {
        return true;
    }

    for token in text.split_whitespace() {
        let trimmed =
            token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
        let lower_token = trimmed.to_ascii_lowercase();
        if lower_token.starts_with("sk-") && trimmed.len() >= 20 {
            return true;
        }
        if trimmed.len() >= 40 {
            let unique = trimmed
                .chars()
                .collect::<std::collections::HashSet<_>>()
                .len();
            if unique >= 12 {
                return true;
            }
        }
    }
    if text.contains("://") {
        let authority = text
            .split_once("://")
            .map(|(_, rest)| rest.split('/').next().unwrap_or_default())
            .unwrap_or_default();
        if authority.contains('@') {
            return true;
        }
    }
    false
}

fn contains_path(text: &str) -> bool {
    for token in text.split_whitespace() {
        let token = token.trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')' | ','));
        if token.starts_with('/')
            || token.starts_with("~/")
            || token.starts_with("./")
            || token.starts_with("../")
            || (token.len() >= 3
                && token.as_bytes().get(1) == Some(&b':')
                && token
                    .as_bytes()
                    .get(2)
                    .is_some_and(|b| *b == b'\\' || *b == b'/'))
            || token.matches('/').count() >= 2
        {
            return true;
        }
    }
    false
}

fn safe_item(text: &str) -> bool {
    !contains_secret_or_command(text) && !contains_path(text)
}

fn retained_work_items(history: &[Value]) -> Vec<&str> {
    let mut items = Vec::new();
    for entry in history {
        let is_user = entry.get("role").and_then(Value::as_str) == Some("user")
            || entry.get("type").and_then(Value::as_str) == Some("user");
        if is_user {
            if let Some(content) = entry.get("content").and_then(Value::as_str) {
                if safe_item(content) && !content.trim().is_empty() {
                    items.push(content.trim());
                }
            }
        }
        if let Some(todo_items) = entry
            .get("todos")
            .or_else(|| entry.get("todo_list"))
            .and_then(Value::as_array)
        {
            for todo in todo_items {
                if let Some(content) = todo.get("content").and_then(Value::as_str) {
                    if safe_item(content) && !content.trim().is_empty() {
                        items.push(content.trim());
                    }
                }
            }
        }
    }
    items
}

pub(super) fn build(history: &[Value]) -> String {
    let todos = history
        .iter()
        .rev()
        .map(todo_counts)
        .find(|counts| counts.total > 0)
        .unwrap_or_default();
    let tools = tool_operation_count(history);
    let progress = if todos.total > 0 {
        format!(
            "{}/{} structured tasks completed; {tools} tool operations recorded.",
            todos.completed, todos.total
        )
    } else {
        format!("{tools} tool operations recorded; no structured task state was captured.")
    };
    let unfinished = if todos.total > 0 {
        format!("{} structured tasks remain open.", todos.open)
    } else {
        "No structured unfinished-task state was captured.".to_string()
    };
    let work = retained_work_items(history);
    let work = if work.is_empty() {
        String::new()
    } else {
        format!(
            "\nWork: {}",
            work.iter().take(3).copied().collect::<Vec<_>>().join("; ")
        )
    };
    let summary = format!(
        "Purpose: {}.\nProgress: {progress}\nUnfinished: {unfinished}{work}",
        purpose(history)
    );
    if summary.len() <= MAX_SUMMARY_BYTES {
        summary
    } else {
        "Purpose: General continuation.\nProgress: Activity was recorded.\nUnfinished: Summary omitted because it exceeded the size limit.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{build, MAX_SUMMARY_BYTES};
    use serde_json::json;

    #[test]
    fn summary_is_useful_without_copying_user_or_tool_content() {
        let history = vec![
            json!({"role":"user","content":"Implement the requested feature /home/rocky/project export TOKEN=secret run cargo test"}),
            json!({"role":"assistant","content":"private response"}),
            json!({"type":"tool_result","tool_name":"todo","todos":[{"status":"pending","content":"secret path /home/private"},{"status":"completed","content":"done /home/done"}]}),
        ];
        let summary = build(&history);
        assert!(summary.contains("Purpose: Software implementation."));
        assert!(summary
            .contains("Progress: 1/2 structured tasks completed; 1 tool operations recorded."));
        assert!(summary.contains("Unfinished: 1 structured tasks remain open."));
        for forbidden in [
            "Implement the requested feature",
            "private response",
            "/home/",
            "TOKEN=secret",
            "cargo test",
            "secret path",
        ] {
            assert!(!summary.contains(forbidden));
        }
        assert!(summary.len() <= MAX_SUMMARY_BYTES);
    }

    #[test]
    fn purpose_is_a_fixed_category_not_a_copy_of_the_user_prompt() {
        let history =
            vec![json!({"role":"user","content":"Please debug my private service at /srv/acme"})];
        let summary = build(&history);
        assert!(summary.starts_with("Purpose: Debugging and repair."));
        assert!(!summary.contains("private service"));
        assert!(!summary.contains("/srv/acme"));
    }
}
