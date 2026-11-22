//! CLI テストで共有するヘルパー（`decide_cli` と `intake_hint_cli` が使う）。
//! 各テストバイナリは使う関数が違うので、未使用の警告は出さない。
#![allow(dead_code)]
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

pub const TIMEOUT: Duration = Duration::from_secs(8);

/// テストごとに専用の HOME を作る。
/// テストは並列に走るので、同じ `name` でも呼び出しごとに別のディレクトリを返す。
pub fn home(prefix: &str, name: &str) -> std::path::PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!(
        "foundation-{prefix}-{name}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(p.join(".hermes")).unwrap();
    p
}

/// `foundation <subcommand> <args>` を実行する。
/// 返り値は (終了コード, stdout, stderr)。
pub fn run(
    subcommand: &str,
    args: &[&str],
    stdin: &str,
    envs: &[(&str, &str)],
    h: &std::path::Path,
) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_foundation"));
    c.arg(subcommand)
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

pub fn accept(l: &TcpListener) -> TcpStream {
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

/// モックサーバが受けたリクエストを読む。
/// 返り値は (ヘッダ, JSON 本文)。
pub fn request(s: &mut TcpStream) -> (String, Value) {
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

pub fn reply(s: &mut TcpStream, status: &str, body: &str) {
    write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
}
