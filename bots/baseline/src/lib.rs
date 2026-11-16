//! libaw's baseline policy, with libaw's engine compiled into the bot: a bot can bring its own
//! engine instead of asking the host. It sees only its own view, as every bot does.

fn pick(_seat: u32) -> i32 {
    let Ok(s) = postcard::from_bytes::<libaw::State>(&arena_bot::view_bytes()) else {
        return arena_bot::END_TURN;
    };
    let a = libaw::baseline::choose(&s);
    // The host's legal list is libaw's legal_actions on the same view, so the index matches.
    libaw::actions::legal_actions(&s).iter().position(|l| *l == a).map_or(arena_bot::END_TURN, |i| i as i32)
}
arena_bot::bot!(pick);
