fn dotenv_values() -> std::collections::HashMap<String, String> {
    const ALLOWED: &[&str] = &["TYPESAFE_API_KEY", "TYPESAFE_BASE_URL"];
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

pub(super) fn env(name: &str) -> Option<String> {
    env_without_dotenv(name).or_else(|| dotenv_values().remove(name))
}

pub(super) fn auth_env(names: &[&str]) -> Option<(String, String)> {
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
