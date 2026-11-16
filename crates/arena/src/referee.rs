//! The referee: plays a game between bots on libaw's engine and writes a record anyone can
//! check. `verify` replays a record from its seed and actions alone (no bots needed) and
//! confirms the result.
//!
//! Rules of the arena (docs/arena-rules.md):
//! - Each seat's bot sees only its own view (fog applies) and answers one action at a time.
//! - An action that isn't legal, or a bot fault (out of fuel, a trap, a bad reply), ends that
//!   seat's turn and counts as a strike. The game goes on.
//! - At most `max_actions_per_turn` actions a turn; then the turn is ended for the bot.
//! - The game ends when one side is left, or after `max_days` (a draw).
//! - Luck: ChaCha8 seeded with the game's seed, as libaw's server does, one roll per draw.

use crate::bot::{Fault, WasmBot};
use libaw::actions::{is_legal, Action};
use libaw::awbw::{action_from_contract, action_to_contract};
use libaw::{fog, sim, view, PlayerId, State};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const FORMAT: &str = "libaw-arena-game";
pub const VERSION: u64 = 1;

#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub max_days: u32,
    pub max_actions_per_turn: u32,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { max_days: 30, max_actions_per_turn: 400 }
    }
}

/// The dice: ChaCha8 from the seed, a roll in lo..=hi (libaw-server's draw).
pub struct Dice(rand_chacha::ChaCha8Rng);
impl Dice {
    pub fn new(seed: u64) -> Dice {
        Dice(<rand_chacha::ChaCha8Rng as rand_core::SeedableRng>::seed_from_u64(seed))
    }
    pub fn step(&mut self, s: &mut State, a: &Action) -> Result<(), String> {
        let mut rng = self.0.clone();
        let mut roll = |lo: i32, hi: i32| {
            let x = rand_core::RngCore::next_u64(&mut rng);
            lo + (x % (hi - lo + 1) as u64) as i32
        };
        let r = sim::step(s, a, &mut roll);
        if r.is_ok() {
            self.0 = rng;
        }
        r
    }
}

/// drand's quicknet chain (a round every 3 s), whose rounds seed arena games.
pub const DRAND_CHAIN: &str = "52db9ba70e0cc0f6eaf7803dd07447a1f5477735fd3f661792ba94600c84e971";

/// The seed for game `game` of an event seeded by a drand round's randomness (hex): the first
/// 8 bytes of sha256("libaw-arena/RANDOMNESS/GAME"), big-endian. Anyone can fetch the round
/// (https://api.drand.sh/CHAIN/public/ROUND) and recompute it.
pub fn beacon_seed(randomness: &str, game: u64) -> u64 {
    use sha2::Digest;
    let h = sha2::Sha256::digest(format!("libaw-arena/{}/{game}", randomness.to_ascii_lowercase()));
    u64::from_be_bytes(h[..8].try_into().unwrap())
}

/// Whether `a` is a legal action for the seat to move. In fog a bot can only judge by what it
/// sees, so legality is checked against its view; a move into an enemy it couldn't see is then
/// played as AWBW does, stopping short and waiting (a trap; libaw's sim applies it).
pub fn legal_for_mover(s: &State, a: &Action) -> bool {
    if s.fog {
        is_legal(&fog::player_view(s, s.current), a)
    } else {
        is_legal(s, a)
    }
}

/// Reads an action's JSON as the seat to move would: against its view in fog (see
/// `legal_for_mover`), against the game otherwise.
pub fn action_for_mover(s: &State, v: &Value) -> Result<Action, String> {
    if s.fog {
        action_from_contract(&fog::player_view(s, s.current), v).map_err(|e| e.to_string())
    } else {
        action_from_contract(s, v).map_err(|e| e.to_string())
    }
}

/// How the game ended: the winning team (None for a draw), why, and on which day.
fn outcome(s: &State, rules: &Rules) -> Option<Value> {
    if let Some(t) = sim::winner(s) {
        return Some(json!({"winner": t, "reason": "last_standing", "day": s.day}));
    }
    (s.day > rules.max_days).then(|| json!({"winner": Value::Null, "reason": "day_limit", "day": rules.max_days}))
}

