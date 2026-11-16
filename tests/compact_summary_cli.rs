use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(payload: &Value, home: &std::path::Path) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .args(["compact-summary"])
        .env("HERMES_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn constructed_hook_json_saves_redacted_summary_and_restores_after_compaction() {
    let home =
        std::env::temp_dir().join(format!("foundation-compact-summary-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let data = home.join("plugin-data/foundation");
    std::fs::create_dir_all(&data).unwrap();
    std::fs::write(data.join("compact-summary.enabled"), "").unwrap();
    let history = serde_json::json!([
        {"role":"user","content":"Implement a feature; export TOKEN=verysecret; cargo test; /home/private/project"},
        {"tool_name":"todo","todos":[
            {"status":"completed","content":"completed work"},
            {"status":"pending","content":"unfinished work"}
        ]}
    ]);
    let post_turn = serde_json::json!({
        "hook_event_name":"post_llm_call",
        "session_id":"compact-test-session",
        "conversation_history":history
    });
    assert!(invoke(&post_turn, &home).is_empty());
    let compacted = serde_json::json!({
        "hook_event_name":"post_compaction",
        "session_id":"compact-test-session",
        "old_session_id":"old",
        "kind":"auto",
        "reason":"threshold",
        "before_tokens":1000,
        "after_tokens":500,
        "messages_removed":4
    });
    assert!(invoke(&compacted, &home).is_empty());
    let before = std::fs::read_to_string(data.join("compact-summary.jsonl")).unwrap();
    for forbidden in ["verysecret", "cargo test", "/home/private", "export TOKEN"] {
        assert!(!before.contains(forbidden), "summary leaked {forbidden}");
    }
    assert!(before.contains("Purpose: Software implementation."));
    assert!(before.contains("1/2 structured tasks completed"));
    assert!(before.contains("1 structured tasks remain open."));

    let pre_turn = serde_json::json!({
        "hook_event_name":"pre_llm_call",
        "session_id":"compact-test-session",
        "user_message":"continue"
    });
    let restored: Value = serde_json::from_str(&invoke(&pre_turn, &home)).unwrap();
    let context = restored["context"].as_str().unwrap();
    assert!(context.contains("Purpose: Software implementation."));
    assert!(context.contains("1 structured tasks remain open."));
    let second_restore = invoke(&pre_turn, &home);
    assert!(second_restore.is_empty(), "restoration is consumed once");
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn compact_summary_does_not_write_without_opt_in() {
    let home = std::env::temp_dir().join(format!(
        "foundation-compact-summary-off-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&home);
    let payload = serde_json::json!({
        "hook_event_name":"post_llm_call",
        "session_id":"compact-disabled-session",
        "conversation_history":[{"role":"user","content":"Implement something"}]
    });
    assert!(invoke(&payload, &home).is_empty());
    assert!(!home.exists());
    let _ = std::fs::remove_dir_all(home);
}
