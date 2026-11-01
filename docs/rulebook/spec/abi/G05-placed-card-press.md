# ABI design G05 — Press surface for placed cards

**Status:** draft for user review · design only, no source touched.
**Worktree:** `.claude/worktrees/g05-design-review` (branch
`worktree-g05-design-review`), base `master@b97eac0d` (ABI 110, SAVE 14,
RECORD 4).
**Group:** G05, family "Press surface for placed cards".
**Must land with:** G13. Both groups need one `invoke_entry` shape; §6.1 adopts
G13's import and frame wholesale and adds only the press binding.
**Ground truth:** `docs/rulebook/cards.json` (sheet-derived). Every quoted
fragment below is verbatim from it. Rows are indices into
`docs/rulebook/card-review-2026-10-27.json`.

Terminology, fixed here for the whole document. The problem is **three
different shapes** that the sheet writes on placed cards, and the design uses a
different mechanism for each:

* a **press** is a player-initiated activation of a placed card. It is the
  card's own `On::Play` run with the instance bound, in the main phase — exactly
  what a skill button already does, and what `MyGO:即使迷茫着` (2) is. No new
  entry kind: what was missing is the ability to say "the placed instance", not
  a different body shape. §4 adds the one bit that marks a Play as offered on
  the field;
* a **prompt** is an engine-raised question, asked because a printed condition
  became true (「主要阶段中，若此卡上有至少9个水母标记，可以…」,
  「你可以移除一个奇迹水晶视为发动你的（2）技能」). It is an `On::Action` hook
  or an `ctx::ask_yes` in the owning body; the player answers 发动/不发动 and the
  effect runs. A prompt has no button;
* a **passive clause** is a printed effect with no offer at all — it is applied
  when the card enters and read from the instance thereafter (`[场]花音每次倒走
  时…`, 「本卡在[场地]时改变…」).

The remaining terms:

* the **instance** is the `FieldCard` row (`FieldCard.uid`); a press is
  addressed by instance, never by re-finding a card id;
* the **effective card** of an instance is the card whose printed surface the
  instance runs (`FieldCard.effective_card`, §6.2); it is the instance's own
  card unless a stand-in wrote the marker (§9.5);
* an **entry** is one printed clause, named by its required slug `id`. Entries
  matter for cross-card invocation and `query_guard` (§6.1); a press itself
  needs none, because only the Play is ever pressed today.

## 0. Row disposition

The consolidated BACKLOG slug `press-surface-placed-card` lists nine rows. They
are **mis-filed**: none of the nine is a press-surface bug. The press defects
sit on four cards and one engine line, which the plan below closes.

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

The press defects, recomputed from the review file:

| # | slug | severity | true rows | file |
|---|---|---|---|---|
| 1 | `nanami-press-invokes-clause-3` (shared with G13) | high | 734, 735, 737 (also 742, 745) | `rules/cards/card-morfonica/src/nanami_effort.rs` |
| 2 | `kanon-press-surface-abi` | high | 607, 608, 610, 611, 612, 619, 620, 621 | `rules/cards/card-hhw/src/kanon_march.rs` |
| 3 | `even-lost-not-pressable` | high | 798, 801 | `rules/cards/card-mygo/src/even_lost.rs` |
| 4 | `placed-card-press-surface` | high | 1614 | `crates/game-core/src/engine/mod.rs:1416` |
| 5 | `hina-clause-2-swap-missing` (moved from G01 R11) | medium | 930, 931, 932, 936, 972 | `rules/cards/card-pp/src/hina_sound.rs` |

Item 5 moved here whole from G01: its clause (2) is a press of a placed card,
and the effective-card marker the press resolves through is part of the press
surface. G01 keeps nothing of it; the lifetime edge is G08's
(`created_by`, G08 §4.9).

Items 2 and 3 also have a *demoted* half. The review's stated expectation
("appears pressable, reaches `cant_act`") is reachable by deleting one prefix
filter, so a stopgap can ship alone. The full fix is still needed for item 4,
because a filter drop replays every hand-only body from the field.

---

## 1. Problem, per item

### 1.1 `nanami-press-invokes-clause-3` (rows 734, 735, 737)

Book (`Mor:（NNM）稍微努力了一下`, clause (3)):

> （3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的
> （2）技能，此次技能不受数量或轮数限制

「你的（2）技能」 is the *persona skill's clause (2)*. The press runs the skill
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
and (3) as its `On::Play` (line 50). `ctx::play_card` and `invoke_skill` both
resolve "the card's first `On::Play`" (`invoke_skill` via the PRESS alias), so (2) is
unreachable by id. Two further
consequences sit in the same rows:

* the crystal is spent *before* the body, so the removal happens with no
  question — a player who has a crystal but cannot use (2) burns it (row 734's
  probe);
* 「此次技能不受数量或轮数限制」 waives (2)'s per-kind mark cap and 3-turn CD;
  nothing evaluates them today, so nothing is waived (row 735).

The clause is an immediate **prompt**, not a standing button: 「你可以移除一个
奇迹水晶」 is asked as part of resolving (3), at the moment the card lands. The
fix is `ctx::ask_yes` before the crystal is spent, then the invocation (§9.1);
the invocation itself is still a named entry (`"2"`), which is why G13 is
needed.

### 1.2 `kanon-press-surface-abi` (rows 607, 608, 610, 611, 612, 619, 620, 621)

Book (`HHW:（花音）Wacha Mocha 啪嗒进行曲`):

> [场]花音每次倒走时此卡获得一个水母标记；主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。

Two clauses: a [场] placement and a placed press. One `On::Play` serves both, so
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
not a key kind but a *condition*, and there is no hook on the marker count. The
jump is therefore a **prompt**: raised when the count changes (and after a move
resolves, so a decline can be re-offered), gated on the main phase, the main
move being available and ≥9 markers (§9.2). Rows 610-612 add that the jump only
writes the turn plan and never runs it (this tree already calls
`run_planned_move`, so those three are closed; they stay listed because they
were the original evidence). Row 621's main-move gate and row 620's second-copy
case are still open.

### 1.3 `even-lost-not-pressable` (rows 798, 801)

Book (`MyGO:即使迷茫着`, clause (2)):

