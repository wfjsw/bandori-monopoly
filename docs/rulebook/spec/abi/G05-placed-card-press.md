# ABI design G05 — Offers from placed cards

**Status:** draft for user review · design only, no source touched.
**Worktree:** `.claude/worktrees/g05-design-review` (branch
`worktree-g05-design-review`), base `master@b97eac0d` (ABI 110, SAVE 14,
RECORD 4).
**Group:** G05, family "Press surface for placed cards".
**Must land with:** G13. Item 1 invokes another card's named clause, and G13 owns
that import; §6 adopts it verbatim.
**Ground truth:** `docs/rulebook/cards.json` (sheet-derived). Every quoted
fragment below is verbatim from it. Rows are indices into
`docs/rulebook/card-review-2026-10-27.json`.

The sheet writes two kinds of field clause on a placed card, and neither is a
button. This document uses one mechanism for each:

* an **offer** is a question the engine raises, because a printed condition just
  became true — 「主要阶段中，若此卡上有至少9个水母标记，可以…」,
  「你可以移除一个奇迹水晶…」, 「主要阶段中，你可将此卡置入弃牌堆并进入移动
  阶段…」. It is an `On::Action` hook plus `ctx::ask_yes`; the player answers
  发动/不发动 and the effect runs. An offer is anchored at the *event the
  clause names* — the pre-dice plan pass for the two movement offers, the end of
  (3)'s placement for the crystal. An offer has no button;
* a **passive clause** is a printed effect with no question at all: it applies
  when the card enters and is a property of the instance from then on
  (「本卡在[场地]时改变本卡代替的专属卡效果」).

The remaining terms:

* the **instance** is the `FieldCard` row (`FieldCard.uid`). A hook body runs on
  the instance that carries the hook, and the engine binds it
  (`Run::current_uid`), so every instance read answers for that copy;
* the **effective card** of an instance is the card whose printed surface the
  instance runs (`FieldCard.effective_card`, §6.2); it is the instance's own
  card unless a stand-in wrote the marker (§9.5);
* an **entry** is one printed clause, named by its required slug `id`. Entries
  matter for cross-card invocation and `query_guard` (§6.1).

## 0. Row disposition

The consolidated BACKLOG slug `press-surface-placed-card` lists nine rows. None
of the nine is a press-surface bug. The offer defects sit on four cards and two
engine stages, which the plan below closes.

| row | card / file | finding | there or here |
|---|---|---|---|
| 380 | `CRYCHIC:想要抓住...` `want_to_grab.rs:93` | stale TODO (lists `plan::set_teleport_to` as missing; it exists) | neither — note only |
| 487, 488, 490 | `HHW:梦幻的回礼` `dream_return.rs:89,101,48` | payment pipeline is `PayKind::Card`, no rent factor; `payAfter` raised before the caller's record/crystals | neither — G02/G10 |
| 955 | `PP:[白鹭千圣]微笑的铁假面` `chisato_mask.rs:110` | split-pay payers never run the [指定] pipeline | neither — G07 |
| 967 | `PP:[衍生]共鸣` `resonance.rs:17` | module doc paraphrases 「为…乐队卡」 as "to a player" | neither — doc typo |
| 974 | `PP:[衍生]共鸣` `resonance.rs:62` | `ctx::in_group` reads the persona's group, not ownership of a PP band card | neither — band-skill attachment |
| 1521 | `Sumimi:#L12` `l12.rs:128` | (3) is a state trigger only re-checked inline at one call site | neither — state clauses on events |
| 1555 | `Sumimi:Sumimi不会解散哦` `no_breakup.rs:52` | implementing sites do not cite their clause | neither — citation pass |
| 437 | `CRYCHIC:（祥子）带领着大家` `sakiko_lead.rs:27` | missing clause citation | neither — citation pass |

The offer defects, recomputed from the review file:

| # | slug | severity | true rows | file |
|---|---|---|---|---|
| 1 | `nanami-press-invokes-clause-3` (shared with G13) | high | 734, 735, 737 (also 742, 745) | `rules/cards/card-morfonica/src/nanami_effort.rs` |
| 2 | `kanon-press-surface-abi` | high | 607, 608, 610, 611, 612, 619, 620, 621 | `rules/cards/card-hhw/src/kanon_march.rs` |
| 3 | `even-lost-not-pressable` | high | 798, 801 | `rules/cards/card-mygo/src/even_lost.rs` |
| 4 | `placed-card-press-surface` | high | 1614 | `crates/game-core/src/engine/mod.rs:1416`, `crates/game-core/src/engine/play.rs:1623` |
| 5 | `hina-clause-2-swap-missing` (moved from G01 R11) | medium | 930, 931, 932, 936, 972 | `rules/cards/card-pp/src/hina_sound.rs` |

Item 5 moved here whole from G01: its clause (2) is a property of a placed
instance, and the effective-card marker it writes is part of the same offer
surface. G01 keeps nothing of it; the lifetime edge is G08's (`created_by`,
G08 §4.9).

Items 2 and 3 also have a *demoted* half. The review's stated expectation
("appears pressable, reaches `cant_act`") is reachable by deleting one prefix
filter, so a stopgap can ship alone. It only makes the cards *reachable*: the
bodies still have to guess their origin, and the offer still fires at
whatever moment the player presses. The full fix — the offers of §4-§6 — is
still needed, and it is not a button at all (§1.4).

---

## 1. Problem, per item

### 1.1 `nanami-press-invokes-clause-3` (rows 734, 735, 737)

Book (`Mor:（NNM）稍微努力了一下`, clause (3)):

> （3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的
> （2）技能，此次技能不受数量或轮数限制

「你的（2）技能」 is the *persona skill's clause (2)*. The card invokes the skill
card's `On::Play` instead, which is that skill's clause (3):

`rules/cards/card-morfonica/src/nanami_effort.rs:263-283`

```rust
    ctx::add_crystals(-1, 0)?;
    // TODO(规则书)（此次技能不受数量或轮数限制）: the book waives the skill's count/CD
    //   here -- nothing is waived.
    // TODO(规则书)（你可以移除一个奇迹水晶视为发动你的）: the book runs the persona
    //   skill's (2) (pay a pot, gain a mark) -- `play_card` runs its (3) body, which
    //   spends a mark instead.
    ctx::play_card(id.as_str(), player_id)?;
```

`skill:广町七深:这是很普通的事吧？` declares (2) as
`On::Action(Action::CardUsed(…))` (`rules/skills/skill-personas/src/nanami.rs:58`)
and (3) as its `On::Play` (line 50). `ctx::play_card` resolves "the card's first
`On::Play`", so (2) is unreachable by id. Two further consequences sit in the
same rows:

* the crystal is spent *before* the body, so the removal happens with no
  question — a player who has a crystal but cannot use (2) burns it (row 734's
  probe);
* 「此次技能不受数量或轮数限制」 waives (2)'s per-kind mark cap and 3-turn CD;
  nothing evaluates them today, so nothing is waived (row 735).

The clause is an immediate **offer**, already printed as one: 「你可以移除一个
奇迹水晶」 is asked as part of resolving (3), at the moment the card lands. The
fix is `ctx::ask_yes` before the crystal is spent, then the invocation (§9.1);
the invocation itself is still a named entry (`"2"`), which is why G13 is
needed.

### 1.2 `kanon-press-surface-abi` (rows 607, 608, 610, 611, 612, 619, 620, 621)

Book (`HHW:（花音）Wacha Mocha 啪嗒进行曲`):

> [场]花音每次倒走时此卡获得一个水母标记；主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。

Two clauses: a [场] placement and a placed offer. One `On::Play` serves both, so
the gate guesses which one is meant:

