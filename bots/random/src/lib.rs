//! A random bot: any legal action, picked by a small generator seeded from what it sees, so the
//! same position always gets the same choice (bots have no clock or randomness of their own).

fn pick(_seat: u32) -> i32 {
    let legal = arena_bot::legal();
    // End the turn only now and then, so units get to act.
    let seen = [arena_bot::raw(0, 0), arena_bot::raw(1, 0)].concat();
    let mut x = seen.iter().fold(0x9e37_79b9_7f4a_7c15u64, |h, b| (h ^ *b as u64).wrapping_mul(0x1000_0000_01b3));
    x ^= x >> 29;
    let moves: Vec<usize> = (0..legal.len()).filter(|&i| legal[i].kind != arena_bot::KIND_END_TURN).collect();
    if moves.is_empty() || x % 10 == 0 {
        return arena_bot::END_TURN;
    }
    moves[(x as usize / 10) % moves.len()] as i32
}
arena_bot::bot!(pick);
