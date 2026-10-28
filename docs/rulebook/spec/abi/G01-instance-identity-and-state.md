# 1. ABI design G01 — Per-instance identity and state

**Status:** draft for review. Design only; no engine, SDK or card source is
changed by this document.

Closed backlog items (B class: needs an engine/ABI capability):

| # | slug | rows |
|---|------|------|
| 1 | `skill-fired-signal` (merged `rui-x-write`, `sayo-skill-used-event`, `tomori-x-instance-scope`, `umiri-user-key-per-instance`) | 232, 1243, 1244, **1481**, 184, 152, 1091, 445 |
| 2 | `starry-trigger-uid` | 1039 |
| 3 | `black-marker-hook` | 283, 1029 |
| 4 | `black-copy-crystals` | 285 |
| 5 | `tomori-x-instance-scope` (see 1) | 184, 152, 1091 |
| 6 | `detour-counter-hooks-dead` (not in section B) | 217, 950 |
| 7 | `hagumi-multi-standin-abi` | 252 |
| 8 | `sparkler-per-instance-counter-cap` | 342 |
| 9 | `umiri-user-key-per-instance` (see 1) | 445 |
| 10 | `hina-clause-2-swap-missing` (G05's consolidated row) | 487, 488, 490, 1555 |
| 11 | `cs-crystal-cap-read` | 301, 1056 |

Items 5, 6, 7 and 9 are **demoted to A** after this design: existing mechanisms
plus the shared payload from item 2 close them, with no new import. They stay
in this document because their fix depends on the identity payload designed
here, and because the demotion must be recorded rather than silently assumed.

All Chinese fragments below are quoted verbatim from `docs/rulebook/cards.json`
and are identified by their triage row id.

---

## 2. Problem

### 2.1 `rui-x-write` — row 232

> 在此卡结算过程中，每当你的技能被触发后，立即将x设置为5

Today `rui_devil` writes X unconditionally three times:

- `rules/cards/card-morfonica/src/rui_devil.rs:37` after every outbound
  `ctx::transfer`;
- `:43` after every inbound `ctx::transfer`;
- `:50` after both loops, with the log.

The TODO at `:47-49` says exactly what is missing: *"skill-fire signal:
nothing reports whether a skill fired while this body ran"*. A stripped skill,
an immune payee or a trailing write therefore still leaves X at 5 (item
`current`). The sheet keys the write on 「每当你的技能被触发后」 — one write per
fire, and none without a fire.

The skill that fires here is `skill:八潮瑠唯:正论暴击`
(`rules/skills/skill-personas/src/yuri_crit.rs`), a **Pay-hook** body: it is
dispatched by `collect_hook_jobs` (`crates/game-rules/src/wasm_rules.rs:5292`)
from the money pipeline, not by `use_skill`. `use_skill` raises `cardUsed`
(`crates/game-core/src/engine/play.rs:979-990`) only on a press, so hook-driven
skill bodies have no fire signal at all. The card also cannot ask "did this
fire happen during *my* resolution" — `Run` has a single `current_card`
(`wasm_rules.rs:222-228`) which a nested `play_card` replaces, and no stack.

### 2.2 `starry-trigger-uid` — row 1039

> 此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金

The guard at `rules/cards/card-morfonica/src/starry_night.rs:117-124` filters
by `trigger::card_is(ID)` and `trigger::name() == counter::CRYSTALS`. Both are
name compares. `collect_hook_jobs` dispatches every placed instance of the id;
there is no per-uid dedupe. Two copies on one field therefore both pass the
guard when **one** copy's counter changes, and both pay (TODO at `:120-122`).

The engine *has* the instance at the raise: `Run::note_counter_changed` is
called with `uid` (`wasm_rules.rs:648`) but drops it when pushing to
`counter_log` (`:655-656`, tuple `(owner, card, name, change)`), and
`commit_after` has nothing to stamp (`:4858-4864`).

### 2.3 `black-marker-hook` — rows 283, 1029

> 该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果。你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。

`sync_circle_instance` (`rules/cards/card-hhw/src/black_suits.rs:103-131`) is
called only from this card's own `counteract` (`:79`) and `pass_tile` (`:152`).
There is no mark-added/removed point, so:

- an external clear (another card, an external bounce) leaves the borrowed
  `tile:circle` attached after the last crystal is gone;
- a bankrupt owner's purge destroys the crystals with no sync (`:104-108`).

Worse, the borrowed instance is placed on the board owner
(`ctx::place_card_on(-1, group, "tile:circle", …)`, `:120-125`) and nothing
binds it to the stand-in. `destroy_instance_units` only walks `TileMark`/token
rows (`crates/game-core/src/engine/ops.rs:2123-2131`), so the borrowed rule
outlives its crystal forever.

The crystal mark is also created **before** the stand-in is placed
(`black_suits.rs:58-67` then `:86-90`), so `ctx::self_uid()` is still `-1` at
`place_mark`: the wrapper stamps `instance = None` and `src = None`
(`wasm_rules.rs:1442-1450`). The rows are bound to no card at all, so even a
correct purge could not find them from the instance.

### 2.4 `black-copy-crystals` — row 285

> 你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。

`pass_tile` (`black_suits.rs:142-158`) bumps by `-1` unconditionally. The pass
hook dispatches one job per placed copy, so two copies remove two crystals on
one pass (`:148-150`). The residual guard `pass_tile_guard` (`:135-137`) counts
the owner's crystals and admits **both** jobs; only the trailing
`own_crystals == 0` check files, and only for the copy that ran last.

No per-raise identity exists on a `Pass` trigger, so the two jobs cannot tell
they answer the same pass.

### 2.5 `tomori-x-instance-scope` — rows 184, 152, 1091

> 将此卡置于场上并从1-20间选择并声明X，将其写下

`want_human` writes X to a **player** slot (`rules/cards/card-crychic/src/want_human.rs:61`,
key `:39`) and every reader/„reset" goes through `ctx::slot`
(`want_human.rs:95,110,139,145`; `tomori_inner_shout.rs:66,91`). The comment at
`want_human.rs:57-60` states the defect: the value survives the card and two
copies share it, where C# `Mem["x"]` belongs to the instance. `boost` and
`abBefore` (`:79`, `:84`) have the same shape.

`tomori_inner_shout` already implements the cross-instance half by hand:
`want_human_placed` scans `field_instances` (`:191-195`) precisely because the
slot cannot prove placement.

### 2.6 `detour-counter-hooks-dead` — rows 217, 950

> 并将下一次的移动掷骰变更为1d6

The counter path sets the player latch `ctx::state::set(player_id, NEXT_ROLL, 1)`
(`rules/cards/card-ag/src/detour.rs:281`). The two entries that read it are a
`DiceRollPlan` filter (`:81`) and a `DiceRollAfter` consume (`:87`), and both
are **hook** entries. A counter's card is not placed, and hook dispatch only
reaches placed (or lingering) instances, so neither ever runs; the TODO at
`:276-281` records it.

### 2.7 `hagumi-multi-standin-abi` — row 252

> 你经过育美标记时可在那格强制停下并获得2000资金

`hook` (`rules/cards/card-hhw/src/hagumi_marks.rs:141-191`) asks, gains 2000
and removes one mark per job. Two stand-ins mean two asks, 4000 and two marks
for one pass. The cleanup `if !any_marks(player_id) { ctx::set_dest(…) }`
(`:184-186`) files only the copy that reaches it after the *last* removal
(TODO `:181-183`).

### 2.8 `sparkler-per-instance-counter-cap` — row 342

> 放置2个奇迹水晶（上限2）

`ctx::add_crystals(2, 2)` (`rules/cards/card-mujica/src/sparkler.rs:47`) clamps
that one call; the TODO at `:43-46` states that `add_counter_at`'s `max` is
call-local and no per-instance cap is stored (`ops.rs:2148-2160`). Any external
add can take the count to 3, and `turn_end` then pays a third extra turn.

### 2.9 `umiri-user-key-per-instance` — row 445

> 将此卡放置于在此卡使用者下一名行动的玩家场上

The user is written to the **holder's** slot `umiri_user`
(`rules/cards/card-mujica/src/umiri_card.rs:78-83`, read at `:32,36,132,170,198`),
so two copies on one holder overwrite each other. The TODO at `:78-82` says
per-instance state exists (`set_prop`) "but a pre cannot read props".

### 2.10 `hina-clause-2-swap-missing` — rows 487, 488, 490, 1555

> （2）[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。

`rules/cards/card-pp/src/hina_sound.rs:87-96` has two TODOs: there is no
RealId marking on the placed substitute and no `Fx.Actions` surface to offer
the swap. The card marks nothing after `ctx::play_card` (`:81`) and leaves play
(`:86`), so no later code can find the substitute. The offer surface today only
enumerates `skill:*` field instances with an `On::Play`
(`crates/game-core/src/engine/mod.rs:1316-1331`); a non-skill card cannot be
pressed.

`[共鸣]` is the derived fuel card `PP:[衍生]共鸣`, spent by
`resonance::try_resonance` (`rules/cards/card-pp/src/resonance.rs:44-75`); the
gate spelling used elsewhere is `ctx::hand_count(player, "PP:[衍生]共鸣") > 0`
(`see_you_tomorrow.rs:51`).

### 2.11 `cs-crystal-cap-read` — rows 301, 1056

> 移动到另一张可以放置奇迹水晶的卡上。

`Slot::add` (`rules/cards/card-mujica/src/crystal_swap.rs:57-61`) passes
`max = 0` (uncapped) and the destination filter (`:152-156`) offers every
card, including one already at its printed 上限, then overfills it. The TODO at
`:58-60` and `:152-155` asks for a per-uid cap read and a clamped add.

### 2.12 Shared root causes

1. **Raises drop identity.** `note_counter_changed` discards the uid
   (`wasm_rules.rs:648-656`); no raise carries a raise-id; `card_is` is a name
   compare; `collect_hook_jobs` is per-instance but nothing tells a guard which
   instance the raise is about.
2. **No counter-like change point for marks.** Mark mutations
   (`ops.rs:1920,2032,2092,2111,2001,2123`) are invisible to hooks, so a
   lifetime that follows a count cannot be maintained outside the card's own
   bodies.
3. **No lifetime edge between instances.** `place_card_on` creates a child row
   with no creator; destruction walks only mark/token units.
4. **The cap is call-local.** There is no per-instance maximum store.
5. **The offer surface is `skill:`-only.** A placed instance cannot expose a
   named action, and no per-instance identity (`RealId`) is stored.
6. **Per-instance stores exist but are not reached.** `props`/`counters` target
   `current_uid`; a pre needs an instance accessor over the candidate
   (`card.counter` already exists), and a hand-played counter needs a live
   instance to dispatch to.

---

## 3. Capability

**Invariant (one generalization): every raise names the subject instance or
row, every counter-like store is addressed by uid, and every instance may own
other rows for lifetime purposes.**

Concretely:

- A raise carries a monotonically unique `uid` and, where the raise is about
  one instance or row, an `instance` uid. `-1`/`0` mean "none".
- The engine keeps a transient **resolution stack** of `(card, instance)` frames
  so a card can ask whether it is currently resolving and its own hooks hear
  raises during its resolution.
- Mark mutations are logged and raised, with the mark's `instance` (creator),
  owner, tile, kind and signed delta.
- `FieldCard` gains a creator (`created_by`) and a per-counter maximum
  (`counter_max`); destruction follows `created_by`; adds clamp at the stored
  maximum.
- A field instance may carry a `real_id` naming the card whose effect it
  should run, and a provider card may declare a named, offerable **action**;
  the offer carries the instance uid.

**Why per-holder / per-card stores are insufficient.** A holder slot is shared
by two copies of one card on one field (items §2.2, §2.4, §2.5, §2.9) and survives
the card (1.5). A card-id key is not an identity either: `card_is` admits every
copy (§2.2), and a `place_mark` issued before placement stamps no instance at
all (§2.3). Only an engine-assigned instance/raise id distinguishes the copies
that the rulebook distinguishes (「该卡」 / 「每个」 / 「此卡」).

**Scope boundary.** The mechanism is generic. No card name, character, band or
display string enters an import; display text stays in `Msg` keys; no import
pauses for a decision (hina's prompts ride the existing `ask_*` + `Pause`
machinery inside the card body).

---

## 4. Surface

Legend: **new** = new name on the guest surface; **extended** = existing name
whose contract widens; **wire-breaking** = a byte/enum change visible in saved
or recorded data or in the client protocol.

### 4.1 Guest externs — `crates/card-sdk/src/ctx/sys.rs`

Every entry gets the wasm32 import, the native `link_name` shim in
`crates/game-rules/src/native_shims.rs`, and the `cfg(kani)` shim in
`crates/card-sdk/src/ctx/sys_kani.rs` (one `Op` arm each).

```rust
// trigger payload (new; hook reads only)
ruleset_trig_uid() -> i32;              // the raise's unique id; 0 outside a raise
ruleset_trig_instance() -> i32;         // the subject instance/row uid; -1 none

// resolution stack (new)
ruleset_resolving(ptr, len) -> i32;     // 1 when `id` is on the resolution stack

// real id / action offers (new; two-call strings, 0 bytes = none)
ruleset_real_id_at(uid, ptr, len) -> i32;      // byte length of the real id, 0 none
ruleset_set_real_id_at(uid, ptr, len) -> i32;  // 1 set, 0 no such instance
ruleset_real_is_at(uid, ptr, len) -> i32;      // 1 when the instance's real id is `id`

// per-instance counter maximum (new)
ruleset_counter_max_at(uid, ptr, len) -> i32;      // 0 = uncapped / absent
ruleset_set_counter_max_at(uid, ptr, len, max) -> i32; // 0 clears; returns stored
```

`ptrs` are guest pointers, `len` bytes of UTF-8, the established two-call /
`write_str` pattern (`hostfns.rs:3415-3420`). `uid` of a non-existent instance
reads `-1`/`0`/`0` per above and all writers return `0`.

### 4.2 Guest wrappers — `crates/card-sdk/src/ctx.rs`

```rust
// in `pub mod trigger`
pub fn uid() -> i32;          // raise id
pub fn instance() -> i32;     // subject instance, -1

pub fn resolving(id: &str) -> bool;
pub fn real_id_at(uid: i32) -> Option<NameId>;
pub fn set_real_id_at(uid: i32, id: &str) -> i32;
pub fn real_is_at(uid: i32, id: &str) -> bool;
pub fn counter_max_at(uid: i32, name: &str) -> i32;
pub fn set_counter_max_at(uid: i32, name: &str, max: i32) -> i32;
```

Parameter semantics:

- `trigger::uid()` — unique per raise, stable for every job the raise
  dispatches, fresh on a replay pass (a pass re-runs from a snapshot, so a
  claimant must re-claim). `0` outside a raise.
- `trigger::instance()` — the instance whose counter changed
  (`counterChanged`) or the mark's creating instance (`markChanged`); `-1`
  otherwise.
- `resolving(id)` — is a body of `id` on the current run's resolution stack.
- `real_id_at` / `set_real_id_at` — the substitute marker on an instance,
  written by the stand-in's play; `set_real_id_at(uid, "")` clears.
- `counter_max_at` / `set_counter_max_at` — a hard per-instance ceiling for one
  named counter. `0` = uncapped. `add_counter_at` / `add_counter` /
  `add_crystals` clamp to it in addition to their call-local `max`.

### 4.3 Wire enums and kinds — `crates/card-sdk/src/abi.rs`, `kinds.rs`

```rust
// engine trigger ids (104 retired; 105–110 taken; this group lands first, so
// the 111–113 band is its README allocation)
pub const SkillFired: Self = Self::engine(111);   // wire "skillFired", hook-only
pub const MarkChanged: Self = Self::engine(112);  // wire "markChanged", hook-only
pub const Actions: Self = Self::engine(113);      // wire "action", offer-only
```

- `Action::SkillFired(pre, guard, body)`,
  `Action::MarkChanged(pre, guard, body)` — added to the `action_kinds!`
  engine table (`kinds.rs:638`); both are `is_hook_only` so no counteraction
  window opens.
- `Action::Actions(pre, guard, body)` — offer-only; the entry **must** carry
  `On::id("action")`, and `abi::entry::ACTION` (`"action"`) is a new constant
  beside `PRESS`, so `build-ruleset` reserves the id: a card already declaring
  `On::id("action")` must be caught by the duplicate-id check before this
  lands. It never dispatches from a raise.
- **Relation to G05.** G05's press surface is the placement-bound half of the
  same channel (its `SkillAction.uid` / `.entry` and its `invoke_entry`). The
  two must land on one shape: this group supplies the raise-kind and the uid
  payload, G05 supplies the offer/query side; neither defines a second
  `"press"`-style action id.
- `Trigger` gains `pub uid: i32` and `pub instance: i32` (defaults `0`/`-1`);
  wire accessors `ruleset_trig_uid` / `ruleset_trig_instance`.
- `kind_table_pin` and the wire-name round-trip test are extended with the
  three names.

### 4.4 Client / view wire — `SkillAction`, `NetMessage`

`SkillAction` (the existing offer row) is **extended**:

```rust
#[serde(default = "default_act")]   // "skill"
pub act: String,                    // "skill" | "action"
#[serde(default = "default_uid")]   // -1
pub uid: i32,                       // the instance the offer is for
```

A skill offer repeats `act = "skill"` and its uid. An action offer sets
`act = "action"`, `uid`, `id` = the **host** instance's card id (what the
player sees), `source` = the provider card id, and `uid` the host instance. `NetMessage.act` accepts
`"action"` with `value` = the uid; `NetMessage.value` is the only vessel, so a
uid of `0` is invalid.

### 4.5 `ManifestOn` changes

None. The three kinds reuse the existing `On::Action` manifest encoding; only
the enum value and the mandatory `"action"` slug are new. `on` entry order and
existing ids are unchanged.

### 4.6 Replaced / extended exports (host side)

| export | state | change |
|---|---|---|
| `ruleset_add_counter_at` | extended | clamps at the instance's stored maximum (`counter_max`) after the call-local `max` |
| `ruleset_place_card_on` | extended | stamps `created_by = current_uid` when the run has an instance |
| `ruleset_trig_name` | extended | also set on `markChanged` (the mark kind) |
| `ruleset_trig_value` | extended | mark delta |
| `ruleset_trig_tile` / `trig_target` | extended | mark tile / mark owner |
| `view_extra_parts` (engine, not an export) | extended | emits action offers |

No signature is removed and no existing byte layout changes.

---

## 5. Engine wiring

### 5.1 Raise identity (`uid`)

- `MatchState.trigger_uid: i32` — `#[serde(default)]`, monotone, never reused.
  It is the analogue of `next_mark_uid` (`state.rs:179-184`), including the
  floor at `max` seen in existing runs so a restored older save cannot collide.
- The **single** assignment point is the engine choke point
  `play.rs:5017-5021` (`counteract`), covering both the engine's `raise` and
  the guest bridge `Run::raise_core` / `counteract`: `t.uid = next_trigger_uid()`.
  A trigger built directly by a test keeps `0` and reads as "no identity".
- `Ruleset::counteract` copies `t.uid` into the guest `Trigger`
  (`world.rs:35`) and `hostfns::trig_uid` returns it. All jobs of one raise
  share the value because the same `Trigger` is cloned to every drive.

### 5.2 Subject instance (`instance`)

- `counterChanged`: `note_counter_changed` (wasm_rules.rs:648) extends
  `counter_log` to `(owner, card, uid, name, change)`; `commit_after`
  (`:4858`) stamps `t.instance = Some(uid)`.
- `markChanged`: the drain sets it from the mark row's `instance`.
- `skillFired`: unused (`-1`).

### 5.3 Mark mutation log and `markChanged`

`World.mark_log: Vec<MarkLog>` — `#[serde(skip)]`, drained at commit.
Appended by every mutator: `place_mark` / `place_mark_pair` (per row),
`bump_mark` (signed), `remove_marks` (per row, negative), `remove_mark_by_uid`,
and `destroy_instance_units` (per removed row, negative). A row is
`{ uid, tile, kind, instance, owner, card, delta }`.

`commit_after` drains the log after `counter_log`, one raise per row:
`kind = "markChanged"`, `player_id = owner`, `tile`, `name = kind`,
`value = delta`, `t.card = card`, `t.instance = instance`. A row with no
creator still raises with `instance = -1`.

The drain also runs at the end of `remove_from_game`
(`play.rs:4253-4288`) so a purge's removals are raised even when no further
run commits. Guard against re-entrancy: a `markChanged` body that writes marks
raises again; the existing chain depth bounds it, and the contract requires the
listener's own write to be idempotent (black's `sync_circle_instance` is).

