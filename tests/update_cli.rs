use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("foundation-update-{}-{n}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn write_version(home: &Path, v: &str) {
    fs::create_dir_all(home.join("foundation")).unwrap();
    fs::write(home.join("foundation/VERSION"), format!("{v}\n")).unwrap();
}
fn snapshot(p: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(r: &Path, p: &Path, v: &mut Vec<(PathBuf, Vec<u8>)>) {
        for e in fs::read_dir(p).unwrap() {
            let e = e.unwrap();
            let p = e.path();
            if e.file_type().unwrap().is_dir() {
                walk(r, &p, v)
            } else {
                v.push((p.strip_prefix(r).unwrap().into(), fs::read(p).unwrap()))
            }
        }
    }
    let mut v = vec![];
    if p.exists() {
        walk(p, p, &mut v)
    }
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}
fn serve(responses: Vec<(&'static str, String)>) -> (String, thread::JoinHandle<Vec<String>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let t = thread::spawn(move || {
        let mut paths = vec![];
        for (status, body) in responses {
            let (mut s, _) = l.accept().unwrap();
            s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut req = vec![];
            let mut b = [0];
            while !req.ends_with(b"\r\n\r\n") {
                if s.read_exact(&mut b).is_err() {
                    break;
                }
                req.push(b[0])
            }
            paths.push(
                String::from_utf8_lossy(&req)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string(),
            );
            write!(
                s,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap()
        }
        paths
    });
    (format!("http://{addr}"), t)
}
fn api(s: &'static str, b: &str) -> (&'static str, String) {
    (s, b.into())
}
fn sh(_p: &Path, s: &'static str, code: i32) -> (&'static str, String) {
    (
        s,
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$FOUNDATION_TEST_RECORD\"\nexit {code}\n"),
    )
}
fn run(home: &Path, url: &str, record: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_foundation"))
        .arg("update")
        .args(args)
        .arg("--hermes-home")
        .arg(home)
        .env("HOME", home)
        .env_remove("HERMES_HOME")
        .env("FOUNDATION_RELEASE_API_URL", format!("{url}/api"))
        .env("FOUNDATION_INSTALLER_BASE_URL", format!("{url}/raw"))
        .env("FOUNDATION_TEST_RECORD", record)
        .output()
        .unwrap()
}
#[test]
fn updates_latest_via_tag_script_and_passes_home() {
    let t = Temp::new();
    let h = t.0.join("home");
    let r = t.0.join("args");
    write_version(&h, "0.0.20");
    fs::write(h.join("config.yaml"), "config sentinel").unwrap();
    fs::write(h.join("allowlist"), "allowlist sentinel").unwrap();
    let before = snapshot(&h);
    let (u, s) = serve(vec![
        api("200 OK", r#"{"tag_name":"v0.0.24"}"#),
        sh(&r, "200 OK", 0),
    ]);
    let o = run(&h, &u, &r, &[]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("gateway の再起動が必要"));
    assert_eq!(
        fs::read_to_string(&r).unwrap(),
        format!("--version\n0.0.24\n--hermes-home\n{}\n", h.display())
    );
    assert_eq!(
        s.join().unwrap(),
        ["GET /api HTTP/1.1", "GET /raw/v0.0.24/install.sh HTTP/1.1"]
    );
    assert_eq!(before, snapshot(&h));
}
#[test]
fn explicit_version_uses_requested_tag() {
    let t = Temp::new();
    let h = t.0.join("home");
    let r = t.0.join("args");
    write_version(&h, "0.0.20");
    let (u, s) = serve(vec![sh(&r, "200 OK", 0)]);
    let o = run(&h, &u, &r, &["--version", "1.2.3"]);
    assert!(o.status.success());
    assert_eq!(s.join().unwrap(), ["GET /raw/v1.2.3/install.sh HTTP/1.1"]);
    assert_eq!(
        fs::read_to_string(r).unwrap(),
        format!("--version\n1.2.3\n--hermes-home\n{}\n", h.display())
    );
}
#[test]
fn matching_latest_skips_install_and_mismatch_reinstalls() {
    let t = Temp::new();
    let h = t.0.join("home");
    let r = t.0.join("args");
    write_version(&h, env!("CARGO_PKG_VERSION"));
    let (u, s) = serve(vec![api("200 OK", r#"{"tag_name":"v0.0.23"}"#)]);
    let o = run(&h, &u, &r, &[]);
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("最新です"));
    assert!(!r.exists());
    assert_eq!(s.join().unwrap(), ["GET /api HTTP/1.1"]);
    write_version(&h, "0.0.22");
    let (u, s) = serve(vec![
        api("200 OK", r#"{"tag_name":"v0.0.23"}"#),
        sh(&r, "200 OK", 0),
    ]);
    let o = run(&h, &u, &r, &[]);
    assert!(o.status.success());
    assert!(r.exists());
    assert_eq!(
        s.join().unwrap(),
        ["GET /api HTTP/1.1", "GET /raw/v0.0.23/install.sh HTTP/1.1"]
    );
}
#[test]
fn missing_release_fails_without_touching_home() {
    let t = Temp::new();
    let h = t.0.join("home");
    let r = t.0.join("args");
    write_version(&h, "0.0.20");
    fs::write(h.join("config.yaml"), "untouched").unwrap();
    fs::write(h.join("allowlist"), "untouched").unwrap();
    let before = snapshot(&h);
    let (u, s) = serve(vec![("404 Not Found", "not found".into())]);
    let o = run(&h, &u, &r, &["--version", "9.9.9"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("HTTP 404"));
    assert_eq!(before, snapshot(&h));
    assert!(!r.exists());
    assert_eq!(s.join().unwrap(), ["GET /raw/v9.9.9/install.sh HTTP/1.1"]);
}
#[test]
fn api_and_installer_failures_and_bad_version_are_reported() {
    let t = Temp::new();
    let h = t.0.join("home");
    let r = t.0.join("args");
    for (status, body, cause) in [
        ("500 Internal Server Error", "failed", "HTTP 500"),
        ("200 OK", "not json", "invalid release API JSON"),
    ] {
        let (u, s) = serve(vec![api(status, body)]);
        let o = run(&h, &u, &r, &[]);
        assert!(!o.status.success());
        assert!(String::from_utf8_lossy(&o.stderr).contains(cause));
        s.join().unwrap();
    }
    let (u, s) = serve(vec![sh(&r, "200 OK", 7)]);
    let o = run(&h, &u, &r, &["--version", "1.2.3"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("installer exited with status"));
    s.join().unwrap();
    let (u, s) = serve(vec![]);
    let o = run(&h, &u, &r, &["--version", "1.2"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("invalid version: 1.2"));
    assert!(s.join().unwrap().is_empty());
}
