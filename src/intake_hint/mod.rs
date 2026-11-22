//! `foundation intake-hint`: 入口発話を Jev で分類し、hint か「人間へ戻す」結果を返す（ADR 0003）。
//!
//! 結果は参考情報であり、承認や実行の制御には使わない。
//! 判断できないとき（Jev に届かない、応答が不正、低信頼）は人間へ戻す（fail closed）。
//! 標準入力: `{"previous_ai_message": <文字列|null>, "user_message": <文字列>}`

mod judge;

use serde_json::{json, Value};
use std::io::Read;

const USAGE: &str = "usage: foundation intake-hint [--dry-run] [--timeout <secs>]  (stdin: {\"previous_ai_message\": string|null, \"user_message\": string})";
const QUESTIONS: &str = include_str!("questions.json");
const MAX_INPUT: u64 = 1024 * 1024;

pub fn run(args: &[String]) -> i32 {
    let (dry_run, timeout) = match parse_args(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("foundation intake-hint: {e}\n{USAGE}");
            return 2;
        }
    };
    let spec = match read_input().and_then(|input| build_spec(&input)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("foundation intake-hint: {e}\n{USAGE}");
            return 2;
        }
    };
    if dry_run {
        println!("{}", serde_json::to_string_pretty(&spec).unwrap());
        return 0;
    }
    let result = match crate::decide::communicate(&spec, "jev-latest", timeout) {
        Ok((text, _usage)) => judge::judge(&text),
        // 本文は出さない。応答が得られなかったという事実だけを返す。
        Err(_) => json!({"route": "human", "reason": "undecidable"}),
    };
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    0
}

fn parse_args(args: &[String]) -> Result<(bool, u64), String> {
    let mut dry_run = false;
    let mut timeout = 60;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--timeout" => {
                timeout = it
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .filter(|t| (1..=600).contains(t))
                    .ok_or("--timeout must be 1..=600")?
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok((dry_run, timeout))
}

fn read_input() -> Result<Value, String> {
    let mut raw = String::new();
    std::io::stdin()
        .take(MAX_INPUT + 1)
        .read_to_string(&mut raw)
        .map_err(|e| format!("cannot read input: {e}"))?;
    if raw.len() as u64 > MAX_INPUT {
        return Err("input is too large".into());
    }
    serde_json::from_str(&raw).map_err(|e| format!("input is not valid JSON: {e}"))
}

fn build_spec(input: &Value) -> Result<Value, String> {
    let obj = input.as_object().ok_or("input must be a JSON object")?;
    let user = obj
        .get("user_message")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or("`user_message` must be a non-empty string")?;
    let previous = match obj.get("previous_ai_message") {
        None | Some(Value::Null) => Value::Null,
        Some(Value::String(s)) => Value::String(s.clone()),
        Some(_) => return Err("`previous_ai_message` must be a string or null".into()),
    };
    let questions: Value =
        serde_json::from_str(QUESTIONS).map_err(|e| format!("questions: {e}"))?;
    Ok(json!({
        "input": {"previous_ai_message": previous, "user_message": user},
        "questions": questions,
    }))
}
