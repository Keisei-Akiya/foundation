use super::args::{Options, Provider};
use super::env::{auth_env, env};
use super::request::build_typesafe;
use serde_json::Value;

const DEFAULT_MODEL: &str = "jev-latest";
const DEFAULT_BASE: &str = "https://api.typesafe.ai";

pub(super) struct Target {
    pub(super) url: String,
    pub(super) auth_header: (&'static str, String),
    pub(super) body: Value,
}

pub(super) fn target(opts: &Options, spec: &Value, dry_run: bool) -> Result<Target, String> {
    match opts.provider {
        Provider::TypeSafe => {
            let model = opts.model.as_deref().unwrap_or(DEFAULT_MODEL);
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
    }
}

fn checked_endpoint(endpoint: &str) -> Result<String, String> {
    let parsed =
        url::Url::parse(endpoint).map_err(|_| "TYPESAFE_BASE_URL must be a valid absolute URL")?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return Err(
            "TYPESAFE_BASE_URL must not contain credentials, path, query, or fragment".into(),
        );
    }

    let production = parsed.scheme() == "https"
        && parsed.host_str() == Some("api.typesafe.ai")
        && parsed.port().is_none();
    let loopback_host = match parsed.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    let loopback = loopback_host && matches!(parsed.scheme(), "http" | "https");
    if !production && !loopback {
        return Err(
            "TYPESAFE_BASE_URL must be https://api.typesafe.ai or a loopback test URL".into(),
        );
    }
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}
