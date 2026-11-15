# Proofs (design draft)

Status: a draft for review, not built yet. Open bots don't need any of this: their games are
checked by replaying the record (`arena verify`). Proofs are for games where replaying isn't
possible, because a bot's code is private, or isn't enough.

## What a proof should show

For a game between committed bots, anyone holding only the final proof and the public inputs
can check that:

- every action was legal under libaw's rules;
- each action was the one its bot's committed code chooses from that bot's own view (fog
  applied), so no one helped a bot or let it see hidden information;
- the luck came from the drand round named in the record;
- the result and final state are what the record says.

A private bot is identified by the hash of its code. The proof commits to that hash without
revealing the code.

## Shape

1. **One proof per turn.** The guest program is the referee for one turn. It takes the
   commitment to the state before the turn, the seed, and the bot's code hash. It runs the
   bot sandboxed on its own view, checks and applies each action, and outputs the commitment
   to the state after the turn plus the actions taken. Actions are public, as the game is
   played; nothing is adjudicated privately.
2. **Chain the turns by state commitment.** Turn k's input commitment must equal turn k-1's
   output.
3. **Aggregate a game's turn proofs into one.** A recursion guest runs the verifier over N turn
   proofs, checks the chain, and outputs the first and last commitments and the result. If N is
   large, repeat as a tree.
4. **Verify the final proof in a GitHub Actions job**, next to the game record; later, in a
   browser through WebAssembly.

Luck: the seed comes from a drand round fixed before the game, as for open bots
([arena-rules.md](arena-rules.md)). The beacon value is a public input and is checked outside
the proof against drand's signature, so the referee needs no secret.

## Sizes

libaw's referee, as a RISC-V guest, in RISC Zero's RV32IM user cycles (Nov 2026):

| Workload | Cycles |
| --- | --- |
| One move | 6.84M |
| One turn | 45.5M |
| One game | about 4.9B |

A game is far too large for one monolithic proof on a laptop, which is why the plan proves turns
and aggregates them.

## Prover

The first target is [Jolt](https://github.com/a16z/jolt): MIT-licensed, fast, and actively
developed. At git 8a92874 a turn is 106.4M RV64IMAC cycles and takes 12.9 GB to trace. Jolt has
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
6. Bots are WebAssembly modules today. Inside a RISC-V guest they could run under a WebAssembly
   interpreter, keeping one bot format but costing cycles, or be compiled to RISC-V directly.
   Which is the better trade?
