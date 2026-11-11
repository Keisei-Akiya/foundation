pub(super) const USAGE: &str = "usage: foundation decide --provider <openai|microsoft> [--spec <file|->] [--model <name>] [--dry-run] [--timeout <secs>]";

#[derive(Debug, PartialEq)]
pub enum Provider {
    OpenAi,
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
