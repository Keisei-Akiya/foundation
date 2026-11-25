use super::args::{Options, Provider};
use super::env::{auth_env, env};
use super::request::build_typesafe;
use serde_json::Value;

const DEFAULT_MODEL: &str = "jev-latest";
const DEFAULT_BASE: &str = "https://api.typesafe.ai";
const MS_DEFAULT_MODEL: &str = "Microsoft-Decision-1.1";
const MS_TOKEN_RESOURCE: &str = "https://cognitiveservices.azure.com";

pub(super) struct Target {
    pub(super) url: String,
    pub(super) auth_header: (&'static str, String),
    pub(super) body: Value,
}

pub(super) fn target(opts: &Options, spec: &Value, dry_run: bool) -> Result<Target, String> {
    match opts.provider {
        Provider::TypeSafe => target_for(
            spec,
            opts.model.as_deref().unwrap_or(DEFAULT_MODEL),
            dry_run,
        ),
        Provider::Microsoft => target_microsoft(
            spec,
            opts.model.as_deref().unwrap_or(MS_DEFAULT_MODEL),
            dry_run,
        ),
    }
}

/// Foundry の Microsoft-Decision。本文は Jev と同形で、URL と Entra ID の Bearer だけが違う。
fn target_microsoft(spec: &Value, model: &str, dry_run: bool) -> Result<Target, String> {
    let body = build_typesafe(spec, model)?;
    let endpoint = env("MS_DECISION_ENDPOINT").ok_or("MS_DECISION_ENDPOINT is not set")?;
    let endpoint = checked_https_base(&endpoint, "MS_DECISION_ENDPOINT")?;
    let token = if dry_run {
        String::new()
    } else {
        entra_token()?
    };
    Ok(Target {
        url: format!("{endpoint}/providers/microsoft/v1/systemone"),
        auth_header: ("Authorization", format!("Bearer {token}")),
        body,
    })
}

/// AZURE_ENTRA_TOKEN があればそれを使い、無ければ `az` で取る（トークンは約1時間で切れる）。
fn entra_token() -> Result<String, String> {
    if let Some(token) = env("AZURE_ENTRA_TOKEN") {
        return Ok(token);
    }
    let out = std::process::Command::new("az")
        .args([
            "account",
            "get-access-token",
            "--resource",
            MS_TOKEN_RESOURCE,
            "--query",
            "accessToken",
            "-o",
            "tsv",
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("AZURE_ENTRA_TOKEN is not set and az could not run: {e}"))?;
    let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || token.is_empty() {
        return Err(
            "AZURE_ENTRA_TOKEN is not set and `az account get-access-token` failed (az login?)"
                .into(),
        );
    }
    Ok(token)
}

/// https（または loopback の http）で、資格情報・パス・クエリ・フラグメントを含まない URL だけ通す。
fn checked_https_base(endpoint: &str, var: &str) -> Result<String, String> {
    let parsed = parse_base(endpoint, var)?;
    if !(parsed.scheme() == "https" || is_loopback(&parsed)) {
        return Err(format!("{var} must be https or a loopback test URL"));
    }
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

fn parse_base(endpoint: &str, var: &str) -> Result<url::Url, String> {
    let parsed =
        url::Url::parse(endpoint).map_err(|_| format!("{var} must be a valid absolute URL"))?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return Err(format!(
            "{var} must not contain credentials, path, query, or fragment"
        ));
    }
    Ok(parsed)
}

fn is_loopback(parsed: &url::Url) -> bool {
    let host = match parsed.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    host && matches!(parsed.scheme(), "http" | "https")
}

pub(super) fn target_for(spec: &Value, model: &str, dry_run: bool) -> Result<Target, String> {
    let body = build_typesafe(spec, model)?;
    let endpoint =
        checked_endpoint(&env("TYPESAFE_BASE_URL").unwrap_or_else(|| DEFAULT_BASE.into()))?;
    let key = match auth_env(&["TYPESAFE_API_KEY"]) {
        Some((_, key)) => key,
        None if dry_run => String::new(),
        None => return Err("TYPESAFE_API_KEY is not set".into()),
    };
    Ok(Target {
        url: format!("{}/v1/systemone", endpoint.trim_end_matches('/')),
        auth_header: ("Authorization", format!("Bearer {key}")),
        body,
    })
}

fn checked_endpoint(endpoint: &str) -> Result<String, String> {
    let parsed = parse_base(endpoint, "TYPESAFE_BASE_URL")?;
    let production = parsed.scheme() == "https"
        && parsed.host_str() == Some("api.typesafe.ai")
        && parsed.port().is_none();
    if !production && !is_loopback(&parsed) {
        return Err(
            "TYPESAFE_BASE_URL must be https://api.typesafe.ai or a loopback test URL".into(),
        );
    }
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}
