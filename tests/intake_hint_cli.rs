//! `foundation intake-hint` の外から見える振る舞い。Jev はモックサーバで代用する。
//! 発話は合成の短文だけを使う（実発話は置かない）。
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(8);

fn home(name: &str) -> std::path::PathBuf {
    // テストは並列に走るので、呼び出しごとに別のディレクトリを使う。
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("foundation-hint-{name}-{}-{n}", std::process::id()));
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
    c.arg("intake-hint")
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

fn request(s: &mut TcpStream) -> Value {
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
    serde_json::from_slice(&body).unwrap()
}

fn reply(s: &mut TcpStream, status: &str, body: &str) {
    write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
}

/// Jev の応答を返すモックを立てて CLI を 1 回呼ぶ。返り値は (終了コード, stdout, 送られた本文)。
fn with_jev(status: &'static str, body: String, input: &str) -> (i32, String, Value) {
    let h = home("jev");
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut s = accept(&l);
        let sent = request(&mut s);
        reply(&mut s, status, &body);
        sent
    });
    let (code, out, err) = run(
        &[],
        input,
        &[
            ("TYPESAFE_BASE_URL", &base),
            ("TYPESAFE_API_KEY", "mock-key"),
        ],
        &h,
    );
    let sent = server.join().unwrap();
    assert!(!err.contains("mock-key") && !out.contains("mock-key"));
    (code, out, sent)
}

fn answers(proposal: f64, new_task: f64, discuss: f64, standing: f64, choice: &str) -> String {
    json!({"model":"jev-1.14.0","answers":{
        "proposal":{"noul":proposal,"type":"noul"},
        "reply":{"choice":choice,"confidence":0.9,"type":"choice"},
        "new_task":{"noul":new_task,"type":"noul"},
        "discuss":{"noul":discuss,"type":"noul"},
        "standing_rule":{"noul":standing,"type":"noul"},
    }})
    .to_string()
}

const INPUT: &str = r#"{"previous_ai_message":"synthetic: shall I proceed?","user_message":"synthetic: yes, go ahead"}"#;

fn parse(out: &str) -> Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("stdout is not JSON: {e}: {out}"))
}

#[test]
fn clear_utterance_returns_flags_hint() {
    let (c, o, sent) = with_jev("200 OK", answers(0.9, 0.05, 0.1, 0.02, "approve"), INPUT);
    assert_eq!(c, 0);
    let v = parse(&o);
    assert_eq!(v["route"], "hint");
    assert_eq!(v["flags"]["proposal"], true);
    assert_eq!(v["flags"]["new_task"], false);
    assert_eq!(v["flags"]["discuss"], false);
    assert_eq!(v["flags"]["standing_rule"], false);
    assert_eq!(v["reply"], "approve");
    assert_eq!(v["instruction"], Value::Null);
    assert_eq!(v["probabilities"]["proposal"], 0.9);
    // 送った本文: 5 問、入力は前の AI 発言と最後のユーザー発言。
    assert_eq!(sent["model"], "jev-latest");
    assert_eq!(
        sent["state"]["previous_ai_message"],
        "synthetic: shall I proceed?"
    );
    assert_eq!(sent["state"]["user_message"], "synthetic: yes, go ahead");
    let mut names: Vec<_> = sent["questions"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["discuss", "new_task", "proposal", "reply", "standing_rule"]
    );
}

#[test]
fn mixed_utterance_returns_combination_of_flags() {
    // 承認しつつ新しい作業も頼む発話。排他ラベルではなく複数のフラグが立つ。
    let (c, o, _) = with_jev("200 OK", answers(0.95, 0.85, 0.1, 0.05, "approve"), INPUT);
    assert_eq!(c, 0);
    let v = parse(&o);
    assert_eq!(v["route"], "hint");
    assert_eq!(v["flags"]["proposal"], true);
    assert_eq!(v["flags"]["new_task"], true);
    assert_eq!(v["instruction"], "new_task");
}

#[test]
fn standing_rule_takes_priority_over_new_task() {
    let (c, o, _) = with_jev("200 OK", answers(0.1, 0.9, 0.1, 0.85, "none"), INPUT);
    assert_eq!(c, 0);
    let v = parse(&o);
    assert_eq!(v["route"], "hint");
    assert_eq!(v["flags"]["standing_rule"], true);
    assert_eq!(v["flags"]["new_task"], true);
    assert_eq!(v["instruction"], "standing_rule");
}

#[test]
fn low_confidence_returns_human_route() {
    for (new_task, standing) in [
        (0.5, 0.02),
        (0.35, 0.02),
        (0.65, 0.02),
        (0.02, 0.41),
        (0.02, 0.6),
    ] {
        let (c, o, _) = with_jev(
            "200 OK",
            answers(0.1, new_task, 0.1, standing, "none"),
            INPUT,
        );
        assert_eq!(c, 0);
        let v = parse(&o);
        assert_eq!(
            v["route"], "human",
            "new_task={new_task} standing={standing}"
        );
        assert_eq!(v["reason"], "low_confidence");
        assert_eq!(v["probabilities"]["new_task"], new_task);
        assert!(
            v.get("flags").is_none(),
            "人間へ戻すときは flags を返さない"
        );
    }
}

