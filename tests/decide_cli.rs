mod common;

use common::{accept, reply, request};
use serde_json::Value;
use std::{io::Write, net::TcpListener, thread};

const SPEC: &str = r#"{"input":"I was charged twice for my order.","questions":[{"name":"duplicate","type":"predicate","instructions":"Is customer charged twice?"},{"name":"dept","type":"choice","instructions":"Route ticket.","choices":[{"value":"billing","description":"Payment issues"},{"value":"tech","description":"Technical issues"}]},{"name":"severity","type":"score","instructions":"How severe?","levels":[{"label":"low"},{"label":"high"}]}]}"#;

fn home(name: &str) -> std::path::PathBuf {
    common::home("decide", name)
}

fn run(
    args: &[&str],
    stdin: &str,
    envs: &[(&str, &str)],
    h: &std::path::Path,
) -> (i32, String, String) {
    common::run("decide", args, stdin, envs, h)
}

#[test]
fn defaults_typesafe_and_jev_shapes() {
    let h = home("default");
    let (c, o, e) = run(&["--provider", "typesafe", "--dry-run"], SPEC, &[], &h);
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
    let (c, _, e) = run(&["--provider", "other"], SPEC, &[], &h);
    assert_eq!(c, 2);
    assert!(e.contains("unknown provider"));
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
    let (c, o, e) = run(
        &["--provider", "typesafe"],
        SPEC,
        &[("TYPESAFE_BASE_URL", &base)],
        &h,
    );
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
        &["--provider", "typesafe"],
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
        &["--provider", "typesafe", "--dry-run"],
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
    let (c, _, e) = run(&["--provider", "typesafe"], SPEC, &[], &h);
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
            &["--provider", "typesafe", "--dry-run"],
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
        &["--provider", "typesafe"],
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

#[test]
fn provider_resolution_uses_env_then_openai_codex_default() {
    let h = home("provider-default");
    let codex = h.join(".codex");
    std::fs::create_dir_all(&codex).unwrap();
    std::fs::write(
        codex.join("auth.json"),
        r#"{"tokens":{"access_token":"fake-default-token"}}"#,
    )
    .unwrap();
    let (c, o, e) = run(&["--dry-run"], SPEC, &[], &h);
    assert_eq!(c, 0, "{e}");
    let v: Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v["url"], "https://api.openai.com/v1/decisions");
    assert_eq!(v["body"]["model"], "gpt-6-luna");
    assert!(!o.contains("fake-default-token") && !e.contains("fake-default-token"));

    let (c, o, e) = run(
        &["--dry-run"],
        SPEC,
        &[("FOUNDATION_DECIDE_PROVIDER", "typesafe")],
        &home("provider-env-typesafe"),
    );
    assert_eq!(c, 0, "{e}");
    let v: Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v["url"], "https://api.typesafe.ai/v1/systemone");
    assert_eq!(v["body"]["model"], "jev-latest");

    let (c, o, e) = run(
        &["--dry-run"],
        SPEC,
        &[
            ("FOUNDATION_DECIDE_PROVIDER", "microsoft"),
            ("MS_DECISION_ENDPOINT", "https://res.services.ai.azure.com"),
        ],
        &home("provider-env-microsoft"),
    );
    assert_eq!(c, 0, "{e}");
    let v: Value = serde_json::from_str(&o).unwrap();
    assert_eq!(
        v["url"],
        "https://res.services.ai.azure.com/providers/microsoft/v1/systemone"
    );
}

#[test]
fn provider_flag_overrides_environment_and_env_is_validated() {
    let h = home("provider-priority");
    let (c, o, e) = run(
        &["--provider", "openai", "--dry-run"],
        SPEC,
        &[("FOUNDATION_DECIDE_PROVIDER", "typesafe")],
        &h,
    );
    assert_eq!(c, 0, "{e}");
    let v: Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v["url"], "https://api.openai.com/v1/decisions");

    let (c, _, e) = run(
        &["--dry-run"],
        SPEC,
        &[("FOUNDATION_DECIDE_PROVIDER", "invalid")],
        &h,
    );
    assert_eq!(c, 2);
    assert!(e.contains("FOUNDATION_DECIDE_PROVIDER"));

    let (c, o, e) = run(
        &["--dry-run"],
        SPEC,
        &[("FOUNDATION_DECIDE_PROVIDER", "  ")],
        &h,
    );
    assert_eq!(c, 0, "{e}");
    let v: Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v["url"], "https://api.openai.com/v1/decisions");

    let (c, _, e) = run(&["--model", "jev-latest", "--dry-run"], SPEC, &[], &h);
    assert_eq!(c, 2);
    assert!(e.contains("--model supports only gpt-6-luna"));
}

const MS_ARGS: &[&str] = &["--provider", "microsoft"];

fn ms_args<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    MS_ARGS
        .iter()
        .copied()
        .chain(extra.iter().copied())
        .collect()
}

