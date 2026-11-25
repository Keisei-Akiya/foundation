//! `foundation decide`: TypeSafe System One の Jev と、Foundry の Microsoft-Decision を呼ぶ。
//! 認証情報は環境変数（Microsoft は `az` も）からのみ読み、標準出力・標準エラーには出さない。

mod args;
mod env;
mod http;
mod request;
mod spec;
mod target;

use args::{parse_args, USAGE};
use http::send;
use target::target_for;

use serde_json::{json, Value};
use spec::read_spec;
use target::target;

pub fn run(args: &[String]) -> i32 {
    let opts = match parse_args(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("foundation decide: {e}\n{USAGE}");
            return 2;
        }
    };
    let spec = match read_spec(&opts.spec) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("foundation decide: {e}");
            return 2;
        }
    };
    let t = match target(&opts, &spec, opts.dry_run) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("foundation decide: {e}");
            return 2;
        }
    };
    if opts.dry_run {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"method":"POST", "url":t.url, "body":t.body}))
                .unwrap()
        );
        return 0;
    }
    match send(&t, opts.timeout) {
        Ok(text) => {
            match serde_json::from_str::<Value>(&text) {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
                Err(_) => println!("{text}"),
            }
            0
        }
        Err(e) => {
            eprintln!("foundation decide: {e}");
            1
        }
    }
}

/// Acquire Jev authentication, send a generic spec, and return response JSON and usage.
pub(crate) fn communicate(
    spec: &Value,
    model: &str,
    timeout: u64,
) -> Result<(String, Value), String> {
    let target = target_for(spec, model, false)?;
    let text = send(&target, timeout)?;
    let usage = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|response| response.get("usage").cloned())
        .unwrap_or(Value::Null);
    Ok((text, usage))
}
