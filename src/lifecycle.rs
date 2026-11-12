use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;

use super::valid_id;

pub fn lifecycle(payload: &Value) {
    if std::env::var("ECC_HERMES_SKILL_TRACK").ok().as_deref() != Some("1") {
        return;
    }
    let Some(p) = payload.as_object() else {
        return;
    };
    if p.get("hook_event_name").and_then(Value::as_str) != Some("post_tool_call")
        || !p
            .get("tool_name")
            .and_then(Value::as_str)
            .is_some_and(|name| name.eq_ignore_ascii_case("skill"))
    {
        return;
    }
    let Some(args) = p.get("tool_input").and_then(Value::as_object) else {
        return;
    };
    let Some(id) = ["skill_id", "skillId", "skill", "name", "command"]
        .iter()
        .filter_map(|k| args.get(*k).and_then(Value::as_str))
        .find(|v| valid_id(v, 128, "@/:-"))
    else {
        return;
    };
    let version = ["skill_version", "skillVersion", "version"]
        .iter()
        .filter_map(|k| args.get(*k).and_then(Value::as_str))
        .find(|v| valid_id(v, 64, "+-"))
        .unwrap_or("unknown");
    let status = p
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let response_failed = p.get("result").is_some_and(|result| {
        result
            .get("error")
            .is_some_and(|error| !error.is_null() && error != "")
    }) || p
        .get("error_message")
        .is_some_and(|error| !error.is_null() && error != "");
    let outcome = if response_failed
        || ["error", "failed", "failure", "timeout", "timed_out"].contains(&status.as_str())
    {
        "failure"
    } else if ["ok", "success", "succeeded", "completed"].contains(&status.as_str()) {
        "success"
    } else {
        "unknown"
    };
    let Ok(home) = std::env::var("HERMES_HOME") else {
        return;
    };
    let dir = Path::new(&home).join("plugin-data/ecc-hooks");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let record = json!({
        "skill_id": id,
        "skill_version": version,
        "task_description": format!("Skill invocation: {id}"),
        "outcome": outcome
    });
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    if let Ok(mut f) = opts.open(dir.join("skill-runs.jsonl")) {
        let _ = writeln!(f, "{}", record);
    }
}
