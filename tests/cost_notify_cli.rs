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
        r#"{"model-x":{"input_usd_per_million":2.0,"output_usd_per_million":8.0,"cache_write_usd_per_million":10.0,"cache_read_usd_per_million":0.5},"model-y":{"input_usd_per_million":1.0,"output_usd_per_million":4.0}}"#,
    )
    .unwrap();
    let request = r#"{"hook_event_name":"post_api_request","session_id":"s1","api_request_id":"r1","response_model":"model-x","usage":{"input_tokens":10,"output_tokens":4,"cache_creation_input_tokens":2,"cache_read_input_tokens":3}}"#;
    let (code, stdout, stderr) = invoke("cost-tracker", request, &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    let (code, _, _) = invoke("cost-tracker", request, &home);
    assert_eq!(code, 0);
    let second = r#"{"hook_event_name":"post_api_request","session_id":"s1","api_request_id":"r2","response_model":"model-y","usage":{"input_tokens":5,"output_tokens":1}}"#;
    assert_eq!(invoke("cost-tracker", second, &home).0, 0);
    let end = r#"{"hook_event_name":"post_llm_call","session_id":"s1","assistant_response":"private text"}"#;
    let (code, _, _) = invoke("cost-tracker", end, &home);
    assert_eq!(code, 0);
    let path = home.join("plugin-data/foundation/cost_tracker.jsonl");
    let rows: Vec<Value> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["input_tokens"], 10);
    assert_eq!(rows[2]["kind"], "session_total");
    assert_eq!(rows[2]["input_tokens"], 15);
    assert_eq!(rows[2]["output_tokens"], 5);
    assert_eq!(rows[2]["cache_write_tokens"], 2);
    assert_eq!(rows[2]["cache_read_tokens"], 3);
    assert_eq!(rows[2]["cost_status"], "calculated");
    assert_eq!(rows[2]["estimated_cost_usd"], 0.0000825);
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
fn cached_usage_without_cache_rates_is_unpriced_instead_of_underestimated() {
    let home = std::env::temp_dir().join(format!("foundation-cost-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("plugin-data/foundation")).unwrap();
    std::fs::write(
        home.join("plugin-data/foundation/model-pricing.json"),
        r#"{"model-x":{"input_usd_per_million":2.0,"output_usd_per_million":8.0}}"#,
    )
    .unwrap();
    let request = r#"{"hook_event_name":"post_api_request","session_id":"s-cache","response_model":"model-x","usage":{"input_tokens":10,"output_tokens":4,"cache_creation_input_tokens":2}}"#;
    assert_eq!(invoke("cost-tracker", request, &home).0, 0);
    let end = r#"{"hook_event_name":"post_llm_call","session_id":"s-cache"}"#;
    assert_eq!(invoke("cost-tracker", end, &home).0, 0);
    let path = home.join("plugin-data/foundation/cost_tracker.jsonl");
    let rows: Vec<Value> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let total = rows.last().unwrap();
    assert_eq!(total["cost_status"], "unavailable");
    assert_eq!(total["cost_unavailable_reason"], "unknown_cache_price");
    assert!(total.get("estimated_cost_usd").is_none());
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn sub_microdollar_estimates_keep_precision() {
    let home =
        std::env::temp_dir().join(format!("foundation-cost-precision-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("plugin-data/foundation")).unwrap();
    std::fs::write(
        home.join("plugin-data/foundation/model-pricing.json"),
        r#"{"model-x":{"input_usd_per_million":0.01,"output_usd_per_million":0.02}}"#,
    )
    .unwrap();
    let request = r#"{"hook_event_name":"post_api_request","session_id":"s-precision","response_model":"model-x","usage":{"input_tokens":1,"output_tokens":1}}"#;
    assert_eq!(invoke("cost-tracker", request, &home).0, 0);
    let end = r#"{"hook_event_name":"post_llm_call","session_id":"s-precision"}"#;
    assert_eq!(invoke("cost-tracker", end, &home).0, 0);
    let path = home.join("plugin-data/foundation/cost_tracker.jsonl");
    let rows: Vec<Value> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let estimated = rows.last().unwrap()["estimated_cost_usd"].as_f64().unwrap();
    assert!((estimated - 0.00000003).abs() < 1e-15);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn cost_tracker_fails_open_when_state_store_cannot_be_created() {
    let home =
        std::env::temp_dir().join(format!("foundation-cost-fail-open-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::write(&home, "not a directory").unwrap();
    let request =
        r#"{"hook_event_name":"post_api_request","session_id":"s1","usage":{"input_tokens":10}}"#;
    let (code, stdout, stderr) = invoke("cost-tracker", request, &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    let _ = std::fs::remove_file(home);
}

#[test]
fn cost_tracker_fails_open_when_append_fails() {
    let home = std::env::temp_dir().join(format!(
        "foundation-cost-append-fail-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&home);
    let root = home.join("plugin-data/foundation");
    std::fs::create_dir_all(root.join("cost_tracker.jsonl")).unwrap();
    let request =
        r#"{"hook_event_name":"post_api_request","session_id":"s1","usage":{"input_tokens":10}}"#;
    let (code, stdout, stderr) = invoke("cost-tracker", request, &home);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn cost_report_displays_the_session_amount_to_the_user() {
    let home = std::env::temp_dir().join(format!("foundation-cost-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("plugin-data/foundation")).unwrap();
    std::fs::write(
        home.join("plugin-data/foundation/cost_tracker.jsonl"),
        "{\"kind\":\"session_total\",\"session_id\":\"s1\",\"input_tokens\":15,\"output_tokens\":5,\"cache_write_tokens\":2,\"cache_read_tokens\":3,\"cost_status\":\"calculated\",\"estimated_cost_usd\":0.0000825}\n",
    )
    .unwrap();
    let (code, stdout, stderr) = invoke("cost-report", "", &home);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("s1"));
    assert!(stdout.contains("15"));
    assert!(stdout.contains("5"));
    assert!(stdout.contains("$0.00008250"));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn cost_report_explains_when_a_session_price_is_unknown() {
    let home = std::env::temp_dir().join(format!(
        "foundation-cost-report-unknown-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("plugin-data/foundation")).unwrap();
    std::fs::write(
        home.join("plugin-data/foundation/cost_tracker.jsonl"),
        "{\"kind\":\"session_total\",\"session_id\":\"s-unknown\",\"input_tokens\":10,\"output_tokens\":4,\"cost_status\":\"unavailable\",\"cost_unavailable_reason\":\"unknown_model_price\"}\n",
    )
    .unwrap();
    let (code, stdout, stderr) = invoke("cost-report", "", &home);
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("s-unknown"));
    assert!(stdout.contains("unavailable"));
    assert!(stdout.contains("No configured price for this model"));
    assert!(!stdout.contains("$0.00000000"));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn cost_report_identifies_sessions_aggregated_from_multiple_models() {
    let home = std::env::temp_dir().join(format!(
        "foundation-cost-report-multi-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("plugin-data/foundation")).unwrap();
    std::fs::write(
        home.join("plugin-data/foundation/cost_tracker.jsonl"),
        "{\"kind\":\"session_total\",\"session_id\":\"s-multi\",\"model\":\"model-a\",\"models\":[\"model-a\",\"model-b\"],\"input_tokens\":15,\"output_tokens\":5,\"cost_status\":\"calculated\",\"estimated_cost_usd\":0.0000825}\n",
    ).unwrap();

    let (code, stdout, stderr) = invoke("cost-report", "", &home);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(
        stdout.contains("multiple models: model-a, model-b"),
        "{stdout}"
    );
    assert!(stdout.contains("$0.00008250"));
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn cost_report_marks_incomplete_totals_instead_of_showing_zero_tokens() {
    let home = std::env::temp_dir().join(format!(
        "foundation-cost-report-incomplete-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("plugin-data/foundation")).unwrap();
    std::fs::write(
        home.join("plugin-data/foundation/cost_tracker.jsonl"),
        "{\"kind\":\"session_total\",\"session_id\":\"s-broken\",\"cost_status\":\"unavailable\"}\n",
    ).unwrap();

    let (code, stdout, stderr) = invoke("cost-report", "", &home);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("invalid/incomplete record"), "{stdout}");
    assert!(!stdout.contains("s-broken\tunknown\t0\t0"), "{stdout}");
    assert!(!stdout.contains("$0.00000000"));
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
