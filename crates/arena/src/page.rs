//! A static page for a round-robin: standings, the beacon, and every game with its turns linked
//! into libaw's replay viewer. Layout of the site it writes into:
//!
//!   index.html                       this page
//!   games/NAME.json                  the game records
//!   games/NAME/turn-NNN.json         storyboards
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
    pub file: String,
    pub first: String,
    pub second: String,
    pub result: Value,
    /// Team ids in seat order, to name the winner.
    pub teams: Vec<String>,
    pub turns: Vec<(u64, String)>,
}

pub fn render(standings: &Value, games: &[Game]) -> String {
    let mut h = String::new();
    h.push_str(r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Arena standings</title><style>
:root{--bg:#fff;--fg:#1b1d21;--mute:#626873;--line:#e2e4e8;--link:#2457c5}
@media (prefers-color-scheme:dark){:root{--bg:#14161a;--fg:#e6e7ea;--mute:#9aa0aa;--line:#2a2e35;--link:#8fb0ff}}
body{background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,sans-serif;margin:0 auto;max-width:860px;padding:24px 16px}
h1{font-size:22px;margin:0 0 4px}p.sub{color:var(--mute);margin:0 0 20px}
table{border-collapse:collapse;width:100%;margin-bottom:28px}th,td{text-align:left;padding:6px 8px;border-bottom:1px solid var(--line)}
th{color:var(--mute);font-weight:500;font-size:13px}td.n{font-variant-numeric:tabular-nums;text-align:right}th.n{text-align:right}
a{color:var(--link)}details{border-bottom:1px solid var(--line);padding:8px 0}summary{cursor:pointer}
.turns{display:flex;flex-wrap:wrap;gap:4px 10px;margin:8px 0 4px;font-size:13px}.mute{color:var(--mute)}
</style></head><body><h1>Arena standings</h1>
"#);
    let b = &standings["beacon"];
    if b.is_null() {
        let _ = writeln!(h, r#"<p class="sub">Seeded by hand (a re-run or a test).</p>"#);
    } else {
        let _ = writeln!(
            h,
            r#"<p class="sub">Luck from drand quicknet round <a href="https://api.drand.sh/{}/public/{}">{}</a>. Every game below replays from its record with <code>arena verify</code>.</p>"#,
            esc(b["chain"].as_str().unwrap_or("")), b["round"], b["round"]
        );
    }
    h.push_str(r#"<table><tr><th>Bot</th><th class="n">Won</th><th class="n">Drawn</th><th class="n">Lost</th><th class="n">Points</th></tr>"#);
    for r in standings["standings"].as_array().into_iter().flatten() {
        let _ = write!(
            h,
            r#"<tr><td>{}</td><td class="n">{}</td><td class="n">{}</td><td class="n">{}</td><td class="n">{}</td></tr>"#,
            esc(r["bot"].as_str().unwrap_or("")), r["won"], r["drawn"], r["lost"], r["points"]
        );
    }
    h.push_str("</table>\n<h2>Games</h2>\n");
    for g in games {
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
    h.push_str("</body></html>\n");
    h
}
