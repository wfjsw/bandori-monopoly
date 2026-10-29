# ABI design G13 — Skill invoke and copy context

**Status: draft for user review. Design only — no source edits, no commit.**
Worktree `/home/monopoly/web/.claude/worktrees/card-rulebook-review`, branch
`worktree-card-rulebook-review`. Group G13, family **"Skill invoke and copy
context"**.

Ground truth: `docs/rulebook/cards.json`; every fragment below was re-read from
that file on 2026-10-28 against the card id named beside it. The skill-clause
text (nanami's and tomori's (2)) is not in `cards.json` — it is the character
sheet, quoted here from the rule file that owns it.

**Closed slugs and rows:** both items are covered.

| # | slug | merge of | cls | severity | rows |
|---|---|---|---|---|---|
| 1 | `invoke-skill-entry` | `nanami-press-invokes-clause-3`, `mutsumi-nested-invoke` | B | medium | 437, 996 |
| 2 | `tomori-borrowed-second-half` | `tomori-borrowed-2-second-half` | B | medium | 187, 995 |

**Row note.** The four rows above are the ones filed in the group/BACKLOG.
Re-reading `card-review-2026-10-27.json`, rows 437 (sakiko_lead citation), 996
(ranger comment), 187 (crystal_swap citation) and 995 (ranger comment) do not
touch invocation — they are mis-filed next to this group. The rows that *do*
describe the defects are **734, 735, 737** (nanami press) and **407, 408, 421,
426** (mutsumi cash-out; tomori second half). Both sets are carried through the
tables below; the fragments come from `cards.json` either way.

Two sub-parts demote to **A** and need no ABI (marked in §2):
`skill-groups/src/crychic.rs:131` can read `ctx::group_crystals(player_id)` today
(the filed item 81), and tomori's 1d20 / [停留] / half-lifetime can be spelled
with existing imports. The capability itself — a named entry, an instance, and a
frame — is B for both items.

---

## 2. Problem

### 2.1 `invoke-skill-entry` — Mor:（NNM）稍微努力了一下 (rows 437, 996; review rows 734, 735, 737) and CRYCHIC:（睦）从没有觉得... (review row 426)

`cards.json#Mor:（NNM）稍微努力了一下`, clause (3):

> 「（3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的
> （2）技能，此次技能不受数量或轮数限制」

`cards.json#CRYCHIC:（睦）从没有觉得...`, clause (2):

> 「（2）消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的
> （2）效果，然后弃一张卡。」

The borrowed clause for nanami is 广町七深's character sheet (2)
(`rules/skills/skill-personas/src/nanami_ordinary.rs:5-6`):

> 「（2）当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记（当场上
> 存活玩家数量大于等于3时：每种角色标记上限1，该技能对被动技能存在3回合CD）」

Two distinct holes, one root.

**(a) The clause is unaddressable.** `ctx::play_card` / `ctx::invoke_skill` look
up `OnKind::Play` and nothing else (`hostfns.rs:2726-2747`, `:1358-1371`), so
"the (2) skill" is whatever that card happens to expose as its press:

* 广町七深's `On::Play` is clause **(3)** `use_skill` — the mark-spending
  recursion, not (2). `nanami_effort.rs:285` therefore runs (3); review row 737
  states exactly this. When the owner has a mark, (3) then asks which mark and
  invokes *another* skill, which is a different effect from (2).
* The CRYCHIC band skill's `On::Play` happens to be its (2) `transform_now`
  (`skill-groups/src/crychic.rs:262-275`), so mutsumi gets the right body by
  luck, not by name. Any band skill whose (2) is a hook would break mutsumi.

The `entry::PRESS` slug from v98 can *probe* an entry, but there is no way to
*run* one: `query_guard` is read-only.

**(b) The invoked skill loses its instance.** `play_card` and `invoke_skill`
call `enter_card`, which replaces `current_uid` with `-1` and never re-binds
(`hostfns.rs:2723-2726`, `:1360-1362`; `wasm_rules.rs:2343-2351`). Every
self-addressed read inside the invoked body therefore answers 0 / false:
`ctx::crystals()` (`bandori-std/src/lib.rs:213-215` → `counter` →
`counter_at(current_uid, …)`, `wasm_rules.rs:1318`), `ctx::is_placed()`
(`:1315`), `ctx::self_uid()`, `ctx::prop_at(self_uid(), …)`, the instance pools,
and the `CounterChanged` raise against the instance.

Consequences in the tree:

* CRYCHIC band (3) 「当此卡移除时，你获得X*500资金，X为此卡上的奇迹水晶数」 reads
  `let x = ctx::crystals();` (`skill-groups/src/crychic.rs:131`). Invoked by
  mutsumi (5 crystals, 3 spent ⇒ 2 left) the read is 0, the cash-out is skipped
  and `spend_crystals` never runs. The existing test pins the wrong behaviour:
  `card_paths.rs::mutsumi_never_2_runs_the_band_skill_2` asserts
  `"no money clause in (2)"`. Review row 426.
* Any invoked card's own `CounterChanged` hook (the shape `nanami_effort.rs:43`
  uses) would raise against the void uid when the invoked body writes a
  counter. The CRYCHIC band skill has no such hook today; its (3) is the direct
  read above, the same defect at its removal point.
* Nanami's `cant_play` gate (`nanami_effort.rs:77-84`) checks 「a mark exists」
  because the **wrong** clause (3) is run; the (2) body needs a **pot** rule
  instead. The mark check and the pot cost both belong to the clause, so the
  gate is wrong in both directions.

