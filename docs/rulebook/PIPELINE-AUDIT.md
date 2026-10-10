# Pipeline audit — four pipeline specs vs our engine

Status: **read-only audit, 2026-10-07**. Source material: the four private-repo
pipeline docs in `target/scratch/specs/` (PAYMENT_PIPELINE, BANKRUPTCY_PIPELINE,
EFFECT_ACTIVATION_RESOLUTION_RESPONSIBILITY, TILE_RESOLUTION_PIPELINE), used as a
*behavioural* spec. The rulebook (`data/rules.txt`, `docs/rulebook/rulebook-doc.md`
for the 时点流程 tables) is ground truth; where a spec item contradicts it the row
is **CONFLICT-WITH-RULEBOOK** and quotes both. Class names, Unity/Mirror
structures, UI flow and threading from those docs are **not** implementation
targets and are listed per spec under "Not adopted".

Classification: **MATCHES** / **DIFFERS** / **MISSING** / **AMBIGUOUS** /
**CONFLICT-WITH-RULEBOOK**. "Not adopted" rows are deliberate, not gaps.

Per-spec counts are at the end of each section.

---

## 0. Not adopted (all four specs)

Out of scope entirely: **AI / bot / auto-answer behaviour** (the specs' timer
auto-pass, and anything about how a machine seat picks answers). Not audited,
not a gap.

| Spec artefact | Why not adopted |
|---|---|
| `PaymentController` / `BankruptcyController` / `TileResolutionController` / `TileController` / `EffectReactionController` / `TurnStateController` class split | Our split is engine shell (`crates/game-core/src/engine`) + wasm rule crates (`rules/`); the controllers' *behaviours* are mapped below, the class structure is not. |
| `MatchStateData` / `MatchPlayerStateData` / `MatchNeutralStateData` facades | `World` + `state.rs` + `board_field` (`BOARD_OWNER = -1`). |
| `BaseResolvableEffect` / `BaseEffectSource` / `ActiveEffectRegistry` / `SourceId` | `FieldCard.uid` + ruleset manifest + `CardRules` trait. |
| `GameEventContext` / `GameEventPayload` / `ReactionWindowStack` (`WindowId`/`EventId`/`ParentEventId`) | `Trigger` (incl. `seq`/`answers`, `rules.rs:119-123`) + the deterministic snapshot-replay prompt log (`cx.rs`). No window-identity revision checks: replay makes a stale answer structurally unreachable. |
| Mirror `Command`/`TargetRpc`, `PlayerReactionHandler`, `GameUIManager`, Unity scene/presentation | Browser client + `MatchPrompt` / `Ask` protocol; the server validates acts (`mod.rs`). |
| Coroutine-suspended nested controllers, `WaitingForBankruptcy` stack entries, LIFO `MoneyChangeEntry` operation stack with saved stages | Straight-line Rust over `Cx` with `Halt` re-run from snapshot; nesting is ordinary Rust recursion (`money_depth`). |
| `GetRoundRobinPlayerOrderStartingAt`, neutral responder appended to the ring | Our ring is the 2026-10-07 ruling (below); there is no neutral responder seat. |
| Prompt-revision counters, `OptionId` route tables | Answer-log position identifies a prompt (`cx.rs`); revision/OptionId are protocol details of their netcode. |
| Their tile taxonomy (Ring / Developer / Circle / Circle Cafe / Instrument Store / Meteor Hall) and `DefaultTileResolutionFunctionLibrary` | Our board is `property / ring / agent / circle / edogawa / cafe / ryuseido` (`docs/TILES.md`, `rules/tiles`). |

---

## 1. Payment Pipeline (`PAYMENT_PIPELINE.md`)

Our one pipeline: `Cx::money` / `money_inner` (`crates/game-core/src/engine/play.rs:2652-2895`).
Print (`from = None`), delete (`to = None`) and pay-player (both `Some`) all go
through it (`Pay`, `play.rs:34-78`). Stage order is pinned by
`rb_money::gain_runs_the_modifier_stages` (`crates/game-rules/tests/rb_money.rs:34`)
and `rb_gap_money::g02_*` (`tests/rb_gap_money.rs:159`).

### 1.1 Behavioural requirements

