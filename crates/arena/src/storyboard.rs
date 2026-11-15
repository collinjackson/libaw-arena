//! Arena games as libaw storyboards (libaw docs/storyboard.md), one per turn, so libaw's replay
//! viewer (web/replay.html) can play them. The steps come from replaying the record and
//! comparing the state before and after each action.

use crate::referee::Dice;
use libaw::actions::{Action, Then};
use libaw::awbw::action_from_contract;
use libaw::{Hp, State, Unit};
use serde_json::{json, Value};

/// AWBW country codes for the seats in turn order, so `art=awbw` has armies to draw.
const COUNTRIES: [&str; 4] = ["os", "bm", "ge", "yc"];

fn side(u: Option<&Unit>, id: u64, at: (u16, u16)) -> Value {
    match u {
        Some(u) => json!({"unit_id": u.id, "at": [u.x, u.y], "hp": u.hp, "display_hp": (u.hp.hi as u32 + 9) / 10}),
        None => json!({"unit_id": id, "at": [at.0, at.1], "hp": Hp { lo: 0, hi: 0 }, "display_hp": 0}),
    }
}

/// The steps one action makes, from the states before and after it.
fn steps(before: &State, after: &State, a: &Action, contract: &Value) -> Vec<Value> {
    let mut out = vec![];
    match a {
        Action::Move { unit, path, then } => {
            out.push(json!({"type": "move", "unit_id": unit, "path": contract["path"], "trapped": false}));
            let to = path.last().copied().unwrap_or((0, 0));
            match then {
                Then::Attack { target } => {
                    let def = before.units.values().find(|u| (u.x, u.y) == *target);
                    let did = def.map_or(0, |u| u.id as u64);
                    out.push(json!({"type": "fire",
                        "attacker": side(after.units.get(unit), *unit as u64, to),
                        "defender": side(def.and_then(|d| after.units.get(&d.id)), did, *target)}));
                }
                Then::Capture => {
                    if let Some(b) = after.buildings.get(&to) {
                        let done = before.buildings.get(&to).map(|x| x.owner) != Some(b.owner);
                        out.push(json!({"type": "capture", "at": [to.0, to.1], "capture": b.capture, "done": done,
                                        "new_owner": if done { json!(b.owner) } else { Value::Null }}));
                    }
                }
                Then::Join => {
                    if let Some(into) = before.units.values().find(|u| (u.x, u.y) == to && u.id != *unit) {
                        out.push(json!({"type": "join", "from": unit, "into": into.id, "unit": after.units.get(&into.id)}));
                    }
                }
                _ => {}
            }
        }
        Action::Build { .. } => {
            if let Some(u) = after.units.values().find(|u| !before.units.contains_key(&u.id)) {
                out.push(json!({"type": "build", "unit": u}));
            }
        }
        Action::Power { kind } => {
            let co = before.player(before.current).map(|p| p.co.clone()).unwrap_or_default();
            out.push(json!({"type": "power", "player": before.current, "co": co, "name": kind, "super": kind.contains("super")}));
        }
        Action::EndTurn => out.push(json!({"type": "end", "next": after.current})),
    }
    out
}

/// One storyboard per turn of an arena game record.
pub fn turns(rec: &Value) -> Result<Vec<Value>, String> {
    let start: State = serde_json::from_value(rec["start"].clone()).map_err(|e| format!("start: {e}"))?;
    let mut order: Vec<_> = start.players.iter().collect();
    order.sort_by_key(|p| p.order);
    let names: serde_json::Map<String, Value> = rec["bots"].as_array().into_iter().flatten()
        .map(|b| (b["seat"].to_string(), b["name"].clone())).collect();
    let countries: serde_json::Map<String, Value> = order.iter().enumerate()
        .map(|(i, p)| (p.id.to_string(), json!(COUNTRIES[i % COUNTRIES.len()]))).collect();
    let mut s = start;
    let mut dice = Dice::new(rec["seed"].as_u64().ok_or("no seed")?);
    let mut out = vec![];
    let mut turn_start = s.clone();
    let mut turn_steps = vec![];
    let finish = |ts: &State, steps: Vec<Value>| json!({"format": "libaw-storyboard", "version": 1, "day": ts.day,
        "player": ts.current, "names": names, "countries": countries, "start": ts, "steps": steps});
    for (i, m) in rec["moves"].as_array().ok_or("no moves")?.iter().enumerate() {
        let a = action_from_contract(&s, &m["action"]).map_err(|e| format!("move {i}: {e}"))?;
        let before = s.clone();
        dice.step(&mut s, &a).map_err(|e| format!("move {i}: {e}"))?;
        turn_steps.extend(steps(&before, &s, &a, &m["action"]));
        if a == Action::EndTurn {
            out.push(finish(&turn_start, std::mem::take(&mut turn_steps)));
            turn_start = s.clone();
        }
    }
    if !turn_steps.is_empty() {
        out.push(finish(&turn_start, turn_steps));
    }
    Ok(out)
}
