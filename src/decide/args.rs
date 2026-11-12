pub(super) const USAGE: &str = "usage: foundation decide [--provider typesafe] [--spec <file|->] [--model <name>] [--dry-run] [--timeout <secs>]";

#[derive(Debug, PartialEq)]
pub enum Provider {
    TypeSafe,
}

pub(super) struct Options {
    pub(super) provider: Provider,
    pub(super) spec: String,
    pub(super) model: Option<String>,
    pub(super) dry_run: bool,
    pub(super) timeout: u64,
}

pub(super) fn parse_args(args: &[String]) -> Result<Options, String> {
    let provider = Provider::TypeSafe;
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
                let value = value("--provider")?;
                if value != "typesafe" {
                    return Err(format!("unknown provider: {value}"));
                }
            }
            "--spec" => spec = value("--spec")?,
            "--model" => {
                let value = value("--model")?;
                if value != "jev-latest" {
                    return Err("--model supports only jev-latest".into());
                }
                model = Some(value);
            }
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
        provider,
        spec,
        model,
        dry_run,
        timeout,
    })
}
