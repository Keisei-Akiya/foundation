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

fn run_raw(command: &str, payload: &str, home: &std::path::Path) -> (i32, String) {
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
        .write_all(payload.as_bytes())
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
    let root = home.join("plugin-data/foundation");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("session-summary.enabled"), "").unwrap();
    let payload = json!({"hook_event_name":"post_llm_call", "session_id":"session-123", "conversation_history":[{"role":"user","content":"Implement hookify feature in session_summary.rs /home/private TOKEN=secret run cargo test"},{"type":"tool_result","tool_name":"todo","todos":[{"status":"pending","content":"private todo /home/private"},{"status":"completed","content":"private completed todo"}]}], "assistant_response":"private response", "model":"m", "platform":"cli"});
    let (code, stdout) = run("session-end", &payload, &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    let path = home.join("plugin-data/foundation/session-summary.jsonl");
    let text = std::fs::read_to_string(path).unwrap();
    let record: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(record["session_id"], "session-123");
    assert!(record["summary"]
        .as_str()
        .unwrap()
        .contains("Purpose: Software implementation."));
    assert!(record["summary"]
        .as_str()
        .unwrap()
        .contains("Progress: 1/2 structured tasks completed"));
    assert!(record["summary"]
        .as_str()
        .unwrap()
        .contains("Unfinished: 1 structured tasks remain open."));
    assert!(!text.contains("Implement feature") && !text.contains("private todo"));
    assert!(
        !text.contains("/home/") && !text.contains("TOKEN=secret") && !text.contains("cargo test")
    );
    assert!(!text.contains("private response"));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn session_end_is_disabled_by_default() {
    let home = temp_home("session-summary-default-off");
    let payload = json!({"hook_event_name":"post_llm_call", "session_id":"s-default", "conversation_history":[{"role":"user","content":"Implement session_summary.rs"}]});
    assert_eq!(run("session-end", &payload, &home).0, 0);
    assert!(!home
        .join("plugin-data/foundation/session-summary.jsonl")
        .exists());
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn session_end_retains_local_work_identifiers_and_discards_unsafe_items() {
    let home = temp_home("session-summary-filter");
    let root = home.join("plugin-data/foundation");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("session-summary.enabled"), "").unwrap();
    let payload = json!({"hook_event_name":"post_llm_call", "session_id":"s-filter-safe", "conversation_history":[{"role":"user","content":"Implement hookify using session_summary.rs"}]});
    assert_eq!(run("session-end", &payload, &home).0, 0);
    let safe_text = std::fs::read_to_string(root.join("session-summary.jsonl")).unwrap();
    for allowed in ["hookify", "session_summary.rs"] {
        assert!(
            safe_text.contains(allowed),
            "missing {allowed}: {safe_text}"
        );
    }
    let windows_path = format!(
        "C:{}Users{}rocky{}secret.txt",
        char::from(92),
        char::from(92),
        char::from(92)
    );
    for (index, forbidden) in [
        "token/password/secret/key=private-value".to_string(),
        "Authorization: Bearer abcdef...wxyz".to_string(),
        "sk-123456789012345678901234567890".to_string(),
        "/home/rocky/x".to_string(),
        "~/.hermes/.env".to_string(),
        "`rm -rf important`".to_string(),
        "$ cargo test".to_string(),
        "a | b".to_string(),
        "../private/thing".to_string(),
        "a/b/c".to_string(),
        windows_path,
        "https://user:password@example.com".to_string(),
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".to_string(),
    ]
    .iter()
    .enumerate()
    {
        let payload = json!({"hook_event_name":"post_llm_call", "session_id":format!("s-filter-{index}"), "conversation_history":[{"role":"user","content":forbidden}]});
        assert_eq!(run("session-end", &payload, &home).0, 0);
        let text = std::fs::read_to_string(root.join("session-summary.jsonl")).unwrap();
        assert!(!text.contains(forbidden), "leaked {forbidden}: {text}");
    }
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn session_end_caps_summary_and_handles_malformed_history_without_panicking() {
    let home = temp_home("session-summary-bound");
    let root = home.join("plugin-data/foundation");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("session-summary.enabled"), "").unwrap();
    let payload = json!({"hook_event_name":"post_llm_call", "session_id":"s-bound", "conversation_history":[null, 4, "bad", {"todos":[null, {"status":true}]}, {"role":"user","content":"x".repeat(5000)}]});
    assert_eq!(run("session-end", &payload, &home).0, 0);
    let text = std::fs::read_to_string(root.join("session-summary.jsonl")).unwrap();
    let record: Value = serde_json::from_str(text.trim()).unwrap();
    assert!(record["summary"].as_str().unwrap().len() <= 512);
    assert!(record["summary"]
        .as_str()
        .unwrap()
        .contains("omitted because it exceeded"));
    let malformed_home = temp_home("session-summary-malformed-json");
    let malformed_root = malformed_home.join("plugin-data/foundation");
    std::fs::create_dir_all(&malformed_root).unwrap();
    std::fs::write(malformed_root.join("session-summary.enabled"), "").unwrap();
    assert_eq!(
        run_raw("session-end", "{invalid JSON", &malformed_home).0,
        0
    );
    assert!(!malformed_root.join("session-summary.jsonl").exists());
    let _ = std::fs::remove_dir_all(malformed_home);
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