Expected: the press runs the owner's **named (2) entry**, with the skill
instance bound as the running instance, for the duration of the nested run.
`curr→expected` from the group, verbatim: *"The invoked skill's named (2) hook
runs with the skill instance bound; the cash-out reads the skill pool."*

**Demote to A:** the cash-out's 0 read is fixed *today* without any ABI by
reading `ctx::group_crystals(player_id)` (`bandori-std/src/lib.rs:260-262`)
instead of `ctx::crystals()`. That is a stopgap only: it names "the player's
first group instance", so it still cannot read a *persona* skill instance and it
does not fix (a).

### 2.2 `tomori-borrowed-second-half` — CRYCHIC:（灯）内心的呐喊 (rows 187, 995; review rows 407, 408, 421)

`cards.json#CRYCHIC:（灯）内心的呐喊`, clause (3):

> 「（3）若你的乐队技能为“MyGO!!!!!”，打出此卡时将其放置于你最早拥有的格子上，
> 你位于此卡前后5格内时可在时机合适时消耗1火罐使用一次高松灯（MyGO!!!!!）的
> （2）技能。」

The borrowed clause is 高松灯's character sheet (2)
(`rules/skills/skill-personas/src/tomori_poem.rs:5-8`):

> 「（2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家
> [触发结算]前强制将该玩家[传送]至自己所在的格子并[触发结算]，然后使其投掷1d20，
> 若出目小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；在此技能影响
> 下的支付价格变为原本的一半。」

`tomori_inner_shout.rs::pull` (`:213-245`) is a hand-written **copy of the first
half only**: it asks, spends 1 fire, teleports the mover to the controller's
tile, sets `latch` / `half`. It never rolls 1d20, never adds [停留], never owes
a settle, and never runs the skill's own code — so every future fix to 诗超绊
(2) skips the borrowed path. The site TODO at `:241-243` says so; review rows
407/408 record it; `BACKLOG TOMORI-BIND-01` owns it.

The existing implementation also shows *why* a bare "call the skill" is not
enough: the skill's body asks and spends **4** fire, while 内心的呐喊 (3) says
**1**; and the skill's own delay uses `state::set(player_id, DUE, 1)` +
`On::Action(Action::TurnEnd("actor == owner" …))` (`tomori_poem.rs:191`,
`:59-62`), i.e. a **placed** instance's hook — the borrowed skill is not placed,
so the hook never dispatches. The delay must be expressible by the caller of
the borrowed body (existing `ctx::at_next_turn_end`, `ctx.rs:2929-2934`).

Expected: the borrowed (2) **body** runs — teleport, latch, 1d20, ≤6 ⇒ [停留] and
a settle owed at the mover's next turn end, half price — under the inner
shout's cost (1 fire) and the shout's own [停留]-clearing schedule.

**Demote to A:** the roll spelling (`ctx::roll(mover, 1, 20,
RollPurpose::Effect, roll_source::SKILL)`), `rules_status::stay::add`, and
`ctx::card_settle_at` all exist; a copy could be finished with no ABI. The B
part is "run the borrowed body, not a copy".

### 2.3 Shared root causes

1. **No entry addressing on the run surface.** v98 added slugs for *queries*
   only; the run imports still hard-code `OnKind::Play`.
2. **No instance frame.** `enter_card` clears the uid instead of binding the
   target's own instance, so a nested run is a *name*, not an instance.
3. **No invoke context.** The invoked body cannot see "who borrowed me" and the
   trigger's card payload is stale, so a hook-shaped entry (the only shape most
   (2) clauses have) cannot find the skill it is acting as.
4. **Borrowed cost/timing is re-implemented per caller** because nothing marks
   "this invocation's cost is already paid" or returns the delay to the caller.

---

## 3. Capability as an invariant

**An invocation names its target entry, binds the target's live instance as the
running instance, seeds the invoked card as the skill in use, and pushes a frame
that survives re-entrancy and prompt pauses; the frame is popped on return with
the caller's instance, card and trigger restored. A press resolves by clause id
when one is named, and by the reserved press entry otherwise.**

Invariants the implementation must hold (each gets a test or a Kani obligation,
§9):

* **I1 Named entry.** Every invocation resolves an **entry id**: either the
  reserved `entry::PRESS` (`abi.rs:996-1008`) or a slug the target card
  declared (`On::….id("2")`, `card-sdk/src/lib.rs:1185-1199`). Never "the first
  `On::Play`". Unknown card traps; unresolvable entry is a no-op.
* **I2 Instance.** Before the body runs, `current_uid` is the target's field
  instance on the invoking player, or `-1` when the target is not placed. The
  frame records the **caller's** instance as `by_uid` and restores it on return.
  `self_uid()` inside the body is the target instance, never the caller's, and
  never inherited.
* **I3 Frame.** Frames nest (`MAX_NESTING`, `host.rs:94`) and pause through the
  existing `Asked`/`EXIT_NEED_INPUT` machinery; a re-run re-derives the stack.
  Restoration happens on every exit path, including a trap-free no-op.
* **I4 Trigger.** The caller's trigger view (kind, actor, tile, value, move) is
  inherited unchanged; `trigger.card` / `trigger.cards` are **replaced** by the
  invoked card for the frame and restored on return. "The skill in use" is the
  invoked card, which is what 「该角色」 / 「该技能」 reads.