/// The bot's next action for the seat to move, as the referee will play it: its answer if that's
/// legal, else the end of its turn with a note saying why (a strike). Also the fuel it used.
fn choose(s: &State, bot: &mut WasmBot, this_turn: u32, rules: &Rules) -> (Action, Option<String>, u64) {
    let reply = bot.act(fog::player_view(s, s.current));
    let (a, note) = match reply.action {
        Ok(Some(a)) if legal_for_mover(s, &a) => (a, None),
        Ok(_) => (Action::EndTurn, Some("bad pick: not -1 or an index into the legal list".to_string())),
        Err(Fault::OutOfFuel) => (Action::EndTurn, Some("out of fuel".to_string())),
        Err(Fault::Trap(e)) => (Action::EndTurn, Some(format!("fault: {e}"))),
    };
    if this_turn >= rules.max_actions_per_turn && a != Action::EndTurn {
        return (Action::EndTurn, Some("action limit for the turn".to_string()), reply.fuel_used);
    }
    (a, note, reply.fuel_used)
}

/// Plays a game. `bots`: one per seat, by player id. Returns the game record.
pub fn play(start: &State, bots: &mut BTreeMap<PlayerId, WasmBot>, seed: u64, rules: Rules) -> Value {
    let mut s = start.clone();
    let mut dice = Dice::new(seed);
    let mut moves = vec![];
    let mut strikes: BTreeMap<PlayerId, u32> = BTreeMap::new();
    let mut fuel: BTreeMap<PlayerId, u64> = BTreeMap::new();
    let mut this_turn = 0u32;
    let result = loop {
        if let Some(r) = outcome(&s, &rules) {
            break r;
        }
        let me = s.current;
        let (a, note, used) = choose(&s, bots.get_mut(&me).expect("a bot for every seat"), this_turn, &rules);
        *fuel.entry(me).or_default() += used;
        if note.is_some() {
            *strikes.entry(me).or_default() += 1;
        }
        let label = view::describe(&s, &a);
        let before = s.clone();
        if let Err(e) = dice.step(&mut s, &a) {
            s = before;
            // The engine refused something is_legal accepted: an engine bug. End the turn so the
            // game can go on, and say so in the record.
            let _ = dice.step(&mut s, &Action::EndTurn);
            moves.push(json!({"seat": me, "action": action_to_contract(&Action::EndTurn), "note": format!("engine refused {label}: {e}")}));
            this_turn = 0;
            continue;
        }
        this_turn = if a == Action::EndTurn { 0 } else { this_turn + 1 };
        let mut m = json!({"seat": me, "action": action_to_contract(&a), "fuel": used});
        if let Some(n) = note {
            m["note"] = n.into();
        }
        moves.push(m);
    };
    json!({
        "format": FORMAT, "version": VERSION, "seed": seed,
        "rules": {"max_days": rules.max_days, "max_actions_per_turn": rules.max_actions_per_turn},
        "bots": bots.iter().map(|(p, b)| json!({"seat": p, "name": b.name, "sha256": b.hash})).collect::<Vec<_>>(),
        "start": serde_json::to_value(start).unwrap_or(Value::Null),
        "moves": moves,
        "result": result,
        "strikes": strikes, "fuel": fuel,
        "final_hash": view::state_hash(&s),
    })
}

/// Replays a record from its start, seed and actions; Ok(result) if it reaches the recorded
/// result and final state, Err(why) otherwise. Needs no bots.
pub fn verify(rec: &Value) -> Result<Value, String> {
    if rec["format"] != FORMAT {
        return Err("not an arena game record".into());
    }
    let start: State = serde_json::from_value(rec["start"].clone()).map_err(|e| format!("start: {e}"))?;
    let rules = Rules {
        max_days: rec["rules"]["max_days"].as_u64().unwrap_or(30) as u32,
        max_actions_per_turn: rec["rules"]["max_actions_per_turn"].as_u64().unwrap_or(400) as u32,
    };
    let seed = rec["seed"].as_u64().ok_or("no seed")?;
    if let Some(b) = rec.get("beacon").filter(|b| !b.is_null()) {
        let (Some(r), Some(g)) = (b["randomness"].as_str(), b["game"].as_u64()) else { return Err("beacon needs randomness and game".into()) };
        if beacon_seed(r, g) != seed {
            return Err(format!("seed {seed} doesn't come from drand round {}", b["round"]));
        }
    }
    let mut s = start;
    let mut dice = Dice::new(seed);
    for (i, m) in rec["moves"].as_array().ok_or("no moves")?.iter().enumerate() {
        if outcome(&s, &rules).is_some() {
            return Err(format!("move {i} comes after the game ended"));
        }
        if m["seat"].as_u64() != Some(s.current as u64) {
            return Err(format!("move {i}: seat {} moved on {}'s turn", m["seat"], s.current));
        }
        let a = action_for_mover(&s, &m["action"]).map_err(|e| format!("move {i}: {e}"))?;
        if a != Action::EndTurn && !legal_for_mover(&s, &a) {
            return Err(format!("move {i}: not legal"));
        }
        // A refused action was recorded only as the end turn that followed it.
        dice.step(&mut s, &a).map_err(|e| format!("move {i}: {e}"))?;
    }
    let got = outcome(&s, &rules).ok_or("the moves end before the game does")?;
    if got != rec["result"] {
        return Err(format!("result {got} differs from the recorded {}", rec["result"]));
    }
    if view::state_hash(&s) != rec["final_hash"].as_str().unwrap_or("") {
        return Err("final state differs from the record".into());
    }
    Ok(got)
}