`rules/cards/card-hhw/src/kanon_march.rs:41-57`

```rust
fn cant_play(player_id: i32) -> Option<Msg> {
    // TODO(规则书)(ABI)（视为本次主要移动）: the gate cannot tell a hand play
    // from a press -- `ctx::play_from_hand()` is only set once the hand play is
    // committed, so a held copy also admits a below-9 / move-spent press (which
    // `jump` then re-checks for markers only).
    if ctx::hand_count(player_id, ID) > 0 || !ctx::is_placed() {
        return None;
    }
```

and the body branches again at line 62. The jump is on no surface at all: it is
not a key kind but a *condition*, and there is no hook on the main move. The
jump is therefore an **offer raised before the main move's dice** (§9.2), gated
on ≥9 markers and on no other offer having claimed the same move. Rows 610-612 add that the jump only
writes the turn plan and never runs it (this tree already calls
`run_planned_move`, so those three are closed; they stay listed because they
were the original evidence). Row 621's main-move gate and row 620's second-copy
case are still open.

### 1.3 `even-lost-not-pressable` (rows 798, 801)

Book (`MyGO:即使迷茫着`, clause (2)):

> （2）[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。

The card is never played from hand — it enters through its own [反击] (1) — so
its `On::Play` is a field activation by definition; once the clause becomes a
hook at the plan pass, nothing calls it. What is missing is the two things that
make the clause usable: a place to ask, and a place to write the distance.

The printed permission is 「进入移动阶段」: the card's use *is* the entry into
the movement phase, so the question belongs exactly where the engine opens that
phase — before the main move's dice are thrown. Asked any earlier it competes
with the normal move (the player would have to decide before seeing whether they
even want to move); asked after the roll it is too late. The distance is
「你当前手牌张数」, a fixed count, so the move must not throw a die at all.

Today `go` runs a whole second move of its own instead
(`rules/cards/card-mygo/src/even_lost.rs:65-78`): it calls `plan::set_steps`
then `run_planned_move` from inside the Play. From the plan pass the same writes
shape the move that is already running, so `run_planned_move` stops being part
of the answer (§9.3). `can_go` additionally asks `!ctx::is_placed()` and never
asks whether the move is available, so a use after the main move was cleared
burned for nothing (row 798).

### 1.4 `placed-card-press-surface` (row 1614)

The umbrella, at `crates/game-core/src/engine/mod.rs:1416`:

```rust
            if !id.starts_with("skill:") || !self.rules.has_play(&id) {
                continue;
            }
```

A skill becomes a button because it has a `skill:` id and a Play; a plain card
has neither, so no placed card offers anything. The offer list, its `source`
derivation (`strip_prefix("skill:")`, lines 1427-1430), the client's
`act: "skill"` sender (`webui/src/scenes/board/Side.tsx:214`), `skillMarks`'s
id lookup (`Side.tsx:95`), the browser autopilot (`webui/src/game/autopilot.ts:750`)
and the bot policy (`crates/bot-core/src/policy.rs:435-444`) are all built on
that one prefix.

The row's `fix_plan` says "Needs a capability, not a filter tweak", and that is
right — but the capability turns out **not** to be a wider button. A button is
pressed at a moment the player picks; three of these four clauses are printed at
a moment the *game* picks. Widening the button list would put 即使迷茫着 and
進行曲 in front of the player while the movement phase is not open, and make
their bodies guess whether the press is meant now. The capability the rows need
is an **engine-raised, stage-anchored question on a placed instance** — §4 and
§5 — plus the invocation and marker surfaces of §6. Item 4's own residue is the
first: the plan pass can dispatch a pure Filter but cannot pause for a player.

### 1.5 `hina-clause-2-swap-missing` (rows 930, 931, 932, 936, 972)

Book (`PP:[冰川日菜]会发出怎样的声音呢？`, clause (2)):

> （2）[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。

`rules/cards/card-pp/src/hina_sound.rs:17-20` registers one entry and no hook:

```rust
    &[On::Play("", None, hina_sound)],
```

`hina_sound` runs a substitute's `[手]` and then banishes the stand-in
(`:73`), so 「本卡在[场地]时」 can never hold again. The clause is **passive**
and attaches to the *substitute* from then on: nothing is offered, the
instance's surface is simply changed, and only an effective card can say which
surface that is (§9.5). Rows 930/932 also record that `SUBS` lacked 大和麻弥 —
the one substitute with no `[手]`. That half is already closed in the tree:
`SUBS` carries four and the body places 麻弥 with `place_card`
(`hina_sound.rs:43-48`, `:70-71`); the eligibility TODO at `:51-53` stays open
and the swap must keep the same `place_card` arm. Row 936 is a stale TODO
(`resonance::try_resonance` exists) and row 972 the missing fuel call.

---

## 2. Shared root causes

1. **One `On::Play` is two clauses.** `On::Play` means "play from hand" *and*
   (for these cards) "the placed clause". The body and gate recover which one by
   `is_placed()` / `play_from_hand()` / `hand_count()`. The recovery is per
   card, unenforced, and wrong with a second copy in hand.
2. **A stage body cannot ask.** `On::Filter` bodies are pure value folds
   (`fn(i32, PlanValue) -> PlanValue`, no `Result`), so a question raised at the
   plan pass — the only point before the dice — is impossible to write. The
   engine raises `diceRollPlan` (`play.rs:1623`), and every hook kind is
   dispatched the same way (`wasm_rules.rs:5836-5870`), but the SDK exposes no
   `Action` constructor over that kind and the loader refuses one (§4).
3. **Nothing marks a move as already claimed.** Two offers at the same pass
   would both fire and both spend a card on one move (§5).
4. **An instance has exactly one printed surface.** Hook dispatch resolves
   `FieldCard.card` (`wasm_rules.rs:5849-5868`), so a stand-in cannot change
   what an instance does (§6.2).
5. **A clause cannot be named.** Cross-card invocation resolves "the first
   Play" (`wasm_rules.rs:10201`), which is the wrong clause for nanami (§6.1).

### 2.5 Necessity — why the group cannot be smaller

**Every row is printed text, not a preference.** The sheet puts an offer on a
placed card three times: nanami 「你可以移除一个奇迹水晶」, kanon 「主要阶段中，
若此卡上有至少9个水母标记，可以…」, even-lost 「主要阶段中，你可将此卡置入弃牌堆
并进入移动阶段」, and a passive change once, hina 「本卡在[场地]时改变…效果」.
Today two are unreachable and two are wrong. A placed card with an offer is a
whole category, not four oddities, so leaving it unimplemented is a functional
hole the triage graded high.

**Nothing smaller reaches the text.** The retrenchments and what each leaves:

| retrenchment | buys | still fails |
|---|---|---|
| A1, drop the `skill:` filter (§13) | kanon and even-lost appear as buttons | both still guess their origin, and both are pressed before the phase opens; every `[手]` card replays from the field |
| A2/A3, a prop or a new kind (§13) | the same button | a prop must be read from the manifest anyway; a kind buys a second button no clause here uses |
| card-side only, no ABI | nothing | a card cannot teach the plan pass to wait, cannot mark a move claimed, and has nowhere to record "this instance runs the stand-in's surface" |
| offers without the invocation half | item 3 alone | nanami still runs the wrong clause; item 1 is the group's shared row with G13 |

**Four mechanisms, and only the plan stage is new machinery.** The crystal
question is an ordinary `ask_yes` inside a resolving body; the marker question
is an ordinary `ask_yes` inside a hook — the engine already pauses a hook body
(`host.rs:1960-2014`) and resumes it with the answer. Only the two movement
offers need something absent: an **action** hook at the plan pass (§4), and a
per-move claim so the two do not both spend (§5). Hina needs none of that and
gets the marker instead (§6.2).

**The instance is the common denominator.** Every one of the four needs the
instance (kanon's marker pool and teleport target, nanami's crystal, hina's
substitute, even-lost's discard) and every one but even-lost needs a way to run
another card's named clause (`invoke_entry`) or to swap the surface an instance
runs. That, and only that, is the ABI this group adds — one `Action` constructor
over an existing kind, one claim, G13, and the effective-card marker.

**The cost is small and half already paid.** The hook drive already binds
`current_uid` and already pauses for prompts; the plan pass already re-reads the
plan after the raise (`play.rs:1624`). What is added is one guest constructor
plus its loader relaxation, one `TurnCtx` bool and one import, two imports for
the marker, and two card rewrites that delete code rather than add it. No
existing body shape moves; no client, wire or bot surface changes (§8).

**G13 is in the same window.** Item 1's invocation is G13's surface; landing G05
later means writing the offer against a call it cannot make.

**What would make the group unnecessary.** A ruling that placed cards never
offer anything collapses items 2, 3 and 4 into a `skill:`-prefix tweak; the
sheet contradicts it three times. If only item 1 were wanted, it needs no
surface and could ship on G13 alone — but the four remaining cards stay broken.

---

## 3. Capability as an invariant

**An offer printed on a placed card is a question raised by the engine at the
event the clause names, on the instance that carries it, and at most one offer
per event is taken — never a button, never an origin guess, never a card
activation.**

Corollaries, all of which the surface below makes true:

1. The clause is an `On::Action` hook. A hand play and a placed effect are
   different entries, so neither body tests where it came from.
2. What prints a condition is an offer and what prints no offer is passive —
   neither gets a button. `ask_yes` covers the first; the instance and the
   effective card cover the second.
3. The offered instance **is** the hook's `current_uid`. Every instance read
   (`is_placed`, named counters, `crystals`, props, `dest`) answers for that
   instance, and the discard moves that copy.
4. The question is asked only when its own guard admits — the guard is the
   printed condition's remainder (the count, the movement gate, the phase). A
   declined offer writes nothing.
5. The first accepted offer on a move **claims** it; later offers at the same
   event stand down (§5). The claim dies with the move.
6. A hook is not an activation: no `cardUsed` raise, no `cards_played`
   increment, nothing negates it before its body. Its own effects raise their own
   windows.
7. Cross-card invocation names the entry: `ctx::invoke_entry(player_id, card_id,
   entry_id)`. `ctx::invoke_skill` is sugar for it on `entry::PRESS`, the card's
   first Play.
8. An instance's printed surface is its effective card. Wherever the engine
   resolves an instance's entries — hook dispatch, play, invocation — it reads
   `effective_card`, never the row's own card alone.

---

## 4. Surface — an action hook at the plan pass

**No new engine kind.** The pass exists (`TriggerKind::DiceRollPlan`, engine id
67), it is raised on every move (`play.rs:1623` for the main move,
`play.rs:1063` for a card-driven one) before any die, and the whole hook drive
already runs there. What is missing is the *action* side of it: `DiceRollPlan`
is declared only in `filter_kinds!` (`crates/card-sdk/src/kinds.rs:619`), its
kind constant sits in `impl FilterKind` (`abi.rs:2496`), and the loader rejects
an `On::Action` that lists a filter kind
(`crates/game-rules/src/host.rs:1058-1078`).

A filter body cannot pause: its type is `fn(i32, PlanValue) -> PlanValue`
(`crates/card-sdk/src/lib.rs:720`), with no `Result` to carry a prompt. An
action body is `fn(i32) -> Asked` and may ask. Both write the plan through the
same `ctx::plan::*` setters, and the pass re-reads the plan after the raise, so
the two forms are interchangeable in effect and differ only in whether they may
wait.

`crates/card-sdk/src/abi.rs`, inside `impl ActionKind` (line 1988):

```rust
    /// Action. The move's plan pass (行动阶段 8, before the dice are rolled) --
    /// the **prompting** side of [`FilterKind::DiceRollPlan`], same engine kind
    /// 67. An action body may pause (`ask_yes`) and rewrites the plan through
    /// `ctx::plan::*`; a filter body folds `PlanValue`. Both run, actions first
    /// (`host::CardInfo::hook_entries`); a body that pausing-replays re-runs
    /// from the saved pre-action state, so a write before the ask is safe.
    pub const DiceRollPlan: Self = Self(TriggerKind::DiceRollPlan);
```

and a `pub const fn is_dual_action_filter(self) -> bool` beside it answering
`true` only for that kind. `crates/card-sdk/src/kinds.rs`:

* `action_kinds!` gains the one line `DiceRollPlan @ dual;`
* a `@dual` shape arm, so its `KindInfo` carries `is_filter: true` while the
  constructor is still `Action` (`fn(i32) -> Asked`), and the `@engine_entries`
  arm still builds `ActionKind::DiceRollPlan`.

The type pins flip accordingly (`lib.rs:1624-1629`,
`lib.rs:1650-1672`, `kinds.rs:777,780`): a `Filter` kind is always
`is_filter`; an `Action` kind is `!is_filter` **unless** it is dual; the
`Action::KINDS` row-count pin moves 76 -> 77.

`host.rs:1058-1078` relaxes to one direction: a `Filter` entry may list only a
filter kind; an `Action` entry may list any engine kind that is not
filter-*only* — i.e. `!tk.is_filter() || tk.is_dual_action_filter()`. A dual
kind on a `Filter` entry is unchanged behaviour, so no manifest needs a rewrite.

The export is called with the `fn(i32) -> Asked` signature (`call_card`,
`host.rs:3140-3147`); the kind does not pick the signature, the entry does, so an
action body at a filter kind is invoked as an action with no other change.

**Not on `On::Gate`**: a gate is a pure decision and is folded per gate kind
(`load` refuses a gate at a hook kind), so the offer stays an Action.

---

## 5. Surface — the per-move claim

Two offers sit at the same pass, and a main move is one move: accepting 進行曲
after accepting 即使迷茫着 must not both discard. The claim is a one-shot flag
on the move:

```rust
// crates/card-sdk/src/ctx.rs -- `mod plan`
/// A card has claimed this move's plan at the pre-dice pass; later offers at
/// the same pass stand down. One-shot, cleared with the move.
pub fn claim();
/// Has any card claimed this move? (`false` outside a move.)
pub fn is_claimed() -> bool;
```

```rust
// crates/card-sdk/src/ctx/sys.rs
pub fn plan_claim();            // link_name = "ruleset_plan_claim"
pub fn plan_claimed() -> i32;   // link_name = "ruleset_plan_claimed"
```

Host side: `TurnCtx.plan.claimed: bool` in `MoveCtx` (`crates/game-core/src/engine/move_ctx.rs:188`,
beside `steps_fresh`; `TurnCtx.plan` is the `MoveCtx` at `world.rs:207`), written by `HostRequest::PlanClaim` and read by the plain
`fn plan_claimed(&self) -> i32` host method. It joins the fresh-bit family:
`plan::claim` sets `claimed = true`, `drop_plan_fresh` (`play.rs:2263`)
and `drop_stale_plan_shape` (`play.rs:2220`) clear it, so it covers exactly the
move whose pass raised it. `ctx::plan::is_claimed()` reads the live plan; inside
a hook the plan *is* the move, so no separate stack is needed.

Offers test it in their guard, never in the condition (it is derived plan
state, like `fixed_roll`):

```rust
if ctx::plan::is_claimed() { return false; }
```

and call `ctx::plan::claim()` immediately **before** their ask, so a decline
leaves the move open for the other card. A body is re-run from the saved
pre-action state after the answer, so the claim write repeats harmlessly.

---

## 6. Surface — invocation and the effective card

### 6.1 Shared with G13

G05 does not respell G13's invocation surface; it adopts it verbatim and adds
only the caller-side instance in the frame. If both groups land, **G13 §4.1-4.4
is the normative text** for the import itself.

```rust
// crates/card-sdk/src/ctx/sys.rs
pub fn invoke_entry(
    player_id: i32, card_p: i32, card_n: i32, entry_p: i32, entry_n: i32, flags: i32,
) -> i32; // link_name = "ruleset_invoke_entry"
/// The innermost invocation frame, postcard `InvokeFrame`; `(0, 0)` = size probe.
pub fn invoke_frame(out_ptr: i32, cap: i32) -> i32; // link_name = "ruleset_invoke_frame"
```

```rust
// card_sdk::abi::invoke (G13)
/// The caller has already paid the entry's cost; the body must not collect it.
pub const COST_PAID: i32 = 1;
```

```rust
// card_sdk::ctx
/// Run one named entry of `card_id` nested, as `player_id`, with its field
/// instance bound. Body only — it does **not** evaluate the target's gate (ask
/// `query_guard` first when the gate matters); the invoke frame seeds
/// `trigger::card` / `trigger::cards` instead of raising a category.
pub fn invoke_entry(player_id: i32, card_id: &str, entry_id: &str) -> Result<Dest, Prompt>;
/// `invoke_entry` with the cost already paid (`ctx::invoke_paid()`).
pub fn invoke_entry_paid(player_id: i32, card_id: &str, entry_id: &str) -> Result<Dest, Prompt>;
pub fn invoke() -> Option<InvokeFrame>;
pub fn invoke_paid() -> bool;
```

`ctx::invoke_skill(player_id, id)` becomes guest sugar (`ctx.rs`, no `sys`
declaration): `invoke_entry(player_id, id, entry::PRESS)`. Its host import
`ruleset_invoke_skill` stays as an alias so a pre-v111 wasm still links; the
guest declaration is what goes.

`ctx::play_card` **stays** a guest function with its own `ruleset_play_card`
import. It is used by many cards, it returns `(Dest, Prompt)` and defaults
`play_from_hand = false`; routing it through `invoke_entry` would need a
resolution mode `invoke_entry` does not take.

The frame's `entry` is always the **resolved** entry's declared id (`""` when
that entry is unnamed, which is the common case), never the alias spelling.
`entry::PRESS` stays the alias for the card's **first Play**; the
effective-card rule of §6.2 decides *whose* Play that is when the target is a
placed instance.

The frame's `by_instance` is the **caller's** `current_uid` — for this group
always the placed instance whose hook asked. It is `-1` from a hand card.

### 6.2 Effective card

Moved here from G01, whose original group carried the marker only as an offer
concern. Two-call string imports:

```rust
// crates/card-sdk/src/ctx/sys.rs -- two-call string, 0 bytes = none
ruleset_effective_card_at(instance, ptr, len) -> i32;      // byte length of the effective card id, 0 = none
ruleset_set_effective_card_at(instance, ptr, len) -> i32;  // 1 set, 0 no such instance
```

```rust
// card_sdk::ctx
pub fn effective_card_at(instance: i32) -> Option<NameId>;
pub fn set_effective_card_at(instance: i32, card: &str);
```

`effective_card_at(instance)` names the card whose **printed surface** the
instance runs; it is not a rename of `FieldCard.card`, so counters, props and
`dest` stay the instance's. `set_effective_card_at(instance, "")` clears. The
marker needs a live instance and a declared id and does not validate the pair.
Field:

```rust
pub struct FieldCard {
    // …
    /// The card whose printed surface this instance runs (`None` = its own).
    /// Written by a stand-in, dies with the instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_card: Option<String>,
}
```

**Resolution points.** Every place the engine asks "what does this instance
do?" resolves `effective_card` first and falls back to the row's card:

* hook collection and dispatch (`wasm_rules.rs:5849-5868` field walk,
  `:5948-5968` board walk, `:9500-9515` pure buy hooks) — both the ruleset index
  and the `card_id` string passed to `drive_hook`;
* the counteract offer loop's instance walk;
* `wasm_rules.rs:10201` (`play`) and `:9879-9900` (`cant_play`), so a play of or
  into a marked instance runs the effective card's Play;
* `apply_dest` (`wasm_rules.rs:6469-6493`) — it unplaces `uid` but files `card`
  (the caller's `current_card`), which under effective-card dispatch is the
  stand-in. It must file the row's own card, falling back to `card_id`. The
  same fix applies to `ctx::send_to_dest`'s `move_now` (`wasm_rules.rs:698`),
  its immediate sibling.

**What does not resolve it**: `ctx::effective_card_at` is *not* the row's card,
so a swap body that must name the current substitute reads it from
`field_instances` (§9.5 `own_card_at`).

Op ids, carved from G01's band (which ends at 346): `Op::effective_card_at =
347`, `Op::set_effective_card_at = 348`, `Op::plan_claimed = 349` (§5). G08
shifts to 350; the README amendment records it.

All three targets: wasm32 resolves the imports; native / `native-rules` needs
`HostOps::op_effective_card_at` / `op_set_effective_card_at` / `op_plan_claimed`
plus their forwarding impls in `crates/game-rules/src/native_shims.rs` and
`crates/rules-native/tests/abi_link_names.rs` pins; Kani routes the `sys`
declarations through `sys_kani` to the model ops (§7.6).

---

## 7. Engine wiring

### 7.1 Hook dispatch — already the right shape

`collect_hook_jobs` (`wasm_rules.rs:5836-5870`) walks every seat's field
instances in placement order, skips face-down cards (`card_face_down`), and
emits one job per instance whose rule hooks the kind; `run_hook_job` →
`drive_hook(cx, call, card_id, uid, trigger)` binds `current_uid = uid` and
pushes the resolving frame (`:3268`). A paced body suspends the raise and parks
a resume (`:6011-6020`). **No engine change is needed to offer from a placed
card** — the offers are card-side once §4 exists.

### 7.2 Ask, replay, ordering

A hook body that asks is rolled back to the saved pre-action state and re-run
with the answer (`host.rs:1960-2014`: each entry's condition and guard are re-evaluated and
the body re-entered; the shape `hagumi_marks.rs:143-150` already relies on), so every write before an `ask_yes` is re-applied once, not
twice. Body order is therefore:

```
guard -> claim (§5) -> ask -> [decline: nothing] -> writes -> set_dest
```

The ask is always the first thing that can pause, so a decline leaves the
instance untouched.

Dispatcher order is `On::Action` entries first, then `On::Filter`
(`host.rs:213-221`), both in declaration order: a prompting body always sees the
plan before any pure fold of the same raise. An offer's own guard is re-evaluated on the resume pass
(`host.rs:1966-1980`: `entry_admitted` runs per pass), so a guard reading `is_claimed` after
the claim must not exclude itself — the cards read it *before* claiming (§9.2,
§9.3).

### 7.3 Plan writes from a hook

The plan pass seeds `turn.plan` from the move and re-clones it after the raise
(`play.rs:1623-1624`), so `ctx::plan::*` writes are honoured:

* `set_kind(Teleport)` + `set_teleport_to(tile)` takes main_move's teleport
  branch (`play.rs:1636-1659`): no die, `diceRollBefore` still raised, the
  landing settles per `set_settles`;
* `set_fixed_roll(n, CARD)` (`ctx.rs:3387`, host `wasm_rules.rs:2500-2505`) is
  read by
  `run_move_roll` right after the raise (`play.rs:1668-1670`) and suppresses the
  cast (`roll.rs:532-534`), leaving the ladder's windows on a fixed face;
* `plan::set_steps` is fresh-scoped (`drop_stale_plan_shape`, `play.rs:2228`),
  so neither write leaks into a later move.

**One leak to close.** A main move that becomes a teleport never clears
`turn.fixed_roll` (only the roll branch does, `play.rs:1670`). An earlier offer
that pinned a face and a later one that teleported would leave the pin for the
turn's next move. The teleport branch clears it too.

### 7.4 `apply_dest` and `move_now`

§6.2's correction is the only dest change: file the instance's own card. A hook
offer discards through the run's `dest` tail, which is `current_uid`-bound, so
the offered copy is the one that leaves and nothing else is touched.

### 7.5 `Ruleset::build`

One new rule beside the existing kind/variant check (`host.rs:1058-1078`, §4):
`An Action entry may list a filter kind only when it is dual`. A typo'd
`On::Action(Filter::SettleMul(…))` still fails to load.

### 7.6 Kani model

* `Op::effective_card_at = 347`, `Op::set_effective_card_at = 348`,
  `Op::plan_claimed = 349`. Model: `effective_card: [NameId; MAX_FIELD]` plus
  one tag bit per slot, and `plan_claimed: bool` reset by the move arms.
* `arm_is_placed` becomes instance-bound:
  `w.field.iter().any(|f| f.uid == w.last_uid)` (was `field_n > 0`).
* `arm_placed_cards` already resolves each `FieldCard.card` through
  `intern::resolve_str(NameId(..))` (`rules/kani/src/host.rs:2605-2627`) and now
  resolves `effective_card` first, answering the row's own card when unset.
* `arm_plan_claim` sets the flag; `arm_plan_claimed` reads it.
* `Op::invoke_entry = 431` (the retired `invoke_skill` was 132, never reused;
  431-440 is this group's README band). G13 §5.5's `arm_invoke_entry` /
  `invoke_n` / `last_invoke` are adopted as-is plus the caller binding: the
  record extends to `(card, entry, paid, by_instance)` and `by_instance` must
  equal `last_uid` before the frame is pushed.
* Harness helper `place_instance(player, card) -> instance` writes `field[]` and
  `last_uid`; a pass harness sets `last_uid` to the offered instance and
  `plan.is_main_move`.

---

## 8. What does not change

Scope, stated so the review can reject it as a whole:

* **No `NetMessage` change.** Nothing is sent; no `act`, no instance field, no
  `skip_serializing_if` gymnastics, `RECORD_VERSION` untouched.
* **No `SkillAction` / view change.** A marked instance still shows what it
  shows; the offer list is still the skill button list and still correct.
* **No client, autopilot or bot-policy change.** A prompt is the generic prompt
  UI, and `answer_for_entry` (`policy.rs:640`) already answers every ask (the
  fallback — decline). A tuned weight for the two new offers is a `docs/BOT.md`
  follow-up, not part of this group.
* **No `TurnCtx.pressed`.** Without buttons nothing needs per-turn
  de-duplication; the claim of §5 is per move, not per turn.
* **No `On::Play` `.field()` bit.** A Play stays a hand play; the placed clause
  is a second entry.
* **No `Call::Press`, no `CardRules::press*`, no `entry::PRESS` change.**
  `entry::PRESS` keeps meaning "the first Play", which is what `invoke_skill`
  and `query_guard` already resolve.

---

## 9. Card usage before / after

### 9.1 `nanami-press-invokes-clause-3` (rows 734, 735, 737)

Before: one entry, an origin branch, the wrong target.

```rust
On::Play("", Some(cant_play), play),
// …
fn play(player_id) { if ctx::is_placed() { use_skill(player_id) } else { nanami_effort(player_id) } }
// gate: if is_placed { crystals, owner_skill, marks } else { token sum }
//   -> play_card(skill) -> the skill's On::Play = clause (3)
```

After — the placement stays the hand Play, and (3)'s 「你可以」 becomes the offer
it is printed as:

```rust
On::Play("", Some(cant_play), nanami_effort),

fn cant_play(player_id: i32) -> Option<Msg> {
    let names = ctx::held_names(player_id, TOKEN_PREFIX);
    if names.iter().any(|n| ctx::held_count(player_id, n.as_str()) > 0) {
        return None;
    }
    Some(Msg::new(key!("nanami_effort_no_tok")))
}

// in (3), after the crystals are placed on the new instance:
let Some(id) = owner_skill(player_id) else { return Ok(()) };
// 规则书（3）: 「你可以移除一个奇迹水晶视为发动你的（2）技能」 -- asked, not
// assumed, and asked before the crystal goes so a misoffer cannot burn it.
if bandori_std::fire::total(player_id) < 1 {
    return Ok(());                       // (2) could not pay; skip silently
}
if ctx::ask_yes(
    player_id,
    &Msg::new(key!("nanami_effort_ask_title")),
    &Msg::new(key!("nanami_effort_ask_text")),
)? {
    ctx::add_crystals(-1, 0)?;
    ctx::invoke_entry(player_id, id.as_str(), "2")?; // body only
}
```

The old `cant_press` disappears with the press: the offer's own conditions are
the two inline checks. The order is deliberate — a pot and a skill must exist
before the question is asked, so the answer is never a dead end, and the crystal
leaves only after a yes (row 734's burn).

nanami's skill gains `.id("2")` on its `Action::CardUsed` entry
(`skill-personas/src/nanami.rs:58`); the label 「（2）」 stays. New locale keys
`nanami_effort_ask_title` / `nanami_effort_ask_text` (zh/en);
`nanami_effort_no_pot` / `_no_skill` / `_no_crystal` stay for the hand path and
the inline skips. Both TODOs at `nanami_effort.rs:274-281` close, and the
comment "`play_card` runs its (3) body" is rewritten to "runs the invoked (2)
body".

`fire::total` (pool + temporary pots) is the read, not `fire::get`: `spend_skill`
draws temporary pots first, so a temporary pot alone pays (2).

### 9.2 `kanon-press-surface-abi` (rows 607, 608, 610-612, 619-621)

The stopgap (ship if the group slips): delete the `skill:` test in
`view_extra_parts` and derive `source` defensively. The card then appears and
its gate is reached. Known cost: it is a button with no phase, and hand-only
cards appear too.

Under G05 the [场] placement stays the Play and the jump becomes the offer the
text prints:

```rust
On::Play("", Some(cant_place), place),           // hand play: placement only
On::Action(Action::DiceRollPlan(
    "card.placed && actor == owner && move.is_main_move",
    Some(can_jump),
    jump,
)),
On::Action(Action::MoveAfter(
    "actor == owner && move.kind != Teleport && move.dir < 0",
    None,
    on_pass,
)),

fn can_jump(player_id: i32) -> bool {
    // 「若此卡上有至少9个水母标记」; a claim by another card stands this one down.
    ctx::held_count(player_id, JELLY) >= 9 && !ctx::plan::is_claimed()
}

fn place(player_id: i32) -> Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("kanon_march_note")));
    ctx::log(player_id, &Msg::new(key!("kanon_march_placed")).player_id("who", player_id));
    ctx::add_held_by_name(player_id, JELLY, 0, 9)
}

/// 「主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆
/// 或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆」.
fn jump(player_id: i32) -> Asked {
    let a = ctx::tile_named("水族馆");
    let b = ctx::tile_named("弦卷豪宅");
    let a = if a >= 0 { a } else { 3 };
    let b = if b >= 0 { b } else { 29 };
    let pool: Vec<i32> = [a, b].into_iter().filter(|t| *t >= 0).collect();
    if pool.is_empty() {
        return Ok(());
    }
    ctx::plan::claim();
    let pick = ctx::ask_tile(
        player_id,
        &Msg::new(key!("kanon_march_title")),
        &Msg::new(key!("kanon_march_where")),
        &pool,
    )?;
    ctx::set_held(player_id, JELLY, 0, 0, -1);        // 「清除所有标记」
    // 「视为本次主要移动」 -- this *is* that move: the plan becomes a teleport and
    // main_move's teleport branch runs it, no die, landing settles.
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(pick);
    ctx::plan::set_settles(true);
    ctx::log(player_id, &Msg::new(key!("kanon_march_jump")).tile("tile", pick));
    ctx::set_dest(ctx::Dest::Graveyard);              // 「然后此卡置入弃牌堆」
    Ok(())
}
```

`run_planned_move` leaves the body: the plan pass is already inside the move
(§7.3). `cant_place` keeps its old shape with the marker/press arm removed
(「[场]」 placement is unconditional; `place` keeps the 9-cap).

A teleport is legal while [停留]/[晕眩] (rules.txt 47 「无法使用非[传送]的移动
效果」), so the jump needs no movement-gate guard — only the main move's
availability, which the anchor itself guarantees (`main_move` returns before the
pass when the move is spent, `play.rs:1565-1576`). A hand copy in hand no longer
matters at all: the offer is on the field, and the hand play is the placement.

### 9.3 `even-lost-not-pressable` (rows 798, 801)

Stopgap: the same filter drop. Under G05 the Play is dropped (the card is
placed by its [反击] body, never played) and the clause becomes the offer:

```rust
use bandori_std::{ctx, status};
use card_sdk::abi::roll_source;

&[
    On::Counteract(
        &[ChainKind::Effect],
        "by != owner && by >= 0 && effect.hits(owner)",
        None,
        counteract,
    ).counter_reason(key!("even_lost_when")),
    On::Action(Action::DiceRollPlan(
        "card.placed && actor == owner && move.is_main_move",
        Some(can_use),
        use_it,
    )),
];

fn can_use(player_id: i32) -> bool {
    // 「主要阶段中」 is the anchor; what can still shut the move is the movement
    // gate ([停留] / [晕眩] / [除外]), and another offer's claim.
    if ctx::plan::is_claimed() || ctx::fixed_roll().is_some() { return false; }
    if ctx::hand_size(player_id) <= 0 { return false; }
    status::stun::get(player_id) == 0
        && (status::stay::get(player_id) == 0 || status::unstoppable::get(player_id) > 0)
        && status::exile::get(player_id) == 0
}

fn use_it(player_id: i32) -> Asked {
    let n = ctx::hand_size(player_id);
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("even_lost_title")),
        &Msg::new(key!("even_lost_ask")).i("n", n as i64),
    )? {
        return Ok(());
    }
    ctx::plan::claim();
    // 「将此卡置入弃牌堆」 -- this instance, through the run's bound uid.
    ctx::set_dest(ctx::Dest::Graveyard);
    // 「使你的此次主要移动格数为你当前手牌张数」 -- the face *is* the hand size
    // (no die is thrown), and the step count is pinned to it so a 掷骰值 modifier
    // cannot change the printed 格数.
    ctx::set_fixed_roll(n, roll_source::CARD);
    ctx::plan::set_steps(n);
    ctx::log(player_id, &Msg::new(key!("even_lost_go")).i("n", n as i64));
    Ok(())
}
```

The move gate is spelled with the same three status hosts the engine's own clamp
uses (`stay.rs:55`, `stun.rs:54`, `exile.rs:72`; `unstoppable` exempts stay only,
`rules.txt` 51). The `fixed_roll` half of the guard mirrors eve's
(`eve.rs:60-65`): another card that already pinned the face wins.
`hand_size <= 0` is refused because a 0-step move discards the card for nothing.

No Play entry is left. `ctx::play_card("MyGO:即使迷茫着")` from a card that
wants a forced play would find none (`DEST_UNSET` no-op); nothing calls it, and
the counteract body owns the placement. The TODO at `even_lost.rs:18-19`
closes.

### 9.4 `placed-card-press-surface` (row 1614)

Before: the prefix filter, `has_play`, an id-derived source, and both client
policies on the same list.

After: nothing in the list changes (§8) — the list stays the skill buttons — and
the four cards reach the player through their own anchors. A hand-only card
(`anon_tokyo`, `haneoka`, …) gains no entry point because it needs none; its
`[手]` body cannot be replayed from the field. The capability the row asked for
is §4 (a stage that can ask) plus §5 (one offer per move).

### 9.5 `hina-clause-2-swap-missing` (rows 930-932, 936, 972)

「本卡」 is the stand-in, banished once its substitute lands, so the clause
attaches to the substitute and never fires again. It is **passive**: it is
resolved at the moment the substitute lands, and thereafter it is a property of
the instance — the effective-card marker (§6.2). Nothing is added to any list;
what changes is *which card the instance runs*, so a later hook, play or
invocation of that instance runs the stand-in's effect, not the substitute's
own.

Before (`hina_sound.rs:17-20`): one entry, no marker, no surface, and the
`[共鸣]` fuel of (2) never offered (row 972).

After:

```rust
use bandori_std::counter;
const ID: &str = "PP:[冰川日菜]会发出怎样的声音呢？";

