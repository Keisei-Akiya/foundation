mod args;
mod format;
mod score;
mod wiki;
use args::{parse, USAGE};
use serde_json::json;
pub fn run(args: &[String]) -> i32 {
    let opts = match parse(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("foundation wiki-search: {e}\n{USAGE}");
            return 2;
        }
    };
    let pages = match wiki::discover(&opts.wiki) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("foundation wiki-search: {e}");
            return 1;
        }
    };
    let domains = match wiki::schema_domains(&opts.wiki) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("foundation wiki-search: {e}");
            return 1;
        }
    };
    let (domain_scores, domain_usage) =
        match score::score_domains(&opts.question, &domains, opts.timeout) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("foundation wiki-search: {e}");
                return 1;
            }
        };
    let candidates = score::candidates(
        &pages,
        &domain_scores,
        opts.domain_threshold,
        opts.domain_max,
    );
    let (scores, page_usage) = match score::score(&opts.question, &candidates, opts.timeout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("foundation wiki-search: {e}");
            return 1;
        }
    };
    let result = format::select(&candidates, &scores, opts.threshold, opts.max_results);
    print!("{}", format::render(&result));
    let mut usage = domain_usage;
    score::sum_usage(&mut usage, &page_usage);
    let domain_json: serde_json::Map<String, serde_json::Value> = domain_scores
        .iter()
        .map(|d| (d.domain.clone(), json!(d.yes)))
        .collect();
    let mut page_json = serde_json::Map::new();
    for page in &pages {
        let value = candidates
            .iter()
            .position(|candidate| candidate.path == page.path)
            .map(|i| json!(scores[i]))
            .unwrap_or(serde_json::Value::Null);
        page_json.insert(page.path.clone(), value);
    }
    let returned: Vec<_> = result
        .iter()
        .map(|(score, page)| json!({"path":page.path,"score":score}))
        .collect();
    let record = json!({"timestamp": timestamp(), "input":opts.question, "domain_probabilities":domain_json, "page_scores":page_json, "returned_pages":returned, "usage":usage});
    if let Err(error) = append_log(&opts.log_path, &record) {
        eprintln!("foundation wiki-search: warning: unable to append log: {error}");
    }
    0
}
fn append_log(path: &std::path::Path, record: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, record)?;
    file.write_all(b"\n")
}
fn timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    let rem = seconds.rem_euclid(86400);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}
