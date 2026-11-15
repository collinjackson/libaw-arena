//! libaw's baseline policy, with libaw's engine compiled into the bot: a bot can bring its own
//! engine instead of asking the host. It sees only its own view, as every bot does.
use arena_bot::serde_json::{self, json, Value};

fn act(input: &Value) -> Value {
    let Ok(s) = serde_json::from_value::<libaw::State>(input["state"].clone()) else {
        return json!({"type": "end_turn"});
    };
    libaw::awbw::action_to_contract(&libaw::baseline::choose(&s))
}
arena_bot::bot!(act);