> （2）[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。

The card is never played from hand — it enters through its own [反击] (1) — so
its only `On::Play` **is** the field activation: a placed card's activation is
that card's Play run on the instance, with no body branching needed
(`rules/cards/card-mygo/src/even_lost.rs:19-21`). What is missing is only the
offer: the view lists `skill:*` instances and nothing else
(`engine/mod.rs:1416`), and no bit says "this Play is field-activated". `can_go`
additionally asks `!ctx::is_placed()` and never asks whether the main move is
available, so a press after the main move was cleared for nothing (row 798).

### 1.4 `placed-card-press-surface` (row 1614)

The umbrella, at `crates/game-core/src/engine/mod.rs:1416`:

```rust
            if !id.starts_with("skill:") || !self.rules.has_play(&id) {
                continue;
            }
```

There is no field-activation identity at all:

* `view_extra_parts` skips every id without the `skill:` prefix, and `source`
  is derived by `strip_prefix("skill:")` (lines 1427-1431), so a plain card has
  no source;
* the client's only press sender is `act: "skill"` off that list
  (`webui/src/scenes/board/Side.tsx:214`), and `skillMarks` finds the instance
  by **card id**, not instance (`Side.tsx:95`);
* both the browser autopilot (`webui/src/game/autopilot.ts:750`) and the bot
  policy (`crates/bot-core/src/policy.rs:435-444`) read the same list.

Dropping the filter is not enough: `CardRules::has_play` is true for every hand
play, so hand-only `[手]` bodies would appear and rerun their hand body on a
placed copy with no reservation. The row's own `fix_plan`: "Needs a
capability, not a filter tweak".

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
surface that is. Rows 930/932 also record that `SUBS` lacked 大和麻弥 — the one substitute with
no `[手]`. That half is already closed in the tree: `SUBS` carries four and the
body places 麻弥 with `place_card` (`hina_sound.rs:43-48`, `:70-71`); the
eligibility TODO at `:51-53` stays open and the swap must keep the same
`place_card` arm. Row 936 is a stale TODO (`resonance::try_resonance` exists)
and row 972 the missing fuel call.

---

## 2. Shared root causes

1. **One `On::Play` is two clauses.** `On::Play` means "play from hand" *and*
   "press on the field". The body and gate recover which one by `is_placed()` /
   `play_from_hand()` / `hand_count()`. The recovery is per card, unenforced, and
   wrong with a second copy in hand.
2. **No clause identity on activations.** Activating a placed skill is
   identified by the *card* (`act: "skill"`, `card: id`); the clause inside the
   card is found positionally (`entry(OnKind::Play, None)`). A card whose
   activation is another entry kind cannot be invoked at all — which is exactly
   the nanami defect.
3. **No instance identity on activations.** `Ruleset::play` (`wasm_rules.rs:10201`
   region) and `Ruleset::cant_play` (`9879-9900`) re-find the instance by first
   match of the id, so two copies share one press and one marker read.
4. **The offer surface is a prefix convention.** `skill:` is a rule-id
   namespace, not a capability bit; the view and both AI readers hard-code it.

### 2.5 Necessity — why the group cannot be smaller

**Every row is printed text, not a preference.** The sheet puts a player-activated
clause on a placed card four times: nanami 「你可以移除一个奇迹水晶视为发动你的
（2）技能」, kanon 「主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并
传送…」, even-lost 「主要阶段中，你可将此卡置入弃牌堆并进入移动阶段」, hina
「本卡在[场地]时改变本卡代替的专属卡效果」. Today two are unreachable and two
are wrong. A placed card with a switchable clause is a whole category of card,
not four oddities, so leaving it unimplemented is a functional hole the triage
graded high.

**Nothing smaller reaches the text.** The retrenchments and what each leaves:

| retrenchment | buys | still fails |
|---|---|---|
| A1, drop the `skill:` filter (§13) | kanon and even-lost appear | their gate and body still guess the origin, every `[手]` card replays from the field, kanon's jump can never be reached on the placed copy |
| A2/A3, the bit on a prop or on a new kind | the same one button | a prop has to be read from the manifest anyway; a kind buys a second pressable clause that no clause here uses (§13) |
| card-side only, no ABI | nothing | a card cannot teach the engine to list it, cannot be handed its instance, and has nowhere to record "this instance runs the stand-in's surface" |

**Three different mechanisms, and only one needs a surface at all.** Kanon's
jump and nanami's crystal are printed as conditions (「若…可以」, 「你可以」 as part
of (3)); hina's change has no offer. Only even-lost is a plain standing 「你可」,
and it is a Play. Collapsing the other three into buttons — the first draft —
over-fits; leaving them out under-delivers. The design therefore adds one bit
(`.field()`), one prompt-shaped card rewrite each for two of them, and nothing
interactive for the third.

Yet the surface is still the common denominator: every one of the four needs the
**instance** (kanon's copy, nanami's crystal, hina's substitute, even-lost's
discard target) and every one but even-lost needs a way to run another card's
named clause (`invoke_entry`). That, and only that, is the ABI this group adds —
`Call::Press`, the instance parameter on the Play path, and G13.

**The cost is small and mostly already paid.** One manifest bit
(`serde(default)`), one internal `Call`, an `instance` parameter on three
existing methods, two imports for the surface marker, one optional request field
kept out of hashes by `skip_serializing_if`, one `serde(default)` turn field, one
sender, one animation arm, two policy scanners. No import gains a player
decision; no card outside the four owners changes; nothing about the Play body
shape moves.

**G13 is in the same window.** The skill invocation is this group's item 1, and
`invoke_entry` is G13's surface; landing press later means writing it press-less
and re-cutting its frame.

**What would make the group unnecessary.** A ruling that placed cards are not
pressable collapses even-lost and the offer half; the sheet contradicts it. If
only items 1/2 (the prompts) were wanted, they need no surface and could ship on
`invoke_entry` alone — but even-lost and hina stay broken, which is why the group
is not merely the prompts.

---

## 3. Capability as an invariant

**A press is a placed instance's own `On::Play`, run on that instance, in the
main phase — never a card-wide yes/no, never an origin guess, and never a card
activation.**

Corollaries, all of which the surface below makes true:

1. The clause is the card's Play. `MyGO:即使迷茫着` (2) is a Play that only ever
   runs on the field; `HHW:…进行曲`'s placement is a Play that only ever runs
   from hand. Which of the two a given request is follows from *where the card
   is*, not from a test inside the body or gate.
2. What is printed as a state condition is a prompt, and what prints no offer
   is passive — neither gets a button. `On::Action` plus `ctx::ask_yes` cover
   the first; the instance is the second's storage (§9).
3. The pressed instance **is** the request's `instance`. Dispatch validates
   that it is in play, on the presser's field, and carries the requested
   effective card; it never re-finds by id. Every instance read (`is_placed`,
   named counters, `crystals`, props, `dest`) answers for that instance.
4. The offer is enumerated from the field: an instance is offered iff its
   effective card's Play carries the field bit (`.field()`), gated through the
   same `cant_act` a request takes, so "shown enabled" ⇔ "accepted". The only
   prefix test left is the temporary skill alias (§7.3).
5. A press is an activation, not a hook: it is not reentrancy-marked. It is
   **not** a `cardUsed` activation either (§7.4), so it neither trips the
   「使用技能」 conditions nor `cards_played`, and nothing negates it before its
   body; its own effects raise their own windows.
6. Cross-card invocation names the entry:
   `ctx::invoke_entry(player_id, card_id, entry_id)`. `ctx::invoke_skill` is
   sugar for it on `entry::PRESS`, the alias for a card's activation entry.
7. Same barrier order as a play: gate at offer, `declare` before the point of
   no return, body after; no re-gate at body time.

The invariant is what makes §1.1 expressible: "run clause (2)" is
`invoke_entry(skill, "2")`; "press the card" is the instance, not a flag; "offer
the jump" is a hook on the count, not a button.

