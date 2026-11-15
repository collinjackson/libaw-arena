# libaw-arena

Bot-vs-bot battles for Advance Wars–style games, run on [libaw](https://github.com/collinjackson/libaw)'s
engine instead of on AWBW, so no bot game touches AWBW's servers. Private while it takes shape;
the tournament's name is still to be settled.

## Try it

    rustup target add wasm32-unknown-unknown
    cargo build --release && scripts/build-bots.sh
    ./target/release/arena round-robin --map maps/skirmish.json --out out/rr \
      --bot greedy=bots/out/greedy.wasm --bot random=bots/out/random.wasm --bot baseline=bots/out/baseline.wasm
    ./target/release/arena verify out/rr/game-*.json

Writing a bot: [docs/bot-abi.md](docs/bot-abi.md). The rules and the game record:
[docs/arena-rules.md](docs/arena-rules.md). `.github/workflows/round-robin.yml` plays the
round-robin weekly and verifies every game.

## What works

- `crates/arena`: the referee (`arena play`, `verify`, `round-robin`). Bots are core WebAssembly
  modules under wasmtime that may import only `libaw.query`; fuel per action, 64 MiB memory,
  faults end the turn as strikes. Records replay from seed and actions alone.
- `bots/`: the Rust SDK and three examples: random, greedy, and baseline (libaw's own policy,
  with the engine compiled into the bot).

## Plan

- **Referee:** libaw's engine plays every game. Each game's dice come from a seed drawn from a
  public randomness beacon (drand) chosen before the game, and the game is saved as a libaw
  storyboard anyone can replay to check the result.
- **Audit trail:** matches run as GitHub Actions in this repo once it is public: pinned commits of
  the referee and every bot, full logs anyone can read, and the replay records as artifacts. The
  referee runs in a container with networking off.
- **Open bots run sandboxed** as WASI modules under wasmtime: no network, no files, a memory cap,
  and a per-move budget in fuel (instructions), so every bot gets the same budget on any machine
  and replays time out at the same point. The engine's questions (`legal_actions`,
  `unit_actions`, `is_legal`) are host functions, so bots stay small.
- **Two leaderboards:**
  - **Verified:** open bots (source public, built and run here) and private bots that prove each
    move in a zkVM (later; the proving work is not here yet).
  - **Unverified:** bots on their owners' servers, called over HTTP for each move. The game is
    fair, but nothing shows the moves were unaided. Games between the two kinds count on the
    unverified board only.
- **Next:** drand seeds, the referee in a container with networking off, publishing standings and
  replays to a page (arena games in libaw's replay viewer), bot submissions, the unverified board.

## License

MIT or Apache-2.0, at your option, like libaw.
