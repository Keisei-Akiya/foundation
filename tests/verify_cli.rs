use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

fn invoke(name: &str, payload: &Value) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .arg(name)
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
    String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
}

#[test]
fn stop_hookify_blocks_matching_stop_rule() {
    let root = std::env::temp_dir().join(format!("verify-hookify-{}", std::process::id()));
    let rules = root.join(".claude");
    std::fs::create_dir_all(&rules).unwrap();
    std::fs::write(
        rules.join("hookify.stop.local.md"),
        "---\nevent: stop\naction: block\npattern: TODO\n---\nResolve TODOs first.\n",
    )
    .unwrap();
    let out = invoke(
        "stop-hookify",
        &serde_json::json!({"cwd":root,"final_response":"TODO remains"}),
    );
    assert_eq!(
        serde_json::from_str::<Value>(out.trim()).unwrap()["action"],
        "continue"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn check_console_log_reports_changed_typescript_path() {
    let root = std::env::temp_dir().join(format!("verify-console-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("app.ts"), "console.log('debug');").unwrap();
    let out = invoke(
        "check-console-log",
        &serde_json::json!({"cwd":root,"changed_paths":["app.ts"]}),
    );
    assert!(
        serde_json::from_str::<Value>(out.trim()).unwrap()["message"]
            .as_str()
            .unwrap()
            .contains("app.ts")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn format_typecheck_is_noop_without_project_tools() {
    let root = std::env::temp_dir().join(format!("verify-format-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("app.ts"), "const x=1").unwrap();
    let out = invoke(
        "format-typecheck",
        &serde_json::json!({"cwd":root,"changed_paths":["app.ts"]}),
    );
    assert!(out.is_empty());
    let _ = std::fs::remove_dir_all(root);
}
