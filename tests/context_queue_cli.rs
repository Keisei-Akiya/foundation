use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(command: &str, payload: &str, home: &std::path::Path) -> String {
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn doc_warning_queues_only_suspicious_write_paths_and_pre_llm_drains_per_session() {
    let home = std::env::temp_dir().join(format!("foundation-doc-warning-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let base = serde_json::json!({"hook_event_name":"pre_tool_call","tool_name":"write_file","session_id":"s1"});
    let mut payload = base.clone();
    payload["tool_input"] = serde_json::json!({"path":"/tmp/NOTES.md"});
    assert!(invoke("doc-warning", &payload.to_string(), &home).is_empty());
    let mut allowed = base;
    allowed["tool_input"] = serde_json::json!({"path":"/repo/docs/NOTES.md"});
    invoke("doc-warning", &allowed.to_string(), &home);
    let drained = invoke(
        "doc-warning",
        r#"{"hook_event_name":"pre_llm_call","session_id":"s1"}"#,
        &home,
    );
    let value: Value = serde_json::from_str(&drained).unwrap();
    assert!(value["context"].as_str().unwrap().contains("/tmp/NOTES.md"));
    assert!(invoke(
        "doc-warning",
        r#"{"hook_event_name":"pre_llm_call","session_id":"s1"}"#,
        &home
    )
    .is_empty());
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn suggest_compact_counts_all_tool_calls_per_session_and_queues_at_threshold() {
    let home = std::env::temp_dir().join(format!("foundation-compact-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let payload = serde_json::json!({"hook_event_name":"pre_tool_call","tool_name":"read_file","session_id":"s1","tool_input":{"path":"/repo/src/a.rs"}}).to_string();
    for _ in 0..49 {
        invoke("suggest-compact", &payload, &home);
    }
    assert!(invoke("suggest-compact", &payload, &home).is_empty());
    let drained = invoke(
        "doc-warning",
        r#"{"hook_event_name":"pre_llm_call","session_id":"s1"}"#,
        &home,
    );
    assert!(drained.contains("50"), "drained={drained:?}");
    let _ = std::fs::remove_dir_all(home);
}