| # | Requirement (spec) | Where in our engine | Class | Notes |
|---|---|---|---|---|
| P1 | Root caller or effect requests Print / Delete / PlayerPayPlayer | `Pay::new` + `Cx::money`; card path `HostRequest::Pay` (`game-rules/src/wasm_rules.rs:1971-2022`) | MATCHES | One entry point for all three directions. |
| P2 | Validate endpoints before opening events; reject invalid silently | `money_inner` early-outs: `amount <= 0`, `from == to`, dead endpoints (`play.rs:2672-2677`) | MATCHES (with gaps P14/P15) | |
| P3 | Command-wide pre-split: AddDiff → MultDiff → Cancel, composing | `effect` ([反击] window, `play.rs:2736-2771`) → `payAdd` → `payMul` → `payChoose` → `payAt` (`play.rs:2774-2804`) | DIFFERS | See P3a. Rulebook 支付阶段 (rulebook-doc.md:227-235) is 1 计算前 / 2 计算时(±) / 3 计算后 / 4 支付前(×) / 5 支付时(取消/改目标) / 6 支付后 / 7 资金变动. |
| P3a | Cancel runs *after* Add and Mult | our `effect` (cancel-capable) runs *before* `payAdd`/`payMul`; a second cancel-capable point is `pay` after the modifiers | DIFFERS | Already recorded: `docs/rulebook/TEST-FINDINGS.md:189-192` (「One `effect` raise carries window 5's cancel/retarget power at window 1's position」). Card-facing order pinned by `rb_gap_money::g01`(ignored)/`g02`. |
| P4 | Build split-entry list (print: one/payee; delete: one/payer; transfer: payee-major Cartesian) | **No split in the pipeline.** Multi-endpoint plays split at the card body; each 分摊 leg is its own `money()` call (`rb_gap_money.rs:298` ruling comment) | DIFFERS | See P4a. Spec's own text calls the split-damage/pay-me content "future". |
| P4a | Split share = divide final amount, round each share **up to a multiple of ten** | Card body computes `ceil10(total/n)` (微笑的铁假面 legs; `rb_gap_money::g03/g05/g06`) | MATCHES (rule) / DIFFERS (home) | The ceil-10 rule is implemented, but in the card body, not the pipeline. |
| P4b | Pre-split modifiers are **command-wide** (one double hits the total before division) | Each leg runs its own full pipeline; a modifier on one leg does not hit the others | DIFFERS | Ruling question Q2. |
| P5 | Post-split Add/Mult/Cancel per concrete entry | Per-leg pipeline (same stages as P3) — equivalent when there is one leg | MATCHES (single-pair) | |
| P6 | Inactive/Void entries skip; explicit cancel runs `PlayerRejectPayPlayer` for that pair only | Cancel → `payAfter` value 0 + `Paid::default` (`play.rs:2755-2767`, `2825-2837`). No rejection event | DIFFERS | No `PlayerRejectPayPlayer`-equivalent. Observable: nothing in the log names "this pair was refused". |
| P7 | Negative final → zero + exactly one positive **reverse child** (swapped endpoints) | Clamped to 0 at every stage (`t.value.max(0)`, `play.rs:2769/2789/2820`); no reverse child | DIFFERS | For *prints* the clamp is pinned: `rb_money::negative_final_amount_clamps_to_zero` (`rb_money.rs:99`, FEVER! 「可小于0」). For a pay-player going negative the spec would reverse direction. Q3. |
| P8 | Normalize, then settle: Before* events → affordability → mortgage loop → commit | `payAdd..payAt` → `pay` → affordability check (`play.rs:2839-2852`) → `raise_funds` (`play.rs:2955-3002`) → debit/credit | MATCHES (shape) | |
| P9 | When short: try mortgage (`BeforeMortgage` → commit marker + fixed credit → `AfterMortgage`), loop until affordable or exhausted | `raise_funds` → `Ask::mortgage` / auto-mortgage → `mortgage()` (`mortgageBefore` → `do_mortgage` → `mortgage`, `play.rs:2582-2615`) | MATCHES | `do_mortgage` credits `mortgage_value` directly — event-free, as rulebook L92 requires. Tests: `rb_rulebook::gf05_mortgage_pays_fifty_percent_of_the_price`. |
| P10 | Mortgage proceeds are a fixed, event-free credit | `do_mortgage` (`play.rs:2582-2599`); `redeem` likewise (`play.rs:2618-2635`) | MATCHES | 规则书 L92: 「抵押，赎回，和强制购买的资金变动不受任何效果影响」. Pinned by `rb_rulebook::o06_mortgage_and_redeem_ignore_pay_modifiers` (`rb_rulebook.rs:858`). |
| P11 | Strict unaffordability → mark pair cancelled + rejection flow (no settlement, no bankruptcy) | Our `must` flag: `must = true` → raise funds then bankruptcy; `must = false` → **cap the loss to available cash and settle** (`play.rs:2840-2850`) | DIFFERS | See P11a. There is no "refuse the pair" outcome. |
| P11a | Only rent sets `must` | `p.must = true` at `play.rs:1855` (rent) is the **only** site. Card `HostRequest::Pay` defaults `must: false` (`wasm_rules.rs:2004`) | **DIFFERS — correctness bug** | 规则书 L16: 「当玩家无法支付某笔支出时（包括抵押）」 → 折现 + 破产. A card 「[支付]/[消耗]」 the payer cannot cover silently underpays instead of offering mortgage / going bankrupt. See §5.1 bug B1. |
| P12 | Non-strict shortfall → cap settled amount, mark bankruptcy requirement, settle what is available, then bankrupt | Cap exists for `must = false` but **never** triggers bankruptcy; for `must = true` the available cash is *not* settled first — `raise_funds` → `bankrupt` then `money_inner` returns without a payment event | DIFFERS | 规则书 L16 defines bankruptcy as 折现 all + pay all to the creditor, so "no partial settle" matches the book; the missing piece is that non-rent expenses never reach it (B1). Spec's "partial settle then estate to auction only" is CONFLICT-WITH-RULEBOOK (see §2). |
| P13 | Exact-to-zero payment does not bankrupt | `money < loss` is false when equal (`play.rs:2841`) | MATCHES | |
| P14 | PrintMoney accepts zero and still fires Before/After credit events | `amount <= 0` returns before any event (`play.rs:2672-2673`) | DIFFERS (minor) | Rulebook window-4 examples say 「不等于0」; zero-change is a no-op in our reading. |
| P15 | PlayerPayPlayer atomic per pair; payee overflow pre-checked; failed credit compensates the payer | Debit then credit, no overflow pre-check, no compensation (`play.rs:2839-2862`). `money` is `i32` | MISSING (robustness) | Unreachable in honest content; a hostile wasm body can wrap. |
| P16 | Self-payment follows the affordability/bankruptcy path (zero net) | `from == to` returns `Paid::default` before any event (`play.rs:2672-2674`) | DIFFERS (minor) | No events, no bankruptcy on a self-charge. Q4. |
| P17 | Operation stack LIFO; a child cannot overwrite the paused parent's stage; resume at saved stage | Rust recursion over `money_inner` with `money_depth` (`play.rs:2652-2671`); each call owns its locals | MATCHES (effect) | No explicit stack object — not adopted (§0). |
| P18 | Nested money from a responding effect opens its own windows at every depth | Same recursion; `effect` opens per movement (`play.rs:2733-2735`); `reentrant_hooks` prevents self-retrigger (`wasm_rules.rs:1465-1500`); `MAX_MONEY_DEPTH = 32` traps loudly (`play.rs:25`) | MATCHES | Documented in `docs/ENGINE.md` 「Money pipeline depth」. |
| P19 | Before/After per transaction type (`BeforePlayerGainMoney` / `AfterPlayerGainMoney` / `BeforePlayerLossMoney` / `AfterPlayerLossMoney` / `Before/AfterPlayerPayPlayer`) | Coarser: `effect` / `payAdd..payAt` / `pay` / `payAfter` / `paid`. `payAfter` fires on any money change (print included); `paid` only on payer-side loss | DIFFERS (shape) | 规则书 支付阶段 7 「资金变动(合并到[支付后]?)」 → merged into `payAfter` by design (`docs/ENGINE.md` 「资金变动 is payAfter」). Direction-specific Before events do not exist. Pinned: `rb_money::card_payment_opens_the_counteract_window`. |
| P20 | DeleteMoney is an event-free authoritative debit | Card prints/deletes go through the pipeline; `gain_fixed` is the event-free API (`ops.rs:517-532`) | MATCHES | |
| P21 | Cancellation is successful completion; state/overflow/callback failures are operation failures | Cancel returns `Paid::default` = success; panics on depth cap | MATCHES | |
| P22 | Ordered pair results + one completion callback to the root caller | Callers receive `Paid { paid, loss, gain }`; no multi-pair result list | MATCHES (single-pair) | Multi-pair result list is part of the unadopted split model. |

### 1.2 DIFFERS/MISSING — consequence, test, fix

**B1 (correctness, top).** Card/effect payments never raise funds or bankrupt.
`HostRequest::Pay` builds `Pay::new(asked, "card")` without `must = true`
(`wasm_rules.rs:2004-2021`), so a 「[支付]5000」 on a player with 1000 silently
takes 1000. Consequence: a forced payment can never eliminate a player; the
rulebook's 破产 path is rent-only. Test: `rb_money::card_shortfall_raises_funds_then_bankrupts`
— P1 has 1000 cash and one un-mortgaged deed (value 500); P0 plays a card that
makes P1 pay 5000; expect the mortgage offer, then bankruptcy, creditor P0
receiving the liquidation (rules.txt L16/L76). Fix sketch: set `must = true` on
the card `Pay` (and on any engine-imposed delete), and route the shortfall
through `raise_funds` — no new classes. Optional offers (buy/build) keep their
pre-gates (`play.rs:2051`, `2092`) and stay `must = false`.

**B2 (correctness).** P3a — window-5 cancel power sits at window-1 position
(known: `TEST-FINDINGS.md:189-192`). Consequence: a 「取消支付」 counter resolves
before 「支付增加/翻倍」 windows have run, so it cannot see the shaped amount, and
a window-2 modifier still applies to a payment that window 5 would have
cancelled if ordering were per the table. Test: `rb_money::cancel_window_sees_the_shaped_amount`
— a payAdd +500 and a window-5 cancel; expect the cancel to be offered against
the modified figure and to drop the payment entirely. Fix sketch: split the
`effect` raise into a pre-window (计算前, declaration only) and move the
cancel/retarget gestures onto the `pay` raise (支付时) — or add a
`payCancel` stage after `payAt`. Keep one `Trigger` so the [反击] key is stable
(`docs/ENGINE.md` 「Effect vs settlement」).

**B3 (missing stage).** 支付阶段 3 「支付计算后」 has no trigger kind
(`TEST-FINDINGS.md:184-188`). Test: only meaningful once a card hooks it; hold
as a TODO(ABI) alongside 行动阶段 3/6/11.

**B4 (DIFFERS).** P7 reverse-on-negative for pay-player entries. Consequence:
a modifier stack that drives a 500-payment to −200 moves nothing (ours) instead
of collecting 200 from the payee (spec). Test: `rb_money::negative_pay_leg_reverses_or_clamps`
— currently asserts clamp; flip the assertion if Q3 rules for reversal.