* **I5 Body, not gate.** The invoke runs the entry's **body**. The condition,
  residual guard and play gate are *not* evaluated; a caller that must know
  first asks `ctx::query_guard` (which now seeds the same frame). `On::Filter`
  (a value fold) is not invocable.
* **I6 Fate.** The import returns the inner body's `Dest`; moving the card is
  the caller's job (the `play_card` contract, `ctx.rs:2490-2497`). The engine
  never silently unplaces the target.
* **I7 Determinism.** No import asks a player. Prompts inside the body pause the
  invocation through the existing sentinel; the ids are literals in the caller's
  body, so a replay re-derives the same call.
* **I8 Totality.** The import exists in wasm, native and Kani; the Kani arm
  records the call (card id, entry id, `paid`) and returns a symbolic `Dest`.

---

## 4. Surface

### 4.1 `abi.rs` additions

```rust
/// Invocation flags for `ruleset_invoke_entry` (`ctx::invoke_entry_paid`).
pub mod invoke {
    /// The caller already paid the entry's activation cost; the body must not
    /// collect it again. Read with `ctx::invoke_paid()`.
    pub const COST_PAID: i32 = 1;
}

/// `(card, entry, by, by_uid, uid, paid, depth)` -- the innermost invocation
/// frame (`ctx::invoke`), postcard on the out-buffer. `entry` is the id (the
/// reserved `entry::PRESS` when the caller used `invoke_skill` / `play_card`).
/// `by` / `by_uid` are the invoking player and the invoking card instance
/// (`-1` when the caller was not a placed card); `uid` is the target instance
/// (`-1` when the target is not placed). The host frame stores `String`s; the
/// guest interns them on decode like `group_skills`
/// (`bandori-std/src/lib.rs:311-327`).
pub struct InvokeFrame {
    pub card: NameId,
    pub entry: NameId,
    pub by: i32,
    pub by_uid: i32,
    pub uid: i32,
    pub paid: bool,
    pub depth: i32,
}
```

No new `OnKind`, no `ManifestOn` change: entry ids are the v98 field.

### 4.2 Import

```rust
// guest declaration (ctx/sys.rs), ruleset module
pub fn invoke_entry(
    player_id: i32,
    card_p: i32, card_n: i32,
    entry_p: i32, entry_n: i32,
    flags: i32,
) -> i32;

// the innermost frame as postcard `InvokeFrame`; `(0, 0)` = size probe
pub fn invoke_frame(out_ptr: i32, cap: i32) -> i32;

// host table (host.rs) and native shims `op_invoke_entry([i32; 6])` /
// `op_invoke_frame([i32; 6])`
```

`invoke_entry` returns the inner `Dest` code, or `EXIT_NEED_INPUT`; six `i32`s,
the native `[i32; 6]` maximum already in use. `invoke_frame` returns the
postcard byte count (`out_ptr = 0` asks for the size only, the `group_skills`
shape).

`ruleset_invoke_skill` and `ruleset_play_card` are **kept** and become thin
host-side calls into the same core with `entry::PRESS` and `flags = 0`; no
signature moves (§8).

### 4.3 Guest wrappers (`card-sdk/src/ctx.rs`, next to `play_card` / `query_guard`)

```rust
/// Run one named entry of `id` nested, as `player_id`, with its field instance
/// bound. Body only -- ask `query_guard` first when the gate matters.
pub fn invoke_entry(player_id: i32, id: &str, entry_id: &str) -> Result<Dest, Prompt> {
    invoke_entry_flags(player_id, id, entry_id, 0)
}

/// `invoke_entry` for an entry whose cost the caller has already paid:
/// the body sees `ctx::invoke_paid()` and must not collect it again.
pub fn invoke_entry_paid(player_id: i32, id: &str, entry_id: &str) -> Result<Dest, Prompt> {
    invoke_entry_flags(player_id, id, entry_id, crate::abi::invoke::COST_PAID)
}

fn invoke_entry_flags(player_id: i32, id: &str, entry_id: &str, flags: i32) -> Result<Dest, Prompt>;

/// The innermost invocation frame, if this body is running inside one.
pub fn invoke() -> Option<InvokeFrame>;

/// `invoke().is_some_and(|f| f.paid)` -- sugar, not a second import.
pub fn invoke_paid() -> bool;
```

`ctx::invoke_paid()` is a read, not a decision. `ctx::invoke()` is what a
logged invocation names (`by_uid`), and what future entries will branch on.

### 4.4 Entry addressing

* `entry::PRESS` (`abi.rs:996-1008`) is unchanged and keeps resolving through
  `play_entry_preferring_gated()` (`host.rs:186-190`) — "whatever the engine
  would press". `ctx::invoke_skill` is therefore exactly
  `invoke_entry(…, entry::PRESS)`.
* Clause slugs are declared by the target: 广町七深's (2) becomes
  `On::Action(Action::CardUsed(…), …).id("2").label("（2）")`; 诗超绊's (2)
  offer becomes `.id("2")` and its effect half `.id("2-effect")`. `"2"` is a
  valid slug (`is_entry_slug`, `host.rs:3045`); labels stay display text.
* `entry::PRESS` remains reserved (`host.rs:1072-1080`); the press entry *may*
  also carry `.id("2")` where the sheet numbers it, because a clause id is
  identity and `PRESS` is a resolver, not a name.
* Resolution: `PRESS` → `play_entry_preferring_gated()`; any other id →
  `entry_by_id` (`host.rs:162-170`). Card unknown ⇒ trap
  (`invoke_entry: unknown card {id:?}`); entry unknown/unnamed ⇒ **no-op**
  returning `DEST_UNSET`, matching today's "no `On::Play` entry" path.

