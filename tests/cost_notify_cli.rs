use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(hook: &str, payload: &str, home: &std::path::Path) -> (i32, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .env("HERMES_HOME", home)
        .env_remove("FOUNDATION_DESKTOP_NOTIFY")
        .arg(hook)
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
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn usage_is_saved_and_session_total_contains_token_counts_only() {
    let home = std::env::temp_dir().join(format!("foundation-cost-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("plugin-data/foundation")).unwrap();
    std::fs::write(
        home.join("plugin-data/foundation/model-pricing.json"),
        r#"{"model-x":{"input_usd_per_million":2.0,"output_usd_per_million":8.0}}"#,
    )
    .unwrap();
    let request = r#"{"hook_event_name":"post_api_request","session_id":"s1","api_request_id":"r1","response_model":"model-x","usage":{"input_tokens":10,"output_tokens":4,"cache_creation_input_tokens":2,"cache_read_input_tokens":3}}"#;
    let (code, stdout, stderr) = invoke("cost-tracker", request, &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    let (code, _, _) = invoke("cost-tracker", request, &home);
    assert_eq!(code, 0);
    let end = r#"{"hook_event_name":"post_llm_call","session_id":"s1","assistant_response":"private text"}"#;
    let (code, _, _) = invoke("cost-tracker", end, &home);
    assert_eq!(code, 0);
    let path = home.join("plugin-data/foundation/cost_tracker.jsonl");
    let rows: Vec<Value> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["input_tokens"], 10);
    assert_eq!(rows[1]["kind"], "session_total");
    assert_eq!(rows[1]["input_tokens"], 10);
    assert_eq!(rows[1]["output_tokens"], 4);
    assert_eq!(rows[1]["cache_write_tokens"], 2);
    assert_eq!(rows[1]["cache_read_tokens"], 3);
    assert_eq!(rows[1]["cost_status"], "calculated");
    assert_eq!(rows[1]["estimated_cost_usd"], 0.000052);
    assert!(rows[1].get("cost_unavailable_reason").is_none());
    assert!(!rows.iter().any(|r| r.to_string().contains("private text")));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn unknown_model_price_is_recorded_without_a_cost() {
    let home = std::env::temp_dir().join(format!("foundation-cost-unknown-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let request = r#"{"hook_event_name":"post_api_request","session_id":"s-unknown","response_model":"subscription-model","usage":{"input_tokens":10,"output_tokens":4}}"#;
    assert_eq!(invoke("cost-tracker", request, &home).0, 0);
    let end = r#"{"hook_event_name":"post_llm_call","session_id":"s-unknown"}"#;
    assert_eq!(invoke("cost-tracker", end, &home).0, 0);
    let path = home.join("plugin-data/foundation/cost_tracker.jsonl");
    let rows: Vec<Value> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let total = rows.last().unwrap();
    assert_eq!(total["cost_status"], "unavailable");
    assert_eq!(total["cost_unavailable_reason"], "unknown_model_price");
    assert!(total.get("estimated_cost_usd").is_none());
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn desktop_notify_is_noop_on_linux_and_ignores_conversation_content() {
    let home = std::env::temp_dir().join(format!("foundation-notify-{}", std::process::id()));
    let payload = r#"{"hook_event_name":"post_llm_call","session_id":"s1","assistant_response":"private text"}"#;
    let (code, stdout, stderr) = invoke("desktop-notify", payload, &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert!(!home.join("plugin-data/foundation").exists());
    let _ = std::fs::remove_dir_all(home);
}
