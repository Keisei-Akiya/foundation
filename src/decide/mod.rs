//! `foundation decide`: TypeSafe System One の Jev モデルを呼ぶ。
//! 認証情報は環境変数からのみ読み、標準出力・標準エラーには出さない。

mod args;
mod env;
mod http;
mod request;
mod spec;
mod target;

use args::{parse_args, USAGE};
use http::send;

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
