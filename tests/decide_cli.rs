use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
const SPEC: &str = r#"{"input":"I was charged twice for my order.","questions":[{"name":"duplicate","type":"predicate","instructions":"Is customer charged twice?"},{"name":"dept","type":"choice","instructions":"Route ticket.","choices":[{"value":"billing","description":"Payment issues"},{"value":"tech","description":"Technical issues"}]},{"name":"severity","type":"score","instructions":"How severe?","levels":[{"label":"low"},{"label":"high"}]}]}"#;
const TIMEOUT: Duration = Duration::from_secs(8);
fn home(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("foundation-decide-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(p.join(".hermes")).unwrap();
    p
}
fn run(
    args: &[&str],
    stdin: &str,
    envs: &[(&str, &str)],
    h: &std::path::Path,
) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_foundation"));
    c.arg("decide")
        .args(args)
        .env("HOME", h)
        .env_remove("HERMES_HOME")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("TYPESAFE_BASE_URL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        c.env(k, v);
    }
    let mut ch = c.spawn().unwrap();
    if let Err(e) = ch.stdin.take().unwrap().write_all(stdin.as_bytes()) {
        assert_eq!(
            e.kind(),
            std::io::ErrorKind::BrokenPipe,
            "stdin write failed: {e}"
        );
    }
    let end = Instant::now() + TIMEOUT;
    loop {
        if ch.try_wait().unwrap().is_some() {
            let o = ch.wait_with_output().unwrap();
            return (
                o.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&o.stdout).into(),
                String::from_utf8_lossy(&o.stderr).into(),
            );
        }
        if Instant::now() >= end {
            let _ = ch.kill();
            let _ = ch.wait();
            panic!("CLI timed out after {TIMEOUT:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn accept(l: &TcpListener) -> TcpStream {
    l.set_nonblocking(true).unwrap();
    let end = Instant::now() + TIMEOUT;
    loop {
        match l.accept() {
            Ok((s, _)) => return s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < end, "mock server timed out accepting");
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("accept failed: {e}"),
        }
    }
}
fn request(s: &mut TcpStream) -> (String, Value) {
    s.set_read_timeout(Some(TIMEOUT)).unwrap();
    let mut b = Vec::new();
    let mut x = [0];
    while !b.ends_with(b"\r\n\r\n") {
        s.read_exact(&mut x).unwrap();
        b.push(x[0]);
    }
    let h = String::from_utf8_lossy(&b).to_string();
    let n: usize = h
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_string)
        })
        .unwrap()
        .parse()
        .unwrap();
    let mut body = vec![0; n];
    s.read_exact(&mut body).unwrap();
    (h, serde_json::from_slice(&body).unwrap())
}
fn reply(s: &mut TcpStream, status: &str, body: &str) {
    write!(s,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
}
#[test]
fn defaults_typesafe_and_jev_shapes() {
    let h = home("default");
    let (c, o, e) = run(&["--dry-run"], SPEC, &[], &h);
    assert_eq!(c, 0, "{e}");
    let v: Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v["method"], "POST");
    assert_eq!(v["url"], "https://api.typesafe.ai/v1/systemone");
    assert_eq!(v["body"]["model"], "jev-latest");
    assert_eq!(v["body"]["questions"]["duplicate"]["type"], "noul");
    assert_eq!(
        v["body"]["questions"]["dept"]["criteria"]["billing"],
        "Payment issues"
    );
    assert_eq!(
        v["body"]["questions"]["severity"]["criteria"],
        serde_json::json!(["low", "high"])
    );
}
#[test]
fn provider_and_model_are_restricted_to_jev() {
    let h = home("provider");
    for p in ["openai", "microsoft", "other"] {
        let (c, _, e) = run(
            &["--provider", p, "--model", "jev-latest", "--dry-run"],
            SPEC,
            &[],
            &h,
        );
        assert_eq!(c, 2);
        assert!(e.contains("unknown provider"));
    }
    let (c, _, e) = run(
        &[
            "--provider",
            "typesafe",
            "--model",
            "jev-preview",
            "--dry-run",
        ],
        SPEC,
        &[],
        &h,
    );
    assert_eq!(c, 2);
    assert!(e.contains("--model"));
}
#[test]
fn hermes_env_key_is_used_by_live_local_mock() {
    let h = home("dotenv");
    let key = "local-mock-secret-key";
    std::fs::write(
        h.join(".hermes/.env"),
        format!("TYPESAFE_API_KEY='{key}' # comment\n"),
    )
    .unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut s = accept(&l);
        let (headers, b) = request(&mut s);
        assert!(headers
            .to_lowercase()
            .contains(&format!("authorization: bearer {key}")));
        assert_eq!(b["model"], "jev-latest");
        reply(
            &mut s,
            "200 OK",
            r#"{"answers":{"duplicate":{"type":"noul","noul":0.9}}}"#,
        );
    });
    let (c, o, e) = run(&[], SPEC, &[("TYPESAFE_BASE_URL", &base)], &h);
    server.join().unwrap();
    assert_eq!(c, 0, "{e}");
    assert!(o.contains("0.9"));
    assert!(!o.contains(key) && !e.contains(key));
}
#[test]
fn redirects_are_not_followed_or_sent_authentication() {
    let h = home("redirect");
    let destination = TcpListener::bind("127.0.0.1:0").unwrap();
    destination.set_nonblocking(true).unwrap();
    let dest_url = format!("http://{}", destination.local_addr().unwrap());
    let source = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", source.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut stream = accept(&source);
        let (headers, _) = request(&mut stream);
        assert!(headers
            .to_ascii_lowercase()
            .contains("authorization: bearer "));
        write!(
            stream,
            "HTTP/1.1 302 Found\r\nLocation: {dest_url}/v1/systemone\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
    });
    let (code, out, err) = run(
        &[],
        SPEC,
        &[
            ("TYPESAFE_API_KEY", "redirect-secret"),
            ("TYPESAFE_BASE_URL", &base),
        ],
        &h,
    );
    server.join().unwrap();
    assert_eq!(code, 1, "{err}");
    assert!(out.is_empty());
    assert!(!err.contains("redirect-secret"));
    assert!(matches!(
        destination.accept(),
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
    ));
}

