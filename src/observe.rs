use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn config_enabled(home: &std::path::Path) -> bool {
    if home.join("continuous-learning-v2/disabled").exists() {
        return false;
    }
    let path = std::env::var_os("CLV2_CONFIG")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| home.join("continuous-learning-v2/config.json"));
    let Ok(data) = fs::read(path) else {
        return false;
    };
    serde_json::from_slice::<Value>(&data)
        .ok()
        .and_then(|v| v.pointer("/observer/enabled").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn safe_tool_name(value: Option<&Value>) -> Option<String> {
    let name = value?.as_str()?.trim();
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
    {
        return None;
    }
    Some(name.to_ascii_lowercase())
}

pub fn run(payload: &Value) {
    // Match the upstream observer's explicit observer.enabled gate and disabled marker.
    let Some(home) = std::env::var_os("HERMES_HOME").map(PathBuf::from) else {
        return;
    };
    if !config_enabled(&home) {
        return;
    }
    let Some(tool) = safe_tool_name(payload.get("tool_name")) else {
        return;
    };
    let path = home.join("plugin-data/ecc-hooks/continuous-learning-observations.jsonl");
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let record = json!({"event":"pre:observe:continuous-learning", "tool_name":tool});
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    let _ = writeln!(file, "{record}");
}
