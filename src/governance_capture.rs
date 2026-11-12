use serde_json::{json, Value};
use std::io::Write;

const MAX_TEXT: usize = 1_048_576;

fn matches_secret(text: &str) -> Vec<&'static str> {
    let patterns: &[(&str, &str)] = &[
        ("aws_key", r"(?i)(AKIA|ASIA)[A-Z0-9]{16}"),
        (
            "private_key",
            r"-----BEGIN (RSA |EC |DSA )?PRIVATE KEY-----",
        ),
        ("github_token", r"gh[pousr]_[A-Za-z0-9_]{36,}"),
        (
            "jwt",
            r"eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
        ),
        (
            "generic_secret",
            r#"(?i)(secret|password|token|api[_-]?key)\s*[:=]\s*[\"'][^\"']{8,}"#,
        ),
    ];
    patterns
        .iter()
        .filter_map(|(name, pattern)| {
            regex::Regex::new(pattern)
                .ok()?
                .is_match(text)
                .then_some(*name)
        })
        .collect()
}

fn sensitive_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.contains(".env")
        || p.contains("credential")
        || p.contains("secret.")
        || p.ends_with(".pem")
        || p.ends_with(".key")
        || p.contains("id_rsa")
}

pub fn run(payload: &Value) {
    if std::env::var("ECC_GOVERNANCE_CAPTURE").ok().as_deref() != Some("1") {
        return;
    }
    let Some(tool) = payload.get("tool_name").and_then(Value::as_str) else {
        return;
    };
    let input = payload.get("tool_input").unwrap_or(&Value::Null);
    let text = match serde_json::to_string(input) {
        Ok(s) if s.len() <= MAX_TEXT => s,
        _ => return,
    };
    let session = payload
        .get("session_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let phase = "pre";
    let mut events = Vec::new();
    let secrets = matches_secret(&text);
    if !secrets.is_empty() {
        events.push(json!({"eventType":"secret_detected","payload":{"toolName":tool,"hookPhase":phase,"secretTypes":secrets,"location":"input","severity":"critical"}}));
    }
    if let Some(path) = input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(Value::as_str)
    {
        if sensitive_path(path) {
            events.push(json!({"eventType":"policy_violation","payload":{"toolName":tool,"hookPhase":phase,"filePath":path.chars().take(200).collect::<String>(),"reason":"sensitive_file_access","severity":"warning"}}));
        }
    }
    let normalized = tool.to_ascii_lowercase();
    if normalized == "bash" || normalized == "terminal" {
        if let Some(command) = input.get("command").and_then(Value::as_str) {
            let lower = command.to_ascii_lowercase();
            let patterns: Vec<&str> = [
                "git\\s+push\\s+.*--force",
                "git\\s+reset\\s+--hard",
                "rm\\s+-rf?\\s",
                "drop\\s+(table|database)",
                "delete\\s+from\\s+\\w+\\s*(;|$)",
            ]
            .iter()
            .filter(|p| regex::Regex::new(p).is_ok_and(|r| r.is_match(&lower)))
            .copied()
            .collect();
            if !patterns.is_empty() {
                events.push(json!({"eventType":"approval_requested","payload":{"toolName":tool,"hookPhase":phase,"matchedPatterns":patterns,"severity":"high"}}));
            }
        }
    }
    for mut event in events {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        event["id"] = json!(format!("gov-{now}"));
        event["sessionId"] = session.map_or(Value::Null, |s| json!(s));
        event["resolvedAt"] = Value::Null;
        event["resolution"] = Value::Null;
        let _ = writeln!(std::io::stderr(), "[governance] {}", event);
    }
}

#[cfg(test)]
mod tests {
    use super::{matches_secret, sensitive_path};

    #[test]
    fn detects_secret_categories_without_emitting_secret_values() {
        assert_eq!(
            matches_secret(r#"{"token":"ghp_abcdefghijklmnopqrstuvwxyz0123456789AB"}"#),
            vec!["github_token"]
        );
        assert!(matches_secret("ordinary text").is_empty());
    }

    #[test]
    fn detects_sensitive_paths_without_flagging_regular_docs() {
        assert!(sensitive_path("/home/user/.env.production"));
        assert!(sensitive_path("/tmp/server.pem"));
        assert!(!sensitive_path("/repo/README.md"));
    }
}