### 4.5 Invocable kinds

| kind | invocable | why |
|---|---|---|
| `Play` | yes | the press; the existing path |
| `Action` | yes | (2) clauses are reaction-shaped; the body is `fn(i32) -> Asked` |
| `Settle` / `Schedule` | yes | same body shape |
| `Filter` | no | a pure value fold (`i32 -> i32`), not an activation; trap |
| `Counteract` | no | the body reads the answered chain link; trap |
| `Message` | no | the body reads a message payload; trap |

The kind is taken from `info.on[entry].kind`. The trigger **kind** stays the
caller's; only `card` / `cards` are seeded (I4).

### 4.6 `ctx::query_guard` extension

`query_guard` already resolves by the same ids and clones the world
(`hostfns.rs:2804-2824`, `:2868-2902`). It gains the same frame seeding: look up
the target's instance and `enter_invoke(id, uid)` instead of `enter_card(id)`.
This is required, not cosmetic: a gate shaped as a reaction residual
(`on_skill_applies` reads `trigger::cards()`, `nanami_ordinary.rs:79-84`) is
otherwise always false under a press and the probe is useless.

The CEL scope needs the same fact. `fill_window_ambient` fills
`actor = player_id` and `trigger_card = 0` (`cond_pre.rs:1393-1422`), and a
reaction-shaped pre such as `"actor != owner && trigger_card != 0 && …"`
(`nanami_ordinary.rs:62-65`) therefore refuses. After the scope is built,
`query_guard_entry` overwrites `scope.trigger_card = card` (and keeps
`actor = player_id`) so the probe asks the question the invoke would answer. No
wire change; PRESS probes become *more* accurate because `is_placed()`-shaped
guards now see the real instance.

### 4.7 What does *not* move

`ctx::play_card` (card play with a returned `Dest`), `ctx::invoke_skill` (press
by `entry::PRESS`), `ruleset_send` and the declare/`declared_arg` machinery are
untouched. `declare` is per-entry and unrelated to invocation: an invoke runs
`OP_RUN` only, so a body with `declare` gets `T::default()` from
`drive_body_arg` (`card-sdk/src/lib.rs:215-229`) and its collector never asks.
That is documented on the entry wrapper.

---

## 5. Engine wiring

### 5.1 One core

`hostfns::invoke_skill` (`:1338-1399`), `hostfns::play_card` (`:2700-2767`) and
the new `hostfns::invoke_entry` become one `invoke_inner(c, player_id, id,
entry_id, flags)`:

1. `guest_str` the id and the entry id; `rules.by_id` (trap on unknown);
2. `depth >= MAX_NESTING` ⇒ trap `invoke_entry nested deeper than 8`;
3. resolve the entry (4.4) and check the kind (4.5);
4. `let uid = st.w().field_instances(player_id).iter().find(|(_, c)| c == id)`
   — the same lookup the press paths already do (`wasm_rules.rs:9392-9397`,
   `:9090-9100`);
5. take fuel/answers, `HostState::new(…, depth + 1)`, `next_answer` carried;
6. `enter_invoke(&id, uid)` (5.2), `play_from_hand(false)` saved/cleared;
7. `call_entry(…, OP_RUN, player_id, &[], false)`; **no** `OP_GUARD`, **no**
   `OP_DECLARE`, no `cardUsed` raise;
8. restore world/answers/asked/host_request, `leave_invoke(saved)`,
   `set_play_from_hand(was)`;
9. `Ok(dest)`, `EXIT_NEED_INPUT` as `Ok(EXIT_NEED_INPUT)`, other errors trap.

`play_card` passes `entry::PRESS` and keeps its "card play" doc; `invoke_skill`
passes `entry::PRESS`.

### 5.2 The frame (`wasm_rules.rs`; `CardWorld` default in `world.rs:1132-1141`)

`enter_card` / `leave_card` are generalised to:

```rust
/// Open a nested invocation of `id` with the target instance `uid` (or -1).
/// Saves the caller's card, dest, uid and trigger card payload.
fn enter_invoke(&mut self, id: &str, uid: i32) -> InvokeSaved;
/// Restore the caller's frame; returns the inner body's Dest.
fn leave_invoke(&mut self, saved: InvokeSaved) -> i32;
```

`enter_invoke` is `enter_card` plus `self.current_uid = uid` and
`self.trigger.card/cards` saved + replaced with `id` (single: `card = Some(id)`,
`cards = vec![id]`). `leave_invoke` restores both and `current_uid = saved.uid`.
`Run.invoke_stack: Vec<InvokeFrame>` (not saved, §7) is pushed/popped here;
`ctx::invoke()` and `invoke_paid()` read its top through one `invoke_frame`
output-buffer import, and `self_uid()` keeps reading `current_uid` as today
(`wasm_rules.rs:1379-1382`).

`enter_card` / `leave_card` stay **as they are** (clear uid, no seeding) so
`send` and the event paths do not change; `enter_invoke` reuses their
save/restore. `send` already binds a uid the same way
(`hostfns.rs:3315-3319`) and keeps its own message context instead of the
trigger seed; unifying it onto `enter_invoke` is a later cleanup, not this
design.

### 5.3 Trigger

