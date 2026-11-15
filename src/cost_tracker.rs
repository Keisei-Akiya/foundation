//! Aggregate authoritative Hermes usage counts by session without inventing prices.
use crate::state::StateStore;
use serde_json::{json, Value};

fn count(usage: &Value, key: &str) -> u64 {
    usage.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn first_count(usage: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| usage.get(key).and_then(Value::as_u64))
        .unwrap_or(0)
}

fn usage_record(payload: &Value) -> Value {
    let usage = payload.get("usage").unwrap_or(&Value::Null);
    let raw_input = first_count(usage, &["prompt_tokens", "input_tokens"]);
    let cache_read = first_count(usage, &["cache_read_input_tokens", "cached_tokens"]);
    // OpenAI-style prompt_tokens includes cached_tokens. Anthropic's input_tokens
    // reports uncached input, while cache read/write counts are separate.
    let input_tokens =
        if usage.get("prompt_tokens").is_some() && usage.get("cached_tokens").is_some() {
            raw_input.saturating_sub(cache_read)
        } else {
            raw_input
        };
    json!({
        "kind": "api_usage",
        "session_id": payload.get("session_id").and_then(Value::as_str).unwrap_or(""),
        "api_request_id": payload.get("api_request_id").cloned().unwrap_or(Value::Null),
        "model": payload.get("response_model").or_else(|| payload.get("model")).and_then(Value::as_str).unwrap_or("unknown"),
        "input_tokens": input_tokens,
        "output_tokens": first_count(usage, &["completion_tokens", "output_tokens"]),
        "cache_write_tokens": count(usage, "cache_creation_input_tokens"),
        "cache_read_tokens": cache_read,
    })
}

/// Print the latest per-session totals without exposing prompts or other content.
pub fn report() {
    let Ok(store) = StateStore::from_env() else {
        println!("No session cost records yet.");
        return;
    };
    let mut totals: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
    for row in store.read("cost_tracker").unwrap_or_default() {
        if row.get("kind").and_then(Value::as_str) == Some("session_total") {
            if let Some(session) = row.get("session_id").and_then(Value::as_str) {
                totals.insert(session.to_owned(), row);
            }
        }
    }
    if totals.is_empty() {
        println!("No session cost records yet.");
        return;
    }
    println!("Session\tModel\tInput tokens\tOutput tokens\tEstimated cost (USD)");
    for (session, row) in totals {
        let Some(input) = row.get("input_tokens").and_then(Value::as_u64) else {
            println!("{session}\tinvalid/incomplete record (missing input token count)");
            continue;
        };
        let Some(output) = row.get("output_tokens").and_then(Value::as_u64) else {
            println!("{session}\tinvalid/incomplete record (missing output token count)");
            continue;
        };
        let Some(status) = row.get("cost_status").and_then(Value::as_str) else {
            println!("{session}\tinvalid/incomplete record (missing cost status)");
            continue;
        };
        let models: Vec<&str> = row
            .get("models")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let model = if models.len() > 1 {
            format!("multiple models: {}", models.join(", "))
        } else {
            row.get("model")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned()
        };
        match status {
            "calculated" => {
                let Some(cost) = row.get("estimated_cost_usd").and_then(Value::as_f64) else {
                    println!("{session}\tinvalid/incomplete record (missing estimated cost)");
                    continue;
                };
                if !cost.is_finite() || cost < 0.0 {
                    println!("{session}\tinvalid/incomplete record (invalid estimated cost)");
                    continue;
                }
                println!("{session}\t{model}\t{input}\t{output}\t${cost:.8}");
            }
            "unavailable" => {
                let Some(reason) = row.get("cost_unavailable_reason").and_then(Value::as_str)
                else {
                    println!("{session}\tinvalid/incomplete record (missing unavailable reason)");
                    continue;
                };
                let reason = match reason {
                    "unknown_cache_price" => "No configured cache price for this model",
                    "unknown_model_price" => "No configured price for this model",
                    _ => "Price unavailable (check model-pricing.json)",
                };
                println!("{session}\t{model}\t{input}\t{output}\tunavailable: {reason}");
            }
            _ => println!("{session}\tinvalid/incomplete record (unknown cost status)"),
        }
    }
}

