//! libaw's baseline policy through the host's BASELINE answer: the same moves as `baseline`,
//! which compiles libaw into the module, at a small part of the cost (the host runs the engine
//! natively). A starting point for a bot that improves on the baseline's suggestion.

fn pick(_seat: u32) -> i32 {
    arena_bot::baseline()
}
arena_bot::bot!(pick);