**B5 (minor).** P6/P14/P15/P16 — no rejection event, zero-print events,
overflow compensation, self-pay path. Consequence is log-shape / edge-case only.
Fix sketch: raise a `payAfter` with `value = 0` and a `paid_refused` reason key
for P6; gate `amount == 0` prints on a `payAfter`-only path for P14; saturating
add + pre-check for P15; drop the `from == to` early-out and let the normal path
settle a zero-net pair for P16.

### 1.3 Counts — Payment

MATCHES 10 · DIFFERS 8 (of which 1 correctness bug B1, 1 known-shape B2) ·
MISSING 2 · AMBIGUOUS 0 · CONFLICT-WITH-RULEBOOK 0 (spec's split machinery is
unruled rather than conflicting) · Not adopted: 6 artefact groups (§0) + the
operation-stack/rejection vocabulary.

---

## 2. Bankruptcy Pipeline (`BANKRUPTCY_PIPELINE.md`)

Our path: `raise_funds` → `bankrupt` (`play.rs:3005-3054`) → `remove_from_game`
(`play.rs:3089-3128`) → `auction_leftovers` (`play.rs:3131-3163`) →
`auction_tile` (`play.rs:3166-3278`). Pinned by `rb_rulebook::b01/b02/b03`
(`tests/rb_rulebook.rs:751,772,808`).