pub fn run(payload: &Value) {
    let event = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let Ok(store) = StateStore::from_env() else {
        return;
    };
    if event == "post_api_request" {
        let row = usage_record(payload);
        let session = row.get("session_id").and_then(Value::as_str).unwrap_or("");
        if session.is_empty() {
            return;
        }
        if let Some(id) = row.get("api_request_id").filter(|v| !v.is_null()) {
            if store
                .read("cost_tracker")
                .unwrap_or_default()
                .iter()
                .any(|r| {
                    r.get("kind").and_then(Value::as_str) == Some("api_usage")
                        && r.get("session_id").and_then(Value::as_str) == Some(session)
                        && r.get("api_request_id") == Some(id)
                })
            {
                return;
            }
        }
        let _ = store.append("cost_tracker", &row);
    } else if event == "post_llm_call" {
        let session = payload
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        if session.is_empty() {
            return;
        }
        let records = store.read("cost_tracker").unwrap_or_default();
        let rows = records.iter().filter(|r| {
            r.get("kind").and_then(Value::as_str) == Some("api_usage")
                && r.get("session_id").and_then(Value::as_str) == Some(session)
        });
        let (mut input, mut output, mut cache_write, mut cache_read) = (0u64, 0u64, 0u64, 0u64);
        let mut model = "unknown";
        let mut models = std::collections::BTreeSet::new();
        let mut cost = 0.0_f64;
        let mut unavailable_reason = None;
        let prices = std::fs::read(store.root().join("model-pricing.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or(Value::Null);
        for row in rows {
            let row_input = count(row, "input_tokens");
            let row_output = count(row, "output_tokens");
            input += row_input;
            output += row_output;
            cache_write += count(row, "cache_write_tokens");
            cache_read += count(row, "cache_read_tokens");
            if let Some(m) = row.get("model").and_then(Value::as_str) {
                model = m;
                models.insert(m);
            }
            let rates = row
                .get("model")
                .and_then(Value::as_str)
                .and_then(|name| prices.get(name));
            let input_rate = rates
                .and_then(|r| r.get("input_usd_per_million"))
                .and_then(Value::as_f64);
            let output_rate = rates
                .and_then(|r| r.get("output_usd_per_million"))
                .and_then(Value::as_f64);
            let cache_write_tokens = count(row, "cache_write_tokens");
            let cache_read_tokens = count(row, "cache_read_tokens");
            let cache_write_rate = rates
                .and_then(|r| r.get("cache_write_usd_per_million"))
                .and_then(Value::as_f64);
            let cache_read_rate = rates
                .and_then(|r| r.get("cache_read_usd_per_million"))
                .and_then(Value::as_f64);
            if !matches!((input_rate, output_rate), (Some(i), Some(o)) if i >= 0.0 && o >= 0.0) {
                unavailable_reason.get_or_insert("unknown_model_price");
            } else if (cache_write_tokens > 0 && !cache_write_rate.is_some_and(|rate| rate >= 0.0))
                || (cache_read_tokens > 0 && !cache_read_rate.is_some_and(|rate| rate >= 0.0))
            {
                unavailable_reason.get_or_insert("unknown_cache_price");
            } else {
                cost += row_input as f64 * input_rate.unwrap() / 1_000_000.0;
                cost += row_output as f64 * output_rate.unwrap() / 1_000_000.0;
                cost += cache_write_tokens as f64 * cache_write_rate.unwrap_or(0.0) / 1_000_000.0;
                cost += cache_read_tokens as f64 * cache_read_rate.unwrap_or(0.0) / 1_000_000.0;
            }
        }
        let cost_fields = if unavailable_reason.is_none() {
            json!({"cost_status":"calculated", "estimated_cost_usd":cost})
        } else {
            json!({"cost_status":"unavailable", "cost_unavailable_reason":unavailable_reason.unwrap_or("unknown_model_price")})
        };
        let mut total = json!({
            "kind":"session_total", "session_id":session, "model":model,
            "models":models, "input_tokens":input, "output_tokens":output,
            "cache_write_tokens":cache_write, "cache_read_tokens":cache_read
        });
        if let (Some(object), Some(fields)) = (total.as_object_mut(), cost_fields.as_object()) {
            object.extend(fields.clone());
        }
        let _ = store.append("cost_tracker", &total);
    }
}

#[cfg(test)]
mod tests {
    use super::usage_record;
    use serde_json::json;

    #[test]
    fn subtracts_cached_tokens_in_prompt_total_to_avoid_double_charging() {
        let row = usage_record(&json!({"session_id":"s1", "model":"m1", "usage":{
            "prompt_tokens":100, "completion_tokens":4, "cached_tokens":30
        }}));
        assert_eq!(row["input_tokens"], 70);
        assert_eq!(row["cache_read_tokens"], 30);
    }

    #[test]
    fn leaves_provider_net_input_tokens_unchanged_when_cache_is_separate() {
        let row = usage_record(&json!({"session_id":"s1", "model":"m1", "usage":{
            "input_tokens":70, "output_tokens":4, "cache_read_input_tokens":30
        }}));
        assert_eq!(row["input_tokens"], 70);
        assert_eq!(row["cache_read_tokens"], 30);
    }

    #[test]
    fn records_usage_without_adding_an_unconfigured_price() {
        let row = usage_record(&json!({"session_id":"s1", "response_model":"m1", "usage":{
            "input_tokens":10, "output_tokens":4,
            "cache_creation_input_tokens":2, "cache_read_input_tokens":3
        }}));
        assert_eq!(row["model"], "m1");
        assert_eq!(row["input_tokens"], 10);
        assert_eq!(row["output_tokens"], 4);
        assert_eq!(row["cache_write_tokens"], 2);
        assert_eq!(row["cache_read_tokens"], 3);
        assert!(row.get("estimated_cost_usd").is_none());
    }
}