On::Play("", None, hina_sound),

fn hina_sound(player_id: i32) -> Asked {
    // …the existing pick/ask/place body…
    let before = instance_floor(player_id);
    let Some(instance) = new_instance_since(player_id, &before, sub) else { return Ok(()) };
    // 规则书（2）「[共鸣]」: the change costs the fuel, and is passive -- asked
    // here, at the only moment 「本卡在[场地]」 can hold, and applied on yes.
    if crate::resonance::try_resonance(player_id)? {
        ctx::set_effective_card_at(instance, ID);
        ctx::set_created_by_at(instance, -1);   // G08; TODO until it lands
        // The changed effect itself (OPEN-substitution-effect): re-pick the
        // substitute, carrying the crystals across.
        hina_change(player_id, instance)?;
    }
    ctx::set_dest(ctx::Dest::Banished);
    Ok(())
}

/// The changed effect. The marked instance's own card is the current
/// substitute; `effective_card_at` is the stand-in, so it cannot answer.
fn hina_change(player_id: i32, instance: i32) -> Asked {
    let crystals = ctx::counter_at(instance, counter::CRYSTALS);
    let Some(cur) = own_card_at(player_id, instance) else { return Ok(()) };
    let others: Vec<&str> = SUBS.iter().copied().filter(|s| *s != cur).collect();
    let pick = ctx::ask_card(
        player_id,
        &Msg::new(key!("hina_sound_title")),
        &Msg::new(key!("hina_sound_swap_ask")),
        &others,
    )?;
    let sub = others[pick];
    let as_msg = Msg::new(key!("hina_sound_as")).card("card", ID).card("sub", sub);
    ctx::log(player_id, &as_msg);
    ctx::unplace_at(instance);
    let before = instance_floor(player_id);
    if sub == MAMI {
        ctx::place_card(player_id, sub, &as_msg);    // 大和麻弥 has no [手]
    } else {
        ctx::play_card(sub, player_id)?;
    }
    let Some(next) = new_instance_since(player_id, &before, sub) else { return Ok(()) };
    ctx::set_effective_card_at(next, ID);
    ctx::add_counter_at(next, counter::CRYSTALS, crystals, 0);
    Ok(())
}
```

* `instance_floor(player) -> Vec<i32>` is the player's current field instance
  ids and `new_instance_since(player, &floor, card) -> Option<i32>` the first
  instance of `card` not in the floor. The play and place imports do not hand
  back an instance (`play_card` answers `Dest`), and one helper keeps both arms
  honest. `own_card_at(player, instance) -> Option<String>` reads the instance's
  own card from `ctx::field_instances(player)`. All three are free functions in
  this card file over existing imports; none is new ABI. The floor is taken
  before the placement, so `HINA_SOUND`'s own instance — still on the field while
  its body runs — is excluded.
* The body needs no origin test: the Play is always (1), and `hina_change`
  is called from it.
* `[共鸣]` spends through `resonance::try_resonance` (the fuel card, its ask and
  its two crystals). It is offered once, when the substitute lands — the same
  shape 千圣's (2) uses on its turn-start offer — so a decline leaves the
  substitute running its own effect, and no [共鸣] is asked again until the
  effect is used again.
* The crystals are read as a plain counter and re-added at the new instance —
  the same read/write pair `card-mujica/src/crystal_swap.rs:49,56` uses. They
  are the substitute's own (its `[手]` filler), so they travel with the change.
* `set_dest(Banished)` at the body's tail is correct in both arms: the tail
  applies to the run's `current_uid` — the stand-in's own instance — and
  `hina_change` has already unplaced it, so the substitute it just placed stays.
  §6.2's `apply_dest` fix is what keeps the *banished* card the stand-in and not
  the substitute it names as its effective card.
* New locale keys `hina_sound_swap_ask` (zh/en); `hina_sound_title` is reused.
  Row 930/932's fourth substitute stays reachable — both arms map it to
  `place_card`. A `TODO(规则书)（此卡在符合使用条件时可替代…）` stays on the ask:
  the per-substitute eligibility filter is still unimplemented.
* The content of 「改变…效果」 is unprinted (pp.md `OPEN-substitution-effect`).
  The re-pick above is the interim reading; §14 records it.

## 10. Save, record, replay

* **Save:** `FieldCard.effective_card` and `TurnCtx.plan.claimed` are
  `#[serde(default)]`; **SAVE_VERSION 15 -> 16** (`engine/mod.rs:398`; eighth
  SAVE bump in the README landing order — take the next free one if the order
  moves).