#[test]
fn microsoft_dry_run_uses_foundry_path_and_jev_shaped_body() {
    let h = home("ms-dry");
    let (c, o, e) = run(
        &ms_args(&["--dry-run"]),
        SPEC,
        &[("MS_DECISION_ENDPOINT", "https://res.services.ai.azure.com/")],
        &h,
    );
    assert_eq!(c, 0, "{e}");
    let v: Value = serde_json::from_str(&o).unwrap();
    assert_eq!(
        v["url"],
        "https://res.services.ai.azure.com/providers/microsoft/v1/systemone"
    );
    assert_eq!(v["body"]["model"], "Microsoft-Decision-1.1");
    assert_eq!(v["body"]["questions"]["duplicate"]["type"], "noul");
}

#[test]
fn microsoft_sends_entra_bearer_to_decision_path_and_does_not_echo_it() {
    let h = home("ms-live");
    let token = "entra-token-from-env";
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut s = accept(&l);
        let (headers, b) = request(&mut s);
        let lower = headers.to_lowercase();
        assert!(lower.starts_with("post /providers/microsoft/v1/systemone "));
        assert!(lower.contains("authorization: bearer entra-token-from-env"));
        assert!(!lower.contains("api-key"));
        assert_eq!(b["model"], "my-deployment");
        reply(
            &mut s,
            "200 OK",
            r#"{"answers":{"duplicate":{"noul":0.8}}}"#,
        );
    });
    let (c, o, e) = run(
        &ms_args(&["--model", "my-deployment"]),
        SPEC,
        &[
            ("MS_DECISION_ENDPOINT", &base),
            ("AZURE_ENTRA_TOKEN", token),
        ],
        &h,
    );
    server.join().unwrap();
    assert_eq!(c, 0, "{e}");
    assert!(o.contains("0.8"));
    assert!(!o.contains(token) && !e.contains(token));
}

#[test]
fn microsoft_falls_back_to_az_for_the_token() {
    let h = home("ms-az");
    let bin = h.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let az = bin.join("az");
    std::fs::write(
        &az,
        "#!/bin/sh\n[ \"$1 $2\" = \"account get-access-token\" ] && echo token-from-az\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&az, std::fs::Permissions::from_mode(0o755)).unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut s = accept(&l);
        let (headers, _) = request(&mut s);
        assert!(headers
            .to_lowercase()
            .contains("authorization: bearer token-from-az"));
        reply(&mut s, "200 OK", r#"{"answers":{}}"#);
    });
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let (c, _, e) = run(
        MS_ARGS,
        SPEC,
        &[("MS_DECISION_ENDPOINT", &base), ("PATH", &path)],
        &h,
    );
    server.join().unwrap();
    assert_eq!(c, 0, "{e}");
}

#[test]
fn microsoft_requires_endpoint_token_and_safe_urls() {
    let h = home("ms-validate");
    let (c, _, e) = run(&ms_args(&["--dry-run"]), SPEC, &[], &h);
    assert_eq!(c, 2);
    assert!(e.contains("MS_DECISION_ENDPOINT"));

    let empty = h.join("empty-bin");
    std::fs::create_dir_all(&empty).unwrap();
    let (c, _, e) = run(
        MS_ARGS,
        SPEC,
        &[
            ("MS_DECISION_ENDPOINT", "https://res.services.ai.azure.com"),
            ("PATH", empty.to_str().unwrap()),
        ],
        &h,
    );
    assert_eq!(c, 2);
    assert!(e.contains("AZURE_ENTRA_TOKEN"));

    for url in [
        "http://res.services.ai.azure.com",
        "https://user:pw@res.services.ai.azure.com",
        "https://res.services.ai.azure.com/path",
        "https://res.services.ai.azure.com/?q=1",
        "https://res.services.ai.azure.com/#f",
    ] {
        let (c, _, e) = run(
            &ms_args(&["--dry-run"]),
            SPEC,
            &[("MS_DECISION_ENDPOINT", url)],
            &h,
        );
        assert_eq!(c, 2, "url={url}");
        assert!(!e.contains("pw@"), "url={url}");
    }
}

#[test]
fn model_is_checked_per_provider_regardless_of_arg_order() {
    let h = home("ms-model");
    let (c, _, e) = run(
        &[
            "--model",
            "jev-preview",
            "--provider",
            "typesafe",
            "--dry-run",
        ],
        SPEC,
        &[],
        &h,
    );
    assert_eq!(c, 2);
    assert!(e.contains("--model"));
    let (c, _, e) = run(
        &["--model", "dep", "--provider", "microsoft", "--dry-run"],
        SPEC,
        &[("MS_DECISION_ENDPOINT", "https://r.azure.com")],
        &h,
    );
    assert_eq!(c, 0, "{e}");
}

const OPENAI_ARGS: &[&str] = &["--provider", "openai"];
fn openai_args<'a>(provider: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    ["--provider", provider]
        .into_iter()
        .chain(extra.iter().copied())
        .collect()
}

