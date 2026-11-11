use super::spec::validate;
use serde_json::{json, Map, Value};

pub fn build_openai(spec: &Value, model: &str) -> Result<Value, String> {
    let (input, questions) = validate(spec)?;
    if questions.len() > 400 {
        return Err("OpenAI Decisions API accepts at most 400 questions per request".into());
    }
    for (i, q) in questions.iter().enumerate() {
        if q["type"] == "score" {
            let n = q["levels"].as_array().map_or(0, Vec::len);
            if !(2..=10).contains(&n) {
                return Err(format!(
                    "questions[{i}]: OpenAI score levels must contain 2..=10 items"
                ));
            }
        }
    }
    let input = match input {
        Value::String(_) => input.clone(),
        Value::Object(_) | Value::Array(_) => json!([{
            "role": "user",
            "content": [{"type": "input_json", "data": input}],
        }]),
        _ => return Err("OpenAI input JSON must be an object or array".into()),
    };
    Ok(json!({"model": model, "input": input, "questions": questions}))
}

pub fn build_microsoft(spec: &Value, deployment: &str) -> Result<Value, String> {
    let (input, questions) = validate(spec)?;
    let mut out = Map::new();
    for q in questions {
        let name = q["name"].as_str().unwrap_or_default();
        let mut m = Map::new();
        m.insert("instructions".into(), q["instructions"].clone());
        match q["type"].as_str().unwrap_or_default() {
            "predicate" => {
                m.insert("type".into(), "noul".into());
            }
            "choice" => {
                m.insert("type".into(), "choice".into());
                let criteria: Map<String, Value> = q["choices"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| {
                        (
                            c["value"].as_str().unwrap_or_default().to_string(),
                            c["description"].clone(),
                        )
                    })
                    .collect();
                m.insert("criteria".into(), Value::Object(criteria));
            }
            _ => {
                m.insert("type".into(), "score".into());
                let labels: Vec<Value> = q["levels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|l| {
                        let label = l["label"].as_str().unwrap_or_default();
                        match l["description"].as_str().filter(|d| !d.trim().is_empty()) {
                            Some(description) => json!(format!("{label}: {description}")),
                            None => json!(label),
                        }
                    })
                    .collect();
                m.insert("criteria".into(), Value::Array(labels));
            }
        }
        out.insert(name.to_string(), Value::Object(m));
    }
    Ok(json!({"model": deployment, "state": input, "questions": out}))
}