* **Record:** nothing on the wire changes. `RECORD_VERSION` stays 4.
* **Replay:** an old record of a played nanami replays into the new body and
  meets a prompt the old run answered implicitly; the answer is in the event
  log, so the replay answers it identically instead of re-deriving it. A record
  of a *placed* nanami/kanon/even-lost now has effects where it had none — that
  difference is the rulebook fix itself and is accepted.
* **Event log:** no new `MatchEvent.kind`. The offers log through their existing
  `ctx::log` lines and the generic prompt entries.
* **Bots:** nothing to save; the claim is per move, so no restored seat can loop.

---

## 11. Versioning

**ABI_VERSION 110 -> 111** (eighth group in the README landing order; take the
next free number if the order moves). Wire break: the removed guest declaration
of `invoke_skill` (a name-keyed import; its host alias keeps pre-v111 wasm
linking). New imports: G13's, `effective_card_at` / `set_effective_card_at` /
`plan_claimed` / `plan_claim`. **No engine kind id is consumed** — the new
`ActionKind::DiceRollPlan` is a guest-side spelling over engine kind 67 — so the
README's kind column stays `—`. Changelog block, after the
`pub const ABI_VERSION` line, in the v104 shape:

```rust
/// v111: **offers from placed cards**. A clause a player may take on a placed
///    card is an `On::Action` hook that raises `ask_yes` at the event the
///    clause names; the movement offers ask at the plan pass, which gains its
///    action side (`ActionKind::DiceRollPlan` / `Action::DiceRollPlan`, engine
///    kind 67, dual with `FilterKind::DiceRollPlan`). `TURN.plan.claimed` and
///    `plan_claim` / `plan_claimed` let one offer take a move and stand the
///    others down. New imports `effective_card_at` / `set_effective_card_at`
///    name the card a stand-in's instance runs (dispatch, play and
///    invocation resolve it). Shared with G13: `invoke_entry` /
///    `invoke_frame`, guest `ctx::invoke_entry` / `invoke_entry_paid` /
///    `invoke`; the guest declaration of `invoke_skill` collapses onto them
///    (`ruleset_invoke_skill` stays a host alias; `invoke_skill` is sugar over
///    `invoke_entry(…, entry::PRESS)`; `play_card` keeps its import).
///    **SAVE_VERSION 15 -> 16** (`FieldCard.effective_card`,
///    `TurnCtx.plan.claimed`; both `serde(default)`). RECORD_VERSION unchanged.
```