### 5.4 Resolution stack and `skillFired`

- `Cx.resolving: Vec<(String, i32)>` — transient. Pushed in
  `Ruleset::drive_inner_body_from` (`wasm_rules.rs:3064`) at body entry when
  the top is not already `(card_id, uid)`; popped when the body completes.
  A body that halts (ask / suspend) leaves its frame: an ask re-runs the
  routine from a snapshot with a fresh `Cx`, and a suspend resumes the same
  drive, so the top check prevents a duplicate and the resume's completion
  pops it.
- The stack is mirrored into `World.resolving: Vec<String>` (`#[serde(skip)]`)
  by the same push/pop so `fill_window` can snapshot it; `SnapView` gains
  `resolving()`, and the CEL vocabulary gains `resolving('card:id')`
  (`crates/cond/src/vocab.rs`, `view_impl.rs`; `Scope::Window`).
- `collect_hook_jobs` adds, **before** the field loop, one job per resolving
  frame that declares the kind, with the frame's instance uid, deduped against
  field jobs by uid. A placed card still hears every raise exactly once.
- `skillFired` is raised once per **body entry** in `drive_inner_body_from`,
  before the pass loop, when `card_id.starts_with("skill:")`: a fresh
  `CoreTrigger::new("skillFired", call.player_id())`, `card = Some(card_id)`,
  `by_card = Some(card_id owner)`, then `self.counteract`. A counter may cancel
  it; a cancelled `skillFired` skips the body (the fire was negated). One
  raise per entry per pass; replay passes re-raise, matching `cardUsed`'s
  existing model (`play.rs:979-990`), so listeners must be idempotent per
  raise.

