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
    json!({
        "kind": "api_usage",
        "session_id": payload.get("session_id").and_then(Value::as_str).unwrap_or(""),
        "api_request_id": payload.get("api_request_id").cloned().unwrap_or(Value::Null),
        "model": payload.get("response_model").or_else(|| payload.get("model")).and_then(Value::as_str).unwrap_or("unknown"),
        "input_tokens": first_count(usage, &["prompt_tokens", "input_tokens"]),
        "output_tokens": first_count(usage, &["completion_tokens", "output_tokens"]),
        "cache_write_tokens": count(usage, "cache_creation_input_tokens"),
        "cache_read_tokens": first_count(usage, &["cache_read_input_tokens", "cached_tokens"]),
    })
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
        let mut cost = 0.0_f64;
        let mut priced = true;
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
            match (input_rate, output_rate) {
                (Some(input_rate), Some(output_rate))
                    if input_rate >= 0.0 && output_rate >= 0.0 =>
                {
                    cost += row_input as f64 * input_rate / 1_000_000.0;
                    cost += row_output as f64 * output_rate / 1_000_000.0;
                }
                _ => priced = false,
            }
        }
        let cost_fields = if priced {
            json!({"cost_status":"calculated", "estimated_cost_usd":(cost * 1_000_000.0).round() / 1_000_000.0})
        } else {
            json!({"cost_status":"unavailable", "cost_unavailable_reason":"unknown_model_price"})
        };
        let mut total = json!({
            "kind":"session_total", "session_id":session, "model":model,
            "input_tokens":input, "output_tokens":output,
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