`docs/ACTIVATION.md` needs **no** G05 rewrite: `entry::PRESS` keeps its meaning
and its reservation; the reading note stays about skills. `docs/BOT.md`'s
"Placed-card presses (G05)" paragraph becomes an entry about the offers and the
fallback answer. `docs/ENGINE.md:543-546` likewise. `README.md`'s G05 title
becomes "Offers from placed cards", the G08 Op band shifts one to 350, and the
consolidated-map entry keeps the mis-filing note.

---

## 12. Tests

Ruleset tests (`rules/tests/tests/common/mod.rs` gains `Table::pass_plan`
helpers — drive a main move up to the plan raise and answer the offer):

* 1 (nanami): `tests/morfonica.rs::nanami_third_prompts_and_invokes_skill_two`
  (answer yes: crystal removed, pot spent, mark gained, CD set, card stays
  placed, the invoked frame pushed and popped);
  `…_third_prompt_declined_keeps_the_crystal`; `…_third_press_waives_cap_and_cd`
  (CD 3 preset, mark already held, 3+ alive; still gains);
  `…_third_without_pot_skips_the_prompt` (no prompt is raised);
  `…_third_without_crystal_skips_the_prompt`; `…_third_uses_the_second_copy`
  (the crystal and the mark belong to the newly placed instance).
