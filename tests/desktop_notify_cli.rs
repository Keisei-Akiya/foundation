use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn invoke(payload: &str, enabled: Option<&str>, threshold: Option<&str>, webhook: Option<&str>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_foundation"));
    command
        .arg("desktop-notify")
        .env("HERMES_HOME", std::env::temp_dir());
    for (key, value) in [
        ("FOUNDATION_DESKTOP_NOTIFY", enabled),
        ("FOUNDATION_DESKTOP_NOTIFY_THRESHOLD_SECONDS", threshold),
        ("FOUNDATION_DESKTOP_NOTIFY_WEBHOOK", webhook),
    ] {
        command.env_remove(key);
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

fn read_request(mut stream: TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut byte = [0u8; 1];
    while !request.windows(4).any(|part| part == b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        request.push(byte[0]);
    }
    let header_end = request.len();
    let content_length = String::from_utf8_lossy(&request)
        .lines()
        .find_map(|line| {
            line.split_once(':').and_then(|(name, length)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| length.trim().parse::<usize>().ok())
                    .flatten()
            })
        })
        .unwrap_or(0);
    request.resize(header_end + content_length, 0);
    stream.read_exact(&mut request[header_end..]).unwrap();
    let response = b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    let _ = stream.write_all(response);
    request
}

#[test]
fn desktop_notification_obeys_opt_in_threshold_and_sends_only_constant_content_to_loopback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let receiver = thread::spawn(move || loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let mut requests = vec![read_request(stream)];
                let deadline = Instant::now() + Duration::from_millis(150);
                while Instant::now() < deadline {
                    match listener.accept() {
                        Ok((stream, _)) => requests.push(read_request(stream)),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(error) => panic!("loopback listener failed: {error}"),
                    }
                }
                break requests;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(error) => panic!("loopback listener failed: {error}"),
        }
    });
    let url = format!("http://{address}/webhook/");
    let body = r#"{"elapsed_seconds":60,"assistant_response":"secret-token","command":"private command","path":"/private/path"}"#;
    invoke(body, None, Some("30"), Some(&url));
    invoke(body, Some("1"), Some("61"), Some(&url));
    invoke(body, Some("true"), Some("30"), Some(&url));
    let requests = receiver.join().unwrap();
    assert_eq!(
        requests.len(),
        1,
        "disabled and short turns must not notify"
    );
    let text = String::from_utf8_lossy(&requests[0]);
    assert!(text.contains("Long task completed (60s)."));
    assert!(!text.contains("secret-token"));
    assert!(!text.contains("private command"));
    assert!(!text.contains("/private/path"));
}

#[test]
fn desktop_notification_requires_valid_json_duration_and_does_not_follow_external_http() {
    invoke(
        r#"{"elapsed_seconds":5}"#,
        Some("1"),
        Some("0"),
        Some("http://example.invalid/"),
    );
    invoke(
        r#"{}"#,
        Some("1"),
        Some("0"),
        Some("https://example.invalid/"),
    );
}

#[test]
fn desktop_notification_fails_open_when_loopback_webhook_is_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    invoke(
        r#"{"elapsed_seconds":60}"#,
        Some("1"),
        Some("30"),
        Some(&format!("http://{address}/webhook/")),
    );
}
