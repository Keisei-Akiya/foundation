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
pub fn score(question: &str, pages: &[Page], timeout: u64) -> Result<Vec<f64>, String> {
    let mut all = Vec::new();
    for batch in pages.chunks(BATCH) {
        let questions:Vec<_>=batch.iter().map(|p|json!({"name":format!("page:{}",p.path),"type":"score","instructions":format!("このページを選ぶべきか採点する。\nタイトル: {}\n要約: {}",p.title,p.summary),"levels":[{"label":"none"},{"label":"low"},{"label":"high"}]})).collect();
        let spec = json!({"input":question,"questions":questions});
        let (raw, _) = decide::communicate(&spec, "jev-latest", timeout)
            .map_err(|e| format!("foundation decide failed: {e}"))?;
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
    Ok(all)
}