---

## 4. Surface — the field bit on `On::Play`

**No new `OnKind`.** A press is the card's Play run on the instance (`MyGO:即使
迷茫着` (2) is literally an `On::Play` body), so the only thing the ABI has to
carry is *"this Play is also offered on the field"* — one data bit, because a
Play is normally hand-only and the offer must not smear every `[手]` card onto
the board.

```rust
pub struct PlayEntry {
    // …
    /// Also offered as this card's field activation while it is placed: the
    /// view lists it on the instance and a press runs this same body with the
    /// instance bound. A hand play is unaffected. Default `false`.
    pub field: bool,
}

impl On {
    /// Mark this Play as the card's **field activation** (see `On::Play`).
    pub const fn field(self) -> Self;
}
```

`ManifestOn` gains `#[serde(default)] field: bool`; the value reaches
`WasmRules` with the rest of the entry and is cached beside `has_play`. Only a
card that prints a field activation sets it — today just `MyGO:即使迷茫着`
(§9.3). Kind 15 is left free for a later, genuinely different body shape.

`abi::entry::PRESS` (`abi.rs:1153-1162`) keeps its spelling and stops meaning
"whatever the engine would pick". It is now the alias for the card's
**activation entry**: its first Play with `.field()`, else its first Play. A
card still declares no `"press"` id. `ctx::invoke_skill` and old `act: "skill"`
records reach a press through it; the `On::Play` fallback keeps every unmigrated
skill working.

**`entry::PRESS` answers can change for a card that gains `.field()`.** Such a
card resolves `PRESS` to the field Play, so a caller asking "may I use this
from hand?" must name the Play's own id. §12 pins `detour`/`nfo`/
`please_choose`.

---

## 5. Surface — `card_sdk::lib` builder and gate

`On::Play`'s signature is untouched and every existing call site stays valid;
`PlayEntry` gains the one `field: bool` (default `false`) and `.field()` is the
builder that sets it — a marker, not a new constructor:

```rust
On::Play("", Some(can_go), go).field()
```

`condition` and `guard` stay exactly the gate a hand play uses — same pair,
same order, same `OP_GUARD`. Nothing about the body differs: a press body is a
Play body, and the instance is bound for it (`current_uid = instance`), so
`is_placed`, named counters, `crystals`, props and `dest` all answer for it.

---

## 6. Surface — imports and the effective card

### 6.1 Shared with G13

G05 does not respell G13's invocation surface; it adopts it verbatim and adds
only the caller's instance in the frame. If both groups land, **G13 §4.1-4.4 is
the normative text** for the import itself.

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

`ctx::invoke_skill(player_id, id)` becomes guest sugar
(`ctx.rs`, no `sys` declaration): `invoke_entry(player_id, id, entry::PRESS)`.
Its host import `ruleset_invoke_skill` stays as an alias so a pre-v111 wasm still
links; the guest declaration is what goes.

`ctx::play_card` **stays** a guest function with its own `ruleset_play_card`
import. It is used by many cards, it returns `(Dest, Prompt)` and defaults
`play_from_hand = false`; routing it through `invoke_entry` would need a
resolution mode `invoke_entry` does not take. Host-side it is a thin call into
the same core and always takes the first Play — a play is a play, even on a
card whose Play also carries `.field()`.

The frame's `entry` is always the **resolved** entry's declared id (`""` when
that entry is unnamed, which is the common case, e.g. a `play_card` into a bare
`On::Play`), never the alias spelling. `entry::PRESS` resolves the field Play
first, then any Play; that fallback is what keeps `query_guard(PRESS)` truthful
for every skill and every unmigrated card.

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

`effective_card_at(instance)` names the card whose **activation surface** the
instance runs; it is not a rename of `FieldCard.card`, so counters, props and
`dest` stay the instance's. `set_effective_card_at(instance, "")` clears. The
marker needs a live instance and a declared id and does not validate the pair.
Field:

```rust
pub struct FieldCard {
    // …
    /// The card whose printed activation surface this instance runs (`None` =
    /// its own). Written by a stand-in, dies with the instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_card: Option<String>,
}
```

`SAVE_VERSION` moves for the `TurnCtx` fields in §7.6, not for this.

Every read of an effective card on a placed instance
(`effective_card_at(instance).unwrap_or(own)`) is this group's: the offer, the
press request, and the press gate. Until G05 lands nothing reads it.

All three targets: wasm32 resolves both imports; native / `native-rules` needs
`HostOps::op_effective_card_at` / `op_set_effective_card_at` plus their
forwarding impls in `crates/game-rules/src/native_shims.rs` and a
`crates/rules-native/tests/abi_link_names.rs` pin; Kani routes the `sys`
declarations through `sys_kani` to the model ops (§8.6).

Op ids, carved from G01's band (which ends at 346): `Op::effective_card_at =
347`, `Op::set_effective_card_at = 348`. G08 keeps 349.

---

## 7. Surface — engine

### 7.1 `game-rules` dispatch

```rust
pub enum Call {
    // …
    /// A press — the placed instance's own `On::Play`, run on that instance.
    /// `instance` is the field row; `arg` is the argument the Play's
    /// `OP_DECLARE` collected. Same entry the body of a hand play would get.
    Press {
        card: i32,
        player_id: i32,
        instance: i32,
        arg: Vec<u8>,
    },
}
```

`Call::player_id()` / `Call::card()` include it. `Ruleset::run` resolves
`Call::Press` to the card's Play index — the same resolution a `Call::Play`
takes, now through the effective card of the instance — takes `arg`, and keeps
Press **out** of the body-time admission block (the gate runs at offer).
`drive_inner`'s reentrancy marker becomes
`uid >= 0 && !matches!(call, Call::Play { .. } | Call::Press { .. })`
(`wasm_rules.rs:3294`). `announce_drive` maps it to `card_trigger::PRESS` and
the 「发动」 wording (`wasm_rules.rs:3144-3153`).

The Play path itself grows one parameter: an instance. `Ruleset::play`,
`cant_play` and `declare_activation` take `instance: i32` (`-1` = from hand),
and use it in place of the first-match lookup
(`wasm_rules.rs:10201`, `9879-9900`). `has_field_play(card)` answers the
`.field()` bit statically. There is no `press_*` family: the press is the Play.

`cant_play`'s candidate is built with
`condition::fill_candidate_uid(world, player_id, card, instance, true)` so
`card.counter(…)` reads **that** instance, and the guard `Run` carries
`current_uid = instance`. This is already what the skill press gets
(`wasm_rules.rs:9890-9900` finds the uid); it only has to take the uid from the
request instead of finding it.

**Note — the nanami (2) condition does not apply to a self-press.** (2)'s
condition is `actor != owner && trigger_card != 0 && charge(owner) >= 1`, and
`fill_candidate(…, player_id, …)` sets the candidate's `owner = player_id`, so
for the owner's own invocation `actor != owner` is false. A caller must
therefore not gate on `query_guard(skill, "2")`; nanami's prompt tests its own
costs directly (§9.1). The invoked body is not re-gated, which is what waives
the cap and CD.

`Ruleset::build` (`host.rs:1086-1120` region) gains one rule for the bit: a
manifest row may carry `field` only when its kind is `Play`.

`WasmRules::press` (the `CardRules` method; `play` at `wasm_rules.rs:10201` is
the model):

```rust
fn press(&self, cx: &mut PlayCtx, player_id: usize, card: &str, instance: i32)
    -> Flow<Dest>
