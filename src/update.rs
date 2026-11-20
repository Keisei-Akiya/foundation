use serde_json::Value;
use std::{env, fs, path::PathBuf, time::Duration};

const RELEASE_API: &str = "https://api.github.com/repos/Keisei-Akiya/foundation/releases/latest";
const BINARY_VERSION: &str = env!("CARGO_PKG_VERSION");

fn hermes_home(args: &[String]) -> Result<PathBuf, String> {
    let mut explicit = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--hermes-home" => {
                explicit =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        "missing value for --hermes-home".to_string()
                    })?))
            }
            other => return Err(format!("unknown update argument: {other}")),
        }
    }
    if let Some(path) = explicit {
        return Ok(path);
    }
    if let Some(path) = env::var_os("HERMES_HOME") {
        return Ok(PathBuf::from(path));
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".hermes"))
        .ok_or_else(|| "HOME is not set; use --hermes-home to specify the Hermes home".to_string())
}

/// Resolve the latest release tag through the configured GitHub release API endpoint.
pub fn resolve_latest_tag() -> Result<String, String> {
    let endpoint =
        env::var("FOUNDATION_RELEASE_API_URL").unwrap_or_else(|_| RELEASE_API.to_string());
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let mut response = agent
        .get(&endpoint)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "foundation-update-check")
        .call()
        .map_err(|error| format!("release API request failed: {error}"))?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| format!("could not read release API response: {error}"))?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "release API returned HTTP {status}: {}",
            body.trim()
        ));
    }
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| format!("invalid release API JSON: {error}"))?;
    value
        .get("tag_name")
        .and_then(Value::as_str)
        .filter(|tag| !tag.trim().is_empty())
        .map(|tag| tag.trim().trim_start_matches('v').to_string())
        .ok_or_else(|| "release API response is missing a string tag_name".to_string())
}

fn version_segments(version: &str) -> Result<Vec<u64>, String> {
    let version = version.trim().trim_start_matches('v');
    let parts: Vec<u64> = version
        .split('.')
        .map(|part| {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            if digits.is_empty() {
                Err(format!("invalid version: {version}"))
            } else {
                digits
                    .parse()
                    .map_err(|_| format!("invalid version: {version}"))
            }
        })
        .collect::<Result<_, _>>()?;
    if parts.is_empty() {
        return Err(format!("invalid version: {version}"));
    }
    Ok(parts)
}

/// Return true when either installed version is numerically older than the release.
pub fn update_available(latest: &str, binary: &str, shared: Option<&str>) -> Result<bool, String> {
    let latest = version_segments(latest)?;
    let older = |installed: &str| -> Result<bool, String> {
        let installed = version_segments(installed)?;
        let length = latest.len().max(installed.len());
        Ok((0..length)
            .find_map(|i| {
                let current = installed.get(i).copied().unwrap_or(0);
                let target = latest.get(i).copied().unwrap_or(0);
                (current != target).then_some(current < target)
            })
            .unwrap_or(false))
    };
    if older(binary)? {
        return Ok(true);
    }
    match shared {
        Some(version) => older(version),
        None => Ok(false),
    }
}

pub fn run(args: &[String]) -> i32 {
    if args.first().map(String::as_str) != Some("--check")
        || args.iter().filter(|arg| arg.as_str() == "--check").count() != 1
    {
        eprintln!("usage: foundation update --check [--hermes-home DIR]");
        return 2;
    }
    let home = match hermes_home(&args[1..]) {
        Ok(home) => home,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    let latest = match resolve_latest_tag() {
        Ok(version) => version,
        Err(error) => {
            eprintln!("could not determine latest version: {error}");
            return 1;
        }
    };
    let version_path = home.join("foundation").join("VERSION");
    let shared = match fs::read_to_string(&version_path) {
        Ok(version) => Some(version.trim().to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            eprintln!("could not read {}: {error}", version_path.display());
            return 1;
        }
    };
    println!("Latest version: {latest}");
    println!("Binary version: {BINARY_VERSION}");
    match &shared {
        Some(version) => println!("Shared tree version: {version}"),
        None => println!("Shared tree version: unavailable (VERSION file not found)"),
    }
    if let Some(version) = &shared {
        if version != BINARY_VERSION {
            eprintln!("Warning: binary version {BINARY_VERSION} does not match shared tree version {version}");
        }
    }
    match update_available(&latest, BINARY_VERSION, shared.as_deref()) {
        Ok(true) => println!("Update available"),
        Ok(false) => println!("Already up to date"),
        Err(error) => {
            eprintln!("could not compare installed versions: {error}");
            return 1;
        }
    }
    0
}
