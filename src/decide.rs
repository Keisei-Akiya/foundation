//! `foundation decide`: System One 型の decision モデルを呼ぶ。
//!
//! 共通の質問仕様 (spec) を受け取り、プロバイダごとのリクエストに変換して送る。
//! - openai: `POST {OPENAI_BASE_URL|https://api.openai.com}/v1/decisions`
//! - microsoft: `POST {AZURE_ENDPOINT}/providers/microsoft/v1/systemone`
//!
//! 認証情報は環境変数からのみ読み、標準出力・標準エラーには出さない。
//! 応答はプロバイダの JSON をそのまま標準出力へ書く。

use serde_json::{json, Map, Value};
use std::io::Read;
use std::time::Duration;

const USAGE: &str = "usage: foundation decide --provider <openai|microsoft> [--spec <file|->] [--model <name>] [--dry-run] [--timeout <secs>]";
const MAX_SPEC: u64 = 4 * 1024 * 1024;
const DEFAULT_OPENAI_MODEL: &str = "gpt-6-luna";

#[derive(Debug, PartialEq)]
pub enum Provider {
    OpenAi,
    Microsoft,
}

struct Options {
    provider: Provider,
    spec: String,
    model: Option<String>,
    dry_run: bool,
    timeout: u64,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut provider = None;
    let mut spec = "-".to_string();
    let mut model = None;
    let mut dry_run = false;
    let mut timeout = 60;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match arg.as_str() {
            "--provider" => {
                provider = Some(match value("--provider")?.as_str() {
                    "openai" => Provider::OpenAi,
                    "microsoft" => Provider::Microsoft,
                    other => return Err(format!("unknown provider: {other}")),
                })
            }
            "--spec" => spec = value("--spec")?,
            "--model" => model = Some(value("--model")?),
            "--timeout" => {
                timeout = value("--timeout")?
                    .parse::<u64>()
                    .ok()
                    .filter(|t| (1..=600).contains(t))
                    .ok_or("--timeout must be 1..=600")?
            }
            "--dry-run" => dry_run = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Options {
        provider: provider.ok_or("--provider is required")?,
        spec,
        model,
        dry_run,
        timeout,
    })
}

fn read_spec(path: &str) -> Result<Value, String> {
    let mut raw = String::new();
    let result = if path == "-" {
        std::io::stdin().take(MAX_SPEC + 1).read_to_string(&mut raw)
    } else {
        std::fs::File::open(path).and_then(|f| f.take(MAX_SPEC + 1).read_to_string(&mut raw))
    };
    result.map_err(|e| format!("cannot read spec: {e}"))?;
    if raw.len() as u64 > MAX_SPEC {
        return Err("spec is too large".into());
    }
    serde_json::from_str(&raw).map_err(|e| format!("spec is not valid JSON: {e}"))
}

fn str_field<'a>(obj: &'a Map<String, Value>, key: &str, ctx: &str) -> Result<&'a str, String> {
    obj.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{ctx}: `{key}` must be a non-empty string"))
}

/// 共通 spec:
/// { "input": <string|json>, "questions": [
///   {"name","type":"predicate|choice|score","instructions",
///    "choices":[{"value","description"}], "levels":[{"label","description"}]} ] }
fn validate(spec: &Value) -> Result<(&Value, &Vec<Value>), String> {
    let obj = spec.as_object().ok_or("spec must be a JSON object")?;
    let input = obj.get("input").ok_or("spec: `input` is required")?;
    if input.is_null() || input.as_str().is_some_and(|s| s.trim().is_empty()) {
        return Err("spec: `input` must not be empty".into());
    }
    let questions = obj
        .get("questions")
        .and_then(Value::as_array)
        .filter(|q| !q.is_empty())
        .ok_or("spec: `questions` must be a non-empty array")?;
    let mut names = std::collections::HashSet::new();
    for (i, q) in questions.iter().enumerate() {
        let ctx = format!("questions[{i}]");
        let q = q.as_object().ok_or(format!("{ctx}: must be an object"))?;
        let name = str_field(q, "name", &ctx)?;
        if !names.insert(name) {
            return Err(format!("{ctx}: duplicate name `{name}`"));
        }
        str_field(q, "instructions", &ctx)?;
        match str_field(q, "type", &ctx)? {
            "predicate" => {}
            "choice" => {
                let choices = q
                    .get("choices")
                    .and_then(Value::as_array)
                    .filter(|c| c.len() >= 2)
                    .ok_or(format!("{ctx}: `choices` needs at least 2 items"))?;
                let mut values = std::collections::HashSet::new();
                for c in choices {
                    let c = c
                        .as_object()
                        .ok_or(format!("{ctx}: choice must be an object"))?;
                    let value = str_field(c, "value", &ctx)?;
                    if !values.insert(value) {
                        return Err(format!("{ctx}: duplicate choice value `{value}`"));
                    }
                    str_field(c, "description", &ctx)?;
                }
            }
            "score" => {
                let levels = q
                    .get("levels")
                    .and_then(Value::as_array)
                    .filter(|c| c.len() >= 2)
                    .ok_or(format!("{ctx}: `levels` needs at least 2 items"))?;
                for l in levels {
                    let l = l
                        .as_object()
                        .ok_or(format!("{ctx}: level must be an object"))?;
                    str_field(l, "label", &ctx)?;
                }
            }
            other => return Err(format!("{ctx}: unknown type `{other}`")),
        }
    }
    Ok((input, questions))
}

