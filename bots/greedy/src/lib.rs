//! A greedy bot: attack if it can, else capture, else build, else move toward the nearest enemy,
//! else end the turn. It reads only the legal list and the units it can see.
use arena_bot::{Legal, ATTACK, CAPTURE, KIND_BUILD, KIND_MOVE, WAIT};

fn pick(seat: u32) -> i32 {
    let enemies: Vec<(i32, i32)> = arena_bot::units().iter().filter(|u| u.owner != seat).map(|u| (u.x as i32, u.y as i32)).collect();
    let near = |t: (u8, u8)| enemies.iter().map(|e| (e.0 - t.0 as i32).abs() + (e.1 - t.1 as i32).abs()).min().unwrap_or(0);
    let score = |a: &Legal| -> i32 {
        match (a.kind, a.then) {
            (KIND_MOVE, ATTACK) => 1000,
            (KIND_MOVE, CAPTURE) => 900,
            (KIND_BUILD, _) => 800,
            (KIND_MOVE, WAIT) => match (a.from, a.to) {
                // Only moves that get closer; staying put isn't worth an action.
                (Some(f), Some(t)) if near(t) < near(f) => 500 - near(t),
                _ => -1,
            },
            _ => -1,
        }
    };
    let legal = arena_bot::legal();
    // max_by_key keeps the last of the best, as the JSON version did.
    (0..legal.len()).filter(|&i| score(&legal[i]) >= 0).max_by_key(|&i| score(&legal[i]))
        .map_or(arena_bot::END_TURN, |i| i as i32)
}
arena_bot::bot!(pick);
