//! The sandbox's promises, with tiny hand-written bots (WAT), plus a full game with the example
//! bots when they've been built (scripts/build_registry.py).
use arena::bot::{Fault, Limits, WasmBot};
use arena::referee::{play, verify, Rules};
use serde_json::json;
use std::collections::BTreeMap;

fn skirmish() -> libaw::State {
    arena::load_state(include_str!("../../../maps/skirmish.json")).unwrap()
}

/// A bot that always ends its turn.
const PASS: &str = r#"(module
  (memory (export "memory") 1)
  (data (i32.const 1024) "{\"type\":\"end_turn\"}")
  (global $next (mut i32) (i32.const 4096))
  (func (export "arena_alloc") (param $n i32) (result i32)
    (local $p i32) (local.set $p (global.get $next))
    (global.set $next (i32.add (global.get $next) (local.get $n))) (local.get $p))
  (func (export "arena_act") (param i32 i32) (result i64)
    (i64.or (i64.shl (i64.const 1024) (i64.const 32)) (i64.const 19))))"#;

/// A bot that never returns.
const SPIN: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "arena_alloc") (param i32) (result i32) (i32.const 4096))
  (func (export "arena_act") (param i32 i32) (result i64) (loop $l (br $l)) (i64.const 0)))"#;

/// A bot that asks for the clock through WASI.
const WASI: &str = r#"(module
  (import "wasi_snapshot_preview1" "clock_time_get" (func (param i32 i64 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "arena_alloc") (param i32) (result i32) (i32.const 0))
  (func (export "arena_act") (param i32 i32) (result i64) (i64.const 0)))"#;

fn bot(name: &str, wat: &str, fuel: u64) -> WasmBot {
    WasmBot::new(name, wat.as_bytes(), Limits { fuel_per_action: fuel, ..Limits::default() }).unwrap()
}

#[test]
fn imports_other_than_libaw_query_are_refused() {
    let e = WasmBot::new("wasi", WASI.as_bytes(), Limits::default()).err().unwrap();
    assert!(e.contains("wasi_snapshot_preview1.clock_time_get"), "{e}");
}

#[test]
fn a_spinning_bot_runs_out_of_fuel_and_the_game_goes_on() {
    let s = skirmish();
    let mut spin = bot("spin", SPIN, 1_000_000);
    let r = spin.act(&json!({}), s.clone());
    assert_eq!(r.action, Err(Fault::OutOfFuel));
    assert_eq!(r.fuel_used, 1_000_000);
    // In a game: every one of its turns ends with a strike; the day limit ends it as a draw.
    let mut bots = BTreeMap::from([(101, bot("spin", SPIN, 100_000)), (102, bot("pass", PASS, 100_000))]);
    let rec = play(&s, &mut bots, 1, Rules { max_days: 3, ..Rules::default() });
    assert_eq!(rec["result"]["reason"], "day_limit");
    assert_eq!(rec["strikes"]["101"], 3);
    assert!(rec["strikes"].get("102").is_none());
    assert_eq!(verify(&rec).unwrap()["reason"], "day_limit");
}

#[test]
fn verify_catches_a_changed_record() {
    let s = skirmish();
    let mut bots = BTreeMap::from([(101, bot("pass", PASS, 100_000)), (102, bot("pass", PASS, 100_000))]);
    let mut rec = play(&s, &mut bots, 5, Rules { max_days: 2, ..Rules::default() });
    assert!(verify(&rec).is_ok());
    rec["result"]["winner"] = json!("101");
    assert!(verify(&rec).is_err());
}

#[test]
fn example_bots_play_a_verified_game() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../bots/out");
    let (Ok(g), Ok(r)) = (std::fs::read(format!("{dir}/greedy.wasm")), std::fs::read(format!("{dir}/random.wasm"))) else {
        eprintln!("skipped: run scripts/build_registry.py first");
        return;
    };
    let mut bots = BTreeMap::from([
        (101, WasmBot::new("greedy", &g, Limits::default()).unwrap()),
        (102, WasmBot::new("random", &r, Limits::default()).unwrap()),
    ]);
    let rec = play(&skirmish(), &mut bots, 7, Rules::default());
    assert_eq!(rec["strikes"], json!({}));
    assert_eq!(verify(&rec).unwrap(), rec["result"]);
    // It converts to one libaw storyboard per turn, each ending its turn but maybe the last.
    let sbs = arena::storyboard::turns(&rec).unwrap();
    assert!(sbs.len() > 2);
    for sb in &sbs[..sbs.len() - 1] {
        assert_eq!(sb["format"], "libaw-storyboard");
        assert_eq!(sb["steps"].as_array().unwrap().last().unwrap()["type"], "end");
    }
    assert_eq!(sbs[0]["names"]["101"], "greedy");
    let n: usize = sbs.iter().map(|s| s["steps"].as_array().unwrap().iter().filter(|x| x["type"] == "build").count()).sum();
    assert!(n > 0, "builds show up as build steps");
    // The same seed and bots give the same game.
    let mut again = BTreeMap::from([
        (101, WasmBot::new("greedy", &g, Limits::default()).unwrap()),
        (102, WasmBot::new("random", &r, Limits::default()).unwrap()),
    ]);
    assert_eq!(play(&skirmish(), &mut again, 7, Rules::default())["final_hash"], rec["final_hash"]);
}

#[test]
fn a_beacon_seed_is_checked() {
    use arena::referee::beacon_seed;
    let x = "a".repeat(64);
    assert_eq!(beacon_seed(&x, 0), beacon_seed(&x.to_uppercase(), 0));
    assert_ne!(beacon_seed(&x, 0), beacon_seed(&x, 1));
    let s = skirmish();
    let mut bots = BTreeMap::from([(101, bot("a", PASS, 1_000_000)), (102, bot("b", PASS, 1_000_000))]);
    let mut rec = play(&s, &mut bots, beacon_seed(&x, 3), Rules { max_days: 2, ..Rules::default() });
    rec["beacon"] = json!({"round": 1, "randomness": x, "game": 3});
    assert!(verify(&rec).is_ok());
    rec["beacon"]["game"] = json!(4);
    assert!(verify(&rec).unwrap_err().contains("drand"));
}
