mod args;
mod format;
mod score;
mod wiki;
use args::{parse, USAGE};
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
    let domain_scores = match score::score_domains(&opts.question, &domains, opts.timeout) {
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
    let scores = match score::score(&opts.question, &candidates, opts.timeout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("foundation wiki-search: {e}");
            return 1;
        }
    };
    let result = format::select(&candidates, &scores, opts.threshold, opts.max_results);
    print!("{}", format::render(&result));
    0
}
