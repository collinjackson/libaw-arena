//! Write an arena bot in Rust: a function that picks one of the seat's legal actions. Build for
//! wasm32-unknown-unknown (see docs/bot-abi.md).
//!
//! ```ignore
//! fn pick(_seat: u32) -> i32 {
//!     let legal = arena_bot::legal();
//!     legal.iter().position(|a| a.then == arena_bot::ATTACK).map_or(arena_bot::END_TURN, |i| i as i32)
//! }
//! arena_bot::bot!(pick);
//! ```
//!
//! Answers are fixed-size little-endian records read in place: no JSON, so a bot spends its fuel
//! (and, in a proof, its cycles) on deciding rather than parsing.

#[link(wasm_import_module = "libaw")]
extern "C" {
    fn ask(kind: i32, arg: i32) -> i64;
}

/// What `pick` returns to end the turn; anything else is an index into `legal()`.
pub const END_TURN: i32 = -1;

/// `Legal::kind`
pub const KIND_END_TURN: u8 = 0;
pub const KIND_MOVE: u8 = 1;
pub const KIND_BUILD: u8 = 2;
pub const KIND_POWER: u8 = 3;
/// `Legal::then`, for a move
pub const WAIT: u8 = 0;
pub const ATTACK: u8 = 1;
pub const CAPTURE: u8 = 2;
pub const LOAD: u8 = 3;
pub const JOIN: u8 = 4;
pub const SUPPLY: u8 = 5;
pub const REPAIR: u8 = 6;
pub const UNLOAD: u8 = 7;
pub const ATTACK_SEAM: u8 = 8;
pub const HIDE: u8 = 9;
pub const UNHIDE: u8 = 10;
pub const LAUNCH: u8 = 11;
pub const DELETE: u8 = 12;

/// The host writes its answers into memory it asks for here.
#[no_mangle]
pub extern "C" fn arena_alloc(len: i32) -> i32 {
    let mut v = Vec::<u8>::with_capacity(len as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p as i32
}

/// The raw answer to `ask(kind, arg)` (docs/bot-abi.md has every format).
pub fn raw(kind: i32, arg: i32) -> Vec<u8> {
    let r = unsafe { ask(kind, arg) };
    let (p, n) = ((r as u64 >> 32) as usize, (r as u64 & 0xffff_ffff) as usize);
    // Safety: the host wrote n bytes at p, into a buffer of capacity n from arena_alloc.
    unsafe { Vec::from_raw_parts(p as *mut u8, n, n) }
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn records(b: &[u8], size: usize) -> impl Iterator<Item = &[u8]> {
    let n = if b.len() >= 4 { u32_at(b, 0) as usize } else { 0 };
    b.get(4..4 + n * size).unwrap_or(&[]).chunks_exact(size)
}
fn pos(x: u8, y: u8) -> Option<(u8, u8)> {
    (x != 255).then_some((x, y))
}

/// One legal action. `pick` answers with its index in `legal()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Legal {
    pub kind: u8,
    /// For a move: what the unit does where it stops (WAIT, ATTACK, ...).
    pub then: u8,
    /// The moving unit's type, or what's built; for a power, 1 CO power, 2 super power.
    pub unit_type: u8,
    /// Tiles in the move's path, the start included.
    pub path_len: u8,
    pub unit: u32,
    pub from: Option<(u8, u8)>,
    pub to: Option<(u8, u8)>,
    /// What an attack, repair, launch or unload aims at.
    pub target: Option<(u8, u8)>,
    pub cargo: u32,
}

/// The seat's legal actions, in the order `pick` indexes.
pub fn legal() -> Vec<Legal> {
    let b = raw(0, 0);
    records(&b, 20).map(|r| Legal {
        kind: r[0], then: r[1], unit_type: r[2], path_len: r[3], unit: u32_at(r, 4),
        from: pos(r[8], r[9]), to: pos(r[10], r[11]), target: pos(r[12], r[13]), cargo: u32_at(r, 14),
    }).collect()
}

/// A unit the seat can see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unit {
    pub id: u32,
    pub owner: u32,
    pub unit_type: u8,
    pub x: u8,
    pub y: u8,
    /// Shown HP, 1-10.
    pub hp: u8,
    pub ammo: u8,
    pub fuel: u8,
    /// 1 moved, 2 carried, 4 dived, 8 stunned.
    pub flags: u8,
}

pub fn units() -> Vec<Unit> {
    let b = raw(1, 0);
    records(&b, 16).map(|r| Unit {
        id: u32_at(r, 0), owner: u32_at(r, 4), unit_type: r[8], x: r[9], y: r[10], hp: r[11], ammo: r[12], fuel: r[13], flags: r[14],
    }).collect()
}

/// A building: where, its terrain id, its owner (0 if none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Building {
    pub x: u8,
    pub y: u8,
    pub terrain: u16,
    pub owner: u32,
}

pub fn buildings() -> Vec<Building> {
    let b = raw(2, 0);
    records(&b, 8).map(|r| Building { x: r[0], y: r[1], terrain: u16_at(r, 2), owner: u32_at(r, 4) }).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: u32,
    pub funds: u32,
    pub meter: u32,
    pub order: u8,
    pub eliminated: bool,
    /// 0 none, 1 CO power, 2 super power.
    pub power: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub width: u16,
    pub height: u16,
    pub day: u32,
    pub current: u32,
    pub players: Vec<Player>,
}

pub fn game() -> Game {
    let b = raw(3, 0);
    let n = u32_at(&b, 12) as usize;
    let players = b[16..16 + n * 16].chunks_exact(16).map(|r| Player {
        id: u32_at(r, 0), funds: u32_at(r, 4), meter: u32_at(r, 8), order: r[12], eliminated: r[13] != 0, power: r[14],
    }).collect();
    Game { width: u16_at(&b, 0), height: u16_at(&b, 2), day: u32_at(&b, 4), current: u32_at(&b, 8), players }
}

/// Every tile's terrain id, row by row.
pub fn terrain() -> Vec<u16> {
    raw(4, 0).chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
}

/// The whole view as libaw's State in postcard, for a bot that brings libaw's engine.
pub fn view_bytes() -> Vec<u8> {
    raw(5, 0)
}

/// libaw's baseline policy's choice for this position, as an index into `legal()` (or END_TURN),
/// computed natively by the host: much cheaper than bringing libaw into the bot.
pub fn baseline() -> i32 {
    let b = raw(6, 0);
    match b.get(..4) {
        Some(x) => match u32::from_le_bytes([x[0], x[1], x[2], x[3]]) {
            u32::MAX => END_TURN,
            i => i as i32,
        },
        None => END_TURN,
    }
}

/// Exports `arena_pick` calling `f(seat) -> index into legal(), or END_TURN`.
#[macro_export]
macro_rules! bot {
    ($f:path) => {
        #[no_mangle]
        pub extern "C" fn arena_pick(seat: i32) -> i32 {
            $f(seat as u32)
        }
    };
}
