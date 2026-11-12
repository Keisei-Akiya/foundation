use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn post_observer(payload: &Value) {
    let Some(p) = payload.as_object() else {
        return;
    };
    if p.get("hook_event_name").and_then(Value::as_str) != Some("post_tool_call") {
        return;
    }
    let tool = p.get("tool_name").and_then(Value::as_str).unwrap_or("");
    if ![
        "write_file",
        "patch",
        "edit",
        "edit_file",
        "write",
        "apply_patch",
        "str_replace_editor",
        "Edit",
        "Write",
        "MultiEdit",
    ]
    .contains(&tool)
    {
        return;
    }
    let Some(args) = p.get("tool_input").and_then(Value::as_object) else {
        return;
    };
    let Some(raw) = args
        .get("file_path")
        .or_else(|| args.get("path"))
        .and_then(Value::as_str)
    else {
        return;
    };
    let cwd = p.get("cwd").and_then(Value::as_str).unwrap_or(".");
    let path = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        Path::new(cwd).join(raw)
    };
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ![
        "js", "jsx", "ts", "tsx", "astro", "css", "html", "scss", "svelte", "vue",
    ]
    .contains(&ext.as_str())
    {
        return;
    }
    let Ok(meta) = path.metadata() else {
        return;
    };
    if meta.len() > 2_000_000 {
        return;
    }
    let Ok(source) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut signals = Vec::new();
    if Regex::new(r"\bconsole\s*\.\s*(?:log|debug|info)\s*\(")
        .unwrap()
        .is_match(&source)
    {
        signals.push("console logging");
    }
    if Regex::new(r"(?i)#(?:000|fff)(?:fff)?\b")
        .unwrap()
        .is_match(&source)
    {
        signals.push("literal black/white color");
    }
    if Regex::new(r"(?i)\b(?:Arial|Helvetica|sans-serif)\b")
        .unwrap()
        .is_match(&source)
    {
        signals.push("generic font family");
    }
    if !signals.is_empty() {
        eprintln!(
            "Post-edit warning: {} in {}",
            signals.join(", "),
            path.display()
        );
    }
}
