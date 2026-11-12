use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};
fn invoke(cmd: &str, payload: &str, home: &std::path::Path) -> String {
    let mut c = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .env("HERMES_HOME", home)
        .args([cmd])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    String::from_utf8(c.wait_with_output().unwrap().stdout).unwrap()
}
#[test]
fn mcp_failure_is_saved_and_blocks_same_server_until_ttl() {
    let h = std::env::temp_dir().join(format!("foundation-mcp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&h);
    let post = r#"{"hook_event_name":"post_tool_call","tool_name":"mcp__serverA__fetch","status":"error","error_type":"tool_error"}"#;
    assert!(invoke("mcp-health", post, &h).is_empty());
    let pre = r#"{"hook_event_name":"pre_tool_call","tool_name":"mcp__serverA__fetch"}"#;
    assert_eq!(
        serde_json::from_str::<Value>(&invoke("mcp-health", pre, &h)).unwrap()["action"],
        "block"
    );
    let other = r#"{"hook_event_name":"pre_tool_call","tool_name":"mcp__serverB__fetch"}"#;
    assert!(invoke("mcp-health", other, &h).is_empty());
    let _ = std::fs::remove_dir_all(h);
}
#[test]
fn gateguard_blocks_first_edit_per_session_and_path_only() {
    let h = std::env::temp_dir().join(format!("foundation-gg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&h);
    let p = r#"{"hook_event_name":"pre_tool_call","tool_name":"write_file","session_id":"s1","tool_input":{"path":"/tmp/a"}}"#;
    assert_eq!(
        serde_json::from_str::<Value>(&invoke("gateguard", p, &h)).unwrap()["action"],
        "block"
    );
    assert!(invoke("gateguard", p, &h).is_empty());
    let other = r#"{"hook_event_name":"pre_tool_call","tool_name":"write_file","session_id":"s2","tool_input":{"path":"/tmp/a"}}"#;
    assert_eq!(
        serde_json::from_str::<Value>(&invoke("gateguard", other, &h)).unwrap()["action"],
        "block"
    );
    let _ = std::fs::remove_dir_all(h);
}
