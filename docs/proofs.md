# Proofs (design draft)

Status: a draft for review, not built yet. Open bots don't need any of this: their games are
checked by replaying the record (`arena verify`). Proofs are for bots whose code is private.

## Who holds what

- **Open bots** publish their source and their built module; anyone can run them, and replaying
  the record checks a game. No proofs needed.
- **Private bots** never send their code anywhere. A module can be decompiled, so the arena
  can't hold it either. The bot's owner proves the bot's turns in their own (private) repo's
  GitHub Actions, or on their own machine, and sends the arena only proofs.
- **The arena** holds nothing secret and is trusted for nothing. It publishes the game, checks
  each proof against a pinned program id, and replays the public actions with libaw. Anyone
  can repeat both checks, so a change to the arena's workflow can't change a result
  unnoticed.

## What a turn proof shows

That the actions taken this turn are the ones the bot with hash H chooses, from the game as its
seat sees it, with no other input, under libaw's rules.

"No other input" is the hard part. If the proven program were the owner's own code, it could
read an extra private input (a human's advice, say), and its program id wouldn't reveal that. So
the proven program is the arena's, public and built reproducibly from a tag of this repo, so
its program id is known: the **turn prover**. It takes:

- public: the state commitment before the turn, the drand beacon value, the bot hash H;
- private: the bot's WebAssembly module.

It checks that the module hashes to H, runs it in the same sandbox as the open league (only
`libaw.query`, fuel, memory) on the seat's view, applies each action with libaw, and outputs
the actions, the state commitment after the turn and H. The bot's queries to libaw run as
native code in the prover; only the bot's own logic runs under the WebAssembly interpreter.

The arena verifies the proof against the turn prover's id, checks that the commitment before the
turn is the last turn's commitment after, and that H is the hash the bot registered.

## A game

1. A private bot registers `{"name", "repo", "hash"}`: the sha256 of its module, fixed for the
   season (an update is a new hash). Its owner installs the libaw-arena GitHub App on the bot's
   repo and adds `prove-turn.yml` from [templates/bot](../templates/bot). The app can start and
   read that repo's workflow runs, and nothing else; it never sees the code.
2. On each of the bot's turns, the arena publishes the state to move from (on its `turns`
   branch; the beacon round is fixed before the turn starts) and starts the owner's
   `prove-turn` workflow through the app. That run is the turn notification.
3. The owner's run fetches the state, runs the turn prover, and uploads the turn record and its
   proof as an artifact. The arena's job waits for that run, downloads the artifact, verifies
   it, and plays on. Actions minutes are free in a public repo, so the waiting costs nothing.
4. A turn not proven in time ends the turn, like a strike.

An owner can also prove on their own machine: the same workflow on a self-hosted runner
(`runs-on: self-hosted`), which also avoids the Actions minutes a private repo pays for.

`.github/workflows/turn.yml` and the template's `prove-turn.yml` run this loop today without the
proof: the owner's run plays the turn with `arena turn`, and the arena checks it with
`arena check-turn`.

Games are asynchronous, like AWBW's, because each turn waits for its owner's prover. Two
private bots can play each other without either seeing the other's code.

Luck: the seed comes from a drand round fixed before the turn, as for open bots
([arena-rules.md](arena-rules.md)). The beacon value is a public input and is checked outside
the proof against drand's signature, so the prover needs no secret.

**Fog** is not in the first version. With fog the state is not public, so the owner's prover
can't see the hidden half, and the arena can't replay the game in public. That needs each side's
hidden units committed and an interactive protocol (as in on-chain fog games). The private league
starts with Standard and High Funds only.

Later, a game's turn proofs can be aggregated into one (a recursion guest that verifies N turn
proofs and checks the chain), so a game is checked by verifying one proof.

## Sizes

libaw's referee, as a RISC-V guest, in RISC Zero's RV32IM user cycles (Nov 2026), on
[libaw-zkbench](https://github.com/collinjackson/libaw-zkbench)'s synthetic mid-game workloads:

| Workload | Cycles |
| --- | --- |
| One move | 4.6M |
| One turn (18 actions) | 28.9M |
| 15 days (684 actions) | 1.5B |

These are libaw's own cycles, without a bot. A private bot's own logic adds its interpreted
cycles on top; that cost is the first thing to measure. A game is far too large for one proof
on a laptop, which is another reason to prove turns.

## Prover

The first target is [Jolt](https://github.com/a16z/jolt): MIT-licensed, fast, and actively
developed. At git 8a92874 the benchmark turn is 48.3M RV64IMAC cycles; its tracer keeps the whole trace in memory (12.9 GB for an earlier, busier turn of 106.4M). Jolt has
no folding or continuations today. Recursion means running the Jolt verifier as a guest
(`examples/recursion`, Dory commitments). For long runs Jolt's stated direction is a streaming
prover with bounded memory, which would let chunks grow from turns to longer stretches.

## Open questions

1. Is per-turn proving plus verifier-in-guest aggregation the right shape for Jolt? Or is
   folding over Jolt's sumcheck instances realistic?
2. Is chaining turns by a state-hash public input enough, or should memory consistency across
   chunks be argued more directly?
3. Dory or a lattice commitment for the aggregation layer? What verifier cost per inner proof
   should we expect?
4. Is checking the drand beacon outside the proof (beacon value as a public input) sound here,
   or should the guest verify the BLS signature?
5. Can a final Jolt proof be verified in a browser at reasonable cost, or should it be wrapped
   (for example in Groth16) first?
6. The turn prover interprets the bot's WebAssembly. Translating it to RISC-V ahead of time would
   be faster, but then the proven program would depend on the private module, and its id would no
   longer be a public constant. Is there a way to keep the speed and still rule out extra
   inputs?
7. A GitHub-hosted runner (4 cores, 16 GB) has to prove a turn in reasonable time. Which zkVM
   does that best for a turn of ~30M cycles plus the bot's own?
