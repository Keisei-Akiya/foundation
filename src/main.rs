use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::path::Path;

const PROTECTED: &[&str] = &[
    ".eslintrc",
    ".eslintrc.js",
    ".eslintrc.cjs",
    ".eslintrc.json",
    ".eslintrc.yml",
    ".eslintrc.yaml",
    "eslint.config.js",
    "eslint.config.mjs",
    "eslint.config.cjs",
    "eslint.config.ts",
    "eslint.config.mts",
    "eslint.config.cts",
    ".prettierrc",
    ".prettierrc.js",
    ".prettierrc.cjs",
    ".prettierrc.json",
    ".prettierrc.yml",
    ".prettierrc.yaml",
    "prettier.config.js",
    "prettier.config.cjs",
    "prettier.config.mjs",
    "biome.json",
    "biome.jsonc",
    ".ruff.toml",
    "ruff.toml",
    ".shellcheckrc",
    ".stylelintrc",
    ".stylelintrc.json",
    ".stylelintrc.yml",
    ".markdownlint.json",
    ".markdownlint.yaml",
    ".markdownlintrc",
    "lint-ratchet-baseline.json",
];
const EDIT_TOOLS: &[&str] = &[
    "patch",
    "write_file",
    "edit",
    "edit_file",
    "write",
    "apply_patch",
    "str_replace_editor",
];
const PATH_KEYS: &[&str] = &["path", "file_path", "filePath"];

fn warn(message: &str) {
    eprintln!("[ecc-config-protection] {message}");
}

fn as_nonempty_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn run_config_protection(payload: &Value) {
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
    let input_value = object.get("tool_input").filter(|v| !v.is_null());
    let Some(input) = input_value.and_then(Value::as_object) else {
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
                    for key in PATH_KEYS {
                        if let Some(path) = as_nonempty_string(item.get(*key)) {
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
            let result = json!({"decision":"block", "reason": format!(
                "BLOCKED: modifying {base} is not allowed. Fix the source code so the linter/formatter passes instead of weakening the config. For a legitimate config change, ask the owner to edit it directly."
            )});
            println!("{result}");
            return;
        }
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (Some("config-protection"), None) => {}
        _ => {
            eprintln!("usage: ecc-hermes-hooks config-protection");
            std::process::exit(2);
        }
    }
    let mut raw = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut raw) {
        warn(&format!("could not read payload ({error}) — allowing"));
        return;
    }
    let payload = if raw.trim().is_empty() {
        json!({})
    } else {
        match serde_json::from_str::<Value>(&raw) {
            Ok(payload) => payload,
            Err(error) => {
                warn(&format!("unparseable payload ({error}) — allowing"));
                return;
            }
        }
    };
    run_config_protection(&payload);
    let _ = io::stdout().flush();
}
