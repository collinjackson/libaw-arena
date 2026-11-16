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
  (func (export "arena_alloc") (param i32) (result i32) (i32.const 4096))
  (func (export "arena_pick") (param i32) (result i32) (i32.const -1)))"#;

/// A bot whose pick isn't in the legal list.
const BAD_PICK: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "arena_alloc") (param i32) (result i32) (i32.const 4096))
  (func (export "arena_pick") (param i32) (result i32) (i32.const 99999)))"#;

/// A bot that never returns.
const SPIN: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "arena_alloc") (param i32) (result i32) (i32.const 4096))
  (func (export "arena_pick") (param i32) (result i32) (loop $l (br $l)) (i32.const 0)))"#;

/// A bot that asks for the clock through WASI.
const WASI: &str = r#"(module
  (import "wasi_snapshot_preview1" "clock_time_get" (func (param i32 i64 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "arena_alloc") (param i32) (result i32) (i32.const 0))
  (func (export "arena_pick") (param i32) (result i32) (i32.const -1)))"#;

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
    let r = spin.act(s.clone());
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
fn a_pick_outside_the_legal_list_is_a_strike() {
    let mut bots = BTreeMap::from([(101, bot("bad", BAD_PICK, 100_000)), (102, bot("pass", PASS, 100_000))]);
    let rec = play(&skirmish(), &mut bots, 1, Rules { max_days: 2, ..Rules::default() });
    assert_eq!(rec["strikes"]["101"], 2);
    assert!(rec["moves"][0]["note"].as_str().unwrap().starts_with("bad pick"));
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

#[test]
fn modes_set_fog_and_funds() {
    // The skirmish map: the first player (101) has 4 income properties, credited on day 1.
    let mut s = skirmish();
    assert_eq!(s.player(101).unwrap().funds, 4000);
    arena::apply_mode(&mut s, &json!({"name": "high-funds", "fog": true, "starting_funds": 0, "funds_per_property": 2000}));
    assert!(s.fog);
    assert_eq!(s.funds_per_property, 2000);
    assert_eq!(s.player(101).unwrap().funds, 8000); // day-1 income at 2,000 a property
    assert_eq!(s.player(102).unwrap().funds, 0);
    let mut same = skirmish();
    arena::apply_mode(&mut same, &json!({"name": "standard"}));
    assert_eq!(libaw::view::state_hash(&same), libaw::view::state_hash(&skirmish()));
}

#[test]
fn in_fog_a_move_into_an_unseen_enemy_is_a_trap_not_a_strike() {
    // Bots judge legality by their own view; moves into hidden units stop short (AWBW's trap).
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../bots/out");
    let (Ok(g), Ok(b)) = (std::fs::read(format!("{dir}/greedy.wasm")), std::fs::read(format!("{dir}/baseline.wasm"))) else {
        eprintln!("skipped: run scripts/build_registry.py first");
        return;
    };
    let mut s = skirmish();
    arena::apply_mode(&mut s, &json!({"name": "fog", "fog": true}));
    for seed in [3, 4] {
        let mut bots = BTreeMap::from([
            (101, WasmBot::new("greedy", &g, Limits::default()).unwrap()),
            (102, WasmBot::new("baseline", &b, Limits::default()).unwrap()),
        ]);
        let rec = play(&s, &mut bots, seed, Rules::default());
        assert_eq!(rec["strikes"], json!({}), "seed {seed}");
        assert_eq!(verify(&rec).unwrap(), rec["result"]);
        assert!(arena::storyboard::turns(&rec).is_ok());
    }
}

#[test]
fn turns_played_one_at_a_time_are_checked_from_the_state() {
    use arena::referee::{check_turn, play_turn};
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../bots/out");
    let (Ok(g), Ok(r)) = (std::fs::read(format!("{dir}/greedy.wasm")), std::fs::read(format!("{dir}/random.wasm"))) else {
        eprintln!("skipped: run scripts/build_registry.py first");
        return;
    };
    let mut bots = [WasmBot::new("greedy", &g, Limits::default()).unwrap(), WasmBot::new("random", &r, Limits::default()).unwrap()];
    let mut s = skirmish();
    for t in 0..8u64 {
        let (rec, after) = play_turn(&s, &mut bots[(t % 2) as usize], 50 + t, Rules::default());
        assert_eq!(rec["strikes"], 0);
        let checked = check_turn(&s, &rec).unwrap();
        assert_eq!(libaw::view::state_hash(&checked), libaw::view::state_hash(&after));
        assert_ne!(checked.current, s.current, "the turn passes to the other seat");
        // Not from another state, nor with a changed move, nor cut short.
        assert!(check_turn(&after, &rec).is_err());
        let mut bad = rec.clone();
        bad["moves"][0]["action"] = json!({"type": "end_turn"});
        if rec["moves"].as_array().unwrap().len() > 1 {
            assert!(check_turn(&s, &bad).is_err());
        }
        let mut short = rec.clone();
        short["moves"].as_array_mut().unwrap().pop();
        assert!(check_turn(&s, &short).is_err());
        s = checked;
    }
}

#[test]
fn any_decision_replays_on_its_own_for_a_spot_check() {
    use arena::spot::{decisions, sample};
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../bots/out");
    let (Ok(g), Ok(r)) = (std::fs::read(format!("{dir}/greedy.wasm")), std::fs::read(format!("{dir}/random.wasm"))) else {
        eprintln!("skipped: run scripts/build_registry.py first");
        return;
    };
    let mut bots = BTreeMap::from([
        (101, WasmBot::new("greedy", &g, Limits::default()).unwrap()),
        (102, WasmBot::new("random", &r, Limits::default()).unwrap()),
    ]);
    let rec = play(&skirmish(), &mut bots, 11, Rules { max_days: 15, ..Rules::default() });
    for (seat, wasm) in [(101, &g), (102, &r)] {
        let ds = decisions(&rec, seat).unwrap();
        assert!(ds.len() >= 5, "{} decisions", ds.len());
        // A fresh bot given only that decision's view picks the same, for every decision.
        let mut fresh = WasmBot::new("again", wasm, Limits::default()).unwrap();
        for d in &ds {
            let got = fresh.act(d.view.clone()).action.unwrap().unwrap();
            let want = if d.pick == -1 { libaw::actions::Action::EndTurn } else { arena_abi::legal(&d.view)[d.pick as usize].clone() };
            assert_eq!(got, want, "decision at move {}", d.move_index);
        }
        let s = sample(ds.len(), &"ab".repeat(32), 5);
        assert_eq!(s, sample(ds.len(), &"AB".repeat(32), 5));
        assert_eq!(s.len(), 5);
        assert!(s.iter().all(|&i| i < ds.len()) && s.iter().collect::<std::collections::BTreeSet<_>>().len() == 5);
    }
}
