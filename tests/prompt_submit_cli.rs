use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(input: &str, home: &std::path::Path) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .args(["prompt-submit"])
        .env("HERMES_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
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
        String::from_utf8(output.stdout).unwrap(),
    )
}

#[test]
fn blocks_matching_prompt_rule_using_hermes_contract() {
    let root = std::env::temp_dir().join(format!("foundation-prompt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".claude")).unwrap();
    std::fs::write(root.join(".claude/hookify.no-secret.local.md"),
        "---\nname: no-secret\nevent: prompt\naction: block\npattern: secret\n---\nDo not submit secrets.\n").unwrap();
    let payload = serde_json::json!({"cwd":root,"hook_event_name":"pre_llm_call","user_message":"send SECRET now"}).to_string();
    let (code, stdout) = invoke(&payload, &root);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(code, 0);
    let result: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(result["action"], "block");
    assert!(result["message"]
        .as_str()
        .unwrap()
        .contains("Do not submit secrets"));
}

#[test]
fn injects_last_session_summary_only_on_first_turn() {
    let home = std::env::temp_dir().join(format!("foundation-summary-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let store = home.join("plugin-data/foundation");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(
        store.join("session-summary.jsonl"),
        "{\"summary\":\"Resume task 42\"}\n",
    )
    .unwrap();
    let payload = |first| {
        serde_json::json!({"cwd":"/nonexistent/project","hook_event_name":"pre_llm_call","user_message":"hello","is_first_turn":first}).to_string()
    };
    let (code, stdout) = invoke(&payload(true), &home);
    assert_eq!(code, 0);
    let result: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(result["context"]
        .as_str()
        .unwrap()
        .contains("Resume task 42"));
    let (code, stdout) = invoke(&payload(false), &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn prompt_hook_fails_open_without_matching_rules_or_summary() {
    let home = std::env::temp_dir().join(format!("foundation-prompt-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let payload = serde_json::json!({"cwd":home,"hook_event_name":"pre_llm_call","user_message":"hello","is_first_turn":true}).to_string();
    let (code, stdout) = invoke(&payload, &home);
    let _ = std::fs::remove_dir_all(&home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
}
