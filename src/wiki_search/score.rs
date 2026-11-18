use super::wiki::Page;
use crate::decide;
use serde_json::{json, Value};
pub const BATCH: usize = 50;
pub fn select(answer: &Value) -> Result<f64, String> {
    let invalid = |m: &str| format!("invalid Jev response: {m}");
    let p = answer
        .get("probabilities")
        .and_then(Value::as_object)
        .filter(|m| !m.is_empty())
        .ok_or_else(|| invalid("probabilities must be a nonempty dictionary"))?;
    let legend = answer
        .get("legend")
        .and_then(Value::as_object)
        .filter(|m| !m.is_empty())
        .ok_or_else(|| invalid("legend must be a nonempty dictionary"))?;
    for (k, v) in p {
        if k.parse::<u64>().is_err() {
            return Err(invalid("probability keys must be integer indices"));
        }
        let n = v
            .as_f64()
            .ok_or_else(|| invalid("probabilities must be numbers between 0 and 1"))?;
        if !n.is_finite() || !(0.0..=1.0).contains(&n) {
            return Err(invalid("probabilities must be between 0 and 1"));
        }
    }
    let mut by_label = std::collections::HashMap::new();
    for (k, label) in legend {
        if k.parse::<u64>().is_err() {
            return Err(invalid("legend keys must be integer indices"));
        }
        let label = label
            .as_str()
            .ok_or_else(|| invalid("legend must map integer indices to labels"))?
            .to_lowercase();
        if matches!(label.as_str(), "none" | "low" | "high") {
            let value = p
                .get(k)
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid("missing probability for legend label"))?;
            if by_label.insert(label, value).is_some() {
                return Err(invalid("duplicate legend label"));
            }
        }
    }
    if by_label.len() != 3 {
        return Err(invalid(
            "legend must identify none, low, and high probabilities",
        ));
    }
    Ok(by_label["low"] + by_label["high"])
}
#[derive(Clone)]
pub struct DomainProbability {
    pub domain: String,
    pub yes: f64,
}
pub fn score_domains(
    question: &str,
    domains: &[(String, String)],
    timeout: u64,
) -> Result<(Vec<DomainProbability>, Value), String> {
    let questions: Vec<_> = domains.iter().map(|(name, desc)| json!({"name":format!("domain:{name}"),"type":"score","instructions":format!("判定この検索質問がドメインに該当するか。ドメイン: {name}\n説明: {desc}"),"levels":[{"label":"yes"},{"label":"no"}]})).collect();
    let spec = json!({"input":question,"questions":questions});
    let (raw, usage) = decide::communicate(&spec, "jev-latest", timeout)
        .map_err(|e| format!("foundation decide failed: {e}"))?;
    let result: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("invalid Jev response: output is not JSON: {e}"))?;
    let answers = result
        .get("answers")
        .and_then(Value::as_object)
        .ok_or("invalid Jev response: answers must be a dictionary")?;
    let probabilities = domains
        .iter()
        .map(|(name, _)| {
            let key = format!("domain:{name}");
            let ans = answers.get(&key).ok_or_else(|| {
                format!("invalid Jev response: missing answer for question {key}")
            })?;
            let p = ans
                .get("probabilities")
                .and_then(Value::as_object)
                .ok_or("invalid Jev response: probabilities must be a dictionary")?;
            let legend = ans
                .get("legend")
                .and_then(Value::as_object)
                .ok_or("invalid Jev response: legend must be a dictionary")?;
            let yes_key = legend
                .iter()
                .find(|(_, v)| v.as_str().is_some_and(|s| s.eq_ignore_ascii_case("yes")))
                .map(|(k, _)| k)
                .ok_or("invalid Jev response: legend must identify yes")?;
            let yes = p
                .get(yes_key)
                .and_then(Value::as_f64)
                .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
                .ok_or("invalid Jev response: missing valid yes probability")?;
            Ok(DomainProbability {
                domain: name.clone(),
                yes,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((probabilities, usage))
}
pub fn candidates(
    pages: &[Page],
    scores: &[DomainProbability],
    threshold: f64,
    max: usize,
) -> Vec<Page> {
    let mut ranking = scores.to_vec();
    ranking.sort_by(|a, b| {
        b.yes
            .total_cmp(&a.yes)
            .then_with(|| a.domain.cmp(&b.domain))
    });
    let mut picked: Vec<_> = ranking
        .iter()
        .filter(|s| s.yes >= threshold)
        .take(max)
        .map(|s| s.domain.as_str())
        .collect();
    if picked.is_empty() {
        picked.extend(ranking.first().map(|s| s.domain.as_str()));
    }
    pages
        .iter()
        .filter(|p| p.domains.is_empty() || p.domains.iter().any(|d| picked.contains(&d.as_str())))
        .cloned()
        .collect()
}
pub fn score(question: &str, pages: &[Page], timeout: u64) -> Result<(Vec<f64>, Value), String> {
    let mut all = Vec::new();
    let mut usage = Value::Object(Default::default());
    for batch in pages.chunks(BATCH) {
        let questions:Vec<_>=batch.iter().map(|p|json!({"name":format!("page:{}",p.path),"type":"score","instructions":format!("このページを選ぶべきか採点する。\nタイトル: {}\n要約: {}",p.title,p.summary),"levels":[{"label":"none"},{"label":"low"},{"label":"high"}]})).collect();
        let spec = json!({"input":question,"questions":questions});
        let (raw, batch_usage) = decide::communicate(&spec, "jev-latest", timeout)
            .map_err(|e| format!("foundation decide failed: {e}"))?;
        sum_usage(&mut usage, &batch_usage);
        let result: Value = serde_json::from_str(&raw)
            .map_err(|e| format!("invalid Jev response: output is not JSON: {e}"))?;
        let answers = result
            .get("answers")
            .and_then(Value::as_object)
            .ok_or_else(|| "invalid Jev response: answers must be a dictionary".to_string())?;
        for page in batch {
            let name = format!("page:{}", page.path);
            let ans = answers.get(&name).ok_or_else(|| {
                format!("invalid Jev response: missing answer for question {name}")
            })?;
            all.push(select(ans)?)
        }
    }
    Ok((all, usage))
}
pub(crate) fn sum_usage(total: &mut Value, next: &Value) {
    if let (Some(dst), Some(src)) = (total.as_object_mut(), next.as_object()) {
        for (key, value) in src {
            if let Some(n) = value.as_u64() {
                let current = dst.get(key).and_then(Value::as_u64).unwrap_or(0);
                dst.insert(key.clone(), json!(current + n));
            }
        }
    }
}