`CoreTrigger` already carries `card` / `cards`; the raised `cardUsed` press sets
both (`play.rs:983-988`), so a *normal* press inside the invoked body's view is
already "the skill in use". A borrowed invocation reproduces that fact without a
raise: the frame seeds it. `ctx::trigger::*` in the invoked body therefore reads
the caller's actor / tile / value (needed by 诗超绊 (2): the mover is
`trigger::player_id()`, the abandoned landing is `trigger::tile()`) and the
invoked card as 「该角色」 (needed by 七深 (2)).

A raise is **not** added: an invocation is already committed on the caller's
line, and a second counteraction window would let a [反击] negate a body whose
cost was paid three effects ago.

### 5.4 Limits, fate and traps

* `MAX_NESTING` and the shared fuel budget are reused verbatim; the nested
  guest and the outer run share one budget (`call_entry` already does,
  `host.rs:3005-3016`).
* Cost is the caller's business. `invoke_entry` is only the "cost paid" signal
  (I5); it does not validate that anything was paid.
* A trap inside the invoked body aborts the whole play (the play already
  committed); the no-op entry path returns `DEST_UNSET`.
* `Dest`: returned only. A placed target that wants to leave uses
  `ctx::unplace_at(ctx::self_uid())` in its own body (the band (2) sweep
  already does, `skill-groups/src/crychic.rs:196`).

### 5.5 Kani model (`rules/kani/src/host.rs`, `crates/card-sdk/src/kani_host.rs`)

The Kani host cannot execute the nested guest, so the arm records facts:

```rust
pub fn arm_invoke_entry(a: Args) -> i64 {
    world::with_world_mut(|w| {
        w.invoke_n += 1;
        w.last_invoke = (name_id(a[1], a[2]), name_id(a[3], a[4]), a[5] & 1);
    });
    // symbolic fate: DEST_UNSET, Graveyard or Field, the three a borrowed body uses
    world::with_world(|w| w.dest_hint) as i64   // a `dest_hint` field on the model
}
```

`invoke_skill` / `play_card` map to the same arm (they are PRESS/0). `invoke_paid`
is a cheap `last_invoke.2 == 1` where a proof needs it. Both are added to
`rules/kani/build.rs`'s allowlist (`:124` region) and to each group's `host_subset!`
table that uses them. A proof-only reader `last_invoke()` exposes
`(card, entry, paid)` and `invoke_n`.

### 5.6 Probing and the gate

Nanami's gate becomes "a crystal, a persona skill, and its (2) admits":
`ctx::query_guard(player_id, skill, "2")`. Because the query seeds the frame
(4.6), `on_skill_applies` sees `trigger_cards = [skill]` and the pot precondition
`charge(owner) >= 1`, so the probe answers (almost) exactly "would the (2) body
do anything". One gap stays: the pre gates on **pots**, while `fire::spend` can
substitute a 不再迷茫 crystal, so a 0-pot owner with a crystal is refused before
the substitution window (open question 8). The press then invokes **unpaid**:
the (2)'s own pot cost is part of the clause and is still collected by the
body's ask.

---

## 6. Card usage — before / after

**G05 note.** The button/press *surface* — when a skill is offered, its
`why_not` text, the `use_skill` dispatch — is the G05 group's. This document
only guarantees what a press (and a card) can *address* and *bind*; the gate
rewrites below are the minimum to keep the cards coherent with the clause and
are G05's to finish.

### 6.1 `rules/cards/card-morfonica/src/nanami_effort.rs` (item 1)

```rust
// before: cant_play (:77-84)
if owner_skill(player_id).is_none() { …no_skill }
if ctx::tok_names(player_id, TOKEN_PREFIX).is_empty() { …no_mark }
// after: the (2) needs a pot, not a mark
let Some(skill) = ctx::persona_skill(player_id) else { …no_skill };
if !ctx::query_guard(player_id, &skill, "2") { …no_pot_or_cd }

// before: use_skill (:259-287)
fn owner_skill(p) -> Option<NameId> { ctx::placed_cards(p).find(starts_with("skill:")) }
ctx::add_crystals(-1, 0)?;
ctx::play_card(id.as_str(), player_id)?;    // runs (3)
// after
let Some(skill) = ctx::persona_skill(player_id) else { return Ok(()) };
ctx::add_crystals(-1, 0)?;                  // 「移除一个奇迹水晶」
ctx::invoke_entry(player_id, &skill, "2")?; // 「视为发动你的（2）技能」
```

`owner_skill` → `ctx::persona_skill` so a band attachment can never be mistaken
for the character skill (`OPEN-借用技能` remains open for a non-七深 owner; §11).
The 「不受数量或轮数限制」 TODO resolves: the body runs with no gate, and the
CD lives in `on_skill_applies` which the invoke does not run.

Borrowed (2) body (`rules/skills/skill-personas/src/nanami_ordinary.rs`):

```rust
On::Action(Action::CardUsed(…), Some(on_skill_applies), on_skill).id("2").label("（2）")
```

No body change: `trigger::cards()` now carries the invoked skill id (I4), and
`ctx::ask_yes` + `fire::spend(1)` + the mark grant run as written.

### 6.2 `rules/cards/card-crychic/src/mutsumi_never.rs` (item 1)

```rust
// before (:81-83)
ctx::invoke_skill(player_id, &skill)?;
// after
ctx::invoke_entry(player_id, &skill, ctx::entry::PRESS)?;   // now instance-bound
ctx::log(player_id, &Msg::new(key!("mutsumi_never_invoked")).card("card", skill));
```