* 2 (kanon): `tests/hhw.rs::kanon_jump_offered_before_the_dice`;
  `…_jump_not_offered_below_nine`; `…_jump_declined_rolls_normally`;
  `…_jump_teleports_and_settles_the_destination`; `…_jump_clears_the_pool`;
  `…_jump_discards_the_card`; `…_jump_offered_while_stayed` (a teleport is legal
  under the movement gate); `…_hand_play_places_and_offers_nothing`;
  `…_second_copy_not_offered_after_a_claim`.
* 3 (even-lost): `tests/mygo.rs::even_lost_offered_before_the_dice`;
  `…_yes_discards_and_sets_the_face_to_the_hand`;
  `…_no_die_is_thrown` (no cast in the roll log);
  `…_declined_rolls_normally`; `…_not_offered_while_stayed`;
  `…_not_offered_after_the_main_move`; `…_empty_hand_not_offered`;
  `…_second_copy_after_a_claim_not_offered`.
* 4 (surface): `crates/game-core/tests/plan_hook.rs::action_at_a_filter_kind_runs`,
  `…_action_runs_before_the_filter_fold`, `…_decline_writes_nothing`,
  `…_ask_replays_once` (a write before the ask lands once);
  `tests/engine.rs::claim_survives_the_resume`, `…_claim_cleared_after_the_move`;
  `tests/basic.rs::hand_only_card_not_offered` (a card with no hook offers
  nothing).