#[test]
fn dry_run_hides_secret_and_allows_loopback() {
    let h = home("dry");
    let key = "do-not-print-this-key";
    let (c, o, e) = run(
        &["--dry-run"],
        SPEC,
        &[
            ("TYPESAFE_API_KEY", key),
            ("TYPESAFE_BASE_URL", "http://127.0.0.1:43210"),
        ],
        &h,
    );
    assert_eq!(c, 0, "{e}");
    assert!(o.contains("http://127.0.0.1:43210/v1/systemone"));
    assert!(!o.contains(key) && !e.contains(key));
}
#[test]
fn endpoint_validation_blocks_insecure_or_credential_bearing_urls() {
    let h = home("validation");
    let (c, _, e) = run(&[], SPEC, &[], &h);
    assert_eq!(c, 2);
    assert!(e.contains("TYPESAFE_API_KEY"));
    for url in [
        "http://example.com",
        "https://user:private@api.typesafe.ai",
        "https://api.typesafe.ai:8443",
        "https://api.typesafe.ai.evil",
        "https://api.typesafe.ai/?token=private",
        "https://api.typesafe.ai/#fragment",
    ] {
        let (c, o, e) = run(
            &["--dry-run"],
            SPEC,
            &[
                ("TYPESAFE_BASE_URL", url),
                ("TYPESAFE_API_KEY", "safe-test-key"),
            ],
            &h,
        );
        assert_eq!(c, 2, "url={url}");
        assert!(!o.contains("private") && !e.contains("private"));
        assert!(!o.contains("safe-test-key") && !e.contains("safe-test-key"));
    }
}
#[test]
fn invalid_spec_rejected() {
    let h = home("invalid");
    let (c, _, e) = run(
        &["--dry-run"],
        r#"{"input":"x","questions":[{"name":"q","type":"choice","instructions":"pick","choices":[]}] }"#,
        &[],
        &h,
    );
    assert_eq!(c, 2);
    assert!(e.contains("choices"));
}
#[test]
fn remote_error_does_not_echo_key() {
    let h = home("error");
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let key = "mock-private-token";
    let server = thread::spawn(move || {
        let mut s = accept(&l);
        let _ = request(&mut s);
        reply(
            &mut s,
            "401 Unauthorized",
            &format!(r#"{{"detail":"bad {key}"}}"#),
        );
    });
    let (c, o, e) = run(
        &[],
        SPEC,
        &[("TYPESAFE_BASE_URL", &base), ("TYPESAFE_API_KEY", key)],
        &h,
    );
    server.join().unwrap();
    assert_eq!(c, 1);
    assert!(o.is_empty());
    assert!(e.contains("HTTP 401"));
    assert!(!e.contains(key));
}
