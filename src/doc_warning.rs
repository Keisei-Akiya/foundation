use crate::state::StateStore;
use serde_json::{json, Value};

const AD_HOC: &[&str] = &[
    "NOTES",
    "TODO",
    "SCRATCH",
    "TEMP",
    "DRAFT",
    "BRAINSTORM",
    "SPIKE",
    "DEBUG",
    "WIP",
];
const STRUCTURED_DIRS: &[&str] = &[
    "docs/",
    ".claude/",
    ".github/",
    "commands/",
    "skills/",
    "benchmarks/",
    "templates/",
    ".history/",
    "memory/",
];

fn path_from(payload: &Value) -> Option<String> {
    let input = payload.get("tool_input")?;
    ["file_path", "path", "target_file"]
        .iter()
        .find_map(|key| input.get(key).and_then(Value::as_str).map(str::to_owned))
}

fn suspicious(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let Some((base, ext)) = normalized.rsplit_once('.') else {
        return false;
    };
    if !matches!(ext, "md" | "txt") || STRUCTURED_DIRS.iter().any(|dir| normalized.contains(dir)) {
        return false;
    }
    let filename = base.rsplit('/').next().unwrap_or(base);
    AD_HOC.contains(&filename)
}

fn queue(store: &StateStore, session_id: &str, message: &str) {
    if let Ok(items) = store.read("pending-context") {
        let id = items
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_u64))
            .max()
            .unwrap_or(0)
            + 1;
        let _ = store.append(
            "pending-context",
            &json!({"id":id,"session_id":session_id,"context":message}),
        );
    }
}

fn drain(store: &StateStore, session_id: &str) -> Option<String> {
    if session_id.is_empty() {
        return None;
    }
    let items = store.read("pending-context").ok()?;
    let cursor = store
        .read("pending-context-cursor")
        .ok()?
        .iter()
        .filter(|item| item.get("session_id").and_then(Value::as_str) == Some(session_id))
        .filter_map(|item| item.get("id").and_then(Value::as_u64))
        .max()
        .unwrap_or(0);
    let pending: Vec<_> = items
        .iter()
        .filter(|item| item.get("session_id").and_then(Value::as_str) == Some(session_id))
        .filter(|item| item.get("id").and_then(Value::as_u64).unwrap_or(0) > cursor)
        .collect();
    if pending.is_empty() {
        return None;
    }
    let context = pending
        .iter()
        .filter_map(|item| item.get("context").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let id = pending
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_u64))
        .max()?;
    let _ = store.append(
        "pending-context-cursor",
        &json!({"session_id":session_id,"id":id}),
    );
    (!context.is_empty()).then_some(context)
}

pub fn run(payload: &Value) {
    let Ok(store) = StateStore::from_env() else {
        return;
    };
    let event = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let session = payload
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    match event {
        "pre_tool_call" => {
            if payload.get("tool_name").and_then(Value::as_str) != Some("write_file") {
                return;
            }
            if let Some(path) = path_from(payload).filter(|path| suspicious(path)) {
                queue(&store, session, &format!("[Hook] WARNING: Ad-hoc documentation filename detected. File: {path}. Consider using a structured path (docs/, .claude/, skills/, .github/, benchmarks/, templates/)."));
            }
        }
        "pre_llm_call" => {
            if let Some(context) = drain(&store, session) {
                println!("{}", json!({"context":context}));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::suspicious;

    #[test]
    fn detects_expected_ad_hoc_basename() {
        assert!(suspicious("/tmp/NOTES.md"));
        assert!(!suspicious("/repo/docs/NOTES.md"));
    }
}
