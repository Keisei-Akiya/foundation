use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(args: &[&str], input: &str) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .env_remove("ECC_HERMES_SKILL_TRACK")
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

fn invoke_with_env(args: &[&str], input: &str, home: &std::path::Path) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .env("ECC_HERMES_SKILL_TRACK", "1")
        .env("HERMES_HOME", home)
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
fn preflight_emits_hermes_block_action_for_literal_no_verify() {
    let (code, stdout, _) = invoke(
        &["preflight"],
        r#"{"tool_name":"terminal","tool_input":{"command":"git commit --no-verify"}}"#,
    );
    assert_eq!(code, 0);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(value["action"], "block");
    assert!(value["message"].as_str().unwrap().contains("--no-verify"));
}

#[test]
fn hookify_blocks_matching_project_bash_rule() {
    let root = std::env::temp_dir().join(format!("ecc-hookify-test-{}", std::process::id()));
    let rules = root.join(".claude");
    std::fs::create_dir_all(&rules).unwrap();
    std::fs::write(
        rules.join("hookify.no-rm.local.md"),
        "---\nname: no-rm\nevent: bash\naction: block\npattern: rm -rf\n---\nDo not recursively delete files.\n",
    ).unwrap();
    let payload = serde_json::json!({"cwd":root,"tool_name":"terminal","tool_input":{"command":"rm -rf /tmp/demo"}}).to_string();
    let (code, stdout, _) = invoke(&["hookify"], &payload);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(code, 0);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(value["decision"], "block");
    assert!(value["reason"]
        .as_str()
        .unwrap()
        .contains("Do not recursively delete"));
}

#[test]
fn hookify_ignores_non_matching_rule() {
    let root = std::env::temp_dir().join(format!("ecc-hookify-clean-{}", std::process::id()));
    let rules = root.join(".claude");
    std::fs::create_dir_all(&rules).unwrap();
    std::fs::write(
        rules.join("hookify.rule.local.md"),
        "---\nname: no-rm\nevent: bash\naction: block\npattern: rm -rf\n---\nBlocked\n",
    )
    .unwrap();
    let payload = serde_json::json!({"cwd":root,"tool_name":"terminal","tool_input":{"command":"git status"}}).to_string();
    let (code, stdout, _) = invoke(&["hookify"], &payload);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
}

#[test]
fn lifecycle_is_disabled_without_explicit_opt_in() {
    let (code, stdout, stderr) = invoke(
        &["lifecycle"],
        r#"{"hook_event_name":"post_tool_call","tool_name":"skill","tool_input":{"skill":"demo"},"status":"success"}"#,
    );
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
}

#[test]
fn lifecycle_records_hermes_skill_failure_without_persisting_error_text() {
    let home = std::env::temp_dir().join(format!("foundation-lifecycle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let payload = serde_json::json!({
        "hook_event_name": "post_tool_call",
        "tool_name": "Skill",
        "tool_input": {"skill": "demo", "version": "1.2.3"},
        "result": {"error": "private failure detail"},
        "status": "error",
        "error_type": "tool_error",
        "error_message": "private failure detail"
    })
    .to_string();
    let (code, stdout, stderr) = invoke_with_env(&["lifecycle"], &payload, &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    let record_path = home.join("plugin-data/ecc-hooks/skill-runs.jsonl");
    let record: Value =
        serde_json::from_str(std::fs::read_to_string(&record_path).unwrap().trim()).unwrap();
    assert_eq!(record["skill_id"], "demo");
    assert_eq!(record["skill_version"], "1.2.3");
    assert_eq!(record["outcome"], "failure");
    assert_eq!(record["task_description"], "Skill invocation: demo");
    assert!(!record.to_string().contains("private failure detail"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn lifecycle_records_hermes_success_status() {
    let home = std::env::temp_dir().join(format!("foundation-lifecycle-ok-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let payload = serde_json::json!({
        "hook_event_name": "post_tool_call",
        "tool_name": "skill",
        "tool_input": {"skill": "demo"},
        "status": "ok"
    })
    .to_string();
    let (code, _, _) = invoke_with_env(&["lifecycle"], &payload, &home);
    assert_eq!(code, 0);
    let record_path = home.join("plugin-data/ecc-hooks/skill-runs.jsonl");
    let record: Value =
        serde_json::from_str(std::fs::read_to_string(&record_path).unwrap().trim()).unwrap();
    assert_eq!(record["outcome"], "success");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn observe_is_opt_in_and_persists_no_tool_arguments_or_session_id() {
    let home = std::env::temp_dir().join(format!("foundation-observe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let payload = r#"{"tool_name":"terminal","tool_input":{"command":"secret command"},"session_id":"private-session"}"#;
    let (code, _, _) = invoke_with_env(&["observe"], payload, &home);
    assert_eq!(code, 0);
    let path = home.join("plugin-data/ecc-hooks/continuous-learning-observations.jsonl");
    assert!(!path.exists());
    let config = home.join("continuous-learning-v2/config.json");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, r#"{"observer":{"enabled":true}}"#).unwrap();
    let (code, _, _) = invoke_with_env(&["observe"], payload, &home);
    assert_eq!(code, 0);
    let record = std::fs::read_to_string(path).unwrap();
    assert!(record.contains("\"tool_name\":\"terminal\""));
    assert!(!record.contains("secret command"));
    assert!(!record.contains("private-session"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn session_finalize_writes_private_marker_with_boundary_metadata() {
    let home = std::env::temp_dir().join(format!("foundation-marker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let payload = serde_json::json!({
        "hook_event_name": "on_session_finalize",
        "session_id": "session-123",
        "platform": "cli",
        "reason": "exit"
    })
    .to_string();
    let (code, stdout, stderr) = invoke_with_env(&["session-hooks"], &payload, &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    let marker = home.join("plugin-data/ecc-hooks/session-markers.jsonl");
    let record: Value =
        serde_json::from_str(std::fs::read_to_string(&marker).unwrap().trim()).unwrap();
    assert_eq!(record["event"], "session:end:marker");
    assert_eq!(record["session_id"], "session-123");
    assert_eq!(record["platform"], "cli");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(marker).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn session_start_detects_project_lockfile_without_emitting_hook_output() {
    let root =
        std::env::temp_dir().join(format!("foundation-session-start-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();
    let payload = serde_json::json!({
        "hook_event_name": "on_session_start",
        "cwd": root,
        "session_id": "session-456"
    })
    .to_string();
    let (code, stdout, stderr) = invoke(&["session-hooks"], &payload);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.contains("\"manager\":\"pnpm\""));
    assert!(stderr.contains("\"lockfile\":\"pnpm-lock.yaml\""));
    let _ = std::fs::remove_dir_all(&root);
}