| # | Requirement (spec) | Where | Class | Notes |
|---|---|---|---|---|
| K1 | Enter bankruptcy only after a non-strict pair settled what the payer could provide | `raise_funds` is entered only when `money < loss` and `must` (`play.rs:2840-2843`) | DIFFERS | 规则书 L16 makes bankruptcy *itself* the settlement (「将所有可折现的资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象」). Ours matches the book; the spec's "settle partial, then estate goes to auction, creditor gets only the partial" is **CONFLICT-WITH-RULEBOOK** (quote both in §5.3). |
| K2 | Mark dead **before** `BeforeBankruptcy` so the dead player's effects cannot join | `bankruptBefore` raises first, `players[i].bankrupt = true` only at `play.rs:3024` | DIFFERS | Consequence: during `bankruptBefore` the player is still `!out()` and their field hooks can still fire. |
| K3 | If the bankrupt player owns the turn, park the next alive player without starting their turn | `bankrupt` does not touch `next_turn_pending`; the host tick sets it when the current seat is `out()` (`mod.rs:862-864`). `forfeit` sets it explicitly (`play.rs:3082-3084`) | MATCHES (effect) | The spec's explicit "parked successor starts only after the old counteraction stack empties" is not adopted: our routines are linear and unwind before `NextTurn` starts. |
| K4 | If another player owns the turn, current player and phase unchanged | Same — no turn write in `bankrupt` | MATCHES | |
| K5 | `BeforeBankruptcy` resolves before cleanup; other players may inspect the remaining state | `bankruptBefore` → cash-in → … → `remove_from_game` | MATCHES | |
| K6 | Cleanup: retire effect sources first, then drop property membership, then reset player gameplay state | `remove_from_game` clears money, status keys, hand, extra turns, deeds (`play.rs:3089-3128`) — **not** `s.field` / skills / tokens | **DIFFERS — correctness bug** | 规则书 L81: 「将其控制的所有棋子，角色卡，乐队卡，和手卡移出游戏。所有其正在生效的卡，技能效果停止生效。」 Field cards stay and still run hooks (`wasm_rules.rs:3617-3643` walks every player's `field_instances` with no `out()` filter; `buy_hook_instances` at `3097-3123` likewise). See §5.1 bug B2. |
| K7 | A property enters the auction pool only if it is left with no owners; multi-owner deeds never enter | No multi-owner model; `remove_from_game` un-owns every deed, `auction_leftovers` filters `owners[t] < 0` (`play.rs:3131-3136`) | MATCHES (for our single-owner model) | Multi-owner is out of scope (`docs/PURCHASE.md` ruling 11). |
| K8 | Mortgage markers cleared with ownership | `mortgaged[t] = false` in `remove_from_game` (`play.rs:3111`) | MATCHES | `rb_rulebook::b02`. |
| K9 | Randomly draw and serially auction **at most three** of the saved pool | `rng.shuffle` + `truncate(3)` + serial `auction_tile` (`play.rs:3137-3160`) | MATCHES | 规则书 L82 「抽取3张其中格子的地契进行[拍卖]（少于3则全部）」. `rb_rulebook::b02`. |
| K10 | Auction bidding is a payment pipeline, bids use TotalAssets, mortgage to fund the bid, event-free debit, award only after debit | One live `Ask::auction` bidding war (`mod.rs:958-997`) with min bid 100 / min raise 100; debit is direct `money -= bid` (`play.rs:3228-3229`); a bidder short on cash **voids** the auction (`play.rs:3220-3226`) rather than mortgaging | DIFFERS | Three sub-items: (a) min bid/raise 100 and unlimited raises **MATCH** 规则书 L20 (`rb_rulebook::b03`); (b) cash-only eligibility + void is **DIFFERS** vs L76 「需[支付]或[消耗]资金且资金不足时可以选择抵押」; (c) event-free debit is **DIFFERS** vs L92 (auction is *not* in the immunity list) — already `TODO(规则书)` ruling 7 (`docs/PURCHASE.md`). Spec's "TotalAssets" basis is **CONFLICT-WITH-RULEBOOK** (L20 says 「可[消耗]资金」 = cash). |
| K11 | `AfterBankruptcy` resolves before the suspended payment callback | No after-event; `bankrupt` (`play.rs:3046`) and `beforeOut` (`play.rs:3048`) both fire *before* cleanup | MISSING | Consequence: nothing observes "cleanup + auctions finished". Fix sketch: raise `bankruptcyAfter` (or reuse `leaveAfter`'s shape) after `auction_leftovers`. |
| K12 | Only one bankruptcy / one auction active at a time | Linear routines; `leftovers` is a `VecDeque` drained on forfeit (`play.rs:3081`) | MATCHES | |
| K13 | Marking dead and completed cleanup are not rolled back on later failure | Rust fallthrough; no transaction rollback | MATCHES | |
| K14 | Dead player's effect sources cannot start or resume; only active ancestry stays | See K6 — sources are **not** retired | **DIFFERS** (same bug as K6) | The chain ring itself does skip them as *responders* (`can_counteract_now`, `wasm_rules.rs:3009-3021`) but their automatic field hooks still run. |
| K15 | Bankrupt player's Money and TotalAssets are zero after cleanup | `money = 0` (`play.rs:3092`); `assets` is only recomputed in `finish` (`play.rs:3723-3735`) | MATCHES (money) / DIFFERS (assets field) | Minor: `players[i].assets` is stale until match end. |
| K16 | Auction bid debits and mortgage proceeds are event-free; mortgage *selection* opens mortgage lifecycle | Bankruptcy cash-in is a bare loop (`play.rs:3013-3017`, no per-deed events); auction winner's mortgage is not exercised (K10b) | DIFFERS | Spec wants `Before/AfterMortgage` around each realization. Ours raises none inside `bankrupt`. |
| K17 | Explicit self-payment follows the affordability/bankruptcy path | See P16 | DIFFERS | |
| K18 | Cancellation is not a bankruptcy failure | Cancel paths return `Ok` | MATCHES | |
| K19 | Dead-player removal from the ring never removes neutral; no revival re-inserts | No neutral responder (not adopted); ring skips dead per-visit (`can_counteract_now`) | MATCHES (effect) | |

### 2.1 DIFFERS/MISSING — consequence, test, fix

**B2 (correctness, top).** Bankruptcy does not retire effect sources (K6/K14).
Consequence: a bankrupt player's placed cards keep running buy hooks, pay hooks
and field effects for the rest of the match — e.g. a lingering 「[拥有者]被[支付]时…」
on a dead seat still fires. Test: `rb_rulebook::bankruptcy_stops_field_effects`
— P1 has a placed FEVER!-style payAdd hook; P1 goes bankrupt; P0 then gains
money; expect no boost and `P1.field` empty. Fix sketch: in `remove_from_game`,
clear `s.field` (and `s.tokens`, character/band bindings) after raising
`beforeOut`, and keep only instances whose uid is on the live answer tree if a
cleanup-time unwind needs them (`docs/ENGINE.md`'s "exact active ancestry"
idea — a small `World::retiring: Vec<i32>` allow-list the hook dispatch skips).

**B3 (correctness).** K2 — dead flag after `bankruptBefore`. Test:
`rb_rulebook::bankrupt_before_sees_a_dead_player` — a `bankruptBefore` hook that
tries to move money for the dying player; expect it refused (player already
`out()`). Fix sketch: set `bankrupt = true` (and `out`) before raising
`bankruptBefore`; keep the cash-in after.

**B4 (feature gap).** K10b — auction bid cannot be funded by mortgage; a short
bidder voids. Test: `rb_rulebook::auction_winner_may_mortgage_to_fund_the_bid`
— bidder with 50 cash and a 200-value deed bids 100; expect a mortgage offer and
the deed awarded. Fix sketch: in `auction_tile`, replace the
`money < bid → void` check with `raise_funds(b, bid, None)` and only void if the
player goes out.

**B5 (known TODO).** K10c — auction spend skips the pipeline (ruling 7).
Rulebook evidence it *should* enter: 支付阶段 7's example says
「该技能对有角色破产后的地契拍卖支出不生效」 — i.e. auction spend is a 资金变动
that skills would otherwise see, excluded per-skill. Test:
`rb_rulebook::auction_spend_fires_pay_after` — a payAfter listener on the winner
sees the bid. Fix sketch: route the bid through `Cx::money` with
`kind: "auction"` and a `FORCE_FIXED`-style opt-out only for the price-stages.

**B6 (minor).** K11 missing `bankruptcyAfter`; K15 stale `assets`; K16 no
per-deed mortgage events on bankruptcy cash-in. Log/observability.

### 2.2 Counts — Bankruptcy

MATCHES 9 · DIFFERS 6 (2 correctness bugs B2/B3, 1 feature gap B4, 1 known TODO
B5, 2 minor) · MISSING 1 (K11) · CONFLICT-WITH-RULEBOOK 2 (spec's K1 creditor
model vs L16; spec's K10 TotalAssets bid basis vs L20) · Not adopted: controller
split, `WaitingForBankruptcy` stack, parked-turn + counteraction-stack handshake,
multi-owner membership.

---

## 3. Effect Activation & Resolution Responsibility (`EFFECT_ACTIVATION_RESOLUTION_RESPONSIBILITY.md`)

Our chain model is the **2026-10-07 ruling** (project memory; supersedes the
per-visit/next-seat reading of 规则书 L89): the ask ring starts at the initial
user and each seat — including that user — exhausts or passes before priority
advances; the round closes at the end of a full lap that brings no new
declaration; resolution is LIFO over the answer tree. Implemented in
`game-rules/src/wasm_rules.rs:2454-2938` (`hand_counteractions` / `build_round` /
`resolve_rounds` / `declare_one`) + `Priority` (`2968-3004`) +
`chain_starter` (`2932-2938`). Pinned by `rb_chain.rs` (12 tests).

| # | Requirement (spec) | Where | Class | Notes |
|---|---|---|---|---|
| E1 | Triggering use case raises a root event and waits for the complete counteraction stack | `raise!` runs `CardRules::counteract` synchronously; caller resumes after | MATCHES | |
| E2 | Dispatcher creates immutable event context and opens root/nested windows | `Trigger` built by `bridge_trigger` (`wasm_rules.rs`); nested work is recursion | MATCHES (effect) | Context mutability is per-link (`set_value` etc.), by design so counters can reshape. |
| E3 | Responder ring: alive players starting at the acting player in turn order, then neutral | Ring starts at **initial user** (`chain_starter`: `by_card` if card-caused, else turn player), then turn order; **no** neutral responder | DIFFERS (vs spec) / MATCHES (vs ruling) | Spec's "acting player first then remaining alive then Neutral" ≠ 2026-10-07 ruling. 规则书 L89 says 「从…触发[反击]时点的玩家的下一位玩家开始」; the ruling amends this to include the initial user. **Spec is CONFLICT-WITH-RULEBOOK-RULING** on start seat and on the neutral responder. |
| E4 | Dead players removed when visited; removal is not a pass | `can_counteract_now` (`wasm_rules.rs:3009-3021`) skips out/AI-stand-in/exiled/stunned/no-hand each visit | MATCHES (effect) | Spec's "remove from the ring" vs our "skip per visit" is unobservable. |
| E5 | One activation then advance responder; pass marks the whole group processed and advances the *group* | A seat keeps the floor until explicit pass or no eligible card (exhaust); a declaration re-offers the same seat | DIFFERS (vs spec) / MATCHES (vs ruling) | Spec: "Activating one option resolves only that option and then advances to the next responder." Ruling 2026-10-07: exhaust before advance. Pinned: `rb_chain::one_seat_declares_twice_in_one_visit_before_the_next_seat_is_asked`, `an_explicit_pass_advances_priority`. **Spec is CONFLICT-WITH-RULEBOOK-RULING.** |
| E6 | Lookup groups status → band → character → field → hand → discard → deck → removed | No group concept. Hand counteractions = one flat eligible list per visit; field hooks run automatically before the ring | DIFFERS | Not rulebook-backed in our text (L89 is only about [反击] timing). Content model of their source registry — not adopted unless a card needs ordering across zones. Q5. |
| E7 | Automatic effects take priority over optional in the current group; ties by source order then registration order | Field-card hooks are automatic and run in placement order per player *before* the ring (`wasm_rules.rs:3576-3815`); hand counteractions are all optional and flat | DIFFERS (shape) | Ordering guarantee (automatic first, then declared) matches in effect. |
| E8 | A resolved effect is marked processed for the window | A declared card leaves the hand at declaration (`wasm_rules.rs:2612-2617`); no processed-set for hand options | MATCHES (effect) | |
| E9 | Cancellable target-selection cancel leaves the effect unprocessed and returns to the same group | `ask_*` prompts are mandatory (index clamped); cancel only via an explicit skip option (`ctx.rs:1479+`) | AMBIGUOUS | Cancellability is per-card ad hoc. Spec's framework flag is not adopted. |
| E10 | Every target-selection request names its selecting player; distinct, bounded candidates | `ask_player/ask_tile/ask_card(player_id, …)` + `target()` immune/untargetable/redirect gates (`ctx.rs:1781+`, `abi.rs:1403-1412`) | MATCHES | |
| E11 | Automatic/mandatory effects may request mandatory selection; only a player-activated optional effect may make selection cancellable | No such split (see E9) | AMBIGUOUS | Rulebook L90 「非写明可选择的效果在可发动时必须发动」 is enforced by card bodies + `cant_play`, not by a framework flag. |
| E12 | Nested event opens a nested window; parent resumes only after it closes | Recursion; parent continues after `raise!` returns | MATCHES | |
| E13 | Full traversal with no activation closes the window; a successful activation resets traversal | `Priority::answered` — round closes only after a fully quiet lap (`wasm_rules.rs:2968-3003`) | MATCHES | Pinned: `rb_chain::the_round_closes_after_a_quiet_lap`, `a_later_seats_declaration_re_opens_an_earlier_seat`. |
| E14 | Response must match the current window / responder / prompt revision | Snapshot-replay (`cx.rs`): an answer log entry is consumed by the next matching ask; a stale answer cannot apply | MATCHES (effect) | Mechanism not adopted (§0); the guarantee holds. |
| E15 | Effects that emit events they respond to must prove termination | `reentrant_hooks` (money), `MAX_MONEY_DEPTH = 32` trap, `MAX_COUNTERACT_DEPTH = 16`, `MAX_COUNTERACT_PER_VISIT = 16`, budget counter, hand-card removal (`wasm_rules.rs:39,46,2505-2528`) | MATCHES | |
| E16 | `BaseResolvableEffect`: `CanRespondToEvent` / `CanResolve` / `Resolve` | Manifest hook bitmask + `can_counteract` guard + body (`declare_one`, `wasm_rules.rs:2724-2779`) | MATCHES (shape) | Class not adopted. |
| E17 | Counter resolution priority: every counter settles before the timing it answers; siblings newest-first | `resolve_rounds` post-order, `.rev()` (`wasm_rules.rs:2652-2719`) | MATCHES | Pinned: `rb_chain::counters_resolve_newest_first`, `lifo_resolution_with_multiple_links_from_one_seat`, `counters_to_a_counter_wait_for_the_round_on_x_to_close`. |
| E18 | Negation: activation ("never happened") vs effect ("settled to nothing") vs spare(seat) | `Negation::{Activation, Effect}` + `Trigger.spared` (`rules.rs:42-52, 207-233`) | MATCHES | |
| E19 | Player input is a request; server validates window, revision, addressed responder, option, targets | `Match::act` validation (`mod.rs`); prompt identity by answer-log position | MATCHES | |
| E20 | Presentation never decides legality | Server-side eligibility before prompting (`declare_one` re-probes `can_counteract` per offer) | MATCHES | |
| E21 | Chain-kind taxonomy: what is counterable vs hook-only | `ChainKind` / `HookKind` / `GateKind` three-way split + `is_hook_only` (`abi.rs:1223-1418`, `wasm_rules.rs:3861-3879`) | MATCHES | Richer than the spec's binary. |

### 3.1 Notes on the two real diffs

E3/E5 are **the spec disagreeing with the 2026-10-07 ruling**, not engine bugs.
Do not "fix" the ring toward the spec. If a future sheet text re-bans the
initial user out of the ring, that is a sheet update and wins
(`memory/sheet-updates-supersede-rulings`).

E6 (lookup groups) has no rulebook basis in our text and no card needs it today.
Keep as a question (Q5), not a work item.

### 3.2 Counts — Effect

MATCHES 12 · DIFFERS 3 (all vs-spec; 2 of them CONFLICT-WITH-RULEBOOK-RULING) ·
AMBIGUOUS 2 · MISSING 0 · Not adopted: registry/source IDs, window stack, UI
adapters, lookup-group cursor, neutral responder.

---

## 4. Tile Resolution Pipeline (`TILE_RESOLUTION_PIPELINE.md`)

Our path: `settle` → `settle_at` (`play.rs:1297-1333`) → `CardRules::settle_tile`
→ tile rule instances on `board_field` (`docs/TILES.md`, `rules/tiles/src/*`) with
`land_at_built_in` (`play.rs:1521-…`) as the built-in fallback. Purchase surface
in `engine/play/purchase.rs`.

| # | Requirement (spec) | Where | Class | Notes |
|---|---|---|---|---|
| T1 | Lifecycle: BeforeDefault → default function exactly once → ActivateSpecial → After → terminal TileResolved | `settle` (declaration + [反击] window) → `settleBody` (chain link: field replace + board instances) → `settleAfter` | DIFFERS (shape) | Mapped below. |
| T1a | A pre-default counteraction window | `settle` is the [反击] window for the whole settlement; cancelling it skips body *and* `settleAfter` | MATCHES (function) | 规则书 L15 「执行格子上的所有效果」+ L6 「技能和卡的效果优先」. |
| T1b | Default function runs exactly once, before special effects | `land_at_built_in` / the tile's own rule instance runs once inside `settleBody`; a card may *replace* the body (`settleBody` cancel) or stack beside it | DIFFERS (vs spec) / MATCHES (vs rulebook) | Spec's "default always first, then special window" forbids body-replacement. 规则书 L6 「技能和卡的效果优先于规则书中的所有规则」 favours our replaceable body. Pinned: `rb_general` settle-body cases, `rb_ras` `hey_kids_*` (SettleBody link). |
| T1c | Separate ActivateSpecial round-robin (one effect at a time, children) | Body instances run in instance order; each is its own effect link (counterable); children are recursion | MATCHES (effect) | |
| T1d | `AfterTileResolution` then terminal `TileResolved` | `settleAfter` only — no terminal event | MISSING (minor) | 规则书 行动阶段 16 is 「主要移动阶段后」, not a tile terminal. Q6. |
| T2 | Fresh-position read before every lifecycle stage; relocation never re-runs the default | `after_walk` raises `settleBefore` against live `pos` (`play.rs:1289`), then `settle()` **re-reads** `pos` (`play.rs:1298`) and `settle_at` threads that one `at` through `settle` / `settleBody` / default / `settleAfter` (`play.rs:1306-1331`) | DIFFERS | Two behaviours: a `settleBefore` relocation **does** change which tile settles (the re-read is after it); a relocation inside `settle`/`settleBody` does **not** — the originally landed tile finishes, and the default runs exactly once per `settle_at`. Spec rereads before every stage and would settle the new tile. Our "settle the tile you landed on" matches 行动阶段 15 「触发地块/落点效果」. AMBIGUOUS — Q7. |
| T3 | Root request requires empty stack and no open counteraction window; effect-owned request requires parent + callback | `settle_at(i, at, m)` is callable from a card body (`card_settle_at`, `play.rs:604-612`, and the `ctx::settle_*` primitives); nesting is recursion | MATCHES (effect) | No explicit stack to be non-empty. Card-play nesting is capped at `MAX_NESTING = 8` (`game-rules/src/host.rs:53`). |
| T4 | Child tile resolutions complete before the parent effect resumes; never replaced/cancelled | Recursion returns before parent continues | MATCHES | |
| T5 | Every counteraction window and child op completes before the stage advances; no concurrent siblings | Linear routines | MATCHES | |
| T6 | Tile resolution is phase-independent and never changes `TurnPhaseType` | `settle_at` does not write `st.step` | MATCHES | |
| T7 | General Property branch: purchase / development / acquisition / rent | `land_at_built_in` four-way (unowned / own / other / mortgaged) via `offer_buy` / `offer_build` / `pay_rent` / `offer_force_buy` (`play.rs:1521+`, `1792-2092`); `tile:property` body quotes 规则书 L100-106 | MATCHES | Tests: `rb_rulebook::s07-s12`. |
| T8 | Other tile types explicitly unavailable until defined | All of our six kinds are implemented (`rules/tiles`) | N/A | Their taxonomy ≠ ours (§0). |
| T9 | Rent: unmortgaged → pay current-level rent; mortgaged → optional double-value force-buy | `pay_rent` (`play.rs:1792-1890`), `offer_force_buy` (`play.rs:1933-…`) | MATCHES | 规则书 L105-106; `rb_rulebook::s10/s11`. Force-buy is a direct transfer (L92/L106 「不受任何资金变动效果影响，收款方无论处于何种状态都可正常收款」) — matches; the over-strict "no card may ever touch it" is `TODO(规则书)` ruling 12. |
| T10 | Development (build) on own unmortgaged land, level cap, cannot fund by mortgaging the deed being upgraded | `offer_build` + `why_not_build_on` (`play.rs:2092+`) | MATCHES | `rb_rulebook::s08/s09`. |
| T11 | Agent: all same-colour owned by others → forced half-charge sweep front-to-back (ceil 10); else optional single settle | `tile:agent`'s body (`rules/tiles/src/agent.rs`); `Play::agent_landing` is the built-in fallback | MATCHES | 规则书 L107; `rb_rulebook::s12_agent_half_charge_rounds_up_to_ten`, `s12b`. 「同色」 widening is open (ruling 5). |
| T12 | Pass vs land: [经过] fires per path step, never for the start tile; CiRCLE reward only when 移动起点 ≠ CiRCLE | Walk raises `passBefore` → `passTile` → `pass` per step (`play.rs:1091-1137`); `tile:circle`'s **Pass entry** (`rules/tiles/src/circle.rs`) → `circle::settle_reward`; landing settle is the separate `On::Settle` entry; start-tile / exile checks are the Pass entry's residual guard | MATCHES | 规则书 L86/L95/L96; `rb_rulebook::t05_pass_fires_per_step_and_never_for_the_start`. A walk that both passes over and ends on CiRCLE earns the pass reward **and** the landing draw — two independent effects, as L95+L96 read together. |
| T13 | Settle-body replace vs settle cancel are distinct gestures | `settle` cancel skips body+after; `settleBody` cancel skips body only (`play.rs:1305-1333`) | MATCHES | `docs/TILES.md`; `rb_ras::hey_kids_*`. |
| T14 | Mortgage/redeem/force-buy money is outside all effects unless written | `do_mortgage` / `redeem` / force-buy direct transfer | MATCHES | 规则书 L92. |

### 4.1 Notes

The spec's "default then special" staging (T1b) and its "fresh position before
every stage" (T2) are the only real diffs; both are defensible readings where
the rulebook is silent, and our current behaviour is the one the tests pin. No
correctness bug in the tile pipeline from this spec.

### 4.2 Counts — Tile

MATCHES 11 · DIFFERS 2 (T1b CONFLICT-WITH-RULEBOOK in our favour; T2 AMBIGUOUS) ·
MISSING 1 (T1d terminal) · N/A 1 · Not adopted: their tile taxonomy, the
`TileResolutionController`/`TileController` split, `ResolutionId` snapshots.

---

## 5. Priority

### 5.1 Correctness bugs (do these first)

| id | What | Spec / rulebook | Consequence in a game |
|---|---|---|---|
| **B1** | Card/effect payments never raise funds or bankrupt (`must` only on rent) | 规则书 L16/L76; PAYMENT P11a | A forced 「[支付]/[消耗]」 cannot eliminate anyone; the payer silently underpays and keeps playing. |
| **B2** | Bankruptcy does not retire field cards / skills / their effects | 规则书 L81; BANKRUPTCY K6/K14 | A dead seat's placed cards keep firing buy/pay hooks all match. |
| **B3** | `bankruptBefore` fires while the player is still alive | BANKRUPTCY K2 | Hooks in that window can act for a player who is already dead in the rulebook's sense. |
| **B4** | Auction bid is cash-only and voids if short; no mortgage to fund | 规则书 L76/L20; BANKRUPTCY K10b | A player who could mortgage loses the deed at auction; also `TODO(规则书)` ruling 7 cousin. |

### 5.2 Missing features / known gaps (next)

| id | What | Notes |
|---|---|---|
| M1 | Window-shape: 支付阶段 5 cancel power at window-1 position (B2 in §1) | Already in `TEST-FINDINGS.md:189-192`; needs the trigger-kind split. |
| M2 | 支付阶段 3 / 行动阶段 3/6/11 trigger kinds | `TODO(ABI)`; only matters when a card hooks those exact windows. |
| M3 | `bankruptcyAfter` event | K11. |
| M4 | Auction spend through the money pipeline | K10c, ruling 7. |
| M5 | Pay-player negative → reverse child vs clamp | P7/Q3. |
| M6 | Rejection event for a cancelled pair; zero-print events; overflow compensation; self-pay path | P6/P14/P15/P16, log-shape only. |

### 5.3 Spec items that conflict with the rulebook (do **not** adopt)

| Spec claim | Rulebook | Verdict |
|---|---|---|
| Bankruptcy: settle the available cash to the creditor, then auction the estate; auction bids are event-free deletes to the bank; creditor gets only the partial | L16 「将所有可折现的资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象」 | Keep ours (cash in all → creditor). Spec is wrong for this game. |
| Auction bid basis = TotalAssets, with mortgage realization | L20 「可[消耗]资金的玩家参与喊价…[消耗]同等资金」 | Cash basis. (Funding by mortgage is still allowed by L76 — that is bug B4, not the spec's TotalAssets.) |
| Counter ring: acting player first, then remaining alive, then Neutral; one activation advances the responder | 2026-10-07 ruling (memory) amending L89; `rb_chain` pins | Keep ours. |
| Default tile function runs before any special effect can replace it | L6 「技能和卡的效果优先」, L15 「所有效果」 | Keep our replaceable body. |
| Negative payment reverses direction as a child operation | No rulebook clause; our clamp is pinned for prints (`rb_money.rs:99`, FEVER! 「可小于0」) | Unruled — Q3. |

### 5.4 Ruling questions (options)

**Q1 — non-rent shortfalls (feeds B1).** Should a card-imposed 「[支付]/[消耗]」 the
payer cannot cover (a) offer mortgage then bankrupt (L16/L76 reading, proposed),
(b) be cancelled outright (the spec's "strict" path), or (c) keep underpaying?
*(Proposal: (a), with `must = true` on the card `Pay`.)*

**Q2 — 分摊 modifiers.** For 「其他玩家[分摊][支付]2000」, do amount modifiers apply
to the **total before division** (spec's command-wide pre-split) or to **each
leg's share** (current ruling, `rb_gap_money.rs:298`)? *(Current: per-leg.
Changing would move the ceil-10 split into `money_inner`.)*

**Q3 — negative pay-player final.** Clamp to 0 (current; pinned for prints) or
reverse direction as a positive child with swapped endpoints (spec)?
*(Ruled 2026-10-07, user: adjustments compose freely — no clamp between
stages. When the counteraction chain has resolved and the final amount is
negative, the original payment is adjusted to 0 and a **new payment** starts
with the participants swapped and the absolute value; it runs the full
pipeline (its own modifiers, counteractions, Q1 shortfall → mortgage →
bankruptcy). Implementation pending, after the settle-stage batch.)*

**Q4 — self-payment.** Should 「A[支付]A X」 run affordability/bankruptcy (spec) or
be a no-op (current)? Rulebook L14 does not exclude it.

**Q5 — lookup groups.** Do we ever need the spec's status → band → character →
field → hand → … ordering (e.g. a character skill that must be offered before a
hand counter)? Options: (a) no — keep the flat hand list, (b) yes — add a source
order to the offer list. *(No card needs it today.)*

**Q6 — terminal tile event.** Do we want a `TileResolved`-style terminal hook
(distinct from `settleAfter`) for 「结算完成时」 cards? *(Nothing in the current
sheet asks for it.)*

**Q7 — mid-settle relocation.** If a counteraction moves the player during a settle,
do we finish settling the tile they landed on (current) or reread the position
and settle the new tile (spec)? *(Current matches 行动阶段 15 「落点效果」. Note
the split we already have: `settleBefore` relocation does redirect the settle,
later-stage relocation does not.)*

---

## 6. Totals

| Spec | MATCHES | DIFFERS | MISSING | AMBIGUOUS | CONFLICT-WITH-RULEBOOK |
|---|---|---|---|---|---|
| Payment | 10 | 8 | 2 | 0 | 0 |
| Bankruptcy | 9 | 6 | 1 | 0 | 2 |
| Effect | 12 | 3 | 0 | 2 | 2 (ruling-level) |
| Tile | 11 | 2 | 1 | 0 | 1 (in our favour) |

Top issues: **B1** (card payments never bankrupt), **B2** (bankruptcy leaves
effect sources alive), **B3** (`bankruptBefore` timing), **B4** (auction funding),
then the known window-shape gap **M1**.

Existing black-box coverage that already pins the audited behaviour:
`rb_money.rs` (4), `rb_gap_money.rs` (7), `rb_rulebook.rs` (mortgage/redeem/
force-buy/agent/pass/bankruptcy/auction, `b01-b03`, `gf05/gf06`, `s07-s12`,
`t05`, `o06`), `rb_chain.rs` (12), `rb_guards.rs` (10). None of the four bugs
above is pinned today — each proposed test in §5.1 is new.

---

## 7. Implementation status (2026-10-07 batch)

Status per decided item. User rulings of 2026-10-07 (with the two mid-batch
coordinator updates: Q4 follows the affordability/bankruptcy path in full;
"reaction" is spelled "counteraction" throughout).

### Q1 / B1 -- card-imposed shortfall (done)

A card's forced 「[支付]/[消耗]」 now runs the same raise-funds-then-bankrupt
path rent does. `HostRequest::Pay` sets `Pay::must = true`
(`game-rules/src/wasm_rules.rs`, the `HostRequest::Pay` arm), so a shortfall
routes through `raise_funds` (L76 抵押 offer) and then `bankrupt` (L16).
Voluntary purchases (buy / build) keep their pre-gates and stay `must = false`
(`play.rs` `Pay::new(price, "buy")` / `"build"`).

* Tests: `rb_money::card_shortfall_raises_funds_then_bankrupts`,
  `rb_money::card_shortfall_may_be_funded_by_mortgage`.
* Files: `crates/game-rules/src/wasm_rules.rs`.

### Q4 -- self-payment 「A[支付]A」 (done, per the coordinator update)

`money_inner` no longer early-outs on `from == to`. The counteraction windows
run in full (a counteraction may redirect the payee) and the affordability /
raise-funds / bankruptcy path applies exactly as to any other payment; only the
net balance effect is zero when the pair settles -- the debit and the credit
cancel (`BANKRUPTCY_PIPELINE` invariant, `P16`/`K17`).

* Tests: `rb_money::self_payment_leaves_money_unchanged_when_affordable`,
  `rb_money::self_payment_may_be_funded_by_mortgage`,
  `rb_money::self_payment_shortfall_raises_funds_then_bankrupts`.
* Files: `crates/game-core/src/engine/play.rs`.

### Q2 -- 分摊 modifiers, the pre-split stage (done)

New hook kinds `payTotalAdd` / `payTotalMul` / `payTotalCancel` (TriggerKind
84/85/86) run **command-wide on the total** before any 「[分摊]」 divides it --
the spec's `PreSplitAddDiff` / `PreSplitMultDiff` / `PreSplitCancel`, renamed
after our per-share `payAdd` / `payMul` / `payChoose` / `payAt`. Order is
add -> mul -> cancel, composing. `Cx::pay_total` / `ctx::pay_total` expose the
stage on its own; `ctx::split_pay` / `ctx::pay_leg` drive a 「[分摊]」 command
(total stage once, then one `money()` per share with `Pay::total_stage = false`).
For a single-pair payment the command total *is* the amount, so the same three
stages run inside `money_inner` after the `effect` declaration and before the
per-share stages.

Cards moved onto the new stage (both say 「分摊前」):

| Card | Text | Was | Now |
|---|---|---|---|
| `skill:丸山彩:With~` (2) | 「此次支付的分摊前资金减少Y×100（最少0）」 | `payChoose` | `payTotalAdd` |
| `skill:白鹭千圣:保持坦率的你` (2) | 「此次获得的分摊前数量增加Y×100」 | `payChoose` | `payTotalAdd` |

Every other card stays on its current stage. The 「[分摊]」 bodies
(`PP:[白鹭千圣]微笑的铁假面`, `R:NFO`, `skill:鳰原令王那:梦幻可爱♪女仆`,
`R:FRONT OR BACK`, `skill:Poppin'Party:星之鼓动山丘`) now route their totals
through the pre-split stage so those two 「分摊前」 clauses reach them; their
own per-leg rounding is unchanged.

* Tests: `rb_money::pre_split_modifier_shapes_the_total_not_each_leg`.
* Files: `rules/card-sdk/src/abi.rs`, `rules/card-sdk/src/ctx.rs`,
  `crates/game-core/src/engine/play.rs`, `crates/game-rules/src/host.rs`,
  `rules/skills/skill-characters/src/aya_with.rs`,
  `rules/skills/skill-characters/src/chisato_frank.rs`,
  `rules/skills/skill-characters/src/numazu_maid.rs`,
  `rules/cards/card-pp/src/chisato_mask.rs`, `rules/cards/card-roselia/src/nfo.rs`,
  `rules/events/src/front_or_back.rs`, `rules/skills/skill-bands/src/poppin.rs`.

### B2 -- bankruptcy stops field effects (done)

`remove_from_game` clears the seat's `field` (角色卡 / 乐队卡 / every 「[持续]」
card, including the bound `skill:*` instances) and `actions` (armed skill
offers) -- 规则书 L81 「将其控制的所有棋子，角色卡，乐队卡，和手卡移出游戏。
所有其正在生效的卡，技能效果停止生效」. The hook / buy-hook dispatch also skips
`out()` seats (`wasm_rules.rs`), so an instance any later path re-places cannot
answer.

**Deviation from the ruling's 「tokens」.** `s.tokens` (「标志物」: 火罐 /
奇迹水晶 / P✽P粉丝 ...) are **kept**. L81's removal list is 棋子 / 角色卡 /
乐队卡 / 手卡 plus 「正在生效的卡，技能效果」; 「标志物解释」 is a separate
vocabulary and L81 does not sweep the markers. `rb_pp::
shanyao_counter_on_short_payment` pins this: a P✽P fan the rescue granted
survives the bankruptcy that follows. Say the word and the markers go too.

* Tests: `rb_rulebook::bankruptcy_stops_field_effects`.
* Files: `crates/game-core/src/engine/play.rs`, `crates/game-rules/src/wasm_rules.rs`.

### B3 -- `bankruptBefore` timing (done, rulebook-consistent)

The seat is marked dead **before** the `bankruptBefore` window. 规则书 L16
defines 破产 as the state entered when the payment fails, L81 「所有其正在生效
的卡，技能效果停止生效」 is what entering it does, and L17 「[存活]：非[破产]
状态且未离开游戏的玩家」 excludes it. So the flip agrees with the rulebook (the
spec's K2 was right). Other players' `bankruptBefore` hooks still fire and may
inspect the remaining state (K5) -- the cash-in has not run yet.

* Tests: `rb_rulebook::bankrupt_before_sees_a_dead_player`.
* Files: `crates/game-core/src/engine/play.rs`.

### B4 -- auction winner may 抵押 to fund the bid (done)

`auction_tile` calls `raise_funds(b, bid, None)` when the winner is short and
only voids if they go out (L76 「需[支付]或[消耗]资金且资金不足时可以选择抵押」).
The bid itself stays a direct delete (L20 「[消耗]同等资金」; ruling 7 is still
`TODO(规则书)`). Bid **eligibility** is now cash plus what the seat could raise
by 抵押 (`Match::answer`), not cash alone -- a short bidder could not even bid
before.

* Tests: `rb_rulebook::auction_winner_may_mortgage_to_fund_the_bid`.
* Files: `crates/game-core/src/engine/play.rs`, `crates/game-core/src/engine/mod.rs`.

### Q5 -- deterministic effect ordering (done)

`LookupGroup` + `effect_order_key(group, source, decl)` in
`game-rules/src/wasm_rules.rs`: the spec's group order
status -> band -> character -> field -> hand -> discard -> deck -> removed (plus
`Board` for our neutral `tile:*` / `event:*` / `mark:*` rules, sorting after the
player groups), source order within the group from the authoritative state
lists, declaration order within the source. No `HashMap` iteration, no clock.

Wired at the hand-counteraction offer list (`declare_one`) and the buy-hook
candidate list (`buy_hook_instances`); the field-hook walk is already in source
order (player-major, placement). Field hooks before hand counteractions is
group `Band`/`Character`/`Field` < `Hand`, so the phase split the dispatch
already has is the key's own order.

**Sim A/B (2026-10-07, `sim 50 4 200`, seed-stable):** identical event counts
with and without the `buy_hook_instances` sort --
`{"bankrupt": 96, "build": 4669, "buy": 2899, "draw": 2320, "event": 1187,
"forcebuy": 392, "lose": 7313, "mortgage": 2245, "move": 5347, "overlap": 1517,
"pass": 5909, "play": 2709, "redeem": 1517, "rent": 19567, "roll": 34398,
"text": 55654, "turn": 34437}`. Today's behaviour is unchanged.

Note one place the key records a deviation rather than moving anything:
`bind_skills` places a player's **character** skill before their **band** skill
(`GameData::skill_rules_of`), so within one player's field the source order is
character -> band while the spec's group order is band -> character. The key
classifies them correctly (the group component says band first); because the
sort is stable and the two never share a window in a way that shifts an outcome,
the sim counts are identical. Adopting the spec's cross-player group-major scan
would move things and is **not** done (E6 has no rulebook basis in our text).

### Q6 -- terminal `<thing>Resolved` hooks (done, selectively)

| Candidate | Verdict | Why |
|---|---|---|
| `tileResolved` | **added** (87) | The spec's `TileResolved` is the terminal of the tile lifecycle, distinct from `AfterTileResolution`. Fires after `settleAfter`, and also when the settle was cancelled (the resolution is complete as nothing). |
| `moveResolved` | **added** (88) | Nothing fired at the end of a move. Now fires at the end of `after_walk` (including a 「不[触发结算]」 move) and on the non-settling teleport path. |
| `bankruptResolved` | **added** (89) | K11's gap: nothing observed "cleanup + auctions finished". Fires after `auction_leftovers`. |
| `payResolved` | **skipped** | `payAfter` is already the terminal for one payment command -- 规则书 支付阶段 6 「支付后」 merged with 7 「资金变动」 (`docs/ENGINE.md` 「资金变动 is `payAfter`」). |
| `buyResolved` | **skipped** | `bought` + `buyAfter` already fire after the deal commits (`BuyAssign` -> `assign_deal` -> `bought` -> `buyAfter`). |
| `eventResolved` | **skipped** | `eventAfter` already fires "the event is fully resolved and filed away". |

* Files: `rules/card-sdk/src/abi.rs`, `crates/game-core/src/engine/play.rs`.

### Not decided (unchanged)

* **Q3** negative-payment reversal: **done** (2026-10-07 late batch). The
  per-stage `max(0)` clamps in `money_inner` / `pay_total_stages` /
  `scale_settle_payment` are gone; adjustments compose freely. After the chain
  resolves, a negative final settles the original at 0 and starts a new payment
  with the participants swapped and the absolute value (one-sided too: a
  negative 「[获得]」 becomes a 「[消耗]」 and vice versa). The child runs the
  full pipeline including Q1 shortfall, and flip-flops are capped at
  `MAX_PAY_REVERSALS = 8` (logged). Tests: `rb_money::
  two_sided_negative_final_reverses_the_payment`,
  `one_sided_negative_gain_becomes_a_loss`,
  `one_sided_negative_loss_becomes_a_gain`,
  `reversal_shortfall_raises_funds_then_bankrupts`. The old
  `negative_final_amount_clamps_to_zero` is renamed
  `negative_gain_modifier_shrinks_the_gain` (its scenario nets positive).
* **Q7** mid-settle position re-read: unchanged. `TODO(规则书)` (PIPELINE-AUDIT Q7) --
  `settleBefore` relocation redirects the settle, later-stage relocation does not.

### ABI

`card-sdk` ABI **40 -> 42** across the two batches (v41 was the purchase-surface
removals; v42 is this batch's pre-split stage + terminals). `SAVE_VERSION`
unchanged -- no save field entered or left.

### Q3 — negative-payment reversal (done, late 2026-10-07 batch)

Adjustments compose freely: the per-stage `max(0)` clamps in `money_inner`,
`pay_total_stages` and `scale_settle_payment` are gone. After the counteraction
chain resolves, a **negative final** settles the original at 0 and starts a
**new** payment with the participants swapped and the absolute value. Applies
to one-sided movements too (a negative 「[获得]」 becomes a 「[消耗]」 and vice
versa). The child runs the full pipeline (its own modifiers, counteraction
windows, Q1 shortfall → mortgage → bankruptcy). A child that is itself driven
negative reverses again, capped at `MAX_PAY_REVERSALS = 8` (logged as
`log.pay_reversal_capped`).

* Tests: `rb_money::two_sided_negative_final_reverses_the_payment`,
  `one_sided_negative_gain_becomes_a_loss`,
  `one_sided_negative_loss_becomes_a_gain`,
  `reversal_shortfall_raises_funds_then_bankrupts`.
* `rb_money::negative_final_amount_clamps_to_zero` is renamed
  `negative_gain_modifier_shrinks_the_gain` (its scenario nets positive and
  stays valid; the old name misdescribed the rule).
* Files: `crates/game-core/src/engine/play.rs`.

### V5 — `pay` settlement honours `set_cancelled()` (done)

A counter body that cancels the `pay` link stops the payment outright.
Files: `crates/game-core/src/engine/play.rs`.

### Marker windows + ownership (done, 2026-10-07 late batch)

* **(a)** `markerSpend` / `markerGain` (TriggerKind 92/93, ChainKind) open
  **before** the markers move (`HostRequest::Marker`). A cancelled link moves
  nothing. No shipped card listens yet; `rules/fixtures/test-cards`'s
  `TEST:markerDeny` pins the shape. Test:
  `rb_money::marker_spend_window_cancels_the_spend`.
* **(b)** Marker costs keep today's timing: spent as the effect resolves,
  nothing spent if the effect is negated. Press checks (「消耗7个[火罐]」) stay
  activation guards. HHW:（美咲） may counteract with 0 火罐 -- 「消耗所有火罐」
  is vacuous when the pot is empty (`rb_money::misaki_may_counteract_with_zero_fire`).
* **(c)** A marker is owned by the **rule that creates it**, wherever its
  copies sit (`World::marker_owner`). `skill:要乐奈:投币式停车场的猫` owns all
  抹茶芭菲 -- its player's counter, every other player's counter, and those on
  tiles.
* **(d)** Bankruptcy clears everything the player holds (火罐 / 奇迹水晶 /
  P✽P粉丝 / tokens / statuses) plus every marker owned by one of their rules
  wherever it sits. Neutral board marks ([CP点], owner -1) stay. This reverses
  the previous batch's keep-tokens deviation; `rb_pp::
  shanyao_counter_on_short_payment` now expects the fan to be cleared.
  Test: `rb_money::bankruptcy_clears_owned_markers_wherever_they_sit`.

### Bot-only estimated execution cost (done, 2026-10-07 late batch)

Each card carries an *estimated execution cost* (`prop::EST_COST`) in its rule
metadata, used **only** by bots / autopilot as a reserve check -- never for
legality. Constant for now (an X-dependent cost may later become a
`rules-cond` expression). Exposed via `CardRules::card_prop` and the view's
per-card `estCost` (parallel to `playable`), so `ai.rs`, `bot-core` and
`webui/src/game/autopilot.ts` read one source. Standard bots keep
`BUY_RESERVE`, chaos keeps `CHAOS_RESERVE`. Filled for cards with fixed
payments (the six formerly-gated ones plus the other flat `ctx::pay(N)`
bodies); `0` otherwise.

* Files: `rules/card-sdk/src/abi.rs`, `crates/game-core/src/state.rs`,
  `crates/game-core/src/engine/{mod.rs,ai.rs}`, `crates/web-glue/src/lib.rs`,
  `crates/rules-worker/src/lib.rs`, `crates/bot-core/src/{view.rs,action.rs}`,
  `crates/bot-service/src/lib.rs`, `webui/src/core/types.ts`,
  `webui/src/game/autopilot.ts`.

### ABI / SAVE

`card-sdk` ABI **43 -> 44** (v44 = `markerSpend` / `markerGain`). `SAVE_VERSION`
**4 -> 5** (v5 = `World.marker_owner`).