* 5 (hina): `tests/pp.rs::hina_swap_marks_the_substitute_instance`,
  `…_swap_declined_leaves_no_marker`, `…_swap_carries_the_crystals`,
  `…_marked_instance_runs_the_stand_in_surface` (a later hook on the substitute
  dispatch to `HINA_SOUND`), `…_unmarked_substitute_runs_its_own_effect`,
  `…_banish_files_the_stand_in`.
* ABI/ruleset: `crates/game-rules/tests/ruleset.rs::action_at_a_filter_only_kind_rejected`
  (a `Filter::SettleMul` on an Action still fails to load),
  `…_action_at_the_dual_kind_loads`;
  `crates/rules-native/tests/abi_link_names.rs` covers the three new ops.
* Record: `tests/record.rs::record_without_offers_body_check_unchanged` (golden
  hash — nothing on the wire moved).
* Invocation is G13's surface; G05 adds the caller binding only:
  `tests/declare_arg.rs::invoke_frame_by_instance_is_the_caller_instance` and
  `…_invoke_from_plain_card_and_skill`.

Kani obligations (`rules/kani`):

* `Op::effective_card_at` / `arm_effective_card_at`, `Op::set_effective_card_at`
  / `arm_set_effective_card_at` (347/348) with the slot array and tag bits;
  `Op::plan_claimed` / `arm_plan_claimed` (349); `arm_is_placed`
  instance-bound; `place_instance` helper.
