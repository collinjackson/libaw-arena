# Keeping the arena running

What the arena runs on, what needs a maintainer, and how to fork it.

## What runs, and what it costs

- **GitHub Actions** plays every event (on any change to a bot or the referee, and weekly) and
  checks every pull request to the registry. **GitHub Pages** serves the site the event builds.
  Both are free for public repositories.
- **drand** supplies the luck: a public beacon run by a league of independent organizations,
  read with one unauthenticated request.
- Nothing else: no server, no database, no secrets, no accounts to renew, no domain to pay for.
  The weekly event keeps the build cache warm and the site fresh.

## What needs a person

- **Names:** auto-merge can't judge a misleading or rude bot name; rename or remove one by
  pull request if needed.
- **Updates and new bots by their owners** merge themselves when the check is clean
  (`auto-merge.yml`, rules in `scripts/automerge.py`). The rest wait for a maintainer.
- **Rule or engine changes** (the libaw version in Cargo.lock, rust-toolchain.toml, the map
  pool) are seasons: change them on purpose, all at once, and say so in the release notes,
  because they change every bot's results.

## Forking

- Every game record replays without us: `arena verify GAME.json` needs only the record.
- Everything pins what it builds: bots by commit, the referee's dependencies by Cargo.lock, Rust
  by rust-toolchain.toml. A fork years from now rebuilds the same modules.
- Anyone can fork the repo, turn on Actions and Pages, and have the whole arena, history
  included, under a new name.
- Please keep more than one maintainer with merge rights, ideally in a GitHub organization, so
  the project never depends on one person's account.
