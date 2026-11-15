//! The arena's site: one page with a leaderboard per league, a call to enter a bot, and every
//! game of the latest event with its turns linked into libaw's replay viewer. Layout of the
//! site it writes into:
//!
//!   index.html                       this page
//!   games/MODE/NAME.json             the game records
//!   games/MODE/NAME/turn-NNN.json    storyboards
//!   libaw/web/, libaw/data/          a copy of libaw's viewer (the workflow checks libaw out)

use serde_json::Value;
use std::fmt::Write;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn result_text(r: &Value, first: &str, second: &str, seats: &[String]) -> String {
    match r["winner"].as_str() {
        None => format!("draw, day {}", r["day"]),
        Some(w) => {
            let who = if seats.first().map(String::as_str) == Some(w) { first } else { second };
            format!("{} wins, day {}", esc(who), r["day"])
        }
    }
}

/// One game's entry: `turns` is (day, seat name) for each storyboard, in order.
pub struct Game {
    /// Path of the record under games/ (MODE/NAME.json).
    pub file: String,
    pub first: String,
    pub second: String,
    pub result: Value,
    /// Team ids in seat order, to name the winner.
    pub teams: Vec<String>,
    pub turns: Vec<(u64, String)>,
}

/// One round-robin (one mode): its standings.json and its games.
pub struct Event {
    pub standings: Value,
    pub games: Vec<Game>,
}

/// Where the page points for the code, the registry and the run that made it.
pub struct Links {
    /// https://github.com/OWNER/REPO
    pub repo: String,
    /// The Actions run that played this event, if known.
    pub run: Option<String>,
    /// When the event was played (UTC, as text).
    pub when: Option<String>,
}

