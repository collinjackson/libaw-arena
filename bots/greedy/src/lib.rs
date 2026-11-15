//! A greedy bot: attack if it can, else capture, else build, else move toward the nearest enemy,
//! else end the turn. It reads only its input and the engine's legal list.
use arena_bot::serde_json::{json, Value};

fn tile(v: &Value) -> Option<(i64, i64)> {
    Some((v[0].as_i64()?, v[1].as_i64()?))
}

fn act(input: &Value) -> Value {
    let me = input["seat"].as_u64().unwrap_or(0);
    let enemies: Vec<(i64, i64)> = input["state"]["units"].as_object().map(|us| {
        us.values().filter(|u| u["owner"].as_u64() != Some(me)).filter_map(|u| Some((u["x"].as_i64()?, u["y"].as_i64()?))).collect()
    }).unwrap_or_default();
    let near = |t: (i64, i64)| enemies.iter().map(|e| (e.0 - t.0).abs() + (e.1 - t.1).abs()).min().unwrap_or(0);
    let legal = arena_bot::ask(&json!({"q": "legal_actions"}));
    let score = |a: &Value| -> i64 {
        match (a["type"].as_str(), a["then"]["type"].as_str()) {
            (Some("move"), Some("attack")) => 1000,
            (Some("move"), Some("capture")) => 900,
            (Some("build"), _) => 800,
            (Some("move"), Some("wait")) => {
                let path = a["path"].as_array().cloned().unwrap_or_default();
                let (from, to) = (path.first().and_then(tile), path.last().and_then(tile));
                match (from, to) {
                    // Only moves that get closer; staying put isn't worth an action.
                    (Some(f), Some(t)) if near(t) < near(f) => 500 - near(t),
                    _ => -1,
                }
            }
            _ => -1,
        }
    };
    legal.as_array().and_then(|l| l.iter().filter(|a| score(a) >= 0).max_by_key(|a| score(a)).cloned())
        .unwrap_or_else(|| json!({"type": "end_turn"}))
}
arena_bot::bot!(act);
