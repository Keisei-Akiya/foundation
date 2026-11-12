use super::spec::validate;
use serde_json::{json, Map, Value};

pub fn build_typesafe(spec: &Value, model: &str) -> Result<Value, String> {
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
                let criteria: Vec<Value> = q["levels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(
                        |l| match l["description"].as_str().filter(|d| !d.trim().is_empty()) {
                            Some(d) => {
                                json!(format!("{}: {d}", l["label"].as_str().unwrap_or_default()))
                            }
                            None => l["label"].clone(),
                        },
                    )
                    .collect();
                m.insert("criteria".into(), Value::Array(criteria));
            }
        }
        out.insert(name.to_string(), Value::Object(m));
    }
    Ok(json!({"model": model, "state": input, "questions": out}))
}
