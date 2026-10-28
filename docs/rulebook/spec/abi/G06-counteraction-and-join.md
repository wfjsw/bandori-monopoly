# ABI design G06 — Counteraction and join windows

**Status:** draft for user review · design only, no source touched.
**Worktree:** `.claude/worktrees/card-rulebook-review` (branch `worktree-card-rulebook-review`).
**Group:** G06, family "Counteraction and join windows".
**Ground truth:** `docs/rulebook/cards.json`; every fragment below is verbatim from it.

Terminology: **counteraction** = the [反击] mechanism (chain / window / link).
**Hook** = a passive listener ([持续] / [特] / `On::Action` / `On::Filter`).
Never "reaction".

**Closed slugs and rows:** all fifteen items are covered (eleven from the first draft, four consolidated sub-slugs the draft missed).

| # | slug | severity | rows |
|---|---|---|---|
| 1 | `net-error-join-draw-ownership` | medium | 942 |
| 2 | `net-error-join-used-getter` | low | 1312 |
| 3 | `extreme-range-filter-missing` | medium | 158, 982, 990 |
| 4 | `event-effects-not-counterable` | medium | 980 |
| 5 | `vocal-split-leg` | medium | 21 |
| 6 | `mortis-empty-counteract-links` | medium | 313, 1045 |
| 7 | `repaint-counter-link-roll` | medium | 838 |
| 8 | `cla-counterer-set` | high | 426, 1060 |
| 9 | `mana-nested-window` | medium | 845, 1090 |
| 10 | `mana-joiner-count` | medium | 844, 1094 |
| 11 | `nowsumimi-cancelled-guard` | medium | 936, 940 |
| 12 | `rain-designates-prop` | medium | 1510 |
| 13 | `ordinary-effect-window-not-post-landing` | medium | 395 |
| 14 | `own-stage-immuneall-ordering` | medium | 1527, 1643 |
| 15 | `mortis-copy-effect-unimplemented` | medium | 312, 315 |

Item 4 is **demoted to A** (fixable with the current ABI, no new surface).
Item 6 is **half A, half B**: the empty-link matcher rule is A, the copy that
would fill the link is B (1.15). Everything else needs B-class capability.

---

## 1. Problem, per item

### 1.1 `net-error-join-draw-ownership` (row 942)

Book (`Sumimi:（真奈）歌唱大赛5连冠`):
「若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡」

The draw belongs to the **join opener's** effect: the patient is "the player
who played that [反击] card", chosen by the opener's clause, not by the joiner's
own card. Today it sits in `net_error.rs:145-151`, inside the joiner's optional
`ask_yes`, with the TODO at `net_error.rs:142-144` naming the move. Two
consequences:

* any other [反击] card that joins cannot draw at all (only 网络链接异常 can),
* the opener (`mana_champion`) cannot see **who** joined or whether they
  actually shielded it, so it cannot run the clause from its own body.

### 1.2 `net-error-join-used-getter` (row 1312)

`ctx::set_join_used()` (`ctx.rs:2859`) writes `Trigger.join_used` through
`ruleset_set_join_used`; there is **no** reader — neither `ctx::join_used()` nor
a host import. The write lands on the *answered* link
(`wasm_rules.rs:2288-2290` sets `self.trigger.join_used`; `resolve_rounds`
writes `on_link` back to `chain[answered].link`), which is exactly where the
opener could read it — but no export exists, so the only reader is a
substitution: `mana_champion.rs:94` reads `ctx::designation_cancelled(player_id)`,
which is a *different* fact (did a designation get cancelled) and is true for
reasons unrelated to the join.

### 1.3 `extreme-range-filter-missing` (rows 158, 982, 990)

Book: 「使任意结果只有数字区间的效果以理论最大值或最小值结算，除@Tsugu ycm
以外无法直接改变骰子点数（ex：可以对YOLO生效但无法对无论是何种颜色的夕阳
生效）」

`can_counteract` (`tsugumi_can.rs:46-61`) returns `true` for **every** `card`
trigger. Two missing predicates:

1. **numeric shape** — only effects whose result is a pure number interval
   (YOLO's `1d4`, ycm's `3d10`) qualify; a die-face *table* (夕阳's `1d6`
   branching) does not. Nothing on the link or the entry says which a played
   card is.
2. **not already armed** — C# `t.Play.Extreme == 0`: a second copy must not
   re-arm an extreme. `turn.extreme` is only written at *resolution*
   (`set_extreme`), and all offers for one window are collected before any
   body runs, so a second offer never sees the first.

The extreme itself also has the wrong lifetime for events (see 1.4) and the
`@Tsugu ycm` exception is unreachable: the filter cannot name the exception
because it cannot name the class.

### 1.4 `event-effects-not-counterable` (row 980) — **demote to A**

Book: 「任意结果只有数字区间的效果」 has no source restriction, so an *event*
that rolls a number interval is a legal target. `tsugumi_can.rs:22` declares
only `ChainKind::Card`, and `draw_event` (`play.rs:4943-5010`) never scopes
`extreme`: the arm/restore pair exists only in `play_from_hand`
(`play.rs:4803-4804`, `4909`). An event-armed extreme therefore leaks into the
next play, and an event counteraction is never offered.

Both halves are A: `ChainKind::Event` is an existing constant, and
`turn.extreme` is existing saved state.

### 1.5 `vocal-split-leg` (row 21)

Book: 「一次性向其他玩家支付至少5000资金时，免除此次支付。」

