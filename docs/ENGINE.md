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

Routines: `Opening`, `NextTurn`, `Ai(player_id)`, `Act(player_id, command)`, `Leftovers(deeds)`.

### Tile rules

Tile settlement is **rule instances**, the way card and skill content is -- see
[TILES.md](TILES.md). At match start `bind_tiles` places one instance per board
tile on a neutral board owner (`state::BOARD_OWNER = -1`, `World::board_field`),
stamping the tile's data (price, rent table, group, level cap) into the
instance's `props`. `settle_at` is a two-phase link: `settle` (the declaration
and its [反击] window), then `settleBody` -- which is **its own chain link**
(ABI v32, `ChainKind::SettleBody`), so 「replace the body」 (「将本次结算改为…」)
and 「the settle never happened」 are two separate [反击] targets. Cancelling
`Settle` skips the body *and* `settleAfter`; cancelling `SettleBody` skips only
the body. `CardRules::settle_tile` then runs the tile's instances in order; the
**default** impl is the built-in `land_at_built_in` (the plain BanG Dream
Monopoly settlement), which is what `StubRules` and an unbound kind get.

Board-owned instances also hear the **field hooks** they declare: the hook
dispatch in `game-rules/src/wasm_rules.rs` visits `board_field` after the
player fields (so a suppressing card can arm a prop before the tile reads it),
tile-filtered for a tile-carrying trigger like `passTile`. That is how
`tile:circle`'s **Pass entry** runs the [经过] CiRCLE reward
(`ctx::settle_circle_reward`), with the walk's `circle_reward` as the built-in
fallback when no instance is bound.

The engine keeps the money/deck work as primitives the bodies call
(`ctx::pay_rent` / `offer_buy` / `offer_build` / `offer_force_buy` /
`agent_landing` / `draw` / `draw_event` / `settle_circle_reward`), so a tile
body is a list of rulebook citations. Cards bend tiles by attaching, swapping
or retuning instances -- `ctx::prop` / `ctx::set_prop` on the running instance,
`ctx::tile_prop` / `ctx::set_tile_prop` on the instance governing a tile --
not by engine flags (`docs/TILES.md`'s Phase 3; the flags still to go are
listed there).

### Effect vs settlement

A card **effect** is a chain link (`Trigger`), separate from its settlement.
`ChainKind::Effect` is raised when the link's recipients are *named*, before any
modifier has touched it -- that is the stable [反击] key for the rulebook's
「被其他玩家的卡效果影响」. `Target` / `Abnormal` / `Pay` are settlement hooks:
they fire as the effect settles and cannot be used to reconstruct that clause.

Counters answer one timing per round (rulebook 89): the ring starts at the seat
after the player the timing belongs to and asks them last; every counter in the
round answers the *same* link; the declared counters then become new timings,
newest first, with their own rounds. Resolution is LIFO over the resulting
answer tree -- a counter's own answers settle before it, sibling counters settle
newest first, every counter before the timing it answers -- so a counter can
invalidate the effect before it settles. `Negation::{Activation, Effect}` and
`spare(seat)` replace the single `Cancelled` flag -- "the link never happened",
"it happened and settled to nothing", and "everyone but this seat settles" are
three different things. See `game-core/src/engine/rules.rs` and
`game-rules/src/wasm_rules.rs` (`hand_counteractions`).

### Why host events are deferred

The world's event and prompt counters are re-derived on every replay, so ids stay
stable and clients never see duplicates. Anything the *host* logs (vote messages,
disconnects, time-outs) while a routine is paused would steal an id the replay later
hands to a different event. Those changes are queued and applied once the routine
commits.

### Money pipeline depth

The rulebook (「[支付]时可以打出」) opens a [反击] window for every payment with
no depth limit. Nested money movements (a hook that forces another payment)
therefore open their windows at every depth. The **termination argument** is
that a hook cannot re-trigger on its own movement: while card X's hook is
running, nested money pipelines skip X (`Cx::reentrant_hooks`). A safety cap
(`MAX_MONEY_DEPTH = 32`) exists only to catch genuine runaway recursion and
**traps loudly** (panics) rather than settling silently.

