//! `foundation decide`: TypeSafe Jev, Foundry Microsoft-Decision, and OpenAI Decisions APIs.
//! 認証情報は環境変数（Microsoft は `az` も）からのみ読み、標準出力・標準エラーには出さない。

mod args;
mod env;
mod http;
mod request;
mod spec;
mod target;

use args::{parse_args, USAGE};
use http::send;

use serde_json::{json, Value};
use spec::read_spec;
use target::target;

pub fn run(args: &[String]) -> i32 {
    let opts = match parse_args(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("foundation decide: {e}\n{USAGE}");
            return 2;
        }
    };
    let spec = match read_spec(&opts.spec) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("foundation decide: {e}");
            return 2;
        }
    };
    let t = match target(&opts, &spec, opts.dry_run) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("foundation decide: {e}");
            return 2;
        }
    };
    if opts.dry_run {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"method":"POST", "url":t.url, "body":t.body}))
                .unwrap()
        );
        return 0;
    }
    match send(&t, opts.timeout) {
        Ok(text) => {
            match serde_json::from_str::<Value>(&text) {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
                Err(_) => println!("{text}"),
            }
            0
        }
        Err(e) => {
            let e = if opts.provider == args::Provider::OpenAiCodex && e.contains("HTTP 401") {
                format!("{e}; access token may be expired; run codex to refresh the sign-in")
            } else {
                e
            };
            eprintln!("foundation decide: {e}");
            1
        }
    }
}

/// Send a generic decision spec using the configured provider and normalize OpenAI answers.
pub(crate) fn communicate(spec: &Value, timeout: u64) -> Result<(String, Value), String> {
    let provider = args::resolve_provider(None)?;
    let target = target::target_for_provider(&provider, spec)?;
    let text = send(&target, timeout)?;
    let is_openai = matches!(
        provider,
        args::Provider::OpenAi | args::Provider::OpenAiCodex
    );
    let parsed = serde_json::from_str::<Value>(&text);
    let usage = parsed
        .as_ref()
        .ok()
        .and_then(|response| response.get("usage"))
        .cloned()
        .unwrap_or(Value::Null);
    let result = if is_openai {
        let response =
            parsed.map_err(|error| format!("OpenAI decision response is not JSON: {error}"))?;
        serde_json::to_string(&normalize_openai(spec, response)?).map_err(|e| e.to_string())?
    } else {
        text
    };
    Ok((result, usage))
}

fn normalize_openai(spec: &Value, mut response: Value) -> Result<Value, String> {
    let questions = spec
        .get("questions")
        .and_then(Value::as_array)
        .ok_or("spec questions must be an array")?;
    let answers = response
        .get("answers")
        .and_then(Value::as_array)
        .ok_or("OpenAI answers must be an array")?;
    let normalized = answers.iter().try_fold(
        serde_json::Map::new(),
        |mut normalized, answer| -> Result<_, String> {
            let (name, value) = normalize_answer(questions, answer)?;
            check_unique_answer(&normalized, name)?;
            normalized.insert(name.to_string(), value);
            Ok(normalized)
        },
    )?;
    if normalized.len() != questions.len() {
        return Err("OpenAI answers do not cover every question".into());
    }
    response["answers"] = Value::Object(normalized);
    Ok(response)
}

fn normalize_answer<'a>(
    questions: &'a [Value],
    answer: &'a Value,
) -> Result<(&'a str, Value), String> {
    let name = answer
        .get("name")
        .and_then(Value::as_str)
        .ok_or("OpenAI answer is missing name")?;
    let question = questions
        .iter()
        .find(|question| question.get("name").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("OpenAI answer {name} has no matching question"))?;
    let kind = question
        .get("type")
        .and_then(Value::as_str)
        .ok_or("question is missing type")?;
    check_answer_type(answer, kind, name)?;
    let value = match kind {
        "predicate" => normalize_predicate(answer, name)?,
        "choice" => normalize_choice(answer, name)?,
        "score" => normalize_score(question, answer, name)?,
        other => return Err(format!("unsupported OpenAI answer type {other}")),
    };
    Ok((name, value))
}

fn check_answer_type(answer: &Value, kind: &str, name: &str) -> Result<(), String> {
    if answer.get("type").and_then(Value::as_str) == Some(kind) {
        Ok(())
    } else {
        Err(format!(
            "OpenAI answer {name} type does not match its question"
        ))
    }
}

fn check_unique_answer(
    normalized: &serde_json::Map<String, Value>,
    name: &str,
) -> Result<(), String> {
    if normalized.contains_key(name) {
        Err(format!("duplicate OpenAI answer name {name}"))
    } else {
        Ok(())
    }
}

fn normalize_predicate(answer: &Value, name: &str) -> Result<Value, String> {
    let probability = answer
        .get("probability")
        .filter(|value| value.as_f64().is_some())
        .cloned()
        .ok_or_else(|| format!("OpenAI predicate {name} is missing numeric probability"))?;
    Ok(serde_json::json!({"noul": probability}))
}

