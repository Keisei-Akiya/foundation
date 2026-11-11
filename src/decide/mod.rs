//! `foundation decide`: System One 型の decision モデルを呼ぶ。
//!
//! 共通の質問仕様 (spec) を受け取り、プロバイダごとのリクエストに変換して送る。
//! - openai: `POST {OPENAI_BASE_URL|https://api.openai.com}/v1/decisions`
//! - microsoft: `POST {AZURE_ENDPOINT}/providers/microsoft/v1/systemone`
//!
//! 認証情報は環境変数からのみ読み、標準出力・標準エラーには出さない。
//! 応答はプロバイダの JSON をそのまま標準出力へ書く。

mod args;
mod env;
mod http;
mod request;
mod spec;
mod target;

pub use args::Provider;
pub use request::{build_microsoft, build_openai};

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
        let out = json!({"method": "POST", "url": t.url, "body": t.body});
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
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
