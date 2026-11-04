# ABI design G14 — CardDef AI hooks and hand limit

**Status:** draft for user review · design only, no source touched.
**Worktree:** `.claude/worktrees/g14-design-review` (branch `worktree-g14-design-review`).
**Group:** G14, family "CardDef AI hooks and hand limit".
**Ground truth:** `docs/rulebook/cards.json`; every fragment below is verbatim from it
and was re-read on 2026-11-04 against the card id named beside it.
**Row ids** are the review-sheet ids carried by `G14.json`; they are **not**
`cards.json` indices.

**Base:** master at ABI **113**, `SAVE_VERSION` **16**. G14 landed before G09–G12,
so it takes the next free numbers, **114** / **17**; §9 and the README
landing table carry the arithmetic and the later rows' shift.

Terminology. **The seat's AI** means either bot policy (standard or advanced) or
the browser autopilot (托管); **chaos** is the separate disruptive policy. A
**bot offer** is a card a policy may choose, which is never legality -- a human
still plays it by hand. An **allowed excess** is an over-limit hand the card
permits and that therefore does not block ending the turn. The client-side AI
runs in the user's browser; a server bot exists only for a bot seat. Neither may
read hidden state. Never "reaction".

**Closed slugs and rows:** both items are covered; none is demoted.

| # | slug | severity | rows | verdict |
|---|---|---|---|---|
| 1 | `hand-limit-lift` | medium | 441 | B |
| 2 | `aiplay-hook` | low | 1253 | B |

`hand-limit-lift` names an existing but dead parameter
(`crates/game-core/src/engine/ops.rs:1168`, `let _ = over_hand_limit` at `:1202`),
not a live mechanism: the guest draw request has no flag and the routine that
trims never sees one. `aiplay-hook` has a live consultation point
(`crates/bot-core/src/policy.rs:370`) but no way for a card to answer it, and the
client-side extra that the autopilot reads (`playable`) is legality only.

---

## 0. Review record

This revision came out of a full review of the 2026-10-28 draft. Every cited
site was re-read against master, every engine claim re-derived, and the band
arithmetic re-run from the landing order. What the draft got wrong:

| # | Draft defect | Correction |
|---|---|---|
| R1 | The draft based the work on `card-rulebook-review` and ABI 116→117. Master is ABI 113; G14 lands before G09–G12, so §9 takes the next free numbers 114 / 17 and the README rows of the later groups shift one up. | Header states the real base; README reconciled 2026-11-04. |
| R2 | Nearly every citation was stale (`ops.rs:1024`, `wasm_rules.rs:4347`, `ai.rs:329`, `hostfns.rs:846`). G07 and the policy split moved them. | Every site is re-cited: the dead parameter 1168/1202, the `Draw` arm 5199-5229, the policy seam `bot-core/src/policy.rs:339-375`, `hostfns::draw` 1073. |
| R3 | The draft let the hand stay over the limit until turn end but never revisited the `end` gate. `cant_act` refuses `end` while `over_hand` (`engine/mod.rs:3014`), so the seat could never *reach* turn end and the card's own scheduled discard would never run. The server view's `can_end_here` (`:923-937`), the client's `turnFlow` / `Side` / autopilot gates carried the same refusal. | §5.2 adds the allowed-excess fact; the end gate, the view preview and the client gates read it. §10 pins the whole story. |
| R4 | The draft said `SAVE_VERSION` unchanged. An over-limit hand is a live, savable state whose end gate depends on the allowance, so a save taken over the limit must carry it. | `SAVE_VERSION` 16→17 (`MatchPlayer.over_hand_allowed`, `#[serde(default)]`). §8. |
| R5 | The draft never mentioned the formal hand-limit clause `G-CARDS-01` (`rules/spec/src/global/cards.rs:184-238`), which fails plain on any act that leaves a hand over its limit. | §10.2 amends the clause with the allowance fact and adds a spec test. |
| R6 | The draft said "no locale key" and simultaneously suppressed the over-limit warning, leaving the player no explanation why six cards on a limit of five is fine. | A new part key `log.part.over_hand_allowed` replaces the warning on the exempt draw only. §5.6. |
| R7 | The draft described chaos via a "numeric `params`" argument at `autopilot.ts:803/:807`. Since the policy split chaos is a separate policy function; the numeric argument is `CHAOS_RESERVE`. | §5.10 / §6.1 use the real discriminator. |
| R8 | The draft's `ai_playable` was `playable && ai_play`. The speculative root resets `playable` to all-true (`bot-core/src/view.rs:280-295`), so an `aiNever` card that happens to be unplayable in the sampled frame would lose its mark and be offered. | `aiPlayable` is the bare static `ai_play` answer; every consumer combines `playable && aiPlayable`. §5.9. |
| R9 | The draft invented a generic `import` module for the flag word, while the house pattern is a named flag module (`abi::pay_flag`, `abi::invoke`). | `abi::draw_flag`, one bit. §5.1. |
| R10 | `draw_over_hand` clipped "hand limit". | `ctx::draw_over_hand_limit` / `HostRequest::Draw.over_hand_limit`. §4. |
| R11 | The draft had no necessity section, no naming section and no clause match. | §4, §13 and §14 exist. |
| R12 | The draft's "suppress the over-limit warning" applied to `ops::draw_cards`'s `over_hand_limit` too, but that routine has no trim loop; the parameter's meaning was never stated. | The parameter means "exempt draw" on both routines; `ops::draw_cards` sets the allowance and the log part, and the trim loop lives in the match routine. §5.4, §5.6. |
| R13 | The draft named only the `HostRequest` arm and the `CardWorld` trait but missed the positive-inline `Run::draw` path, which also moves cards for fixtures. | `CardWorld::draw_over` is implemented on `Run`; §5.5. |
| R14 | The draft claimed the `drawOut` window was unaffected but never stated where it sits relative to the flag. | §5.6: the window still runs first and can still cancel; the flag only reaches the draw routine. |
| R15 | The draft's Kani section named the unused `nnm_02_effects_ok`, which expected **one** `Effect::Draw` of `n = x` although `ctx::draw` issues one import per card. There is no nnm-02 proof today. | §10.3 replaces it with an IFF over one `DrawOver` per card and adds the mutant. |
| R16 | The draft's tests were named without files. | One new file on the rules side, `rules/tests/tests/g14_ai_and_hand_limit.rs`; the client cases join the existing `webui/src/game/autopilot.test.ts` harness, and the toolbelt cases stay where their harness lives. |
| R17 | The draft's §5.2 moved the hand-limit read to "the trim loop" but never said what happens when the scheduled discard itself was skipped (card left, prompt open, cap lowered below five). | §5.6 adds the unconditional turn-end sweep to the cap, then clears the allowance. |
| R18 | The allowance could go stale: after a manual discard brings the hand within the limit, a later 「加入手牌」 (not a draw) could raise it again and the end gate would wrongly stay open. | Every hand-shrink site calls `settle_over_hand_allowance`; §5.2 enumerates them. |
| R19 | The draft's `BotView.ai_never` set does not exist; the type is `SeatView`. | §5.10 uses `SeatView::offerable`, and the predicted frame simply keeps `ai_playable`. |
| R20 | The draft's necessity for item 2 was one sentence and never eliminated the strategy book, which already has a per-card `play_weight_milli`. | §13.3 rejects the book with the reason (offline, persona-keyed, not shipped with the card). |
| R21 | The draft had no note on `docs/BOT.md` §1.1's hidden-state audit, although `ai_play` becomes a new gate the client computes. | §5.10 adds the audit row (static prop read, always equal). |
| R22 | The draft's necessity was four bullets, each asserting rather than showing: no trace, no demotion path, and no priced near-miss — although **A3 would actually make the card work**. | §13 now carries literal traces with card ids and counts, a demotion table, the priced A3/A13 near-misses, per-element severities and a residual-risk list. §11 gains A13. |

## 1. Items, per item

### 1.1 `hand-limit-lift` (row 441)

Book (`cards.json#Mor:（NNM）稍微努力了一下`), clause (1):

> （1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张

The body raises the guest's own `handLimit` state to 「no limit」, draws, and
restores (`rules/cards/card-morfonica/src/nanami_effort.rs:220-237`):

