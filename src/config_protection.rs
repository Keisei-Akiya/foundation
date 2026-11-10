use serde_json::{json, Value};
use std::path::Path;

use super::{as_nonempty_string, json_type, warn, EDIT_TOOLS, PROTECTED};

pub fn run_config_protection(payload: &Value) {
    let Some(object) = payload.as_object() else {
        warn(&format!(
            "payload is {}, not an object — allowing",
            json_type(payload)
        ));
        return;
    };
    let tool = object
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !EDIT_TOOLS.contains(&tool) {
        return;
    }
    let Some(input) = object
        .get("tool_input")
        .filter(|v| !v.is_null())
        .and_then(Value::as_object)
    else {
        warn("tool_input is not an object — allowing");
        return;
    };
    let mut candidates = Vec::new();
    for key in [
        "path",
        "file_path",
        "filePath",
        "target",
        "filename",
        "notebook_path",
    ] {
        if let Some(path) = as_nonempty_string(input.get(key)) {
            candidates.push(path.to_owned());
        }
    }
    for key in ["edits", "operations"] {
        if let Some(items) = input.get(key).and_then(Value::as_array) {
            for item in items {
                if let Some(item) = item.as_object() {
                    for key in ["path", "file_path", "filePath"] {
                        if let Some(path) = as_nonempty_string(item.get(key)) {
                            candidates.push(path.to_owned());
                        }
                    }
                }
            }
        }
    }
    if candidates.is_empty() {
        warn(&format!(
            "no path found in tool_input for tool {tool:?} — allowing"
        ));
        return;
    }
    for candidate in candidates {
        let base = Path::new(&candidate)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if PROTECTED.contains(&base.to_lowercase().as_str())
            && std::fs::symlink_metadata(&candidate).is_ok()
        {
            println!(
                "{}",
                json!({"decision":"block", "reason": format!("BLOCKED: modifying {base} is not allowed. Fix the source code so the linter/formatter passes instead of weakening the config. For a legitimate config change, ask the owner to edit it directly.")})
            );
            return;
        }
    }
}