Behavioural flip: with the band instance bound, `at_turn_end`'s
`let x = ctx::crystals()` reads the post-spend count (e.g. 5 → 2 ⇒ 1000), pays
it, and `spend_crystals` consumes the instance. The `"no money clause in (2)"`
assertion in `card_paths.rs:361` flips to the paid figure. `skill-groups` needs
**no** change once bound; `group_crystals` at `:131` is only the A stopgap.

### 6.3 `rules/skills/skill-personas/src/tomori_poem.rs` and
`rules/cards/card-crychic/src/tomori_inner_shout.rs` (item 2)

Poem: split the (2) into two named entries and route both callers through one
effect body.

```rust
/// The (2) offer was accepted and paid on this settle; `"2-effect"` consumes
/// it. Cleared on entry to the effect, so the pair is one-shot per settle.
const OFFERED: &str = "skill.tomoriPoem.offered";

// (2) offer -- ask + pay, latch OFFERED; the effect is a second entry so a
//     borrower can enter it alone
On::Action(Action::SettleBefore(…), Some(before_settle_applies), before_settle)
    .id("2").label("（2）"),
// (2) effect -- teleport + latch + roll + delay; runs on the same settle as a
//     just-paid offer, or alone when a borrower invoked it
On::Action(Action::SettleBefore("", Some(after_offer_applies), before_settle_effect))
    .id("2-effect").label("（2）"),

fn before_settle(player_id: i32) -> Asked {
    let mover = ctx::trigger::player_id();
    // (3) is the owner's own landing near another (cost 1, *their* tile);
    // (2) is someone else's landing near the owner (cost 4, the owner's tile).
    let (cost, dest) = if mover == player_id {
        (1, nearest_other_tile(player_id))
    } else {
        (4, ctx::player_pos(player_id))
    };
    if dest < 0 { return Ok(()); }
    if !ctx::invoke_paid() {                            // a borrower may have paid
        if !ask_yes(…) { return Ok(()); }
        if !fire::spend(player_id, cost, …)? { return Ok(()); }
    }
    state::set(mover, OFFERED, 1);                      // "2-effect" consumes it
    Ok(())                                              // the effect is its own entry
}

/// `"2-effect"` applies exactly once: `"2"` just offered, so the latch is set;
/// a paid invocation marks the cost paid instead.
fn after_offer_applies(player_id: i32) -> bool {
    let mover = ctx::trigger::player_id();
    ctx::invoke_paid() || ctx::slot(mover, OFFERED) != 0
}

/// The destination is recomputed so the entry is self-contained when invoked
/// (`"2-effect"` takes only `player_id`).
fn before_settle_effect(player_id: i32) -> Asked {
    let mover = ctx::trigger::player_id();
    state::set(mover, OFFERED, 0);                      // one shot
    let dest = if mover == player_id { nearest_other_tile(player_id) }
               else { ctx::player_pos(player_id) };
    if dest < 0 { return Ok(()); }
    ctx::teleport_to(mover, dest);
    state::set(mover, ACTED, 1);
    if mover != player_id {                            // the (2) arm, not cost == 4
        let n = ctx::roll(mover, 1, 20, RollPurpose::Effect, roll_source::SKILL)?;
        if n <= 6 {
            rules_status::stay::add(mover, 1);
            state::set(mover, DUE, 1);                  // the **payer's** slot
            ctx::at_next_turn_end(mover, "due_settle"); // 「自己的下回合结束时」
        }
    }
    log;
}
On::Schedule("due_settle", "", None, due_settle)       // = the old at_turn_end
```

Two entries, not one body calling the other: the engine **runs every matching
entry**, so a normal press enters `"2"` and then `"2-effect"` on the same
settle. `"2"` therefore only offers and latches; `"2-effect"` is the single
effect body, gated by the `OFFERED` latch (cleared on entry) or by
`invoke_paid` on a call that skipped the offer.

`nearest_other_tile` is the existing scan lifted out of `before_settle`
(`tomori_poem.rs:141-160`).

`DUE` moves from the owner to the **mover** (`state::set(mover, DUE, 1)`), which
makes the existing `half` filter correct as written: its body receives the payer
and reads `state::get(player_id, DUE) > 0` (`tomori_poem.rs:74-78`). The
`TurnEnd` entry and its `actor == owner` pre go away; a schedule fires on the
mover's next turn end (already skip-safe, `ctx.rs:2929-2934`), which is what
「自己的下回合结束时」 means, and it survives the card not being placed — the
borrowed case. `due_settle` clears `DUE` first, then settles.

Shout `pull`:

```rust
// before (:229-237): teleport_to + latch_key + half_key + log
// after
ctx::set_slot(mover, &half_key(), 1);                     // covers the forced settle
// Paid call of the offer entry: `invoke_paid` makes "2" skip ask/pay, and the
// settle then reaches "2-effect" through the normal filter.
ctx::invoke_entry_paid(player_id, "skill:高松灯:诗超绊", "2")?;
ctx::log(...);                                            // 「使用一次…（2）技能」
```

`clear_latch` on `MoveResolved` clears `latch_key` and **`half_key`** (the forced
settle is over). The shout adds its own `On::Schedule("due_settle", …)` entry:
set `half_key`, `ctx::card_settle_at(mover, pos, true)`, clear `half_key`. One
fire (not four) because the shout paid before invoking (`COST_PAID`); the roll
and delay come from the skill body.

`tomori_poem.rs::at_turn_end` becomes `due_settle` (`On::Schedule` instead of
`On::Action(Action::TurnEnd)`) and is unchanged apart from the clear order.

---

## 7. Save / record / replay

