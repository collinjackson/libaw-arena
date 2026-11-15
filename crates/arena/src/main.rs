//! arena: play, check and rank sandboxed bot games.
//!
//!   arena play  --map MAP.json --bot NAME=BOT.wasm --bot NAME=BOT.wasm [--seed N] [--days N] [--fuel N] --out GAME.json
//!   arena verify GAME.json...
//!   arena round-robin --map MAP.json --bot NAME=BOT.wasm... [--seed N] [--days N] [--fuel N] --out DIR
//!
//! Bots take the seats in turn order (the first --bot moves first). round-robin plays every pair
//! twice, once from each seat, with seeds seed, seed+1, ...; it writes each game and standings.json.

use arena::bot::{Limits, WasmBot};
use arena::referee::{self, Rules};
use libaw::{PlayerId, State};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::process::exit;

fn die(msg: &str) -> ! {
    eprintln!("arena: {msg}");
    exit(2)
}

struct Opts {
    map: Option<String>,
    bots: Vec<(String, String)>,
    seed: u64,
    rules: Rules,
    limits: Limits,
    out: Option<String>,
    rest: Vec<String>,
}

fn opts(args: &[String]) -> Opts {
    let mut o = Opts { map: None, bots: vec![], seed: 1, rules: Rules::default(), limits: Limits::default(), out: None, rest: vec![] };
    let mut i = 0;
    let val = |i: usize| args.get(i + 1).cloned().unwrap_or_else(|| die(&format!("{} needs a value", args[i])));
    while i < args.len() {
        match args[i].as_str() {
            "--map" => { o.map = Some(val(i)); i += 1; }
            "--bot" => {
                let v = val(i);
                let (n, p) = v.split_once('=').unwrap_or_else(|| die("--bot NAME=PATH.wasm"));
                o.bots.push((n.to_string(), p.to_string()));
                i += 1;
            }
            "--seed" => { o.seed = val(i).parse().unwrap_or_else(|_| die("--seed N")); i += 1; }
            "--days" => { o.rules.max_days = val(i).parse().unwrap_or_else(|_| die("--days N")); i += 1; }
            "--fuel" => { o.limits.fuel_per_action = val(i).parse().unwrap_or_else(|_| die("--fuel N")); i += 1; }
            "--out" => { o.out = Some(val(i)); i += 1; }
            a => o.rest.push(a.to_string()),
        }
        i += 1;
    }
    o
}

fn read(path: &str) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| die(&format!("{path}: {e}")))
}

fn seats(start: &State) -> Vec<PlayerId> {
    let mut ps: Vec<_> = start.players.iter().collect();
    ps.sort_by_key(|p| p.order);
    ps.iter().map(|p| p.id).collect()
}

fn game(start: &State, names: &[(String, String)], seed: u64, o: &Opts) -> Value {
    let order = seats(start);
    if order.len() != names.len() {
        die(&format!("the map has {} seats, {} bots given", order.len(), names.len()));
    }
    let mut bots = BTreeMap::new();
    for (p, (n, path)) in order.iter().zip(names) {
        bots.insert(*p, WasmBot::new(n, &read(path), o.limits).unwrap_or_else(|e| die(&format!("{n}: {e}"))));
    }
    referee::play(start, &mut bots, seed, o.rules)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else { die("usage: arena play|verify|round-robin ... (see src/main.rs)") };
    let o = opts(&args[1..]);
    let start = || {
        let m = o.map.clone().unwrap_or_else(|| die("--map MAP.json"));
        arena::load_state(&String::from_utf8_lossy(&read(&m))).unwrap_or_else(|e| die(&format!("{m}: {e}")))
    };
    match cmd.as_str() {
        "play" => {
            let rec = game(&start(), &o.bots, o.seed, &o);
            let out = o.out.clone().unwrap_or_else(|| die("--out GAME.json"));
            std::fs::write(&out, serde_json::to_vec(&rec).unwrap()).unwrap_or_else(|e| die(&e.to_string()));
            println!("{} moves; result {}; strikes {}", rec["moves"].as_array().map_or(0, |m| m.len()), rec["result"], rec["strikes"]);
        }
        "verify" => {
            let mut bad = 0;
            for f in &o.rest {
                let rec: Value = serde_json::from_slice(&read(f)).unwrap_or_else(|e| die(&format!("{f}: {e}")));
                match referee::verify(&rec) {
                    Ok(r) => println!("{f}: ok, {r}"),
                    Err(e) => { println!("{f}: FAILED: {e}"); bad += 1; }
                }
            }
            exit(if bad > 0 { 1 } else { 0 });
        }
        "round-robin" => {
            let s = start();
            let dir = o.out.clone().unwrap_or_else(|| die("--out DIR"));
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| die(&e.to_string()));
            let order = seats(&s);
            if order.len() != 2 {
                die("round-robin needs a two-player map");
            }
            let mut table: BTreeMap<String, [u32; 3]> = o.bots.iter().map(|(n, _)| (n.clone(), [0; 3])).collect();
            let mut games = vec![];
            let mut seed = o.seed;
            for i in 0..o.bots.len() {
                for j in 0..o.bots.len() {
                    if i == j {
                        continue;
                    }
                    let pair = [o.bots[i].clone(), o.bots[j].clone()];
                    let rec = game(&s, &pair, seed, &o);
                    let w = rec["result"]["winner"].as_str().map(str::to_string);
                    let team_of = |k: usize| s.player(order[k]).map(|p| p.team.clone()).unwrap_or_default();
                    for k in 0..2 {
                        let e = table.get_mut(&pair[k].0).unwrap();
                        match &w {
                            None => e[1] += 1,
                            Some(t) if *t == team_of(k) => e[0] += 1,
                            Some(_) => e[2] += 1,
                        }
                    }
                    let file = format!("{dir}/game-{seed}-{}-vs-{}.json", pair[0].0, pair[1].0);
                    std::fs::write(&file, serde_json::to_vec(&rec).unwrap()).unwrap();
                    println!("seed {seed}: {} vs {}: {}", pair[0].0, pair[1].0, rec["result"]);
                    games.push(json!({"file": file.rsplit('/').next(), "seed": seed, "first": pair[0].0, "second": pair[1].0, "result": rec["result"]}));
                    seed += 1;
                }
            }
            let mut rows: Vec<_> = table.iter().map(|(n, [w, d, l])| json!({"bot": n, "won": w, "drawn": d, "lost": l, "points": 2 * w + d})).collect();
            rows.sort_by_key(|r| std::cmp::Reverse(r["points"].as_u64().unwrap_or(0)));
            std::fs::write(format!("{dir}/standings.json"), serde_json::to_vec_pretty(&json!({"games": games, "standings": rows})).unwrap()).unwrap();
            for r in &rows {
                println!("{:<16} {:>3} won {:>3} drawn {:>3} lost  {} pts", r["bot"].as_str().unwrap(), r["won"], r["drawn"], r["lost"], r["points"]);
            }
        }
        _ => die("usage: arena play|verify|round-robin ..."),
    }
}