```

1. `ruleset.card(card)`; unknown -> `log.card_not_ported`, `Dest::Graveyard`.
2. Validate `field_by_uid(instance)`, `holder_of(instance) == player_id`,
   the effective card of the instance `== card`, and that its Play carries
   `.field()`; else log and no body.
3. Push the resolving frame; `drive(cx, Call::Press { …, arg: take(play_arg) },
   card, instance, &mut trigger)`.
4. `dest_from(dest)`.

Dest needs no new work: the tail applies to the run's bound instance
(`wasm_rules.rs:5366`, `after.current_uid`), so `Dest::Graveyard` / `Hand` move
**the pressed instance** and `DEST_FIELD` is a no-op for it (already placed).

One correction while wiring `apply_dest` (`wasm_rules.rs:6469`): it currently
files `card_id`, which for a press is the **effective card**. The card that
physically left is the instance's own (`field_by_uid(instance).card`), so
`apply_dest` resolves that and falls back to `card_id` — otherwise a stand-in's
discard would file `HINA_SOUND` instead of the substitute. `ctx::send_to_dest`
(`move_now`, `wasm_rules.rs:698`) has the same defect by way of `current_card`;
it is fixed the same way, since it is `apply_dest`'s immediate sibling.

A press that wants a fresh instance must `unplace_at` first and place again
(§9.5).

### 7.2 `game-core` — request

```rust
pub struct NetMessage {
    // …
    /// `press`: the field instance. Absent = the `skill` alias (first instance).
    #[serde(default, skip_serializing_if = "is_zero_instance")]
    pub instance: i32,
}
```

`skip_serializing_if` is load-bearing: `body_check` hashes the re-serialized
`RecordBody` (`crates/game-core/src/record.rs:494-497`), so an always-emitted
field would change every stored hash. A private `is_zero_instance` helper sits
beside `NetMessage`. `Match::act` parses `"press"` at `mod.rs:2427` and
dispatches it at `:2886`; `"skill"` stays an accepted alias forever and is
normalised to `{ act: "press", instance: -1 }` before `cant_act`.

No entry on the wire: only the Play is ever pressed, and the effective card
already decides whose Play that is. If a card ever prints two activatable
clauses, an `entry` field is the additive change.

### 7.3 `game-core` — `CardRules`

```rust
/// Is this card's Play offered on the field? (Static; `WasmRules` reads the
/// `.field()` bit, plus the `skill:` compat arm while skills are unmigrated.)
fn has_field_play(&self, _card: &str) -> bool { false }
/// The press gate for the instance: `cant_play` with the instance bound.
fn press_cant_act(&self, _cx: &PlayCtx, _player: usize, _card: &str, _instance: i32)
    -> Option<Msg> { None }
/// Run the press; returns the instance's dest.
fn press(&self, cx: &mut PlayCtx, player: usize, card: &str, instance: i32) -> Flow<Dest>;
```

`StubRules` keeps the defaults. Answers are memoised separately:
`PlayCtx::press_memo` keyed `(player, instance)` with the same world-stamp reset
as `PlayMemo` (`cx.rs:593-648`), because two copies of one card differ per
instance and the `(player, card)` play memo would cross-talk. The stamp-reset
precondition is fatal here, so the memo is a new vector pair rather than a
widened key (a `&str` key plus an `i32` cannot reuse `PlayMemo`'s comparator).

Every `card` below is the instance's **effective card**. When a marker is set
the instance's own Play is replaced by the effective card's — *replaced*, not
added: a marked substitute's own Play is not offered.

### 7.4 `game-core` — `Match::use_press`

Replaces `use_skill` (`play.rs:1164`):

1. Resolve the instance (`m.instance`, else first field instance whose effective
   or own card is `m.card`) and its effective card.
2. `rules.declare_activation(self, i, card, instance)`; `Decline` ->
   stop; `Halt` propagates.
3. Store the argument in `turn.play_arg`; `rules.play(…, instance)`;
   `turn.pressed.push(instance)`.
4. `card_activated(card_trigger::PRESS, …)`.

The `turn.played` push and `play_drove_move` / `drop_spent_plan_shape` handling
stay exactly as `use_skill` has them. `use_skill` becomes the alias arm.

**A press is not a card activation.** `use_skill` raises `cardUsed` (`play.rs:
1168-1176`), and that raise is what 七深 (2) answers — 「另一名玩家使用技能
时」. A placed card being pressed is not a skill use, so the `cardUsed` raise
goes, together with its `is_cancelled` branch: nothing can negate a press
before its body, because nothing is *using* anything. `cards_played` is
likewise untouched (it drives `TwoCards`, a hand-play counter). What a press
does raise is whatever its own body produces (a pay, a move, an effect), each
with its existing window. `card_trigger::PRESS` is only the log/anim kind.

Skills keep their raise through the alias path: the `skill` arm still runs
`use_skill`, so 七深's trigger and the negate window are unchanged for them.

### 7.5 `game-core` — `cant_act`

New `"press"` arm (`mod.rs:2927` region is the skill arm it replaces), order:

1. `!playing() || step != OPS || busy` -> `err.press_not_now`.
2. Resolve the instance: `field_by_uid` when `m.instance >= 0`, else the first
   instance of `m.card`; not on the viewer's field, unknown, or effective card
   ≠ `m.card` (when given) -> `err.press_stale`.
3. The effective card must carry `.field()` (or be a `skill:` under the compat
   arm) -> else `err.press_unknown`.
4. `Gate::OnBoard`, then `Gate::Act` with `card_act_exemption`.
5. `cx.rules_press_cant_act(i, card, instance)`.

New locale keys `err.press_not_now` / `err.press_stale` / `err.press_unknown`
(zh/en). The `err.skill_*` keys stay: `MatchEvent.msg` in stored saves still
names them.

### 7.6 `game-core` — view and turn context

`view_extra_parts` (`mod.rs:1380-1447`): drop the prefix filter; for each
instance on the viewer's field, skip `face_down`, resolve the effective card,
and push one action iff that card's Play carries `.field()` (or is a `skill:`
under the compat arm). `source` is the
`skill:` owner when the effective card has the prefix, else the effective card
id. `title` / `text` stay `ask.cardOption` card references; `label` is the
Play's label, which is what the face renders.

```rust
pub struct SkillAction {
    /// The instance's **effective card**.
    pub id: String,
    /// The field instance the action belongs to.
    pub instance: i32,
    /// Display source: the `skill:` owner for a skill, else the card id.
    pub source: String,
    pub title: Msg,
    pub text: Msg,
    /// Opaque clause label (「（2）」), `""` = none.
    pub label: String,
    pub enabled: bool,
    pub reason: Msg,
}
```

The JSON key `skills` and the array order (field order) stay. `MatchState`
gains `pressed: Vec<i32>` filled from `world.turn.pressed`, so both client
policies can grey out an instance already pressed; the field is additive and
`serde(default)`.

`TurnCtx` (`world.rs:205`): `pressed: Vec<i32>` (`#[serde(default)]`). The
declare argument needs no new field — a press stores it in the existing
`play_arg`, scoped exactly as a hand play's is. Bot and client bookkeeping only
— a press never gates a human.

