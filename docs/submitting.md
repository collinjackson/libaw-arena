# Entering a bot

The registry is [`bots.json`](../bots.json) in this repo. There is no sign-up site: a pull request
enters a bot, the file's history is the public record of every bot and version, and an agent can
do it as easily as a person.

## Open bots

Your bot's source lives in a public GitHub repo of yours, and builds with
`cargo build --release --target wasm32-unknown-unknown` to one core WebAssembly module (see
[bot-abi.md](bot-abi.md); `bots/sdk` is the Rust side). To enter it, open a pull request adding:

```json
{"name": "my-bot", "repo": "https://github.com/OWNER/REPO", "commit": "FULL_40_HEX_COMMIT", "path": "."}
```

- `name`: 1–32 of `a-z 0-9 _ -`, unique in the file.
- `commit`: a full commit id. The arena builds exactly that commit, every time, until you change it.
- `path`: the bot's crate inside the repo (`.` if it's at the top).

The pull request's check builds your bot at that commit and plays it against the example bots
from both seats. The log shows the games, any strikes (illegal actions, faults, running out of
fuel) and the records, which you can download and watch with `arena page`.

To **update** your bot, open a pull request changing its `commit`. Once merged, the round-robin
runs again with the new version, and every game record names the repo and commit each bot was
built from, next to the sha256 of the module that played.

Only the owner of a bot's repo should enter or update it.

## Private and API bots

Not yet. Private bots that prove their moves in a zkVM, and bots on their owners' servers called
over HTTP, will have entries of their own and their own leaderboard (see the README's plan).