```rust
let cap = ctx::state::get(player_id, card_sdk::abi::state_key::HAND_LIMIT);
ctx::state::set(..., HAND_LIMIT, NO_HAND_LIMIT);
ctx::draw(player_id, x)?;
ctx::state::set(..., HAND_LIMIT, cap);
```

`ctx::draw` becomes one `ruleset_draw` import per card
(`crates/card-sdk/src/ctx.rs:885-921`), the host parks `HostRequest::Draw`
(`crates/game-rules/src/hostfns.rs:1073-1098`), and the draw arm
(`crates/game-rules/src/wasm_rules.rs:5199-5229`) calls
`draw_cards_with_hooks(player, n, true)` (`engine/play.rs:5497`). That routine
ends with an immediate discard-down (`:5575-5583`).

`over_hand` reads `MatchPlayer::hand_limit()` (`engine/play.rs:5455`,
`state.rs:827-844`), which reads the **live** `handLimit` state only. The
guest's `NO_HAND_LIMIT` stays on the throw-away run copy: the routine-boundary
overlay copies a guest state key only when the guest moved it **down**
(`engine/cx.rs:1018-1023`, "Decrease only"). `i32::MAX` is an increase, so the
trim still uses the old cap and the scheduled `discard_down_to_five`
(`nanami_effort.rs:242-259`) finds nothing to do.

The supposedly-generic hook is already in the signature and unused
(`ops.rs:1168`, `:1202`).

**The second half of the bug the draft missed.** With the trim fixed, the hand
is over the limit until turn end -- but `cant_act`'s `end` gate refuses an
over-limit hand (`engine/mod.rs:3014-3017`), so the turn cannot end and the
card's own scheduled callback never runs. The base rule
「任何时刻…立刻弃至上限」 (`rulebook-doc.md:126`) and the card's 「可超过上限」
have to be made to coexist: the excess is *allowed* until the card's own
dispose at turn end. The same refusal sits in the server's `can_end_here`
preview (`mod.rs:923-937`) and in the client's `overHand` gates
(`webui/src/scenes/board/{model.ts,turnFlow.ts,Side.tsx}`,
`webui/src/game/autopilot.ts:944`), and in the formal `G-CARDS-01` clause.

Expected: the draw exceeds the cap; the hand stays over-limit and `overHand`
remains true; **`end` is accepted anyway** because the excess is allowed; the
turn-end callback discards down to five; the allowance is gone afterwards. The
`handLimit` state itself is unchanged -- 「可超过上限」 qualifies this draw, not
the cap.

### 1.2 `aiplay-hook` (row 1253)

Card text (`cards.json#Mor:夏日合宿`):

> 打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌

The sheet says nothing about bots; this item is a heuristic, not a clause. The
card is purely defensive and its guaranteed value is a single draw *if* nothing
was negated; a standard policy that plays every legal card spends it on the
first 运营 for no incoming targeting.

The engine seam exists and the standard policy uses it:
`ai_card_choice` filters hand cards through `self.rules.ai_play(self, i, &id)`
(`crates/bot-core/src/policy.rs:339-375`, the call at `:370`). But
`Rules::ai_play` (`engine/rules.rs:929-931`) defaults `true` and
`RulesBridge<M>` (the `WasmRules` type) never overrides it, so every card
answers "playable". Chaos deliberately ignores the hook (`policy.rs:346-357`).

The human-side takeover has the same hole from the other direction: the browser
autopilot plays from `view.playable` (`webui/src/game/autopilot.ts:709-723`),
which is `cant_play` legality only (`engine/mod.rs:1395-1426`); `docs/SERVER.md`
states the limitation. A standard bot and a 托管 human both offer 夏日合宿,
which they should never play.

Expected: a card can statically declare "bot policies must not offer this";
standard / advanced skip it; the declaration rides the manifest to the
client-side AI; **human legality is untouched** (a human still plays it by
hand).

### 1.3 Why one group

Both gaps are the same shape: a **static card fact** is known at ruleset-build
time, but the engine routine or policy that needs it reads only ambient state
that the card cannot reach.

* Item 1: the card reaches state (`state::set`), but the draw routine's trim
  reads live state, and the guest overlay is deliberately one-way (decreases).
* Item 2: the engine has the policy seam (`ai_play`) but no carrier from the
  card definition.

Neither needs a new decision mechanism. Item 1 needs one bit on an existing
host request and one durable permission bit; item 2 needs one static
declaration on the existing generic `props` map, which already rides the
manifest to the client.

---

## 2. Shared root cause

The host boundary is one-way for state (a guest raise may only *clear*), and the
AI seam is one-way for policy (the engine asks, the card cannot answer).
Both are correct defaults: a general state-increase crossing is a separate
open ruling (A3), and a per-card wasm AiPlay hook is a separate, larger seam
(A2). This group takes the narrow, exact forms.

---

## 3. Capability as an invariant

> **INV-G14-1 — an effect can declare one draw exempt from the immediate
> hand-limit trim, and the excess it creates is allowed until the hand fits
> again or the turn ends.** The draw still moves cards, still raises the
> per-card before/after points, and still logs; only the trim and the blocking
> force of the excess are lifted. `overHand` stays true (it is the hand size,
> and the discard affordance, glow and discard legality stay as they are); only
> the end gates switch to the blocking form, `over_hand_blocking`. The `handLimit`
> state and the `HAND_LIMIT_DELTA` field-prop ladder are untouched. The
> turn-end path discards down and clears the allowance, so no turn starts over
> the cap.
>
> **INV-G14-2 — a card can statically declare that bot policies must not offer
> it.** The declaration is per card id, lives on the manifest, is readable by
> the client-side seat-view engine without a guest call or hidden read, and
> never changes `cant_play` / `playable` legality.

Derived obligations:

* **Default preserved.** `ctx::draw` without the flag behaves exactly as today;
  a card with no `aiNever` prop is offered exactly as today.
* **Exactness.** `draw_over_hand_limit` sets exactly the flag bit; the host
  skips exactly the trim and swaps exactly the log part; no other draw
  behaviour changes.
* **Gates agree.** Engine `end`, engine `can_end_here`, client `overHand`
  gates, and the formal `G-CARDS-01` all read the same allowance fact.
* **Bounded allowance.** The allowance is set only by an exempt draw that left
  the hand over, and is cleared at every hand-shrink site that brings the hand
  within the limit, at turn end, and at turn start.