### 「支付减半」 rides `payMul`

A rule that halves a settlement payment scales it at the pipeline's **`payMul`
stage** (`play.rs::scale_settle_payment`), not at `pay_rent`. That is the
「支付减半」 ruling (2026-10-06): the scale covers the payment **as card effects
have shaped it** -- a rent-region expansion that adds a surcharge, a forced
stop-and-pay -- and not only the rent table's number. Two homes are read, both
in milli-units (`prop::RENT_FACTOR` / `prop::PAY_FACTOR` on the tile's rule
instance, and the move plan's `plan::set_rent_factor` / `set_pay_factor` for a
hand card that shapes a settle and then leaves the field). A purchase
(`buy` / `build`), a forced purchase (rulebook 「此次购买的价格不受任何资金变动
效果影响」) and a print (`gain`) are never scaled. See
[TILES.md](TILES.md) 「支付减半」 wording comparison.

## Not in the shell (card content / later phases)

| What | C# | Where it goes |
|---|---|---|
| Card, skill, band and `Fx` hooks (`PayAdd`, `CircleRewardChoice`, `SkipTile`, `BuyPrice`...) | nested classes | card rules (WASM) |
| Active events (`EvOn(...)`: 协助CiRCLE重建, Forbidden Moca, ...) | `EventEffect` | card rules |
| Per-player card variables (`V(i, ...)`), marks, embers, field cards | `V`/`AddMark`/... | with card content |
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

## Allocators

Two different heaps, two different lifetimes.

**Card modules (short-lived, one run per instantiation)** keep a bump allocator
(`rules/card-sdk/src/rt.rs`, `guest` feature): a run allocates a handful of small
strings and never frees them, so a pointer that only moves up is enough and far
smaller than dlmalloc. The cursor starts at `__heap_base` and linear memory grows
on demand with `memory.grow`; `realloc` grows (or shrinks) in place when the block
is the most recent allocation, else falls back to alloc + copy. The ceiling is
16 MiB (`MAX_MEMORY`), matching the host's per-instance `StoreLimits`
(`be_*::MAX_MEMORY_BYTES`); growth past either returns null and the guest traps.

Measured on a stable 250-card aggregate (absolute module size drifts as card
content does):

| | before | after |
|---|---|---|
| module bytes | 336,074 | 336,196 (+122 code) |
| initial linear memory | 22 pages / 1408 KiB | 18 pages / 1152 KiB |
| fire-up, one real run | 608 µs | 574 µs |
| fire-up, full-run breakdown | 496 µs | 416 µs |

(Fire-up is `cargo test -p game-rules --test ruleset fire_up`, medians of 5 runs
each; the old fixed 256 KiB static arena was BSS and cost no file bytes, so the
file-size change is only the new code. Wall-clock on this machine is noisy and the
ranges overlap; the deterministic win is 256 KiB less memory eagerly allocated per
instance and a soft ceiling instead of a hard one. Host memory is capped at 16 MiB
per instance -- neither wasmtime nor wasmi had a limit before.)

**The browser engine (`crates/web-glue`)** is long-lived and allocates on every
tick, so it wants a real allocator. wasm32 defaults to dlmalloc; it now uses
`talc` 5.1.1 (`talc::wasm::new_wasm_dynamic_allocator()`, pinned -- the global
allocator surface changed in 5.x). Bot-game loop through the glue API under node
(30 games, 4 players, fixed seeds 1..30, 940,355 ticks total):

| | dlmalloc | talc 5.1.1 |
|---|---|---|
| `glue_bg.wasm` | 4,536,065 B | 4,534,742 B (−1,323) |
| total / median per game | 1554.2 ms / 43.40 ms | 1426–1436 ms / 37.3–37.8 ms |

Adopted: better on both size and speed (~8% faster mean, ~14% faster median).
