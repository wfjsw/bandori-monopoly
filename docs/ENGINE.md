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
  mechanism. **Simulation mode** (`docs/BOT.md` §3.2) installs an
  `AnswerProvider` on the `Cx`: every ask whose log entry is missing is answered
  inline instead of halting, so a routine runs once per simulation. The live match
  never installs one.

## Structure

| File | Contents |
|---|---|
| `mod.rs` | `Match` (host): clocks, live prompt (bot answers, time-outs, auction bidding), end-match vote, pending routine + snapshot + answer log, deferred host events, commands (`act`) |
| `world.rs` | `World` — everything a routine touches, incl. RNG and event/prompt counters |
| `cx.rs` | `Cx` routine context, `Flow`/`Halt`, `Ask` prompt builders |
| `play.rs` | routines: turns, movement, CiRCLE reward, landing, agents, rent, forced purchase, buy/build, payments, raising funds, bankruptcy, auctions, hand/draw, events, play card, scoring |
| `ai.rs` | bot decisions: the standard policy (same thresholds as the C#) and the chaos one |
| `setup.rs` | turn order, ban (Ranked), pick, deck (bots take the deck book entry) |
| `rules.rs` | `CardRules` — where card/event content plugs in; `StubRules` = no effects |

The card content itself lives in `game-rules`: `wasm_rules.rs` is the
`CardRules` bridge (`RulesBridge<M>`, with `WasmRules = RulesBridge<Ruleset>`
for the sandboxed modules and `rules-native`'s `NativeRules` for the bot's
sandbox-free simulations — `docs/BOT.md` §3.1). The `bandori` host imports
are backend-neutral functions in `game-rules/src/hostfns.rs` over a
`HostCtx` (guest memory + fuel + nested calls); `host.rs`'s linker and
`rules-native`'s `#[no_mangle]` shims both call them, so the two cannot
drift in behaviour.

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

Two other kinds of board-owned instance sit beside the `tile:*` ones
([TILES.md](TILES.md)): **event** rules (`event:*`, `tile = -1`, bound while the
event is live) and **mark owners** (`mark:*`, one per mark category,
`game_core::data::mark_rule_ids`). Neither governs a single tile, so the hook
dispatch adds them to the board list for a tile-carrying trigger as well.
`mark:cp` is the [CP点] **tile-mark** owner: [CP点] is its own tile-mark category
(`TileMark.category`, 「放置于路面上的指示物」, `data/rules.txt` 125), held by the
neutral board owner and **never by a player** (`TileMark.owner = -1`);
provenance is `TileMark.src` (the placing card instance) and `TileMark.card`
(its id). Cards place / count / clear through `ctx::place_cp` / `count_cp` /
`clear_cp` / `cp_src_at` -- the small API that owner implements. The other
[CP点] kind is the **on-card** count (`FieldCard::cp`, 「自己[场上]N个[CP点]」,
user ruling 2026-10-07): the card rule's own stock, written with `ctx::add_cp`
/ `add_cp_at` and read with `ctx::cp_attached` / `cp_at` (ABI v38).
`HookKind::CpChanged` / `TriggerKind::CpChanged` (`cpChanged`, ABI v36/38)
fires whenever a card instance's **on-card** [CP点] count is written, the same
shape as v29's `crystalsChanged`, so 「…时」 clauses on the count live in one
event handler (通用:该清CP了's graveyard rule) instead of at each spend site.

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

Counters answer one timing per round (rulebook 89, under **ruling 2026-10-07**):
the ask ring starts at the **initial user** -- the player whose action or effect
raised the link (`by_card`; a board-driven link -- rent, buy, build, turn flow --
is a system / tile event and starts at the active turn player) and runs forward
in turn order from there. Each seat **exhausts its counteractions before
priority moves on**: on its visit it is kept being offered its eligible cards
until it passes explicitly (one "not playing" option ends the visit) or holds
none left. Every counter in the round answers the *same* link. The round closes
at the end of a full lap of the ring that brings no new declaration -- a lap
that carried a declaration never closes it. The declared counters then become
new timings, newest first, with their own rounds under the same rules, where
that counter's declarer is the new round's initial user. Resolution is LIFO over
the resulting answer tree -- a counter's own answers settle before it, sibling
counters settle newest first, every counter before the timing it answers -- so a
counter can invalidate the effect before it settles.
`Negation::{Activation, Effect}` and `spare(seat)` replace the single `Cancelled`
flag -- "the link never happened", "it happened and settled to nothing", and
"everyone but this seat settles" are three different things. See
`game-core/src/engine/rules.rs` and `game-rules/src/wasm_rules.rs`
(`hand_counteractions`).

**Window algorithm (2026-10-08, `docs/GUARDS.md` §4.5).** The ring above is the
semantics; the implementation is **valid-option-first**: before any per-window
machinery (answer-tree clone, `Priority`, per-visit offers, world copies, the
CEL scope) a pre-scan asks whether any seat can respond at all -- seat
eligibility, then the per-card kind bitmask, then the compiled conditions
against a window context built straight from the live world. Zero survivors
skips the whole ring (a quiet lap would have closed it identically). Offers
themselves run lazily: a card whose condition admits and whose residual guard
is deleted (G4) is eligible with no world copy at all; only a surviving entry
that still carries a guard builds a `Run`. Within one ring a processed set
remembers each `(seat, card)` probe's verdict and reuses it on later laps when
the inputs it reads cannot have changed -- the ring's only world change is a
declaration removing a card from a hand. Verified byte-identical
(`Match::save()` per turn) against the naive ring by `examples/ckpt_equiv.rs`
A/B.

### Why host events are deferred

The world's event and prompt counters are re-derived on every replay, so ids stay
stable and clients never see duplicates. Anything the *host* logs (vote messages,
disconnects, time-outs) while a routine is paused would steal an id the replay later
hands to a different event. Those changes are queued and applied once the routine
commits.

### Negative finals reverse (Q3, 2026-10-07)

Adjustments compose freely -- the per-stage `max(0)` clamps are gone. When the
counteraction chain has resolved and the final amount is **negative**, the
original payment is adjusted to 0 and a **new** payment starts with the
participants swapped and the absolute value, running the full pipeline
(its own modifiers, counteraction windows, Q1 shortfall → mortgage →
bankruptcy). Applies to one-sided movements too: a negative 「[获得]」 becomes
a 「[消耗]」 and vice versa. A child that is itself driven negative reverses
again, capped at `MAX_PAY_REVERSALS = 8` (logged). The `pay` settlement window
honours `set_cancelled()` (V5).

### Marker windows and ownership (2026-10-07)

`markerSpend` / `markerGain` (TriggerKind 92/93) open **before** the markers
move; a cancelled link moves nothing. Marker costs otherwise keep today's
timing (spent as the effect resolves; nothing spent if the effect is negated).
A marker is owned by the **rule that creates it**, wherever its copies sit
(`World::marker_owner`) -- `skill:要乐奈` owns every 抹茶芭菲. Bankruptcy
clears everything the player holds plus every marker their rules own,
wherever it sits; neutral board marks ([CP点], owner -1) stay.

### Money pipeline depth

The rulebook (「[支付]时可以打出」) opens a [反击] window for every payment with
no depth limit. Nested money movements (a hook that forces another payment)
therefore open their windows at every depth. The **termination argument** is
that a hook cannot re-trigger on its own movement: while card X's hook is
running, nested money pipelines skip X (`Cx::reentrant_hooks`). A safety cap
(`MAX_MONEY_DEPTH = 32`) exists only to catch genuine runaway recursion and
**traps loudly** (panics) rather than settling silently.

### The pre-split stage (「分摊前」)

A payment **command** runs a command-wide modifier stage on its total before
any 「[分摊]」 divides it into shares: `payTotalAdd` -> `payTotalMul` ->
`payTotalCancel`, composing (`PIPELINE-AUDIT` Q2). This is the 「分摊前」 figure
of 「此次支付的分摊前资金减少Y×100」 (丸山彩 (2)) and 「此次获得的分摊前数量
增加Y×100」 (白鹭千圣 (2)); both hook `payTotalAdd`. The per-share stages
(`payAdd` / `payMul` / `payChoose` / `payAt`) still run on each settled leg.

A single-pair payment's command total *is* its amount, so the same three stages
run inside `money_inner` -- after the `effect` declaration (a counter still
hears the **declared** amount) and before the per-share stages. A 「[分摊]」 body
calls `ctx::pay_total` first, divides the answer, and runs each share through
`ctx::pay_leg` (`Pay::total_stage = false`, so the legs do not re-run the total
stage). `ctx::split_pay` is the 「[分摊][支付]」 convenience (ceil10 share).

### Self-payment 「A[支付]A」

Not a no-op (`PIPELINE-AUDIT` Q4). The counteraction windows run in full -- a
counteraction may redirect the payee -- and the affordability / raise-funds /
bankruptcy path applies exactly as to any other payment (规则书 L16 「当玩家
无法支付某笔支出时（包括抵押）」). Only the **net balance effect** is zero when
the pair settles: the debit and the credit cancel. Pinned by
`rb_money::self_payment_*`.

### Effect ordering (the lookup-group key)

Two effect sources answering the same window are ordered by an explicit
deterministic key -- `(lookup group, source order, declaration order)`
(`PIPELINE-AUDIT` Q5, `game-rules/src/wasm_rules.rs` `effect_order_key`):

* **lookup group** -- `status` -> `band` -> `character` -> `field` -> `hand` ->
  `discard` -> `deck` -> `removed` (`LookupGroup`; `Board` for our neutral
  `tile:*` / `event:*` / `mark:*` rules, after the player groups).
* **source order** -- the candidate's index in that group's authoritative state
  list: player-major then placement order for the field-ish groups
  (`players[p].field`), the hand `Vec` index for `hand`.
* **declaration order** -- the entry index within the source's manifest `on`
  list.

No `HashMap` iteration, no clock: the key is a pure function of the
authoritative state lists and the manifest. Field hooks before the hand
counteraction ring is already group `Band`/`Character`/`Field` < `Hand`, so the
dispatch's phase split is the key's own order and wiring it changes nothing
(the sim's event counts are identical with and without the sort -- see
`PIPELINE-AUDIT` §7 Q5). One recorded deviation: `bind_skills` places a
player's character skill before their band skill, so within one player's field
the source order is character -> band while the group order says band ->
character; the key classifies them correctly and the stable sort leaves the
outcome alone.

### Move / settle stage model (`SETTLE-STAGES.md` §7, ABI v43)

Every move -- walk or teleport, main or card-driven, settling or not -- runs
the same tail. The plan (`turn.plan`) is the authoritative move *shape*; the
running `MoveCtx` carries the engine's run fields. `moveBefore` folds only the
window's plan writes into the run (`fold_plan_delta`), never the other way
round (a blanket copy would wipe a `moveRoll` counteraction's `pay_factor`).

```
moveBefore         「移动前」  plan fixed, nothing walked. Counteractions that
                               cancel or alter the move go here.
passBefore/passTile/pass       行动阶段 12 「[经过]」   (per step / endpoint)
passPlayer                     行动阶段 13 「[重叠]」   (end tile only)
moveAfter          「移动后」/「主要移动结束时」/「[移动终点]」
                               after 重叠, before the settle stages, for every
                               completed move including 「不触发结算」 (R1).
settleBefore       行动阶段 14  「[触发结算]前」. Q7: a relocation here redirects
                               the settle and re-runs this window at the new
                               tile, capped at MAX_SETTLE_REDIRECTS = 8.
settle/settleBody/settleAfter/tileResolved   行动阶段 15–16 (only if the move
                               settles). A relocation inside these does NOT
                               re-target -- the landed tile finishes.
moveResolved       行动阶段 16  「主要移动阶段后」 (always, strictly last).
teleported         teleport-specific, after the settle (or at once if none).
```

A 「不[触发结算]」 teleport still fires `passTile` + `passPlayer` at its
destination (R2 / M6a): 专名词 9 gives the teleport a [路径] of just the
endpoint, B41 fires [经过],[重叠],[结算] there, and 其他规则注意事项 1.2 removes
only the [结算].

### Terminal `<thing>Resolved` hooks

`tileResolved` (after `settleAfter`, and after a cancelled settle),
`moveResolved` (after a move and any settle it asked for) and `bankruptResolved`
(after the leftover auctions) are the terminal points a 「结算完成时」 card
hooks (`PIPELINE-AUDIT` Q6). `payAfter` / `buyAfter`+`bought` / `eventAfter`
are already the terminals of their pipelines and get no `<thing>Resolved`
companion.

### Bankruptcy retires the seat (B2 / B3)

`remove_from_game` clears the seat's `field` (角色卡 / 乐队卡 / every 「[持续]」
card, the bound `skill:*` instances included) and `actions` -- 规则书 L81
「所有其正在生效的卡，技能效果停止生效」 -- and the hook / buy-hook dispatch
skips `out()` seats. The 「标志物」 (`s.tokens`: 火罐 / 奇迹水晶 / P✽P粉丝 ...) stay:
L81's removal list is cards and skill effects, not the marker vocabulary. The
seat is marked dead **before** `bankruptBefore`, so the dying player's own
sources cannot join that window (L16/L17/L81 read together).

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

### The purchase surface (`docs/PURCHASE.md`)

Eligibility, price and the deal are decided by the rules crate; the engine keeps
the generic mechanics (`purchase.rs`):

* **Quote.** `CardRules::buy_quote(w, data, q)` returns `base_quote` (land +
  houses; 2× for `Force`) for `StubRules`. `WasmRules` overrides it and runs the
  `BuyGate` / `BuyAdd` → `BuyMul` → `BuySet` hooks in **pure guard mode**
  against a world copy -- the `cant_play` shape, so a quote never mutates the
  match and never prompts -- but only when a hooking instance exists. The
  commit re-quotes, so quote == charge. It takes the world by reference, so the
  view's `st.buy_price` preview asks without cloning.
* **Gate.** `BuyGate` runs for every `BuyKind` (Land / Agent / Card / Force /
  Acquire / Auction); a `set_cancelled` refuses the buy. `buyBefore` cancel is
  also honoured. rana_parking's 「该次传送不可进行地契购买」 is a **native** gate
  on the Land offer: `plan.no_buy`.
* **Assign.** `assign_deed` writes owner / houses / mortgage in one place;
  `BuyAssign` hooks rewrite the deal before commit, on every kind (land,
  agent, card, force, acquire, auction). Force-buy keeps the mortgage
  (「获得的地契仍为抵押状态」).
* **Preview.** `st.buy_price` / `st.build_cost` are written from the quote at
  the END step, `-1` otherwise. `MatchPrompt.price` / `prices[]` carry the
  quoted prices on buy / force_buy / agent prompts. The gates (`buyable_here`,
  `why_not_act "buy"`) and the AI (`ai_wants_buy` / `ai_agent_choice`) read the
  same quote, so a player who can afford only the discounted price is no longer
  refused.
* **`linger`.** A hand card binds a turn-scoped instance in `TurnCtx.lingering`
  (cleared at turn start, carried by `adopt_turn_policy`). It is the hand-card
  home for 「本回合」 effects: the def's own `BuyAdd` / `BuyMul` / `BuySet` /
  `BuyAssign` hooks reach the buy pipeline, and a `set_prop` made before
  `ctx::linger` lands on the instance's props (`prop::NO_BUILD`). The lingering
  instances hear the same field-hook dispatch as placed cards and board rules.
  It replaces the retired `TurnCtx` flags (`buy_discount` / `free_buy` /
  `raze_on_buy`) and the per-player `noBuild` scratch key.

### The guest's pending state, overlaid (not persisted)

A card body that pauses for a host routine (pay / gain / move / ...) would
otherwise run that routine against a world that does not yet see the body's
own writes: 壱雫空 「清除场上所有[停留]与[眩晕]效果」 then has every player pay,
and a payer the body just un-stunned is still blocked by `can_pay` on the live
world. Two earlier attempts failed -- *persisting* all keys double-counts
additive writes (the body replays from the top), and persisting only clears
breaks `rb_general::parking_replaces_settle_with_stay`.

The design that works is a **merged view that is not persisted**
(`cx.rs::overlay_guest_state` / `restore_guests_to`). At the `NeedHost`
boundary the guest's pending player-state **clears** (status keys that went
*down*) are overlaid on the live world for the routine's duration; when the
drive's next iteration starts, the overlay is dropped, **keeping only the
routine's own effects** (a key the routine also touched keeps its delta on the
base value). The replay then re-applies the guest writes deterministically.

