//! The host side of the arena's bot interface (docs/bot-abi.md), shared by the referee
//! (wasmtime) and the turn prover (wasmi in a zkVM guest), so both answer a bot alike.
//!
//! A bot asks `libaw.ask(kind, arg)` and gets fixed-size little-endian records, which it reads
//! without parsing; it answers `arena_pick` with an index into the legal list. Everything is
//! computed from the seat's own view.
#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use alloc::vec::Vec;
use libaw::actions::{legal_actions, Action, Then};
use libaw::State;

pub use libaw;

/// What a bot can ask for (`kind`).
pub const LEGAL: i32 = 0;
pub const UNITS: i32 = 1;
pub const BUILDINGS: i32 = 2;
pub const GAME: i32 = 3;
pub const TERRAIN: i32 = 4;
pub const VIEW: i32 = 5;
pub const BASELINE: i32 = 6;

pub const LEGAL_SIZE: usize = 20;
pub const UNIT_SIZE: usize = 16;
pub const BUILDING_SIZE: usize = 8;
pub const PLAYER_SIZE: usize = 16;

/// No position (255, 255).
const NONE: (u16, u16) = (255, 255);

fn b(v: u16) -> u8 {
    v.min(255) as u8
}

fn then_code(t: &Then) -> (u8, (u16, u16), u32) {
    match t {
        Then::Wait => (0, NONE, 0),
        Then::Attack { target } => (1, *target, 0),
        Then::Capture => (2, NONE, 0),
        Then::Load => (3, NONE, 0),
        Then::Join => (4, NONE, 0),
        Then::Supply => (5, NONE, 0),
        Then::Repair { target } => (6, *target, 0),
        Then::Unload { cargo, to } => (7, *to, *cargo),
        Then::AttackSeam { target } => (8, *target, 0),
        Then::Hide => (9, NONE, 0),
        Then::Unhide => (10, NONE, 0),
        Then::Launch { target } => (11, *target, 0),
        Then::Delete => (12, NONE, 0),
    }
}

/// One legal action as a 20-byte record:
/// kind (0 end turn, 1 move, 2 build, 3 power), then (for a move: 0 wait, 1 attack, 2 capture,
/// 3 load, 4 join, 5 supply, 6 repair, 7 unload, 8 attack a pipe seam, 9 hide, 10 unhide,
/// 11 launch, 12 delete), unit type (the moving unit's, or what's built; for a power, 1 CO
/// power, 2 super power), path length (tiles, capped at 255), unit id (u32), from x, y, to x, y,
/// target x, y (255 if none), cargo id for an unload (u32), 2 bytes reserved.
pub fn legal_record(s: &State, a: &Action, out: &mut Vec<u8>) {
    let mut r = [0u8; LEGAL_SIZE];
    let (mut from, mut to, mut target) = (NONE, NONE, NONE);
    match a {
        Action::EndTurn => {}
        Action::Move { unit, path, then } => {
            let (code, t, cargo) = then_code(then);
            r[0] = 1;
            r[1] = code;
            r[2] = s.units.get(unit).map_or(255, |u| u.kind as u8);
            r[3] = path.len().min(255) as u8;
            r[4..8].copy_from_slice(&unit.to_le_bytes());
            from = *path.first().unwrap_or(&NONE);
            to = *path.last().unwrap_or(&NONE);
            target = t;
            r[14..18].copy_from_slice(&cargo.to_le_bytes());
        }
        Action::Build { at, kind } => {
            r[0] = 2;
            r[2] = *kind as u8;
            from = *at;
            to = *at;
        }
        Action::Power { kind } => {
            r[0] = 3;
            r[2] = *kind as u8;
        }
    }
    r[8..14].copy_from_slice(&[b(from.0), b(from.1), b(to.0), b(to.1), b(target.0), b(target.1)]);
    out.extend_from_slice(&r);
}

fn counted(n: usize, size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + n * size);
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out
}