pub fn build_openai(spec: &Value, model: &str) -> Result<Value, String> {
    let (input, questions) = validate(spec)?;
    if questions.len() > 400 {
        return Err("OpenAI Decisions API accepts at most 400 questions per request".into());
    }
    for (i, q) in questions.iter().enumerate() {
        if q["type"] == "score" {
            let n = q["levels"].as_array().map_or(0, Vec::len);
            if !(2..=10).contains(&n) {
                return Err(format!(
                    "questions[{i}]: OpenAI score levels must contain 2..=10 items"
                ));
            }
        }
    }
    let input = match input {
        Value::String(_) => input.clone(),
        Value::Object(_) | Value::Array(_) => json!([{
            "role": "user",
            "content": [{"type": "input_json", "data": input}],
        }]),
        _ => return Err("OpenAI input JSON must be an object or array".into()),
    };
    Ok(json!({"model": model, "input": input, "questions": questions}))
}

pub fn build_microsoft(spec: &Value, deployment: &str) -> Result<Value, String> {
    let (input, questions) = validate(spec)?;
    let mut out = Map::new();
    for q in questions {
        let name = q["name"].as_str().unwrap_or_default();
        let mut m = Map::new();
        m.insert("instructions".into(), q["instructions"].clone());
        match q["type"].as_str().unwrap_or_default() {
            "predicate" => {
                m.insert("type".into(), "noul".into());
            }
            "choice" => {
                m.insert("type".into(), "choice".into());
                let criteria: Map<String, Value> = q["choices"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| {
                        (
                            c["value"].as_str().unwrap_or_default().to_string(),
                            c["description"].clone(),
                        )
                    })
                    .collect();
                m.insert("criteria".into(), Value::Object(criteria));
            }
            _ => {
                m.insert("type".into(), "score".into());
                let labels: Vec<Value> = q["levels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|l| {
                        let label = l["label"].as_str().unwrap_or_default();
                        match l["description"].as_str().filter(|d| !d.trim().is_empty()) {
                            Some(description) => json!(format!("{label}: {description}")),
                            None => json!(label),
                        }
                    })
                    .collect();
                m.insert("criteria".into(), Value::Array(labels));
            }
        }
        out.insert(name.to_string(), Value::Object(m));
    }
    Ok(json!({"model": deployment, "state": input, "questions": out}))
}

fn dotenv_values() -> std::collections::HashMap<String, String> {
    const ALLOWED: &[&str] = &[
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "AZURE_ENDPOINT",
        "AZURE_API_KEY",
        "AZURE_ENTRA_TOKEN",
        "DEPLOYMENT_NAME",
    ];
    let home = env_without_dotenv("HERMES_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|p| p.join(".hermes"))
        })
        .or_else(|| std::env::home_dir().map(|p| p.join(".hermes")));
    let Some(path) = home.map(|p| p.join(".env")) else {
        return Default::default();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Default::default();
    };
    let mut values = std::collections::HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((name, raw)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if !ALLOWED.contains(&name) {
            continue;
        }
        if let Some(value) = dotenv_value(raw.trim()) {
            if !value.trim().is_empty() {
                values.insert(name.to_string(), value);
            }
        }
    }
    values
}

fn dotenv_value(raw: &str) -> Option<String> {
    if let Some(rest) = raw.strip_prefix('\'') {
        let end = rest.find('\'')?;
        let tail = rest[end + 1..].trim();
        return (tail.is_empty() || tail.starts_with('#')).then(|| rest[..end].to_string());
    }
    if let Some(rest) = raw.strip_prefix('"') {
        let mut value = String::new();
        let mut escaped = false;
        let mut end = None;
        for (i, c) in rest.char_indices() {
            if escaped {
                value.push(match c {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    other => other,
                });
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                end = Some(i + c.len_utf8());
                break;
            } else {
                value.push(c);
            }
        }
        let tail = rest.get(end?..)?.trim();
        return (tail.is_empty() || tail.starts_with('#')).then_some(value);
    }
    Some(
        raw.split_once(" #")
            .map_or(raw, |(v, _)| v)
            .trim()
            .to_string(),
    )
}

