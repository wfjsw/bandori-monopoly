# Match engine (P3)

`crates/game-core/src/engine/` — the port of `MatchHost`'s game shell. Runs natively
(server) and in wasm32 (browser).

## Decisions

* **Behaviour, not bits.** The port reproduces the game's rules, not its exact numbers.
  The RNG is our own seeded xoshiro256\*\* (`rng.rs`), not .NET `System.Random`. The
  .NET golden-master oracle from the original plan is dropped; verification is by
  scenario tests and invariants instead.
* **Determinism is still required**: the same seed and the same inputs give the same
  game on server and browser. Replay depends on it.
* **Replay instead of coroutines.** The C# runs game flow as nested coroutines that
  suspend at prompts. Here flow is straight-line Rust over a `Cx`. `Cx::ask` returns
  the logged answer, or halts the routine; the host shows the prompt and later re-runs
  the routine from its starting snapshot with the answer appended. Card effects
  (`ruleset.wasm`) use the same model, so engine prompts and card prompts are one
  mechanism.

## Structure

| File | Contents |
|---|---|
| `mod.rs` | `Match` (host): clocks, live prompt (bot answers, time-outs, auction bidding), end-match vote, pending routine + snapshot + answer log, deferred host events, commands (`act`) |
| `world.rs` | `World` — everything a routine touches, incl. RNG and event/prompt counters |
| `cx.rs` | `Cx` routine context, `Flow`/`Halt`, `Ask` prompt builders |
| `play.rs` | routines: turns, movement, CiRCLE reward, landing, agents, rent, forced purchase, buy/build, payments, raising funds, bankruptcy, auctions, hand/draw, events, play card, scoring |
| `ai.rs` | bot decisions (same thresholds as the C#) |
| `setup.rs` | turn order, ban (Ranked), pick, deck |
| `rules.rs` | `CardRules` — where card/event content plugs in; `StubRules` = no effects |

Routines: `Opening`, `NextTurn`, `Ai(seat)`, `Act(seat, command)`, `Leftovers(deeds)`.

### Why host events are deferred

The world's event and prompt counters are re-derived on every replay, so ids stay
stable and clients never see duplicates. Anything the *host* logs (vote messages,
disconnects, time-outs) while a routine is paused would steal an id the replay later
hands to a different event. Those changes are queued and applied once the routine
commits.

## Not in the shell (card content / later phases)

| What | C# | Where it goes |
|---|---|---|
| Card, skill, band and `Fx` hooks (`PayAdd`, `CircleRewardChoice`, `SkipTile`, `BuyPrice`...) | nested classes | card rules (WASM) |
| Active events (`EvOn(...)`: 协助CiRCLE重建, Forbidden Moca, ...) | `EventEffect` | card rules |
| Per-seat card variables (`V(i, ...)`), marks, embers, field cards | `V`/`AddMark`/... | with card content |
| Character skills (`skill` command) | `DoSkill` | card rules; currently rejected with a message |
| Debug commands (`debug`) | `DebugAct` | P7 (dev panel) |
| Speed multiplier | `Speed` | P7 |

The stub rules give a complete game of plain BanG Dream Monopoly: cards can be played
(no effect) and events are drawn (no effect).

## Verification

* `cargo test -p game-core` — routine tests on hand-built boards (rent, RiNG rent,
  agent half-rent and purchases, raise funds, bankruptcy + game over, leftover
  auctions, forced purchase, CiRCLE reward, scoring/ranking, exact replays) and
  end-to-end tests (bot games to the end with invariants, determinism, a human turn
  by command, prompt time-outs, vote, leaving, disconnect/reconnect).
* `cargo run -p game-core --release --example sim -- 50 4 200` — plays bot matches and
  counts what happened.

Sample (50 games, 4 bots, 200-round cap): ~360 ms per game in release; 34,461 rolls,
5,915 CiRCLE passes, 19,612 rent payments, 4,641 houses, 2,908 purchases, 415 forced
purchases, 264 auctions, 99 bankruptcies. 39/50 games reached the round cap: bots
rarely go broke, so bot-only games mostly end by `finish()` (human games end by vote).

## Save / restore

`Match::save()` serializes everything except the shared `GameData` and card rules
(world, pending routine + answer log, clocks, vote, deferred host actions) to JSON;
`Match::restore(data, rules, json)` rebuilds it. A restored match continues
identically (`tests/engine.rs::save_and_restore_continue_identically`). The format
is versioned (`SAVE_VERSION`); older saves are rejected rather than misread. The
web client uses it for solo refresh recovery (`SoloMatch.save` / `SoloMatch.restore`).