fn normalize_choice(answer: &Value, name: &str) -> Result<Value, String> {
    let choice = answer
        .get("choice")
        .filter(|value| value.is_string())
        .cloned()
        .ok_or_else(|| format!("OpenAI choice {name} is missing choice"))?;
    let values = answer
        .get("probabilities")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("OpenAI choice {name} probabilities must be an array"))?;
    let probabilities = values.iter().try_fold(
        serde_json::Map::new(),
        |mut probabilities, probability| -> Result<_, String> {
            let label = probability
                .get("value")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("OpenAI choice {name} probability is missing value"))?;
            let value = probability
                .get("probability")
                .filter(|value| value.as_f64().is_some())
                .cloned()
                .ok_or_else(|| {
                    format!("OpenAI choice {name} probability is missing numeric probability")
                })?;
            check_unique_probability(&probabilities, label, name)?;
            probabilities.insert(label.to_string(), value);
            Ok(probabilities)
        },
    )?;
    Ok(serde_json::json!({"choice": choice, "probabilities": probabilities}))
}

fn check_unique_probability(
    probabilities: &serde_json::Map<String, Value>,
    label: &str,
    name: &str,
) -> Result<(), String> {
    if probabilities.contains_key(label) {
        Err(format!(
            "OpenAI choice {name} has duplicate probability value"
        ))
    } else {
        Ok(())
    }
}

fn normalize_score(question: &Value, answer: &Value, name: &str) -> Result<Value, String> {
    let levels = question
        .get("levels")
        .and_then(Value::as_array)
        .ok_or("score question levels must be an array")?;
    let values = answer
        .get("probabilities")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("OpenAI score {name} probabilities must be an array"))?;
    check_score_probability_count(levels, values, name)?;
    let (probabilities, legend) = levels.iter().zip(values).enumerate().try_fold(
        (serde_json::Map::new(), serde_json::Map::new()),
        |(mut probabilities, mut legend), (index, (level, probability))| -> Result<_, String> {
            let label = level
                .get("label")
                .and_then(Value::as_str)
                .ok_or("score level is missing label")?;
            check_score_probability_order(probability, label, index, name)?;
            let value = probability
                .get("probability")
                .filter(|value| value.as_f64().is_some())
                .cloned()
                .ok_or_else(|| {
                    format!("OpenAI score {name} probability is missing numeric probability")
                })?;
            let key = index.to_string();
            probabilities.insert(key.clone(), value);
            legend.insert(key, Value::String(label.to_string()));
            Ok((probabilities, legend))
        },
    )?;
    Ok(serde_json::json!({"probabilities": probabilities, "legend": legend}))
}

fn check_score_probability_count(
    levels: &[Value],
    probabilities: &[Value],
    name: &str,
) -> Result<(), String> {
    if levels.len() == probabilities.len() {
        Ok(())
    } else {
        Err(format!(
            "OpenAI score {name} probability count does not match levels"
        ))
    }
}

fn check_score_probability_order(
    probability: &Value,
    label: &str,
    index: usize,
    name: &str,
) -> Result<(), String> {
    if probability.get("label").and_then(Value::as_str) == Some(label)
        && probability.get("value").and_then(Value::as_u64) == Some(index as u64)
    {
        Ok(())
    } else {
        Err(format!(
            "OpenAI score {name} probability order does not match requested levels"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_openai;
    use serde_json::json;

    #[test]
    fn normalizes_predicate_choice_and_ordered_score_levels() {
        let spec = json!({"questions":[
            {"name":"p","type":"predicate"},
            {"name":"c","type":"choice"},
            {"name":"s","type":"score","levels":[{"label":"none"},{"label":"low"},{"label":"high"}]}
        ]});
        let response = json!({"answers":[
            {"name":"p","type":"predicate","probability":0.75},
            {"name":"c","type":"choice","choice":"approve","probabilities":[{"value":"approve","probability":0.8},{"value":"none","probability":0.2}]},
            {"name":"s","type":"score","probabilities":[{"value":0,"label":"none","probability":0.1},{"value":1,"label":"low","probability":0.3},{"value":2,"label":"high","probability":0.6}]}
        ]});
        let got = normalize_openai(&spec, response).unwrap();
        assert_eq!(got["answers"]["p"]["noul"], 0.75);
        assert_eq!(got["answers"]["c"]["choice"], "approve");
        assert_eq!(got["answers"]["c"]["probabilities"]["approve"], 0.8);
        assert_eq!(got["answers"]["s"]["probabilities"]["0"], 0.1);
        assert_eq!(got["answers"]["s"]["probabilities"]["1"], 0.3);
        assert_eq!(got["answers"]["s"]["probabilities"]["2"], 0.6);
        assert_eq!(got["answers"]["s"]["legend"]["2"], "high");
    }

    #[test]
    fn rejects_ambiguous_malformed_duplicate_and_unsupported_answers() {
        let pred = json!({"questions":[{"name":"p","type":"predicate"}]});
        for response in [
            json!({"answers":[]}),
            json!({"answers":[{"name":"p","type":"predicate"}]}),
            json!({"answers":[{"name":"p","type":"predicate","probability":true}]}),
            json!({"answers":[{"name":"p","type":"predicate","probability":0.1},{"name":"p","type":"predicate","probability":0.2}]}),
            json!({"answers":[{"name":"p","type":"categorical","probability":0.1}]}),
        ] {
            assert!(normalize_openai(&pred, response).is_err());
        }
        let score = json!({"questions":[{"name":"s","type":"score","levels":[{"label":"yes"},{"label":"no"}]}]});
        assert!(normalize_openai(&score, json!({"answers":[{"name":"s","type":"score","probabilities":[{"value":1,"label":"yes","probability":0.5},{"value":0,"label":"no","probability":0.5}]}]})).is_err());
    }
}
