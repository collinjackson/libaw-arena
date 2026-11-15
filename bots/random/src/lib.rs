//! A random bot: any legal action, picked by a small generator seeded from the input, so the
//! same position always gets the same choice (bots have no clock or randomness of their own).
use arena_bot::serde_json::{json, Value};

fn act(input: &Value) -> Value {
    let legal = arena_bot::ask(&json!({"q": "legal_actions"}));
    let legal = legal.as_array().cloned().unwrap_or_default();
    if legal.is_empty() {
        return json!({"type": "end_turn"});
    }
    // End the turn only now and then, so units get to act.
    let mut x = input.to_string().bytes().fold(0x9e37_79b9_7f4a_7c15u64, |h, b| (h ^ b as u64).wrapping_mul(0x1000_0000_01b3));
    x ^= x >> 29;
    let moves: Vec<&Value> = legal.iter().filter(|a| a["type"] != "end_turn").collect();
    if moves.is_empty() || x % 10 == 0 {
        return json!({"type": "end_turn"});
    }
    moves[(x as usize / 10) % moves.len()].clone()
}
arena_bot::bot!(act);
