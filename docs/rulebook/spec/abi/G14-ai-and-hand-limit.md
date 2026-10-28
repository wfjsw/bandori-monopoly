# ABI design G14 — CardDef AI hooks and hand limit

**Status: draft for user review. Design only — no source edits, no commit.**
Worktree `/home/monopoly/web/.claude/worktrees/card-rulebook-review`, branch
`worktree-card-rulebook-review`. Fits on top of ABI v104 (`ruleset_turn_snap`
carries money + board arrays).

**Group:** G14, family "CardDef AI hooks and hand limit".

**Ground truth:** `docs/rulebook/cards.json`; every fragment below was re-read
from that file on 2026-10-28 against the card id named beside it. The two
behaviours are different in kind: item 1 is book text, item 2 is C#-derived
behaviour that the sheet does not print.

**Closed slugs and rows:** both items are covered.

| # | slug | severity | class | rows |
|---|------|----------|-------|------|
| 1 | `hand-limit-lift` | medium | B | 441 |
| 2 | `aiplay-hook` | low | B | 1253 |

No item is demoted to A. `hand-limit-lift` names an existing but dead
parameter (`ops.rs:1024`), not a live mechanism: the guest draw request has no
flag and the routine that trims never sees one. `aiplay-hook` has a live
consultation point (`Rules::ai_play`, `game-core/src/engine/rules.rs:619`) but
no way for a card to answer it, and the client-side extra that the user's AI
reads (`playable`) is legality-only.

Terminology: the client-side AI ("user's AI") runs in the user's browser; a
server bot exists only for a bot seat. The same static card fact must reach both
paths, and neither is allowed to read hidden state.

---

## 2. Problem, per item

### 2.1 `hand-limit-lift` — Mor:（NNM）稍微努力了一下, row 441

Book (`cards.json#Mor:（NNM）稍微努力了一下`):

> 「（1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张」

The body (`rules/cards/card-morfonica/src/nanami_effort.rs:217-228`) raises the
guest's own state to 「no limit」, draws, then restores:

```rust
let cap = ctx::state::get(player_id, card_sdk::abi::state_key::HAND_LIMIT);
ctx::state::set(..., HAND_LIMIT, NO_HAND_LIMIT);
ctx::draw(player_id, x)?;
ctx::state::set(..., HAND_LIMIT, cap);
```

`ctx::draw` becomes one `ruleset_draw` import per card
(`card-sdk/src/ctx.rs:728-766`), the host parks a `HostRequest::Draw`
(`game-rules/src/hostfns.rs:846-871`), and the draw arm
(`game-rules/src/wasm_rules.rs:4347-4378`) calls
`cx.draw_cards_with_hooks(player, n, true)`. That routine ends with an
immediate discard-down:

```rust
// game-core/src/engine/play.rs:4661
while self.over_hand(i) { let k = ...; self.discard(i, &card)?; }
```

`over_hand` reads `MatchPlayer::hand_limit()` (`game-core/src/state.rs:792`),
which reads the **live** `handLimit` state only. The guest's `NO_HAND_LIMIT`
stays on the throw-away run copy: the routine-boundary overlay copies a guest
state key only when the guest moved it **down**
(`game-core/src/engine/cx.rs:887-892`, "Decrease only"). `i32::MAX` is an
increase, so the trim uses the old cap 5 and the scheduled
`discard_down_to_five` (`nanami_effort.rs:238-256`) finds nothing to do.

The supposedly-generic hook is already in the signature and unused:

```rust
// game-core/src/engine/ops.rs:990, 1024
pub fn draw_cards(&mut self, player_id: i32, n: i32, over_hand_limit: bool) -> i32 {
    ...
    let _ = over_hand_limit;
}
```

and the only card-driven caller hard-codes `true` (`wasm_rules.rs:1018`).
Expected: the draw exceeds the cap; the hand stays over-limit (the public
`overHand` derivation / end gate), and the scheduled body discards down to
five at turn end. The `handLimit` state itself is unchanged — 「可超过上限」
qualifies this draw, not the cap.

### 2.2 `aiplay-hook` — Mor:夏日合宿, row 1253

Card text (`cards.json#Mor:夏日合宿`):

> 「打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌」

The sheet says nothing about bots. The C# reference (`CardSummerCamp`,
`MatchHost.cs:4584-4643`) declares `AiPlay => false`; the card file records
that as prose only (`summer_camp.rs:44-45`, `TODO(ABI) ai-play`).

