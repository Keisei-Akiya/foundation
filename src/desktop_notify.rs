//! Opt-in long-turn completion notifications via a configured Discord webhook.
use serde_json::{json, Value};
use std::time::Duration;

const DEFAULT_THRESHOLD_SECONDS: u64 = 600;

fn enabled() -> bool {
    std::env::var("FOUNDATION_DESKTOP_NOTIFY")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn threshold_seconds() -> u64 {
    std::env::var("FOUNDATION_DESKTOP_NOTIFY_THRESHOLD_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_THRESHOLD_SECONDS)
}

fn trusted_webhook_url(value: &str) -> bool {
    let Ok(url) = ureq::http::Uri::try_from(value) else {
        return false;
    };
    match (url.scheme_str(), url.host()) {
        (Some("https"), Some(host)) => !host.is_empty(),
        (Some("http"), Some("127.0.0.1" | "localhost" | "[::1]")) => true,
        _ => false,
    }
}

/// Send only a constant, content-free message. Errors are intentionally ignored.
pub fn run(payload: &Value) {
    if !enabled() {
        return;
    }
    let Some(elapsed) = payload.get("elapsed_seconds").and_then(Value::as_u64) else {
        return;
    };
    if elapsed < threshold_seconds() {
        return;
    }
    let Ok(webhook) = std::env::var("FOUNDATION_DESKTOP_NOTIFY_WEBHOOK") else {
        return;
    };
    if !trusted_webhook_url(&webhook) {
        return;
    }

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .new_agent();
    let body = json!({"content": format!("Long task completed ({elapsed}s).")});
    let _ = agent
        .post(&webhook)
        .header("content-type", "application/json")
        .send(body.to_string());
}
