//! Evaluate project-local Hookify prompt rules for Hermes pre-LLM calls.
use std::fs;
use std::path::Path;

const MAX_PROMPT_BYTES: usize = 256 * 1024;
const MAX_RULE_BYTES: u64 = 64 * 1024;
const MAX_RULE_PATTERN_BYTES: usize = 512;
const MAX_RULE_FILES: usize = 128;
const MAX_DIRECTORY_ENTRIES: usize = 1024;

struct Rule {
    name: String,
    event: String,
    action: String,
    pattern: String,
    message: String,
    enabled: bool,
}

fn scalar(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 {
        let first = value.as_bytes()[0];
        if (first == b'\'' && value.as_bytes().last() == Some(&b'\''))
            || (first == b'"' && value.as_bytes().last() == Some(&b'"'))
        {
            return value[1..value.len() - 1].to_owned();
        }
    }
    value.to_owned()
}

fn parse_rule(path: &Path) -> Option<Rule> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > MAX_RULE_BYTES {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }

    let mut values = std::collections::HashMap::new();
    let mut body = Vec::new();
    let mut ended = false;
    for line in lines {
        if !ended && line.trim() == "---" {
            ended = true;
            continue;
        }
        if ended {
            body.push(line);
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            return None;
        }
        let (key, value) = line.split_once(':')?;
        let key = key.trim();
        if !matches!(
            key,
            "name" | "enabled" | "event" | "action" | "pattern" | "tool_matcher"
        ) || values.contains_key(key)
        {
            return None;
        }
        values.insert(key, scalar(value));
    }
    if !ended {
        return None;
    }
    let enabled = match values.get("enabled").map(|value| value.as_str()) {
        None | Some("true") | Some("True") | Some("TRUE") => true,
        Some("false") | Some("False") | Some("FALSE") => false,
        _ => return None,
    };
    let action = values.get("action").map(String::as_str).unwrap_or("warn");
    if !matches!(action, "block" | "warn" | "context") {
        return None;
    }
    let pattern = values.get("pattern")?.as_str();
    if pattern.is_empty() || pattern.len() > MAX_RULE_PATTERN_BYTES {
        return None;
    }
    let name = values
        .get("name")
        .filter(|name| !name.is_empty())
        .cloned()
        .or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })?;
    let message = body.join("\n").trim().to_owned();
    Some(Rule {
        name,
        event: values.get("event").cloned().unwrap_or_default(),
        action: action.to_owned(),
        pattern: pattern.to_owned(),
        message: if message.is_empty() {
            "Prompt rule matched.".to_owned()
        } else {
            message
        },
        enabled,
    })
}

/// Return user-facing context and an optional blocking message for a prompt.
pub(super) fn evaluate(cwd: &str, prompt: &str) -> (Vec<String>, Option<String>) {
    if prompt.is_empty() || prompt.len() > MAX_PROMPT_BYTES {
        return (Vec::new(), None);
    }
    let dir = Path::new(cwd).join(".claude");
    let Ok(entries) = fs::read_dir(dir) else {
        return (Vec::new(), None);
    };
    let mut paths: Vec<_> = entries
        .take(MAX_DIRECTORY_ENTRIES)
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with("hookify.") && name.ends_with(".local.md")
                })
        })
        .collect();
    paths.sort();
    paths.truncate(MAX_RULE_FILES);

    let mut contexts = Vec::new();
    for path in paths {
        let Some(rule) = parse_rule(&path) else {
            continue;
        };
        if !rule.enabled
            || !matches!(
                rule.event.as_str(),
                "prompt" | "all" | "llm" | "pre_llm_call"
            )
        {
            continue;
        }
        let Ok(regex) = regex::RegexBuilder::new(&rule.pattern)
            .case_insensitive(true)
            .size_limit(1024 * 1024)
            .dfa_size_limit(1024 * 1024)
            .build()
        else {
            continue;
        };
        if !regex.is_match(prompt) {
            continue;
        }
        if rule.action == "block" {
            return (contexts, Some(format!("{}: {}", rule.name, rule.message)));
        }
        contexts.push(format!(
            "Prompt rule '{}' matched: {}",
            rule.name, rule.message
        ));
    }
    (contexts, None)
}
