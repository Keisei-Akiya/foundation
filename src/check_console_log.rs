use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn run(payload: &Value) {
    let Some(obj) = payload.as_object() else {
        return;
    };
    let cwd = obj.get("cwd").and_then(Value::as_str).unwrap_or("");
    let Some(paths) = obj.get("changed_paths").and_then(Value::as_array) else {
        return;
    };
    let root = Path::new(cwd);
    let mut findings = Vec::new();
    for value in paths.iter().take(512) {
        let Some(raw) = value.as_str() else { continue };
        let rel = Path::new(raw);
        if rel.is_absolute()
            || rel
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            continue;
        }
        let ext = rel.extension().and_then(|s| s.to_str()).unwrap_or("");
        if !["js", "jsx", "ts", "tsx", "mjs", "cjs"].contains(&ext)
            || [
                ".test.",
                ".spec.",
                ".config.",
                "scripts/",
                "__tests__/",
                "__mocks__/",
            ]
            .iter()
            .any(|x| raw.contains(x))
        {
            continue;
        }
        let path: PathBuf = root.join(rel);
        if path.is_symlink() || !path.is_file() {
            continue;
        }
        if let Ok(contents) = std::fs::read_to_string(path) {
            if contents.contains("console.log") {
                findings.push(raw.to_owned());
            }
        }
    }
    if !findings.is_empty() {
        println!(
            "{}",
            json!({"action":"continue", "message":format!("Remove console.log from changed file(s): {}", findings.join(", "))})
        );
    }
}