* `proofs/mor.rs`: `nanami_third_yes_iff_at` — post = crystal -1, invoke
  recorded at the owner skill with `by_instance` = the placed instance and
  `paid = 0`, exactly one effect log, frame unchanged;
  `…_third_no_crystal_noop_at`; `…_third_no_pot_noop_at`;
  `…_third_binds_second_instance`.
* `proofs/hhw.rs`: `kanon_jump_at_nine_iff` (pool cleared, teleport plan, dest
  graveyard), `…_jump_below_nine_noop`, `…_jump_claimed_move_noop`.
* `proofs/mygo.rs`: `even_lost_yes_sets_face_and_steps_iff` (face = hand size,
  steps = hand size, dest graveyard, claim set),
  `…_blocked_move_noop` ([停留] preset), `…_claimed_move_noop`.
* `proofs/pp.rs`: `hina_mark_iff` — post = exactly one new instance with
  `effective_card = "PP:[冰川日菜]会发出怎样的声音呢？"` and the carried crystal
  count, the fuel row gone, a second marked copy unchanged;
  `…_mark_without_resonance_noop` (world snapshot equal).
* Invariant: `placed_instance_answers_with_its_effective_card` — for two field
  instances with and without a marker, each resolves its own surface.

## 13. Alternatives

**A1 — filter drop only (no ABI).** Delete the `skill:` test and derive
`source`. Cheapest; satisfies items 2 and 3's stated expectation, which is why
they carry a stopgap. Fails item 4 (the offers are buttons with no phase, and
every `[手]` card replays from the field), gives no claim, no invocation and no
marker.

**A2 — a card-wide `prop::FIELD_PRESS` data prop.** The bit lives on the card
rather than the entry. It only makes sense alongside A1; with the offers it is
redundant, because the offer is an entry.

**A3 — a new `OnKind::Press = 15`.** The moved clause gets its own entry type
with an id and a gate, so a card can keep the hand Play and a button beside it;
kanon and nanami keep two buttons. What it buys is direct addressing of the
placed clause; what it costs is a kind, a `PressEntry` struct, a `press_*`
family on `CardRules` and `Ruleset`, an `entry` on the wire, and a press id on
every card — for clauses that turn out to be offers or passives. Re-open if a
card prints a genuinely player-timed action (a 「可随时」 without a triggering
event).

**A4 — a raise-based press.** `ctx::raise` a new `cardPressed` kind and let a
hook run the body; the client sends only the card id. A raise has no return
`Dest` and no declare barrier, so the discard of even-lost/kanon would need
`set_dest` at the tail anyway (it has it), but the button still lacks a phase.

**A5 — declare (2) on each substitute card instead of a marker.** The stand-in's
`[共鸣]` clause would live on 丸山彩, 大和麻弥, 白鹭千圣 and 若宫伊芙, found by
graphing the stand-in. Saves §6.2 and its two imports, but the substitute is a
shared card: its (2) would apply on every copy, not only where it stands in for
日菜, so the clause needs a condition that re-derives "do I stand in" — the
origin guess again, one card further away. 大和麻弥 makes it worse: its own
`On::Play` is already its `[共鸣]` pile-swap, so a second `[共鸣]` clause lands
beside a different one on the same card. It also cannot move the crystals of
「本卡」 to the right copy, which is the instance half. Kept only if the marker
is judged too much state.

**A6 — inline (2)'s body in the effort card.** No invocation at all for item 1:
the effort body copies the skill's clause. Cheapest for nanami alone. But
「视为发动你的（2）技能」 names the skill's clause, so the copy drifts on the
skill's next erratum, and the gate must be hand-written from the same terms
anyway. The other four items still need their surfaces.

**A7 — a new engine kind `diceRollPlanBefore` instead of a dual `ActionKind`.**
Cleaner to validate (no dual concept) but wastes an id and duplicates a raise
the engine already performs, with a second payload struct and a second
`is_filter` answer to document. The dual spelling reuses kind 67 and its plan
semantics exactly.

---

## 14. Open questions

1. **C-Q46 / 「不受数量或轮数限制」.** `invoke_entry` never evaluates the
   target's gate, so the waiver is structural rather than a flag. It does not
   waive the *cost*: (2) spends a pot inside its body, so the offer checks a pot
   first (§9.1). Confirm the waiver covers the 「每种角色标记上限1」 cap and the
   3-turn CD (assumed yes).
2. **Is (3)'s crystal offer one-shot?** Read as printed — asked as part of
   resolving (3), never again. If it is re-usable later, it needs a hook on the
   placed card instead of an inline prompt.
3. **Kanon's offer breadth.** Anchored at the plan pass, so a marker gained
   later in the same main move is not offered until the next turn's move.
   Confirm that is the intended reading of 「主要阶段中」, or whether the marker
   hook must also re-offer (which would need the move to be interruptible).
4. **Declining twice a turn.** Nothing re-offers after a decline; the main move
   is one-shot, so the offer arrives once per move. Confirm the sheet means one
   question per move rather than a standing permission throughout the phase.
5. **Nothing negates an offer.** An offer is a hook, so a [反击] answers what
   its body does, not the offer. Is that right for the jump and the discard?
6. **Opponent presses.** Every offer is raised to the instance's holder. No text
   today lets another seat take one; if one appears, the hook seat becomes the
   trigger's seat rather than the row's holder.
7. **`send_to_dest` on an offer.** It unplaces at once and files the run's card
   id; §6.2 makes that the instance's own card. An immediate discard and a
   deferred `set_dest` therefore differ only in timing. Confirm no clause needs
   the immediate form.
8. **`Dest::Field` in an offer.** Documented no-op (already placed); a re-place
   offer must `unplace_at` first. Confirm no clause wants a "return to field"
   offer.
9. **`entry::PRESS` with a multi-clause card** — unchanged from today: the first
   Play. With G13 available, should `PRESS` keep resolving to (3) on nanami's
   skill while (2) is the intended entry, or should every skill's activation
   clause carry an explicit id and `PRESS` retire? Assumed unchanged (narrow
   scope).
10. **Effective-card scope beyond dispatch.** §6.2 resolves it for dispatch,
    play, invocation and dest. Should a body's `query_guard` reads resolve it
    too? Assumed no (bodies address by slug).
11. **Temporary pots.** `fire::total` admits a payment by a temporary pot alone.
    Confirm that is intended (`spend_skill` draws temp first, so it is payable)
    rather than "a held pot".
12. **OPEN-substitution-effect.** What 「改变本卡代替的专属卡效果」 changes the
    effect *to* is unprinted (pp.md). The re-pick in §9.5 is the interim
    reading; when the text arrives it replaces `hina_change`'s body, and possibly
    needs entries of its own.
