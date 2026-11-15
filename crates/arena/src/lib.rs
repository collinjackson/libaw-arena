//! libaw-arena: sandboxed bot-vs-bot games on libaw's engine (README.md, docs/bot-abi.md).
pub mod bot;
pub mod referee;
pub mod page;
pub mod storyboard;

use libaw::State;
use serde_json::Value;

/// A starting position from a file: an AWBW-style turn file (libaw's web/states) or a libaw state.
pub fn load_state(text: &str) -> Result<State, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if v.get("gameState").is_some() {
        return Ok(libaw::awbw::state_from(&v));
    }
    serde_json::from_value(v).map_err(|e| e.to_string())
}

/// Applies a mode from modes.json to a starting position: `fog` (bool), `starting_funds` (each
/// player's funds before any income) and `funds_per_property`. Missing keys leave the map as
/// it is. A map's day 1 already credits the first player's income, so when funds change, the
/// first player gets starting funds plus day-1 income at the mode's rate, the others just the
/// starting funds (their income comes when their turn starts).
pub fn apply_mode(s: &mut libaw::State, mode: &serde_json::Value) {
    if let Some(f) = mode["fog"].as_bool() {
        s.fog = f;
    }
    if let Some(n) = mode["funds_per_property"].as_i64() {
        s.funds_per_property = n;
    }
    if mode["starting_funds"].is_i64() || mode["funds_per_property"].is_i64() {
        let start = mode["starting_funds"].as_i64();
        let first = s.players.iter().min_by_key(|p| p.order).map(|p| p.id);
        let fpp = s.funds_per_property;
        let income: std::collections::BTreeMap<_, i64> = s.players.iter().map(|p| {
            let n = s.buildings.values().filter(|b| b.owner == Some(p.id) && libaw::rules::is_income_property(b.terrain)).count() as i64;
            (p.id, n * (fpp + libaw::co::property_fund_increase(&p.co)))
        }).collect();
        for p in &mut s.players {
            let base = start.unwrap_or(0);
            if start.is_some() || Some(p.id) == first {
                p.funds = base + if Some(p.id) == first { income[&p.id] } else { 0 };
            }
        }
    }
}
