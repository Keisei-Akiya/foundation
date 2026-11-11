use super::args::Options;
use super::env::{auth_env, env};
use super::{build_microsoft, build_openai, Provider};
use serde_json::Value;

const DEFAULT_OPENAI_MODEL: &str = "gpt-6-luna";

pub(super) struct Target {
    pub(super) url: String,
    pub(super) auth_header: (&'static str, String),
    pub(super) body: Value,
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

pub(super) fn target(opts: &Options, spec: &Value, dry_run: bool) -> Result<Target, String> {
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