/// The answer to `ask(kind, arg)` for a seat whose view is `view`, given that decision's legal
/// list. Unknown kinds get an empty answer.
pub fn answer(view: &State, legal: &[Action], kind: i32, _arg: i32) -> Vec<u8> {
    match kind {
        LEGAL => {
            let mut out = counted(legal.len(), LEGAL_SIZE);
            for a in legal {
                legal_record(view, a, &mut out);
            }
            out
        }
        // Every unit the seat can see: id, owner (player id), type, x, y, HP (shown, 1-10), ammo
        // and fuel (capped at 255), flags (1 moved, 2 carried, 4 dived, 8 stunned), carrier id
        // (u16, 0 if none), reserved.
        UNITS => {
            let mut out = counted(view.units.len(), UNIT_SIZE);
            for u in view.units.values() {
                let flags = u.moved as u8 | (u.carried_by.is_some() as u8) << 1 | (u.dived as u8) << 2 | (u.stunned as u8) << 3;
                out.extend_from_slice(&u.id.to_le_bytes());
                out.extend_from_slice(&u.owner.to_le_bytes());
                out.extend_from_slice(&[u.kind as u8, b(u.x), b(u.y), u.hp.display_hi(), u.ammo.clamp(0, 255) as u8, u.fuel.clamp(0, 255) as u8, flags]);
                out.extend_from_slice(&[0]);
            }
            out
        }
        // Every building: x, y, terrain id (u16), owner (player id, 0 if none).
        BUILDINGS => {
            let mut out = counted(view.buildings.len(), BUILDING_SIZE);
            for ((x, y), bl) in &view.buildings {
                out.extend_from_slice(&[b(*x), b(*y)]);
                out.extend_from_slice(&bl.terrain.to_le_bytes());
                out.extend_from_slice(&bl.owner.unwrap_or(0).to_le_bytes());
            }
            out
        }
        // width, height (u16 each), day, the seat to move, then the players: id, funds, power
        // meter (u32 each), turn order, eliminated, power on (0, 1 CO power, 2 super), reserved.
        GAME => {
            let mut out = Vec::new();
            out.extend_from_slice(&view.width.to_le_bytes());
            out.extend_from_slice(&view.height.to_le_bytes());
            out.extend_from_slice(&view.day.to_le_bytes());
            out.extend_from_slice(&view.current.to_le_bytes());
            out.extend_from_slice(&(view.players.len() as u32).to_le_bytes());
            for p in &view.players {
                out.extend_from_slice(&p.id.to_le_bytes());
                out.extend_from_slice(&(p.funds.clamp(0, u32::MAX as i64) as u32).to_le_bytes());
                out.extend_from_slice(&(p.meter.clamp(0, u32::MAX as i64) as u32).to_le_bytes());
                out.extend_from_slice(&[p.order.min(255) as u8, p.eliminated as u8, p.power as u8, 0]);
            }
            out
        }
        // The terrain id of every tile, row by row (u16 each).
        TERRAIN => view.terrain.iter().flat_map(|t| t.to_le_bytes()).collect(),
        // The whole view, as libaw's State in postcard, for bots that bring libaw's engine.
        VIEW => postcard::to_allocvec(view).unwrap_or_default(),
        // libaw's baseline policy's choice, as an index into the legal list (u32; u32::MAX to
        // end the turn): libaw's engine work, run natively for any bot that wants a suggestion.
        BASELINE => {
            let a = libaw::baseline::choose(view);
            let i = legal.iter().position(|l| *l == a).map_or(u32::MAX, |i| i as u32);
            i.to_le_bytes().to_vec()
        }
        _ => Vec::new(),
    }
}

/// One decision's legal list, in the order `LEGAL` answers it and `arena_pick` indexes it.
pub fn legal(view: &State) -> Vec<Action> {
    legal_actions(view)
}

/// The action a bot's pick names: -1 ends the turn; anything else must index the legal list.
pub fn resolve(legal: &[Action], pick: i32) -> Option<Action> {
    if pick == -1 {
        return Some(Action::EndTurn);
    }
    usize::try_from(pick).ok().and_then(|i| legal.get(i)).cloned()
}
