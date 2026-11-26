use serde_json::{json, Value};
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
fn serve(response: &str) -> (String, thread::JoinHandle<Value>) {
    let response = response.to_owned();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut first, _) = listener.accept().unwrap();
        let _domain_payload = request(&mut first);
        let domain_response = r#"{"answers":{"domain:engineering":{"probabilities":{"0":1.0,"1":0.0},"legend":{"0":"yes","1":"no"}},"domain:economics":{"probabilities":{"0":1.0,"1":0.0},"legend":{"0":"yes","1":"no"}}}}"#;
        write!(first,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",domain_response.len(),domain_response).unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        let body = request(&mut stream);
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
        body
    });
    (url, handle)
}
fn serve_custom_stages(
    domain_response: &str,
    page_response: &str,
) -> (String, thread::JoinHandle<Vec<Value>>) {
    let domain_response = domain_response.to_owned();
    let page_response = page_response.to_owned();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut payloads = Vec::new();
        for response in [&domain_response, &page_response] {
            let (mut stream, _) = listener.accept().unwrap();
            payloads.push(request(&mut stream));
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        }
        payloads
    });
    (url, handle)
}
fn serve_stage1(response: &str) -> (String, thread::JoinHandle<Value>) {
    let response = response.to_owned();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let payload = request(&mut stream);
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        payload
    });
    (url, handle)
}
fn serve_search_stages() -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut payloads = Vec::new();
        for stage in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let payload = request(&mut stream);
            let questions: Vec<(String, Value)> = match payload["questions"].as_array() {
                Some(items) => items
                    .iter()
                    .map(|q| (q["name"].as_str().unwrap().to_owned(), q.clone()))
                    .collect(),
                None => payload["questions"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            };
            let answers = questions.into_iter().map(|(name, _)| {
                let answer = if stage == 0 {
                    let yes = if name.contains("engineering") || name.contains("economics") { 0.8 } else { 0.2 };
                    serde_json::json!({"probabilities":{"0":yes,"1":1.0-yes},"legend":{"0":"yes","1":"no"}})
                } else {
                    serde_json::json!({"probabilities":{"0":0.1,"1":0.2,"2":0.7},"legend":{"0":"none","1":"low","2":"high"}})
                };
                (name, answer)
            }).collect::<serde_json::Map<_, _>>();
            let response = serde_json::json!({"answers":answers}).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            payloads.push(payload);
        }
        payloads
    });
    (url, handle)
}
fn serve_batches(expected: usize) -> (String, thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut payloads = Vec::new();
        for stage in 0..=expected {
            let (mut stream, _) = listener.accept().unwrap();
            let payload = request(&mut stream);
            let answers: serde_json::Map<String, Value> = if stage == 0 {
                let names: Vec<String> = match payload["questions"].as_array() {
                    Some(items) => items
                        .iter()
                        .map(|q| q["name"].as_str().unwrap().to_owned())
                        .collect(),
                    None => payload["questions"]
                        .as_object()
                        .unwrap()
                        .keys()
                        .cloned()
                        .collect(),
                };
                names.into_iter().map(|name| (name, serde_json::json!({"probabilities":{"0":1.0,"1":0.0},"legend":{"0":"yes","1":"no"}}))).collect()
            } else {
                payload["questions"].as_object().unwrap().iter().map(|(name, _)| (
                    name.clone(), serde_json::json!({"probabilities":{"0":0.1,"1":0.2,"2":0.7},"legend":{"0":"none","1":"low","2":"high"}})
                )).collect()
            };
            let response = serde_json::json!({"answers":answers}).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            if stage > 0 {
                payloads.push(payload);
            }
        }
        payloads
    });
    (url, handle)
}
fn wiki() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "foundation-wiki-search-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("SCHEMA.md"), "# Wiki Schema\n\n## Domain\n\n| Domain | 範囲 | スコープ外（当面） |\n|--------|------|-------------------|\n| **engineering** | Software and systems | Cooking only |\n| **economics** | Markets and policy | Personal logs |\n").unwrap();
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
        .env("FOUNDATION_DECIDE_PROVIDER", "typesafe")
        .env("TYPESAFE_BASE_URL", endpoint)
        .env("HOME", root.join(".test-home"))
        .env("CODEX_HOME", root.join(".test-home/.codex"))
        .env_remove("OPENAI_API_KEY_DECISIONS")
        .env_remove("OPENAI_BASE_URL")
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
fn run_openai(root: &std::path::Path, endpoint: &str) -> (i32, String, String) {
    let home = root.join(".test-home");
    std::fs::create_dir_all(&home).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_foundation"));
    cmd.arg("wiki-search")
        .arg("--wiki")
        .arg(root)
        .arg("--log-path")
        .arg(root.join("openai-usage.jsonl"))
        .arg("How do pages relate?")
        .env("HOME", &home)
        .env("FOUNDATION_DECIDE_PROVIDER", "openai")
        .env("OPENAI_BASE_URL", endpoint)
        .env("OPENAI_API_KEY_DECISIONS", "fake")
        .env_remove("CODEX_HOME")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("TYPESAFE_BASE_URL")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, _) in std::env::vars().filter(|(key, _)| key.starts_with("OPENAI_")) {
        if key != "OPENAI_BASE_URL" && key != "OPENAI_API_KEY_DECISIONS" {
            cmd.env_remove(key);
        }
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into(),
        String::from_utf8_lossy(&out.stderr).into(),
    )
}

