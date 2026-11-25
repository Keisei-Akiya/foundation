pub(super) const USAGE: &str = "usage: foundation decide [--provider typesafe|microsoft] [--spec <file|->] [--model <name>] [--dry-run] [--timeout <secs>]";

#[derive(Debug, PartialEq)]
pub enum Provider {
    TypeSafe,
    Microsoft,
}

pub(super) struct Options {
    pub(super) provider: Provider,
    pub(super) spec: String,
    pub(super) model: Option<String>,
    pub(super) dry_run: bool,
    pub(super) timeout: u64,
}

pub(super) fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut provider = Provider::TypeSafe;
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
                provider = match value("--provider")?.as_str() {
                    "typesafe" => Provider::TypeSafe,
                    "microsoft" => Provider::Microsoft,
                    other => return Err(format!("unknown provider: {other}")),
                };
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
    // --model は --provider より前に書けるので、全引数を読んでから検証する。
    match (&provider, model.as_deref()) {
        (Provider::TypeSafe, Some(m)) if m != "jev-latest" => {
            return Err("--model supports only jev-latest for typesafe".into());
        }
        (Provider::Microsoft, Some(m)) if m.trim().is_empty() => {
            return Err("--model must be a non-empty deployment name".into());
        }
        _ => {}
    }
    Ok(Options {
        provider,
        spec,
        model,
        dry_run,
        timeout,
    })
}