### 7.7 Client and bots

* `webui/src/scenes/board/Side.tsx`: the slot type gains `instance` / `label`;
  `skillMarks(me, slot.instance)` finds by instance; the React key is the
  instance; `fire` sends `{ act: "press", card, instance }`; the face shows
  `label`.
* `webui/src/scenes/board/anim.ts:941` grows `case "press":` beside
  `case "skill":` — the same banner and sfx for a `skill:` id, a new
  `anim.press` banner otherwise.
* `webui/src/core/types.ts`: `SkillAction` gains `instance` / `label`;
  `MatchState` gains `pressed`.
* `webui/src/game/autopilot.ts`: `usableSkills` returns the action objects and
  filters `S.pressed`; presses become `{ act: "press", card: a.id, instance }`;
  a non-`skill:` id with no `skills[id]` weight defaults to the neutral weight
  (pressed once per turn — the `pressed` filter is what stops the repeat).
  Documented as a policy change in `docs/BOT.md` §3.8.
* `crates/bot-core/src/policy.rs::ai_skill_choice`: scan `field_instances`
  whose effective card has `has_field_play`, skip `turn.pressed`
  (`turn.played` stays the coarse budget), gate with `rules_press_cant_act`,
  keep the strategy weight keyed by card id, default non-`skill:` ids to once
  per turn; the chaos branch presses every enabled instance once. Returns a
  `NetMessage` with `instance` set.

### 7.8 Classification

| # | surface | class |
|---|---|---|
| 1 | `PlayEntry.field` / `On::field()` / `ManifestOn.field` | NEW (source + manifest, `serde(default)`) |
| 2 | G13 imports; guest `invoke_skill` becomes sugar | NEW / **WIRE-BREAK** (guest import `invoke_skill` removed; its host alias and all of `play_card` stay) |
| 3 | `Call::Press`; `WasmRules::press`; instance on `play` / `cant_play` / `declare_activation` | NEW (internal) |
| 4 | `CardRules::has_field_play` / `press_cant_act` / `press` | EXTENDED trait |
| 5 | `NetMessage.instance`, `act: "press"` | EXTENDED wire; `skill` accepted as alias |
| 6 | `SkillAction.instance` / `.label`, `MatchState.pressed`, `skills` contents | EXTENDED view |
| 7 | `TurnCtx.pressed` | EXTENDED save (**SAVE_VERSION 15**) |
| 8 | `FieldCard.effective_card` + ops 347/348 | EXTENDED save / NEW imports |
| 9 | `card_trigger::PRESS` | EXTENDED event string (replay-safe) |

No new `OnKind`: a press reuses Play's kind, ops and builder chain.

---

## 8. Engine wiring

### 8.1 Request path

`Match::act` routes `"press"` to `use_press` after `cant_act` has run the same
checks in the same order, exactly as every other turn action does. `"skill"` is
normalised to the press alias before the gate, so one code path serves both.

### 8.2 Gate, declare, commit

The gate is `press_cant_act` at offer. `declare_activation` runs behind the
`OP_DECLARE` barrier; the argument rides `Call::Press.arg` and
`ctx::declared_arg::<T>` serves it in the body (the `drive` monomorphization
already does this for any entry with a declare). A decline returns before any
state write.

### 8.3 Instance reads and dest

The whole drive runs with `current_uid = instance`, so `is_placed` is true,
named counters / `crystals` / props resolve on that card, and
`add_held_by_name` binds to it. `set_dest(Graveyard)` at effect end moves that
instance out (§7.1).

### 8.4 Cross-card invocation

Follow G13 §5 (the routine replaces `hostfns::invoke_skill` `hostfns.rs:1531-1600`,
which `play_card` at `hostfns.rs:3045` reuses with its Play-first entry) with the
press additions marked:

1. Resolve `card_id`; bind its instance — the first on the caller's field — and
   `by_instance = the caller's current_uid` (for a press, the pressed instance);
   check `MAX_NESTING`.
2. Compute the entry: `entry::PRESS` -> the field Play, else the first Play;
   other ids -> `entry_by_id`; unknown/unnamed -> `DEST_UNSET` no-op.
3. `enter_invoke(card_id, instance)` (G13 §5.2); push a frame `{ card, entry,
   by = caller, by_instance, instance, paid, depth }`; seed the nested trigger
   from the frame (`card`, `cards = [card_id]`, `by_card = caller`). No raise —
   the caller's line owns the window — and the seed is what makes
   `trigger::cards()` answer inside the body.
4. `OP_RUN` (after `OP_DECLARE` when `has_declare`; decline -> `EXIT_DECLINED`).
5. `leave_invoke`; pop the frame; return the nested `Dest`; restore fuel.

The gate is **not** evaluated (G13: body only). That is how 「此次技能不受数量或
轮数限制」 waives (2)'s CD and cap without a flag. Nanami's prompt still checks
the caller-side costs before asking (crystal, pot — §9.1), and the body's
`fire::spend` remains the last line. No new `TriggerKind`.

### 8.5 Offer

`view_extra_parts` enumerates as §7.6. No prefix test except the compat arm in
`has_field_play`. The bit is cached on the engine (manifests are static per
match); each pressable instance yields one action, gated separately.

### 8.6 Kani model