const STYLE: &str = r#"
:root{--bg:#fbfbfa;--panel:#fff;--fg:#1b1d21;--mute:#5f6670;--line:#e3e4e7;--link:#2153c4;--accent:#1f7a4d;--accent-fg:#fff;--chip:#eef0f3}
@media (prefers-color-scheme:dark){:root{--bg:#121417;--panel:#191c20;--fg:#e7e8ea;--mute:#9ba1ab;--line:#2a2e35;--link:#8fb0ff;--accent:#3ba872;--accent-fg:#0d1210;--chip:#23272d}}
*{box-sizing:border-box}body{background:var(--bg);color:var(--fg);font:15px/1.55 system-ui,-apple-system,sans-serif;margin:0}
main{max-width:900px;margin:0 auto;padding:32px 16px 64px}
h1{font-size:28px;margin:0 0 6px;letter-spacing:-.01em}h2{font-size:19px;margin:36px 0 10px}h3{font-size:16px;margin:0 0 4px}
p{margin:0 0 12px}.lede{font-size:17px;color:var(--mute);max-width:640px}
a{color:var(--link)}.mute{color:var(--mute)}code{font-size:13px;background:var(--chip);padding:1px 5px;border-radius:4px}
.cta{display:flex;flex-wrap:wrap;gap:10px;margin:18px 0 8px}
.btn{display:inline-block;padding:9px 16px;border-radius:8px;text-decoration:none;font-weight:600;border:1px solid var(--line);color:var(--fg);background:var(--panel)}
.btn.primary{background:var(--accent);border-color:var(--accent);color:var(--accent-fg)}
.points{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:12px;margin:22px 0 4px}
.points div{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:12px 14px;font-size:14px}
.league{background:var(--panel);border:1px solid var(--line);border-radius:12px;padding:16px;margin:12px 0}
.league .head{display:flex;justify-content:space-between;gap:12px;flex-wrap:wrap;align-items:baseline;margin-bottom:8px}
.chip{font-size:12px;background:var(--chip);border-radius:999px;padding:2px 9px;color:var(--mute)}
.scroll{overflow-x:auto}table{border-collapse:collapse;width:100%}th,td{text-align:left;padding:6px 8px;border-bottom:1px solid var(--line);white-space:nowrap}
th{color:var(--mute);font-weight:500;font-size:13px}.n{text-align:right;font-variant-numeric:tabular-nums}
details{border-bottom:1px solid var(--line);padding:8px 0}summary{cursor:pointer}
.turns{display:flex;flex-wrap:wrap;gap:4px 10px;margin:8px 0 4px;font-size:13px}
footer{margin-top:40px;font-size:13px;color:var(--mute)}
.wdl{display:block;font-size:12px;color:var(--mute)}td.n,th.n{padding-left:4px;padding-right:6px}th{white-space:normal;vertical-align:bottom}
"#;

pub fn render(events: &[Event], links: &Links) -> String {
    let repo = esc(&links.repo);
    let mut h = String::new();
    let _ = write!(
        h,
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>libaw arena</title><meta name="description" content="Advance Wars-style bots play each other here, sandboxed and verifiable, instead of on AWBW."><style>{STYLE}</style></head><body><main>
<h1>libaw arena</h1>
<p class="lede">Bots for Advance Wars–style games play each other here, on <a href="https://github.com/collinjackson/libaw">libaw</a>'s open engine, instead of on Advance Wars By Web. No bot game touches AWBW's servers, and anyone can check every result.</p>
<div class="cta"><a class="btn primary" href="{repo}/blob/main/docs/submitting.md">Enter your bot</a><a class="btn" href="{repo}/blob/main/docs/bot-abi.md">Write one</a><a class="btn" href="{repo}">Source</a></div>
<div class="points">
<div><b>Sandboxed.</b> Open bots are WebAssembly modules with no network, files or clock, and the same instruction budget on any machine.</div>
<div><b>Checkable.</b> Luck comes from a public <a href="https://drand.love">drand</a> round drawn after the bots are built. Every game replays from its record with <code>arena verify</code>.</div>
<div><b>In the open.</b> Bots enter by pull request, every game is played by GitHub Actions with public logs, and the registry's history shows each version.</div>
</div>
"#
    );

    // Open league: the event on this page.
    let _ = write!(h, r#"<h2>Leagues</h2><section class="league"><div class="head"><h3>Open bots</h3><span class="chip">source public, built and run here</span></div>"#);
    let b = events.iter().map(|e| &e.standings["beacon"]).find(|b| !b.is_null());
    let mut sub = vec![];
    if let Some(w) = &links.when {
        sub.push(format!("Played {}", esc(w)));
    }
    match b {
        None => sub.push("seeded by hand".into()),
        Some(b) => sub.push(format!(
            r#"luck from drand round <a href="https://api.drand.sh/{}/public/{}">{}</a>"#,
            esc(b["chain"].as_str().unwrap_or("")), b["round"], b["round"]
        )),
    }
    if let Some(r) = &links.run {
        sub.push(format!(r#"<a href="{}">the run's log</a>"#, esc(r)));
    }
    let _ = write!(
        h,
        r#"<p class="mute">{}. In each mode every pair plays twice, once from each seat. Each cell: the win rate, then won-drawn-lost; points are 2 a win, 1 a draw, over all modes.</p>"#,
        sub.join(", ")
    );
    // bot -> per-mode (won, drawn, lost); and total points.
    let mut bots: Vec<String> = vec![];
    let mut cell: std::collections::BTreeMap<(String, usize), (u64, u64, u64)> = Default::default();
    for (i, e) in events.iter().enumerate() {
        for r in e.standings["standings"].as_array().into_iter().flatten() {
            let n = r["bot"].as_str().unwrap_or("").to_string();
            if !bots.contains(&n) {
                bots.push(n.clone());
            }
            cell.insert((n, i), (r["won"].as_u64().unwrap_or(0), r["drawn"].as_u64().unwrap_or(0), r["lost"].as_u64().unwrap_or(0)));
        }
    }
    let points = |n: &String| (0..events.len()).filter_map(|i| cell.get(&(n.clone(), i))).map(|(w, d, _)| 2 * w + d).sum::<u64>();
    bots.sort_by_key(|n| std::cmp::Reverse(points(n)));
    h.push_str(r#"<div class="scroll"><table><tr><th>#</th><th>Bot</th>"#);
    for e in events {
        let _ = write!(h, r#"<th class="n">{}</th>"#, esc(e.standings["title"].as_str().unwrap_or(e.standings["mode"].as_str().unwrap_or("Games"))));
    }
    h.push_str(r#"<th class="n">Pts</th></tr>"#);
    for (rank, n) in bots.iter().enumerate() {
        let _ = write!(h, "<tr><td>{}</td><td>{}</td>", rank + 1, esc(n));
        for i in 0..events.len() {
            match cell.get(&(n.clone(), i)) {
                Some((w, d, l)) if w + d + l > 0 => {
                    let _ = write!(h, r#"<td class="n">{}%<small class="wdl">{w}-{d}-{l}</small></td>"#, (100 * w + (w + d + l) / 2) / (w + d + l));
                }
                _ => h.push_str(r#"<td class="n mute">–</td>"#),
            }
        }
        let _ = write!(h, r#"<td class="n">{}</td></tr>"#, points(n));
    }
    h.push_str("</table></div></section>\n");
    let _ = write!(
        h,
        r#"<section class="league"><div class="head"><h3>Private bots</h3><span class="chip">coming</span></div><p class="mute">Closed source, but each move comes with a zero-knowledge proof that the bot's committed code picked it from the legal moves: no hidden human helping, no Mechanical Turk. As checkable as an open bot.</p></section>
<section class="league"><div class="head"><h3>API bots</h3><span class="chip">coming · unverified</span></div><p class="mute">Bots on their owners' servers, called over the network at a normal pace. Fair games, but nothing shows the moves were unaided, so they get their own board. <a href="{repo}/issues">Tell us you're interested</a>.</p></section>
"#
    );

    h.push_str("<h2>Games</h2>\n");
    for e in events {
    if events.len() > 1 {
        let _ = write!(h, "<h3 style=\"margin-top:18px\">{}</h3>", esc(e.standings["title"].as_str().unwrap_or("")));
    }
    for g in &e.games {
        let stem = g.file.trim_end_matches(".json");
        let _ = write!(
            h,
            r#"<details><summary>{} vs {}: <span class="mute">{}</span></summary><div class="turns">"#,
            esc(&g.first), esc(&g.second), result_text(&g.result, &g.first, &g.second, &g.teams)
        );
        for (i, (day, who)) in g.turns.iter().enumerate() {
            let _ = write!(
                h,
                r#"<a href="libaw/web/replay.html?sb=../../games/{}/turn-{:03}.json&amp;art=awbw">day {} {}</a>"#,
                esc(stem), i + 1, day, esc(who)
            );
        }
        let _ = writeln!(h, r#"</div><a class="mute" href="games/{}">record</a></details>"#, esc(&g.file));
    }
    }
    let _ = write!(
        h,
        r#"<footer>Rebuilt by every event; nothing here runs on a server. Fork <a href="{repo}">the repo</a> and you have the whole arena. Advance Wars is Nintendo's; this is a fan project.</footer></main></body></html>
"#
    );
    h
}
