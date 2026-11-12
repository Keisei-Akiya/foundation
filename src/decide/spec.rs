use serde_json::{Map, Value};
use std::io::Read;

const MAX_SPEC: u64 = 4 * 1024 * 1024;

pub(super) fn read_spec(path: &str) -> Result<Value, String> {
    let mut raw = String::new();
    let result = if path == "-" {
        std::io::stdin().take(MAX_SPEC + 1).read_to_string(&mut raw)
    } else {
        std::fs::File::open(path).and_then(|f| f.take(MAX_SPEC + 1).read_to_string(&mut raw))
    };
    result.map_err(|e| format!("cannot read spec: {e}"))?;
    if raw.len() as u64 > MAX_SPEC {
        return Err("spec is too large".into());
    }
    serde_json::from_str(&raw).map_err(|e| format!("spec is not valid JSON: {e}"))
}

fn str_field<'a>(obj: &'a Map<String, Value>, key: &str, ctx: &str) -> Result<&'a str, String> {
    obj.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{ctx}: `{key}` must be a non-empty string"))
}

/// 共通 spec:
/// { "input": <string|json>, "questions": [
///   {"name","type":"predicate|choice|score","instructions",
///    "choices":[{"value","description"}], "levels":[{"label","description"}]} ] }
pub(super) fn validate(spec: &Value) -> Result<(&Value, &Vec<Value>), String> {
    let obj = spec.as_object().ok_or("spec must be a JSON object")?;
    let input = obj.get("input").ok_or("spec: `input` is required")?;
    if input.is_null() || input.as_str().is_some_and(|s| s.trim().is_empty()) {
        return Err("spec: `input` must not be empty".into());
    }
    let questions = obj
        .get("questions")
        .and_then(Value::as_array)
        .filter(|q| !q.is_empty())
        .ok_or("spec: `questions` must be a non-empty array")?;
    let mut names = std::collections::HashSet::new();
    for (i, q) in questions.iter().enumerate() {
        let ctx = format!("questions[{i}]");
        let q = q.as_object().ok_or(format!("{ctx}: must be an object"))?;
        let name = str_field(q, "name", &ctx)?;
        if !names.insert(name) {
            return Err(format!("{ctx}: duplicate name `{name}`"));
        }
        str_field(q, "instructions", &ctx)?;
        match str_field(q, "type", &ctx)? {
            "predicate" => {}
            "choice" => {
                let choices = q
                    .get("choices")
                    .and_then(Value::as_array)
                    .filter(|c| c.len() >= 2)
                    .ok_or(format!("{ctx}: `choices` needs at least 2 items"))?;
                let mut values = std::collections::HashSet::new();
                for c in choices {
                    let c = c
                        .as_object()
                        .ok_or(format!("{ctx}: choice must be an object"))?;
                    let value = str_field(c, "value", &ctx)?;
                    if !values.insert(value) {
                        return Err(format!("{ctx}: duplicate choice value `{value}`"));
                    }
                    str_field(c, "description", &ctx)?;
                }
            }
            "score" => {
                let levels = q
                    .get("levels")
                    .and_then(Value::as_array)
                    .filter(|c| c.len() >= 2)
                    .ok_or(format!("{ctx}: `levels` needs at least 2 items"))?;
                for l in levels {
                    let l = l
                        .as_object()
                        .ok_or(format!("{ctx}: level must be an object"))?;
                    str_field(l, "label", &ctx)?;
                }
            }
            other => return Err(format!("{ctx}: unknown type `{other}`")),
        }
    }
    Ok((input, questions))
}