* `Op::invoke_entry = 431` (the retired `invoke_skill` was 132, never reused;
  431-440 is this group's README band). G13 §5.5's `arm_invoke_entry` /
  `invoke_n` / `last_invoke` are adopted as-is plus the press binding: the
  record extends to `(card, entry, paid, by_instance)` and `by_instance` must
  equal `last_uid` before the frame is pushed. The nested body is not called
  (the host has no per-id dispatcher); proofs assert the record and the caller's
  effects. `invoke_frame` returns the model record.
* `Op::effective_card_at = 347`, `Op::set_effective_card_at = 348`. Model:
  `effective_card: [NameId; MAX_FIELD]` plus one tag bit per slot; both arms
  mirror the live / declared checks.
* `arm_is_placed` becomes instance-bound:
  `w.field.iter().any(|f| f.uid == w.last_uid)` (was `field_n > 0`).
* `arm_placed_cards` already resolves each `FieldCard.card` through
  `intern::resolve_str(NameId(..))` and answers real card names
  (`rules/kani/src/host.rs:2605-2627`); the `skill:` prefix test there stays
  meaningful.
* Harness helper `place_instance(player, card) -> instance` writes `field[]` and
  `last_uid`; a press harness sets `last_uid` to the pressed instance.

## 9. Card usage before / after

### 9.1 `nanami-press-invokes-clause-3` (rows 734, 735, 737)

Before: one entry, an origin branch, the wrong target.

```rust
On::Play("", Some(cant_play), play),
// …
fn play(player_id) { if ctx::is_placed() { use_skill(player_id) } else { nanami_effort(player_id) } }
// gate: if is_placed { crystals, owner_skill, marks } else { token sum }
// press: play_card(skill)  -> the skill's On::Play = clause (3)
```

After — the placement stays the hand Play, and (3)'s 「你可以」 becomes the
prompt it is printed as:

```rust
On::Play("", Some(cant_play_from_hand), nanami_effort),

fn cant_play_from_hand(player_id: i32) -> Option<Msg> {
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

The old `cant_press` disappears with the press: the prompt's own conditions
are the two inline checks. The order is deliberate — a pot and a skill must
exist before the question is asked, so the answer is never a dead end, and the
crystal leaves only after a yes (row 734's burn).

nanami's skill gains `.id("2")` on its `Action::CardUsed` entry
(`skill-personas/src/nanami.rs:58`); the label 「（2）」 stays. New locale keys
`nanami_effort_ask_title` / `nanami_effort_ask_text` (zh/en);
`nanami_effort_no_pot` / `_no_skill` / `_no_crystal` stay for the hand path and
the inline skips. Both TODOs at `nanami_effort.rs:273-281` close, and the
comment "`play_card` runs its (3) body" is rewritten to "runs the invoked (2)
body".

`fire::total` (pool + temporary pots) is the read, not `fire::get`: `spend_skill`
draws temporary pots first, so a temporary pot alone pays (2).

### 9.2 `kanon-press-surface-abi` (rows 607, 608, 610-612, 619-621)

The stopgap (ship if the group slips): delete the `skill:` test in
`view_extra_parts` and derive `source` defensively. The card then appears and
its gate is reached. Known cost: hand-only cards appear too (item 4).

Under G05 the jump is a prompt on the count, and the Play stays the placement:

```rust
On::Play("", None, place),                        // hand play: placement only
On::Action(Action::MarkChanged(
    "card.placed && actor == owner",              // the count, not the card
    Some(cant_jump),
    jump,
)),
On::Action(Action::MoveResolved(
    "card.placed && actor == owner",              // re-offer after a move
    Some(cant_jump),
    jump,
)),

fn cant_jump(player_id: i32) -> Option<Msg> {
    // 「主要阶段中，若此卡上有至少9个水母标记」 -- the count and the phase.
    if ctx::held_count(player_id, JELLY) < 9 {
        return Some(Msg::new(key!("kanon_march_need")));
    }
    // 「视为本次主要移动」 -- refused while that move is shut.
    rules_status::move_blocked(player_id)
}
```

### 9.3 `even-lost-not-pressable` (rows 798, 801)

Stopgap: the same filter drop. Under G05 the Play simply becomes a field
activation:

```rust
On::Play("go", Some(can_go), go).field()

fn can_go(player_id: i32) -> Option<Msg> { rules_status::move_blocked(player_id) }
```

The clause is 「主要阶段中，你可…」 and nothing else gates it, so the Play is the
right shape and `.field()` is the whole change to the registration. The
`ctx::play_card` path (§6.1) keeps `ctx::play_card("MyGO:即使迷茫着")` working
for the [反击]-adjacent callers, since it takes the Play too. The TODO at
`:19-21` closes. `go` keeps `send_to_dest(Graveyard)`: it discards **now**,
which is what 「将此卡置入弃牌堆并进入移动阶段」 orders, and a press has a bound
instance so it works (§7.1).

### 9.4 `placed-card-press-surface` (row 1614)

Before: the prefix filter, `has_play`, an id-derived source, and both client
policies on the same list.

After: §7.3-§7.7. A hand-only card (`anon_tokyo`, `haneoka`, …) does not set
`.field()`, so it never appears and its `[手]` body cannot be replayed from the
field. A card with both halves keeps one Play and one gate — kanon's jump and
nanami's (2) are prompts, not second buttons — so the list cannot grow past
"one action per pressable instance".

### 9.5 `hina-clause-2-swap-missing` (rows 930-932, 936, 972)

「本卡」 is the stand-in, banished once its substitute lands, so the clause
attaches to the substitute and never fires again. It is **passive**: it is
resolved at the moment the substitute lands, and thereafter it is a property of
the instance — the effective-card marker (§6.2). No clause is added to the field
list; what changes is *which card the instance runs*, so a press on it runs the
stand-in's effect, not the substitute's own.

Before (`hina_sound.rs:17-20`): one entry, no marker, no surface, and the
`[共鸣]` fuel of (2) never offered (row 972).

After:

```rust
use bandori_std::counter;
const ID: &str = "PP:[冰川日菜]会发出怎样的声音呢？";

On::Play("", None, hina_sound).field(),

fn hina_sound(player_id: i32) -> Asked {
    // Pressed on a marked substitute: resolve the changed effect only.
    let here = ctx::self_uid();
    if here >= 0 && ctx::effective_card_at(here).is_some() {
        return hina_change(player_id, here);
    }
    // …the existing placement body…
    let before = instance_floor(player_id);
    // …ask, log, unplace-free placement of the substitute (below)…
    let Some(instance) = new_instance_since(player_id, &before, sub) else { return Ok(()) };
    // 规则书（2）「[共鸣]」: the change costs the fuel, and is passive -- asked
    // here, at the only moment 「本卡在[场地]」 can hold, and applied on yes.
    if crate::resonance::try_resonance(player_id)? {
        ctx::set_effective_card_at(instance, ID);
        ctx::set_created_by_at(instance, -1);   // G08; TODO until it lands
    }
}

/// The changed effect (OPEN-substitution-effect): re-pick the substitute, carry
/// the crystals across. The marked instance is the one being replaced.
fn hina_change(player_id: i32, instance: i32) -> Asked {
    let crystals = ctx::counter_at(instance, counter::CRYSTALS);
    // The *own* card is the current substitute; `effective_card_at` is the
    // stand-in, so it cannot answer this.
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
* The `here` branch is the one body-level origin test left, and it is about a
  *marker*, not about `is_placed()`: a press always has `self_uid >= 0`, but only
  a marked instance is the stand-in's surface.
* `[共鸣]` spends through `resonance::try_resonance` (the fuel card, its ask and
  its two crystals). It is offered once, when the substitute lands — the same
  shape 千圣's (2) uses on its turn-start offer — so a decline leaves the
  substitute running its own effect, and no [共鸣] is asked again until the
  effect is used again.
* The crystals are read as a plain counter and re-added at the new instance —
  the same read/write pair `card-mujica/src/crystal_swap.rs:49,56` uses. They
  are the substitute's own (its `[手]` filler), so they travel with the change.
* `set_dest(Banished)` at the body's tail is harmless in both arms: the tail
  applies to the run's `current_uid`, which `hina_change` has already unplaced,
  so the substitute it just placed stays.
* New locale keys `hina_sound_swap_ask` (zh/en); `hina_sound_title` is reused.
  Row 930/932's fourth substitute stays reachable — the placement arm maps it to
  `place_card`, and the change arm too. A `TODO(规则书)（此卡在符合使用条件时可
  替代…）` stays on the ask: the per-substitute eligibility filter is still
  unimplemented.
* The content of 「改变…效果」 is unprinted (pp.md `OPEN-substitution-effect`).
  The re-pick above is the interim reading; §14 records it.

## 10. Save, record, replay

* **Save:** `TurnCtx.pressed` and `FieldCard.effective_card` are
  `#[serde(default)]`; **SAVE_VERSION 14 -> 15** (`engine/mod.rs:398`). The
  declare argument reuses `play_arg`, so it adds nothing.
* **Record:** the new `NetMessage.instance` is `skip_serializing_if`, so a body
  written before v111 re-serializes byte-identically and `body_check` stays
  equal. `RECORD_VERSION` stays 4.
* **Replay:** an old `act: "skill"` replays through the alias — first instance,
  the card's Play — which is exactly what it did. It does **not** replay the
  same *outcome* for the cards the design rewrites: kanon's old record runs the
  branch by `is_placed()` as before, but nanami's old field play called
  `play_card` into the skill's (3) and now prompts for (2), and even-lost is
  reached at all for the first time. That difference is the rulebook fix itself
  and is accepted; the prompt is recorded in the event log, so a replay answers
  it identically rather than re-deriving it.
* **Event log:** new presses emit `MatchEvent.kind = "press"`; old `"skill"`
  events still render through the client's default arm.
* **Bots:** `turn.pressed` is saved so a restored seat does not loop on a no-op
  press; it never gates a human.

---

## 11. Versioning

**ABI_VERSION 110 -> 111** (eighth group in the README landing order; take the
next free number if the order moves). Wire breaks: the new imports and the removed
guest declaration of `invoke_skill` (a name-keyed import; its host alias keeps
pre-v111 wasm linking, and `play_card` does not move). No `OnKind` is added —
`.field()` rides `OnKind::Play` and is `serde(default)`, so an old host still
parses the manifest, it just never offers a press. Changelog block, after the
`pub const ABI_VERSION`
line, in the v104 shape:

```rust
/// v111: **the press surface for placed cards**. A `[持续]`/`[场]` clause a
///    player may activate on a placed card is the card's own `On::Play`, run on
///    the field instance (`.field()` on the Play). Requests say which instance
///    (`act: "press"`, `NetMessage.instance`; `act: "skill"` is an alias
///    resolving the first instance). `entry::PRESS` resolves the card's field
///    Play, else its first Play; `"press"` stays undeclarable. A press is not a
///    `cardUsed` activation. New imports `effective_card_at` /
///    `set_effective_card_at` name the card a stand-in's instance runs.
///    Shared with G13: `invoke_entry` / `invoke_frame`, guest
///    `ctx::invoke_entry` / `invoke_entry_paid` / `invoke`; the guest
///    declaration of `invoke_skill` collapses onto them (`ruleset_invoke_skill`
///    stays a host alias; `invoke_skill` is sugar over
///    `invoke_entry(…, entry::PRESS)`; `play_card` keeps its import).
///    **SAVE_VERSION 14 -> 15** (`TurnCtx.pressed`,
///    `FieldCard.effective_card`; both `serde(default)`). RECORD_VERSION
///    unchanged (`NetMessage.instance` is `skip_serializing_if`; old
///    `act: "skill"` records replay via the alias).
```

`docs/ACTIVATION.md` §3's `entry::PRESS` paragraph keeps the spelling and gains
the field-Play-first/any-Play-fallback rule; the borrow examples (`nfo`,
`please_choose`, `detour`) keep the spelling and gain a note that the answer now
names the field Play first. `docs/rulebook/spec/abi/README.md`'s G05 item count becomes 5 (one
shared with G13), and the consolidated-map entry marks the nine mis-filed rows.

---

## 12. Tests

Ruleset tests (`rules/tests/tests/common/mod.rs` gains `Table::press(who,
instance)`; `Table::skill` delegates):

* 1 (nanami): `tests/morfonica.rs::nanami_third_prompts_and_invokes_skill_two`
  (answer yes: crystal removed, pot spent, mark gained, CD set, card stays
  placed, the invoked frame pushed and popped);
  `…_third_prompt_declined_keeps_the_crystal`; `…_third_press_waives_cap_and_cd`
  (CD 3 preset, mark already held, 3+ alive; still gains);
  `…_third_without_pot_skips_the_prompt` (no prompt is raised);
  `…_third_without_crystal_skips_the_prompt`; `…_third_uses_the_second_copy`
  (the crystal and the mark belong to the newly placed instance).
* 2 (kanon): `tests/hhw.rs::kanon_jump_prompts_at_nine`;
  `…_jump_not_prompted_below_nine`; `…_jump_declined_then_reoffered_after_a_move`;
  `…_jump_needs_main_move`; `…_hand_play_places_and_does_not_prompt`;
  `…_jump_prompts_for_the_second_copy` (markers are one pool; the instance is
  the prompted copy).
* 3 (even-lost): `tests/mygo.rs::even_lost_press_discards_and_moves_hand_size`;
  `…_press_needs_main_move`; `…_hand_only_card_not_offered` (a card without
  `.field()`).
* 4 (surface): `crates/game-core/tests/view_extra.rs::press_list_has_instance_label`,
  `…_hand_only_play_not_offered`, `…_stale_instance_refused`,
  `…_pressed_instance_not_listed`; `tests/engine.rs::press_unknown`,
  `…_press_not_now`, `…_press_raises_no_card_used` (a 七深 (2) copy stays
  unoffered when a plain card is pressed, and is offered when a skill is).
* 5 (hina): `tests/pp.rs::hina_swap_marks_the_substitute_instance`,
  `…_swap_declined_leaves_no_marker`, `…_swap_carries_the_crystals`,
  `…_press_on_a_marked_substitute_replaces_that_copy` (two stand-ins; pressing
  B changes B and leaves A alone), `…_unmarked_substitute_runs_its_own_effect`.
  Interface half with 4: `view_extra.rs::press_offer_uses_effective_card` (the
  marked substitute offers the stand-in's Play on the substitute's instance,
  `source` = `HINA_SOUND`) and `…_substitute_own_play_suppressed`.
* ABI/ruleset: `crates/game-rules/tests/ruleset.rs::field_bit_on_a_non_play_kind_rejected`;
  `crates/rules-native/tests/abi_link_names.rs` covers both effective-card ops.
* Record: `tests/record.rs::old_skill_record_body_check_unchanged` (golden hash)
  and `…_press_record_round_trip`.
* Invocation is G13's surface; G05 adds the press binding only:
  `tests/declare_arg.rs::invoke_frame_by_instance_is_pressed_instance` and
  `…_invoke_from_plain_card_and_skill`.

Kani obligations (`rules/kani`):

* `Op::invoke_entry` / `arm_invoke_entry` + the G13 record fields;
  `Op::effective_card_at` / `arm_effective_card_at`, `Op::set_effective_card_at`
  / `arm_set_effective_card_at` (347/348) with the slot array and tag bits;
  `arm_is_placed` instance-bound; `place_instance` helper.
* `proofs/mor.rs`: `nanami_third_yes_iff_at` — post = crystal -1, invoke
  recorded at the owner skill with `by_instance` = the placed instance and
  `paid = 0`, exactly one effect log, frame unchanged;
  `…_third_no_crystal_noop_at`; `…_third_no_pot_noop_at`;
  `…_third_binds_second_instance`.
* `proofs/hhw.rs`: `kanon_jump_at_nine_iff` (pool cleared, teleport plan, dest
  graveyard), `…_jump_below_nine_noop`.
* `proofs/pp.rs`: `hina_mark_iff` — post = exactly one new instance with
  `effective_card = "PP:[冰川日菜]会发出怎样的声音呢？"` and the carried crystal
  count, the fuel row gone, a second marked copy unchanged;
  `…_mark_without_resonance_noop` (world snapshot equal).
* Invariant: `press_is_placed_is_instance_bound` — with two field instances,
  `is_placed` is true only while `last_uid` is one of them.

## 13. Alternatives

**A1 — filter drop only (no ABI).** Delete the `skill:` test and derive
`source`. Cheapest; satisfies items 2 and 3's stated expectation, which is why
they carry a stopgap. Fails item 4 (hand-only cards replay their hand body from
the field), has no instance, and cannot name a clause, so item 1 stays
impossible.

**A2 — a card-wide `prop::FIELD_PRESS` data prop.** The bit lives on the card
rather than the entry. Nearly the same as the chosen `.field()`, but a prop is
per-card *instance* state and must be seeded from the manifest anyway, so the
only difference is where the bit is stored; the entry is the smaller place.

**A3 — a new `OnKind::Press = 15`.** The press gets its own entry type, so a
card can carry a hand Play and a press side by side with independent ids and
gates; kanon and nanami keep two buttons. What it buys is the second clause
being directly pressable; what it costs is a kind, a `PressEntry` struct, a
`press_*` family on `CardRules` and `Ruleset`, an `entry` on the wire, and a
press id on every card — for three clauses that turn out to be prompts and one
that turns out to be a passive marker. This was the first draft here; the
Play-reuse is smaller for the same five rows. Re-open if a card prints two
genuinely activatable clauses.

**A4 — raise-based press.** `ctx::raise` a new `cardPressed` kind and let a hook
run the body; the client sends only `act: "press"` with the card id. No
`Call::Press` / `CardRules` additions. But a raise is a synchronous fan-out with
no return `Dest` and no declare barrier; `drive` gives both for a resolved Play.
It would also make the press a raise, which §7.4 currently avoids on purpose.

**A5 — declare (2) on each substitute card instead of a marker.** The stand-in's
[共鸣] clause would live on 丸山彩, 大和麻弥, 白鹭千圣 and 若宫伊芙, found by
graphing the stand-in. Saves §6.2 and its two imports, but the substitute is a
shared card: its (2) would apply on every copy, not only where it stands in for
日菜, so the clause needs a condition that re-derives "do I stand in" — the
origin guess again, one card further away. 大和麻弥 makes it worse: its own
`On::Play` is already its [共鸣] pile-swap, so a second [共鸣] clause lands
beside a different one on the same card. It also cannot move the crystals of
「本卡」 to the right copy, which is the instance half. Kept only if the marker
is judged too much state.

**A6 — inline (2)'s body in the effort card.** No invocation at all for item 1:
the effort body copies the skill's clause. Cheapest for nanami alone. But
「视为发动你的（2）技能」 names the skill's clause, so the copy drifts on the
skill's next erratum, and the gate must be hand-written from the same terms
anyway. The other four items still need the surface.

**A7 — no prompt; a standing button for (3) and the jump.** What the first
draft had. Simpler to test (no answer to record) but wrong on the printed shape:
(3) is a one-shot 「你可以」 inside its own resolution, and the jump is a
condition 「若…可以」; neither is a button the player holds.

---

## 14. Open questions

1. **C-Q46 / 「不受数量或轮数限制」.** `invoke_entry` never evaluates the
   target's gate, so the waiver is structural rather than a flag. It does not
   waive the *cost*: (2) spends a pot inside its body, so the prompt checks a
   pot first (§9.1). Confirm the waiver covers the 「每种角色标记上限1」 cap and
   the 3-turn CD (assumed yes).
2. **Is (3)'s crystal offer one-shot?** Read as printed — asked as part of
   resolving (3), never again. If it is re-usable later, it needs a field bit on
   the effort card instead of an inline prompt.
3. **Kanon's prompt points.** `markChanged` fires when the count moves and
   `MoveResolved` re-offers after a decline (the sheet's 「主要阶段中」 is a
   standing window, which one event cannot express). Is the re-offer enough, or
   does a declined prompt need to survive to the next main phase?
4. **Nothing negates a press.** §7.4 drops the `cardUsed` raise, so a [反击]
   answers what the body does, not the activation. Is that right for every
   printed [反击] (花音's jump, 日菜's change), or is a `cardPressed` trigger
   wanted after all?
5. **Opponent presses.** Every press is offered to the instance's holder. No
   text today lets another seat press; if one appears, `cant_act` needs a holder
   override and the request a seat.
6. **`send_to_dest` on a press.** It unplaces the run's instance at once and
   files the run's **card id**; §7.1 makes that the instance's own card. An
   immediate discard and a deferred `set_dest` therefore differ only in timing.
   Confirm no clause needs the immediate form's ordering (even-lost's does, and
   keeps it).
