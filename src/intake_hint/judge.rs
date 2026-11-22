//! Jev の応答から hint か「人間へ戻す」かを決める純粋な判定。ADR 0003 の確定仕様に従う。

use serde_json::{json, Map, Value};

/// この確率以上なら、そのフラグが立つ。
const FLAG_AT: f64 = 0.5;
/// 指示（`new_task`、`standing_rule`）がこの帯に入ったら低信頼として人間へ戻す（両端を含む）。
const LOW: f64 = 0.35;
const HIGH: f64 = 0.65;

const PREDICATES: [&str; 4] = ["proposal", "new_task", "discuss", "standing_rule"];
const INSTRUCTIONS: [&str; 2] = ["new_task", "standing_rule"];
const REPLIES: [&str; 4] = ["approve", "revise", "decline", "none"];

fn human(reason: &str, probabilities: Option<Map<String, Value>>) -> Value {
    match probabilities {
        Some(p) => json!({"route": "human", "reason": reason, "probabilities": p}),
        None => json!({"route": "human", "reason": reason}),
    }
}

/// `foundation decide` の応答本文を受け取り、結果を返す。
/// 判断できない入力（JSON でない、フラグの欠け、範囲外の確率）は `undecidable` とする。
pub(super) fn judge(response: &str) -> Value {
    let Ok(parsed) = serde_json::from_str::<Value>(response) else {
        return human("undecidable", None);
    };
    let Some(answers) = parsed.get("answers").and_then(Value::as_object) else {
        return human("undecidable", None);
    };
    let mut probabilities = Map::new();
    for name in PREDICATES {
        let p = answers
            .get(name)
            .and_then(|a| a.get("noul"))
            .and_then(Value::as_f64)
            .filter(|p| (0.0..=1.0).contains(p));
        let Some(p) = p else {
            return human("undecidable", None);
        };
        probabilities.insert(name.into(), json!(p));
    }
    let p = |name: &str| probabilities[name].as_f64().unwrap_or_default();

    if INSTRUCTIONS.iter().any(|n| (LOW..=HIGH).contains(&p(n))) {
        return human("low_confidence", Some(probabilities));
    }

    let flags: Map<String, Value> = PREDICATES
        .iter()
        .map(|n| ((*n).to_string(), json!(p(n) >= FLAG_AT)))
        .collect();
    // 恒久ルールは、同時に `new_task` が立っても優先する。
    let instruction = ["standing_rule", "new_task"]
        .into_iter()
        .find(|n| flags[*n] == json!(true));
    let reply = answers
        .get("reply")
        .and_then(|a| a.get("choice"))
        .and_then(Value::as_str)
        .filter(|c| REPLIES.contains(c));
    json!({
        "route": "hint",
        "flags": flags,
        "instruction": instruction,
        "reply": reply,
        "probabilities": probabilities,
    })
}
