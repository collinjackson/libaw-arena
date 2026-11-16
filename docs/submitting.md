# Entering a bot

The registry is [`bots.json`](../bots.json) in this repo. There is no sign-up site: a pull request
enters a bot, the file's history is the public record of every bot and version, and an agent can
do it as easily as a person.

## Open bots

You build your bot and publish the module; the arena never builds or runs your code outside its
sandbox. The bot is one core WebAssembly module ([bot-abi.md](bot-abi.md); `bots/sdk` is the
Rust side) in a public GitHub repo of yours, built from the commit you name.

The quickest start is [`templates/bot`](../templates/bot): copy it into a new repo, change
`src/lib.rs`, and push a tag (`git tag v1 && git push origin v1`). Its release workflow builds the
module, attaches it to a GitHub release, and writes the entry below into the release notes.

To enter it, open a pull request adding:

```json
{"name": "my-bot",
 "repo": "https://github.com/OWNER/REPO",
 "commit": "FULL_40_HEX_COMMIT",
 "wasm": "https://github.com/OWNER/REPO/releases/download/v1/my-bot.wasm",
 "sha256": "64_HEX_SHA256_OF_THE_MODULE"}
```

- `name`: 1–32 of `a-z 0-9 _ -`, unique in the file.
- `commit`: the full id of the commit the module was built from, so readers can see its source.
- `wasm`: where the module is, in the same repo: a release asset
  (`https://github.com/OWNER/REPO/releases/download/TAG/FILE`) or a file at a commit
  (`https://raw.githubusercontent.com/OWNER/REPO/COMMIT/PATH`). At most 32 MiB.
- `sha256`: the module's sha256 (`sha256sum my-bot.wasm`). The arena downloads the module and
  refuses it unless the hash matches, so replacing the file later changes nothing; the hash is
  the bot.

Before you open it, you can play your module locally against the examples:

    arena play --map maps/skirmish.json --bot mine=my-bot.wasm --bot greedy=bots/out/greedy.wasm --out game.json
    arena verify game.json

The pull request's check downloads your module and plays it against the example bots from both
seats in every mode. The log shows the games, any strikes (illegal actions, faults, running out of
fuel) and the records, which you can download and watch with `arena page`.

To **update** your bot, publish a new module and open a pull request changing its `commit`,
`wasm` and `sha256`.

**One bot per pull request**, with only the five fields above.

**Merging is automatic** when the check is clean: the pull request changes only `bots.json`
and only one bot in it, you own the bot's GitHub repo (to remove a bot: you own its repo or
you're the one who added it), the file is valid JSON with nothing else changed, and in every mode your bot
loaded, played real moves (not just ending its turn) and got no strikes, with every game
verifying. Anything else waits for a maintainer, and a comment says why. After a merge the
round-robin runs again with the new version, and every game record names the repo and commit
each bot says it was built from, next to the sha256 of the module that played.

Only the owner of a bot's repo should enter or update it.

The arena doesn't rebuild your module from source, so `commit` is your word for what it was
built from. The template pins the Rust toolchain and the SDK so that anyone can rebuild it and
compare hashes.

## Private and API bots

Not yet. Private bots that prove their moves in a zkVM, and bots on their owners' servers called
over HTTP, will have entries of their own and their own leaderboard (see the README's plan).
