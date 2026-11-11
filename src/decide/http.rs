use super::target::Target;
use std::time::Duration;

pub(super) fn send(t: &Target, timeout: u64) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(timeout)))
        .build()
        .into();
    let mut resp = agent
        .post(&t.url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header(t.auth_header.0, &t.auth_header.1)
        .send(t.body.to_string())
        // 接続エラー文言に認証情報は含まれないが、念のため URL もヘッダも出さない。
        .map_err(|e| format!("request failed: {}", scrub(&e.to_string(), t)))?;
    let status = resp.status().as_u16();
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("cannot read response: {e}"))?;
    if (200..300).contains(&status) {
        Ok(text)
    } else {
        Err(format!("HTTP {status}: {}", scrub(&text, t)))
    }
}

fn scrub(text: &str, t: &Target) -> String {
    let secret = t.auth_header.1.trim_start_matches("Bearer ");
    if secret.is_empty() {
        text.to_string()
    } else {
        text.replace(secret, "[REDACTED]")
    }
}