* **Static facts only.** `aiNever` is readable from the manifest alone: no
  hidden zones, no prompt, no RNG. `aiPlayable` is therefore exact in the
  seat-view engine (new `docs/BOT.md` §1.1 row, "static card props, always
  equal").
* **All three targets.** The draw flag compiles for wasm32, native
  (`native-rules`) and Kani, with the Kani model asserting flag propagation.
* **No display text in the ABI.** `aiNever` is an ASCII prop key;
  `draw_over_hand_limit` has no locale key. The one log part is a locale key,
  which is the display side, not the ABI.
* **No player decision inside an import.** Both are engine facts.

---

## 4. Names and naming decisions

| concept | name | why not the draft's |
|---|---|---|
| the guest sugar | `ctx::draw_over_hand_limit` | `draw_over_hand` clipped the noun; the limit is the hand **limit**, not the hand |
| the import flag module / bit | `abi::draw_flag::OVER_HAND_LIMIT` | a new generic `import` module has no peers; `pay_flag` / `invoke` are the pattern |
| the request/engine parameter | `over_hand_limit` | the dead parameter already has this name on `ops::draw_cards` and `draw_cards_with_hooks` |
| the allowed excess | `MatchPlayer::over_hand_allowed` / wire `overHandAllowed` / `over_hand_blocking` | "lifted" collides with `handLimit`; "allowed" says permission, not a changed limit |
| the bot-avoid prop | `abi::prop::AI_NEVER = "aiNever"` | `botSkip` breaks the stem of `ai_play` / `aiAnswer`; `aiNever` is a camelCase prop like `estCost` |
| the static offer extra | `ViewExtra::ai_playable` / wire `aiPlayable` | `aiPlayable` pairs with `playable`; it means "a bot would offer it", never legality |

`ai` is an abbreviation but it is the established stem across the engine
(`ai_play`, `aiAnswer`, `ai_wants_*`); renaming it would be a separate sweep.

The prop constant is mirrored in `game_core::state::prop::AI_NEVER` (like
`EST_COST`) because the engine, not the host, reads it; `game_core` does not
depend on `card-sdk`. The draw flag needs no mirror: it is decoded in
`game-rules`, which already uses `card_sdk::abi` (`hostfns.rs` imports
`abi::EXIT_NEED_INPUT`), and in `card-sdk`'s own Kani arm.

---

## 5. Surface

Everything is generic mechanism. Summary: **new** — `abi::draw_flag`,
`ctx::draw_over_hand_limit`, `CardWorld::draw_over`, `MatchPlayer::over_hand_allowed`,
`over_hand_blocking`, `World::settle_over_hand_allowance`,
`prop::AI_NEVER` (both modules), `ViewExtra::ai_playable`, the log part;
**extended** — `HostRequest::Draw`, `ruleset_draw` arity,
`draw_cards_with_hooks`, the dead `draw_cards` parameter, `MatchPlayer`, the
seat-view JSON; **none** — no new manifest field, no new op/export name, no new
prompt.

### 5.1 Guest flag

`crates/card-sdk/src/abi.rs`, beside `pay_flag` / `invoke`:

```rust
/// `ruleset_draw` flags.
pub mod draw_flag {
    /// Bit 0: this draw is exempt from the immediate hand-limit discard-down
    /// (「可超过上限」). The limit itself is untouched; the hand may stay over
    /// until it fits again or the turn ends.
    pub const OVER_HAND_LIMIT: i32 = 1 << 0;
}
```

`crates/card-sdk/src/ctx/sys.rs` (`link_name = "ruleset_draw"`):

```rust
pub fn draw(player_id: i32, n: i32, flags: i32) -> i32;
```

`crates/card-sdk/src/ctx.rs` — one loop, two spellings:

```rust
/// Draw `n` cards, trimming to the hand limit immediately (unchanged).
pub fn draw(player_id: i32, n: i32) -> Result<i32, Prompt> {
    draw_flags(player_id, n, 0)
}

/// Draw `n` cards **without** the immediate hand-limit trim
/// (「可超过上限」). The excess is allowed until the hand fits again or the
/// turn ends; the card's own scheduled discard is the usual disposer.
pub fn draw_over_hand_limit(player_id: i32, n: i32) -> Result<i32, Prompt> {
    draw_flags(player_id, n, crate::abi::draw_flag::OVER_HAND_LIMIT)
}

fn draw_flags(player_id: i32, n: i32, flags: i32) -> Result<i32, Prompt> { /* current body */ }
```

The Kani arm inside `draw_flags` passes the same `flags` to every
`sys::draw(player, 1, flags)`. Sugar only: no new export name, no new `Prompt`
shape, no second loop.

**Wire-breaking:** `ruleset_draw` moves from `(i32, i32) -> i32` to
`(i32, i32, i32) -> i32`. `ABI_VERSION` 117 → 118. A pre-bump module is
rejected by the load-time version check.

### 5.2 The allowed excess (`crates/game-core/src/state.rs`, `engine/ops.rs`)

`MatchPlayer` gains:

```rust
/// The current over-limit hand was created by a draw that permits it
/// (「可超过上限」). While set, the excess does not block `end`; it is cleared
/// as soon as the hand fits again and after the turn-end sweep. `[serde(default)]`
/// so a pre-v19 save restores with it unset. Wire: `overHandAllowed`.
#[serde(default)]
pub over_hand_allowed: bool,
```

and the manual `Default` impl gets `over_hand_allowed: false`.

```rust
impl MatchPlayer {
    /// Over the limit **and** not permitted to stay there.
    pub fn over_hand_blocking(&self, held: usize) -> bool {
        self.over_hand(held) && !self.over_hand_allowed
    }
}
```

`PlayCtx::over_hand_blocking(&self, i)` wraps it like `over_hand` does
(`engine/play.rs:5455`).

`World` gains one maintenance helper. Every hand-shrink site calls it:

```rust
/// Clear the permitted-over-limit state once the hand fits again.
pub fn settle_over_hand_allowance(&mut self, i: usize) {
    let fits = self.hidden.get(i).is_some_and(|h| !self.st.players[i].over_hand(h.hand.len()));
    if fits { self.st.players[i].over_hand_allowed = false; }
}
```

Call sites (each removes exactly one card from a hand): `engine/play.rs:5588`
(`discard`), `:5690` (`play_from_hand`), `:5173` (`hand.clear` on bankruptcy),
`engine/ops.rs:1226` (`to_discard`), `game-rules/src/wasm_rules.rs:2238`
(`discard_from_hand`), `:1198` (`take_card` with `CardPile::Hand`), `:2257`
(`shuffle_into_deck` with `hand = true`), `:7392` (counteraction declaration
leaving the hand). `take_card` holds a `&mut` to the pile list while it
removes, so its settle goes after that borrow (`draw` refills inside the same
borrow already). `discard` (and `to_discard`) are the hot path and settle per
removed card; it is a two-field read until the hand fits.

Setting happens only in the two draw routines (§5.4, §5.6): an exempt draw that
leaves the hand over sets it.

### 5.3 Host request (`crates/game-rules/src/host.rs`)

```rust
Draw {
    player_id: i32,
    n: i32,
    /// Skip the immediate hand-limit discard-down and allow the excess to
    /// reach turn end (「可超过上限」). The limit state is untouched.
    over_hand_limit: bool,
},
```

`HostRequest` is not serialized (the guest body re-derives it on replay), so
this is not a record/save shape. Every `matches!(req, HostRequest::Draw { .. })`
keeps working; three sites need the new field or an explicit `..`:
`wasm_rules.rs:5199` (destructure), `hostfns.rs:1094/1097` (construction), and
`rules/kani/tests/conf_model.rs:1348` (destructure).

### 5.4 Host import (`hostfns.rs`, `host.rs`, `native_shims.rs`)

```rust
pub fn draw<C: HostCtx>(c: &mut C, player_id: i32, n: i32, flags: i32) -> Result<i32, HostError>
```

* `over_hand_limit = flags & abi::draw_flag::OVER_HAND_LIMIT != 0`;
  `flags` is validated to be `0` or the known bit (other bits ignored).
* Carried on `HostRequest::Draw` (§5.3).
* `host.rs:4659` linker: `func_wrap("draw", |c, player_id, n, flags| ...)`.
* `native_shims.rs`: `op_draw` already takes `[i32; 6]`; pass `a[2]` through
  (`:1060`) and give `ruleset_draw` three parameters (`:2776`).
* `apply_reply` (standalone fixtures / a positive inline reply) calls
  `w().draw_over(..)` when over, else `w().draw(..)`.

The arm must keep the `drawOut` window first (`wasm_rules.rs:5209-5230`, raised
only for out-of-turn draws); a cancel returns 0 before any flag is used, and
the draw itself is then `cx.draw_cards_with_hooks(player, n, true, over)` with
the same `-got - 1` reply.

### 5.5 `CardWorld` (`crates/game-rules/src/world.rs`)

`fn draw(&mut self, player_id: i32, n: i32) -> i32;` stays. One defaulted method
is added so no existing impl changes:

```rust
/// Draw without the immediate hand-limit trim. Default: same as `draw`.
fn draw_over(&mut self, player_id: i32, n: i32) -> i32 { self.draw(player_id, n) }
```

`impl CardWorld for Run` (`wasm_rules.rs:1112`) implements both through one
private `draw_impl(player, n, over)` and passes `over` to `world.draw_cards`;
on this path the flag sets the allowance and swaps the log part (the trim lives
in the match routine). Note the current call passes the dead parameter as
`true` (`:1130`): under the new meaning that literal becomes `false` for
`draw`, and only `draw_over` passes `true`. `NullWorld` (`host.rs:6349`) keeps
the default.

### 5.6 Engine (`engine/play.rs`, `engine/ops.rs`)

```rust
pub fn draw_cards_with_hooks(
    &mut self, i: usize, n: usize, hooks: bool, over_hand_limit: bool,
) -> Flow<i32>
```

The log's over part becomes:

```
over_hand_limit && over_hand  -> log.part.over_hand_allowed (limit)
otherwise over_hand           -> log.part.over_hand (limit, unchanged)
```

The new part key goes in both locale files directly after `log.part.over_hand`
(they are alphabetical, `en/game.json:396`, `zh-CN/game.json:396`): zh
「（可超过上限 {{limit}} 张，回合结束时弃到五张）」, en " (may exceed the limit
of {{limit}} -- discard down at turn end)". The existing part still names the
discard duty, which is right for an ordinary overflow.

