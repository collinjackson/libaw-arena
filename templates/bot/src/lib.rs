//! A starting point: attack when it can, else capture, else build, else the first legal action.
use arena_bot::serde_json::{json, Value};

fn act(_input: &Value) -> Value {
    let legal = arena_bot::ask(&json!({"q": "legal_actions"}));
    let legal = legal.as_array().cloned().unwrap_or_default();
    let then = |a: &Value, t: &str| a["then"]["type"] == t;
    legal.iter().find(|a| then(a, "attack"))
        .or_else(|| legal.iter().find(|a| then(a, "capture")))
        .or_else(|| legal.iter().find(|a| a["type"] == "build"))
        .or_else(|| legal.iter().find(|a| a["type"] != "end_turn"))
        .cloned()
        .unwrap_or_else(|| json!({"type": "end_turn"}))
}
arena_bot::bot!(act);