* **SAVE_VERSION unchanged.** The invoke stack is transient: a pause returns
  `EXIT_NEED_INPUT` through the caller, the answer log is saved as today, and
  the resume re-runs the outer entry, re-enters and re-derives the whole chain
  (the same model `play_card` nested calls already rely on). The target
  uid is re-looked-up. No new serde field; the newly *used* `Scheduled` rows are
  the v102 shape and already save.
* **RECORD_VERSION unchanged (3).** Ids are body literals and prompt answers are
  already recorded; no RNG or input shape moves. The poem's 1d20 goes through
  the serialized RNG stream like every roll.
* **Replay.** An archived bundle replays through its own host; nothing in the
  record names the new import. A record made after this design replays only on a
  bundle that has it — the normal forward rule.

If a pause-with-open-frame is ever observed across a save boundary to lose
state, the fallback is to persist `invoke_stack` with `#[serde(default)]` and
bump SAVE_VERSION by one (15 → 16 today, riding the G05 batch; take the head
at landing); the
design above does not need it.

## 8. Versioning

**No bump of its own.** The new import is additive; no existing import moves
arity and no manifest field changes. `ruleset_invoke_skill` and
`ruleset_play_card` stay at their current signatures. This group lands **with
G05**, which does bump (110 → 111 for the press entries), and this block
appends to that version's note.

Insert below the current head note (the `v111` block, inside G05's note):

```
/// v111 (continued): **named-entry invocation**. New imports
///    `ruleset_invoke_entry(player_id, card, entry, flags)` and
///    `ruleset_invoke_frame(out, cap)` -- run one named
///    entry of `card` nested with its field instance bound as `current_uid`
///    and the invoked card seeded as the skill in use (`trigger.card` /
///    `trigger.cards`), both restored on return. `ruleset_invoke_skill` /
///    `ruleset_play_card` are the `entry::PRESS` wrappers over the same core;
///    signatures unchanged. `flags` bit 0 = `invoke::COST_PAID`
///    (`ctx::invoke_paid`). Guest surface `ctx::invoke_entry` /
///    `ctx::invoke_entry_paid` / `ctx::invoke`; `ctx::query_guard` seeds and
///    binds the same frame. Play / Action / Settle / Schedule entries are
///    invocable; Filter / Counteract / Message trap; an unknown entry is a
///    no-op. ABI_VERSION unchanged (no signature moved).
```

If the reviewer prefers the alternative of extending `ruleset_invoke_skill` with
the entry pair (5 args), that *is* a wire break on top and the group takes the
next number instead (112 → 113), shifting the rest, with a
one-line reason.

## 9. Tests (per item) and Kani obligations

Black-box tests use the existing `Table` harness and `t.option` / `answer_q`.

| item | test |
|---|---|
| 1 | `crychic.rs::nanami_effort_press_runs_the_owners_second_skill` — 1 fire, persona skill of 七深 bound; after the press: 1 pot spent, one `角色标记:skill:广町七深:…` gained, the `nanami_ordinary_mark` log present, no `nanami_ordinary_which` (the (3) pick) prompt |
| 1 | `crychic.rs::nanami_effort_press_keeps_the_crystal_cost_and_one_crystal_only` — starting 2 crystals, 1 removed |
| 1 | `crychic.rs::nanami_effort_press_refuses_without_a_pot` — 0 fire, no crystal substitute → `nanami_effort_no_skill`/gate refusal before the crystal is spent |
| 1 | `crychic.rs::mutsumi_never_cash_out_reads_the_band_instance` — 5 crystals: after (2) the band card is gone, money `+2*500` (not the pre-spend 5), band crystals 0 (no `Sumimi:#L11` placed, so the substitution order is not involved) |
| 1 | `crychic.rs::mutsumi_never_cash_out_is_zero_on_a_shortfall` — 1 crystal: all spent, no money |
| ABI | `crates/game-rules/tests/card_activation.rs::invoke_binds_the_target_and_restores_the_outer_frame` — test card A invokes test card B; B reads `self_uid` = B's uid, A's `self_uid` is restored after |
| ABI | `card_activation.rs::invoke_seeds_and_restores_the_trigger_card` / `::invoke_unknown_entry_is_a_noop` / `::invoke_unknown_card_traps` / `::invoke_filter_entry_traps` |
| ABI | `card_activation.rs::invoke_skill_is_invoke_entry_press` — identical `Dest` and log as the PRESS slug |
| 2 | `crychic.rs::tomori_shout_pull_runs_the_borrowed_second_body` — the poem's own `tomori_poem_moved` log appears (proves the body, not the copy), mover teleported to the controller's tile |
| 2 | `crychic.rs::tomori_shout_pull_rolls_once_and_leans_on_six_or_less` — forced face 6 ⇒ `stay` 1 plus a scheduled settle; face 7 ⇒ none |
| 2 | `crychic.rs::tomori_shout_delayed_settle_pays_half_at_the_movers_next_turn_end` — the settle runs on the mover's next turn end (not the controller's), at half, and then clears |
| 2 | `crychic.rs::tomori_shout_spends_one_fire_not_four` — fire pool 2, one spent, `COST_PAID` suppresses the poem's 4 |
| 2 | `mygo.rs::tomori_poem_second_settles_at_the_movers_turn_end` — the standalone skill, same delay assertion |

Static: `rules/kani/tests/on_parse.rs` (`.id` on an `On::Action` survives the
manifest), `split_hook_entries.rs` (the poem's two (2) entries keep distinct
kinds/ids), `crates/card-sdk/src/kinds.rs` count unchanged.

