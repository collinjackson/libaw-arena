# Writing an arena bot

An arena bot is a WebAssembly module. For each action the referee asks it to pick one of its
seat's legal actions, and the bot answers with an index into the legal list. While it thinks it
can ask the host about its own view of the game. Answers are fixed-size binary records that the
bot reads in place, so there is no JSON to parse. That keeps the bot's fuel, and a private bot's
proving cost ([proofs.md](proofs.md)), for deciding.

The examples in `bots/` are Rust; `bots/sdk` (crate `arena-bot`) is the Rust side of
everything below, and [templates/bot](../templates/bot) is a starting point. Any language that
compiles to a core WebAssembly module with these exports will do.

## The sandbox

- **Imports:** only `libaw.ask`. A module that imports anything else, WASI included, is refused
  before it plays. So a bot has no clock, no randomness, no files and no network: given the same
  position it gives the same answer, which is what makes games replayable.
- **Fuel:** each call gets a budget of 2,000,000,000 fuel (about one per WebAssembly
  instruction), the same on any machine. The host's work answering questions costs no fuel. A
  bot that runs out ends its turn.
- **Memory:** 64 MiB.
- **Faults:** a trap (out of fuel, a panic, a bad pointer) or a pick that isn't in the list ends
  the bot's turn and counts as a strike. The game goes on. After a trap the bot starts from a
  fresh instance, so memory kept between calls is lost.

## Exports

| Export | Signature | What it does |
|---|---|---|
| `memory` | memory | The bot's linear memory. |
| `arena_alloc` | `(len: i32) -> i32` | Returns a pointer to `len` free bytes. The host writes its answers there. |
| `arena_pick` | `(seat: i32) -> i32` | Picks the next action: an index into the `LEGAL` answer, or -1 to end the turn. |

## Import

| Import | Signature | What it does |
|---|---|---|
| `libaw.ask` | `(kind: i32, arg: i32) -> i64` | Asks the host a question about the seat's own view. The answer is written into memory from `arena_alloc`; the return value packs its pointer (high 32 bits) and length (low 32). |

## Answers

All numbers are little-endian. A list starts with its count (u32), then that many records of a
fixed size. Positions are (x, y) bytes; 255 means none. Player ids are u32; 0 means none.

| Kind | Answer |
|---|---|
| 0 `LEGAL` | The legal actions, 20 bytes each: kind (0 end turn, 1 move, 2 build, 3 power); then, for a move (0 wait, 1 attack, 2 capture, 3 load, 4 join, 5 supply, 6 repair, 7 unload, 8 attack a pipe seam, 9 hide, 10 unhide, 11 launch, 12 delete); unit type (the moving unit's, or what's built; for a power, 1 CO power, 2 super power); path length; unit id (u32); from x, y; to x, y; target x, y; cargo id for an unload (u32); 2 reserved. |
| 1 `UNITS` | Every unit the seat can see, 16 bytes each: id (u32), owner (u32), type, x, y, HP shown (1–10), ammo, fuel (both capped at 255), flags (1 moved, 2 carried, 4 dived, 8 stunned), 1 reserved. |
| 2 `BUILDINGS` | Every building, 8 bytes each: x, y, terrain id (u16), owner (u32). |
| 3 `GAME` | Width, height (u16 each), day, the seat to move (u32 each), then the players, 16 bytes each: id, funds, power meter (u32 each), turn order, eliminated, power on (0 none, 1 CO power, 2 super), 1 reserved. |
| 4 `TERRAIN` | Every tile's terrain id, row by row (u16 each, no count). |
| 5 `VIEW` | The whole view as libaw's `State` in [postcard](https://docs.rs/postcard), for a bot that brings libaw's engine. It decodes with the libaw commit in this repo's `Cargo.lock`; pin the same one. |
| 6 `BASELINE` | libaw's baseline policy's pick for this position, as an index into `LEGAL` (u32; 0xFFFFFFFF to end the turn), worked out by the host. |

Unit types are libaw's `UnitType` in order (`data/units.json`), and terrain ids are AWBW's. The
legal list is libaw's `legal_actions` on the seat's view, in libaw's order, and the index a bot
picks refers to that decision's list.

## In Rust

```rust
use arena_bot::{ATTACK, END_TURN, KIND_MOVE};

fn pick(_seat: u32) -> i32 {
    let legal = arena_bot::legal();
    legal.iter().position(|a| a.kind == KIND_MOVE && a.then == ATTACK).map_or(END_TURN, |i| i as i32)
}
arena_bot::bot!(pick);
```

Build with `cargo build --release --target wasm32-unknown-unknown` (the crate type is
`cdylib`), then play it:

    arena play --map maps/skirmish.json --bot mine=target/wasm32-unknown-unknown/release/my_bot.wasm --bot greedy=bots/out/greedy.wasm --out game.json
    arena verify game.json

## What things cost

A bot's own code is what costs, in fuel here and in cycles when a private bot's turn is proven.
The host's answers are native code. One turn on a mid-game position (18 actions), in RISC Zero
cycles, with the turn prover ([libaw-zkbench](https://github.com/collinjackson/libaw-zkbench)'s
`turn` workload):

| Bot | What it does | Cycles |
|---|---|---|
| random | picks from `LEGAL` | 71M |
| greedy | scores every legal action against the enemy units | 282M |
| hinted | takes `BASELINE` | 387M |
| baseline | libaw compiled into the module, reading `VIEW` | 21.5B |

libaw's own work for the same turn is 28.9M. A bot that brings its own engine pays for running
that engine interpreted. Asking the host costs far less.
