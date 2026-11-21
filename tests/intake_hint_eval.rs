//! tools/intake-hint の計測ハーネスの受け入れテスト。
//! 実際の判断モデルには接続せず、手元の偽サーバーで集計を確かめる。

use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

const BIN: &str = env!("CARGO_BIN_EXE_foundation");

fn tool_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/intake-hint")
}

fn scratch(name: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("intake-hint-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(p.join(".hermes")).unwrap();
    p
}

/// リポジトリの外（git 作業ツリーでない場所）に作業ディレクトリを作る。
fn outside_repo(name: &str) -> PathBuf {
    let in_git = |d: &Path| d.ancestors().any(|a| a.join(".git").exists());
    let base = [
        std::env::temp_dir(),
        PathBuf::from("/dev/shm"),
        PathBuf::from("/tmp"),
    ]
    .into_iter()
    .find(|d| d.is_dir() && !in_git(d))
    .expect("git 作業ツリーの外の一時ディレクトリがない");
    let p = base.join(format!("intake-hint-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(p.join(".hermes")).unwrap();
    p
}

fn eval(args: &[&str], home: &Path, envs: &[(&str, &str)]) -> (i32, String, String) {
    let mut c = Command::new("python3");
    c.arg("-I")
        .arg(tool_dir().join("eval.py"))
        .args(args)
        .env("HOME", home)
        .env_remove("HERMES_HOME")
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("TYPESAFE_BASE_URL");
    for (k, v) in envs {
        c.env(k, v);
    }
    let o = c.output().expect("python3 を起動できない");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn jsonl(rows: &[Value]) -> String {
    rows.iter().map(|r| format!("{r}\n")).collect()
}

fn record(id: &str, source: &str, user: &str, labels: Value) -> Value {
    json!({"id": id, "source": source, "previous_ai_message": null, "user_message": user, "labels": labels})
}

fn labels(new_task: bool, standing_rule: bool) -> Value {
    json!({"proposal": false, "reply": "none", "new_task": new_task, "discuss": false, "standing_rule": standing_rule})
}

#[test]
fn tracked_datasets_are_valid_and_synthetic_only() {
    let mut found = 0;
    for entry in std::fs::read_dir(tool_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "jsonl") {
            found += 1;
            for line in std::fs::read_to_string(&path).unwrap().lines() {
                let rec: Value = serde_json::from_str(line).unwrap();
                assert_eq!(
                    rec["source"],
                    "synthetic",
                    "{}: リポジトリに実発話は置けない",
                    path.display()
                );
            }
        }
    }
    assert!(found > 0);
    let h = outside_repo("validate");
    let synthetic = tool_dir().join("synthetic.jsonl");
    let (code, out, err) = eval(
        &["validate", "--dataset", synthetic.to_str().unwrap()],
        &h,
        &[],
    );
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("'real': 0"), "{out}");
}

#[test]
fn dataset_covers_required_utterance_kinds() {
    let tags: Vec<String> = std::fs::read_to_string(tool_dir().join("synthetic.jsonl"))
        .unwrap()
        .lines()
        .flat_map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            v["tags"].as_array().cloned().unwrap_or_default()
        })
        .filter_map(|t| t.as_str().map(String::from))
        .collect();
    for need in [
        "standing_rule",
        "proposal_form",
        "reply_task",
        "reply_discuss",
    ] {
        assert!(tags.iter().any(|t| t == need), "タグ {need} の発話がない");
    }
}

#[test]
fn four_and_five_flag_specs_pass_decide_dry_run() {
    let h = outside_repo("dry");
    let data = tool_dir().join("synthetic.jsonl");
    for (variant, has_rule) in [("baseline4", false), ("narrow4", false), ("five", true)] {
        let (code, out, err) = eval(
            &[
                "run",
                "--dataset",
                data.to_str().unwrap(),
                "--variants",
                variant,
                "--foundation",
                BIN,
                "--dry-run",
            ],
            &h,
            &[("TYPESAFE_API_KEY", "dry-run-key")],
        );
        assert_eq!(code, 0, "{variant}: {err}");
        assert_eq!(out.contains("standing_rule"), has_rule, "{variant}: {out}");
        assert!(
            !out.contains("dry-run-key"),
            "{variant}: 鍵が出力に出ている"
        );
    }
}

#[test]
fn real_utterances_inside_the_repository_are_rejected() {
    let dir = scratch("real-in-repo");
    let path = dir.join("real.jsonl");
    std::fs::write(
        &path,
        jsonl(&[record("real-001", "real", "x", labels(false, false))]),
    )
    .unwrap();
    let (code, _, err) = eval(
        &["validate", "--dataset", path.to_str().unwrap()],
        &dir,
        &[],
    );
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("source=real"), "{err}");
}

fn read_request(s: &mut TcpStream) -> Value {
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head_end, len) = loop {
        let n = s.read(&mut chunk).unwrap();
        assert!(n > 0, "接続が途中で閉じた");
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
            let len = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .map(|v| v.trim().parse::<usize>().unwrap())
                .unwrap_or(0);
            break (i + 4, len);
        }
    };
    while buf.len() < head_end + len {
        let n = s.read(&mut chunk).unwrap();
        assert!(n > 0, "本文の途中で閉じた");
        buf.extend_from_slice(&chunk[..n]);
    }
    serde_json::from_slice(&buf[head_end..head_end + len]).unwrap()
}

