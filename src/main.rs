use serde_json::Value;
use std::io::{self, Read, Write};

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
const MAX_STDIN: usize = 1_048_576;

fn warn(message: &str) {
    eprintln!("[ecc-config-protection] {message}")
}
fn as_nonempty_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
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

fn input() -> Option<Value> {
    let mut raw = String::new();
    if io::stdin()
        .take((MAX_STDIN + 1) as u64)
        .read_to_string(&mut raw)
        .is_err()
        || raw.len() > MAX_STDIN
    {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

fn frontmatter(text: &str) -> Option<(std::collections::HashMap<String, String>, String)> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut map = std::collections::HashMap::new();
    let mut body = String::new();
    let mut ended = false;
    for line in lines {
        if !ended && line.trim() == "---" {
            ended = true;
            continue;
        }
        if ended {
            body.push_str(line);
            body.push('\n');
            continue;
        }
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            return None;
        }
        let (key, val) = line.split_once(':')?;
        let key = key.trim();
        let mut val = val.trim().to_string();
        if key.is_empty()
            || !key.chars().all(|c| c.is_ascii_lowercase() || c == '_')
            || map.contains_key(key)
            || val.starts_with(['[', '{', '|', '>'])
        {
            return None;
        }
        if val.len() >= 2
            && ((val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\'')))
        {
            val = val[1..val.len() - 1].to_string();
        }
        if let Some((before, _)) = val.split_once(" #") {
            val = before.trim().to_string();
        }
        map.insert(key.to_string(), val);
    }
    if ended {
        Some((map, body.trim().to_string()))
    } else {
        None
    }
}

fn valid_id(v: &str, max: usize, extra: &str) -> bool {
    !v.is_empty()
        && v.len() <= max
        && v.split('/').all(|p| p != "." && p != "..")
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._".contains(c) || extra.contains(c))
}

mod config_protection;
mod decide;
mod enable;
mod hookify;
mod lifecycle;
mod post_observer;
mod preflight;

fn main() {
    let all: Vec<String> = std::env::args().skip(1).collect();
    if all.first().map(String::as_str) == Some("enable") {
        enable::run(&all[1..]);
        return;
    }
    if all.first().map(String::as_str) == Some("decide") {
        std::process::exit(decide::run(&all[1..]));
    }
    let mut args = all.into_iter();
    let command = args.next().unwrap_or_default();
    if args.next().is_some()
        || ![
            "config-protection",
            "preflight",
            "hookify",
            "post-observer",
            "lifecycle",
        ]
        .contains(&command.as_str())
    {
        eprintln!(
            "usage: foundation <config-protection|preflight|hookify|post-observer|lifecycle|enable|decide>"
        );
        std::process::exit(2);
    }
    let Some(payload) = input() else {
        return;
    };
    match command.as_str() {
        "config-protection" => config_protection::run_config_protection(&payload),
        "preflight" => preflight::preflight(&payload),
        "hookify" => hookify::hookify(&payload),
        "post-observer" => post_observer::post_observer(&payload),
        "lifecycle" => lifecycle::lifecycle(&payload),
        _ => unreachable!(),
    }
    let _ = io::stdout().flush();
}
