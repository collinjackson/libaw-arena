# libaw-arena

Bot-vs-bot battles for Advance Wars–style games, run on [libaw](https://github.com/collinjackson/libaw)'s
engine instead of on Advance Wars By Web, so no bot game touches AWBW's servers. Open bots run
sandboxed as WebAssembly, every game is played by GitHub Actions with public logs, and every
result can be checked from its record.

**Enter a bot:** open a pull request to [`bots.json`](bots.json) ([how](docs/submitting.md)).
**Keeping it running:** [docs/maintaining.md](docs/maintaining.md).

## Try it

    rustup target add wasm32-unknown-unknown
    cargo build --release && scripts/build_registry.py
    ./target/release/arena round-robin --map maps/skirmish.json --mode fog --manifest bots/out/manifest.json --out out/rr
    ./target/release/arena verify out/rr/game-*.json

`arena page out/rr/standard out/rr/fog ... --out out/site` makes a standings page linking every turn into libaw's
replay viewer (the workflow adds the viewer; locally, link libaw's `web` and `data` into
`out/site/libaw/`). Or one game: `arena storyboards out/rr/GAME.json --out out/sb` writes
one storyboard per turn; serve a folder holding both repos and open
`libaw/web/replay.html?sb=/libaw-arena/out/sb/turn-005.json&art=awbw`.

Writing a bot: [docs/bot-abi.md](docs/bot-abi.md). Entering one: a pull request to
[`bots.json`](bots.json), the registry ([docs/submitting.md](docs/submitting.md)). The rules and the game record:
[docs/arena-rules.md](docs/arena-rules.md). `.github/workflows/round-robin.yml` plays the
round-robin weekly and verifies every game.

## What works

- `crates/arena`: the referee (`arena play`, `verify`, `round-robin`, `storyboards`, `page`). Bots are core WebAssembly
  modules under wasmtime that may import only `libaw.query`; fuel per action, 64 MiB memory,
  faults end the turn as strikes. Records replay from seed and actions alone.
- `bots/`: the Rust SDK and three examples: random, greedy, and baseline (libaw's own policy,
  with the engine compiled into the bot).

## Leagues

- **Open bots** (live): source public, built from a pinned commit and run here in the sandbox.
- **Private bots** (planned): closed source, with a zero-knowledge proof for each move that the
  bot's committed code picked it from the legal moves the referee supplied. The rules don't need
  proving (the referee is open); the proof shows no one helped the bot.
- **API bots** (planned): bots on their owners' servers, called over the network at a normal
  pace. Fair games, but unverified, so a separate leaderboard; games between kinds count there.

Each league plays Standard, High Funds and Fog ([modes.json](modes.json)) and shows win rates
per mode. Open and private leagues play as fast as the machines allow and re-run whenever a bot
changes.

## Next

- Merge owner-only bot updates automatically on a green check.
- The referee in a container with networking off while it plays.
- The private and API leagues; more maps; a history of past events on the site.

## License

MIT or Apache-2.0, at your option, like libaw.