The engine seam exists and standard bots use it: `ai_card_choice` filters hand
cards through `self.rules.ai_play(self, i, &id)`
(`game-core/src/engine/ai.rs:329`) before the strategy filters. But
`Rules::ai_play` (`game-core/src/engine/rules.rs:618-621`) defaults `true` and
`RulesBridge` (the `WasmRules` type) never overrides it, so every card answers
"playable". Chaos deliberately ignores the hook (`ai.rs:292-293`), which is
correct for chaos but is not the standard path.

The human-side takeover has the same hole from the other direction: the browser
autopilot plays from `view.playable` (`webui/src/game/autopilot.ts:653-666`),
which is `cant_play` legality only (`game-core/src/engine/mod.rs:1296-1307`);
`docs/SERVER.md:144` names the gap. A standard bot and a 托管 human both offer
夏日合宿, which they should never play.

Expected: a card can declare "bots should not offer this"; standard / advanced
policies skip it, the declaration rides the manifest to the client-side AI, and
**human legality is untouched** (a human can still play it by hand).

---

### 2.3 Shared root cause

Both gaps are the same shape: a **static card fact** is known at ruleset-build
time, but the host routine / AI policy that needs it reads only ambient state
that the card cannot reach.

* Item 1: the card reaches state (`state::set`), but the draw routine's trim
  reads live state, and the guest overlay is deliberately one-way (decreases).
* Item 2: the engine has the policy seam (`ai_play`) but no carrier from the
  card definition.

Neither needs a new decision mechanism. Item 1 needs one bit on an existing
host request — the parameter already exists and is dead. Item 2 needs one
static declaration on the existing generic `props` map, which already rides the
manifest to the client.

---

## 3. Capability as an invariant

> **INV-G14-1 — an effect can declare one draw exempt from the immediate
> hand-limit trim.** The draw still moves cards, still raises the per-card
> before/after points, still logs the draw, and the ordinary over-hand path
> (manual `discard`, the end gate, a scheduled discard body) still reaches the
> limit afterwards. The client-derived `overHand` stays true; only the
> "over the limit" warning part is suppressed because the card permits it. The
> `handLimit` state and its `HAND_LIMIT_DELTA` field-prop ladder are not
> touched by the exemption.
>
> **INV-G14-2 — a card can statically declare that bot policies must not offer
> it.** The declaration is per card id, lives on the manifest, is readable by
> the client-side seat-view engine without a guest call, and never changes
> `cant_play` / `playable` legality.

Derived obligations:

* **Default preserved.** `ctx::draw` without the flag behaves exactly as today;
  a card with no `aiNever` prop is offered by bots exactly as today.
* **Exactness.** `draw_over_hand` sets exactly the flag bit; the host skips
  exactly the trim loop and the over-limit warning part; no other draw
  behaviour changes.
* **Public over-hand.** `overHand` remains true so the UI and the end gate
  agree with the new hand size; the `log.part.over_hand` warning part is
  suppressed on the exempt draw (the card says it is allowed) but every other
  over-hand path keeps it.
* **Static facts only.** `aiNever` must be readable from the manifest alone:
  no hidden zones, no prompt, no RNG. The client-side extras path may therefore
  carry it (BOT.md §1.1 audit: new row, "static card props, always equal").
* **All three targets.** The draw flag compiles for wasm32, native
  (`native-rules`) and Kani, with the Kani model asserting flag propagation.
* **No display text.** `aiNever` is an ASCII prop key; `draw_over_hand` has no
  locale key.