### 5.5 Creator binding and destruction

- `FieldCard.created_by: Option<i32>` — `#[serde(default, with =
  "opt_i32_neg1")]`, stamped by `World::place_card_on` from the running
  instance (`current_uid`, passed down by `Run::place_card_on`). `None` when
  the placing run has no instance (a hand play, a tile's own body).
- `destroy_instance_units(uid)` (`ops.rs:2123`) first collects live instances
  whose `created_by == Some(uid)` and `unplace_at`s them (recursively, depth
  cap 8), then clears mark/token units. It runs even when `unplace_at` already
  removed the parent, because bankruptcy clears the player's field
  (`play.rs:4279`) **before** destroying owned uids (`:4287-4288`).

### 5.6 Counter maximum

- `FieldCard.counter_max: BTreeMap<String, i32>` — `#[serde(default,
  skip_serializing_if = "…::is_empty")]`.
- Static caps are declared as card props with the reserved prefix
  `counter_max.` (`abi::prop::COUNTER_MAX_PREFIX = "counter_max."`, e.g.
  `("counter_max.crystals", 2)`). At placement (`Run::place_card*` /
  `place_card_on`) the prefixed keys are split out of `props_of` into
  `counter_max`; all other props still land in `props`.
- `World::add_counter_at` clamps, in order: at 0, at the call-local `max` when
  `> 0`, at the stored cap when `> 0` (`ops.rs:2148-2160`).
  `set_counter_max_at` writes the store (`0` removes the key). The Kani arm
  mirrors both.

### 5.7 Real id and action offers

- `FieldCard.real_id: Option<String>` — `#[serde(default,
  skip_serializing_if = "Option::is_none")]`.
- A **press** entry on the real-id card is offered too, but by G05 §4.6: it
  resolves the instance's effective card (`real_id` or the card itself) on
  every press read, so the offer loop below is for `"action"` entries only and
  hina's §6.10 clause (2) reaches the field through G05's press surface
  (`On::Press("press_swap", …)`, G05 §6.5) rather than an `ActionKind::Actions`
  entry.
- `view_extra_parts` (`mod.rs:1280-1348`) gains a second loop over the
  viewer's field instances carrying a `real_id` whose card declares an
  `"action"` entry. Per offer: pre + guard are probed with `current_uid = uid`
  (the same `admits` machinery as the skill offer, `mod.rs:1318-1331`);
  `enabled` is the probe result, `reason` the refusal; `act = "action"`,
  `uid`, `id` = host id. Guards must be pure (they run inside a view build).
- `why_not_act` (`mod.rs:2932`) gains `"action"`: ops stage + `!busy`, uid
  exists, holder is `i`, card face up, not immune, and the provider entry
  admits. `Routine::Act` (`:2901`) dispatches `"action"` to a new
  `Cx::use_action(i, uid)` which resolves `real_id`, then
  `rules.act(cx, i, provider, uid)`: `Call::Play` with the host uid bound as
  `current_uid` (the `drive` parameter, like `rules.play`'s skill binding at
  `wasm_rules.rs:9392-9400`). No `cardUsed` / `skillFired` is raised for an
  action.
- The host id stays the instance; the provider's `pre` sees `card.placed`
  true, `actor` = the presser and `owner` = the host instance's owner.

### 5.8 Kani model representation

- `World` gains symbolic `trig_uid: i32`, `trig_instance: i32`, a bounded
  `resolving: [u16; MAX_RESOLVING]` + length, `real_id: [NameId; MAX_FIELD]` +
  a `real_tag` bit per slot, and `counter_max` in the per-field counter area.
- Each new op has an `arm_*`; unmodeled ops keep failing loudly
  (`rules/kani/src/host.rs:1-10`).
- Invariants to assert: `add_counter_at` never exceeds a positive stored cap; a
  counter set with a cap and then set uncapped is no longer clamped; a
  `created_by` child disappears from the field after the parent's
  `unplace_at`; `real_is_at` is true only after `set_real_id_at` and false
  after the instance is unplaced; `set_dest` applies to `last_uid`; a
  non-empty `resolving` contains the currently driven id.

---

## 6. Card usage

### 6.1 `rui-x-write` (before → after)

```rust
// before (rui_devil.rs:29-50)
for &o in &others {
    ctx::transfer(player_id, o, 100, &why)?;
    ctx::set_slot(player_id, X, 5);          // unconditional
}
…
ctx::set_slot(player_id, X, 5);              // trailing, unconditional

// after
// `actor` is the fired skill's owner and `owner` is this card's user, so the
// pre already says 「你的技能」; `resolving` keeps it inside this resolution.
On::Action(Action::SkillFired(
    "resolving('Mor:（Rui）正论恶魔') && actor == owner", None, on_skill))
…
fn on_skill(player_id: i32) -> Asked {
    ctx::set_slot(player_id, X, 5);
    ctx::log(player_id, &Msg::new(key!("rui_devil_crit")));
    Ok(())
}
```

The transfer legs stay; the three writes and the trailing log move into the
hook.

### 6.2 `starry-trigger-uid`

```rust
// before (starry_night.rs:117-124)
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && trigger::name() == counter::CRYSTALS

// after
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && trigger::instance() == ctx::self_uid()
        && trigger::name() == counter::CRYSTALS
```

### 6.3/6.4 `black-marker-hook` and `black-copy-crystals`

```rust
// before (black_suits.rs:58-67, before the instance exists)
ctx::place_mark(group, MARK, "", player_id, ctx::self_uid(), 1, &note, Fresh);
…
if !ctx::is_placed() { ctx::set_dest(Field); ctx::place_card(player_id, ID, &note); }

// after: place first, then mark, so the row is bound to the stand-in
if !ctx::is_placed() { ctx::set_dest(Field); ctx::place_card(player_id, ID, &note); }
let row = ctx::place_mark(group, MARK, "", player_id, ctx::self_uid(), 1, &note, Fresh);
```

```rust
// new MarkChanged entry (external paths while the stand-in lives)
On::Action(Action::MarkChanged(
    "card.placed && tile.id == tile_named('弦卷集团')",
    Some(changed_is_my_crystal), resync))
fn changed_is_my_crystal(_p: i32) -> bool {
    trigger::name() == MARK && trigger::instance() == ctx::self_uid()
}
fn resync(_p: i32) -> Asked { sync_circle_instance(trigger::tile()); Ok(()) }

// pass body: one removal per raise, every copy files
fn pass_tile(player_id: i32) -> Asked {
    let t = trigger::tile();
    let key = alloc::format!("black.pass.{}", trigger::uid());
    if ctx::slot(player_id, &key) == 0 {
        ctx::set_slot(player_id, &key, 1);
        ctx::bump_mark(t, &MarkFilter::any().kind(MARK).owner(player_id), -1);
    }
    sync_circle_instance(t);
    if own_crystals(t, player_id) == 0 { ctx::set_dest(ctx::Dest::Graveyard); }
    Ok(())
}
```

`pass_tile_guard` and its residual `count_marks` check are dropped (the
identity does the work); `sync_circle_instance`'s `place_card_on` call now
stamps `created_by` automatically.

### 6.5 `tomori-x-instance-scope` (demote to A)

```rust
// before (want_human.rs:58-64)
ctx::set_slot(player_id, SLOT_X, x);
ctx::set_slot(player_id, SLOT_AB_BEFORE, 0);
// after
ctx::set_prop(SLOT_X, x);          // current_uid = the instance just placed
ctx::set_prop(SLOT_AB_BEFORE, 0);
```

Readers become `ctx::prop(SLOT_X)` / `ctx::prop(SLOT_BOOST)` /
`ctx::prop(SLOT_AB_BEFORE)`. `tomori_inner_shout` finds the instance and uses
the existing cross-instance form:

```rust
// before (tomori_inner_shout.rs:66-67)
let old = ctx::slot(player_id, SLOT_X);
let mut reset = old > 0 && want_human_placed(player_id);
// after
let target = ctx::field_instances(player_id).into_iter()
    .find(|(_, c)| c.as_str() == "CRYCHIC:想要成为人类").map(|(u, _)| u);
let old = target.map_or(0, |u| ctx::prop_at(u, SLOT_X));
…
ctx::set_prop_at(target.unwrap(), SLOT_X, x);
```

The values now die with the instance (props are destroyed with it); no slot
survives a re-place.

### 6.6 `detour-counter-hooks-dead` (demote to A)

```rust
// before (detour.rs:281)
ctx::state::set(player_id, NEXT_ROLL, 1);
// after
ctx::linger(player_id, 0);        // the card's own def becomes a turn instance
ctx::state::set(player_id, NEXT_ROLL, 1);
```

The existing `DiceRollPlan` / `DiceRollAfter` entries then dispatch to the
lingering instance and the pre `slot('detour_next_roll') != 0` still owns the
gate. The instance is cleared at turn start (`TurnCtx.lingering`), so no
cleanup is added.

### 6.7 `hagumi-multi-standin-abi` (demote to A)

```rust
// before (hagumi_marks.rs:170-186) — every job asks and pays
ctx::gain(player_id, 2000, …)?;
ctx::bump_mark(t, &mark_filter(player_id), -1);
if !any_marks(player_id) { ctx::set_dest(ctx::Dest::Graveyard); }

// after — claim per raise, every copy files itself
let key = alloc::format!("hagumi.stop.{}", trigger::uid());
if ctx::slot(player_id, &key) == 0 {
    ctx::set_slot(player_id, &key, 1);
    if !ctx::ask_yes(…) { return Ok(()); }
    if !ctx::gate(…) { return Ok(()); }
    if trigger::move_remaining() > 0 { ctx::plan::set_stop_at(t); }
    ctx::gain(player_id, 2000, …)?;
    ctx::bump_mark(t, &mark_filter(player_id), -1);
}
if !any_marks(player_id) { ctx::set_dest(ctx::Dest::Graveyard); }
```

`set_dest` already applies to the drive's `current_uid` (the acting copy) at
`commit_after` (`wasm_rules.rs:4920`), so no per-uid dest import is needed.

### 6.8/6.11 `sparkler` and `crystal_swap`

```rust
// sparkler: the printed cap is a declared static prop
.props(&[("counter_max.crystals", 2)])
// body unchanged; the add now clamps at the stored cap
ctx::add_crystals(2, 2)?;

// crystal_swap: destination excludes a full card
let can_take = |s: &Slot| {
    let cap = ctx::counter_max_at(s.uid, counter::CRYSTALS);
    cap == 0 || s.get() < cap
};
let to: Vec<usize> = (0..all.len()).filter(|&i| i != src && can_take(&all[i])).collect();
if to.is_empty() { /* the existing "no move" branch */ }
```

`Slot::add` keeps `max = 0`; the stored cap is the clamp.

### 6.9 `umiri-user-key-per-instance` (demote to A)

```rust
// before (umiri_card.rs:83, 32)
ctx::set_slot(next, USER_KEY, player_id + 1);
On::Action(Action::TurnStart("card.placed && actor == slot('umiri_user') - 1", …))

// after — the existing on-card counter store
ctx::set_counter(USER_KEY, player_id + 1);
On::Action(Action::TurnStart(
    "card.placed && card.counter('umiri_user') - 1 == actor", …))
```

`card.counter` is already in the condition vocabulary
(`crates/cond/src/vocab.rs:685-687`), so two copies keep separate users and
the value dies with the instance. `turn_start` / `turn_end` swap `ctx::slot` for
`ctx::counter` / `ctx::set_counter`, and the hop re-sets the counter on the new
instance.

### 6.10 `hina-clause-2-swap-missing`

Press half is G05 §6.5 — the swap is declared `On::Press("press_swap", …)` and
offered through the press surface's effective-card resolution, so the
`On::Action(Action::Actions(…))` spelling in the sketch below is superseded
there; the marker (`set_real_id_at`) and crystal carry are adopted verbatim.

```rust
// after placement of the substitute (hina_sound.rs:69-82)
let new_uid = <the instance `play_card`/`place_card` just placed>;
ctx::set_real_id_at(new_uid, ID);

// new provider entry on HINA_SOUND
On::Action(Action::Actions(
    "actor == owner && card.placed", Some(has_resonance), swap)).id("action")

fn swap(player_id: i32) -> Asked {
    let uid = ctx::self_uid();
    let crystals = ctx::counter_at(uid, CRYSTALS);
    let cur = ctx::real_id_at(uid).map(|c| c.as_str()); // the substitute's own id
    let others: Vec<&str> = SUBS.iter().copied().filter(|s| Some(*s) != cur).collect();
    if !crate::resonance::try_resonance(player_id)? { return Ok(()); }
    let i = ctx::ask_card(player_id, &title, &ask, &others)?;
    ctx::unplace_at(uid);
    let before: Vec<i32> = ctx::field_instances(player_id).iter().map(|(u, _)| *u).collect();
    ctx::play_card(others[i], player_id)?;
    let Some(new_uid) = ctx::field_instances(player_id).iter()
        .find(|(u, c)| !before.contains(u) && c.as_str() == others[i]).map(|(u, _)| *u) else { return Ok(()) };
    ctx::set_real_id_at(new_uid, ID);
    ctx::add_counter_at(new_uid, CRYSTALS, crystals, 0);
    Ok(())
}
```

The offer is `act = "action"`, `uid` = the host substitute, `id` = the host
card. `has_resonance` is `ctx::hand_count(player_id, "PP:[衍生]共鸣") > 0`
(pure). The prompt inside the body uses the ordinary `ask_card` + `Pause`
shape; the replay stores the answer. The crystals are read as a plain counter
and re-added at the new instance, which is what keeps them across
`unplace` + re-place.

---

## 7. Save / record / replay

| added field | serde | SAVE_VERSION |
|---|---|---|
| `FieldCard.counter_max` | `default`, skip-if-empty | unchanged |
| `FieldCard.created_by` | `default`, `opt_i32_neg1` | unchanged |
| `FieldCard.real_id` | `default`, skip-if-none | unchanged |
| `MatchState.trigger_uid` | `default` | unchanged |
| `World.mark_log` | `skip` (rebuilt from re-raise; rows are transient) | unchanged |
| `World.resolving` | `skip` (transient stack) | unchanged |

`SAVE_VERSION` stays **12**. Every saved field is `#[serde(default)]`, which
`docs/ENGINE.md:597-602` defines as no bump; the new map/option fields also
skip when empty, so an untouched match's save bytes are unchanged. The
transient logs are safe to drop because a save happens at a routine boundary:
a pending open chain keeps its trigger in `pending`, and the uid of an
already-raised trigger has no saved consumer. `trigger_uid` itself is saved
because it must not repeat after a restore; it is defaulted so pre-v105 saves
load and the allocator floors at any larger existing value.

`RECORD_VERSION` stays **3**. The record is an input log
(`crates/game-core/src/record.rs:43`); the only input change is the
`act = "action"` string plus its `value`, both inside existing message fields.
Triggers are transient and never recorded. Replays ride archived engine
bundles, so an older record replays under the v104 engine that knows nothing
of the new kinds; no record migration is owed.

Checkpoint hashes are A/B'd as usual (`examples/ckpt_equiv.rs`); the
deterministic parts of the design are that `trigger_uid` is derived (not
input) and every claim key is derived from a raise id, so two runs over the
same input produce the same key sequence.

---

## 8. Versioning

`ABI_VERSION` goes 104 → **105** (this group lands first in the README order;
any reordering takes the next free number instead -- the README renumber rule).
The reason is additive but incompatible: a
v105 ruleset imports `ruleset_trig_uid`, `ruleset_trig_instance`,
`ruleset_resolving`, `ruleset_real_id_at`, `ruleset_set_real_id_at`,
`ruleset_real_is_at`, `ruleset_counter_max_at` and
`ruleset_set_counter_max_at`, uses kind ids 111–113, and expects
`add_counter_at`'s stored-cap clamp and `place_card_on`'s creator stamp. A
v104 host has none of them. No existing import signature or byte layout
changes.

The exact doc line to add immediately below `ABI_VERSION` and above the newest
note (notes are newest-first under the constant):

```rust
/// v105: **per-instance identity and state**. Raises carry a unique `uid`
///    (`ruleset_trig_uid`) and a subject instance (`ruleset_trig_instance`);
///    a transient resolution stack is readable (`ruleset_resolving`);
///    `counterChanged` / `markChanged` name their instance; a per-instance
///    counter maximum is stored (`ruleset_counter_max_at` /
///    `ruleset_set_counter_max_at`) and `add_counter_at` clamps at it; an
///    instance stores a `realId` (`ruleset_real_id_at` /
///    `ruleset_set_real_id_at` / `ruleset_real_is_at`) and offers named
///    actions (`ActionKind::Actions`, wire `action`, offer rows carry
///    `act` / `uid`). New hook kinds `skillFired` (111) and `markChanged`
///    (112); `place_card_on` stamps the creating instance.
///    SAVE_VERSION / RECORD_VERSION unchanged.
```

`ABI_VERSION` is checked exactly at load, so old bundles are refused with the
usual version message; the archived bundles cover the old replays.

---

## 9. Tests and Kani

### 9.1 Black-box tests (one per item, `rules/tests/tests/`)

| item | test |
|---|---|
| 1 | `morfonica.rs::rui_x_only_when_the_users_skill_fires` |
| 2 | `morfonica.rs::starry_one_payout_per_instance` |
| 3 | `hhw.rs::black_suits_borrow_tracks_crystals_from_any_path` |
| 4 | `hhw.rs::black_suits_one_crystal_per_pass_with_two_copies` |
| 5 | `crychic.rs::want_human_x_is_per_instance_and_dies_with_it` |
| 6 | `ag.rs::detour_counter_arms_the_next_roll_after_settle` |
| 7 | `hhw.rs::hagumi_one_reward_per_mark_with_two_standins` |
| 8 | `mujica.rs::sparkler_cap_blocks_a_third_external_crystal` |
| 9 | `mujica.rs::umiri_two_copies_keep_their_own_user` |
| 10 | `pp.rs::hina_resonance_swap_keeps_crystals` |
| 11 | `mujica.rs::crystal_swap_skips_a_full_destination` |

Test 1 covers both directions: a fire during the resolution writes X, and a
resolution with no fire (all payees immune or a stripped skill) leaves X
untouched. Test 10 asserts the second copy's `real_id` after the swap and the
crystal count across it. Test 3 exercises an external `bump_mark` and a
bankruptcy purge, and asserts the borrowed `tile:circle` is gone.

### 9.2 Kani obligations

- `proofs/mor.rs::C-MOR-rui-demon-03` — `resolving` + `skillFired` ⇒ exactly
  one X write per fire; no fire ⇒ X unchanged.
- `proofs/mor.rs::C-MOR-starry-night-02` — guard admits iff
  `trig_instance == last_uid`.
- `proofs/hhw.rs::C-HHW-black-suits-02` — a `markChanged` add/remove drives
  attach/drop; a child with `created_by` leaves the field on
  `unplace_at(parent)`.
- `proofs/hhw.rs::C-HHW-black-suits-03` — two drives with one raise id produce
  one `BumpMark(-1)` and two `set_dest` rows.
- `proofs/crychic.rs::C-CRY-want-human-04` — X written on instance A is not
  readable from B, and disappears after A leaves.
- `proofs/ag.rs::C-AG-detour-02` — a lingering instance hears `DiceRollPlan`
  and the plan base becomes 1d6.
- `proofs/hhw.rs::C-HHW-hagumi-03` — one mark, two drives, one `Gain`, two
  dest writes.
- `proofs/mujica.rs::C-MUJICA-sparkler-02` — with the cap set, an external
  `add_counter_at` stops at the cap; clearing the cap unclamps.
- `proofs/mujica.rs::C-MUJICA-umiri-02` — two instances keep separate
  `counter(USER_KEY)` values.
- `proofs/pp.rs::C-PP-hina-03` — a swap keeps the crystal count, sets
  `real_id`, and is offered only with resonance in hand.
- `proofs/mujica.rs::C-MUJICA-crystal-swap-03` — the destination list excludes
  a full instance.

Bounded-stub invariants per new op are in §5.8. Every new `Op` also lands in
`hostops_smoke.rs`; `kind_table_pin` pins the three names and ids.

---

## 10. Alternatives considered

1. **A holder/card-id-keyed dedupe in the engine** ("one payout per
   `(card_id, owner)` per raise"). Rejected: two copies of the same id on one
   field are precisely the failing case (§2.2, §2.4, §2.7), and it cannot
   express "this copy's counter".
2. **Per-card slots with a card-instance suffix key** (the tomori latch
   pattern, `tomori_inner_shout.rs:164-170`). Rejected as the general answer:
   the key needs `self_uid()` in every reader and writer, which a **pre** for
   umiri/black cannot call; and item 5's whole defect is that slots survive the
   card and are shared. Kept for claim latches where an instance-qualified
   string suffices.
3. **Dedupe by `MarkFilter::src`** for the black pass. Rejected: `src` is
   provenance, not a lifetime owner, and the marks are currently stamped with
   no instance at all; `instance` is the owner the purge already uses.
4. **A per-uid `set_dest_at` import** for hagumi/black. Rejected after
   checking `commit_after`: `set_dest` already applies to the drive's
   `current_uid` (`wasm_rules.rs:4920`), and a hook job binds `current_uid` to
   the acting copy, so per-copy filing is a card change.
5. **A boolean `mark_added` flag on `counterChanged`.** Rejected: a mark
   mutation is not necessarily a counter write (pairs, removals by filter,
   destruction) and the payload (tile/owner/instance) is exactly what a
   listener needs; two fields would still be needed.
6. **A `Message`/`send`-based handshake for hina.** Rejected: the swap must be
   *offered* to the player with an enabled/reason state on the view;
   `On::Message` is a blind push and would need a parallel offer table.
7. **Storing the counter cap in `props` only** (`props["max.crystals"]`) and
   clamping there. Rejected: `add_counter_at` is engine-side and would have to
   know the prop prefix anyway; a typed `counter_max` map keeps the clamp
   local and the save field skippable.

---

## 11. Open questions

1. **Winner among two `want_human` copies.** `tomori_inner_shout` resets *one*
   X. Which instance when two are on the field — first by uid, or a pick? The
   sheet does not say. Design assumes the first (lowest uid) and leaves a pick
   open.
2. **Purge ownership of the black crystal kind.** `purge_markers_owned_by`
   (`play.rs:4331-4352`) deletes by `marker_owner` name across all owners, so
   a bankrupt black player also removes the *other* players' crystals of the
   same kind. Is that intended? Not ruled.
3. **Cancelled `skillFired`.** Design: a counter may cancel it, and a
   cancelled fire skips the skill body. Confirm that a listener may negate a
   skill's fire, not only rewrite X.
4. **`markChanged` re-entrancy.** A listener that writes marks raises again;
   the chain depth bounds it, but the contract "listener writes must be
   idempotent per raise" is a ruling the doc assumes.
5. **`Actions` eligibility.** Should a face-down or immune host instance
   suppress its action offer? The sheet's clause (2) is silent; design mirrors
   the skill offer (face-down skipped, immunity checked) and marks it a
   judgement.
6. **Hina clause (2) frequency.** Is the swap once per turn, once per
   resolution, or unlimited while resonance is held? The sheet gives none.
   Design assumes one offer per activation, unlimited across the turn.
7. **Hina's inner play refusal.** If the newly played substitute refuses and
   stays in hand, the host instance is already unplaced and the crystals are
   lost. Re-place the old substitute and refund, or treat it as legal? Open.
8. **`trigger::uid()` on synthetic triggers.** A test-built trigger has `0`.
   Confirm that a claim key of `0` is acceptable (it is unique only within the
   test) or reserve `0` as "no identity" and skip claiming.
