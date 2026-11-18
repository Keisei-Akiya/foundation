use std::path::PathBuf;
pub const USAGE: &str = "usage: foundation wiki-search --wiki <directory> [--threshold <0..1>] [--max-results <n>] [--domain-threshold <0..1>] [--domain-max <n>] [--timeout <secs>] [--log-path <file>] <rewritten question>";
pub struct Options {
    pub wiki: PathBuf,
    pub threshold: f64,
    pub max_results: usize,
    pub domain_threshold: f64,
    pub domain_max: usize,
    pub timeout: u64,
    pub log_path: PathBuf,
    pub question: String,
}
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut wiki = None;
    let mut threshold = 0.2;
    let mut max_results = 10;
    // Provisional defaults; tune only after retrieval evaluation.
    let mut domain_threshold = 0.1;
    let mut domain_max = 5;
    let mut timeout = 60;
    let default_log = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state")
        })
        .join("foundation/wiki-search.jsonl");
    let mut log_path = default_log;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--wiki" | "--threshold" | "--max-results" | "--domain-threshold" | "--domain-max"
            | "--timeout" | "--log-path" => {
                let key = args[i].as_str();
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| format!("{key} requires a value"))?;
                match key {
                    "--wiki" => wiki = Some(PathBuf::from(val)),
                    "--log-path" => log_path = PathBuf::from(val),
                    "--threshold" => {
                        threshold = val
                            .parse()
                            .map_err(|_| "--threshold must be between 0 and 1")?
                    }
                    "--max-results" => {
                        max_results = val
                            .parse()
                            .map_err(|_| "--max-results must be at least 1")?
                    }
                    "--domain-threshold" => {
                        domain_threshold = val
                            .parse()
                            .map_err(|_| "--domain-threshold must be between 0 and 1")?
                    }
                    "--domain-max" => {
                        domain_max = val.parse().map_err(|_| "--domain-max must be at least 1")?
                    }
                    _ => timeout = val.parse().map_err(|_| "--timeout must be 1..=600")?,
                }
            }
            x if x.starts_with('-') => return Err(format!("unknown argument: {x}")),
            x => rest.push(x.to_string()),
        }
        i += 1;
    }
    if !(0.0..=1.0).contains(&domain_threshold) {
        return Err("--domain-threshold must be between 0 and 1".into());
    }
    if domain_max == 0 {
        return Err("--domain-max must be at least 1".into());
    }
    if !(0.0..=1.0).contains(&threshold) {
        return Err("--threshold must be between 0 and 1".into());
    }
    if max_results == 0 {
        return Err("--max-results must be at least 1".into());
    }
    if !(1..=600).contains(&timeout) {
        return Err("--timeout must be 1..=600".into());
    }
    let question = rest.join(" ");
    if question.trim().is_empty() {
        return Err("rewritten question is required".into());
    }
    Ok(Options {
        wiki: wiki.ok_or("--wiki is required")?,
        threshold,
        max_results,
        domain_threshold,
        domain_max,
        timeout,
        log_path,
        question,
    })
}
