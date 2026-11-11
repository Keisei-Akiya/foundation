use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

const SPEC: &str = r#"{
  "input": "I was charged twice for my order.",
  "questions": [
    {"name": "duplicate", "type": "predicate", "instructions": "Is the customer charged twice?"},
    {"name": "dept", "type": "choice", "instructions": "Route the ticket.",
     "choices": [{"value": "billing", "description": "Payment issues"},
                 {"value": "tech", "description": "Technical issues"}]},
    {"name": "severity", "type": "score", "instructions": "How severe?",
     "levels": [{"label": "low"}, {"label": "high"}]}
  ]
}"#;

fn run(args: &[&str], stdin: &str, envs: &[(&str, &str)]) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_foundation"));
    cmd.arg("decide")
        .args(args)
        .env_remove("OPENAI_API_KEY")
        .env_remove("AZURE_ENDPOINT")
        .env_remove("AZURE_API_KEY")
        .env_remove("AZURE_ENTRA_TOKEN")
        .env_remove("DEPLOYMENT_NAME")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("OPENROUTER_API_KEY")
        .env_remove("HERMES_HOME")
        .env(
            "HOME",
            std::env::temp_dir().join(format!("foundation-isolated-home-{}", std::process::id())),
        )
        .env(
            "HERMES_HOME",
            std::env::temp_dir().join(format!("foundation-isolated-hermes-{}", std::process::id())),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into(),
        String::from_utf8_lossy(&out.stderr).into(),
    )
}

#[test]
fn openai_dry_run_builds_decisions_request() {
    let (code, out, _) = run(&["--provider", "openai", "--dry-run"], SPEC, &[]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["url"], "https://api.openai.com/v1/decisions");
    assert_eq!(v["body"]["model"], "gpt-6-luna");
    assert_eq!(v["body"]["questions"].as_array().unwrap().len(), 3);
}

#[test]
fn microsoft_dry_run_maps_question_types() {
    let (code, out, _) = run(
        &["--provider", "microsoft", "--dry-run"],
        SPEC,
        &[
            ("AZURE_ENDPOINT", "https://x.example/"),
            ("DEPLOYMENT_NAME", "dep"),
        ],
    );
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["url"],
        "https://x.example/providers/microsoft/v1/systemone"
    );
    assert_eq!(v["body"]["model"], "dep");
    assert_eq!(v["body"]["questions"]["duplicate"]["type"], "noul");
    assert_eq!(
        v["body"]["questions"]["dept"]["criteria"]["billing"],
        "Payment issues"
    );
}

#[test]
fn dry_run_never_prints_credentials() {
    let (_, out, err) = run(
        &["--provider", "openai", "--dry-run"],
        SPEC,
        &[("OPENAI_API_KEY", "sk-secret-value")],
    );
    assert!(!out.contains("sk-secret-value") && !err.contains("sk-secret-value"));
}

