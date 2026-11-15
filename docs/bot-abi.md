# Writing an arena bot

An arena bot is a WebAssembly module. The referee calls it once per action with the game as
the bot's seat sees it, and the bot answers with one action. It can ask libaw's engine
questions about its own view while it thinks.

The examples in `bots/` are Rust; `bots/sdk` (crate `arena-bot`) is the Rust side of
everything below. Any language that compiles to a core WebAssembly module with these exports
will do.

## The sandbox

- **Imports:** only `libaw.query`. A module that imports anything else, WASI included, is
  refused before it plays. So a bot has no clock, no randomness, no files and no network: given
  the same input it gives the same answer, which is what makes games replayable.
- **Fuel:** each call gets a budget of 2,000,000,000 fuel (about one per WebAssembly
  instruction, the engine's work on your queries included), the same on any machine. A bot
  that runs out ends its turn.
- **Memory:** 64 MiB.
- **Faults:** a trap (out of fuel, a panic, a bad pointer) or an answer that isn't a legal action
  ends the bot's turn and counts as a strike. The game goes on. After a trap the bot starts
  from a fresh instance, so memory kept between calls is lost.

## Exports

| Export | Signature | What it does |
|---|---|---|
| `memory` | memory | The bot's linear memory. |
| `arena_alloc` | `(len: i32) -> i32` | Returns a pointer to `len` free bytes. The referee writes the input and the engine's answers there. |
| `arena_act` | `(ptr: i32, len: i32) -> i64` | Gets the input (JSON, `len` bytes at `ptr`) and returns its action as JSON: the pointer in the high 32 bits, the length in the low 32. |

## Import

| Import | Signature | What it does |
|---|---|---|
| `libaw.query` | `(ptr: i32, len: i32) -> i64` | Asks the engine a question (JSON at `ptr`). The answer is written into memory from `arena_alloc`; the return value packs its pointer and length as above. |

Questions, answered against the bot's own view:

- `{"q": "legal_actions"}`: every legal action, as a list.
- `{"q": "unit_actions", "unit": ID}`: one unit's actions.
- `{"q": "is_legal", "action": ACTION}`: true or false.

## Input and actions

The input is `{"seat": PLAYER_ID, "day": N, "state": STATE}`, where `STATE` is libaw's state
as the seat sees it (fog hides what it should), the same shape libaw's socket API sends.

Actions use libaw's action JSON (the socket API's), for example:

```json
{"type": "move", "unit_id": 12, "path": [[3, 4], [4, 4]], "then": {"type": "attack", "target": [5, 4]}}
{"type": "build", "at": [2, 7], "unit": "Infantry"}
{"type": "end_turn"}
```

The easiest correct answer is one of the actions `legal_actions` returned.

## In Rust

```rust
use arena_bot::serde_json::{json, Value};

fn act(input: &Value) -> Value {
    let legal = arena_bot::ask(&json!({"q": "legal_actions"}));
    legal[0].clone()
}
arena_bot::bot!(act);
```

Build with `cargo build --release --target wasm32-unknown-unknown` (the crate type is
`cdylib`), then play it:

    arena play --map maps/skirmish.json --bot mine=target/wasm32-unknown-unknown/release/my_bot.wasm --bot greedy=bots/out/greedy.wasm --out game.json
    arena verify game.json

A bot can also bring its own engine: `bots/baseline` compiles libaw into the module and never
asks the host anything.
