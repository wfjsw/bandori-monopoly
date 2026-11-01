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

Terminology, fixed here for the whole document:

* a **press** is a player activation of a printed clause of a card that is
  already on the field — a skill button and a placed card's 「可以…」 clause are
  the same mechanism;
* the **instance** is the `FieldCard` row (`FieldCard.uid`); a press is
  addressed by instance, never by re-finding a card id;
* the **effective card** of an instance is the card whose printed surface the
  instance runs (`FieldCard.effective_card`, §6.2); it is the instance's own
  card unless a stand-in wrote the marker (§9.5);
* an **entry** is one printed clause, named by its required slug `id`.

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
resolve "the card's first `On::Play`" (`invoke_skill` via the PRESS alias, which
has no `On::Press` to find), so (2) is unreachable by id. Two further
consequences sit in the same rows:

* the crystal is spent *before* the body, so a press with a crystal and no mark
  burns the crystal and does nothing (row 734's probe);
* 「此次技能不受数量或轮数限制」 waives (2)'s per-kind mark cap and 3-turn CD;
  nothing evaluates them today, so nothing is waived (row 735).

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

and the body branches again at line 62. The press is on no surface at all: the
view lists `skill:*` instances only (`engine/mod.rs:1416`). Rows 610-612 add
that the jump only writes the turn plan and never runs it (this tree already
calls `run_planned_move`, so those three are closed; they stay listed because
they were the original evidence). Row 621's main-move gate and row 620's
second-copy case are still open.

### 1.3 `even-lost-not-pressable` (rows 798, 801)

Book (`MyGO:即使迷茫着`, clause (2)):

> （2）[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。

The card is never played from hand — it enters through its own [反击] (1) — so
its only `On::Play` is the press body and it is on no press surface
(`rules/cards/card-mygo/src/even_lost.rs:19-21`). `can_go` additionally asks
`!ctx::is_placed()` and never asks whether the main move is available, so a
press after the main move cleared the card for nothing (row 798).

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
(`:73`), so 「本卡在[场地]时」 can never hold again. The [共鸣] clause therefore
lives on the *substitute* from then on, and only an instance-bound press can
reach it. Rows 930/932 also record that `SUBS` lacked 大和麻弥 — the one substitute with
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

**Every row is printed text, not a preference.** The sheet puts a
player-activated clause on a placed card four times: nanami 「你可以移除一个
奇迹水晶视为发动你的（2）技能」, kanon 「可以清除所有标记并传送到#4水族馆或者
#30弦卷豪宅」, even-lost 「你可将此卡置入弃牌堆并进入移动阶段」, hina 「本卡在
[场地]时改变本卡代替的专属卡效果」. Today two are unreachable and two are
wrong. A placed card with a switchable clause is a whole category of card, not
four oddities, so leaving it unimplemented is a functional hole the triage
graded high.

**No smaller fix reaches the text.** The three plausible retrenchments, each
with what it buys:

| retrenchment | buys | still fails |
|---|---|---|
| A1, drop the `skill:` filter (§13) | kanon and even-lost appear; their wrong gate is reached | their body still branches on origin, every `[手]` card replays from the field, no clause can be named — nanami stays impossible, and any two-clause card stays a guess |
| A2, a card-wide `prop::FIELD_PRESS` flag (§13) | the same two buttons, without surface churn | "which clause" is still positional, so nanami cannot name (2) and kanon cannot offer the jump independently of the placement; the body still branches |
| card-side only, no ABI | nothing | a card cannot teach the engine to list it, cannot pass an entry to `play_card`, and has nowhere to record "this instance runs the stand-in's surface" |

**Three printed demands converge on one primitive, which is why they are one
group.** Clause identity (nanami names 「（2）」; kanon and hina print two
clauses on one card), instance identity (kanon's second copy, hina's two
substitutes, nanami's mark reads), and surface substitution (hina) are each
unsatisfiable without an addressable, instance-bound activation. Fixing them
one at a time means a partially shaped kind that must be reshaped for the next
card, plus three migrations of the same call sites.

**Why a new kind rather than more `On::Play` (A3).** The sheet itself splits
手 and 场; the engine already gives `On::Play` that meaning inside `play_card`;
the origin guess *is* the defect of §2.1. Reusing the kind keeps the guess and
moves it into five call sites (`entry(OnKind::Play, None)` everywhere) while
still needing slugs, an instance and a surface — the ABI cost is paid either
way, only without the distinction that motivated it.

**Two narrower routes were considered and are worse.** Inlining (2)'s body into
the effort card (A6) avoids the press *for item 1 alone*, but duplicates the
skill's body, so the sheet's 「视为发动你的（2）技能」 — which means the skill's
clause, including its later errata — drifts from the copy. Putting (2) on each
substitute card (A5) avoids the effective-card marker, but the substitute is a
shared card, so its (2) would be offered on every copy rather than only where
it stands in for 日菜.

**The cost is bounded and mostly already paid.** One manifest kind and two
optional request fields; `skip_serializing_if` keeps every stored `body_check`
byte-equal; two `serde(default)` turn fields; one client sender, one animation
arm, two policy scanners — all reading a list that already exists. No import
gains a player decision, and no card outside the four defect owners changes.

**G13 is in the same window and needs the same shape.** The skill invocation is
this group's item 1, and `invoke_entry` is G13's surface. Landing press later
would mean writing `invoke_entry` press-less and re-cutting its frame.

**What would make the group unnecessary.** A ruling that placed cards are not
pressable collapses it; the sheet says otherwise. If only items 2 and 3 were
wanted, A1 can ship first (§1.2, §1.3) and the group would remain for items 1,
4 and 5 — which is the staging the review can use, not an argument that the
group is optional.

---

## 3. Capability as an invariant

**An entry is a clause id, and a press is an entry of a placed instance
addressed by `(instance, entry)` — never a card-wide yes/no and never an origin
guess.**

Corollaries, all of which the surface below makes true:

1. One entry is one printed clause. A hand play and a field activation of the
   same card are different entries (`On::Play` vs `On::Press`), so no body
   branches on `is_placed()` / `play_from_hand()` and no gate consults
   `hand_count()`.
2. A press entry always carries a slug `id` — the identity type
   `docs/ACTIVATION.md` §3 already defines for `query_guard`. Its label
   (「（2）」) is display text.
3. The pressed instance **is** the request's `instance`. Dispatch validates
   that it is in play, on the presser's field, and carries the requested
   effective card; it never re-finds by id. Every instance read (`is_placed`,
   named counters, `crystals`, props, `dest`) answers for that instance.
4. The offer is enumerated: for every field instance, every press entry of its
   effective card is offered and gated through the same `cant_act` a request
   takes, so "shown enabled" ⇔ "accepted". The only prefix test left is the
   temporary skill alias (§7.3).
5. A press is an activation, not a hook: it is not reentrancy-marked, it raises
   a card-activation category, and a counteraction may negate it before the
   body.
6. Cross-card invocation names the entry:
   `ctx::invoke_entry(player_id, card_id, entry_id)`. `ctx::invoke_skill` is
   sugar for it on `entry::PRESS`.
7. Same barrier order as a play: gate at offer, `declare` before the point of no
   return, body after; no re-gate at body time.

The invariant is what makes §1.1 expressible: "run clause (2)" is
`invoke_entry(skill, "2")`; "offer the jump" is a press entry on the instance;
neither is a card-level flag.

---

## 4. Surface — `card-sdk::abi` kind

```rust
pub enum OnKind {
    // …
    /// v111: a placed card's **press** entry (`On::Press(id, condition, guard,
    /// body)`). Addressed by the field instance and the entry's required `id`;
    /// the request raises `cardUsed` (the category `use_skill` raises today).
    /// Id 15 — 4/5/6 stay retired forever.
    Press = 15,
}
```

`OnKind::from_i32` gains `15 => Self::Press`.

`abi::entry::PRESS` (`abi.rs:1153-1162`) keeps its spelling and becomes the
**alias** for a card's default press entry: its first `On::Press`, or — for a
card not yet migrated — its first `On::Play`. `"press"` stays undeclarable.

No new export op: a press entry uses `OP_GUARD` / `OP_DECLARE` / `OP_RUN`
exactly as a play entry does. `ManifestOn` is unchanged; `id` is required for
kind 15 (validated at build, §7.1) and `triggers` stays empty.

**`entry::PRESS` answers can change for a migrated card.** A card that gains a
press entry *and* keeps a Play now resolves `PRESS` to the press. Cards that ask
`query_guard(PRESS)` to decide a hand activation must read their Play by entry
id after migration; §12 pins `detour`/`nfo`/`please_choose` for it.

---

## 5. Surface — `card_sdk::lib::On::Press`

A new entry struct modelled on `PlayEntry`, without the settle `texts` (a press
is never a tile text):

```rust
/// A placed-card **press** entry: a clause a player may activate while the card
/// is on their own field. `id` is required and is identity, not display text.
#[derive(Clone, Copy)]
pub struct PressEntry {
    pub id: &'static str,
    pub condition: &'static str,
    pub counter_reason: Option<MsgKey>,
    pub guard: Option<fn(player_id: i32) -> Option<Msg>>,
    pub body: *const (),
    pub drive: BodyDrive,
    pub shape: BodyShape,
    pub declare: Option<*const ()>,
    pub declare_drive: Option<DeclareDrive>,
    pub label: &'static str,
    pub flash: crate::abi::Flash,
    pub header: crate::abi::Presentation,
}
```

Constructor, next to `On::Play` (`lib.rs:870`) and shaped like `On::Schedule`
(`lib.rs:1066`, the existing required-id constructor):

```rust
impl On {
    /// Press this placed card: run `body` when the player activates clause `id`
    /// of the instance. `condition` / `guard` are the gate — the same pair, same
    /// meaning and same evaluation order as [`Self::Play`]; `body` is the
    /// effect. The press is offered to the instance's holder only, in their
    /// operations stage, and leaves the card where it is unless the body sets a
    /// `dest`.
    ///
    /// `id` is a required per-card-unique ASCII slug
    /// (`[A-Za-z0-9_][A-Za-z0-9_.:-]*`). It is the clause's address for the
    /// press request and for `ctx::query_guard`; display text is `.label("（2）")`.
    /// `"press"` is reserved (see [`abi::entry::PRESS`]).
    #[allow(non_snake_case)]
    pub const fn Press(
        id: &'static str,
        condition: &'static str,
        guard: Option<fn(player_id: i32) -> Option<Msg>>,
        body: fn(player_id: i32) -> Asked,
    ) -> Self { /* PressBody(PressEntry { … }) */ }
}
```

`On` gains `PressBody(PressEntry)`. The `on_entry_field!` arms, the `OP_GUARD`
arm, `declare_of`, the body-union arm, and the accessors `condition` /
`has_guard` / `has_declare` / `id_of` / `label_of` / `header_of` each gain a
`PressBody` arm with the identical `PlayBody` field reads. `On::kind()` maps it
to `OnKind::Press`; `On::triggers()` is empty.

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
the same core with a **Play-first** lookup
(`entry(OnKind::Play, None).or_else(default press)`), because `ctx::play_card` on
a card with both entries means "play this card", not "press it".

The frame's `entry` is always the **resolved** entry's declared id (`""` when
that entry is unnamed, e.g. a `play_card` into a bare `On::Play`), never the
alias spelling. Consequently `entry::PRESS` resolves press-entry-first with an
`On::Play` fallback, and the fallback is what keeps `query_guard(PRESS)`
truthful for unmigrated skills.

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

`effective_card_at(instance)` names the card whose **press surface** the
instance runs; it is not a rename of `FieldCard.card`, so counters, props and
`dest` stay the instance's. `set_effective_card_at(instance, "")` clears. The
marker needs a live instance and a declared id and does not validate the pair.
Field:

```rust
pub struct FieldCard {
    // …
    /// The card whose printed press surface this instance runs (`None` = its
    /// own). Written by a stand-in, dies with the instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_card: Option<String>,
}
```

`SAVE_VERSION` moves for the `TurnCtx` fields in §7.6, not for this.

Every read of an effective card on a placed instance
(`effective_card_at(instance).unwrap_or(own)`) is this group's: the press
request, the offer enumeration, and the press gate. Until G05 lands nothing
reads it.

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
    /// `On::Press` — a placed card's activation. `instance` is the field row,
    /// `entry` the manifest index resolved by `WasmRules::press`; `arg` is the
    /// argument its `OP_DECLARE` collected.
    Press {
        card: i32,
        player_id: i32,
        instance: i32,
        entry: i32,
        arg: Vec<u8>,
    },
}
```

`Call::player_id()` / `Call::card()` include it. `Ruleset::run` resolves
`Call::Press { entry, .. } => Some(entry)`, takes `arg` like `Play`, and keeps
Press **out** of the body-time admission block (the gate runs at offer).
`drive_inner`'s reentrancy marker becomes
`uid >= 0 && !matches!(call, Call::Play { .. } | Call::Press { .. })`
(`wasm_rules.rs:3294`). `announce_drive` maps it to `card_trigger::PRESS` and
the 「发动」 wording (`wasm_rules.rs:3144-3153`).

New `Ruleset` methods (mirrors of the play ones):

```rust
/// Does this card declare a press entry? (Static, no world.)
pub fn has_press_entry(&self, card: i32) -> bool;
/// Press entries as `(entry index, id, label)` in declaration order.
pub fn press_declarations(&self, card: i32) -> Vec<(i32, &str, &str)>;
/// The press gate for one entry, from a world copy with `current` = the
/// instance. `Ok(None)` = pressable.
pub fn press_cant_act<W: CardWorld>(
    &self, world: &W, card: i32, entry: i32, player_id: i32,
) -> Result<Option<crate::Msg>, RuleError>;
/// Cheap pre-filter mirroring `play_gate_vanishes`.
pub fn press_gate_vanishes(&self, card: i32, entry: i32) -> bool;
```

`press_cant_act` is `cant_play`'s shape with three differences, all required by
the invariant: the entry is given (not first-Play), the candidate is built with
`condition::fill_candidate_uid(world, player_id, card, instance, true)` so
`card.counter(…)` reads **that** instance, and the guard `Run` carries
`current_uid = instance`.

**Note — the nanami (2) condition does not apply to a self-press.** (2)'s
condition is `actor != owner && trigger_card != 0 && charge(owner) >= 1`, and
`fill_candidate(…, player_id, …)` sets the candidate's `owner = player_id`, so
for the owner's own press `actor != owner` is false. A press gate must
therefore not be built from `query_guard(skill, "2")`; nanami's press gate tests
its own costs directly (§9.1). The invoked body is not re-gated, which is what
waives the cap and CD.

`Ruleset::build` (`host.rs:1086-1120` region) adds, next to the `Schedule`
rules: a kind-15 entry must carry a non-empty slug `id`, the slug must pass
`is_entry_slug`, must not be `"press"`, and ids stay unique per card. Error
`RuleError::BadPressId(String)` (a `Display` arm naming card and entry index).

`WasmRules::press` (the `CardRules` method; `play` at `wasm_rules.rs:10201` is
the model):

```rust
fn press(&self, cx: &mut PlayCtx, player_id: usize, card: &str, instance: i32, entry: &str)
    -> Flow<Dest>
```

1. `ruleset.card(card)`; unknown -> `log.card_not_ported`, `Dest::Graveyard`.
2. Validate `field_by_uid(instance)`, `holder_of(instance) == player_id`,
   and the effective card of the instance `== card`; else log and no body.
3. Resolve `entry_by_id(entry)`, or for `""` / `entry::PRESS` the first press
   entry, else the first `On::Play`; none -> `DEST_UNSET` no-op.
4. Push the resolving frame; `drive(cx, Call::Press { …, arg: take(press_arg) },
   card, instance, &mut trigger)`.
5. `dest_from(dest)`.

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

The play fallback: `Ruleset::run`'s `Call::Play` arm and `hostfns::play_card`'s
lookup resolve `entry(OnKind::Play, None).or_else(first press entry)`, so
`ctx::play_card` on a migrated skill still runs.

### 7.2 `game-core` — request

```rust
pub struct NetMessage {
    // …
    /// `press`: the field instance. Absent = the `skill` alias (first instance).
    #[serde(default, skip_serializing_if = "is_zero_instance")]
    pub instance: i32,
    /// `press`: the entry slug. Empty = the default press entry (the `skill`
    /// alias and the `query_guard` alias).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entry: String,
}
```

`skip_serializing_if` is load-bearing: `body_check` hashes the re-serialized
`RecordBody` (`crates/game-core/src/record.rs:494-497`), so an always-emitted
field would change every stored hash. A private `is_zero_instance` helper sits beside `NetMessage`.
`Match::act` parses `"press"` at `mod.rs:2427` and dispatches it at `:2886`;
`"skill"` stays an accepted alias forever and is normalised to
`{ act: "press", entry: "press", instance: -1 }` before `cant_act`.

### 7.3 `game-core` — `CardRules`

```rust
/// Does this card rule declare a press entry? (Static.) `WasmRules`: a press
/// entry exists, or — until the skill migration lands — the id is a `skill:`
/// with `has_play` (the compat arm; no other prefix test remains).
fn has_press(&self, _card: &str) -> bool { false }
/// Press declarations of `card` for the viewer list (static, cached). For an
/// unmigrated `skill:` card, one synthetic `entry::PRESS` row labelled with its
/// first `On::Play`'s label, so the button survives the migration.
fn press_declarations(&self, _card: &str) -> Arc<[PressDeclaration]> { Arc::from(Vec::new()) }
/// The press gate for `(instance, entry)`; `None` = pressable.
fn press_cant_act(&self, _cx: &PlayCtx, _player: usize, _card: &str, _instance: i32, _entry: &str)
    -> Option<Msg> { None }
/// Pre-commit argument (docs/ACTIVATION.md §2), as `declare_activation`.
fn press_declare(&self, _cx: &mut PlayCtx, _player: usize, _card: &str, _instance: i32, _entry: &str)
    -> Flow<Option<Vec<u8>>> { Ok(Some(Vec::new())) }
/// Run the press; returns the instance's dest.
fn press(&self, cx: &mut PlayCtx, player: usize, card: &str, instance: i32, entry: &str) -> Flow<Dest>;
```

`PressDeclaration { id: String, label: String }` is game-core's own shape (like
`TileTextDecl`); `WasmRules` builds it from the manifest. `StubRules` keeps the
defaults. Answers are memoised separately: `PlayCtx::press_memo` keyed
`(player, instance, entry)` with the same world-stamp reset as `PlayMemo`
(`cx.rs:593-648`), because two copies of one card differ per instance and the
`(player, card)` play memo would cross-talk. The stamp-reset precondition is
fatal here, so the memo is a new vector pair rather than a widened key (a `&str`
key plus an `i32` cannot reuse `PlayMemo`'s comparator).

Every `card` / `id` below is the instance's **effective card**. When a marker is
set the instance's own entries are replaced by the effective card's; slugs are
per-card-unique, so `(instance, entry)` stays unambiguous.

### 7.4 `game-core` — `Match::use_press`

Replaces `use_skill` (`play.rs:1164`):

1. Resolve the instance (`m.instance`, else first field instance whose effective
   or own card is `m.card`), its effective card, and the entry (slug, else
   `entry::PRESS`).
2. Raise `cardUsed` with `card` / `cards = [effective]` / `by_card = player`.
   `is_cancelled` -> `card_activated(PRESS, …, negated = true)` and stop.
3. `rules.press_declare`; `Decline` -> stop; `Halt` propagates.
4. Store the argument in `turn.press_arg`; `rules.press`;
   `turn.pressed.push(PressUse { instance, entry })`.
5. `card_activated(card_trigger::PRESS, …)` for the non-negated case.

The `turn.played` push and `play_drove_move` / `drop_spent_plan_shape` handling
stay exactly as `use_skill` has them (the coarse per-turn budget and the move
accounting are unchanged); `cards_played` is still not incremented. `use_skill`
becomes the alias arm.

### 7.5 `game-core` — `cant_act`

New `"press"` arm (`mod.rs:2927` region is the skill arm it replaces), order:

1. `!playing() || step != OPS || busy` -> `err.press_not_now`.
2. Resolve the instance: `field_by_uid` when `m.instance >= 0`, else the first
   instance of `m.card`; not on the viewer's field, unknown, or effective card
   ≠ `m.card` (when given) -> `err.press_stale`.
3. Entry: `m.entry`, else `"press"`; not declared (and not the `skill:`
   compat) -> `err.press_unknown`.
4. `Gate::OnBoard`, then `Gate::Act` with `card_act_exemption`.
5. `cx.rules_press_cant_act(i, card, instance, entry)`.

New locale keys `err.press_not_now` / `err.press_stale` / `err.press_unknown`
(zh/en). The `err.skill_*` keys stay: `MatchEvent.msg` in stored saves still
names them.

### 7.6 `game-core` — view and turn context

`view_extra_parts` (`mod.rs:1380-1447`): drop the prefix filter; for each
instance on the viewer's field, skip `face_down`, resolve the effective card,
and for each declaration push one action. `source` is the `skill:` owner when
the effective card has the prefix, else the effective card id. `title` / `text`
stay `ask.cardOption` card references; `label` is what the face renders.

```rust
pub struct SkillAction {
    /// The instance's **effective card**.
    pub id: String,
    /// Entry slug (`entry::PRESS` for a default press).
    pub entry: String,
    /// The field instance the entry belongs to.
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

The JSON key `skills` and the array order (field order, then declaration order)
stay. `MatchState` gains `pressed: Vec<String>` (`"<instance>:<entry>"`) filled
from `world.turn.pressed`, so both client policies can grey out a clause already
used; the field is additive and `serde(default)`.

`TurnCtx` (`world.rs:205`): `press_arg: Vec<u8>` (scoped like `play_arg`) and
`pressed: Vec<PressUse>` (`PressUse { instance: i32, entry: String }`), both
`#[serde(default)]`. Bot and client bookkeeping only — a press never gates a
human.

### 7.7 Client and bots

* `webui/src/scenes/board/Side.tsx`: the slot type gains `instance` / `entry` /
  `label`; `skillMarks(me, slot.instance)` finds by instance; the React key is
  `instance + ":" + entry`; `fire` sends `{ act: "press", card, instance,
  entry }`; the face shows `label`.
* `webui/src/scenes/board/anim.ts:941` grows `case "press":` beside
  `case "skill":` — the same banner and sfx for a `skill:` id, a new
  `anim.press` banner otherwise.
* `webui/src/core/types.ts`: `SkillAction` gains `entry` / `instance` / `label`;
  `MatchState` gains `pressed`.
* `webui/src/game/autopilot.ts`: `usableSkills` returns the action objects and
  filters `S.pressed`; presses become `{ act: "press", card: a.id, instance,
  entry }`; a non-`skill:` id with no `skills[id]` weight defaults to the
  neutral weight (pressed once per turn — the `pressed` filter is what stops the
  repeat). Documented as a policy change in `docs/BOT.md` §3.8.
* `crates/bot-core/src/policy.rs::ai_skill_choice`: scan `field_instances` ×
  `press_declarations` of the effective card, skip `turn.pressed` matches
  (`turn.played` stays the coarse budget), gate with `rules_press_cant_act`,
  keep the strategy weight keyed by card id, default non-`skill:` ids to once
  per turn; the chaos branch presses every enabled declaration once. Returns a
  `NetMessage` with `instance` / `entry` set.

### 7.8 Classification

| # | surface | class |
|---|---|---|
| 1 | `OnKind::Press = 15` + required press ids | NEW / **WIRE-BREAK** (an old host rejects kind 15) |
| 2 | `On::Press` / `PressEntry` / builder arms | NEW (source) |
| 3 | G13 imports; guest `invoke_skill` becomes sugar | NEW / **WIRE-BREAK** (guest import `invoke_skill` removed; its host alias and all of `play_card` stay) |
| 4 | `Call::Press`; `WasmRules::press`; `Ruleset::press_*` | NEW (internal) |
| 5 | `CardRules::has_press` / `press_declarations` / `press_cant_act` / `press_declare` / `press` | EXTENDED trait |
| 6 | `NetMessage.instance` / `.entry`, `act: "press"` | EXTENDED wire; `skill` accepted as alias |
| 7 | `SkillAction.instance` / `.entry` / `.label`, `MatchState.pressed`, `skills` contents | EXTENDED view |
| 8 | `TurnCtx.pressed` / `press_arg` | EXTENDED save (**SAVE_VERSION 15**) |
| 9 | `FieldCard.effective_card` + ops 347/348 | EXTENDED save / NEW imports |
| 10 | `card_trigger::PRESS` | EXTENDED event string (replay-safe) |

---

## 8. Engine wiring

### 8.1 Request path

`Match::act` routes `"press"` to `use_press` after `cant_act` has run the same
checks in the same order, exactly as every other turn action does. `"skill"` is
normalised to the press alias before the gate, so one code path serves both.

### 8.2 Gate, declare, commit

The gate is `press_cant_act` at offer. `press_declare` runs behind the
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

Follow G13 §5 (the routine replaces `hostfns::invoke_skill`
`hostfns.rs:1531-1600`, which `play_card` at `hostfns.rs:3045` reuses with its
Play-first entry) with the press additions marked:

1. Resolve `card_id`; find its first instance on the caller's field for the
   binding **and** `by_instance = the caller's current_uid` (for a press, the
   pressed instance); check `MAX_NESTING`.
2. Compute the entry: `entry::PRESS` -> default press entry, other ids ->
   `entry_by_id`; unknown/unnamed -> `DEST_UNSET` no-op.
3. `enter_invoke(card_id, instance)` (G13 §5.2); push a frame `{ card, entry,
   by = caller, by_instance, instance, paid, depth }`; seed the nested trigger
   from the frame (`card`, `cards = [card_id]`, `by_card = caller`). No raise:
   the press already raised `cardUsed` for its own line, and the seed is what
   makes `trigger::cards()` answer inside the body.
4. `OP_RUN` (after `OP_DECLARE` when `has_declare`; decline -> `EXIT_DECLINED`).
5. `leave_invoke`; pop the frame; return the nested `Dest`; restore fuel.

The gate is **not** evaluated (G13: body only). That is how 「此次技能不受数量或
轮数限制」 waives (2)'s CD and cap without a flag. The press gate still checks
the caller-side costs (crystal, pot — §9.1), and the body's `fire::spend`
remains the last line. No new `TriggerKind`.

### 8.5 Offer

`view_extra_parts` enumerates as §7.6. No prefix test except the compat arm in
`has_press`. `press_declarations` is cached on the engine (manifests are static
per match); a card with several declarations yields several actions, each gated
separately.

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

---

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

After — two entries, no branch, a named target:

```rust
On::Play("", Some(cant_play_from_hand), nanami_effort),
On::Press("invoke_skill_two", "", Some(cant_press), use_skill),

fn cant_play_from_hand(player_id: i32) -> Option<Msg> {
    let names = ctx::held_names(player_id, TOKEN_PREFIX);
    if names.iter().any(|n| ctx::held_count(player_id, n.as_str()) > 0) {
        return None;
    }
    Some(Msg::new(key!("nanami_effort_no_tok")))
}

fn cant_press(player_id: i32) -> Option<Msg> {
    // 规则书（3）: the crystal is the cost, and the invoked (2) pays a pot. Both
    // must be there or the crystal is burned for nothing.
    if ctx::crystals() < 1 {
        return Some(Msg::new(key!("nanami_effort_no_crystal")));
    }
    if bandori_std::fire::total(player_id) < 1 {
        return Some(Msg::new(key!("nanami_effort_no_pot")));
    }
    if owner_skill(player_id).is_none() {
        return Some(Msg::new(key!("nanami_effort_no_skill")));
    }
    None // the (2) gate's own cap/CD are waived by the card text (§8.4)
}

fn use_skill(player_id: i32) -> Asked {
    let Some(id) = owner_skill(player_id) else { return Ok(()) };
    ctx::add_crystals(-1, 0)?;
    ctx::invoke_entry(player_id, id.as_str(), "2")?; // body only
    Ok(())
}
```

nanami's skill gains `.id("2")` on its `Action::CardUsed` entry
(`skill-personas/src/nanami.rs:58`); the label 「（2）」 stays. New locale key
`nanami_effort_no_pot` (zh/en). `nanami_effort_no_mark` stays for the hand path.
Both TODOs at `nanami_effort.rs:273-281` close, and the comment "`play_card` runs
its (3) body" is rewritten to "runs the invoked (2) body".

`fire::total` (pool + temporary pots) is the read, not `fire::get`: `spend_skill`
draws temporary pots first, so a temporary pot alone must admit the press.

### 9.2 `kanon-press-surface-abi` (rows 607, 608, 610-612, 619-621)

The stopgap (ship if the group slips): delete the `skill:` test in
`view_extra_parts` and derive `source` defensively. The card then appears and
its gate is reached. Known cost: hand-only cards appear too (item 4).

Under G05, split the entry:

```rust
On::Play("", None, place),                       // hand play: placement only
On::Press("jump", "", Some(cant_jump), jump),    // placed press: jump only

fn cant_jump(player_id: i32) -> Option<Msg> {
    if ctx::held_count(player_id, JELLY) < 9 {
        return Some(Msg::new(key!("kanon_march_need")));
    }
    // 「视为本次主要移动」 -- refused while that move is shut.
    rules_status::move_blocked(player_id)
}
```

* `place` is the old tail of the hand branch: `set_dest(Field)`, `place_card`,
  log, `add_held_by_name` — no `play_from_hand()` / `is_placed()` test.
* `jump` is the old jump without the marker re-check (the gate owns it) and
  without `is_placed`.
* The combined `cant_play` (`:41-57`) and both TODOs (`:45-48`, `:76-78`) go;
  the hand play gets `guard: None`.
* Bottom gate: hand play always admitted; the placed copy shows
  `kanon_march_need` below nine and `move_blocked` after the main move.

The markers are one name-keyed pool per player (`held_count` sums over the
binding rows), so the press does not split markers per copy; the instance
decides which copy jumps and is discarded.

### 9.3 `even-lost-not-pressable` (rows 798, 801)

Stopgap: the same filter drop. Under G05:

```rust
On::Press("go", "", Some(can_go), go),   // On::Play is deleted entirely

fn can_go(player_id: i32) -> Option<Msg> { rules_status::move_blocked(player_id) }
```

The `ctx::play_card` fallback (§7.1) keeps `ctx::play_card("MyGO:即使迷茫着")`
working for the [反击]-adjacent callers. The TODO at `:19-21` closes. `go`
keeps `send_to_dest(Graveyard)`: it discards **now**, which is what
「将此卡置入弃牌堆并进入移动阶段」 orders, and a press has a bound instance so it
works (§7.1).

### 9.4 `placed-card-press-surface` (row 1614)

Before: the prefix filter, `has_play`, an id-derived source, and both client
policies on the same list.

After: §7.3-§7.7. A hand-only card (`anon_tokyo`, `haneoka`, …) declares no
press entry and never appears, so its `[手]` body cannot be replayed from the
field. A card that is both hand-playable and field-activated (kanon, nanami)
has two entries and two independent gates.

### 9.5 `hina-clause-2-swap-missing` (rows 930-932, 936, 972)

「本卡」 is the stand-in, banished once its substitute lands, so the clause
moves to the substitute. That is what the effective-card marker is for, and what
§7.3's effective-card rule addresses: the press entry is declared on
`HINA_SOUND` and found through the substitute's instance.

Before (`hina_sound.rs:17-20`): one entry, no marker, no surface.

After:

```rust
use bandori_std::counter;
const ID: &str = "PP:[冰川日菜]会发出怎样的声音呢？";

On::Play("", None, hina_sound),
On::Press("swap_substitute", "", Some(cant_swap), hina_swap).label("（2）"),

// in hina_sound, before the ask:
let before = instance_floor(player_id);

// after the substitute is in place (both the MAMI and the play_card arm):
let Some(instance) = new_instance_since(player_id, &before, sub) else { return Ok(()) };
ctx::set_effective_card_at(instance, ID);
ctx::set_created_by_at(instance, -1);   // G08; TODO until it lands

fn cant_swap(player_id: i32) -> Option<Msg> {
    if ctx::hand_count(player_id, crate::resonance::RESONANCE.id) < 1 {
        return Some(Msg::new(key!("hina_no_resonance")));
    }
    None
}

fn hina_swap(player_id: i32) -> Asked {
    let instance = ctx::self_uid();                  // the substitute
    let crystals = ctx::counter_at(instance, counter::CRYSTALS);
    // The effective card names the current substitute; an unmarked instance
    // falls back to its own card (the entry is still reachable by id on it).
    let Some(cur) = surface_card_at(player_id, instance) else { return Ok(()) };
    let others: Vec<&str> = SUBS.iter().copied().filter(|s| *s != cur).collect();
    if !crate::resonance::try_resonance(player_id)? {
        return Ok(());
    }
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
  honest. `surface_card_at(player, instance) -> Option<String>` is the effective
  card, else the instance's own card from `ctx::field_instances(player)`. All
  three are free functions in this card file over existing imports; none is new
  ABI. The first floor is taken before the ask, so `HINA_SOUND`'s own
  instance — still on the field while its body runs — is excluded; the second is
  taken after the `unplace_at` and before the new card is placed, so the diff
  can only see the substitute.
* The crystals are read as a plain counter and re-added at the new instance —
  the same read/write pair `card-mujica/src/crystal_swap.rs:49,56` uses.
* `[共鸣]` spends through `resonance::try_resonance` (the fuel card, its ask and
  its two crystals). It runs **before** the substitute ask, so a declined [共鸣]
  leaves the current substitute untouched — and the swap's own ask therefore
  only appears after the cost is paid, which matches 「发动那张卡的[共鸣]效果」.
* `set_dest(Banished)` at the body's tail is harmless during the press: the tail
  applies only to an instance the run still holds, and the unplace/re-place
  leaves it holding none, so the new substitute stays. A **hand play** while a
  stand-in is already in play still binds that older copy (the `Ruleset::play`
  first-match binding, §2, root cause 3) — unchanged here, and the reason that
  cause exists.
* `[共鸣]` clause (2) is available on **both** the stand-in's own instance
  (while `hina_sound` is in play with a substitute beside it) and every marked
  substitute: the body reads `self_uid` instead of assuming.
* New locale keys `hina_sound_swap_ask` / `hina_no_resonance` (zh/en);
  `hina_sound_title` is reused. Row 930/932's fourth substitute closes via the
  swap route. A `TODO(规则书)（此卡在符合使用条件时可替代…）` stays on the ask: the
  per-substitute eligibility filter is still unimplemented, and now also
  unstamped for the swap ask.
* G01's earlier sketch spelled this `On::Action(Action::Actions(…))`; the press
  supersedes it (the `card.placed` condition and the `actor == owner` pre drop
  out — a press is always placed and owner-acted).

---

## 10. Save, record, replay

* **Save:** `TurnCtx.press_arg` / `pressed` and `FieldCard.effective_card` are
  `#[serde(default)]`; **SAVE_VERSION 14 -> 15** (`engine/mod.rs:398`).
* **Record:** the new `NetMessage` fields are `skip_serializing_if`, so a body
  written before v111 re-serializes byte-identically and `body_check` stays
  equal. `RECORD_VERSION` stays 4.
* **Replay:** an old `act: "skill"` replays through the alias — first instance,
  `entry::PRESS` — which after migration is the first press entry rather than
  the first `On::Play`. For a card whose old Play body branched on `is_placed()`
  (kanon, nanami's effort) the old record ran the field half; the alias runs the
  press entry, which is that same half, so the record still replays the same
  branch. It does **not** replay the same *outcome* for nanami's effort: the old
  field half called `play_card` into the skill's (3), the new one invokes (2).
  That difference is the rulebook fix itself and is accepted. A card that keeps
  its Play (all three here) still answers a genuine hand play.
* **Event log:** new presses emit `MatchEvent.kind = "press"`; old `"skill"`
  events still render through the client's default arm.
* **Bots:** `turn.pressed` is saved so a restored seat does not loop on a no-op
  press; it never gates a human.

---

## 11. Versioning

**ABI_VERSION 110 -> 111** (eighth group in the README landing order; take the
next free number if the order moves). Wire breaks: manifest `OnKind` 15 (an old
host's `from_i32` rejects it), the new imports, and the removed guest
declaration of `invoke_skill` (a name-keyed import; its host alias keeps
pre-v111 wasm linking, and `play_card` does not move). Changelog block, after the
`pub const ABI_VERSION`
line, in the v104 shape:

```rust
/// v111: **press entries for placed cards**. A placed card's printed
///    activation is its own `On::Press(id, condition, guard, body)` entry with a
///    required ASCII slug id; `OnKind::Press = 15` (4/5/6 stay retired).
///    Requests address the field instance and the clause by that id
///    (`act: "press"`, `NetMessage.instance` / `.entry`; `act: "skill"` is an
///    alias resolving the first instance and `entry::PRESS`).
///    `entry::PRESS` resolves the card's first press entry, falling back to its
///    first `On::Play` for an unmigrated ruleset; `"press"` stays undeclarable.
///    New imports `effective_card_at` / `set_effective_card_at` name the card a
///    stand-in's instance runs. Shared with G13: `invoke_entry` /
///    `invoke_frame`, guest `ctx::invoke_entry` / `invoke_entry_paid` /
///    `invoke`; the guest declaration of `invoke_skill` collapses onto them
///    (`ruleset_invoke_skill` stays a host alias; `invoke_skill` is sugar over
///    `invoke_entry(…, entry::PRESS)`; `play_card` keeps its import and is
///    Play-first host-side). **SAVE_VERSION 14 -> 15**
///    (`TurnCtx.pressed` / `press_arg`, `FieldCard.effective_card`; all
///    `serde(default)`). RECORD_VERSION unchanged (new NetMessage fields are
///    `skip_serializing_if`; old `act: "skill"` records replay via the alias).
```

`docs/ACTIVATION.md` §3's `entry::PRESS` paragraph and the
`On::Play("", Some(can_use), use_skill)` example move to `On::Press` plus the
press-first/Play-fallback rule; the borrow examples (`nfo`, `please_choose`,
`detour`) keep the spelling and gain a note that the answer now names the press
entry first. `docs/rulebook/spec/abi/README.md`'s G05 item count becomes 5 (one
shared with G13), and the consolidated-map entry marks the nine mis-filed rows.

---

## 12. Tests

Ruleset tests (`rules/tests/tests/common/mod.rs` gains
`Table::press(who, instance, entry)`; `Table::skill` delegates with the alias):

* 1 (nanami): `tests/morfonica.rs::nanami_press_invokes_skill_two` (crystal
  removed, pot spent, mark gained, CD set, card stays placed);
  `…_press_waives_cap_and_cd` (CD 3 preset, mark already held, 3+ alive; still
  gains, the invoked frame pushed and popped); `…_press_spends_one_crystal_only`;
  `…_press_without_crystal_refused`; `…_press_without_pot_refused` (the crystal
  is **not** burned); `…_press_binds_second_copy`.
* 2 (kanon): `tests/hhw.rs::kanon_press_jumps_and_discards`;
  `…_press_below_nine_refused`; `…_press_needs_main_move`; `…_hand_play_places_and_does_not_jump`;
  `…_press_second_copy_discards_that_copy` (the instance is the pressed one;
  the marker pool is per player).
* 3 (even-lost): `tests/mygo.rs::even_lost_press_discards_and_moves_hand_size`;
  `…_press_needs_main_move`; `…_hand_only_copy_not_offered`.
* 4 (surface): `crates/game-core/tests/view_extra.rs::press_list_has_entry_instance_label`,
  `…_hand_only_play_not_offered`, `…_stale_instance_refused`,
  `…_pressed_entry_is_listed`; `tests/engine.rs::press_unknown`, `press_not_now`,
  `press_negated_by_counteraction` (raise cancelled -> no crystal spent).
* 5 (hina): `tests/pp.rs::hina_press_swaps_sub_and_keeps_crystals`,
  `…_press_without_resonance_refused` (substitute and crystals unchanged),
  `…_press_binds_second_stand_in` (two marked substitutes; pressing B swaps B
  and leaves A alone). Interface half with 4:
  `view_extra.rs::press_offer_uses_effective_card` (the marked substitute offers
  `swap_substitute` on the substitute's instance, `source` = `HINA_SOUND`) and
  `…_substitute_own_entries_suppressed`.
* ABI/ruleset: `crates/game-rules/tests/kind_table_pin.rs` kind 15;
  `tests/ruleset.rs::press_without_id_rejected`, `…_press_id_reserved_rejected`,
  `…_duplicate_press_id_rejected`;
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
* `proofs/mor.rs`: `nanami_effort_press_iff_at` — post = crystal -1, invoke
  recorded at the owner skill with `by_instance` = the pressed instance and
  `paid = 0`, exactly one effect log, frame unchanged;
  `…_press_no_crystal_noop_at`; `…_press_no_pot_noop_at`; `…_press_binds_second_instance`.
* `proofs/hhw.rs`: `kanon_jump_at_nine_iff` (pool cleared, teleport plan, dest
  graveyard), `…_jump_below_nine_noop`.
* `proofs/pp.rs`: `hina_swap_iff_at(instance)` — offered only with resonance;
  post = that instance unplaced, exactly one new instance with
  `effective_card = "PP:[冰川日菜]会发出怎样的声音呢？"` and the carried crystal
  count, the fuel row gone, a second marked copy unchanged;
  `…_swap_without_resonance_noop` (world snapshot equal).
* Invariant: `press_is_placed_is_instance_bound` — with two field instances,
  `is_placed` is true only while `last_uid` is one of them.

---

## 13. Alternatives

**A1 — filter drop only (no ABI).** Delete the `skill:` test and derive
`source`. Cheapest; satisfies items 2 and 3's stated expectation, which is why
they carry a stopgap. Fails item 4 (hand-only cards replay their hand body from
the field), has no instance, and cannot name a clause, so item 1 stays
impossible.

**A2 — a `prop::FIELD_PRESS` card flag.** One card-level boolean marks "this
card is activatable from the field"; the view filters on it and the first
`On::Play` runs with the instance bound. Small diff, one data bit, no manifest
kind. But it is the card-wide yes/no the invariant rejects: a two-clause card
cannot address the second clause, the body still branches on `is_placed()`, and
a card with a hand play and a press shares one gate. Kept as the fallback if
kind-15 churn is judged too high.

**A3 — several `On::Play` entries plus an instance, no new kind.** Press entries
stay `On::Play` with slug ids; the surface offers the non-first entries and the
hand play offers the first. Avoids the kind and the skill migration, but
`entry(OnKind::Play, None)` (`play`, `play_card`, `invoke_skill`, `has_play`,
`play_gate_vanishes`) becomes origin-aware everywhere, and "the first entry is
the hand play" is positional again — the rule this design removes.

**A4 — raise-based press.** `ctx::raise` a new `cardPressed` kind and let a hook
run the body; the client sends only `act: "press"` with the card id. No
`Call::Press` / `CardRules` additions. But a raise is a synchronous fan-out with
no return `Dest`, no declare barrier, and cancellation could only abort by
convention; `drive` already gives all three for a resolved entry.

**A5 — declare (2) on each substitute card instead of a marker.** The stand-in's
[共鸣] clause would live on 丸山彩, 大和麻弥, 白鹭千圣 and 若宫伊芙, found by
graphing the stand-in. Saves §6.2 and its two imports, but the substitute is a
shared card: its (2) is then offered on every copy, not only where it stands in
for 日菜, so the clause needs a condition that re-derives "do I stand in" — the
origin guess again, one card further away. 大和麻弥 makes it worse: its own
`On::Play` is already its [共鸣] pile-swap, so a second [共鸣] clause lands
beside a different one on the same card. It also cannot move the crystals of
「本卡」 to the right copy, which is the instance half. Kept only if the marker
is judged too much state.

**A6 — inline (2)'s body in the effort card.** No press for item 1 at all: the
effort body copies the skill's clause. Cheapest for nanami alone. But
「视为发动你的（2）技能」 names the skill's clause, so the copy drifts on the
skill's next erratum, and the gate must be hand-written from the same three
terms anyway. The other four items still need the surface.

---

## 14. Open questions

1. **C-Q46 / 「不受数量或轮数限制」.** `invoke_entry` never evaluates the
   target's gate, so the waiver is structural rather than a flag. It does not
   waive the *cost*: (2) spends a pot inside its body, so the press gate checks
   a pot first (§9.1). Confirm the waiver covers the 「每种角色标记上限1」 cap and
   the 3-turn CD (assumed yes).
2. **One raise or two?** `use_press` raises `cardUsed` once for the effort card;
   the invocation seeds, and does not raise, the skill side. A tae 「抵消」 answers
   the press and cannot answer the skill separately. Is one window right?
3. **`skillUsed` vs `cardUsed`.** The alias keeps `use_skill`'s payload
   unchanged, so behaviour does not move; confirm no ruling names `skillUsed`
   separately.
4. **Opponent presses.** Every press is offered to the instance's holder. No
   text today lets another seat press; if one appears, `cant_act` needs a holder
   override and the request a seat.
5. **`send_to_dest` on a press.** It unplaces the run's instance at once and
   files the run's **card id**; §7.1 makes that the instance's own card. An
   immediate discard and a deferred `set_dest` therefore differ only in timing.
   Confirm no clause needs the immediate form's ordering (even-lost's does, and
   keeps it).
6. **`Dest::Field` on a press.** Documented no-op (already placed); a
   re-place press must `unplace_at` first. Confirm no clause wants a "return to
   field" press.
7. **`entry::PRESS` for a multi-press card** resolves the first press entry;
   other clauses are queried by slug. Is "first" the right default alias, or
   should `PRESS` mean "any press entry admits" for `query_guard`?
8. **`skill_blocked(player, "")`.** Skill presses keep the skill's own gate;
   plain-card presses do not consult it. Should a global 「技能不能使用」 status
   also block a card-invoked skill (an ungated `invoke_entry`)? Assumed the
   card's gate decides.
9. **Effective-card scope.** §6.2 resolves it for press reads. Should the same
   resolution apply to a body's `query_guard` / `invoke_entry` reads, so a
   substitute's default press is the stand-in's? Assumed unchanged for
   `query_guard` (bodies address by slug).
10. **`entry::PRESS` naming** once skills migrate: the word no longer means
    "`On::Play`". Rename to `entry::DEFAULT_PRESS` (wire spelling unchanged) or
    keep `PRESS` and let its doc carry the fallback?
11. **Temporary pots.** `fire::total` admits a press payable only by a
    temporary pot. Confirm that is intended (`spend_skill` draws temp first, so
    it is payable) rather than "a held pot".
