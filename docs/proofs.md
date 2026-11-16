# Proofs (design draft)

Status: a draft for review, partly built. Open bots don't need any of this: their games are
checked by replaying the record (`arena verify`). Proofs are for bots whose code is private.

## A season

1. **Open league.** Open bots play each other in the round-robin. No proofs: anyone can rerun
   the bots and replay the records.
2. **Qualifying round.** Each private bot plays a scheduled set of games against the open field.
   Its owner plays and proves every game themselves, and the arena verifies the proofs. This
   ranks private bots with no trusted judge.
3. **Finals.** The top 8 private bots are invited to a live cut, playing each other, with a
   judge: AWBW itself, or one trusted with the code (below).

## Who holds what

- **Open bots** publish their source and their built module.
- **Private bots** never send their code to the arena. A module can be decompiled, so the arena
  can't hold it. The owner proves in their own (private) repo's GitHub Actions, or on their own
  machine, and sends the arena only proofs. In the finals, a bot either plays on AWBW from its owner's machine or is shared with a judge.
- **The arena** holds nothing secret and is trusted for nothing. It schedules games, checks
  each proof against a pinned program id, and publishes the results. Anyone can repeat the
  check, so a change to the arena's workflow can't change a result unnoticed. The only thing to
  trust is the verifier, and the site's verifier is public code anyone can run themselves.

## Why the proven program is the arena's

A proof that "the program with id P produced these moves" says nothing useful if P is the
owner's own program: it could read an extra private input, such as a human's advice, and its id
wouldn't reveal that. So the proven program is the arena's, built reproducibly from a tag of
this repo, with a known id. It takes the private bot's module as a private input, checks it
against the registered commitment, and runs it in the same sandbox as the open league (only
`libaw.ask`, fuel, memory). The bot's questions are answered by libaw as native code; only the
bot's own logic runs under the WebAssembly interpreter ([bot-abi.md](bot-abi.md)).

A private bot registers `{"name", "repo", "commitment"}`, where the commitment is
sha256(salt ‖ module), so a module simple enough to guess can't be confirmed by hashing
candidates. The commitment is fixed for the season; an update is a new entry.

## Qualifying: the owner proves whole games

Against an open bot, the owner can run everything: the referee, the open bot (its module is
public) and their own. The **game prover** plays a whole game and proves that the game between
open bot O and private bot C, with dice from drand round R, followed libaw's rules, with each
bot seeing only its own view, and ended in result X. It outputs O's module hash, C, R, the
result and the final state's hash, and optionally the moves, for a replay.

- **Fog works.** The prover holds the full state privately and the proof shows that each bot saw
  only its own view.
- **One proof per game**, made by one party. No turn-by-turn exchange, nobody has to be online.
- **No cherry-picking.** The arena schedules each game. Its dice come from a drand round after
  the bot's commitment is registered, so neither the bot nor its owner could have tuned it to
  that game's luck. A scheduled game not proven by its deadline is a forfeit, so an owner can't
  submit only the wins.
- **Entries are capped per account**, and every entry is shown with its forfeits, so registering
  many bots to keep the best doesn't hide anything.

The proof is wrapped as a **Groth16** proof (a few hundred bytes), and the arena's site verifies
it in the browser, next to the game's replay. Anyone can verify it again.

## Finals: a judge

Between two private bots, neither owner can run the other's code. Proving turn by turn doesn't
work either: no game state is fully public, since even without fog dived Subs and hidden
Stealths are invisible to the other side, so an owner can't prove legal moves against units they
can't see.

So the finals need a judge. Two ways:

- **AWBW as the judge.** The finalists play real AWBW games, invite-only, with AWBW's own engine
  as referee and its replays as the record. Each owner runs their own bot against AWBW (the
  arena would provide the runner that connects a module to an AWBW game, as FlakBot does), so
  no one shares code at all. Nothing is proven, but AWBW is the reference the rules come from,
  and it tests how closely libaw follows them. Needs AWBW's OK, since bot games otherwise stay
  off its servers.
- **A judge trusted with the code.** Owners hand their modules to a judge, who plays the games
  and runs the same game prover as in qualifying. The judge is trusted to keep the code secret
  and with nothing else: each game still comes with a Groth16 proof. Later the judge could run
  in a confidential VM whose attestation shows the code it runs.

Luck: the seed comes from a drand round fixed in advance, as for open bots
([arena-rules.md](arena-rules.md)). The beacon value is a public input, checked outside the proof
against drand's signature.

## Sizes

libaw's referee, as a RISC-V guest, in RISC Zero's RV32IM user cycles (Nov 2026), on
[libaw-zkbench](https://github.com/collinjackson/libaw-zkbench)'s synthetic mid-game workloads:

| Workload | Cycles |
| --- | --- |
| One move | 4.6M |
| One turn (18 actions) | 28.9M |
| 15 days (684 actions) | 1.5B |

These are libaw's own cycles, without a bot. A bot's own logic adds its interpreted cycles on
top. With the binary interface ([bot-abi.md](bot-abi.md)), a whole turn through the turn prover
is 71M cycles for random, 282M for greedy and 387M for a bot that takes the host's `BASELINE`
suggestion; the JSON interface it replaced cost 2.7–2.8B for the first two. A bot that compiles
libaw into itself pays for the engine interpreted: 21.5B. A qualifying game is two bots for
30–60 turns, so on the order of 5–25B cycles for simple bots.

## Prover

The first target is [RISC Zero](https://github.com/risc0/risc0): it splits a long run into
segments and joins their proofs into one receipt, so a whole game is one proof, and it wraps a
receipt as Groth16. [Jolt](https://github.com/a16z/jolt) is the one to watch: MIT-licensed and
fast, but with no continuations yet (at git 8a92874 its tracer keeps the whole trace in memory:
12.9 GB for a turn of 106.4M cycles), and its stated direction is a streaming prover with
bounded memory.

## Open questions

1. How long does proving a qualifying game take on a GitHub-hosted runner (4 cores, 16 GB, 6-hour
   jobs)? If a game doesn't fit, prove stretches of turns and join them.
2. Is chaining turns by a state-hash public input enough, or should memory consistency across
   chunks be argued more directly?
3. Is checking the drand beacon outside the proof (beacon value as a public input) sound here,
   or should the guest verify the BLS signature?
4. The prover interprets the bot's WebAssembly. Translating it to RISC-V ahead of time would be
   faster, but then the proven program would depend on the private module, and its id would no
   longer be a public constant. Is there a way to keep the speed and still rule out extra
   inputs?
5. Private against private without a judge: committed hidden state and an interactive protocol,
   as in on-chain fog games. At what cost per turn?
