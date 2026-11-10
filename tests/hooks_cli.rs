use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(args: &[&str], input: &str) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ecc-hermes-hooks"))
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
