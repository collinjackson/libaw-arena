//! arena: play, check and rank sandboxed bot games.
//!
//!   arena play  --map MAP.json --bot NAME=BOT.wasm --bot NAME=BOT.wasm [--seed N] [--days N] [--fuel N] --out GAME.json
//!   arena verify GAME.json...
//!   arena storyboards GAME.json --out DIR       (one libaw storyboard per turn, for libaw's replay viewer)
//!   arena page RR_DIR --out SITE                (a static page of a round-robin; see src/page.rs)
//!   arena round-robin --map MAP.json --bot NAME=BOT.wasm... [--seed N] [--days N] [--fuel N] --out DIR
//!
//! --manifest bots/out/manifest.json (from scripts/build_registry.py) adds its bots, and each
//! game record names the repo and commit a bot was built from.
//!
//! --beacon ROUND:RANDOMNESS (a drand quicknet round, fetched before the games) replaces --seed:
//! game i's seed is referee::beacon_seed(RANDOMNESS, i), and the record says so for verify.
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
    beacon: Option<(u64, String)>,
    /// Bot name -> {repo, commit}, from --manifest.
    sources: BTreeMap<String, Value>,
    rest: Vec<String>,
}

fn opts(args: &[String]) -> Opts {
    let mut o = Opts { map: None, bots: vec![], seed: 1, rules: Rules::default(), limits: Limits::default(), out: None, beacon: None, sources: BTreeMap::new(), rest: vec![] };
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
            "--manifest" => {
                let f = val(i);
                let m: Value = serde_json::from_slice(&read(&f)).unwrap_or_else(|e| die(&format!("{f}: {e}")));
                for b in m.as_array().unwrap_or_else(|| die("a manifest is a list of bots")) {
                    let (Some(n), Some(w)) = (b["name"].as_str(), b["wasm"].as_str()) else { die("manifest entries need name and wasm") };
                    o.bots.push((n.to_string(), w.to_string()));
                    o.sources.insert(n.to_string(), json!({"repo": b["repo"], "commit": b["commit"]}));
                }
                i += 1;
            }
            "--beacon" => {
                let v = val(i);
                let (r, x) = v.split_once(':').unwrap_or_else(|| die("--beacon ROUND:RANDOMNESS"));
                let r = r.parse().unwrap_or_else(|_| die("--beacon ROUND:RANDOMNESS"));
                if x.len() != 64 || !x.chars().all(|c| c.is_ascii_hexdigit()) {
                    die("drand randomness is 64 hex digits");
                }
                o.beacon = Some((r, x.to_ascii_lowercase()));
                i += 1;
            }
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

/// Game number `n` of this run: its seed, from the beacon if there is one, and the record's
/// "beacon" entry.
fn seed_for(o: &Opts, n: u64) -> (u64, Value) {
    match &o.beacon {
        Some((round, x)) => (
            referee::beacon_seed(x, n),
            json!({"chain": referee::DRAND_CHAIN, "round": round, "randomness": x, "game": n}),
        ),
        None => (o.seed + n, Value::Null),
    }
}

fn game(start: &State, names: &[(String, String)], n: u64, o: &Opts) -> Value {
    let (seed, beacon) = seed_for(o, n);
    let order = seats(start);
    if order.len() != names.len() {
        die(&format!("the map has {} seats, {} bots given", order.len(), names.len()));
    }
    let mut bots = BTreeMap::new();
    for (p, (n, path)) in order.iter().zip(names) {
        bots.insert(*p, WasmBot::new(n, &read(path), o.limits).unwrap_or_else(|e| die(&format!("{n}: {e}"))));
    }
    let mut rec = referee::play(start, &mut bots, seed, o.rules);
    if !beacon.is_null() {
        rec["beacon"] = beacon;
    }
    if let Some(bs) = rec["bots"].as_array_mut() {
        for b in bs {
            if let Some(src) = b["name"].as_str().and_then(|n| o.sources.get(n)) {
                b["source"] = src.clone();
            }
        }
    }
    rec
}

/// Links for the page from the Actions environment (GITHUB_*), or ARENA_REPO locally.
fn page_links() -> arena::page::Links {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let server = env("GITHUB_SERVER_URL").unwrap_or_else(|| "https://github.com".into());
    let repo = env("ARENA_REPO")
        .or_else(|| env("GITHUB_REPOSITORY").map(|r| format!("{server}/{r}")))
        .unwrap_or_else(|| "https://github.com/collinjackson/libaw-arena".into());
    let run = env("GITHUB_RUN_ID").map(|id| format!("{repo}/actions/runs/{id}"));
    arena::page::Links { repo, run, when: env("ARENA_WHEN") }
}

/// Writes a record's storyboards into `dir` as turn-NNN.json and returns them.
fn storyboards(rec: &Value, dir: &str) -> Vec<Value> {
    let sbs = arena::storyboard::turns(rec).unwrap_or_else(|e| die(&e));
    std::fs::create_dir_all(dir).unwrap_or_else(|e| die(&e.to_string()));
    for (i, sb) in sbs.iter().enumerate() {
        std::fs::write(format!("{dir}/turn-{:03}.json", i + 1), serde_json::to_vec(sb).unwrap()).unwrap_or_else(|e| die(&e.to_string()));
    }
    sbs
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
            let rec = game(&start(), &o.bots, 0, &o);
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
        "storyboards" => {
            let f = o.rest.first().unwrap_or_else(|| die("arena storyboards GAME.json --out DIR"));
            let dir = o.out.clone().unwrap_or_else(|| die("--out DIR"));
            let rec: Value = serde_json::from_slice(&read(f)).unwrap_or_else(|e| die(&format!("{f}: {e}")));
            let n = storyboards(&rec, &dir).len();
            println!("{n} turns in {dir}");
        }
        "page" => {
            let rr = o.rest.first().unwrap_or_else(|| die("arena page RR_DIR --out SITE"));
            let site = o.out.clone().unwrap_or_else(|| die("--out SITE"));
            let st: Value = serde_json::from_slice(&read(&format!("{rr}/standings.json"))).unwrap_or_else(|e| die(&e.to_string()));
            std::fs::create_dir_all(format!("{site}/games")).unwrap_or_else(|e| die(&e.to_string()));
            let mut games = vec![];
            for g in st["games"].as_array().into_iter().flatten() {
                let file = g["file"].as_str().unwrap_or_else(|| die("standings.json: a game without a file")).to_string();
                let bytes = read(&format!("{rr}/{file}"));
                std::fs::write(format!("{site}/games/{file}"), &bytes).unwrap_or_else(|e| die(&e.to_string()));
                let rec: Value = serde_json::from_slice(&bytes).unwrap_or_else(|e| die(&format!("{file}: {e}")));
                let sbs = storyboards(&rec, &format!("{site}/games/{}", file.trim_end_matches(".json")));
                let start: State = serde_json::from_value(rec["start"].clone()).unwrap_or_else(|e| die(&e.to_string()));
                let teams = seats(&start).iter().map(|p| start.player(*p).map(|x| x.team.clone()).unwrap_or_default()).collect();
                let turns = sbs.iter().map(|sb| (sb["day"].as_u64().unwrap_or(0), sb["names"][sb["player"].to_string()].as_str().unwrap_or("?").to_string())).collect();
                games.push(arena::page::Game {
                    file, first: g["first"].as_str().unwrap_or("").into(), second: g["second"].as_str().unwrap_or("").into(),
                    result: g["result"].clone(), teams, turns,
                });
            }
            std::fs::write(format!("{site}/index.html"), arena::page::render(&st, &games, &page_links())).unwrap_or_else(|e| die(&e.to_string()));
            println!("{} games in {site}/index.html", games.len());
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
            let mut n = 0u64;
            for i in 0..o.bots.len() {
                for j in 0..o.bots.len() {
                    if i == j {
                        continue;
                    }
                    let pair = [o.bots[i].clone(), o.bots[j].clone()];
                    let rec = game(&s, &pair, n, &o);
                    let seed = rec["seed"].as_u64().unwrap_or(0);
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
                    let file = format!("{dir}/game-{:02}-{}-vs-{}.json", n + 1, pair[0].0, pair[1].0);
                    std::fs::write(&file, serde_json::to_vec(&rec).unwrap()).unwrap();
                    println!("game {} (seed {seed}): {} vs {}: {}", n + 1, pair[0].0, pair[1].0, rec["result"]);
                    games.push(json!({"file": file.rsplit('/').next(), "seed": seed, "first": pair[0].0, "second": pair[1].0, "result": rec["result"]}));
                    n += 1;
                }
            }
            let mut rows: Vec<_> = table.iter().map(|(n, [w, d, l])| json!({"bot": n, "won": w, "drawn": d, "lost": l, "points": 2 * w + d})).collect();
            rows.sort_by_key(|r| std::cmp::Reverse(r["points"].as_u64().unwrap_or(0)));
            let beacon = seed_for(&o, 0).1;
            std::fs::write(format!("{dir}/standings.json"), serde_json::to_vec_pretty(&json!({"beacon": beacon, "games": games, "standings": rows})).unwrap()).unwrap();
            for r in &rows {
                println!("{:<16} {:>3} won {:>3} drawn {:>3} lost  {} pts", r["bot"].as_str().unwrap(), r["won"], r["drawn"], r["lost"], r["points"]);
            }
        }
        _ => die("usage: arena play|verify|round-robin ..."),
    }
}
