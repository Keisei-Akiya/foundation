use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ecc-config-protection-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create isolated test directory");
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn event(tool: &str, input_field: &str, path: &Path) -> String {
    format!(
        r#"{{"hook_event_name":"pre_tool_call","tool_name":"{tool}","tool_input":{{"{input_field}":"{}"}}}}"#,
        path.display()
    )
}

fn run_rust(payload: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ecc-hermes-hooks"));
    command.arg("config-protection");
    run_command(&mut command, payload)
}

fn run_command(command: &mut Command, payload: &str) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start hook process");
    child
        .stdin
        .take()
        .expect("hook stdin")
        .write_all(payload.as_bytes())
        .expect("write hook event");
    child.wait_with_output().expect("collect hook output")
}

fn assert_allows(output: &Output) {
    assert!(
        output.status.success(),
        "hook exited with {:?}",
        output.status
    );
    assert!(
        output.stdout.is_empty(),
        "an allowed event must not emit a block response, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

fn assert_blocks(output: &Output) {
    assert!(
        output.status.success(),
        "hook exited with {:?}",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("block response must be valid JSON");
    assert_eq!(
        response["decision"], "block",
        "unexpected response: {stdout}"
    );
    let reason = response["reason"]
        .as_str()
        .expect("block response must include a string reason");
    assert!(
        reason.contains("linter/formatter"),
        "missing policy explanation: {reason}"
    );
    assert!(
        stdout.ends_with('\n'),
        "block response must be newline-terminated: {stdout:?}"
    );
}

#[test]
fn blocks_edit_of_an_existing_linter_config() {
    let dir = TestDir::new();
    let config = dir.path("eslint.config.js");
    fs::write(&config, "export default {};\n").expect("create protected config");

    let output = run_rust(&event("write_file", "path", &config));

    assert_blocks(&output);
}

#[test]
fn blocks_hermes_edit_file_for_an_existing_protected_config() {
    let dir = TestDir::new();
    let config = dir.path(".prettierrc.json");
    fs::write(&config, "{}\n").expect("create protected config");

    let output = run_rust(&event("edit_file", "file_path", &config));

    assert_blocks(&output);
}

#[test]
fn allows_creation_of_a_protected_config_that_does_not_exist_yet() {
    let dir = TestDir::new();
    let config = dir.path("biome.json");

    let output = run_rust(&event("write_file", "path", &config));

    assert_allows(&output);
}

#[test]
fn allows_editing_an_existing_unrelated_file() {
    let dir = TestDir::new();
    let file = dir.path("src/main.rs");
    fs::create_dir_all(file.parent().unwrap()).expect("create source directory");
    fs::write(&file, "fn main() {}\n").expect("create unrelated file");

    let output = run_rust(&event("patch", "file_path", &file));

    assert_allows(&output);
}

#[test]
fn does_not_block_a_non_edit_tool_targeting_a_protected_config() {
    let dir = TestDir::new();
    let config = dir.path("ruff.toml");
    fs::write(&config, "line-length = 88\n").expect("create protected config");

    let output = run_rust(&event("read_file", "path", &config));

    assert_allows(&output);
}

#[test]
fn fails_open_for_malformed_and_empty_input() {
    assert_allows(&run_rust("{invalid json"));
    assert_allows(&run_rust(""));
}

#[test]
fn matches_protected_config_names_without_case_sensitivity() {
    let dir = TestDir::new();
    let config = dir.path("ESLINT.CONFIG.JS");
    fs::write(&config, "export default {};\n").expect("create protected config");

    let output = run_rust(&event("apply_patch", "path", &config));

    assert_blocks(&output);
}
