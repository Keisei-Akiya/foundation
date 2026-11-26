pub(super) const USAGE: &str = "usage: foundation decide [--provider typesafe|microsoft|openai|openai-codex] [--spec <file|->] [--model <name>] [--dry-run] [--timeout <secs>] (default: FOUNDATION_DECIDE_PROVIDER, else openai-codex)";

#[derive(Debug, PartialEq)]
pub enum Provider {
    TypeSafe,
    Microsoft,
    OpenAi,
    OpenAiCodex,
}

pub(super) struct Options {
    pub(super) provider: Provider,
    pub(super) spec: String,
    pub(super) model: Option<String>,
    pub(super) dry_run: bool,
    pub(super) timeout: u64,
}

fn parse_provider(value: &str) -> Option<Provider> {
    match value {
        "typesafe" => Some(Provider::TypeSafe),
        "microsoft" => Some(Provider::Microsoft),
        "openai" => Some(Provider::OpenAi),
        "openai-codex" => Some(Provider::OpenAiCodex),
        _ => None,
    }
}

pub(crate) fn resolve_provider(explicit: Option<&str>) -> Result<Provider, String> {
    match explicit {
        Some(name) => parse_provider(name).ok_or_else(|| format!("unknown provider: {name}")),
        None => match crate::decide::env::env_without_dotenv("FOUNDATION_DECIDE_PROVIDER") {
            Some(name) => parse_provider(&name).ok_or_else(|| {
                "FOUNDATION_DECIDE_PROVIDER must be one of typesafe|microsoft|openai|openai-codex".into()
            }),
            None => Ok(Provider::OpenAiCodex),
        },
    }
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
                let name = value("--provider")?;
                provider =
                    Some(parse_provider(&name).ok_or_else(|| format!("unknown provider: {name}"))?);
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
    let provider = resolve_provider(provider.as_ref().map(|p| match p {
        Provider::TypeSafe => "typesafe",
        Provider::Microsoft => "microsoft",
        Provider::OpenAi => "openai",
        Provider::OpenAiCodex => "openai-codex",
    }))?;
    // --model は --provider より前に書けるので、全引数を読んでから検証する。
    match (&provider, model.as_deref()) {
        (Provider::TypeSafe, Some(m)) if m != "jev-latest" => {
            return Err("--model supports only jev-latest for typesafe".into());
        }
        (Provider::OpenAi | Provider::OpenAiCodex, Some(m)) if m != "gpt-6-luna" => {
            return Err("--model supports only gpt-6-luna for openai".into());
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