「一次性」 is one transfer, excluding a 「[分摊]」 share. Every split leg raises
its own `effect` window with the share as `value` and `Pay::total_stage =
false` (`play.rs:285`, `316`; the raise sits in `money_inner`,
`play.rs:3445-3464`), and the pay trigger carries no `total_stage`. So
`vocal_too_hard` (`vocal_too_hard.rs:24`) matches a 600 share of a 6000 total
and waives one leg of a single payment. The TODO is at
`vocal_too_hard.rs:20-21`.

### 1.6 `mortis-empty-counteract-links` (rows 313, 1045) — **demote to A (links half)**

Book: 「（此卡复制[反击]卡时可在符合条件时打出）」

`mortis_instinct.rs:22` declares `On::Counteract(&[], …)`. The host builds
`counteract_mask` from `entry.triggers` (`host.rs:1106-1110`), so an empty list
sets no bit, `counteracts_to` is `false` at every kind, and the entry is never
offered — `lib.rs:678` documents "An empty list is never dispatched". The
README's convention is fine for `Message` (empty = never) but wrong here: a
copy's kinds are the *copied* card's, which is dynamic.

The dispatch half is A (one rule in the matcher: empty = every chain kind).
The `can_counteract` half (`mortis_instinct.rs:37-44` returns `false`) still
needs the copy machinery (CH-05, B).

### 1.7 `repaint-counter-link-roll` (row 838)

Book: 「当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡」

A [反击] may itself be answered (「（此卡可以被反击）」 is a general shape,
CH-09). Repaint must be able to answer an inner counter on the *same* roll
(e.g. a YOLO that changed the face) so X counts the **original** path
(`repaint.rs:65-66`). Two things break it:

* a counter link is synthesized bare (`CoreTrigger::new("card"|"cardUsed",
  cursor)`, `wasm_rules.rs:6140-6155`) — no `mv`, so `move.roll != null` in the
  pre (`repaint.rs:21`) fails and the body early-returns at
  `repaint.rs:61-63`;
* the link's *kind* is `card`/`cardUsed`, never `DiceRollResult`, so a card
  declaring only `DiceRollResult` is not offered at all.

`bridge_trigger` (`wasm_rules.rs:7270-7330`) also drops the answered payload
and resets `join_redirect` / `join_used`, so nothing survives to the counter.

### 1.8 `cla-counterer-set` (row 426, 1060)

Book: 「使当前回合内对你打出过[反击]的所有玩家向你选择的**方向**强制移动
1~4以内的任意步数并[触发结算]」

Two defects:

* no per-turn counteraction history exists. `run`
  (`cant_look_away.rs:50-124`) falls back to `trigger::player_id()`: on the
  counter path that is the immediate counterer (one seat), on the hand-play
  path it is the actor, so `counter_user == player_id` and the body returns at
  `:73-75` and moves **nobody**. `ctx::turn_key()` names the turn but cannot
  rebuild the list (BACKLOG ABI-03 / CH-05).
* the same `run` is shared by the `On::Play` and `On::Counteract` entries
  (`cant_look_away.rs:17`, `:26-30`) with no way to say "the target set is the
  history, not the actor", so the fix has to invent one.

The self-move clause (「若使用者在自身回合内选择了使用者自己…」) is unreachable
until the set exists (`cant_look_away.rs:68-71`).

### 1.9 `mana-nested-window` (rows 845, 1090)

Book: 「此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其
反击卡中针对打出玩家自身的效果改为你」

The engine has only half of this: a `SELF_REDIRECT` property and a
`join_redirect` stamp on the answered link (`world.rs:79-86`,
`wasm_rules.rs:6002-6038`, `6145-6166`). There is **no window**: no
participating-seat set, no nested round, no presentation of the joiner as the
target, no per-joiner state. The current join only works when the answered
effect happens to carry the joiner as its own `target` (a per-seat pay leg) —
with a single-target effect no bystander is ever offered, and `net_error`'s
Effect guard `trigger::target() == player_id` (`net_error.rs:79`) is what
decides. `mana_champion.rs:88-92` states the gap.

### 1.10 `mana-joiner-count` (rows 844, 1094)

Book: 「若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。」

The fallback needs "did **anyone** declare a [反击] during this effect", which
is not the same as "was a designation cancelled": a joiner who played a
[反击] that shielded nothing still bars the draw, and a designation cancelled
by a non-joiner does not. Today `mana_champion.rs:94` substitutes
`designation_cancelled` for both the fallback and the joiner's own draw.

### 1.11 `nowsumimi-cancelled-guard` (rows 936, 940)

Book: 「当…失去资金的总额即将超过你所在格子的[收费标价]时打出此卡」

A payment an earlier counter already negated (`set_cancelled`) still reads its
declared `value`, so `now_sumimi` (`now_sumimi.rs:30`) is offered — and gains —
for a payment that will never settle. Guards run at offer time against the
link clone, and declared answers are only folded into the link at *resolution*
(`resolve_rounds`), so there is no pending-negation signal to read. The TODO is
at `now_sumimi.rs:26-29`.

---

### 1.12 `rain-designates-prop` (row 1510)

Book: 「[反击] 一名其他玩家…时，可指定其一个卡上的[持续]…」（`rain::*`）

A joiner answers with a designation (the prop it names) and a later link / the
opener must read it. Today the designation is a prompt answer with no home: the
join link stores the card but no prop, and no getter reaches it from another
seat. Surface: the join link carries `designation: Option<(card, prop)>`, and
`join::designation(joiner)` reads it; the report of 1.10 carries it too.

### 1.13 `ordinary-effect-window-not-post-landing` (row 395)

Book: the `effect` window is for effects that already **landed**; the refusal
(immune, gate) is supposed to happen first.

