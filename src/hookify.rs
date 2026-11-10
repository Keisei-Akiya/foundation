use regex::Regex;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::frontmatter;

pub fn hookify(payload: &Value) {
    let Some(p) = payload.as_object() else {
        return;
    };
    let Some(cwd) = p.get("cwd").and_then(Value::as_str) else {
        return;
    };
    let Some(tool) = p.get("tool_name").and_then(Value::as_str) else {
        return;
    };
    let Some(args) = p.get("tool_input").and_then(Value::as_object) else {
        return;
    };
    let dir = Path::new(cwd).join(".claude");
    if dir.is_symlink() || !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.starts_with("hookify.") && s.ends_with(".local.md"))
        })
        .collect();
    paths.sort();
    paths.truncate(64);
    for path in paths {
        if path.is_symlink() || path.metadata().map(|m| m.len() > 16_384).unwrap_or(true) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some((rule, body)) = frontmatter(&text) else {
            continue;
        };
        if rule
            .get("enabled")
            .is_some_and(|v| v.eq_ignore_ascii_case("false"))
            || !["bash", "file"].contains(&rule.get("event").map(String::as_str).unwrap_or(""))
            || rule.get("action").is_some_and(|v| v != "block")
        {
            continue;
        }
        let Some(pattern) = rule.get("pattern") else {
            continue;
        };
        if pattern.is_empty()
            || pattern.len() > 256
            || pattern.contains("\\1")
            || pattern.contains("(? <")
        {
            continue;
        }
        let Ok(re) = Regex::new(&format!("(?i){pattern}")) else {
            continue;
        };
        let values: Vec<&str> = if rule.get("event").map(String::as_str) == Some("bash")
            && ["terminal", "bash", "Bash"].contains(&tool)
        {
            vec![args.get("command").and_then(Value::as_str).unwrap_or("")]
        } else if rule.get("event").map(String::as_str) == Some("file")
            && [
                "read_file",
                "write_file",
                "patch",
                "edit_file",
                "apply_patch",
                "Edit",
                "Write",
                "MultiEdit",
            ]
            .contains(&tool)
        {
            ["path", "file_path", "old_string", "new_string", "content"]
                .iter()
                .filter_map(|k| args.get(*k).and_then(Value::as_str))
                .collect()
        } else {
            continue;
        };
        if values.iter().any(|v| v.len() <= 65_536 && re.is_match(v)) {
            let name = rule
                .get("name")
                .map(String::as_str)
                .or_else(|| path.file_name().and_then(|s| s.to_str()))
                .unwrap_or("rule");
            let message = if body.is_empty() {
                "Policy matched."
            } else {
                body.as_str()
            };
            println!(
                "{}",
                json!({"decision":"block","reason":format!("Hookify rule {name}: {message}")})
            );
            return;
        }
    }
}
