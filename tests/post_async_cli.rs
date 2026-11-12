use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(args: &[&str], input: &str, home: &std::path::Path) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .env("HERMES_HOME", home)
        .env("ECC_HERMES_SKILL_TRACK", "1")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn post_async_routes_skill_and_observer_without_persisting_tool_content() {
    let home = std::env::temp_dir().join(format!("foundation-post-async-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("continuous-learning-v2")).unwrap();
    std::fs::write(
        home.join("continuous-learning-v2/config.json"),
        r#"{"observer":{"enabled":true}}"#,
    )
    .unwrap();
    let payload = serde_json::json!({
        "hook_event_name": "post_tool_call",
        "tool_name": "Skill",
        "tool_input": {"skill": "demo"},
        "session_id": "session-1",
        "result": {"private": "must not persist"}
    });
    let (code, stdout, stderr) = invoke(&["post-async"], &payload.to_string(), &home);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.is_empty(), "{stderr}");
    let skill_path = home.join("plugin-data/ecc-hooks/skill-runs.jsonl");
    let skill_data = std::fs::read_to_string(&skill_path).unwrap_or_else(|error| {
        panic!(
            "missing skill record at {}: {error}; home={}",
            skill_path.display(),
            home.display()
        )
    });
    let skill_record: Value = serde_json::from_str(skill_data.trim()).unwrap();
    assert_eq!(skill_record["skill_id"], "demo");
    let observe_path = home.join("plugin-data/ecc-hooks/continuous-learning-observations.jsonl");
    let observe_record: Value =
        serde_json::from_str(std::fs::read_to_string(observe_path).unwrap().trim()).unwrap();
    assert_eq!(observe_record["tool_name"], "skill");
    assert!(!skill_record.to_string().contains("must not persist"));
    assert!(!observe_record.to_string().contains("must not persist"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn post_async_ignores_other_events() {
    let home = std::env::temp_dir().join(format!(
        "foundation-post-async-ignore-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&home);
    let (code, stdout, stderr) = invoke(
        &["post-async"],
        r#"{"hook_event_name":"on_session_start","tool_name":"skill","tool_input":{"skill":"demo"}}"#,
        &home,
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert!(!home.join("plugin-data/ecc-hooks/skill-runs.jsonl").exists());
    let _ = std::fs::remove_dir_all(&home);
}
