use serde_json::{json, Value};
use std::path::Path;

fn input(p: &Value) -> Option<&Value> {
    p.get("tool_input").or_else(|| p.get("args"))
}
fn paths(input: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["path", "file_path", "filePath"] {
        if let Some(s) = input.get(key).and_then(Value::as_str) {
            out.push(s.to_owned());
        }
    }
    if let Some(edits) = input.get("edits").and_then(Value::as_array) {
        for edit in edits {
            if let Some(s) = edit.get("file_path").and_then(Value::as_str) {
                out.push(s.to_owned());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
fn write_tool(name: &str) -> bool {
    [
        "edit",
        "edit_file",
        "write",
        "write_file",
        "patch",
        "apply_patch",
        "str_replace_editor",
        "Edit",
        "Write",
        "MultiEdit",
        "NotebookEdit",
    ]
    .contains(&name)
}
pub fn run(payload: &Value) {
    if payload.get("hook_event_name").and_then(Value::as_str) != Some("pre_tool_call") {
        return;
    }
    let Some(tool) = payload.get("tool_name").and_then(Value::as_str) else {
        return;
    };
    if !write_tool(tool) {
        return;
    }
    let Some(args) = input(payload) else {
        return;
    };
    let files = paths(args);
    if files.is_empty() {
        return;
    }
    let session = payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown");
    let Ok(store) = crate::state::StateStore::from_env() else {
        return;
    };
    for file in files {
        let path = Path::new(&file);
        let sensitive = file.to_ascii_lowercase().contains(".env")
            || file.to_ascii_lowercase().contains("secret")
            || file.to_ascii_lowercase().contains("credential");
        #[cfg(unix)]
        let hard_link = std::fs::metadata(path)
            .map(|m| {
                use std::os::unix::fs::MetadataExt;
                m.nlink() > 1
            })
            .unwrap_or(false);
        #[cfg(not(unix))]
        let hard_link = false;
        let key = format!("{}:{}", session, file);
        let seen = store
            .read("gateguard")
            .unwrap_or_default()
            .iter()
            .any(|r| r.get("key").and_then(Value::as_str) == Some(&key));
        if !seen {
            if store
                .append(
                    "gateguard",
                    &json!({"key":key,"sensitive":sensitive,"hard_link":hard_link}),
                )
                .is_err()
            {
                return;
            }
            println!(
                "{}",
                json!({"action":"block","message":format!("GateGuard first edit/write for {file}: state the relevant facts and verify the target before retrying. Sensitive and hard-linked files remain subject to this first-touch check.")})
            );
            return;
        }
    }
}