#[test]
fn just_outside_the_band_is_a_hint() {
    for new_task in [0.3499, 0.6501] {
        let (_, o, _) = with_jev("200 OK", answers(0.1, new_task, 0.1, 0.02, "none"), INPUT);
        assert_eq!(parse(&o)["route"], "hint", "new_task={new_task}");
    }
}

#[test]
fn only_instruction_flags_trigger_low_confidence() {
    // proposal / discuss が帯の中でも経路には影響しない（hint として添えるだけ）。
    let (_, o, _) = with_jev("200 OK", answers(0.5, 0.02, 0.5, 0.02, "none"), INPUT);
    let v = parse(&o);
    assert_eq!(v["route"], "hint");
    assert_eq!(v["flags"]["proposal"], true);
    assert_eq!(v["flags"]["discuss"], true);
}

#[test]
fn jev_http_error_returns_human_route() {
    let (c, o, _) = with_jev(
        "500 Internal Server Error",
        r#"{"detail":"boom"}"#.into(),
        INPUT,
    );
    assert_eq!(c, 0);
    let v = parse(&o);
    assert_eq!(v["route"], "human");
    assert_eq!(v["reason"], "undecidable");
}

#[test]
fn malformed_jev_responses_return_human_route() {
    let missing_flag =
        json!({"answers":{"proposal":{"noul":0.9},"new_task":{"noul":0.1}}}).to_string();
    let out_of_range = answers(0.9, 1.5, 0.1, 0.02, "none");
    let not_number = json!({"answers":{
        "proposal":{"noul":"high"},"new_task":{"noul":0.1},
        "discuss":{"noul":0.1},"standing_rule":{"noul":0.1}}})
    .to_string();
    for body in [
        "not json".to_string(),
        "{}".to_string(),
        missing_flag,
        out_of_range,
        not_number,
    ] {
        let (c, o, _) = with_jev("200 OK", body.clone(), INPUT);
        assert_eq!(c, 0, "{body}");
        let v = parse(&o);
        assert_eq!(v["route"], "human", "{body}");
        assert_eq!(v["reason"], "undecidable", "{body}");
    }
}

#[test]
fn unknown_reply_choice_becomes_null_without_failing() {
    let (_, o, _) = with_jev(
        "200 OK",
        answers(0.1, 0.02, 0.1, 0.02, "something-else"),
        INPUT,
    );
    let v = parse(&o);
    assert_eq!(v["route"], "hint");
    assert_eq!(v["reply"], Value::Null);
}

#[test]
fn unreachable_jev_and_missing_key_return_human_route() {
    let h = home("down");
    // 接続できない（誰も待ち受けていないポート）。
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    drop(l);
    let (c, o, e) = run(
        &["--timeout", "2"],
        INPUT,
        &[
            ("TYPESAFE_BASE_URL", &base),
            ("TYPESAFE_API_KEY", "mock-key"),
        ],
        &h,
    );
    assert_eq!(c, 0, "{e}");
    assert_eq!(parse(&o)["reason"], "undecidable");
    // API キーが無い。
    let (c, o, _) = run(&[], INPUT, &[], &h);
    assert_eq!(c, 0);
    assert_eq!(parse(&o)["route"], "human");
}

#[test]
fn first_message_without_previous_ai_message_is_accepted() {
    let (c, o, sent) = with_jev(
        "200 OK",
        answers(0.02, 0.9, 0.1, 0.02, "none"),
        r#"{"user_message":"synthetic: please add a test"}"#,
    );
    assert_eq!(c, 0);
    assert_eq!(parse(&o)["flags"]["new_task"], true);
    assert_eq!(sent["state"]["previous_ai_message"], Value::Null);
}

#[test]
fn invalid_input_is_rejected_before_any_request() {
    let h = home("invalid");
    for input in [
        "not json",
        "[]",
        r#"{}"#,
        r#"{"user_message":""}"#,
        r#"{"user_message":"  "}"#,
        r#"{"user_message":5}"#,
        r#"{"user_message":"x","previous_ai_message":5}"#,
    ] {
        let (c, o, e) = run(&[], input, &[], &h);
        assert_eq!(c, 2, "input={input} stderr={e}");
        assert!(o.is_empty());
    }
    let (c, _, e) = run(&["--bogus"], INPUT, &[], &h);
    assert_eq!(c, 2);
    assert!(e.contains("unknown argument"));
}

#[test]
fn dry_run_prints_decide_spec_without_network_or_key() {
    let h = home("dry");
    let (c, o, e) = run(&["--dry-run"], INPUT, &[], &h);
    assert_eq!(c, 0, "{e}");
    let v = parse(&o);
    assert_eq!(v["input"]["user_message"], "synthetic: yes, go ahead");
    assert_eq!(v["questions"].as_array().unwrap().len(), 5);
}

#[test]
fn embedded_questions_match_the_measured_spec() {
    // 計測した質問文（tools/）と、実際に送る質問文がずれていないこと。
    let h = home("drift");
    let (_, o, _) = run(&["--dry-run"], INPUT, &[], &h);
    let sent = parse(&o)["questions"].clone();
    let measured: Value =
        serde_json::from_str(include_str!("../tools/intake-hint/questions/five.json")).unwrap();
    assert_eq!(sent, measured);
}
