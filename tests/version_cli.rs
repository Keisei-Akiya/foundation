use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("foundation-version-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str], home: &std::path::Path, hermes_home: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_foundation"))
        .arg("version")
        .args(args)
        .env("HOME", home)
        .env("HERMES_HOME", hermes_home)
        .output()
        .unwrap()
}

fn write_version(home: &std::path::Path, version: &str) {
    fs::create_dir_all(home.join("foundation")).unwrap();
    fs::write(home.join("foundation/VERSION"), format!("{version}\n")).unwrap();
}

#[test]
fn enable_is_rejected_as_an_unknown_subcommand() {
    let output = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .arg("enable")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("usage: foundation <"));
    assert!(!stderr.contains("foundation enable"));
}

#[test]
fn prints_binary_and_shared_tree_versions_without_warning_when_equal() {
    let temp = TestDir::new();
    let home = temp.0.join("home");
    write_version(&home, env!("CARGO_PKG_VERSION"));

    let output = run(&[], &temp.0, &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(&format!("Binary version: {}", env!("CARGO_PKG_VERSION"))));
    assert!(stdout.contains(&format!(
        "Shared tree version: {}",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(output.stderr.is_empty());
}

#[test]
fn warns_when_binary_and_shared_tree_versions_differ() {
    let temp = TestDir::new();
    let home = temp.0.join("home");
    write_version(&home, "9.8.7-test");

    let output = run(&[], &temp.0, &home);

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!(
        "Warning: binary version {} does not match shared tree version 9.8.7-test",
        env!("CARGO_PKG_VERSION")
    )));
}

#[test]
fn explicit_home_precedes_environment_and_missing_version_is_reported() {
    let temp = TestDir::new();
    let explicit = temp.0.join("explicit");
    let environment = temp.0.join("environment");
    write_version(&environment, "wrong-env-version");

    let output = run(
        &["--hermes-home", explicit.to_str().unwrap()],
        &temp.0,
        &environment,
    );

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Shared tree version: unavailable (VERSION file not found)"));
    assert!(output.stderr.is_empty());
}

#[test]
fn environment_home_is_used_and_default_home_is_used_without_override() {
    let temp = TestDir::new();
    let environment = temp.0.join("environment");
    write_version(&environment, "from-env");
    let output = run(&[], &temp.0, &environment);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Shared tree version: from-env"));

    let default_home = temp.0.join(".hermes");
    write_version(&default_home, "from-default");
    let output = Command::new(env!("CARGO_BIN_EXE_foundation"))
        .arg("version")
        .env("HOME", &temp.0)
        .env_remove("HERMES_HOME")
        .output()
        .unwrap();
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Shared tree version: from-default"));
}
