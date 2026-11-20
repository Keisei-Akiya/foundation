use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("foundation-update-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_version(home: &Path, version: &str) {
    fs::create_dir_all(home.join("foundation")).unwrap();
    fs::write(home.join("foundation/VERSION"), format!("{version}\n")).unwrap();
}

fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(root: &Path, path: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                walk(root, &path, files);
            } else {
                files.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut files = Vec::new();
    if dir.exists() {
        walk(dir, dir, &mut files);
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn serve(status: &str, body: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let status = status.to_owned();
    let body = body.to_owned();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") {
            if stream.read_exact(&mut byte).is_err() {
                return;
            }
            request.push(byte[0]);
        }
        write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    format!("http://{address}")
}

fn run(home: &Path, url: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_foundation"))
        .args(["update", "--check", "--hermes-home"])
        .arg(home)
        .env("HOME", home)
        .env_remove("HERMES_HOME")
        .env("FOUNDATION_RELEASE_API_URL", url)
        .output()
        .unwrap()
}

#[test]
fn reports_update_available_and_does_not_change_hermes_home() {
    let temp = TestDir::new();
    let home = temp.0.join("home");
    write_version(&home, "0.0.20");
    let before = snapshot(&home);
    let url = serve("200 OK", r#"{"tag_name":"v0.0.24"}"#);
    let output = run(&home, &url);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Latest version: 0.0.24"));
    assert!(stdout.contains("Binary version: 0.0.23"));
    assert!(stdout.contains("Shared tree version: 0.0.20"));
    assert!(stdout.contains("Update available"));
    assert_eq!(before, snapshot(&home));
}

#[test]
fn reports_up_to_date_and_warns_on_binary_shared_tree_mismatch() {
    let temp = TestDir::new();
    let home = temp.0.join("home");
    write_version(&home, "0.0.23");
    let url = serve("200 OK", r#"{"tag_name":"v0.0.23"}"#);
    let output = run(&home, &url);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Already up to date"));
    assert!(output.stderr.is_empty());

    let url = serve("200 OK", r#"{"tag_name":"v0.0.22"}"#);
    let output = run(&home, &url);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Already up to date"));

    write_version(&home, "0.0.22");
    let url = serve("200 OK", r#"{"tag_name":"v0.0.23"}"#);
    let output = run(&home, &url);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("Warning: binary version 0.0.23 does not match shared tree version 0.0.22"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("Update available"));
}

#[test]
fn rejects_prerelease_tags_with_a_cause_and_nonzero_exit() {
    let temp = TestDir::new();
    let home = temp.0.join("home");
    write_version(&home, "0.0.23");
    let output = run(&home, &serve("200 OK", r#"{"tag_name":"1.2.3-rc.1"}"#));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid version: 1.2.3-rc.1"));
}

#[test]
fn reports_api_and_payload_errors_without_network_access() {
    let temp = TestDir::new();
    let home = temp.0.join("home");
    write_version(&home, "0.0.23");
    for (status, body, expected) in [
        ("404 Not Found", "not found", "HTTP 404"),
        ("500 Internal Server Error", "error", "HTTP 500"),
        ("200 OK", "not json", "invalid release API JSON"),
        ("200 OK", "{}", "tag_name"),
    ] {
        let output = run(&home, &serve(status, body));
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "expected {expected:?}, got {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let output = run(&home, &format!("http://{address}"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("release API request failed"));
}