/// 発話ごとに決めた確信度を返す偽サーバー。(発話, 問い名) の表にない述語は 0.1 を返す。
fn fake_server(
    expected: usize,
    table: Vec<(&'static str, &'static str, f64)>,
) -> (String, thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.set_nonblocking(true).unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let h = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut served = 0;
        while served < expected {
            assert!(
                Instant::now() < deadline,
                "要求が {expected} 件に満たない（{served} 件）"
            );
            let mut s = match l.accept() {
                Ok((s, _)) => s,
                Err(_) => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
            };
            s.set_nonblocking(false).unwrap();
            let body = read_request(&mut s);
            let user = body["state"]["user_message"].as_str().unwrap().to_string();
            let mut answers = serde_json::Map::new();
            for (name, q) in body["questions"].as_object().unwrap() {
                let a = if q["type"] == "choice" {
                    json!({"type": "choice", "choice": "none", "confidence": 0.9, "probabilities": {"none": 0.9}})
                } else {
                    let p = table
                        .iter()
                        .find(|(u, n, _)| *u == user && *n == name)
                        .map(|t| t.2)
                        .unwrap_or(0.1);
                    json!({"type": "noul", "noul": p})
                };
                answers.insert(name.clone(), a);
            }
            let out = json!({"model": "fake", "answers": answers, "usage": {"input_tokens": 1, "output_tokens": 1}}).to_string();
            write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
                out.len()
            )
            .unwrap();
            served += 1;
        }
    });
    (base, h)
}

fn row_of<'a>(section: &'a str, flag: &str) -> Vec<&'a str> {
    section
        .lines()
        .find(|l| l.split_whitespace().next() == Some(flag))
        .unwrap_or_else(|| panic!("{flag} の行がない:\n{section}"))
        .split_whitespace()
        .collect()
}

#[test]
fn eval_counts_false_positives_and_misses_separately_for_synthetic_and_real() {
    let h = outside_repo("run");
    let data = h.join("data");
    std::fs::create_dir_all(&data).unwrap();
    let syn = data.join("syn.jsonl");
    let real = data.join("real.jsonl");
    std::fs::write(
        &syn,
        jsonl(&[
            record("syn-a", "synthetic", "TASK", labels(true, false)),
            record("syn-b", "synthetic", "QUESTION", labels(false, false)),
        ]),
    )
    .unwrap();
    std::fs::write(
        &real,
        jsonl(&[
            record("real-a", "real", "RULE", labels(false, true)),
            record("real-b", "real", "PLAIN", labels(false, false)),
        ]),
    )
    .unwrap();
    let results = data.join("results.jsonl");

    let (base, server) = fake_server(
        4,
        vec![
            ("TASK", "new_task", 0.9),
            ("QUESTION", "new_task", 0.8),
            ("RULE", "standing_rule", 0.2),
        ],
    );
    let (code, out, err) = eval(
        &[
            "run",
            "--dataset",
            syn.to_str().unwrap(),
            "--dataset",
            real.to_str().unwrap(),
            "--variants",
            "five",
            "--foundation",
            BIN,
            "--jobs",
            "1",
            "--out",
            results.to_str().unwrap(),
        ],
        &h,
        &[
            ("TYPESAFE_API_KEY", "test-key"),
            ("TYPESAFE_BASE_URL", &base),
        ],
    );
    server.join().unwrap();
    assert_eq!(code, 0, "{err}\n{out}");

    let (syn_sec, real_sec) = out
        .split_once("== five / real")
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .expect(&out);
    assert!(
        syn_sec.contains("== five / synthetic  n=2 errors=0"),
        "{out}"
    );
    // 列は TP FP FN TN uncertain。
    assert_eq!(
        row_of(&syn_sec, "new_task")[1..],
        ["1", "1", "0", "0", "0"],
        "{out}"
    );
    assert_eq!(
        row_of(&real_sec, "standing_rule")[1..],
        ["0", "0", "1", "1", "0"],
        "{out}"
    );
    assert_eq!(
        row_of(&real_sec, "new_task")[1..],
        ["0", "0", "0", "2", "0"],
        "{out}"
    );
    assert!(syn_sec.contains("new_task FP: syn-b"), "{out}");
    assert!(real_sec.contains("standing_rule FN: real-a"), "{out}");

    // 結果ファイルには発話本文を書かない。
    let saved = std::fs::read_to_string(&results).unwrap();
    for body in ["TASK", "QUESTION", "RULE", "PLAIN"] {
        assert!(
            !saved.contains(body),
            "結果ファイルに本文 {body} が入っている"
        );
    }

    // 保存済みの結果から、通信せずに同じ集計が得られる。
    let (code, again, err) = eval(
        &[
            "report",
            "--dataset",
            syn.to_str().unwrap(),
            "--dataset",
            real.to_str().unwrap(),
            "--results",
            results.to_str().unwrap(),
        ],
        &h,
        &[],
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(again.trim(), out.trim());
}
