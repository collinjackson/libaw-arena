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
