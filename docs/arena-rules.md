# Arena rules

- **Seats:** bots take the seats in turn order. In a round-robin every pair plays twice, once
  from each seat.
- **What a bot sees:** its own view of the game (fog applies), and the engine's answers about
  that view.
- **One action at a time.** An illegal action, a fault (out of fuel, a trap, a reply that isn't
  an action) or a module that can't be started ends that bot's turn and counts as a **strike**.
  The game goes on. At most 400 actions a turn; after that the turn is ended for the bot.
- **The end:** one side left standing wins. After day 30 the game is a draw.
- **Luck:** every roll comes from ChaCha8 seeded with the game's seed, as in libaw's server.
  The seed is part of the game record. In the weekly event it comes from the
  [drand](https://drand.love) quicknet round published right after the bots are built: game i's
  seed is the first 8 bytes (big-endian) of sha256(`libaw-arena/RANDOMNESS/i`). The record's
  `beacon` entry names the round, and `arena verify` checks the seed against it; anyone can
  fetch the round from `https://api.drand.sh/52db9ba70e0cc0f6eaf7803dd07447a1f5477735fd3f661792ba94600c84e971/public/ROUND`
  and check the randomness. The workflow log shows the round drawn, and standings.json repeats it.
- **Standings:** 2 points a win, 1 a draw.

## Game records

`arena play` and `arena round-robin` write each game as JSON (format `libaw-arena-game`,
version 1): the seed, the rules, each bot's name and the sha256 of its module, the starting
state, every action with the fuel it took, strikes, the result and a hash of the final state.

`arena verify GAME.json` replays a record from its start, seed and actions alone, with no bots,
and checks the result and the final state. Anyone can check any game this way.
