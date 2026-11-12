use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

fn run(command: &str, payload: &Value, home: &std::path::Path) -> (i32, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .env("HERMES_HOME", home)
        .args([command])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

fn temp_home(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("foundation-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn history(user_count: usize) -> Vec<Value> {
    (0..user_count)
        .map(|_| json!({"role":"user", "content":"private prompt"}))
        .collect()
}

#[test]
fn session_end_persists_only_minimal_summary_metadata_on_post_llm_call() {
    let home = temp_home("session-summary");
    let payload = json!({"hook_event_name":"post_llm_call", "session_id":"session-123", "conversation_history":history(2), "assistant_response":"private response", "model":"m", "platform":"cli"});
    let (code, stdout) = run("session-end", &payload, &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    let path = home.join("plugin-data/foundation/session-summary.jsonl");
    let text = std::fs::read_to_string(path).unwrap();
    let record: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(record["user_message_count"], 2);
    assert_eq!(record["session_id"], "session-123");
    assert!(!text.contains("private prompt") && !text.contains("private response"));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn evaluate_session_marks_threshold_and_skips_short_sessions() {
    let home = temp_home("evaluate-session");
    let below = json!({"hook_event_name":"post_llm_call", "session_id":"s1", "conversation_history":history(9)});
    let (code, _) = run("evaluate-session", &below, &home);
    assert_eq!(code, 0);
    let path = home.join("plugin-data/foundation/session-evaluation.jsonl");
    assert!(!path.exists());
    let at_threshold = json!({"hook_event_name":"post_llm_call", "session_id":"s2", "conversation_history":history(10), "assistant_response":"secret"});
    let (code, _) = run("evaluate-session", &at_threshold, &home);
    assert_eq!(code, 0);
    let text = std::fs::read_to_string(path).unwrap();
    let record: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(record["user_message_count"], 10);
    assert_eq!(record["status"], "eligible_for_manual_review");
    assert!(!text.contains("private prompt") && !text.contains("secret"));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn session_hooks_ignore_other_events_and_disabled_series() {
    let home = temp_home("session-disabled");
    let root = home.join("plugin-data/foundation");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("session-summary.disabled"), "").unwrap();
    let payload = json!({"hook_event_name":"post_tool_call", "session_id":"s1", "conversation_history":history(10)});
    assert_eq!(run("session-end", &payload, &home).0, 0);
    assert_eq!(run("evaluate-session", &payload, &home).0, 0);
    assert!(!root.join("session-summary.jsonl").exists());
    assert!(!root.join("session-evaluation.jsonl").exists());
    let _ = std::fs::remove_dir_all(home);
}