#[test]
fn reads_openai_credentials_and_connection_from_hermes_env_file() {
    let dir = std::env::temp_dir().join(format!("foundation-dotenv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(".env"),
        "OPENAI_API_KEY='dotenv-test-key' # test comment\nOPENAI_BASE_URL=\"https://dotenv.example\"\n",
    )
    .unwrap();
    let (code, out, err) = run_with_home(&["--provider", "openai", "--dry-run"], SPEC, &[], &dir);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("https://dotenv.example/v1/decisions"));
    assert!(!out.contains("dotenv-test-key") && !err.contains("dotenv-test-key"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn dotenv_supports_home_fallback_and_hermes_profile_selection() {
    let root = std::env::temp_dir().join(format!("foundation-home-dotenv-{}", std::process::id()));
    let home = root.join("home");
    let profile = root.join("profile");
    std::fs::create_dir_all(home.join(".hermes")).unwrap();
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(
        home.join(".hermes/.env"),
        "OPENAI_API_KEY=home-key\nOPENAI_BASE_URL=https://home.example\n",
    )
    .unwrap();
    std::fs::write(
        profile.join(".env"),
        "OPENAI_API_KEY=profile-key\nOPENAI_BASE_URL=https://profile.example\n",
    )
    .unwrap();
    let (code, out, err) = run_with_dotenv(
        &["--provider", "openai", "--dry-run"],
        SPEC,
        &[],
        &home,
        None,
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("https://home.example/v1/decisions"));
    let (code, out, err) = run_with_dotenv(
        &["--provider", "openai", "--dry-run"],
        SPEC,
        &[],
        &home,
        Some(&profile),
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("https://profile.example/v1/decisions"));
    assert!(!out.contains("profile-key") && !err.contains("profile-key"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn process_environment_overrides_dotenv_and_empty_or_missing_files_are_safe() {
    let root = std::env::temp_dir().join(format!("foundation-empty-dotenv-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join(".env"),
        "OPENAI_API_KEY=\nOPENAI_BASE_URL=https://file.example\n",
    )
    .unwrap();
    let (code, out, err) = run_with_home(
        &["--provider", "openai", "--dry-run"],
        SPEC,
        &[
            ("OPENAI_API_KEY", "process-key"),
            ("OPENAI_BASE_URL", "https://process.example"),
        ],
        &root,
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("https://process.example/v1/decisions"));
    assert!(!out.contains("process-key") && !err.contains("process-key"));
    std::fs::remove_file(root.join(".env")).unwrap();
    let (code, _, err) = run_with_home(&["--provider", "openai"], SPEC, &[], &root);
    assert_eq!(code, 2);
    assert!(err.contains("OPENAI_API_KEY"));
    std::fs::write(root.join(".env"), "").unwrap();
    let (code, _, err) = run_with_home(&["--provider", "openai"], SPEC, &[], &root);
    assert_eq!(code, 2);
    assert!(err.contains("OPENAI_API_KEY"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn openrouter_key_does_not_satisfy_missing_openai_api_key() {
    let dir = std::env::temp_dir().join(format!("foundation-no-alias-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".env"), "OPENROUTER_API_KEY=router-file-key\n").unwrap();

    let (code, _, err) = run_with_home(&["--provider", "openai"], SPEC, &[], &dir);
    assert_eq!(code, 2);
    assert!(err.contains("OPENAI_API_KEY is not set"));
    assert!(!err.contains("router-file-key"));

    let (code, _, err) = run_with_home(
        &["--provider", "openai"],
        SPEC,
        &[("OPENROUTER_API_KEY", "router-process-key")],
        &dir,
    );
    assert_eq!(code, 2);
    assert!(err.contains("OPENAI_API_KEY is not set"));
    assert!(!err.contains("router-process-key"));
    let _ = std::fs::remove_dir_all(dir);
}

fn run_with_home(
    args: &[&str],
    stdin: &str,
    envs: &[(&str, &str)],
    home: &std::path::Path,
) -> (i32, String, String) {
    run_with_dotenv(args, stdin, envs, home, Some(home))
}

fn run_with_dotenv(
    args: &[&str],
    stdin: &str,
    envs: &[(&str, &str)],
    home: &std::path::Path,
    hermes_home: Option<&std::path::Path>,
) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_foundation"));
    cmd.arg("decide")
        .args(args)
        .env_remove("OPENAI_API_KEY")
        .env_remove("AZURE_ENDPOINT")
        .env_remove("AZURE_API_KEY")
        .env_remove("AZURE_ENTRA_TOKEN")
        .env_remove("DEPLOYMENT_NAME")
        .env_remove("OPENAI_BASE_URL")
        .env_remove("OPENROUTER_API_KEY")
        .env("HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(path) = hermes_home {
        cmd.env("HERMES_HOME", path);
    } else {
        cmd.env_remove("HERMES_HOME");
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into(),
        String::from_utf8_lossy(&out.stderr).into(),
    )
}

#[test]
fn missing_key_is_usage_error() {
    let (code, _, err) = run(&["--provider", "openai"], SPEC, &[]);
    assert_eq!(code, 2);
    assert!(err.contains("OPENAI_API_KEY"));
}

#[test]
fn invalid_spec_is_rejected() {
    let (code, _, err) = run(
        &["--provider", "openai", "--dry-run"],
        r#"{"input":"x","questions":[{"name":"a","type":"choice","instructions":"q","choices":[]}]}"#,
        &[],
    );
    assert_eq!(code, 2);
    assert!(err.contains("choices"));
}

#[test]
fn openai_rejects_duplicate_choice_values_and_scalar_json() {
    let duplicate = r#"{"input":"x","questions":[{"name":"q","type":"choice","instructions":"pick","choices":[{"value":"same","description":"a"},{"value":"same","description":"b"}]}]}"#;
    let (code, _, err) = run(&["--provider", "openai", "--dry-run"], duplicate, &[]);
    assert_eq!(code, 2);
    assert!(err.contains("duplicate choice value"));

    let (code, _, err) = run(
        &["--provider", "openai", "--dry-run"],
        r#"{"input":42,"questions":[{"name":"q","type":"predicate","instructions":"true?"}]}"#,
        &[],
    );
    assert_eq!(code, 2);
    assert!(err.contains("object or array"));
}

#[test]
fn openai_enforces_documented_question_and_score_level_limits() {
    let questions: Vec<Value> = (0..401)
        .map(|i| serde_json::json!({"name":format!("q{i}"),"type":"predicate","instructions":"true?"}))
        .collect();
    let spec = serde_json::json!({"input":"x","questions":questions}).to_string();
    let (code, _, err) = run(&["--provider", "openai", "--dry-run"], &spec, &[]);
    assert_eq!(code, 2);
    assert!(err.contains("400 questions"));

    let levels: Vec<Value> = (0..11)
        .map(|i| serde_json::json!({"label":format!("level{i}")}))
        .collect();
    let spec = serde_json::json!({"input":"x","questions":[{"name":"q","type":"score","instructions":"rate","levels":levels}]}).to_string();
    let (code, _, err) = run(&["--provider", "openai", "--dry-run"], &spec, &[]);
    assert_eq!(code, 2);
    assert!(err.contains("2..=10"));
}

#[test]
fn endpoint_validation_blocks_insecure_or_credential_bearing_urls() {
    for url in [
        "http://api.example.com",
        "https://user:secret@api.example.com",
        "https://api.example.com/?token=secret",
    ] {
        let (code, out, err) = run(
            &["--provider", "openai", "--dry-run"],
            SPEC,
            &[("OPENAI_BASE_URL", url)],
        );
        assert_eq!(code, 2, "url={url}");
        assert!(!out.contains("secret") && !err.contains("secret"));
    }
}

#[test]
fn microsoft_mapping_preserves_score_level_descriptions() {
    let spec = r#"{"input":"x","questions":[{"name":"q","type":"score","instructions":"rate","levels":[{"label":"low","description":"minor impact"},{"label":"high","description":"service unavailable"}]}]}"#;
    let (code, out, _) = run(
        &["--provider", "microsoft", "--dry-run"],
        spec,
        &[
            ("AZURE_ENDPOINT", "https://x.example"),
            ("DEPLOYMENT_NAME", "dep"),
        ],
    );
    assert_eq!(code, 0);
    let body: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        body["body"]["questions"]["q"]["criteria"][0],
        "low: minor impact"
    );
    assert_eq!(
        body["body"]["questions"]["q"]["criteria"][1],
        "high: service unavailable"
    );
}

#[test]
fn sends_post_to_loopback_mock_with_auth_and_preserves_refusal_response() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        let headers = String::from_utf8_lossy(&request).to_ascii_lowercase();
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length: "))
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let mut body = vec![0; content_length];
        stream.read_exact(&mut body).unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["model"], "gpt-6-luna");
        assert_eq!(body["questions"][0]["name"], "duplicate");
        let response = r#"{"answers":[{"name":"duplicate","type":"refusal"}]}"#;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        headers
    });

    let key = "test-secret-not-for-output";
    let (code, out, err) = run(
        &["--provider", "openai"],
        SPEC,
        &[
            ("OPENAI_BASE_URL", &format!("http://{addr}")),
            ("OPENAI_API_KEY", key),
        ],
    );
    let headers = server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(headers
        .lines()
        .any(|line| line.starts_with("authorization: ")));
    assert!(out.contains("\"type\": \"refusal\""));
    assert!(!out.contains(key) && !err.contains(key));
}

#[test]
fn microsoft_dry_run_without_endpoint_uses_safe_placeholder() {
    let (code, out, err) = run(
        &["--provider", "microsoft", "--dry-run"],
        SPEC,
        &[("DEPLOYMENT_NAME", "dep")],
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("<AZURE_ENDPOINT>"));
}

#[test]
fn does_not_follow_redirects_with_request_body_or_auth() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    let destination = TcpListener::bind("127.0.0.1:0").unwrap();
    destination.set_nonblocking(true).unwrap();
    let destination_addr = destination.local_addr().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut byte = [0u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        write!(stream, "HTTP/1.1 302 Found\r\nLocation: http://{destination_addr}/steal\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let (code, _, err) = run(
        &["--provider", "openai"],
        SPEC,
        &[
            ("OPENAI_BASE_URL", &format!("http://{addr}")),
            ("OPENAI_API_KEY", "test-secret"),
        ],
    );
    server.join().unwrap();
    assert_eq!(code, 1);
    assert!(err.contains("HTTP 302"));
    assert!(destination.accept().is_err());
}
