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
        .map(|tag| -> Result<String, String> {
            version_segments(tag)?;
            Ok(tag.strip_prefix('v').unwrap_or(tag).to_string())
        })
        .transpose()?
        .ok_or_else(|| "release API response is missing a string tag_name".to_string())
}

fn version_segments(version: &str) -> Result<Vec<u64>, String> {
    let invalid = || format!("invalid version: {version}");
    let version = version.strip_prefix('v').unwrap_or(version);
    let mut parts = Vec::new();
    for part in version.split('.') {
        if part.is_empty() || !part.chars().all(|character| character.is_ascii_digit()) {
            return Err(invalid());
        }
        parts.push(part.parse().map_err(|_| invalid())?);
    }
    if parts.len() != 3 {
        return Err(invalid());
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

fn parse_args(args: &[String]) -> Result<(bool, Option<String>, PathBuf), String> {
    let check = args.first().map(String::as_str) == Some("--check");
    let mut version = None;
    let mut home = None;
    let mut iter = args.iter();
    if check {
        iter.next();
    }
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--version" if !check => {
                let value = iter
                    .next()
                    .ok_or_else(|| "missing value for --version".to_string())?;
                let parts = version_segments(value)?;
                if parts.len() != 3 || value.starts_with('v') {
                    return Err(format!("invalid version: {value}"));
                }
                version = Some(value.clone());
            }
            "--hermes-home" => {
                home =
                    Some(PathBuf::from(iter.next().ok_or_else(|| {
                        "missing value for --hermes-home".to_string()
                    })?))
            }
            _ => return Err(format!("unknown update argument: {arg}")),
        }
    }
    let home = match home {
        Some(path) => path,
        None => hermes_home(&[])?,
    };
    Ok((check, version, home))
}

fn fetch_and_run_installer(version: &str, home: &std::path::Path) -> Result<(), String> {
    let installer_base = env::var("FOUNDATION_INSTALLER_BASE_URL").ok();
    let base = installer_base
        .as_deref()
        .unwrap_or("https://raw.githubusercontent.com/Keisei-Akiya/foundation");
    if let Some(base) = installer_base.as_deref() {
        let parsed = url::Url::parse(base)
            .map_err(|error| format!("invalid installer base URL {base}: {error}"))?;
        let loopback = match parsed.host() {
            Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
            Some(url::Host::Ipv4(address)) => address.is_loopback(),
            Some(url::Host::Ipv6(address)) => address.is_loopback(),
            None => false,
        };
        if !loopback {
            return Err(format!(
                "installer base URL must use a loopback host when overridden: {base}"
            ));
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(format!(
                "installer base URL must not contain credentials: {base}"
            ));
        }
    }
    let url = format!("{}/v{version}/install.sh", base.trim_end_matches('/'));
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();
    let mut response = agent
        .get(&url)
        .header("User-Agent", "foundation-update")
        .call()
        .map_err(|error| format!("installer download failed for {url}: {error}"))?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_vec()
        .map_err(|error| format!("could not read installer from {url}: {error}"))?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "installer download failed for {url}: HTTP {status}: {}",
            String::from_utf8_lossy(&body).trim()
        ));
    }
    if String::from_utf8_lossy(&body).trim().is_empty() {
        return Err(format!(
            "installer download returned an empty body from {url}"
        ));
    }
    let temp = env::temp_dir().join(format!(
        "foundation-update-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir(&temp)
        .map_err(|error| format!("could not create temporary installer directory: {error}"))?;
    let script = temp.join("install.sh");
    let result = (|| {
        fs::write(&script, body)
            .map_err(|error| format!("could not save downloaded installer: {error}"))?;
        let output = std::process::Command::new("sh")
            .arg(&script)
            .args(["--version", version, "--hermes-home"])
            .arg(home)
            .output()
            .map_err(|error| format!("could not execute installer: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "installer exited with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        print!("{}", String::from_utf8_lossy(&output.stdout));
        Ok(())
    })();
    let _ = fs::remove_dir_all(temp);
    result
}

pub fn run(args: &[String]) -> i32 {
    let (check, requested, home) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("usage: foundation update [--check | --version X.Y.Z] [--hermes-home DIR]");
            return 2;
        }
    };
    let latest = if check || requested.is_none() {
        match resolve_latest_tag() {
            Ok(version) => version,
            Err(error) => {
                eprintln!("could not determine latest version: {error}");
                return 1;
            }
        }
    } else {
        requested.clone().unwrap()
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
    if check {
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
        return match update_available(&latest, BINARY_VERSION, shared.as_deref()) {
            Ok(true) => {
                println!("Update available");
                0
            }
            Ok(false) => {
                println!("Already up to date");
                0
            }
            Err(error) => {
                eprintln!("could not compare installed versions: {error}");
                1
            }
        };
    }
    let explicit_version = requested.is_some();
    let target = requested.unwrap_or(latest);
    let matches = shared.as_deref() == Some(BINARY_VERSION);
    match update_available(&target, BINARY_VERSION, shared.as_deref()) {
        Ok(false)
            if (explicit_version && BINARY_VERSION == target && matches)
                || (!explicit_version && matches) =>
        {
            println!("最新です (version {target})");
            return 0;
        }
        Err(error) => {
            eprintln!("could not compare installed versions: {error}");
            return 1;
        }
        _ => {}
    }
    if let Err(error) = fetch_and_run_installer(&target, &home) {
        eprintln!("{error}");
        return 1;
    }
    println!("更新が完了しました。gateway の再起動が必要です。");
    0
}