Today the counteraction window opens while the abnormal is still being declared
(`wasm_rules.rs:5548-5595` raises before `ImmuneAll`), so a fully protected
player still spends `own_stage` on a move that never happens (the card's own
TODO) and every `effect` guard sees a move that may be refused. Surface: the
abnormal pipeline gains an explicit `landed` / `refused` phase — the raise
moves after the gate/immune checks, and a refused abnormal raises its own
hook-only `abnormalRefused` (no window). Coupling: G12 §5.1 owns the gate the
raise must move behind.

### 1.14 `own-stage-immuneall-ordering` (rows 1527, 1643)

The card half of 1.13: `R:选择自己的舞台` in front of `ImmuneAll` / `不可阻挡`.
Same surface as 1.13 (the raise runs after the immune verdict); the card needs
no new import, only the ordering. Its settle answer itself is designed in G03
§12.

### 1.15 `mortis-copy-effect-unimplemented` (rows 312, 315)

Book: 「（此卡复制[反击]卡时可在符合条件时打出）」 — the copied card's effect is
what the copy plays, not mortis's own empty body.

B half of 1.6. Surface: a counter link carries `copied: Option<card>`; the
matcher's `counteract_entry` for that link resolves the **source** card's entry
list, and the body runs with `trigger::link_copied()` naming the source. The
copy context itself (which card was copied, its memory) is the CH-05 question
and is read, not decided, here; the entry of 1.6's A fix (empty list = every
kind) stays the fallback for a link whose copy is unresolvable.

---

## 2. Shared root causes

1. **A counteraction entry declares only *kinds*.** Its contract with the link
   — which numeric shape, which payment shape, what it does to the link — is
   not part of the ABI, so the offer index can only ask "does the kind match".
   Every over-offer in this group (1.3, 1.5) is that hole; every under-offer
   (1.6, 1.7) is the same hole seen from the other side.
2. **A counter link is a synthetic stub.** It is built with `kind` + seat + card
   only; the answered link's payload (`mv`, `pay`, `effects`, `subject`,
   extension tag, join state) is dropped (`wasm_rules.rs:6140-6155`,
   `bridge_trigger:7328-7329`). An entry that answers a counter cannot see what
   it is answering, and a *later* counter cannot see what the counter answered.
3. **`[反击]`-window state is ad-hoc trigger fields.** `join_redirect` and
   `join_used` are two scalars on one link, written by one card and read by
   nobody, reset by the bridge, not saved, and with no per-seat granularity.
4. **Guards and bodies see different worlds.** Guards run at offer time on a
   link that carries no resolution state; bodies run at resolution. Anything
   "pending" (a declared negation, a declared extreme) is invisible exactly
   where the book needs it.
5. **The turn is the enemy of "all players this turn".** The engine records
   gains (`gains_this_turn`) but not counteractions, so a per-turn set has to be
   rebuilt from nothing.

---

## 3. Capability as an invariant

> **A counteraction entry declares the numeric range and the source class it
> answers; every declared counter is a full link that carries the answered
> payload forward; and a join is a first-class window with a participating-seat
> set, a per-seat declaration record, and its own nesting.**

Derived obligations:

* **Contract exactness.** An entry is offered **iff** its declared kinds, its
  numeric shape and its payment shape all admit the answered link. No card body
  is asked before the contract passes.
* **Payload carry-forward.** A counter link exposes everything the link it
  answers exposed (move, roll, pay shape, effect list, subject card, join), with
  the counter's own facts added (`card` = the counter, `by_card` = declarer,
  `answered` = parent), and `chain_depth` says how deep it is.
* **Join is a window, not a flag.** An opener's body can open a nested round
  restricted to a participating-seat set; each seat is offered as if it were the
  target; each seat carries its own *declared* and *used* flags; the nested
  round settles to completion before the opener's body continues; the opener
  reads the report.
* **History is turn state.** Every declaration appends `(declarer, countered,
  card)` once, before any body, and the pair list is readable for the rest of
  the turn and empty on the next.
* **Pending is visible to guards.** A guard can read how many declared,
  unresolved answers on this link declare that they negate, and whether an
  extreme is already armed.

---

## 4. Surface

Everything is generic mechanism: no display text, no player decisions inside
imports.

### 4.1 `card_sdk::abi` — entry contract

`CounteractEntry` (`lib.rs:570-601`) gains three fields and one defaulted
builder each. The positional `On::Counteract(&[…], pre, guard, body)` signature
is unchanged; the new fields default.

```rust
/// What numeric shape the answered effect must have.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub enum NumericShape {
    #[default]
    Any = 0,
    /// The answered link's *subject* card declares `prop::RANGE_CARD`.
    NumberInterval = 1,
}

/// What payment shape the answered link must have (`pay`/`paid` only).
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub enum AnswerPayShape {
    #[default]
    Any = 0,
    /// `Pay::total_stage` — one whole payment, never a 「[分摊]」 share.
    Whole = 1,
}

/// What the entry's body does to the link, for offer-time bookkeeping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub struct AnswerEffect(pub u8);

impl AnswerEffect {
    pub const NONE: Self = Self(0);
    /// The body may call `trigger::set_cancelled` / `negate_effect`.
    pub const NEGATES: Self = Self(1);
    /// The body may call `ctx::set_extreme`.
    pub const ARMS_EXTREME: Self = Self(2);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}
```

```rust
pub struct CounteractEntry {
    // …existing fields…
    pub numeric: crate::abi::NumericShape,     // NEW, index filter
    pub pay_shape: AnswerPayShape,             // NEW, index filter
    pub effect: crate::abi::AnswerEffect,      // NEW, pending readers only
}
```

Builders (sugar translates before the ABI — the builders write the fields at
the card table, nothing at runtime):