The trailing trim becomes:

```rust
if over_hand_limit && got > 0 && self.over_hand(i) {
    self.w.st.players[i].over_hand_allowed = true;
}
if !over_hand_limit && self.playing() {
    while self.over_hand(i) { let k = ...; self.discard(i, &card)?; }
}
```

The `playing()` guard stays, so draws outside play (deck refills, fixtures)
still never trim. `draw` (`:5464`) passes `false`, so opening hands and engine
draws keep their exact current behaviour. `draw_cards_with_hooks` callers:
`draw` `:5465`, the `HostRequest::Draw` arm `wasm_rules.rs:5228`, and the three
`deck_refill` fixtures (`:99`, `:112`, `:130`, which pass `false` for the new
argument).

`ops::draw_cards` (`:1168`) stops `let _ = over_hand_limit` (`:1202`): it picks
`log.part.over_hand_allowed` instead of `log.part.over_hand` when the flag is
set, and sets the same allowance if it left the hand over. That routine has no
trim loop at all (its callers are the standalone/fixture `Run::draw` path), so
it never trims either before or after; the flag only changes the log and the
allowance there.

`Run::over_hand` (`:5455`) stays raw; the trim loop uses raw; the gates use
`over_hand_blocking` (§6.2).

**Turn end** (`end_turn`, `:896`, after the out/playing guard following the
`turnEndBefore` raise at `:925`): run the ordinary trim loop to the cap
(covers a callback that drew, and a cap lowered below five), then
`over_hand_allowed = false`. The sweep runs on every end, so a seat that is
over the cap when its turn ends reaches its next turn at the cap whatever the
scheduled body did; no turn-start clear is needed (and a clear at turn start
would wrongly drop an allowance set by an out-of-turn draw before that seat's
own turn-end sweep).

### 5.7 Card declaration — `prop::AI_NEVER`

`crates/card-sdk/src/abi.rs` (near `prop::EST_COST`, `:1601`) and
`crates/game-core/src/state.rs` (`:444`), kept in step:

```rust
/// Bot-only: nonzero = standard / advanced bot policies must not offer this
/// card. Absent or `0` = offer. A static card fact, never legality -- a human
/// can still play the card. Read by `CardRules::ai_play`; rides the existing
/// `ManifestEntry.props`.
pub const AI_NEVER: &str = "aiNever";
```

A prop, not a `CardDef` field, because props are the established static-fact
surface, already exported in `ManifestEntry.props` (`abi.rs:3367-3377`) and
stored in `CardInfo.props` (`host.rs:100-109`); `0`/absent preserves every
existing card with no migration; and no struct field means no threading through
`CardDef::new` and its seven other `const` builders and no manifest schema
change.

Card-side spelling:

```rust
use card_sdk::abi::prop;
pub const SUMMER_CAMP: CardDef = CardDef::new("Mor:夏日合宿", &[...])
    .props(&[(prop::AI_NEVER, 1)])
```

(`props` replaces, so a card that already declares props adds the pair to the
same slice.)

### 5.8 Host policy (`crates/game-rules/src/wasm_rules.rs`)

`RulesBridge<M>` overrides the existing trait method next to `card_prop`
(`:11356`):

```rust
/// Would a bot offer it? Static card fact: `prop::AI_NEVER`.
fn ai_play(&self, _cx: &PlayCtx, _player: usize, card: &str) -> bool {
    self.card_prop(card, game_core::state::prop::AI_NEVER) == 0
}
```

`Rules::ai_play`'s doc gains: standard / advanced consult it; chaos ignores it
by design (`policy.rs:346-357`). `examples/bot_cost.rs` already forward-delegates
and needs no change.

### 5.9 Seat-view extra (`engine/mod.rs`)

```rust
pub struct ViewExtra {
    pub ai_answer: Option<RawAiAnswer>,
    /// Would `cant_play` allow each card (human legality / glow)?
    pub playable: Vec<bool>,
    /// Would a bot offer it: the bare static `ai_play` answer.
    /// Combine with `playable`; never legality. Static facts only, so the
    /// seat-view engine computes it exactly.
    pub ai_playable: Vec<bool>,
    pub estimated_cost: Vec<i32>,
    pub skills: Vec<SkillAction>,
}
```

`view_extra_parts` (`:1412-1426`) fills it in the same hand loop: the memo
becomes `HashMap<&str, (bool, bool)>` holding `(playable, ai_playable)` so
neither is computed twice for a duplicate id -- `ai_playable` from
`self.rules.ai_play(&cx, i, c)`, a static prop read. `view_extra_inner`
(`:1387-1392`) emits `"aiPlayable": p.ai_playable` beside `"playable"`.
`playable` stays the glow source; `aiPlayable` is the offer half.

### 5.10 Client and bot plumbing (all additive, no game rules)

* `bot-core/src/view.rs`: `SeatView.ai_playable: Vec<bool>` (the struct is
  already `#[serde(default, rename_all = "camelCase")]`, so an old frame reads
  empty / `[]`, which means "offer"), added to the `Default` impl beside
  `playable`, filled in `from_match` from the extra. The offer list:

  ```rust
  /// Per card id: `cant_play` allows it **and** a bot would offer it.
  /// A missing `ai_playable` entry (old frame) counts as `true`.
  pub fn offerable(&self) -> Vec<bool> {
      self.playable.iter().enumerate()
          .map(|(k, &p)| p && self.ai_playable.get(k).copied().unwrap_or(true))
          .collect()
  }
  ```

  The predicted frame keeps `ai_playable`: `predict_upcoming_view` starts from
  `view.clone()` and only replaces `playable` and `ai_answer`
  (`view.rs:251-296`), and `ai_playable` is a static card fact that must not be
  reset with `playable`.
* `bot-core/src/extras.rs`: `ViewExtras.ai_playable` beside `playable`,
  `to_json()` emits `"aiPlayable"`, and `view_extras` copies it from the fork
  (`:87`, `ai_playable: extra.ai_playable`).
* `bot-glue/src/lib.rs` (`:591`, `:760`) and `bot-service/src/lib.rs` (`:431`,
  `:569`): pass `&view.offerable()` / `&target.offerable()` to
  `legal_actions_with_cost` instead of `&view.playable`.
  `bot-core/src/sim.rs` (`:207`) holds a `ViewExtras`, not a `SeatView`, so it
  builds the same vector locally (`extra.playable` zipped with
  `extra.ai_playable`, `&&`) and passes that. The cost argument and the
  function's contract are unchanged.
* `rules-worker/src/lib.rs` (`:382` beside `playable`; comment `:344`):
  `needExtras` copies `"aiPlayable"`.
* `web-glue/src/lib.rs::frame` (`:776`): emit `"aiPlayable": extra.ai_playable`
  beside `"playable"`; the null branch (`:1102`) gains it too. Without this the
  browser autopilot never sees the flag in solo.
* `webui/src/core/types.ts:291` beside `playable`: optional
  `aiPlayable?: boolean[]`. `webui/src/game/botPool.ts:31`: the `ViewExtras`
  field. `webui/src/game/viewExtras.ts`: copy it in `mergeExtras` (`:36`) and
  compare it in the same-frame equality check (`:95-100`); `aiPlayable` is
  decision-specific and never carried from another key.
* `webui/src/game/autopilot.ts::affordableCards` (`:709-723`): standard
  (object `params`) skips when `playable` **or** `aiPlayable` says no; chaos
  (bare `CHAOS_RESERVE`) keeps the `playable`-only filter.