7. **`Dest::Field` on a press.** Documented no-op (already placed); a re-place
   press must `unplace_at` first. Confirm no clause wants a "return to field"
   press.
8. **`entry::PRESS` with a multi-clause card** resolves the field Play; other
   clauses are queried by slug. Is "first" the right default alias, or should
   `PRESS` mean "any activatable entry admits" for `query_guard`?
9. **`skill_blocked(player, "")`.** Skill presses keep the skill's own gate;
   plain-card presses do not consult it. Should a global 「技能不能使用」 status
   also block a card-invoked skill (an ungated `invoke_entry`)? Assumed the
   card's gate decides.
10. **Effective-card scope.** §6.2 resolves it for press reads. Should the same
    resolution apply to a body's `query_guard` / `invoke_entry` reads? Assumed
    unchanged for `query_guard` (bodies address by slug).
11. **Temporary pots.** `fire::total` admits a payment by a temporary pot alone.
    Confirm that is intended (`spend_skill` draws temp first, so it is payable)
    rather than "a held pot".
12. **OPEN-substitution-effect.** What 「改变本卡代替的专属卡效果」 changes the
    effect *to* is unprinted (pp.md). The re-pick in §9.5 is the interim
    reading; when the text arrives it replaces `hina_change`'s body, and possibly
    needs an entry of its own.
