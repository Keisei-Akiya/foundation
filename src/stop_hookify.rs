use regex::Regex;
use serde_json::{json, Value};
use std::path::Path;

pub fn run(payload: &Value) {
    let Some(obj) = payload.as_object() else {
        return;
    };
    let cwd = obj.get("cwd").and_then(Value::as_str).unwrap_or("");
    let response = obj
        .get("final_response")
        .and_then(Value::as_str)
        .unwrap_or("");
    if cwd.is_empty() || response.is_empty() {
        return;
    }
    let dir = Path::new(cwd).join(".claude");
    if dir.is_symlink() || !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("hookify.") && n.ends_with(".local.md"))
        })
        .collect();
    files.sort();
    files.truncate(64);
    for file in files {
        if file.is_symlink() || file.metadata().map(|m| m.len() > 16_384).unwrap_or(true) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Some((rule, body)) = super::frontmatter(&text) else {
            continue;
        };
        if rule.get("event").map(String::as_str) != Some("stop")
            || rule.get("action").map(String::as_str) != Some("block")
            || rule
                .get("enabled")
                .is_some_and(|s| s.eq_ignore_ascii_case("false"))
        {
            continue;
        }
        let Some(pattern) = rule
            .get("pattern")
            .filter(|s| !s.is_empty() && s.len() <= 256 && !s.contains("\\1"))
        else {
            continue;
        };
        let Ok(re) = Regex::new(&format!("(?i){pattern}")) else {
            continue;
        };
        if re.is_match(response) {
            let name = rule.get("name").map(String::as_str).unwrap_or("stop rule");
            let message = if body.is_empty() {
                "Completion rule matched."
            } else {
                &body
            };
            println!(
                "{}",
                json!({"action":"continue", "message":format!("Hookify rule {name}: {message}")})
            );
            return;
        }
    }
}