const ANSWERS: &str = r#"{"answers":{"page:one.md":{"probabilities":{"0":0.1,"1":0.2,"2":0.7},"legend":{"0":"none","1":"low","2":"high"}},"page:two.md":{"probabilities":{"0":0.5,"1":0.2,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}},"page:three.md":{"probabilities":{"0":0.1,"1":0.3,"2":0.2},"legend":{"0":"none","1":"low","2":"high"}}}}"#;
#[test]
fn stage1_domain_questions_ignore_pipe_characters_outside_domain_table() {
    let root = wiki();
    std::fs::write(
        root.join("SCHEMA.md"),
        "# Wiki Schema\n\n## Domain\n\n| Domain | 範囲 | スコープ外（当面） |\n|--------|------|-------------------|\n| **engineering** | Software and systems | Cooking only |\n| **economics** | Markets and policy | Personal logs |\n| **management** | Organizations and leadership | Personal logs |\n\n## Frontmatter examples\n\n```yaml\ntype: entity | concept | comparison | query | summary\ndomains: [engineering]   # engineering | economics | management | media\n# media: novel | manga | anime | game\n```\n",
    )
    .unwrap();
    let (endpoint, server) = serve_search_stages();
    let (code, _out, err) = run(&root, &endpoint, &[]);
    let payloads = server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let names: Vec<String> = match payloads[0]["questions"].as_array() {
        Some(items) => items
            .iter()
            .map(|question| question["name"].as_str().unwrap().to_owned())
            .collect(),
        None => payloads[0]["questions"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect(),
    };
    let actual: std::collections::BTreeSet<_> = names.into_iter().collect();
    let expected = [
        "domain:engineering",
        "domain:economics",
        "domain:management",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(actual, expected);
}

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
fn pages_are_scored_once_in_batches_of_at_most_fifty() {
    let root = std::env::temp_dir().join(format!("foundation-wiki-batch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("SCHEMA.md"), "## Domain\n| Domain | 範囲 | スコープ外 |\n|---|---|---|\n| **engineering** | Software and systems | Cooking |\n| **economics** | Markets and policy | Personal logs |\n").unwrap();
    let mut index = String::new();
    for i in 0..120 {
        index.push_str(&format!("- [[page-{i}]] — summary {i}\n"));
        std::fs::write(
            root.join(format!("page-{i}.md")),
            format!("---\ntitle: Title {i}\n---\nBody {i}\n"),
        )
        .unwrap();
    }
    std::fs::write(root.join("index.md"), index).unwrap();
    let (endpoint, server) = serve_batches(3);
    let (code, out, err) = run(&root, &endpoint, &["--max-results", "120"]);
    let payloads = server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(payloads.len(), 3, "one request per batch");
    let names: Vec<_> = payloads
        .iter()
        .flat_map(|p| {
            let questions = p["questions"].as_object().unwrap();
            assert!(questions.len() <= 50);
            questions.keys().cloned().collect::<Vec<_>>()
        })
        .collect();
    let unique: std::collections::HashSet<_> = names.iter().collect();
    assert_eq!(names.len(), 120);
    assert_eq!(unique.len(), 120, "duplicate page questions");
    for i in 0..120 {
        assert!(
            unique.contains(&format!("page:page-{i}.md")),
            "missing page {i}"
        );
    }
    assert_eq!(out.lines().count(), 120);
    let _ = std::fs::remove_dir_all(root);
}
fn assert_bad_response(response: &str, message: &str) {
    let root = wiki();
    let (endpoint, server) = serve(response);
    let (code, out, err) = run(&root, &endpoint, &[]);
    server.join().unwrap();
    assert_ne!(code, 0);
    assert!(err.contains(message), "expected {message:?} in {err:?}");
    assert!(out.is_empty(), "unexpected stdout: {out:?}");
}
#[test]
fn empty_probabilities_dictionary_fails_clearly() {
    assert_bad_response(
        r#"{"answers":{"page:one.md":{"probabilities":{},"legend":{"0":"none","1":"low","2":"high"}}}}"#,
        "probabilities must be a nonempty dictionary",
    );
}
#[test]
fn probability_above_one_fails_clearly() {
    assert_bad_response(
        r#"{"answers":{"page:one.md":{"probabilities":{"0":-0.1,"1":0.2,"2":1.5},"legend":{"0":"none","1":"low","2":"high"}}}}"#,
        "probabilities must be between 0 and 1",
    );
}
#[test]
fn negative_probability_fails_clearly() {
    assert_bad_response(
        r#"{"answers":{"page:one.md":{"probabilities":{"0":-0.1,"1":0.2,"2":0.8},"legend":{"0":"none","1":"low","2":"high"}}}}"#,
        "probabilities must be between 0 and 1",
    );
}
#[test]
fn domain_stage_filters_pages_and_keeps_unassigned_pages() {
    // Domain scoring is an independent Yes/No request before page scoring.
    let root = wiki();
    std::fs::write(root.join("SCHEMA.md"), "## Domain\n| **engineering** | Software and systems | Not cooking |\n| **economics** | Markets and policy | Personal logs |\n").unwrap();
    let mut index = std::fs::read_to_string(root.join("index.md")).unwrap();
    index.push_str("- [[four]] — summary four\n");
    std::fs::write(root.join("index.md"), index).unwrap();
    std::fs::write(
        root.join("four.md"),
        "---\ntitle: four\ndomains: [engineering, economics]\n---\nsecret page body\n",
    )
    .unwrap();
    for (name, domain) in [
        ("one", "engineering"),
        ("two", "economics"),
        ("three", ""),
        ("four", "engineering, economics"),
    ] {
        let front = if domain.is_empty() {
            String::new()
        } else {
            format!("domains: [{domain}]\n")
        };
        std::fs::write(
            root.join(format!("{name}.md")),
            format!("---\ntitle: {}\n{front}---\nsecret page body\n", name),
        )
        .unwrap();
    }
    let (endpoint, server) = serve_search_stages();
    let (code, out, err) = run(
        &root,
        &endpoint,
        &["--domain-threshold", "0.5", "--domain-max", "2"],
    );
    let payloads = server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("one.md")
            && out.contains("two.md")
            && out.contains("three.md")
            && out.contains("four.md")
    );
    let domain_payload = &payloads[0];
    let text = domain_payload.to_string();
    assert!(text.contains("engineering") && text.contains("Software and systems"));
    assert!(text.contains("economics") && text.contains("Markets and policy"));
    assert!(!text.contains("secret page body"));
    let page_payload = &payloads[1];
    assert_eq!(page_payload["questions"].as_object().unwrap().len(), 4);
    assert_eq!(page_payload["state"], "How do pages relate?");
}

#[test]
fn domain_threshold_fallback_keeps_only_the_top_domain_and_unassigned_pages() {
    let root = wiki();
    std::fs::write(
        root.join("one.md"),
        "---\ntitle: one\ndomains: [engineering]\n---\n",
    )
    .unwrap();
    std::fs::write(
        root.join("two.md"),
        "---\ntitle: two\ndomains: [economics]\n---\n",
    )
    .unwrap();
    std::fs::write(root.join("three.md"), "---\ntitle: three\n---\n").unwrap();
    let domains = r#"{"answers":{"domain:engineering":{"probabilities":{"0":0.8,"1":0.2},"legend":{"0":"yes","1":"no"}},"domain:economics":{"probabilities":{"0":0.7,"1":0.3},"legend":{"0":"yes","1":"no"}}}}"#;
    let pages = r#"{"answers":{"page:one.md":{"probabilities":{"0":0.05,"1":0.05,"2":0.9},"legend":{"0":"none","1":"low","2":"high"}},"page:three.md":{"probabilities":{"0":0.8,"1":0.1,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}}}}"#;
    let (endpoint, server) = serve_custom_stages(domains, pages);
    let (code, out, err) = run(&root, &endpoint, &["--domain-threshold", "0.9"]);
    let payloads = server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("one.md") && out.contains("three.md"));
    assert!(!out.contains("two.md"));
    let sent: Vec<_> = payloads[1]["questions"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert!(sent.contains(&"page:one.md") && sent.contains(&"page:three.md"));
    assert!(!sent.contains(&"page:two.md"));
}

#[test]
fn domain_max_and_page_max_results_are_independent_and_domain_args_validate() {
    let root = wiki();
    std::fs::write(
        root.join("one.md"),
        "---\ntitle: one\ndomains: [engineering]\n---\n",
    )
    .unwrap();
    std::fs::write(
        root.join("two.md"),
        "---\ntitle: two\ndomains: [economics]\n---\n",
    )
    .unwrap();
    std::fs::write(
        root.join("three.md"),
        "---\ntitle: three\ndomains: [engineering]\n---\n",
    )
    .unwrap();
    let domains = r#"{"answers":{"domain:engineering":{"probabilities":{"0":0.9,"1":0.1},"legend":{"0":"yes","1":"no"}},"domain:economics":{"probabilities":{"0":0.8,"1":0.2},"legend":{"0":"yes","1":"no"}}}}"#;
    let pages = r#"{"answers":{"page:one.md":{"probabilities":{"0":0.05,"1":0.05,"2":0.9},"legend":{"0":"none","1":"low","2":"high"}},"page:two.md":{"probabilities":{"0":0.8,"1":0.1,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}},"page:three.md":{"probabilities":{"0":0.7,"1":0.2,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}}}}"#;
    let (endpoint, server) = serve_custom_stages(domains, pages);
    let (code, out, err) = run(
        &root,
        &endpoint,
        &[
            "--domain-max",
            "1",
            "--threshold",
            "0",
            "--max-results",
            "2",
        ],
    );
    let payloads = server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.lines().count(), 2);
    assert_eq!(payloads[1]["questions"].as_object().unwrap().len(), 2);
    for (args, msg) in [
        (vec!["--domain-threshold", "1.5"], "--domain-threshold"),
        (vec!["--domain-max", "0"], "--domain-max"),
    ] {
        let (code, out, err) = run(&root, "http://127.0.0.1:1", &args);
        assert_ne!(code, 0);
        assert!(err.contains(msg), "{err}");
        assert!(out.is_empty());
    }
}

#[test]
fn malformed_domain_stage1_responses_fail_without_stdout() {
    for response in [
        r#"{"answers":{"domain:engineering":{"probabilities":{"0":0.8,"1":0.2},"legend":{"0":"yes","1":"no"}}}}"#,
        r#"{"answers":{"domain:engineering":{"legend":{"0":"yes"}}}}"#,
        r#"{"answers":{"domain:engineering":{"probabilities":{"x":0.8},"legend":{"0":"yes"}}}}"#,
        r#"{"answers":{"domain:engineering":{"probabilities":{"0":1.5},"legend":{"0":"yes"}}}}"#,
        r#"{"answers":{"domain:engineering":{"probabilities":{"0":-0.1},"legend":{"0":"yes"}}}}"#,
        r#"{"answers":{"domain:engineering":{"probabilities":{"0":0.8},"legend":{"0":"affirmative"}}}}"#,
        "not JSON",
    ] {
        let root = wiki();
        let (endpoint, server) = serve_stage1(response);
        let (code, out, err) = run(&root, &endpoint, &[]);
        server.join().unwrap();
        assert_ne!(code, 0, "response: {response}");
        assert!(
            err.contains("invalid Jev response"),
            "response: {response}, stderr: {err:?}"
        );
        assert!(out.is_empty(), "response: {response}, stdout: {out:?}");
    }
}

#[test]
fn wiki_search_logs_unscored_filtered_pages_as_null() {
    let root = wiki();
    let log = root.join("nested/filtered-search.jsonl");
    for (name, domain) in [("one", "engineering"), ("two", "economics"), ("three", "")] {
        let front = if domain.is_empty() {
            String::new()
        } else {
            format!("domains: [{domain}]\n")
        };
        std::fs::write(
            root.join(format!("{name}.md")),
            format!("---\ntitle: {name}\n{front}---\n"),
        )
        .unwrap();
    }
    let domains = r#"{"answers":{"domain:engineering":{"probabilities":{"0":0.8,"1":0.2},"legend":{"0":"yes","1":"no"}},"domain:economics":{"probabilities":{"0":0.1,"1":0.9},"legend":{"0":"yes","1":"no"}}}}"#;
    let pages = r#"{"answers":{"page:one.md":{"probabilities":{"0":0.1,"1":0.2,"2":0.7},"legend":{"0":"none","1":"low","2":"high"}},"page:three.md":{"probabilities":{"0":0.5,"1":0.2,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}}}}"#;
    let (endpoint, server) = serve_custom_stages(domains, pages);
    let (code, out, err) = run(
        &root,
        &endpoint,
        &[
            "--domain-threshold",
            "0.5",
            "--log-path",
            log.to_str().unwrap(),
        ],
    );
    let payloads = server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    let sent: Vec<_> = payloads[1]["questions"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert!(sent.contains(&"page:one.md") && sent.contains(&"page:three.md"));
    assert!(!sent.contains(&"page:two.md"));

    let lines = std::fs::read_to_string(&log).unwrap();
    let record: Value = serde_json::from_str(lines.trim()).unwrap();
    assert_eq!(record["domain_probabilities"]["engineering"], 0.8);
    assert_eq!(record["domain_probabilities"]["economics"], 0.1);
    assert_eq!(record["page_scores"]["two.md"], Value::Null);
    assert!(record["page_scores"]["one.md"].is_number());
    assert!(record["page_scores"]["three.md"].is_number());
    assert!(!out.contains("two.md"), "filtered page returned: {out}");
}

#[test]
fn wiki_search_appends_jsonl_with_complete_scores_usage_and_handles_failures() {
    let root = wiki();
    let log = root.join("nested/search.jsonl");
    let domains = r#"{"answers":{"domain:engineering":{"probabilities":{"0":0.8,"1":0.2},"legend":{"0":"yes","1":"no"}},"domain:economics":{"probabilities":{"0":0.1,"1":0.9},"legend":{"0":"yes","1":"no"}}},"usage":{"input_tokens":3,"output_tokens":2}}"#;
    let pages = r#"{"answers":{"page:one.md":{"probabilities":{"0":0.1,"1":0.2,"2":0.7},"legend":{"0":"none","1":"low","2":"high"}},"page:two.md":{"probabilities":{"0":0.1,"1":0.2,"2":0.7},"legend":{"0":"none","1":"low","2":"high"}},"page:three.md":{"probabilities":{"0":0.5,"1":0.2,"2":0.1},"legend":{"0":"none","1":"low","2":"high"}}},"usage":{"input_tokens":7,"output_tokens":4}}"#;
    let (endpoint, server) = serve_custom_stages(domains, pages);
    let (code, out, err) = run(
        &root,
        &endpoint,
        &[
            "--domain-threshold",
            "0.5",
            "--log-path",
            log.to_str().unwrap(),
        ],
    );
    server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(!out.is_empty());
    let lines = std::fs::read_to_string(&log).unwrap();
    let record: Value = serde_json::from_str(lines.trim()).unwrap();
    assert_eq!(record["input"], "How do pages relate?");
    assert!(record["timestamp"].as_str().unwrap().ends_with('Z'));
    assert_eq!(record["domain_probabilities"]["engineering"], 0.8);
    assert_eq!(record["domain_probabilities"]["economics"], 0.1);
    assert!((record["page_scores"]["one.md"].as_f64().unwrap() - 0.9).abs() < 1e-9);
    assert!((record["page_scores"]["two.md"].as_f64().unwrap() - 0.9).abs() < 1e-9);
    assert!((record["page_scores"]["three.md"].as_f64().unwrap() - 0.3).abs() < 1e-9);
    assert_eq!(record["usage"]["input_tokens"], 10);
    assert_eq!(record["usage"]["output_tokens"], 6);
    assert!(!lines.contains("secret body"));

    let (endpoint, server) = serve_custom_stages(domains, pages);
    let (code, _, err) = run(
        &root,
        &endpoint,
        &[
            "--domain-threshold",
            "0.5",
            "--log-path",
            log.to_str().unwrap(),
        ],
    );
    server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 2);

    let directory = root.join("is-a-directory");
    std::fs::create_dir(&directory).unwrap();
    let (endpoint, server) = serve(ANSWERS);
    let (code, out, err) = run(
        &root,
        &endpoint,
        &["--log-path", directory.to_str().unwrap()],
    );
    server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert!(!out.is_empty());
    assert!(err.contains("warning"), "{err}");
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

#[test]
fn openai_responses_are_normalized_through_both_wiki_search_stages() {
    let root = wiki();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut payloads = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let payload = request(&mut stream);
            let items = payload["questions"].as_array().unwrap();
            let answers: Vec<Value> = items.iter().map(|q| {
                let name = q["name"].as_str().unwrap();
                let levels = q["levels"].as_array().unwrap();
                let probabilities: Vec<Value> = levels.iter().enumerate().map(|(i,l)| json!({"value":i,"label":l["label"],"probability":if i==2 {0.7} else if i==1 {0.2} else {0.1}})).collect();
                json!({"name":name,"type":"score","probabilities":probabilities})
            }).collect();
            let response = json!({"answers":answers,"usage":{"input_tokens":12,"output_tokens":4,"total_tokens":16}}).to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
            payloads.push(payload);
        }
        payloads
    });
    let (code, out, err) = run_openai(&root, &endpoint);
    let payloads = server.join().unwrap();
    assert_eq!(code, 0, "{err}");
    assert_eq!(payloads.len(), 2);
    assert_eq!(out.lines().count(), 3);
    let line = std::fs::read_to_string(root.join("openai-usage.jsonl")).unwrap();
    let record: Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(record["usage"]["input_tokens"], 24);
    assert_eq!(record["usage"]["output_tokens"], 8);
    assert_eq!(record["usage"]["total_tokens"], 32);
}