fn env_without_dotenv(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn env(name: &str) -> Option<String> {
    env_without_dotenv(name).or_else(|| dotenv_values().remove(name))
}

fn auth_env(names: &[&str]) -> Option<(String, String)> {
    for name in names {
        if let Some(value) = env_without_dotenv(name) {
            return Some(((*name).to_string(), value));
        }
    }
    let values = dotenv_values();
    names.iter().find_map(|name| {
        values
            .get(*name)
            .cloned()
            .map(|value| ((*name).to_string(), value))
    })
}

struct Target {
    url: String,
    auth_header: (&'static str, String),
    body: Value,
}

/// Allow HTTPS endpoints and plain HTTP only for local test servers.
/// Reject URL components that can carry credentials or be exposed in dry-run output.
fn checked_endpoint(endpoint: &str) -> Result<String, String> {
    let endpoint = endpoint.trim_end_matches('/');
    let (scheme, rest) = endpoint
        .split_once("://")
        .ok_or("endpoint must be an absolute HTTPS URL")?;
    let authority = rest.split('/').next().unwrap_or_default();
    if rest.contains('?') || rest.contains('#') || authority.contains('@') || authority.is_empty() {
        return Err("endpoint must not contain credentials, query, or fragment".into());
    }
    if endpoint.chars().any(char::is_control) {
        return Err("endpoint contains invalid control characters".into());
    }
    let is_loopback = ["localhost", "127.0.0.1", "[::1]"].iter().any(|host| {
        authority == *host
            || authority
                .strip_prefix(host)
                .is_some_and(|s| s.starts_with(':'))
    });
    if scheme != "https" && !(scheme == "http" && is_loopback) {
        return Err(
            "endpoint must use HTTPS (HTTP is allowed only for localhost test servers)".into(),
        );
    }
    Ok(endpoint.to_string())
}

fn target(opts: &Options, spec: &Value, dry_run: bool) -> Result<Target, String> {
    match opts.provider {
        Provider::OpenAi => {
            let model = opts
                .model
                .clone()
                .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.into());
            let body = build_openai(spec, &model)?;
            let base = checked_endpoint(
                &env("OPENAI_BASE_URL").unwrap_or_else(|| "https://api.openai.com".into()),
            )?;
            let key = match auth_env(&["OPENAI_API_KEY"]) {
                Some((_, k)) => k,
                None if dry_run => String::new(),
                None => return Err("OPENAI_API_KEY is not set".into()),
            };
            Ok(Target {
                url: format!("{base}/v1/decisions"),
                auth_header: ("Authorization", format!("Bearer {key}")),
                body,
            })
        }
        Provider::Microsoft => {
            let deployment = opts
                .model
                .clone()
                .or_else(|| env("DEPLOYMENT_NAME"))
                .ok_or("deployment name is required: pass --model or set DEPLOYMENT_NAME")?;
            let body = build_microsoft(spec, &deployment)?;
            let endpoint = match env("AZURE_ENDPOINT") {
                Some(e) => e,
                None if dry_run => "<AZURE_ENDPOINT>".into(),
                None => return Err("AZURE_ENDPOINT is not set".into()),
            };
            let auth_header = match auth_env(&["AZURE_API_KEY", "AZURE_ENTRA_TOKEN"]) {
                Some((name, k)) if name == "AZURE_API_KEY" => ("api-key", k),
                Some((_, t)) => ("Authorization", format!("Bearer {t}")),
                None if dry_run => ("api-key", String::new()),
                _ => return Err("set AZURE_API_KEY or AZURE_ENTRA_TOKEN".into()),
            };
            let endpoint = if dry_run && endpoint == "<AZURE_ENDPOINT>" {
                endpoint
            } else {
                checked_endpoint(&endpoint)?
            };
            Ok(Target {
                url: format!("{}/providers/microsoft/v1/systemone", endpoint),
                auth_header,
                body,
            })
        }
    }
}

fn send(t: &Target, timeout: u64) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(timeout)))
        .build()
        .into();
    let mut resp = agent
        .post(&t.url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header(t.auth_header.0, &t.auth_header.1)
        .send(t.body.to_string())
        // 接続エラー文言に認証情報は含まれないが、念のため URL もヘッダも出さない。
        .map_err(|e| format!("request failed: {}", scrub(&e.to_string(), t)))?;
    let status = resp.status().as_u16();
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("cannot read response: {e}"))?;
    if (200..300).contains(&status) {
        Ok(text)
    } else {
        Err(format!("HTTP {status}: {}", scrub(&text, t)))
    }
}

fn scrub(text: &str, t: &Target) -> String {
    let secret = t.auth_header.1.trim_start_matches("Bearer ");
    if secret.is_empty() {
        text.to_string()
    } else {
        text.replace(secret, "[REDACTED]")
    }
}

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