* `webui/src/core/hand.ts`: `overHand` becomes the blocking form
  (`held > limit && !me.overHandAllowed`); the new `overAny` is the raw test
  for the discard affordance. `webui/src/core/types.ts`: optional
  `MatchPlayer.overHandAllowed?: boolean` (the field serializes with the
  player state; absent reads `false`, i.e. today's gate).
  `webui/src/scenes/board/model.ts:55` sets `overHand:` from `overHand(...)`
  and a new `overAny:` from `overAny(...)`; `Side.tsx:392` switches to
  `overAny` for the discard prompt, and the `turnFlow.ts:35` / `Side.tsx:371`
  gates keep reading `overHand`.
* `docs/BOT.md` §1.1: a new subsection after E (`estCost`), in the same
  one-row shape: `rules.ai_play(&cx, i, card)` reads the **static**
  `CardDef::props` map (`abi::prop::AI_NEVER`, default `0`) and never the world,
  so it is always equal and its only blast radius is "offered when it should
  not be". It is an offer gate, not legality, so a divergence could never
  grey a card.
* `docs/SERVER.md`'s takeover **Limitations** paragraph (`:140`): the takeover
  reads `aiPlayable` and so skips a card a standard bot would skip; a chaos
  takeover does not, matching the engine.

### 5.11 No change

`cant_play`, `playable`, the play command, and every `Msg` key other than the
new log part. A human can always play an `aiNever` card.

---

## 6. Engine wiring

### 6.1 Offer filtering (item 2)

| caller | before | after |
|---|---|---|
| standard engine bot (`policy.rs:370`) | `rules.ai_play` always true | reads `AI_NEVER`; 夏日合宿 filtered |
| chaos engine bot (`policy.rs:346-357`) | ignores `ai_play` | unchanged, documented |
| advanced bot service (`legal_actions*`) | `view.playable` | `offerable()` |
| browser 托管 (`autopilot.ts`) | `view.playable` | `playable && aiPlayable` (standard); `playable` (chaos) |
| human manual play / glow | `playable` | unchanged |

`cant_play` is not consulted differently, so the server still accepts a manual
夏日合宿 play on any seat.

### 6.2 Hand-limit gates (item 1)

The read point does not move. `hand_limit()` stays the single source
(`state.rs:827`); `HAND_LIMIT` / `NO_HAND_LIMIT` / `HAND_LIMIT_DELTA` stay.

```
ctx::draw_over_hand_limit ──> ruleset_draw(p,n,1) ──> HostRequest::Draw{over:true}
  ──> drawOut window (can cancel) ──> draw_cards_with_hooks(..., over=true)
  ──> cards move, hooks run, allowance set
  ──> if !over { discard-down }        ──> hand may stay > cap
...
end: over_hand_blocking? no -> accept
turnEndBefore: scheduled discard to five -> sweep to the cap -> allowance cleared
```

Specific gate edits:

* `cant_act` `"end"` (`mod.rs:3014`): `over_hand_blocking`.
* `can_end_here` preview (`mod.rs:923-937`): `over_hand_blocking`.
* `discard` gate (`mod.rs:2987`): raw `over_hand` (discarding is legal while
  allowed).
* trim loops: raw `over_hand`.

### 6.3 Kani model

* `ctx/sys_kani.rs::draw(player_id, n, flags)` passes `flags` as slot 2.
* `rules/kani/src/host.rs::arm_draw` reads `a[2]`; bit 0 pushes the new
  `Effect::DrawOver { player, n }`, otherwise `Effect::Draw { player, n }`.
* `rules/kani/src/world.rs` gains `DrawOver { player, n }` beside `Draw`. A
  variant, not an `over: bool` field: most existing spec patterns destructure
  `Effect::Draw { player: p, n }` with no `..`, so a new field would break
  every one of them, while a new variant leaves them all matching the plain
  draws they describe. Matches that could see a flagged draw already carry a
  wildcard (`pp.rs:385`, `skills_ppp.rs:389` return/count "other"), so nothing
  silently undercounts.
* No new `kani_host::Op`; `ruleset_draw` stays three slots on all backends.
* The model has no hand limit, so only flag propagation is asserted here; the
  trim/allowance behaviour is covered by the engine tests and the spec clause.

---

## 7. Card usage

### 7.1 `rules/cards/card-morfonica/src/nanami_effort.rs` (item 1)

**Before** (`:220-237`): read `handLimit`, set `NO_HAND_LIMIT`, `ctx::draw`,
restore; the `TODO(ABI)` at `:224-228` explains the failure.

**After**:

```rust
// 规则书（1）: 「抽x张卡（可超过上限）」 -- this draw is exempt from the
// immediate trim; 「回合结束后将手牌弃置到五张」 below is the scheduled
// dispose that gets back under it.
ctx::draw_over_hand_limit(player_id, x)?;
ctx::before_turn_end(player_id, "discard_down_to_five");
```

The state read/write pair drops. `discard_down_to_five` is unchanged.
`effect_draw` gains a Kani-only export `effect_draw_kani`; the full `play` path
cannot reach branch (1) under Kani (`arm_tok_names` returns an empty list), so
the proof covers it directly.

### 7.2 `rules/cards/card-morfonica/src/summer_camp.rs` (item 2)

```rust
use card_sdk::abi::prop;
// Human legality is untouched -- `aiNever` is not a `cant_play` gate: standard
// and advanced bots skip it, a human still plays it by hand.
pub const SUMMER_CAMP: CardDef = CardDef::new(
    "Mor:夏日合宿",
    &[
        On::Play("", None, summer_camp),
        On::Filter(Filter::Untargetable(...)),
        On::Action(Action::TurnStart(...)),
    ],
).props(&[(prop::AI_NEVER, 1)]).meta(&card_sdk::meta::CardMeta::new()
    ...
));
```

The TODO(ABI) ai-play note at `:39` is replaced; the leave-field TODO at `:25`
stays (a different item). Bodies and `*_kani` exports unchanged.

---

## 8. Save / record / replay

* **SAVE_VERSION 16 → 17.** `MatchPlayer.over_hand_allowed` is `#[serde(default)]`
  and `MatchPlayer` is `#[serde(default)]`, so a v16 save restores with it
  unset; a save taken over the limit must carry it because the end gate reads
  it. The guest body re-issues the flag on replay, but a save is a prompt
  boundary and the host answer is negative (already applied), so the allowance
  is not re-derived -- it is durable.
* **RECORD_VERSION unchanged.** The input stream does not move: the draw reply
  is still the negative already-applied count, and bot picks are ordinary
  `act:"play"` answers. A record does not carry the flag.
* **Replays.** Records ride archived engine bundles; an old record replays the
  old body (immediate trim) against its old bundle, a new one the new bundle.
  No migration.
* **Ruleset identity.** The new prop changes the ruleset hash, which the record
  stamp already carries; archived bundles keep their own manifest, and their
  `CardInfo.props` parses as empty (`#[serde(default)]`, `host.rs:108`), so
  `aiNever` reads `0` (offer). No `ManifestEntry` shape change.
* **View frames.** `aiPlayable` / `overHandAllowed` are derived data in the
  frame, not inputs. `overHandAllowed` needs no frame work: it is a
  `serde`-serialized `MatchPlayer` field and ships with the state block for
  free (the public hand count already ships there, and the flag is not
  information the hand size does not imply). A client that does not know
  `aiPlayable` falls back to today's behaviour; `overHandAllowed` absent reads
  `false`, which is exactly the old end gate.

---

## 9. Versioning and band arithmetic

The README row is `G14 | 113→114 | 16→17 | — | —`. G14 lands **before** G09–G12
(they still hold 114–118 / 17–19); this document takes:

* `ABI_VERSION` **113 → 114** — the `ruleset_draw` arity only. The prop and the
  extra are data on existing shapes and need no bump.
* `SAVE_VERSION` **16 → 17** — the allowance bit (the README row reads
  `16→17`).
* engine kinds: **none**; Kani `Op`: **none**.
* `RECORD_VERSION` unchanged.

ABI entry inserted immediately **below** the `pub const ABI_VERSION` line
(newest-first, `abi.rs:783`) in the `/// vNNN:` style:

```
/// v114: **an over-limit draw and a card's static bot-avoid flag**.
///    `ruleset_draw` gains a `flags` word: [`draw_flag::OVER_HAND_LIMIT`] (1)
///    makes the draw skip the immediate hand-limit discard-down while still
///    moving the cards; the excess is *allowed* (`MatchPlayer.over_hand_allowed`,
///    wire `overHandAllowed`) until the hand fits again or the turn ends, so
///    `end` is accepted and the card's scheduled dispose runs; the over-limit
///    warning part is replaced by `log.part.over_hand_allowed` for that draw.
///    `ctx::draw` passes 0, the sugar `ctx::draw_over_hand_limit` passes the
///    bit, and the limit state itself is untouched. **SAVE_VERSION 16 -> 17**
///    (the allowance bit, `serde(default)`). RECORD_VERSION unchanged.
///    `ManifestEntry` is unchanged: the prop `aiNever` (nonzero = standard /
///    advanced policies do not offer the card; absent/0 = offer; never
///    legality) rides the existing `props` map and the existing host
///    `CardRules::ai_play` seam; `ViewExtra.ai_playable` (wire `aiPlayable`)
///    lets the client-side seat-view engine read the same static fact with no
///    hidden read.
```

---

## 10. Tests and Kani obligations

### 10.1 Item 1 — `rules/tests/tests/g14_ai_and_hand_limit.rs`

| test | assertion |
|---|---|
| `g14_nnm_draw_over_cap_is_not_trimmed` | hand of five incl. NNM, two marks, branch (1), x=2: the hand is six and nothing was discarded (five, not six, because CRYCHIC's group skill closes the play gate at `hand >= 6`) |
| `g14_over_hand_allowed_lets_the_turn_end` | after the draw `over_hand` is true, `over_hand_allowed` is true, `act:"end"` is **accepted**, and `can_end_here` is true |
| `g14_nnm_end_of_turn_discards_to_five` | continuing to the end leaves five via the scheduled body; the allowance is false afterwards |
| `g14_turn_end_sweeps_a_lowered_cap` | with the cap at four, turn end leaves four |
| `g14_discard_clears_the_allowance` | a manual discard to the cap clears it; a later `add_to_hand` then blocks `end` again |
| `g14_plain_draw_still_trims` | a normal draw on the same table still discards down immediately and leaves the allowance false |
| `g14_draw_over_hand_limit_reaches_the_host` | the observed `HostRequest::Draw` carries `over_hand_limit: true`; a `ctx::draw` card's request carries `false` (inline-provider pattern) |
| `g14_effect_draw_schedules_the_discard` | the `before_turn_end("discard_down_to_five")` callback exists after the draw |
| `g14_allowance_survives_save_restore` | save after the draw, restore, the end gate and the scheduled discard still behave |
| `g14_over_limit_log_part` | the exempt draw logs `log.part.over_hand_allowed`; a normal overflow logs `log.part.over_hand` |
| `g14_human_can_still_play_summer_camp` | a human still plays 夏日合宿, `playable` stays true and the manual `act:"play"` is accepted (item 2 control) |

### 10.2 Formal hand-limit clause

`rules/spec/src/global/cards.rs::g_cards_01` gets one guard after the
`NO_HAND_LIMIT` skip (`:220-222`):

```rust
// 「可超过上限」 (card clause): the excess is permitted until the turn-end
// dispose. The turn-end sweep clears the flag, so the next act is checked
// normally again.
if p.over_hand_allowed {
    continue;
}
```

Its doc's **Condition** gains: "unless the player carries the permitted excess
(`over_hand_allowed`), which only an exempt draw sets and the turn-end sweep
clears". The **Rulings & OPEN** note records that the base 「立刻」 and a card's
「可超过上限」 coexist via that flag. New test
`g_cards_01_allows_a_card_permitted_draw` (in
`rules/tests/tests/g14_ai_and_hand_limit.rs`, beside the G14 group): play NNM
branch (1) under
`observe(CLAUSES)` and assert no violation, plus the existing plain-overflow
case (`:405`, `:442`) still finding an unallowed overflow.

### 10.3 Kani

`rules/kani/src/spec/g14_ai_and_hand_limit.rs` and
`rules/kani/src/proofs/g14_ai_and_hand_limit.rs` (group files like
`g07_targeting`, registered `#[cfg(feature = "band-morfonica")]` in both
`mod.rs`s):

* `nnm_02_draw_over_x1_iff`, `nnm_02_draw_over_x2_iff`,
  `nnm_02_draw_over_user1_iff` — `effect_draw_kani`: exactly `x`
  `Effect::DrawOver { player, n: 1 }` and nothing else (one import per card).
  `unwrap_done` shows the body returned; frame: hand +x, dest and money
  unchanged. **The import count is concrete per harness.** A symbolic `x` over
  the same body (assumed `1..=4`) pushed each CBMC run past the 900 s budget;
  `x = 1` and `x = 2` bracket the single / repeated import shape, and
  `user = 1` covers a second seat. Unwind drops 32 → 24 with it (24 clears the
  model's `MAX_EXTRA` 16 name scan).
* `nnm_02_draw_over_mutant_plain` (`x = 2`) — the same shape over
  `Effect::Draw` fails.
* `plain_draw_stays_plain` — a plain-draw card still logs `Effect::Draw`. Its
  subset declares the whole 10次招募 body: `pay`, `pay_command` (the
  `PayCommand::send` façade), `state_get` / `state_set` (the 「1回限定」 latch)
  and `draw`.
* `summer_ai_never_declared` / `summer_ai_never_mutant` — `SUMMER_CAMP.props`
  contains `(AI_NEVER, 1)`; a copy without it fails.
* No new `Op`; `arm_draw` must not hit the unmodeled-op guard for flags `0`
  and `1`.

The full NNM `play` body stays BLOCKED-HOST for the Kani harness
(`arm_tok_names` empty, KANI-COVERAGE C-MOR-nnm-02), so the proof drives
`effect_draw`. The second half of the clause already has a proof:
`nnm_discard_dispatch` (`proofs/mor.rs:1377`) drives `discard_down_to_five`
and is unchanged by this group.

### 10.4 Item 2 — ruleset and client

| test | assertion |
|---|---|
| `g14_summer_declares_ai_never` | `rules.card_prop("Mor:夏日合宿", prop::AI_NEVER) == 1` |
| `g14_standard_bot_skips_summer_camp` | `Table::vanilla(2)`, `make_bot(0, Standard)`, 夏日合宿 the only hand card, `begin_turn(0)`, `tick_until` the turn leaves 0 (`pump_bots` drives the seat): the bot rolled and ended, and the card is still in hand |
| `g14_standard_bot_plays_a_normal_card` | contrast: a control card without the prop in the same fixture is played (missing from hand) |
| `g14_chaos_still_plays_summer_camp` | `make_bot(0, Chaos)`: the card leaves the hand (open question 1; invert if the ruling changes) |
| `g14_human_can_still_play_summer_camp` | `playable` true and the manual `act:"play"` accepted |
| `view_extra_ai_playable_is_false_regardless_of_legality` (`crates/game-core/tests/view_extra.rs`) | `TestRules` gains an `ai_never: Vec<String>` and an `ai_play` override (the trait default is `true`; `card_prop` alone does not reach it): `playable[i] == true`, `ai_playable[i] == false`, other cards true |
| `the standard policy skips a card flagged aiNever even when playable` / `chaos plays an aiNever card` (`webui/src/game/autopilot.test.ts`) | the command list honours / ignores `aiPlayable` |
| `view_extra_equiv` (`crates/bot-core/tests/view_extra_equiv.rs`) | `ai_playable` gets its own must-agree counters and divergence message like `playable`, and `strip_extras` clears it (`:120`) |

Existing suites that stay green: `summer_plays_to_field_until_next_turn_then_draws`,
`interaction_summer_shield_vs_foreign_targeting`, `nnm_discard_marks_draw_x`,
`nnm_hand_play_spends_marks_and_draws`, `crychic_band_skill_hand_limit_none`,
`g_cards_01_*`, `interaction_band_hand_limit_vs_draw`.

---

## 11. Alternatives

**A1 — a real `CardDef` field plus `ManifestEntry.ai_play`.** Most explicit and
future-proof for a tri-state, but threads the field through `new` and seven
`const` builders, appends a manifest field (postcard is not self-describing: a second
wire break) and adds a `CardInfo` field. Rejected for v114. It remains the
migration path if the fact ever stops being a bool.

**A2 — a wasm `OP_AI_PLAY` hook.** Truest to "hook" and lets a card inspect the
world; costs a guest run per candidate. The seam can be added later as a prop
`aiHook` naming an entry, with the static bool as the fast path.

**A3 — let guest state increases cross the routine boundary.** One condition in
`overlay_guest_state` (accept `wrote.value > base_v`) would make the existing
set/restore work with no new import. It changes every host routine's view of
pending state, including the explicitly open 心の雨 ruling (`docs/ENGINE.md:460`:
a body that just stunned a player then gains -- should its own stun gate the
gain?), so it belongs in its own batch. Even after it lands, `ctx::draw` still
needs to know *which* draws may exceed, so the flag does not become redundant.

**A4 — set `NO_HAND_LIMIT` around each card of the draw from the guest.** Still
fails on its own (the raise is a guest write and the boundary drops increases),
and even combined with A3 it re-reads/writes the limit per card.

**A5 — encode "over" in the sign of `n` (no ABI bump).** Avoids the arity
break, but negative counts are a natural future "draw up to" shape and a flags
word is the house pattern (`pay_flag`). Rejected.

**A6 — make `playable` bot-filtered and add a separate legality list.** Avoids
a view field but renames an established contract used for glow. Rejected.

**A7 — re-add trimmed cards from the discard in the card body after
`ctx::draw`.** No ABI change, but the cards crossed the discard
(`discarded` / `discardAfter` fire), the order with multiple copies is unstable,
and the draw hooks saw the wrong hand size. Rejected.

**A8 — have the player or the bot simply discard the excess before ending,
keeping the end gate untouched.** Rejected: the card's printed timing is
「回合结束后」; manual discarding makes the scheduled body dead and lets a
timer-expired seat start its next turn over the cap with no allowance.

**A9 — allow ending over the limit whenever any turn-end callback is
scheduled.** Too broad: many cards schedule callbacks that do not dispose a
hand; the seat could end over the limit forever.

**A10 — put the allowance in `TurnCtx` instead of `MatchPlayer`.** It is a
per-seat permission, not a turn plan, and several seats can be over the limit
at once (an out-of-turn draw). Rejected.

**A11 — a strategy-book entry with `play_weight_milli: 0` for 夏日合宿.**
Rejected: the book is an offline, persona-keyed tune (`data/strategy_book.json`,
excluded from `data_sha256`); the never-play is a card fact and should ship
with the card, including for new personas and opponents.

**A12 — a `zero_weight` field on `CardPlayParams` read from the card's props
by the policy.** This only moves the prop lookup into the strategy layer and
still needs the prop; it also makes the standard policy depend on a book the
chaos/advanced paths do not share consistently. Rejected.

**A13 — a second import `ruleset_draw_over` instead of a flag word.** Additive,
so no arity break and no `ABI_VERSION` step (`ruleset_draw_event` is the
precedent for a second import). Rejected: it duplicates the whole call chain on
all three cfg paths (linker entry, `native_shims` extern, Kani `Op` arm — an
`Op` id this group does not have), and every later flag would need a third
import. `pay_flag` already establishes flags as the way an existing call grows,
and the arity break costs one bump today and nothing later.

---

## 12. Open questions

1. **Chaos.** Chaos deliberately ignores `ai_play`. Keep `aiNever` a
   standard/advanced fact (design default), or make it absolute so chaos also
   skips 夏日合宿? Engine and client must agree either way.
2. If a future card wants "skip only when X", does that become a CEL-valued
   prop, a second prop, or A2's hook?
3. **Advanced search.** Exclude an `aiNever` card from the action set (design
   default, cheaper and matches "never offer") or leave it in the tree with a
   lower prior?
4. **Log wording.** Design default (§5.6): the draw may exceed the limit and
   the dispose is at turn end (zh 「（可超过上限 {{limit}} 张，回合结束时弃到五张）」,
   en " (may exceed the limit of {{limit}} -- discard down at turn end)").
5. **Cap below five.** `discard_down_to_five` disposes to five per the card,
   then the turn-end sweep trims to a lower cap. Confirm that reading; the
   alternative (the scheduled body's own `- 5` becomes `- limit`) changes the
   card's printed number.
6. **Scope of the exemption.** Only cards whose text says 「可超过上限」 call
   `draw_over_hand_limit`; the base rule 「任何时刻…立刻弃至上限」
   (`rulebook-doc.md:126`) stays the default. Is any other existing draw text an
   implicit exemption?
7. **Skills.** `ai_play` governs hand-card choice only. Should a skill press be
   able to declare `aiNever` (the `ai_skill_choice` path has its own per-skill
   params)?
8. **General state crossing (A3).** Confirm the 心の雨 ruling before flipping
   the overlay; once increases cross, the allowance and the flag both stay
   (they answer *which* draws may exceed, not whether the world sees a raise).
9. **Bot discard while allowed.** Standard policy and the autopilot currently
   discard whenever raw `over_hand` is true, which reaches the same five but
   bypasses the card's timing. Keep that (design default: the outcome is
   identical) or teach them to leave it to turn end?
10. **`aiNever` and persona value.** 夏日合宿 is often worth playing when the
    seat is actually being targeted. A static never is the design default; a
    per-persona override (A11's book) could refine it later.

---

## 13. Necessity

### 13.1 The standard

A surface element is necessary only if all four hold:

1. **The book (or a reproducible defect) requires the fact.** Item 1 has a
   printed clause. Item 2 is a heuristic, so it must carry a reproducible,
   card-static defect instead. A "nice to have" with no trace is not enough.
2. **No existing surface can express it.** §13.4 lists every candidate and the
   engine reason each fails. Where a candidate *would* work, it is named and
   the cost of taking it instead is priced (§13.4, A3 and A13).
3. **The cheapest alternative fails concretely.** Each row names the observable
   that breaks, not a principle.
4. **The set is minimal.** Every field is read by a named gate, test or spec.
   Nothing is carried for a speculative card.

Necessity is argued per item and per element, and each has a stated **demotion
path** so a reviewer can strike it deliberately (§13.3) rather than let it
drift.

### 13.2 Book basis and the failing trace

**Item 1 — a printed clause, twice violated.** The sheet's clause (1) has two
halves and both are inert today:

* **「可超过上限」.** The draw trims immediately. Trace: a hand of six
  (`通用:GREAT`, `通用:[衍生]PERFECT`, `通用:登上武道馆`, `通用:雨啊，快点来吧`,
  `通用:CiRCLE THANKS PARTY!`, `Mor:（NNM）稍微努力了一下`), two character
  marks, branch (1), x = 2. `ctx::draw` moves to eight,
  `draw_cards_with_hooks` discards three at random in the same transition
  (`play.rs:5575-5583`), and the seat is back at five. The clause's
  parenthetical never happens. This is the probe in row 441's `verify_note`.
* **「回合结束后将手牌弃置到五张」.** Because the trim already happened, the
  scheduled `discard_down_to_five` (`nanami_effort.rs:242-259`) sees a hand of
  five, computes `extra = 0`, discards nothing and logs nothing. The clause's
  whole effect is dead output.

Fixing only the trim creates the second trace: the hand is six or seven at
turn end, `cant_act`'s `end` gate refuses the `end` (`mod.rs:3014-3017`,
`err.over_hand`), and the callback that hangs off the turn end can never run.
So the item is necessary **twice over** — the printed exemption and the printed
timing each fail on their own.

**Item 2 — no clause; a defect on the offer side.** The sheet prints nothing
about bots, so the bar is a reproducible defect in a shipped policy. Trace: a
standard bot seat holding 夏日合宿 at the first 运营 always plays it. The filter
at `policy.rs:339-375` consults legality, the `hold_for_counteract` flag, the
round window, `play_weight_milli` and the money floor — every value defaulted
neutral — and nothing in the sheet or the manifest says "hold this". The card's
payoff clause (「你没有因为此卡效果无效化任何影响则抽一张牌」) and its
「直到下个自己的回合开始前」 shield are spent on a turn chosen by accident of
draw. The same defect reaches the 托管 takeover through `playable`
(`autopilot.ts:709-723`), which is `cant_play` only.

The defect is **card-static**: nothing the sampled world says makes the first
运营 right, so a static declaration is exactly the right shape and can be proven
equal client and server (`docs/BOT.md` §1.1).

### 13.3 Doing nothing, and the demotion path

Doing nothing is not neutral for either item; both leave a knowingly wrong
result in a shipped path. If the reviewer rejects an item, the demotion must be
taken as its own decision:

| item | if the design is not taken | cost |
|---|---|---|
| 1 | implement "trim immediately" and tag the card `TODO(规则书)` | one printed clause ignored and the turn-end body dead; the discrepancy shows up in the black-box suite |
| 2 | leave the card in the bot's unconditional play set | every standard/bot/托管 seat keeps burning the card; no carrier is added for the next defensive card either |

Item 1 is graded medium and `hand-limit-lift` is a real clause; item 2 is graded
low and may legitimately be struck. They are separable (§13.6).

### 13.4 Candidate surfaces, and why each fails

| candidate | item | why it fails / what taking it would cost |
|---|---|---|
| `state::set(HAND_LIMIT, NO_HAND_LIMIT)` in the card | 1 | the guest raise does not cross the routine boundary (`cx.rs:1018-1023`). **A3 would make it work** — see the A3 row |
| A3 — let every guest state increase cross the overlay | 1 | works, and is the honest near-miss: it would revive the existing body with no new import. It changes what every host routine sees of *pending* state and carries the still-open 心の雨 ruling (`docs/ENGINE.md:460`), so it is its own batch. The flag does not become redundant under A3 (Q8) and is local to the call, so the flag is the narrow unit either way |
| a single `NO_HAND_LIMIT` raise with no flag (A3 only) | 1 | cannot express a body that mixes an exempt and a normal draw; re-reads and rewrites the limit per card; the limit is shared state, so a nested draw inherits the lift |
| `ctx::draw` with no flag and re-adding trimmed cards | 1 | the cards crossed the discard (`discarded` / `discardAfter` already fired), the order of multiple copies is unstable, and the draw hooks saw the wrong hand size (A7) |
| a per-card static "draws may exceed" prop | 1 | a body may mix draw kinds; the exemption is a property of the call (A3 row) |
| the `log.part.over_hand` presence as the gate | 1 | it is presentation data and is absent exactly when the hand is under the limit; the fix also suppresses it by design |
| a second import `ruleset_draw_over` (A13) | 1 | works and needs no version break, but duplicates the call chain on three cfg paths and consumes a Kani `Op`; every later flag would need another import |
| "the caller discards the excess before ending" | 1 | the printed timing is 「回合结束后」; the callback becomes dead and a timed-out seat must discard by hand |
| the scheduled callback's existence as the allowance (A9) | 1 | many cards schedule callbacks for unrelated reasons and never dispose a hand |
| `MatchPlayer.hand` (the public count) as the allowance | 1 | it is a mirror updated later than the private hand the gate reads |
| `cant_play` / a legality reason | 2 | a human must still play the card; an `aiNever` card is *legal*, so legality would grey it |
| a TypeScript table entry in `autopilot.ts` | 2 | the engine bot and the advanced service never see it |
| the strategy book's per-card weight (A11/A12) | 2 | offline, persona-keyed, not shipped with the card, excluded from `data_sha256`, and not shared consistently by the engine bot and the client |
| a guest `OP_AI_PLAY` hook (A2) | 2 | works, but runs a guest body per candidate; a static bool is the cheap correct form for a never |
| a `CardDef` field (A1) | 2 | same information, larger surface: `new` plus seven builders, a manifest field, a `CardInfo` field and a second wire break |

### 13.5 Element by element

Honest severity in brackets: **[required]**, **[belt]**, **[plumbing]**.

| element | removed → |
|---|---|
| `draw_flag::OVER_HAND_LIMIT` + the arity [required] | no way to say *which* draw is exempt; the trim cannot be selectively skipped |
| `ctx::draw_over_hand_limit` [plumbing] | every call site spells the bit by hand; the flag has no name in card code |
| `HostRequest::Draw.over_hand_limit` [required] | the host arm cannot carry the decision across the pause boundary; the flag dies at `hostfns` |
| `over_hand_allowed` [required] | the end gate refuses the very act that runs the dispose; the card deadlocks at its own turn end |
| `over_hand_blocking` [plumbing] | the four gate sites each open-code `over_hand && !allowed` |
| `settle_over_hand_allowance` [belt] | the flag can outlive the overflow: a later non-draw hand addition would let `end` through while over the limit. The turn-end sweep still fixes the *hand*; the belt keeps the flag honest at rest |
| the turn-end sweep [required] | a skipped callback, a callback that drew, or a cap lowered below five starts the next turn over the cap |
| the log part [required] | six cards over a limit of five reads as an error the player cannot explain |
| `prop::AI_NEVER` + the `ai_play` override [required for item 2] | every policy offers the card; the seam stays unanswered |
| `ViewExtra::ai_playable` [required for item 2] | the engine bot sees the fact and the 托管 seat and advanced service do not — the defect moves rather than disappears |

### 13.6 Grouping

Item 1 is independent of item 2; they share one ABI step only because both are
small and last. Striking item 2 leaves item 1 fully intact; striking item 1
leaves item 2 intact (the prop, the extra and the client plumbing do not touch
the draw).

### 13.7 Counterarguments

* *"The card could discard its own excess."* — it does not know which cards;
  the card discards **random hand cards**, the routine already owns the trim,
  and `drawn` / `handSize` hooks must observe each card actually in hand.
* *"`over_hand_allowed` is derivable from the hand length and the limit."* — no;
  an over-limit hand looks identical whether or not it is permitted. Only the
  draw that caused it knows, and that knowledge is exactly what must persist to
  the turn end.
* *"Just let the player discard before ending."* — see the A8/A9 rows: it
  changes the printed timing and kills the scheduled body.
* *"Put `aiNever` in the sheet."* — the sheet has no bot column, and a card
  fact should not require a data-file edit; the manifest already carries props
  (`CardInfo.props`) to the client.
* *"One card does not justify an ABI step."* — the step is shared with a
  clause-grade item; item 2 alone adds no op, no kind and no manifest field, and
  it answers a seam (`ai_play`) the engine already calls for every candidate.
* *"Item 2 is only a heuristic."* — true, graded low, and separable (§13.6); it
  is still a deterministic, card-static wrong offer on three shipped paths.

### 13.8 Residual risk a reviewer may still strike

* The **`can_end_here` preview** is not load-bearing (the server gate is); it
  is kept so the client's end affordance and the server's acceptance agree.
* **`settle_over_hand_allowance`** could be replaced by the sweep alone with the
  same end hand, at the price of a stale flag at rest (§13.5).
* **`aiNever`'s reach over chaos** is Q1; narrowing it costs only the chaos
  clause of the test set.
* The **`drawOut` interaction** is out of scope: a replacement is its own draw
  and inherits no flag, and no card combines a replacement with 「可超过上限」
  today (Q6 covers any newly discovered text).

### 13.9 Cost

One ABI step, one SAVE step, one dead-parameter revival, one bool on the saved
player, one prop and one view bit. No new op, no new engine kind, no new export
name, no new manifest field, no new prompt, and no new decision type. The
element that costs the most surface — the slot-3 argument — is retired only by
A13, which costs more (three duplicated externs and a Kani `Op` id) and is
priced in §13.4.

---

## 14. Clause match

### 14.1 `Mor:（NNM）稍微努力了一下` (1)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「抽x张卡（可超过上限）」 | `ctx::draw_over_hand_limit` → `draw_flag::OVER_HAND_LIMIT` → the trim skip (§5.1-§5.6) | `g14_nnm_draw_over_cap_is_not_trimmed`, `nnm_02_draw_over_x1_iff`, `nnm_02_draw_over_x2_iff` | matched |
| 2 | 「回合结束后将手牌弃置到五张」 | the existing `before_turn_end("discard_down_to_five")` schedule; the allowance keeps the turn end reachable (§6.2) | `g14_over_hand_allowed_lets_the_turn_end`, `g14_nnm_end_of_turn_discards_to_five` | matched (Q5 on a lowered cap) |
| 3 | the base rule 「任何时刻…立刻弃至上限」 (`rulebook-doc.md:126`) | the trim stays the default; the allowance is the named exception (§3) | `g14_plain_draw_still_trims`, `g_cards_01_allows_a_card_permitted_draw` | matched |

### 14.2 `Mor:夏日合宿`

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定」 | existing `On::Filter(Untargetable)` / `TurnStart`; unchanged | existing `interaction_summer_shield_vs_foreign_targeting` | matched, out of scope |
| 2 | 「当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌」 | existing `turn_start` body; unchanged | existing `summer_plays_to_field_until_next_turn_then_draws` | matched, out of scope |
| 3 | (no clause) the card's bot value | `prop::AI_NEVER` (§5.7-§5.8) | `g14_standard_bot_skips_summer_camp`, `g14_human_can_still_play_summer_camp` | matched, assumption (Q1/Q10) |
