use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn project() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("foundation-hookify-{nonce}"));
    std::fs::create_dir_all(root.join(".claude")).unwrap();
    root
}

fn invoke(cwd: &std::path::Path, prompt: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .args(["prompt-submit"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let payload = serde_json::json!({
        "cwd": cwd,
        "hook_event_name": "pre_llm_call",
        "user_message": prompt,
        "is_first_turn": false
    });
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    if output.stdout.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

#[test]
fn evaluates_hookify_all_rules_for_prompt_submission() {
    let root = project();
    std::fs::write(
        root.join(".claude/hookify.all.local.md"),
        "---\nevent: all\naction: warn\npattern: credential\n---\nDo not include credential data.\n",
    )
    .unwrap();

    let result = invoke(&root, "review credential handling");
    let _ = std::fs::remove_dir_all(&root);
    assert!(result["context"]
        .as_str()
        .unwrap_or_default()
        .contains("Do not include credential data."));
}

#[test]
fn disabled_hookify_prompt_rule_does_not_block() {
    let root = project();
    std::fs::write(
        root.join(".claude/hookify.disabled.local.md"),
        "---\nenabled: false\nevent: prompt\naction: block\npattern: credential\n---\nDisabled rule.\n",
    )
    .unwrap();

    let result = invoke(&root, "include credential data");
    let _ = std::fs::remove_dir_all(&root);
    assert_ne!(result["action"], "block");
}

#[test]
fn nonmatching_hookify_prompt_rule_does_nothing() {
    let root = project();
    std::fs::write(
        root.join(".claude/hookify.prompt.local.md"),
        "---\nevent: prompt\naction: block\npattern: confidential\n---\nDo not disclose.\n",
    )
    .unwrap();

    let result = invoke(&root, "ordinary request");
    let _ = std::fs::remove_dir_all(&root);
    assert!(result.is_null());
}

#[test]
fn hookify_prompt_defaults_to_warn_action() {
    let root = project();
    std::fs::write(
        root.join(".claude/hookify.all.local.md"),
        "---\nevent: all\npattern: credential\n---\nReview credential handling.\n",
    )
    .unwrap();

    let result = invoke(&root, "review credential handling");
    let _ = std::fs::remove_dir_all(&root);
    assert!(result["context"]
        .as_str()
        .unwrap_or_default()
        .contains("Review credential handling."));
}

#[test]
fn malformed_prompt_rule_fails_open() {
    let root = project();
    std::fs::write(
        root.join(".claude/hookify.invalid.local.md"),
        "---\nevent: prompt\naction: block\npattern: [\n---\ninvalid\n",
    )
    .unwrap();

    let result = invoke(&root, "ordinary prompt");
    let _ = std::fs::remove_dir_all(&root);
    assert!(result.is_null());
}