```rust
impl On {
    pub const fn answers_numbers(self, n: abi::NumericShape) -> Self;
    pub const fn answers_whole_payments(self) -> Self;
    pub const fn declares_negation(self) -> Self;   // |= NEGATES
    pub const fn arms_extreme(self) -> Self;        // |= ARMS_EXTREME
}
```

`prop::RANGE_CARD = "rangeCard"` — NEW card property, `1` = the card's result
is a pure number interval. Declared by `AG:Y.O.L.O`,
`通用:@Tsugu ycm` (the book's own exception) and any future interval card.
`AG:无论是何种颜色的夕阳` does **not** declare it (the book's own negative
example).

### 4.2 `ManifestOn` — wire fields

Four trailing fields, all `#[serde(default)]` (postcard is not
self-describing):

```rust
#[serde(default)] pub numeric: NumericShape,
#[serde(default)] pub pay_shape: AnswerPayShape,
#[serde(default)] pub answer_effect: AnswerEffect,
#[serde(default)] pub all_kinds: bool,   // entry declared with an empty links list
```

### 4.3 Trigger payload / link state

```rust
pub struct TriggerPay { pub is_rent: bool, pub split_leg: bool, .. }  // EXTENDED
// G02 lands first and changes this struct to
// `{ is_rent, rent_kind, shape, settle_tile }` (its §4.4). This group only
// appends `split_leg`, so the merge is append-only against G02's shape; take
// G02's struct as the base, not the four-field one on v104.
pub struct Trigger {
    // …
    /// The card whose effect this link *is*: its own id on a root link, the
    /// answered counter's id on a counter link (pathological root = None).
    pub subject_card: Option<String>,      // NEW, carried forward
    pub join: Option<Join>,                // NEW
    /// Answers declared on this link and not yet resolved (chain indices, in
    /// declaration order). Cleared on a new counter link. Feeds the pending
    /// readers in 5.2.
    pub answers_declared: Vec<u32>,        // NEW
}

pub struct Join {
    /// The `SELF_REDIRECT` declarer.
    pub opener: i32,
    /// Participating seats, seat order. Only these are offered in the
    /// nested round.
    pub seats: Vec<i32>,
    /// Per seat, parallel to `seats`: declared a counter in this window.
    pub declared: Vec<bool>,
    /// Per seat, parallel to `seats`: the counter landed and shielded.
    pub used: Vec<bool>,
    /// Has `open()` already built the round (suspend re-entry guard)?
    pub opened: bool,
}
```

`join_redirect: Option<i32>` and `join_used: bool` stay as the compatibility
view: `join_redirect` mirrors `join.opener`, `join_used` mirrors
`join.used.contains(true)`. `ctx::join_redirect_target()` / `ctx::set_join_used()`
keep working and become aliases.

### 4.4 Imports (`card_sdk::ctx`, re-exported through `bandori_std::ctx`)

```rust
pub mod join {
    /// The opener when this counter answers inside a join window.
    pub fn opener() -> Option<i32>;
    /// The participating seats of the window this link participates in.
    pub fn seats() -> Vec<i32>;
    /// This joiner's counter shields the opener: marks `used` and returns
    /// `true`. `false` (and no mark) outside a join window.
    pub fn shield() -> bool;
    /// Opener side: build and settle the nested join round on the answered
    /// link. Idempotent across a suspend. `Err(Prompt)` if it pauses.
    pub fn open() -> Result<(), Prompt>;
    /// Opener side, after `open()`: the round was built.
    pub fn opened() -> bool;
    /// Opener side: how many seats **declared** (bars the fallback).
    pub fn joiner_count() -> i32;
    /// Opener side: the i-th declarer, declaration order.
    pub fn joiner(i: i32) -> i32;
    /// Opener side: did that declarer's counter land and shield?
    pub fn joiner_shielded(i: i32) -> bool;
}
```

```rust
pub mod trigger {
    /// Declared, unresolved answers on this link whose entry declares
    /// `AnswerEffect::NEGATES`.
    pub fn pending_negation() -> i32;
    /// `turn.extreme != 0` on the current play, or any declared answer on
    /// this link declaring `ARMS_EXTREME`.
    pub fn extreme_armed() -> bool;
    /// See `Trigger::subject_card`.
    pub fn subject_card() -> Option<String>;
    /// 0 = the root link, n = n counters deep.
    pub fn chain_depth() -> i32;
    /// The kind of the link at depth `i` (`None` out of range).
    pub fn answered_kind(i: i32) -> Option<TriggerKind>;
}

/// Distinct players who declared a [反击] against `seat` this turn, in
/// declaration order. Empty on the next turn.
pub fn countered_by_this_turn(seat: i32) -> Vec<i32>;
```

### 4.5 Condition vocabulary (`crates/cond`, window vars)

`subject_range_card` (bool), `extreme_armed` (bool), `pending_negation` (int),
`pay_split_leg` (bool), `chain_depth` (int). Documented in `GUARDS.md` §4.2c
next to the other effect-window vars. Purely additive; no wire id.

### 4.6 State

```rust
// TurnCtx, game-core/src/engine/world.rs
/// [反击] declarations against each seat this turn, in order.
#[serde(default)]
pub countered_this_turn: Vec<CounterRecord>,

pub struct CounterRecord { pub declarer: i32, pub countered: i32, pub card: String }
```

### 4.7 Classification

| item | NEW | EXTENDED | WIRE-BREAK | NO-WIRE |
|---|---|---|---|---|
| 1 | `join::{open,opened,joiner_count,joiner,joiner_shielded,shield}` | `Trigger.join` | `ManifestOn` | |
| 2 | `join::shield` replaces `set_join_used` | `join_used` alias | | yes |
| 3 | `NumericShape`, `prop::RANGE_CARD`, `extreme_armed`, `pending_negation`, `AnswerEffect::ARMS_EXTREME` | `CounteractEntry` | `ManifestOn.numeric` | |
| 4 | — | card link list | | yes |
| 5 | `AnswerPayShape`, `pay_split_leg` | `TriggerPay` | `ManifestOn.pay_shape` | |
  (`PayShape` is G02's; this group's enum is `AnswerPayShape` so both can land.)
| 6 | `ManifestOn.all_kinds` | matcher | `ManifestOn.all_kinds` | |
| 7 | `subject_card`, `chain_depth`, `answered_kind` | counter-link build, `bridge_trigger` | | |
| 8 | `countered_by_this_turn`, `CounterRecord` | `TurnCtx` | | (save only) |
| 9 | `join::*` window | `Trigger.join`, `Priority` | `ManifestOn` (via 1) | |
| 10 | `join::joiner_count` | | | yes |
| 11 | `pending_negation` | `AnswerEffect::NEGATES` | `ManifestOn.answer_effect` | |

**The wire break is the four `ManifestOn` fields** (postcard, not
self-describing). The imports alone would not have needed a bump; the manifest
shape does.

---

## 5. Engine wiring

### 5.1 Contract filter (items 3, 5, 6)

* `Ruleset::build` (`host.rs:1010-1110`): an `OnKind::Counteract` entry with an
  empty `triggers` sets `manifest.all_kinds = true` and fills the card's
  `KindMask` with **every** chain kind instead of none.
* `counteracts_to_play` (`host.rs:2440`) gains a contract check after the kind
  check:

```rust
fn contract_admits(&self, card: i32, t: &Trigger) -> bool {
    let Some(entry) = self.counteract_entry(card, t) else { return false };
    let e = self.entries()[card][entry];
    match e.numeric {
        NumericShape::Any => {}
        NumericShape::NumberInterval => {
            let Some(subject) = t.subject_card.as_deref() else { return false };
            let Some(sidx) = self.card(subject) else { return false };
            if self.card_props(subject).get(prop::RANGE_CARD).copied() != Some(1) {
                return false;
            }
        }
    }
    match e.pay_shape {
        AnswerPayShape::Any => {}
        AnswerPayShape::Whole => {
            let Some(p) = t.pay else { return false };
            if p.split_leg { return false; }
        }
    }
    true
}
```

  `counteracts_to_play` ORs `counteracts_to(card, kind)` with the designates
  rule, then ANDs `contract_admits`. `counteract_entry` (`host.rs:2446`)
  returns `None` when the contract fails, so a wrong entry is never driven.
  `any_counter_candidate` / `declare_one` inherit both unchanged.

* `RangeCard` is read from the **subject** card, never from the counter: on
  `card`/`cardUsed` counter links `subject_card` is the answered counter's card
  (5.3), so tsugumi answering YOLO's counter sees YOLO's prop and arms YOLO's
  `1d4` — matching the book's own 「可以对YOLO生效」.
* `excluded`/`all_kinds` are independent: a `noCounteractTurn` lock still gates
  a hand counter (`can_counteract_now`, `wasm_rules.rs:7633`), so dice_cast's
  lock keeps working.

### 5.2 Pending readers (items 3, 11)

`pending_*` are computed from the chain at offer time, never from saved state:

```rust
fn pending_negation(&self, link: &Trigger) -> i32 {
    link.answers_declared.iter()
        .filter(|&&d| self.entry_effect(link, d).contains(AnswerEffect::NEGATES))
        .count() as i32
}
```

`declared` is the in-flight list of answers on the link (the same one
`resolve_rounds` walks); it is populated at declaration
(`wasm_rules.rs:6150-6156`), before any resolution. So a re-offer after a first
declaration sees it, and a guard can refuse. `extreme_armed` ORs
`turn.extreme != 0` with the same scan for `ARMS_EXTREME`; both are exposed as
imports and window vars.

This is **declared intent**, not outcome: the entry says "my body may negate".
The `AnswerEffect` bit is the entry's own contract and is checked by a test per
`set_cancelled` card (9.3).

### 5.3 Counter links carry the answered payload forward (item 7)

In `build_round`, replace the bare `CoreTrigger::new` with a copy of the
answered link, then override the counter's own facts:

```rust
let mut link = chain[timing].link.clone();   // mv / pay / effects / subject / join / tags
link.kind      = if uid >= 0 { "cardUsed" } else { "card" };
link.player_id = cursor;
link.by_card   = Some(cursor as i32);
link.card      = Some(id.clone());
link.seq       = (at + 1) as u32;
link.answers   = timing as u32;
link.target    = chain[timing].seat as i32;
link.subject_card = chain[timing].link.card.clone();
// The ROOT effect link needs the stamp too: `contract_admits` reads
// `t.subject_card` off the trigger it is deciding for, which on the first
// answer is the play's own link (`play.rs:4814` area, where `t.card` is
// set). Without it `NumericShape::NumberInterval` answers false for every
// root play and the extreme counter is still never offered.
// the new counter has its own pending set
link.answers_declared.clear();
link.negation = Negation::None;
link.spared.clear();
```

`answered_kind(i)` walks `answers` links. `COUNTERACT_MOVE_EXTENSION` now rides
forward with `mv.tags`, so an inner counter sees the outer extension; the
`retain` at `wasm_rules.rs:6389-6392` already strips it per body.

`bridge_trigger` (`:7328-7329`) must keep `subject_card` and rebuild `join`
rather than reset both to `None`/`false`.

### 5.4 Join window (items 1, 2, 9, 10)

**Seat set.** `Join::seats` = every live, on-board seat except the opener, in
seat order (C# `H.Others(i)`; the book's 「场上其他玩家」). Captured at
`open()`, not at declaration, so a seat that leaves mid-window is dropped by
`can_counteract_now`.

**Opening.** `join::open()` is a paused host op (same shape as `card_move`,
`hostfns.rs:2038-2057`, **including** the pause itself): a new
`HostRequest::JoinOpen { chain, seats }` is answered by a new
`HostCtx::join_open` arm, which parks a `JoinResume { chain, order, next,
join_at }` on the Cx, appends a nested round on the answered link and runs
`resolve_jobs` over it, suspending the opener's body. On re-entry, `opened`
short-circuits and the parked job continues. On completion the opener's body
resumes at the `?`. `HostRequest` is not serialized (like `Move`), so no save
change; the resume state rides the work stack. The Kani `Op` is the stub of
5.7; there is no arm for the request in the model beyond the state flag.

**Presentation.** A joiner's offer runs `build_round` with
`Priority::over(&seats)` — a generalization of the fixed full-lap
`Priority::new` (`wasm_rules.rs:7543-7584`) — and builds the link with
`link.target = cursor` (the joiner), so `trigger::target() == player_id` holds
and guards written against "I am the designated target" pass. `by_card` is
still the play's causer, so 「来自你以外」 still reads the causer.

**Shielding.** `join::shield()` marks `used[i]`, and nothing else: the body
still chooses what to cancel, with `join::opener()` as the destination of the
self-targeted effect (「改为你」). `net_error` becomes
`ctx::cancel_designation(ctx::join::opener()?); ctx::join::shield();`.

**Report.** `joiner_count` / `joiner(i)` / `joiner_shielded(i)` read `join`
after the nested round settles; `declared` counts even when the counter was
negated (the book says 「打出过」).

**Nesting depth.** The join round is `depth + 1`; a joiner's counter is a
normal link and can be answered further. `MAX_COUNTERACT_DEPTH` still bounds it.

**Fallback.** No engine-side fallback: the opener's body decides from
`joiner_count()`. mana's body is

```rust
ctx::join::open()?;
for i in 0..ctx::join::joiner_count() {
    let j = ctx::join::joiner(i);
    if ctx::join::joiner_shielded(i) && ctx::ask_yes(j, …)? {
        ctx::draw(j, 1)?;
    }
}
if ctx::join::joiner_count() == 0 { ctx::draw(player_id, 1)?; }
```

### 5.5 History (item 8)

At declaration, before `hand.remove` and before any body:

```rust
cx.world_copy().turn.countered_this_turn.push(CounterRecord {
    declarer: cursor as i32,
    countered: chain[timing].seat as i32,
    card: id.clone(),
});
```

`countered_by_this_turn(seat)` filters the turn vector, dedupes by declarer,
preserves order. `TurnCtx::default()` clears it each turn, so 「当前回合内」 is
the default scope.

`cant_look_away` needs no entry field: `run` takes
`let targets = ctx::countered_by_this_turn(player_id);` and returns only when
that is empty — true for the play entry (empty set on a fresh turn) and for the
counter entry (the immediate counterer is already in the list). The
`target == owner && actor != owner` pre stays for the counter entry.

### 5.6 Extreme lifetime (item 4)

`draw_event` (`play.rs:4943`) gets the same scope pair as `play_from_hand`:
save `prev_extreme`, set `Extreme::Plain` before the `event` raise, restore
after the body / at `eventAfter`. `tsugumi_can.rs` declares
`ChainKind::Event` and `answers_numbers(NumberInterval)`; the `event:*` rule
carries `RANGE_CARD` where the event's effect is an interval.

### 5.7 Kani model

* `sys_kani` / `kani_host::Op` gains one op per import (README band 371-390;
  340 is the current maximum and G02/G04 take 341-370 ahead of this group).
  `native_shims` mirrors them with the `ruleset_*` names. `host.rs` registers
  them next to `join_redirect_target` (`host.rs:5567`).
* `rules/kani/src/host.rs` stubs: `join::open` = no-op that sets `opened`,
  `joiner_count` = 0 by default, `pending_*` = 0, `countered_by_this_turn` =
  empty. The pure contract/report functions are modelled in
  `rules/kani/src/spec/abi_g06.rs` and proved in `proofs/abi_g06.rs` (9.4), so
  the stubbed ops only need shape-consistency.

---

## 6. Card usage

### 1 / 2 / 9 / 10 — `net_error`, `mana_champion`

**Before** (`net_error.rs:134-155`): the joiner branch cancels `to`, calls
`set_join_used()`, asks itself yes/no and draws itself. **After**:

```rust
let designations = ctx::designations(trigger::player_id());
if let Some(to) = ctx::join::opener() {
    ctx::cancel_designation(to);
    ctx::join::shield();
    ctx::log(player_id, &Msg::new(key!("net_error_target")).player_id("who", to));
} else { /* per-pair cancel as today */ }
```

**Before** (`mana_champion.rs:71-108`): body logs, reads
`designation_cancelled`, draws. **After**: log, `ctx::join::open()?`, the
joiner loop over `joiner_count` / `joiner_shielded` (5.4), and the fallback
draw only when `joiner_count() == 0`. `mana_champion` keeps
`prop::SELF_REDIRECT`; the engine still pre-scans the declared answer (not the
hand) before any body, and the `can_counteract` "already shielded" shortcut on
`designation_cancelled` is dropped.

### 3 / 4 — `tsugumi_can`

**Before**: `[ChainKind::Card]`, `can_counteract` returns `true`.
**After**:

```rust
On::Counteract(&[ChainKind::Card, ChainKind::Event], "", Some(can_counteract),
               counteract)
    .answers_numbers(abi::NumericShape::NumberInterval)
    .arms_extreme()
```

`can_counteract` becomes `!trigger::extreme_armed()` (plus the existing
judgement TODOs). No ycm special case: ycm declares `RANGE_CARD`.
`yolo.rs` / `tsugu_ycm.rs` add `prop::RANGE_CARD`.

### 5 — `vocal_too_hard`

`.answers_whole_payments()` on the entry; pre unchanged (the `value >= 5000`
comparison still reads the declared amount, CROSS-TESTS C20).

### 6 — `mortis_instinct`

No card change for the links half: `&[]` now means every kind and the entry is
offered (`can_counteract` still returns `false` until CH-05).

### 7 — `repaint`

Add `ChainKind::Card` to the links and declare `chain_depth() == 1` in the
body; the pre's `move.roll != null` now passes because the payload rides
forward. No other change.

### 8 — `cant_look_away`

`run` uses `countered_by_this_turn(player_id)` (5.5). The play entry is now a
real activation; the self-move clause becomes reachable and the `TODO(规则书)`
at `:68-71` drops.

### 11 — `now_sumimi`

Pre gains `&& trigger::pending_negation() == 0`; `.declares_negation()` is added
by every card that can `set_cancelled` (net_error, and the sweep in 9.3).

---

## 7. Save / record / replay

* **SAVE_VERSION 14 → 15** (third SAVE bump in landing order; take the next free one). `TurnCtx.countered_this_turn` is durable state, so
  it needs a version bump; the field is `#[serde(default)]` so a v12 save
  loads with an empty history (replays ride archived bundles anyway).
* **RECORD_VERSION stays 3.** No input-stream shape changes; the record's
  `EngineStamp.save_version` moves with `SAVE_VERSION`.
* **Join state is not saved.** It lives on the in-flight chain (`CounteractResume`
  is `Box<dyn Any>` on `Cx`), exactly like the rest of the chain (STACK-01). A
  save taken *mid-chain* already loses the chain; joins add nothing new. A
  suspend (a joiner's prompt) writes `on_link` back before parking, so the
  in-memory resume keeps `join`.
* `join_redirect` / `join_used` stay on `Trigger` for modules built before the
  bump, so an archived bundle's card keeps working against its own engine.

---

## 8. Versioning

`ABI_VERSION` 109 → **110** (seventh group in landing order), entry inserted immediately below the constant
(newest first):

```
/// v110: **counteraction answer contracts, join windows, counter history,
///    payload carry-forward**. `CounteractEntry` gains `numeric` /
///    `pay_shape` / `effect` with `On::answers_numbers` /
///    `.answers_whole_payments` / `.declares_negation` / `.arms_extreme`
///    sugar, and `ManifestOn` the trailing `numeric` / `pay_shape` /
///    `answer_effect` / `all_kinds` fields -- postcard is not
///    self-describing, so **that** is the wire break. An empty `links` list
///    now means every chain kind (`all_kinds`) instead of never. New
///    `prop::RANGE_CARD`; new imports `ctx::join::{opener,seats,shield,
///    open,opened,joiner_count,joiner,joiner_shielded}`,
///    `trigger::{pending_negation,extreme_armed,subject_card,chain_depth,
///    answered_kind}` and `ctx::countered_by_this_turn`; new window vars
///    `subject_range_card` / `extreme_armed` / `pending_negation` /
///    `pay_split_leg` / `chain_depth`. Counter links carry the answered
///    link's payload forward (`mv`, `pay.split_leg`, `effects`,
///    `subject_card`, `join`) and set `subject_card` to the answered card.
///    `set_join_used` stays as an alias of `ctx::join::shield`. SAVE_VERSION
///    14 → 15 (`TurnCtx.countered_this_turn`); RECORD_VERSION unchanged.
```

SAVE_VERSION bump reason is stated in 7; the RECORD_VERSION decision is "unchanged,
no input-shape change".

---

## 9. Tests and Kani obligations

### 9.1 Per item (`rules/tests/tests/`)

| item | test | assertion |
|---|---|---|
| 1 | `g06_join_draw_belongs_to_the_opener` | joiner plays net_error under mana; the **opener** draws 1 and the joiner draws 0 |
| 1 | `g06_join_draw_is_optional_per_joiner` | opener declines the ask → no draw, money unchanged |
| 2 | `g06_join_shield_reads_back_on_the_opener_link` | `join::joiner_shielded(0)` is true after a landing joiner; `set_join_used` alias holds |
| 3 | `g06_tsugumi_not_offered_on_sunset` | 夕阳 play → no offer |
| 3 | `g06_tsugumi_offered_on_yolo_and_forces_the_extreme` | YOLO counter link → offered, `1d4` becomes 4 / 1 |
| 3 | `g06_tsugumi_not_offered_twice` | two copies, one play → the second is not offered |
| 4 | `g06_tsugumi_answers_a_numeric_event` | numeric event armed at max, and the next play rolls plain |
| 5 | `g06_vocal_not_offered_on_a_split_leg` | 3-way split of 6000 → no offer on any leg |
| 5 | `g06_vocal_offered_on_a_whole_transfer` | a single 5000 transfer → offered, 0 paid |
| 6 | `g06_mortis_empty_links_match_every_kind` | the matcher dispatches the entry at `card`, `settle`, `pay` … (guard-independent) |
| 7 | `g06_repaint_sees_the_roll_on_a_counter_link` | repaint answers YOLO's counter; X is computed from the **original** face |
| 8 | `g06_cant_look_away_moves_every_counterer_this_turn` | two counterers this turn → both walk |
| 8 | `g06_counter_history_is_empty_next_turn` | next turn the play entry moves nobody |
| 9 | `g06_join_window_offers_a_single_target_effect` | a one-target effect: bystanders **are** offered |
| 9 | `g06_join_offers_no_seat_twice` | seats are a set; duplicate offers impossible |
| 10 | `g06_fallback_draw_bars_on_a_missed_joiner` | a joiner who shields nothing still bars the draw |
| 11 | `g06_now_sumimi_not_offered_after_a_declared_cancel` | 网络链接异常 cancels the link first → no offer |
| 8 | `g06_counter_history_survives_save` | save/load round-trip keeps the list |

Existing suites that must stay green: `cross_chain::c22` / `c23`,
`gap_window::g19` / `g20`, `crychic_vocalist_waives_big_payment`,
`repaint_contract`, all `*_kani` `ppcost` exports.

### 9.2 Guarded-offer tests

`chain_eligibility.rs` gains a table-driven `g06_contract_admits_matrix`:
(kind × subject prop × pay shape × extreme) against a synthetic entry, asserting
offer/refusal exactly — the direct unit of the invariant.

### 9.3 Static sweep

`crates/card-sdk` build check (or a `ruleset-tests` test): every
`On::Counteract` entry whose body calls `trigger::set_cancelled` /
`negate_effect` must declare `.declares_negation()`. The compiler cannot see
the body; a card-level lint can grep the crate source.

### 9.4 Kani obligations

`rules/kani/src/spec/abi_g06.rs` models the pure functions; `proofs/abi_g06.rs`:

* `contract_admits_iff` — for an arbitrary link and an arbitrary entry,
  `admits` is true **iff** kind ∈ links, numeric shape holds, pay shape holds.
  Exactness both ways; one mutant per predicate must fail.
* `join_report_iff` — for an arbitrary declaration/used sequence,
  `joiner_count` = declared count, `joiner(i)` = i-th declarer,
  `joiner_shielded(i)` = used[i]; a mutant that folds `declared` into `used`
  fails.
* `history_append_iff` — `countered_by_this_turn(s)` = exactly the declarers of
  records whose `countered == s`, deduped, order kept; a mutant that keeps
  other seats' records fails.
* `pending_negation_iff` — the count is exactly the declared answers marked
  `NEGATES`; a mutant counting all answers fails.
* `empty_links_total` — for every `ChainKind` k, an empty-links entry admits k
  (the `all_kinds` model).

Card bodies stay PROTOCOL-LEVEL (window/chain), as in `spec/kani/*.md`; the
obligations sit on the SDK-side pure functions.

---

## 10. Alternatives

**A1 — keep the link stamp, add only the getter (no window).**
`join_used` becomes readable, the joiner keeps drawing itself, mana reads the
flag for the fallback. Smallest change; rejected because "who joined" and "who
shielded" collapse into one bool, a single-target effect still offers no
bystander, and every joining card must re-implement the draw. Kept as the
compatibility alias inside the chosen design.

**A2 — a full per-seat `PlayCtx` copy (C#'s literal `H.Counteract(copy, p)`).**
Each joiner gets a cloned link + chain. Truest to C#, but duplicates the answer
tree per seat and breaks one shared LIFO order (a counter to a joiner could not
see the other joiners' results). Rejected for the shared-link-plus-override in
5.4.

**A3 — a dynamic counteract mask (`fn() -> &'static [ChainKind]`) for Mortis.**
Closures cannot see the copied card at ruleset-build time; a runtime call per
offer costs more than a bitmask shift. Rejected in favour of empty = all, which
is one line in the matcher and already expressible in the manifest.

**A4 — express range/pay contracts only as CEL pre conditions**
(`subject_range_card`, `pay_split_leg`) with no entry fields. Simplest surface,
but every offer then pays a world copy + scope + CEL eval to discover the card
is ineligible. Chosen design keeps the entry fields as the cheap index and the
vars as the body/guard escape hatch.

**A5 — counter history as a global log with turn keys instead of per-turn
state.** Saves one `Vec` clear per turn but grows unbounded within a game and
needs a turn filter on every read. Rejected; `TurnCtx` is already the turn's
scratch.

**A6 — make `counteracted` the counter-link kind and deprecate `card`.** Cleaner
naming, but `cant_look_away` / `dice_cast` already declare `Card` for counter
links and would silently close. Deferred to an open question (11).

---

## 11. Open questions

1. **vocal / 「一次性」**: is `total_stage == false` the whole test, or must the
   leg also belong to a `ctx::split_pay` (a bare `transfer_leg` outside a split
   would then still qualify)? Design assumes "any leg".
2. **Negated joiner**: does a joiner whose counter was negated still bar the
   fallback draw (「打出过」 says yes) and still draw (it shielded nothing)?
   Design says bars-but-does-not-draw.
3. **Shield with nothing to shield**: a joiner shields after every designation
   on the play is already cancelled — does the opener still draw for it?
4. **`@Tsugu ycm` exception**: does 「除@Tsugu ycm以外无法直接改变骰子点数」 mean
   ycm *is* a legal target (design assumption, via `RANGE_CARD`), or that it is
   excluded? OPEN-@Tsugu / C-AG-tiny-can-02.
5. **Is "pure number interval" a card property or a per-effect flag?** A card
   could have one interval effect and one table effect. Design assumes card
   level (C# `Def.RangeCard`).
6. **Nested joins**: can a join window open inside a join window (a joiner
   plays another `SELF_REDIRECT` counter)? Design allows it; needs a depth cap
   decision.
7. **`counteracted` as the real counter-link kind** (A6) — worth a compat
   sweep now or later?
8. **`Counteracted` kind** is documented "Not raised by the current engine"
   (`abi.rs:1859`) yet `cant_look_away` declares it. Does the design raise it
   alongside `card`/`cardUsed` (a third alias) or leave it reserved?
