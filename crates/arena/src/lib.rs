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
/// player's funds on day 1) and `funds_per_property`. Missing keys leave the map as it is.
pub fn apply_mode(s: &mut libaw::State, mode: &serde_json::Value) {
    if let Some(f) = mode["fog"].as_bool() {
        s.fog = f;
    }
    if let Some(n) = mode["starting_funds"].as_i64() {
        for p in &mut s.players {
            p.funds = n;
        }
    }
    if let Some(n) = mode["funds_per_property"].as_i64() {
        s.funds_per_property = n;
    }
}