pub const TURN_FORMAT: &str = "libaw-arena-turn";

/// One turn of an asynchronous game, where each turn is played (and, for a private bot, proven)
/// by its bot's owner and sent to the arena: the seat to move plays from `start` with dice from
/// `seed` until it ends its turn or the game ends. Returns the turn record; `check_turn` checks
/// it from the state alone, without the bot.
pub fn play_turn(start: &State, bot: &mut WasmBot, seed: u64, rules: Rules) -> (Value, State) {
    let mut s = start.clone();
    let me = s.current;
    let mut dice = Dice::new(seed);
    let (mut moves, mut strikes, mut fuel, mut n) = (vec![], 0u32, 0u64, 0u32);
    while outcome(&s, &rules).is_none() && s.current == me {
        let (a, note, used) = choose(&s, bot, n, &rules);
        fuel += used;
        strikes += note.is_some() as u32;
        let before = s.clone();
        let (a, note) = match dice.step(&mut s, &a) {
            Ok(()) => (a, note),
            Err(e) => {
                s = before;
                let label = view::describe(&s, &a);
                let _ = dice.step(&mut s, &Action::EndTurn);
                (Action::EndTurn, Some(format!("engine refused {label}: {e}")))
            }
        };
        n += 1;
        let mut m = json!({"action": action_to_contract(&a), "fuel": used});
        if let Some(x) = note {
            m["note"] = x.into();
        }
        moves.push(m);
    }
    let rec = json!({
        "format": TURN_FORMAT, "version": VERSION, "seat": me, "day": start.day, "seed": seed,
        "rules": {"max_days": rules.max_days, "max_actions_per_turn": rules.max_actions_per_turn},
        "bot": {"name": bot.name, "sha256": bot.hash},
        "before_hash": view::state_hash(start), "after_hash": view::state_hash(&s),
        "moves": moves, "strikes": strikes, "fuel": fuel,
        "result": outcome(&s, &rules),
    });
    (rec, s)
}

/// Checks a turn record against the state it was played from: the right seat, every action
/// legal, the turn ended, and the same state after. Returns that state. It shows the moves are
/// legal; that the bot chose them is what a turn proof adds (docs/proofs.md).
pub fn check_turn(start: &State, rec: &Value) -> Result<State, String> {
    if rec["format"] != TURN_FORMAT {
        return Err("not an arena turn record".into());
    }
    if rec["before_hash"].as_str() != Some(view::state_hash(start).as_str()) {
        return Err("played from a different state".into());
    }
    if rec["seat"].as_u64() != Some(start.current as u64) {
        return Err(format!("seat {} moved on {}'s turn", rec["seat"], start.current));
    }
    let rules = Rules {
        max_days: rec["rules"]["max_days"].as_u64().unwrap_or(30) as u32,
        max_actions_per_turn: rec["rules"]["max_actions_per_turn"].as_u64().unwrap_or(400) as u32,
    };
    let mut s = start.clone();
    let mut dice = Dice::new(rec["seed"].as_u64().ok_or("no seed")?);
    let me = s.current;
    for (i, m) in rec["moves"].as_array().ok_or("no moves")?.iter().enumerate() {
        if outcome(&s, &rules).is_some() || s.current != me {
            return Err(format!("move {i} comes after the turn ended"));
        }
        let a = action_for_mover(&s, &m["action"]).map_err(|e| format!("move {i}: {e}"))?;
        if a != Action::EndTurn && (!legal_for_mover(&s, &a) || i as u32 >= rules.max_actions_per_turn) {
            return Err(format!("move {i}: not legal"));
        }
        dice.step(&mut s, &a).map_err(|e| format!("move {i}: {e}"))?;
    }
    if outcome(&s, &rules).is_none() && s.current == me {
        return Err("the turn doesn't end".into());
    }
    if rec["after_hash"].as_str() != Some(view::state_hash(&s).as_str()) {
        return Err("the state after differs from the record".into());
    }
    Ok(s)
}