* **No player decision inside an import.** Both are engine facts; the existing
  `ctx::ai_agent_choice` (an import that asks the engine's heuristic) is prior
  art, not a template.

---

## 4. Surface

Everything is generic mechanism. Summary of kinds: **new** — `ctx::draw_over_hand`,
`import::DRAW_OVER_HAND`, `CardWorld::draw_over`, `prop::AI_NEVER`,
`ViewExtra.ai_playable`; **extended** — `HostRequest::Draw`,
`draw_cards_with_hooks`, `draw_cards`'s dead parameter, the seat-view JSON;
**none** — no new manifest field, no new op/export name, no new prompt. The
only wire-breaking change is the draw import arity; the AI half is data on an
existing wire field.

### 4.1 Guest context — draw flag (new wrapper, extended import)

`card-sdk/src/abi.rs` — a new `import` module next to `export` (the flags
word is host-facing, unlike the `OP_*` / `EXIT_*` words it would otherwise sit
among):

```rust
/// Host **import** argument flags.
pub mod import {
    /// `ruleset_draw` flags: bit 0 = this draw is exempt from the immediate
    /// hand-limit discard-down (「可超过上限」). The limit itself is untouched.
    pub const DRAW_OVER_HAND: i32 = 1;
}
```

`card-sdk/src/ctx/sys.rs` (wasm32 extern / native link name):

```rust
#[cfg_attr(any(not(target_arch = "wasm32"), feature = "native-rules"),
           link_name = "ruleset_draw")]
pub fn draw(player_id: i32, n: i32, flags: i32) -> i32;
```

`card-sdk/src/ctx.rs` — one implementation, two spellings:

```rust
/// Draw `n` cards, trimming to the hand limit immediately (unchanged).
pub fn draw(player_id: i32, n: i32) -> Result<i32, Prompt> {
    draw_flags(player_id, n, 0)
}

/// Draw `n` cards **without** the immediate hand-limit trim
/// (「可超过上限」). The hand stays over-limit until the card's own scheduled
/// discard or the player's discards reach the cap.
pub fn draw_over_hand(player_id: i32, n: i32) -> Result<i32, Prompt> {
    draw_flags(player_id, n, crate::abi::import::DRAW_OVER_HAND)
}

fn draw_flags(player_id: i32, n: i32, flags: i32) -> Result<i32, Prompt> { /* current body */ }
```

The Kani arm inside `draw_flags` passes the same `flags` to every
`sys::draw(player, 1, flags)`. Sugar only: no new export name, no new
`Prompt` shape, no second loop.

**Wire-breaking:** `ruleset_draw` moves from `(i32, i32) -> i32` to
`(i32, i32, i32) -> i32`. `ABI_VERSION` 116 → 117. A v116 module is rejected
by the existing version check at load (`game-rules/src/host.rs:3206-3214`).

### 4.2 Host request (`game-rules/src/host.rs`)

```rust
Draw {
    player_id: i32,
    n: i32,
    /// Skip the immediate hand-limit discard-down for this draw
    /// (「可超过上限」). The limit state itself is untouched.
    over_hand_limit: bool,
},
```

`HostRequest` is not serialized (it is re-derived by re-running the guest
body), so adding the field is not a save/record change. Every
`matches!(req, HostRequest::Draw { .. })` keeps working, but two sites
destructure the variant exhaustively and need the new binding:
`wasm_rules.rs:4347` (`HostRequest::Draw { player_id, n }`) and
`rules/kani/tests/conf_model.rs:1034` (`{ player_id: _, n }`).

### 4.3 Host import (`game-rules/src/hostfns.rs`, `host.rs`, `native_shims.rs`)

```rust
pub fn draw<C: HostCtx>(c: &mut C, player_id: i32, n: i32, flags: i32) -> Result<i32, HostErr>
```

* `over = flags & abi::import::DRAW_OVER_HAND != 0`.
* Carried on `HostRequest::Draw` (4.2).
* `host.rs` linker: `func_wrap("draw", |c, player_id, n, flags| ...)`.
* `native_shims.rs`: `op_draw` already takes `[i32; 6]`; pass `a[2]` through
  and change `ruleset_draw` to three parameters.
* Both `HostRequest::Draw` construction sites gain the field: the canned-answer
  path and `crate::inline::request_or_inline`.
* `apply_reply` (standalone fixtures / a positive inline reply) calls
  `w().draw_over(..)` when `over`, else `w().draw(..)`.

### 4.4 `CardWorld` (`game-rules/src/world.rs`)

`fn draw(&mut self, player_id: i32, n: i32) -> i32;` stays. One defaulted
method is added so no existing impl changes:

```rust
/// Draw without the immediate hand-limit trim. Default: same as `draw`.
fn draw_over(&mut self, player_id: i32, n: i32) -> i32 { self.draw(player_id, n) }
```

`impl CardWorld for Run` (`wasm_rules.rs:688`, the live host-state world)
implements both through one private `draw_impl(player, n, over)` and passes
`over` to `draw_cards`; on this path the flag only suppresses the `over_hand`
warning (the trim lives in the match routine). `NullWorld`
(`host.rs:5719`) keeps the default.

### 4.5 Engine (`game-core/src/engine/play.rs`, `ops.rs`)

```rust
pub fn draw_cards_with_hooks(
    &mut self,
    i: usize,
    n: usize,
    hooks: bool,
    over_hand_limit: bool,
) -> Flow<i32>
```

The trailing trim becomes
`if !over_hand_limit && self.playing() { while self.over_hand(i) { … } }` —
the `playing()` guard stays, so draws outside play (deck refills, fixtures)
still never trim.
`draw_cards`'s existing parameter also stops being `let _ =`: it suppresses the
`log.part.over_hand` warning part for the same draw (the `over` value in both
log sites becomes `over_hand && !over_hand_limit`). Call sites of
`draw_cards_with_hooks` are only `draw` (`play.rs:4549`), the `HostRequest::Draw`
arm (`wasm_rules.rs:4376`) and three `deck_refill` test calls (`:99`, `:112`,
`:130`); `draw` passes `false`, so opening hands and engine draws keep their
exact current behaviour.

The `CardWorld::draw` path (4.4) is the fixture / positive-inline-reply path and
does not run the trim loop; the live card path is always
`HostRequest::Draw -> draw_cards_with_hooks`.

### 4.6 Card declaration — `prop::AI_NEVER`

`card-sdk/src/abi.rs` (`prop`) and `game-core/src/state.rs` (`prop`),
kept in step with each other:

```rust
/// Bot-only: nonzero = bot policies (standard / advanced) must not offer
/// this card. Absent or `0` = offer. A static card fact, never legality:
/// a human can still play the card. Read by `CardRules::ai_play`; rides the
/// existing `ManifestEntry.props`.
pub const AI_NEVER: &str = "aiNever";
```

A prop, not a `CardDef` field, because:

* props are the established generic static-fact surface and are already
  exported in `ManifestEntry.props` (`rt.rs:64-74`) and stored in
  `CardInfo.props`;
* the default (`0` / absent) preserves every existing card with no
  opt-in migration;
* no `CardDef` struct field means no threading through the nine const builder
  methods and no manifest schema field.

Card-side spelling:

```rust
use card_sdk::abi::prop;
pub const SUMMER_CAMP: CardDef = CardDef::new("Mor:夏日合宿", &[...])
    .props(&[(prop::AI_NEVER, 1)])
```

(`props` is replace-not-append; a card that already declares props adds the
pair to the same slice.)

### 4.7 Host policy

`RulesBridge` overrides the existing trait method (`wasm_rules.rs`, next to
`card_prop`):

```rust
/// Would a bot play it now? Static card fact: `prop::AI_NEVER`.
fn ai_play(&self, _cx: &Cx, _player: usize, card: &str) -> bool {
    self.card_prop(card, card_sdk::abi::prop::AI_NEVER) == 0
}
```

`Rules::ai_play`'s doc gains one sentence: standard / advanced consult it;
chaos ignores it by design (unchanged at `ai.rs:292-293`).

### 4.8 Seat-view extra (`game-core/src/engine/mod.rs`)

```rust
pub struct ViewExtra {
    pub ai_answer: Option<RawAiAnswer>,
    /// Would `cant_play` allow each card (human legality / glow)?
    pub playable: Vec<bool>,
    /// Would a bot offer it: `playable[i] && rules.ai_play(..)`.
    /// Static facts only, so the seat-view engine computes it exactly.
    pub ai_playable: Vec<bool>,
    pub est_cost: Vec<i32>,
    pub skills: Vec<SkillAction>,
}
```

`view_extra_parts` fills it next to `playable`:
`ai_playable.push(ok && self.rules.ai_play(&cx, i, c))` (one call per hand card;
`ai_play` is a prop read, no guest run). `view_extra_inner` emits `"aiPlayable"`
beside `"playable"`. `playable` stays the glow source; `aiPlayable` is the
autopilot source, and it is always `playable`-and-something (never an offer of
an illegal card).

Client / bot plumbing (all additive, no game rules):

* `bot-core/src/extras.rs`: `ViewExtras.ai_playable` (aligned with `playable`),
  `to_json()` emits it, `view_extras` copies it from the fork.
* `bot-core/src/view.rs`: `BotView.ai_never: HashSet<String>` filled from
  `hand` + `ai_playable`; a set, not a `Vec`, because `next()` resets
  `playable` to all-true for the speculative root (`:282`) while `aiNever` is
  a property of the card id and must survive. `BotView::offerable()` returns
  `playable[i] && !ai_never.contains(&hand[i])`.
* `bot-glue/src/lib.rs` (`:587`, `:740`), `bot-service/src/lib.rs` (`:400`,
  `:569`), `bot-core/src/sim.rs` (`:202`): pass `offerable()` / the same
  `playable & !aiPlayable` combination to `legal_actions_with_cost` instead of
  `&view.playable`. The `est_cost` argument and the function's own contract are
  unchanged (search policy, never legality).
* `rules-worker/src/lib.rs`: `needExtras` copies `"aiPlayable"` (`:365` beside
  `"playable"`; the field-list comment at `:333` is updated).
* `web-glue/src/lib.rs::frame` (`:761-773`, the offline/托管 twin of
  `view_extra_typed`): emit `"aiPlayable": extra.ai_playable` beside
  `"playable"` — without it the browser autopilot never sees the flag.
* `webui/src/core/types.ts`: optional `aiPlayable?: boolean[]`;
  `webui/src/game/botPool.ts` / `viewExtras.ts`: type + `mergeExtras` and the
  `:94` same-frame equality check;
  `webui/src/game/autopilot.ts::affordableCards` (`:657`): standard / advanced
  use `view.aiPlayable ?? view.playable`; the chaos path (numeric `params`,
  `:803`, `:807`) keeps the `playable`-only filter.
* TS `MatchView.playable` semantics are unchanged (glow stays legality).

---

## 5. Engine wiring

### 5.1 Offer filtering (item 2)

| caller | before | after |
|---|---|---|
| standard engine bot (`ai.rs:329`) | `rules.ai_play` always `true` | reads `AI_NEVER`; 夏日合宿 filtered |
| chaos engine bot (`ai.rs:312-323`) | ignores `ai_play` | unchanged; documented |
| advanced bot service (`legal_actions*`) | `view.playable` | effective `playable & aiPlayable` |
| browser 托管 (`autopilot.ts`) | `view.playable` | `aiPlayable` (standard), `playable` (chaos) |
| human manual play / glow | `playable` | `playable` unchanged |

`cant_play` is not consulted differently, so the server still accepts a manual
夏日合宿 play on any seat. `docs/SERVER.md:144`'s limitation paragraph is
updated with it (the takeover now reads `aiPlayable`; chaos is unchanged).

### 5.2 Hand-limit read (item 1)

The read point does not move. `hand_limit()` stays the single source
(`state.rs:792`); `HAND_LIMIT` / `NO_HAND_LIMIT` / `HAND_LIMIT_DELTA` stay as
they are. The flag only tells the trim loop to wait.

```
ctx::draw_over_hand ──> ruleset_draw(p,n,1) ──> HostRequest::Draw{over:true}
   ──> draw_cards_with_hooks(..., over=true) ──> cards move, hooks run
   ──> if !over { discard-down }              ──> hand may stay > cap
```

* `over_hand` is still true, so `discard` is legal and `end` is refused until
  the hand reaches the cap (`mod.rs:2991-2993`, `3012-3016`); the client
  derives `overHand` from hand length vs `hand_limit()`
  (`webui/src/core/hand.ts:18`) and already shows the discard row on it.
* `discard_down_to_five` (the card's scheduled body) is unchanged.
* `ctx::start_hand` / `ctx::inc_start_hand` need no flag: they are read by the
  engine **directly from live state** after the `deckBeforeGame` bodies commit
  (`play.rs:403-413`, `489-496`), with no host routine in between. `START_HAND`
  is an opening-hand size, not the cap; neither is affected.
* Field props `HAND_LIMIT_DELTA` already work because `hand_limit()` sums the
  live field instances; only guest *state increases* at a routine boundary were
  broken.

### 5.3 Kani model

* `card-sdk/src/ctx/sys_kani.rs::draw(player_id, n, flags)` passes `flags` as
  slot 2.
* `rules/kani/src/host.rs::arm_draw` reads `a[2]`; bit 0 pushes the new
  `Effect::DrawOver { player, n }`, otherwise the existing
  `Effect::Draw { player, n }`.
* `rules/kani/src/world.rs` gains `DrawOver` (all existing `Effect::Draw`
  patterns are unaffected; only over-draw specs match the new variant).
* No new `kani_host::Op`; `ruleset_draw`'s arity stays three on all backends.

---

## 6. Card usage

### 6.1 `nanami_effort.rs` (item 1)

**Before** (`:217-228`): read `handLimit`, set `NO_HAND_LIMIT`, `ctx::draw`, set
the old value back; the `TODO(ABI)` at `:217-220` explains the failure.

**After** (`effect_draw`, `:213-233`):

```rust
// 规则书（1）: 「抽x张卡（可超过上限）」 -- the draw itself is exempt from
// the immediate hand-limit trim; 「回合结束后将手牌弃置到五张」 below is the
// scheduled discard that gets back to the cap.
ctx::draw_over_hand(player_id, x)?;
ctx::before_turn_end(player_id, "discard_down_to_five");
```

The `HAND_LIMIT` / `NO_HAND_LIMIT` read/write pair drops from this file. The
scheduled `discard_down_to_five` body is unchanged. `effect_draw` gains a
Kani-only export `effect_draw_kani` for the harness (the full `play` path
cannot reach branch (1) under Kani: `arm_tok_names` returns an empty list).

### 6.2 `summer_camp.rs` (item 2)

**Before** (`:19-33`, `:44-45`): no prop; the `TODO(ABI) ai-play` comment says
bots still offer it.

**After**:

```rust
use card_sdk::abi::prop;
// C# `CardSummerCamp.AiPlay => false`: standard/advanced bots never offer it.
// Human legality is untouched -- `aiNever` is not a `cant_play` gate.
pub const SUMMER_CAMP: CardDef = CardDef::new("Mor:夏日合宿", &[
    On::Play("", None, summer_camp),
    On::Filter(Filter::Untargetable("actor == owner && by >= 0 && by != owner", None, untargetable)),
    On::Action(Action::TurnStart("actor == owner && card.placed", None, turn_start)),
])
.props(&[(prop::AI_NEVER, 1)])
.meta(...);
```

The play body, the untargetable filter, the turn-start end path and the
`*_kani` exports (`summer_camp_kani` / `untargetable_kani` / `turn_start_kani`)
are unchanged. The TODO is removed; the source comment keeps the C# provenance.

---

## 7. Save / record / replay

* **SAVE_VERSION unchanged.** No durable shape changes. A save is taken at a
  prompt boundary; the guest body re-runs from the top on restore and re-issues
  `draw_over_hand`, so the flag is not stored. Hand contents are saved state;
  `overHand` derives from them and the saved limit. `AI_NEVER` is static
  ruleset data.
* **RECORD_VERSION unchanged.** The input stream does not move: the draw reply
  is still the negative already-applied count; `ai_play` picks are recorded as
  ordinary `act:"play"` answers like any other bot choice. A record does not
  carry the flag.
* **Replays.** Replays ride archived engine bundles. A record from before the
  change replays the old card body against the old bundle (immediate trim); a
  record after it replays the new bundle. No migration.
* **Ruleset identity.** Adding the prop changes the ruleset hash, which the
  record stamp already carries; archived bundles keep their own manifest. Their
  `CardInfo.props` predates the key and parses as empty (`#[serde(default)]`,
  `host.rs:109`), so `aiNever` reads `0` (offer) on an old bundle -- exactly the
  old behaviour. No `ManifestEntry` shape change.
* **View frames.** `aiPlayable` is derived data, not saved; a client that does
  not know the field falls back to `playable`, which is the current behaviour.

---

## 8. Versioning

`ABI_VERSION` 116 → **117** (last group; next after current if the order holds; if G06
`counteraction-and-join` lands first, this becomes 106 — one bump per delivery).
Entry inserted immediately **below** the `pub const ABI_VERSION` line for
newest-first order (`abi.rs:846`, where v104 already sits; the older entries
live in the fenced list below the constant), matching the `/// vNNN:` style:

```
/// v117: **over-limit draws and a card's static bot-avoid flag**.
///    `ruleset_draw` gains a `flags` word: [`import::DRAW_OVER_HAND`] (1)
///    makes the draw skip the immediate hand-limit discard-down while still
///    moving the cards and leaving `overHand` true (the over-limit warning is
///    suppressed for that draw); `ctx::draw` passes 0 and the guest sugar
///    `ctx::draw_over_hand` passes the bit. The limit state itself is
///    untouched. `ManifestEntry` is unchanged: the new card prop `aiNever`
///    (1 = standard/advanced bot policies do not offer the card; absent/0 =
///    offer; never legality) rides the existing `props` map and the existing
///    host `CardRules::ai_play` seam. `ViewExtra` gains `aiPlayable` so the
///    client-side seat-view engine / autopilot reads the same static fact; no
///    hidden reads. SAVE_VERSION / RECORD_VERSION unchanged.
```

The ABI bump is for the **import arity**, not for the prop: a v104 module
calling two-arg `ruleset_draw` would misread the stack on a v117 host.

Manifest line text (both prop modules, identical):

```rust
/// Bot-only: nonzero = bot policies (standard / advanced) must not offer this
/// card. Absent or 0 = offer. Static card fact, never legality. Read by
/// `CardRules::ai_play`; rides `ManifestEntry.props`.
pub const AI_NEVER: &str = "aiNever";
```

---

## 9. Tests and Kani obligations

### 9.1 Per item (`rules/tests/tests/`, `crates/*/tests/`, `webui/src/**/*.test.ts`)

Item 1:

| test | assertion |
|---|---|
| `g14_nnm_draw_over_cap_is_not_trimmed` (`morfonica.rs`) | hand of six incl. NNM, two marks, branch (1), x=2: after the draw the hand is seven (nothing was discarded by the routine) |
| `g14_nnm_end_of_turn_discards_to_five` | continuing that turn to the end leaves five, via the scheduled body |
| `g14_over_draw_leaves_over_hand_true` | after the draw `over_hand` is true, the over-limit warning part is absent, `act:"end"` is refused with `err.over_hand`; a manual discard clears it |
| `g14_plain_draw_still_trims` | a normal draw on the same table still discards down immediately (contrast: the flag is not globally on) |
| `g14_draw_over_hand_reaches_the_host` (`inline_provider.rs` pattern, which sees each `HostRequest` before it applies) | the observed `HostRequest::Draw` carries `over_hand_limit: true`; a `ctx::draw` card's request carries `false` |
| `g14_effect_draw_schedules_the_discard` | the `before_turn_end("discard_down_to_five")` schedule exists after the over-draw |

Item 2:

| test | assertion |
|---|---|
| `g14_summer_declares_ai_never` (ruleset build) | `rules.card_prop("Mor:夏日合宿", prop::AI_NEVER) == 1` |
| `g14_standard_bot_skips_summer_camp` (`morfonica.rs`) | bot seat, 夏日合宿 the only hand card at 运营: the turn passes without a play and the card stays in hand |
| `g14_standard_bot_plays_a_normal_card` | contrast: the same setup with a control card that declares no `aiNever` does play |
| `g14_chaos_still_plays_summer_camp` | chaos keeps its "play everything" contract (open question 1; invert if the ruling changes) |
| `g14_human_can_still_play_summer_camp` | `playable` is true and the manual `act:"play"` is accepted |
| `g14_view_extra_ai_playable_marks_summer_false` (`game-core/tests/view_extra.rs`) | `playable[i] == true`, `ai_playable[i] == false`, other cards true |
| `standard_skips_ai_never` / `chaos_plays_ai_never` (`webui/src/game/autopilot.test.ts`) | the autopilot command list honours / ignores `aiPlayable` |
| `view_extra_equiv` (`bot-core/tests/view_extra_equiv.rs`: the seeded and solo tests) | `aiPlayable` is compared with the same "must agree every time" counter as `playable`, and `strip_extras` clears it |

Existing suites that must stay green:
`summer_plays_to_field_until_next_turn_then_draws`,
`interaction_summer_shield_vs_foreign_targeting`,
`nnm_discard_marks_draw_x`, `nnm_hand_play_spends_marks_and_draws`,
`crychic_band_skill_hand_limit_none`, `g_cards_01_*`,
`interaction_band_hand_limit_vs_draw`, all `*_kani` `ppcost` exports.

### 9.2 Kani obligations

`rules/kani/src/spec/abi_g14.rs` (or the `mor` spec) and
`rules/kani/src/proofs/abi_g14.rs`:

* `nnm_02_draw_over_iff` — call `effect_draw_kani` with symbolic `x` in
  `1..=4`: the effect log is exactly `x` × `Effect::DrawOver { player, n: 1 }`
  (`ctx::draw` issues one import per card, so every entry carries `n = 1`; the
  existing unused `nnm_02_effects_ok` expects one `Draw` of `n = x`, which
  never matched the engine -- the new spec replaces it). The `before_turn_end`
  schedule stays protocol-level: `schedule_turn_end` is `arm_stub_zero`.
  `unwrap_done` shows the body returned; frame: hand +x, dest and money
  unchanged.
* `nnm_02_mutant_plain_draw_is_detected` — the same shape over `Effect::Draw`
  fails once the card uses the flagged draw (a variant + count mismatch).
* `plain_draw_stays_plain` — a normal-draw card (e.g. `band_01`) still logs
  `Effect::Draw`, so the flag is not always-on.
* `summer_ai_never_declared` — `SUMMER_CAMP.props` contains
  `(AI_NEVER, 1)`; `summer_ai_never_mutant` checks a copy without it fails.
* No new `Op`; `arm_draw` must not hit the unmodeled-op guard for flags `0`
  and `1`.

Reaching branch (1) through the full NNM `play` body stays BLOCKED-HOST
(`arm_tok_names` returns an empty list, so the body returns before the pick;
KANI-COVERAGE C-MOR-nnm-02); these proofs cover `effect_draw` directly, which
is the ABI half. The summer `*_kani` exports and bodies are unchanged, so a proof
harness added by a concurrent Kani batch compiles as is.

---

## 10. Alternatives

**A1 — a real `CardDef` field plus a `ManifestEntry.ai_play` bool.**
Most explicit, natural `true` default, and future-proof for a tri-state. Cost:
thread the field through every `CardDef` builder (nine `const fn`s), append a
manifest field (postcard is not self-describing, so a second wire break), and
add a `CardInfo` field. Rejected for v117 because `props` already carries
static facts to the host and the client and the inverted default is confined to
one documented key. A1 remains the migration path if `aiNever` ever needs more
than a bool.

**A2 — a wasm `OP_AI_PLAY` hook (`fn(&Cx, player, card) -> bool`).**
Truest to "hook" and lets a card inspect the world. Rejected now: every view
extra would instantiate and run a module per hand card (the client extras are
on the interaction path), and the C# reference and every current card are
static. Note the seam can be added later: a card prop `aiHook` could name an
entry, with the static bool as the fast path.

**A3 — let guest state increases cross the routine boundary (general fix).**
One condition in `overlay_guest_state` (accept `wrote.value > base_v`, not only
`<`). It would make the existing set/restore in `nanami_effort` work with no
new import. Rejected as the primary: it changes every host routine's view of
pending state, including the explicitly-open ruling in `ENGINE.md:428-435`
(a body that just stunned a player then gains — should its own stun gate the
gain?). That ruling should be decided in its own batch. Kept as a complement:
once increases cross, `ctx::draw` still needs to know *which* draws may exceed
the cap, so the flag does not become redundant.

**A4 — set the limit to `NO_HAND_LIMIT` and restore around each card of the
draw, from the guest.** Still fails: the restore/raise is a guest write and the
routine boundary drops increases. Unless combined with A3.

**A5 — encode "over" in the sign of `n` (no ABI bump).**
`ctx::draw_over_hand` would call `sys::draw(player, -1)`. Avoids the arity
break, but `ctx::draw` already calls the host once per card and negative counts
are also a natural future "draw up to" shape. Rejected for readability and
because a flags word is already the house pattern (`pay_rent`'s `half`).

**A6 — make `playable` itself bot-filtered and add a separate legality list.**
Would avoid a new view field but renames an established contract used for glow,
and every existing consumer of `playable` would silently change meaning.
Rejected.

**A7 — re-add trimmed cards from the discard inside the card body after
`ctx::draw`.** No ABI change, but the cards crossed the discard (`discarded` /
`discardAfter` hooks fire), ordering with multiple copies is unstable, and the
draw hooks saw the wrong hand size. Rejected.

---

## 11. Open questions

1. **Chaos.** Chaos deliberately ignores `ai_play` (`ai.rs:292-293`). Keep
   `aiNever` as a standard/advanced fact only (design default), or make it
   absolute so chaos also skips 夏日合宿? The engine and the client must agree
   either way.
2. **Conditional AI.** C# is a bool. If a future card wants "skip only when
   X", does that become a CEL-valued prop, a second prop, or A2's hook?
3. **Advanced search.** Exclude an `aiNever` card from the action set
   (design default, cheaper and matches "never offer") or leave it in the tree
   with a lower prior?
4. **Naming.** `draw_over_hand` vs `draw_exceeding_limit`; `aiNever` vs
   `botSkip` / `aiPlay: 0`. The `aiNever` name is chosen for the non-inverted
   default, not because "never" is book vocabulary.
5. **Over-limit logging.** Design default: `overHand` stays true (the UI and
   the gates agree) while the `log.part.over_hand` warning is suppressed on the
   exempt draw only, because the card permits it. Confirm that split, or keep
   the warning unconditional.
6. **Scope of the exemption.** Only cards whose text says 「可超过上限」 should
   call `draw_over_hand`; the base rule
   「任何时刻…立刻弃至上限」 (`rulebook-doc.md:126`) stays the default. Is any
   other existing draw text an implicit exemption?
7. **Skills.** `ai_play` currently governs hand-card choice only. Should a
   skill's press entry be able to declare `aiNever` (the `ai_skill_choice`
   path has its own per-skill params)?
8. **General state crossing (A3).** Confirm the 心の雨 ruling before flipping
   the overlay; a batch that does so should decide whether increases cross for
   all keys or only for declared ones.
