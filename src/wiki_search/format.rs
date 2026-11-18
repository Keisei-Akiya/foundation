use super::wiki::Page;
pub fn select(pages: &[Page], scores: &[f64], threshold: f64, max: usize) -> Vec<(f64, Page)> {
    let mut ranked: Vec<_> = pages
        .iter()
        .cloned()
        .zip(scores.iter().copied())
        .map(|(p, s)| (s, p))
        .filter(|(s, _)| s.is_finite())
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    let mut selected: Vec<_> = ranked
        .iter()
        .filter(|(s, _)| *s >= threshold)
        .cloned()
        .collect();
    if selected.is_empty() {
        selected = ranked.into_iter().take(1).collect()
    }
    selected.truncate(max);
    selected
}
pub fn render(rows: &[(f64, Page)]) -> String {
    rows.iter()
        .map(|(s, p)| format!("{s:.2}\t{}\t{}\n", p.path, p.title))
        .collect()
}