Only decreases cross. A status the body itself just *applied* -- 心の雨's
fallback `give_stun` then `gain 1000` -- must not gate its own follow-on money
(`money_inner`'s stunned-payee gate would refuse the print). Whether
「无法收付款」 should block a card's own gain to a player it just stunned is an
open ruling; the pre-overlay behaviour stands until it is decided. Money is
not overlaid at all: a `gain_fixed` is re-applied on the replay, and the
pipeline's own moves are routine effects, not guest writes. `adopt_turn_policy`
still carries the `latch.` keys and the turn-ctx policy (those are "already
taken" and the replay's re-write is a no-op). Nested drives push and pop their
own overlay, so a hook's routine never sees an outer one's dropped.

### 「资金变动」 is `payAfter`

Rulebook 支付阶段 7 (「资金变动, 合并到[支付后]?」) is a hook point, not a
[反击] window, so it merges into `payAfter`: `payAfter` now fires on **any**
money change -- a print (`gain`) and a `gain_fixed` included -- while `paid`
stays the payer-side-loss [反击] window (再次牵起手来 / 游击演出 are
「[消耗]或[支付]」). Every credit also bumps a per-player **turn gain counter**
(`World::gains`, reset for everyone at each turn start), which is what
「当前回合内你每获得过一次资金」 reads (`ctx::gains_this_turn`) -- so a hand
card can count gains with no field stand-in observing them.

### `exileMain` consumes the turn's main move

MyGO:无路矢's 「在[除外]层数归0后[传送]至该格子，视为当回合的主要移动」 is the
`exileMain` slot (`state_key::EXILE_MAIN`). The exile tick reads and consumes
it: the return teleport runs as the main move (`teleport_as(..., main: true)`)
and `TurnCtx::main_moved` is set, so 「一回合只能触发一次［主要移动］」 keeps the
player from rolling as well. `why_not_act`'s `roll` / `end` gates both look at
`main_moved`. Clearing a [停留] down to 0 also un-skips the move
(`give_stay`): 「[停留]：处于该状态时[无法移动]」 makes the skip a consequence of
the state, not a latch.

## Bot AI (mentalities)

Every seat carries a **`BotMentality`** (`state.rs`, serde-defaulted to
`standard` so older saves and room records load unchanged). It is chosen when
the match is created -- solo (`SoloSetup`, one choice for all bots with a
per-bot override) or online (`POST /api/rooms/{id}/bots`, stored on the room
member and copied onto the seat by `Match::new`) -- and it is what
`Cx::bot_mentality` reads. Only `bot` seats take one: **a human who times out or
disconnects is always answered with the standard policy** (`ai` flips on,
`bot` stays off).

Two policies, one decision surface (`ai_step`, `tick_live`, `tick_choice`,
`auto_mortgage`, and the precomputed `Ask::ai` / `ai_picked` / `worth`):

**`standard`** is the ported C# bot. Its thresholds live in `ai.rs` as the
**defaults of `game-core/src/strategy.rs`'s `StrategyParams`** (`docs/BOT.md`
§3.8) and are mirrored by the web client's 托管 autopilot
(`webui/src/game/autopilot.ts`) -- keep the two in sync. A seat's resolved
params come from the strategy book (`data/strategy_book.json`, back-off lookup
by public table key); an empty or absent book leaves every constant at the
values below, which is the pre-book policy exactly. Chaos is not
parameterised -- it keeps its own literals.

| Constant | Value | Meaning |
|---|---|---|
| `BUY_RESERVE` | 2,000 | keep this much after buying |
| `BUILD_RESERVE` | 3,500 | keep this much after building |
| `REDEEM_RESERVE` | 4,000 | keep this much after redeeming |
| `FORCE_BUY_RESERVE` | 4,000 | 「可选择[支付]…两倍…强行购买」 only while it leaves this much |
| `PLAY_CARD_CHANCE` | 0.7 | odds of playing a card rather than rolling in 运营 |
| `MAX_PLAYS_PER_TURN` | 2 | hand cards played in one turn before rolling |

**Deck pick.** A standard bot (and the browser 托管) submits the deck book's
entry for its public table -- own character, own seat, the other seats'
characters in seat order -- when `data/deck_book.json` has one
(`game-core/src/deck_book.rs` back-off lookup, `docs/BOT.md` §3.7), else the
designer's preset (`deck::preset`). The book must match the running ruleset's
hash and the `standard` policy, or it is ignored; every entry still has to
clean down to a complete legal deck. Pure -- no RNG. Chaos never reads it.

**Strategy params.** The same seats read their thresholds from the strategy
book (`data/strategy_book.json`, `game-core/src/strategy.rs`, `docs/BOT.md`
§3.8): `(character, opponents' bands)` → `(character)` → `(band)` → the
defaults above. Stale ruleset hash / policy / `params_version` ignores the
whole book. The browser 托管 gets the same resolved struct from web-glue's
`strategy_for`, so there is one source of truth. Chaos keeps its own
literals.

**`chaos`** is legal but maximally disruptive and effect-heavy: it plays a card
at every legal opportunity (no odds roll, no per-turn cap beyond the engine's
own; the rule's `ai_play` heuristic is ignored, only `cant_play` counts), presses
character and band skills whenever they are usable (at most once each per turn,
so a no-op skill cannot park it), and picks uniformly among the non-default
prompt options -- a tile prompt gets a random target, never 「无」 while one
exists. An offered [反击] is the one gate left: it declares on only
**`CHAOS_COUNTER_CHANCE = 0.3`** of the offers it gets (a random offered card)
and passes the rest, rolled per offer from the match RNG. Ban / pick / deck are
random (the deck is a random legal one, `deck::random`), hand overflow discards
at random, and it only ends the turn when nothing else is legal.

**[反击] offers.** Every seat in the ring is offered the window, bot seats
included -- "is a bot" is not a rulebook reason to skip one (out / [除外] /
`CannotPlay` still are; `can_counteract_now` in `game-rules/src/wasm_rules.rs`).
The offer is an ordinary prompt and is answered without stalling:

* **standard / chaos** (`ai` on) answer inline through the precomputed `Ask::ai`
  fill (`Cx::fill_ai` / `counteract_pick`), which `tick_live`'s bot schedule
  applies: standard declares per [`CounterParams`] propensity
  (`docs/BOT.md` §3.8; **default `DEFAULT_COUNTERACT_PROPENSITY_MILLI` = 600‰
  per offered card**, drawn from the match RNG -- user ruling 2026-10-08,
  "bots must be able to counteract"; the old default was 0 = never, which left
  every [反击] card dead in a standard bot's hand; a book entry at
  `propensity_milli: 0` holds a card back), chaos on `CHAOS_COUNTER_CHANCE`.
* **advanced** (`ai` off, held for `bot-service` / the browser worker) get the
  offer as a prompt like any other decision; the driver answers it (search, or
  the heuristic on its own timeout) and the engine's deadline applies the
  fallback -- the 「不打」 skip -- so a window never stalls. Solo waits for the
  local driver by design; online runs the prompt clock + `REMOTE_GRACE`.

The ring, the exhaust-on-visit floor and the LIFO resolution are unchanged for
humans (`hand_counteractions`, ruling 2026-10-07; `docs/ENGINE.md` above).

Chaos still keeps a coin reserve, or it burns out in a few turns and stops
being disruptive. **`CHAOS_RESERVE = 1,000`** is its only money gate: voluntary
spending (buy, build, redeem, auction bids, and the optional paid offers) happens
only when it leaves that much; a forced purchase is accepted only under the same
condition; auction raises are capped at `money − CHAOS_RESERVE` and otherwise
random within the cap. Card plays are unrestricted -- a card's own cost is not
visible to the AI layer (it lives in the play body), so the reserve cannot gate
them.

All chaos randomness comes from the match RNG (the world's, plus the host's
`live_rng` for answer pacing and auction raises) -- never `thread_rng` -- so a
chaos game replays and restores like any other (`tests/mentality.rs`).

## Not in the shell (card content / later phases)

| What | C# | Where it goes |
|---|---|---|
| Card, skill, band and `Fx` hooks (`PayAdd`, `CircleRewardChoice`, `SkipTile`, `BuyPrice`...) | nested classes | card rules (WASM) |
| Event cards (事件卡: 对邦, 协助CiRCLE重建, Forbidden Moca, ...) | `EventEffect` | `rules/events` (`docs/EVENTS.md`); the engine keeps the deck, the draw, the active list and the filing away |
| Per-player card variables (`V(i, ...)`), marks, embers, field cards | `V`/`AddMark`/... | with card content |
| Character skills (`skill` command) | `DoSkill` | card rules; currently rejected with a message |
| Debug commands (`debug`) | `DebugAct` | P7 (dev panel) |
| Speed multiplier | `Speed` | P7 |

The stub rules give a complete game of plain BanG Dream Monopoly: cards can be played
(no effect) and events are drawn (no effect). With the shipped ruleset
(`card_all`) an event draws its `event:*` rule instance on the neutral board
owner and the body resolves -- see [EVENTS.md](EVENTS.md).

## Verification

* `cargo test -p game-core` — routine tests on hand-built boards (rent, RiNG rent,
  agent half-rent and purchases, raise funds, bankruptcy + game over, leftover
  auctions, forced purchase, CiRCLE reward, scoring/ranking, exact replays),
  end-to-end tests (bot games to the end with invariants, determinism, a human turn
  by command, prompt time-outs, vote, leaving, disconnect/reconnect), and the
  mentality suite (`tests/mentality.rs`: chaos buys with no standard reserve,
  plays every card before rolling, counters on ~30% of offers, picks a
  non-default prompt option, same seed = same game).
* `cargo run -p game-core --release --example sim -- 50 4 200` — plays bot matches and
  counts what happened. Add `standard` or `chaos` as a trailing word to pick the
  bots' mentality (default `standard`): `sim -- 50 4 200 chaos`.

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

A field added under `#[serde(default)]` does **not** bump `SAVE_VERSION`: the
old JSON simply reads as the default. `TileMark.category` / `TileMark.src` are
both defaulted that way, so a pre-category save still loads; `Match::restore`
then re-reads the old [CP点] `kind` (`cards:card-general.clear_cp_mark`) into
`mark_category::CP` and drops the player owner it used to carry (a [CP点] has
none). New writes never use that `kind`.

## Records

`crates/game-core/src/record.rs` (`docs/REPLAY.md`) wraps the public `Match` API
into a replayable input log; `Match` itself is untouched.

* **`RecordedMatch`** forwards `new` / `restore` / `save` / `tick_steps(k)` /
  `act` / `quick_start` / `finish` / `member_left` / `member_back` to the match
  and appends to a `Recorder`. `tick_steps(k)` ticks `dt = k as f32 * 0.05`
  (k in 1..=10); consecutive calls with the same `k` run-length merge into
  `Ticks{k, n}`. Rejected acts are recorded too (`ok: false`).
* **Checkpoints.** Whenever the turn key `(round, turn)` changes, the recorder
  closes the open tick run and writes a `Checkpoint { at, tick, round, turn,
  hash }`, where `hash` is an FNV-1a-64 of `save()`. `at` counts inputs, so
  every checkpoint sits between two of them. Recording stops once the match has
  `ended()`; a final `hash_save(save())` seals the body. The `check` field of a
  `RecordFile` is the FNV-1a-64 hex of `serde_json::to_string(&body)`.
* **`Replayer`** rebuilds the match from `Init::Seed(MatchSetup)` or
  `Init::Snapshot { save }` and drives it through the same public calls,
  re-deriving every checkpoint. `Status.divged` is sticky: the first mismatched
  checkpoint (or an act whose Ok/Err bit disagreed) flags the rest of the
  replay. `seek` restores the nearest keyframe (`save()` every ~4 turn
  boundaries or 20 s of game time, capped at `MAX_KEYFRAME_BYTES`) and
  re-simulates forward. `export_with_events` bundles the public event log by
  re-simulating, because `world.recent` is only a 400-event tail.
* **`EngineStamp`** names the writer: `format` (the record version),
  `save_version`, the card `abi`, and the ruleset / data sha256s. game-core can
  only see the two format versions (`EngineStamp::current`), so web-glue and
  the rules worker pass the full stamp to `Replayer::new_with_stamp`;
  `compat(a, b)` reports the differences (format / abi fatal, the rest
  warnings).
* **Codec.** A `.bdrec` is a zstd-framed `RecordFile` JSON. `encode_record_zst`
  / `decode_record` (`docs/REPLAY.md` "Framing and codec") are the shared
  surface; `decode_record` sniffs zstd / gzip / plain JSON so older recordings
  still load. The wasm build encodes with `structured-zstd` (pure Rust, no C
  toolchain), natively with the `zstd` crate; both land at the same ratio on
  record JSON and emit standard frames, and `ruzstd` decodes either.
* **Cost.** Per tick the recorder is one run-length increment. Per turn change
  it is `save()` plus FNV — about 0.6 ms of an 88 KB save on the dev machine,
  which is **the dominant cost of recording** and is the design's own budget
  (`docs/REPLAY.md` §1). The wrapper's bookkeeping on top of that is under 2%
  (`tests/record.rs::recorded_match_overhead_is_under_two_percent`, ignored,
  run explicitly); `examples/sim` is unchanged.

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
