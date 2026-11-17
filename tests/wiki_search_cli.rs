use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

fn request(stream: &mut TcpStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(8)))
        .unwrap();
    let mut bytes = Vec::new();
    let mut b = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut b).unwrap();
        bytes.push(b[0]);
    }
    let headers = String::from_utf8_lossy(&bytes);
    let len: usize = headers
        .lines()
        .find_map(|l| {
            l.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_owned)
        })
        .unwrap()
        .parse()
        .unwrap();
    let mut body = vec![0; len];
    stream.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}
fn serve(response: &'static str) -> (String, thread::JoinHandle<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let end = std::time::Instant::now() + Duration::from_secs(8);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(pair) => break pair,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < end =>
                {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(e) => panic!("mock server accept failed: {e}"),
            }
        };
        let body = request(&mut stream);
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
        body
    });
    (url, handle)
}
fn wiki() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("foundation-wiki-search-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("index.md"),
        "- [[one]] — summary one\n- [[two]] — summary two\n- [[three]] — summary three\n",
    )
    .unwrap();
    for (name, title, body) in [
        ("one", "First page", "secret body alpha"),
        ("two", "Second page", "secret body beta"),
        ("three", "Third page", "secret body gamma"),
    ] {
        std::fs::write(
            root.join(format!("{name}.md")),
            format!("---\ntitle: {title}\n---\n# {title}\n{body}\n"),
        )
        .unwrap();
    }
    root
}
fn run(root: &std::path::Path, endpoint: &str, extra: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_foundation"));
    cmd.arg("wiki-search")
        .arg("--wiki")
        .arg(root)
        .args(extra)
        .arg("How do pages relate?")
        .env("TYPESAFE_BASE_URL", endpoint)
        .env("TYPESAFE_API_KEY", "mock-key")
        .env_remove("HERMES_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into(),
        String::from_utf8_lossy(&out.stderr).into(),
    )
}
const ANSWERS: &str = r#"{"answers":{"page:one.md":{"probabilities":{"0":0.1,"1":0.2,"2":0.7},"legend":{"0":"none","1":"low","2":"high"}},"page:two.md":{"probabilities":{"0":0.5,"1":0.2,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}},"page:three.md":{"probabilities":{"0":0.1,"1":0.3,"2":0.2},"legend":{"0":"none","1":"low","2":"high"}}}}"#;
#[test]
fn cli_sends_only_page_metadata_and_returns_sorted_threshold_matches() {
    let root = wiki();
    let (endpoint, server) = serve(ANSWERS);
    let (code, out, err) = run(&root, &endpoint, &[]);
    let payload = server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "0.90\tone.md\tFirst page\n0.50\tthree.md\tThird page\n0.30\ttwo.md\tSecond page\n"
    );
    assert_eq!(payload["state"], "How do pages relate?");
    let serialized = payload.to_string();
    assert!(serialized.contains("summary one") && serialized.contains("summary two"));
    assert!(!serialized.contains("secret body"));
}
#[test]
fn falls_back_to_top_one_and_honors_threshold_and_max_results() {
    let root = wiki();
    let low = r#"{"answers":{"page:one.md":{"probabilities":{"0":0.8,"1":0.1,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}},"page:two.md":{"probabilities":{"0":0.8,"1":0.1,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}},"page:three.md":{"probabilities":{"0":0.8,"1":0.1,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}}}}"#;
    let (endpoint, server) = serve(low);
    let (code, out, err) = run(&root, &endpoint, &["--threshold", "0.5"]);
    server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.lines().count(), 1);
    let (endpoint, server) = serve(ANSWERS);
    let (code, out, err) = run(
        &root,
        &endpoint,
        &["--threshold", "0.2", "--max-results", "1"],
    );
    server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.lines().count(), 1);
    assert!(out.starts_with("0.90\t"));
}
#[test]
fn malformed_probabilities_fail_clearly() {
    let root = wiki();
    let response = r#"{"answers":{"page:one.md":{"probabilities":{"x":0.1},"legend":{"0":"none","1":"low","2":"high"}}}}"#;
    let (endpoint, server) = serve(response);
    let (code, _, err) = run(&root, &endpoint, &[]);
    server.join().unwrap();
    assert_eq!(code, 1);
    assert!(err.contains("invalid Jev response"), "{err}");
}
