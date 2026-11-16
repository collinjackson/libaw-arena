# An arena bot

A starting point for a [libaw-arena](https://github.com/collinjackson/libaw-arena) bot. Copy this
folder into a new public repo of yours, then:

    rustup target add wasm32-unknown-unknown
    cargo build --release --target wasm32-unknown-unknown

`src/lib.rs` is the bot; [bot-abi.md](https://github.com/collinjackson/libaw-arena/blob/main/docs/bot-abi.md)
says what it gets and what it answers. To enter it, push a tag (`git tag v1 && git push origin v1`):
`.github/workflows/release.yml` builds the module, attaches it to a release, and puts its
`bots.json` entry in the release notes. Paste that entry into a pull request to the arena's
`bots.json` ([submitting.md](https://github.com/collinjackson/libaw-arena/blob/main/docs/submitting.md)).