#[test]
fn openai_dry_run_shapes_body_and_json_input() {
    let h = home("openai-dry");
    let (c, o, e) = run(&openai_args("openai", &["--dry-run"]), SPEC, &[], &h);
    assert_eq!(c, 0, "{e}");
    let v: Value = serde_json::from_str(&o).unwrap();
    assert_eq!(v["url"], "https://api.openai.com/v1/decisions");
    assert_eq!(v["body"]["model"], "gpt-6-luna");
    assert_eq!(v["body"]["input"], "I was charged twice for my order.");
    let qs = v["body"]["questions"].as_array().unwrap();
    assert_eq!(qs.len(), 3);
    assert_eq!(qs[0]["type"], "predicate");
    assert_eq!(
        qs[1]["choices"][0],
        serde_json::json!({"value":"billing","description":"Payment issues"})
    );
    assert_eq!(
        qs[2]["levels"],
        serde_json::json!([{"label":"low"},{"label":"high"}])
    );
    let json_spec =
        r#"{"input":{"x":1},"questions":[{"name":"q","type":"predicate","instructions":"check"}]}"#;
    let (c, o, e) = run(
        &openai_args("openai", &["--dry-run"]),
        json_spec,
        &[],
        &home("openai-json"),
    );
    assert_eq!(c, 0, "{e}");
    let v: Value = serde_json::from_str(&o).unwrap();
    assert_eq!(
        v["body"]["input"],
        serde_json::json!([{"role":"user","content":[{"type":"input_json","data":{"x":1}}]}])
    );
}

#[test]
fn openai_restricts_model_and_requires_api_key() {
    let h = home("openai-model");
    let (c, _, e) = run(
        &openai_args("openai", &["--model", "other", "--dry-run"]),
        SPEC,
        &[],
        &h,
    );
    assert_eq!(c, 2);
    assert!(e.contains("--model supports only gpt-6-luna"));
    let (c, o, e) = run(OPENAI_ARGS, SPEC, &[], &home("openai-no-key"));
    assert_eq!(c, 2);
    assert!(o.is_empty());
    assert!(e.contains("OPENAI_API_KEY_DECISIONS is not set"));
}

#[test]
fn openai_codex_uses_auth_file_and_scrubs_echoed_token() {
    let h = home("openai-codex");
    let codex = h.join("codex");
    std::fs::create_dir_all(&codex).unwrap();
    let token = "fake-codex-token";
    std::fs::write(
        codex.join("auth.json"),
        format!(r#"{{"tokens":{{"access_token":"{token}"}}}}"#),
    )
    .unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut s = accept(&l);
        let (headers, _) = request(&mut s);
        assert!(headers
            .to_ascii_lowercase()
            .contains("authorization: bearer fake-codex-token"));
        reply(
            &mut s,
            "401 Unauthorized",
            &format!(r#"{{"detail":"{token}"}}"#),
        );
    });
    let codex_s = codex.to_str().unwrap();
    let (c, o, e) = run(
        &openai_args("openai-codex", &[]),
        SPEC,
        &[
            ("CODEX_HOME", codex_s),
            ("OPENAI_BASE_URL", &base),
            ("OPENAI_API_KEY_DECISIONS", "different-api-key"),
        ],
        &h,
    );
    server.join().unwrap();
    assert_eq!(c, 1);
    assert!(o.is_empty());
    assert!(e.contains("HTTP 401"));
    assert!(e.contains("access token may be expired"));
    assert!(!o.contains(token) && !e.contains(token));
}

#[test]
fn openai_codex_scrubs_token_from_server_error() {
    let h = home("openai-codex-500");
    let codex = h.join("codex");
    std::fs::create_dir_all(&codex).unwrap();
    let token = "fake-codex-500-token";
    std::fs::write(
        codex.join("auth.json"),
        format!(r#"{{"tokens":{{"access_token":"{token}"}}}}"#),
    )
    .unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut s = accept(&l);
        let _ = request(&mut s);
        reply(
            &mut s,
            "500 Internal Server Error",
            &format!(r#"{{"detail":"{token}"}}"#),
        );
    });
    let (c, o, e) = run(
        &openai_args("openai-codex", &[]),
        SPEC,
        &[
            ("CODEX_HOME", codex.to_str().unwrap()),
            ("OPENAI_BASE_URL", &base),
        ],
        &h,
    );
    server.join().unwrap();
    assert_eq!(c, 1);
    assert!(!o.contains(token) && !e.contains(token));
}

#[test]
fn openai_codex_missing_auth_is_safe() {
    let h = home("openai-codex-missing");
    let codex = h.join("no-auth");
    std::fs::create_dir_all(&codex).unwrap();
    let (c, o, e) = run(
        &openai_args("openai-codex", &[]),
        SPEC,
        &[
            ("CODEX_HOME", codex.to_str().unwrap()),
            ("OPENAI_API_KEY_DECISIONS", "must-not-fallback"),
        ],
        &h,
    );
    assert_eq!(c, 2);
    assert!(o.is_empty());
    assert!(e.contains("Codex sign-in not found (run `codex login`)"));
    assert!(!e.contains("must-not-fallback"));
}
