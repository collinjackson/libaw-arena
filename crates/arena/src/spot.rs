//! Spot checks for a private bot (docs/proofs.md): after a game, a drand value picks some of
//! one seat's decisions, and the bot's owner proves just those. A decision is the seat's view
//! and the index it picked in that view's legal list; every decision runs on a fresh instance
//! (bot.rs), so each one can be proven on its own.

use crate::referee::{action_for_mover, Dice, FORMAT};
use libaw::actions::Action;
use libaw::{fog, PlayerId, State};
use serde_json::Value;

pub struct Decision {
    /// The move's place in the record.
    pub move_index: usize,
    pub view: State,
    /// The bot's pick: its index in `arena_abi::legal(&view)`, or -1 for ending the turn.
    pub pick: i32,
}

/// Every decision `seat`'s bot made in a game record, replayed from its start. Moves the referee
/// made for the bot (a strike, the action limit) aren't the bot's decisions and are left out.
pub fn decisions(rec: &Value, seat: PlayerId) -> Result<Vec<Decision>, String> {
    if rec["format"] != FORMAT {
        return Err("not an arena game record".into());
    }
    let mut s: State = serde_json::from_value(rec["start"].clone()).map_err(|e| format!("start: {e}"))?;
    let mut dice = Dice::new(rec["seed"].as_u64().ok_or("no seed")?);
    let mut out = vec![];
    for (i, m) in rec["moves"].as_array().ok_or("no moves")?.iter().enumerate() {
        let a = action_for_mover(&s, &m["action"]).map_err(|e| format!("move {i}: {e}"))?;
        if m["seat"].as_u64() == Some(seat as u64) && m.get("note").is_none() {
            let view = fog::player_view(&s, s.current);
            let pick = if a == Action::EndTurn {
                -1
            } else {
                arena_abi::legal(&view).iter().position(|l| *l == a).ok_or(format!("move {i}: not in the legal list"))? as i32
            };
            out.push(Decision { move_index: i, view, pick });
        }
        dice.step(&mut s, &a).map_err(|e| format!("move {i}: {e}"))?;
    }
    Ok(out)
}

/// `k` distinct indices below `n`, drawn from a drand round's randomness (hex): the i-th draw is
/// the first 8 bytes of sha256("libaw-arena/spot/RANDOMNESS/i") mod the number left. Anyone can
/// recompute them.
pub fn sample(n: usize, randomness: &str, k: usize) -> Vec<usize> {
    use sha2::Digest;
    let mut left: Vec<usize> = (0..n).collect();
    let mut out = vec![];
    for i in 0..k.min(n) {
        let h = sha2::Sha256::digest(format!("libaw-arena/spot/{}/{i}", randomness.to_ascii_lowercase()));
        let x = u64::from_be_bytes(h[..8].try_into().unwrap());
        out.push(left.remove((x % left.len() as u64) as usize));
    }
    out
}

/// sha256 of a view's postcard bytes, hex: what a decision proof commits to.
pub fn view_hash(view: &State) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(arena_abi::view_bytes(view)))
}
