use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;

/// Hermes `on_session_start`/`on_session_finalize` side effects that do not
/// depend on Claude Code's hook-stdout protocol.
pub fn run(payload: &Value) {
    let Some(event) = payload.get("hook_event_name").and_then(Value::as_str) else {
        return;
    };
    match event {
        "on_session_start" => {
            session_start(payload);
            append_marker(payload, "session:start:marker");
        }
        "on_session_end" => append_marker(payload, "session:turn:end"),
        "on_session_finalize" => append_marker(payload, "session:end:marker"),
        _ => {}
    }
}

fn session_start(payload: &Value) {
    let cwd = payload
        .get("cwd")
        .and_then(Value::as_str)
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    let manager = [
        ("bun", "bun.lockb"),
        ("bun", "bun.lock"),
        ("pnpm", "pnpm-lock.yaml"),
        ("yarn", "yarn.lock"),
        ("npm", "package-lock.json"),
        ("cargo", "Cargo.lock"),
        ("poetry", "poetry.lock"),
        ("uv", "uv.lock"),
        ("pip", "requirements.txt"),
        ("composer", "composer.lock"),
        ("bundler", "Gemfile.lock"),
    ]
    .iter()
    .find(|(_, lockfile)| cwd.join(lockfile).is_file())
    .map(|(name, lockfile)| json!({"manager": name, "lockfile": lockfile}));
    if let Some(manager) = manager {
        eprintln!(
            "[foundation/session-start] package manager detected: {}",
            manager
        );
    }
    // Hermes builds the system prompt before `on_session_start` and ignores
    // observer return values. ECC's restored-context injection therefore has
    // no equivalent at this boundary; this callback intentionally only reports
    // detection and does not pretend to inject context.
}

fn append_marker(payload: &Value, event: &str) {
    let Ok(home) = std::env::var("HERMES_HOME") else {
        return;
    };
    let dir = Path::new(&home).join("plugin-data/foundation");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let mut record = json!({
        "event": event,
        "session_id": payload.get("session_id").and_then(Value::as_str).unwrap_or(""),
    });
    for field in [
        "platform",
        "reason",
        "completed",
        "interrupted",
        "turn_exit_reason",
    ] {
        if let Some(value) = payload.get(field) {
            record[field] = value.clone();
        }
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(dir.join("session-markers.jsonl")) {
        let _ = writeln!(file, "{record}");
    }
}