**Kani obligations** (specs from the quotes only; the Kani host records call
facts rather than running the nested guest):

* `nanami_press_invokes_exactly_one_entry_iff_a_crystal_and_the_second_admits`
  — one `invoke_entry` with entry `"2"`, `paid == false`, crystal −1; zero when
  the gate refuses.
* `nanami_press_gains_no_mark_by_itself` — the mark arrives from the inner body,
  so the press's own effect set is one crystal + one log + one invoke.
* `mutsumi_burns_min_three_crystals_iff_it_invokes` — `n = min(3, crystals)`
  before the call, and `invoke_n == 1` iff a band skill is bound.
* `mutsumi_borrowed_invoke_is_unpaid` — flags 0: the band (2) is not a cost the
  card pre-paid.
* `tomori_pull_invokes_paid_iff_accepted` — `paid == true`, one fire spent, and
  no invoke on a declined ask or a failed substitution.
* `tomori_roll_follow_up_iff_the_mover_is_not_the_owner` — the `mover != player_id`
  arm, not `cost == 4`.
* `tomori_due_settle_clears_its_latch_iff_it_settles` — `DUE` (poem) / `half`
  (shout) are 0 after the deferred settle, and the settle happens exactly once
  at the mover's next turn end.
* ABI stub totality: `invoke_entry` / `invoke_frame` arms exist in the
  `kani_host` match (compiler-exhaustive) and in `rules/kani/build.rs`.

## 10. Alternatives

1. **Extend `ruleset_invoke_skill` with the entry pair** (5 args) instead of a
   new import. One primitive fewer, but a signature break: every current wasm
   needs the bump and the archived-bundle story gains a version fork. Rejected;
   the additive import is free because `invoke_skill` is already just PRESS.
2. **No ABI: every borrower copies the half it needs** (finish tomori's copy,
   hand-write nanami's (2)). Zero surface, but two implementations per borrowed
   clause — the exact drift that caused item 2 — and the copy cannot bind an
   instance, so the CRYCHIC cash-out stays broken unless it is also special-
   cased to `group_crystals`. Rejected.
3. **Typed invoke argument (the `declare` mirror)**: `invoke_entry::<T>` +
   `ctx::invoke_arg::<T>()`, with `fn(i32, T)` bodies. More general than
   `COST_PAID` (a target tile, a count), but doubles `BodyShape` and adds
   postcard ceremony to every site for two fields. Rejected now; it composes on
   top of this design later.
4. **Raise a synthetic `cardUsed` for each invoke.** The body then gets its own
   event context and payload for free, but a second counteraction window opens
   for an already-committed invocation and every `cardUsed` listener reruns on
   the invoked line. Rejected; seeding without a raise is enough for I4.
5. **Full call handles** (`ctx::call(card, entry) -> RunHandle`, explicit
   return). More general than fire-and-return, unnecessary while `Asked` already
   pauses through. Rejected for this batch.

## 11. Open questions

1. **Who is 「你的（2）技能」 for a non-七深 owner?** `OPEN-借用技能`
   (`spec/OPEN-QUESTIONS.md:276`). This design binds the *invoking* player's
   persona skill; if the sheet ever means the card's owner instead, the lookup
   moves to the card's owner (a new owner-of-instance reader) and the entry id
   stays.
2. **Should an invoke run the gate too?** Body-only matches C# `Offer()` and the
   press path, and 「不受数量或轮数限制」 depends on skipping. A
   `invoke_entry_gated` wrapper could ask `query_guard` first if a clause ever
   needs "normal limits".
3. **Trigger kind.** The invoked body inherits the caller's kind. Should an
   `Action` entry instead present its own declared kind for the frame? Today's
   bodies do not read it, but a future guard might.
4. **Static slug check.** An unknown entry is a no-op; a typo is therefore
   silent. Add a build-time or test-time scan that every literal passed to
   `invoke_entry` resolves on its target.
5. **`play_card` unification.** Should `ctx::play_card` be documented as
   `invoke_entry(PRESS)` (and eventually folded), or stay a separate concept
   because it is for hand/pile cards? It now shares the core either way.
6. **`COST_PAID` validation.** The flag trusts the caller. Should the engine
   compare a declared card-authored cost prop against what was paid, or is
   trust plus one test per site sufficient?
7. **Transient instances for borrowed, unplaced skills.** A persona skill is a
   placed `skill:*` instance, and step 4 of `invoke_inner` finds it, so
   `is_placed()` is **true** in a borrowed persona body. The open case is a card
   invoked from a zone it is not placed in (a hand or pile card), where the
   lookup is empty and `is_placed()` false; if such an entry needs it true, the
   engine must materialize a transient instance. Today's bodies do not.
8. **Pot gates vs the crystal substitution.** 七深's (2) pre says
   `charge(owner) >= 1`; `fire::spend` can take a 不再迷茫 crystal instead
   (`tomori_poem.rs:101-104` is the same question on the poem side and omits the
   gate). Should the (2) gate be `charge(owner) >= 1 || a substitute is armed`,
   or is a 0-pot press meant to be refused? The design keeps the sheet's literal
   pot condition on the gate and lets `fire::spend` substitute only inside the
   body's ask.
9. **Should an invoked entry's `declare` run?** Today it gets `T::default()`.
   A later addendum could run `OP_DECLARE` with the invoke frame seeded, at the
   cost of a prompt inside a nested run.
