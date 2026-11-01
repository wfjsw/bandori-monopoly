# 1. ABI design G01 — Per-instance identity and state

**Status:** revised design, 2026-10-31, against master `a0403648`. Design only;
no engine, SDK or card source is changed by this document.

This revision came out of a full review of the 2026-10-28 draft. The review
re-read every cited site against current master, traced the replay and suspend
paths, and checked the draft against G05 (a group it collides with). Section 1
records what changed; the rest of the document is the corrected design.

Closed backlog items (B class: needs an engine/ABI capability):

| # | slug | rows |
|---|------|------|
| 1 | `skill-fired-signal` (merged `rui-x-write`, `sayo-skill-used-event`, `tomori-x-instance-scope`, `umiri-user-key-per-instance`) | 232, 1243, 1244, **1481**, 184, 152, 1091, 445 |
| 2 | `starry-trigger-uid` | 1039 |
| 3 | `black-marker-hook` | 283, 1029 |
| 4 | `black-copy-crystals` | 285 |
| 5 | `tomori-x-instance-scope` (see 1) | 184, 152, 1091 |
| 6 | `detour-counter-hooks-dead` (not in section B; **already landed**, see §6.6) | 217, 950 |
| 7 | `hagumi-multi-standin-abi` | 252 |
| 8 | `sparkler-per-instance-counter-cap` | 342 |
| 9 | `umiri-user-key-per-instance` (see 1) | 445 |
| 10 | `hina-clause-2-swap-missing` (**moved to G05**; its lifetime edge to G08) | 487, 488, 490, 1555 |
| 11 | `cs-crystal-cap-read` | 301, 1056 |

Items 5, 6, 7 and 9 are **demoted to A** after this design: existing mechanisms
plus the payload from item 2 close them, with no new import. They stay in this
document because their fix depends on the identity payload designed here, and
because the demotion must be recorded rather than silently assumed.

All Chinese fragments below are quoted verbatim from `docs/rulebook/cards.json`
and are identified by their triage row id.

## 1. Review record — what this revision corrects

| # | draft said | why it was wrong | revision |
|---|---|---|---|
| R1 | `ActionKind::Actions` (kind 113), `entry::ACTION`, `act = "action"`, `SkillAction.act`, `NetMessage.value` | G05's press surface (landed after G01) already defines the placed-card offer: `On::Play(...).field()`, `NetMessage.instance`, `act = "press"`, `SkillAction.{instance,label}`. G01's `Actions` is a second offer channel for the same cards, and its only consumer (hina) is explicitly G05's press in G05 §9.5. | Drop the offer half entirely. G01 ships no offer wire; the effective-card store and accessors are G05 §6.2. Kinds go 111–112, not 111–113. Hina's card edit lands with G05. |
| R2 | uid is stamped only in `Match::counteract` (`play.rs:5918`) and `cardFired` is raised at body entry in `drive_inner_body_from` | `Run::raise_core` calls `Ruleset::counteract` directly, not `Match::counteract`, so bridge raises would keep uid 0. And `ctx::invoke_skill` / `ctx::play_card` (and G05/G13's `invoke_entry`) run a body through `call_entry` in the **guest** and never reach the host drive, so neither the signal nor a resolving frame would exist for them. | Stamp in `Ruleset::counteract`, the one choke both paths cross (§5.1). Raise the signal at the guest body entry (`body_enter`), which all three paths cross, and keep the frames host-side, pinned to the raise that carried the body (§5.4). |
| R3 | hook jobs are enumerated from fields only | A `markChanged` caused by destruction names an instance that is **already unplaced** by the time the raise commits (`unplace_at` removes before `commit_after`), so the field walk misses the very instance whose hook must resync/discard. | `collect_hook_jobs` also adds a job for the raise's subject instance (alive or dead) and one per resolving frame (§5.3). |
| R4 | umiri's fix reads `card.counter('umiri_user')` in a pre | `fill_candidate` resolves `card.*` by `(player, card id)` — the **first** copy — so two copies read one counter and item 9 stays broken. | `CandidateCtx` gains the running `uid`; `card.counter` / `card.tile` / `card.placed` resolve through it when set (§5.5). |
| R5 | the G01 README allocation gives no Kani `Op` ids | G01 adds six imports; each needs an `Op` and an arm (`sys_kani`, `kani_host`, `host.rs` are exhaustive). Without ids the Kani build cannot compile. | Take 341–346 and shift the later groups' bands (README table updated with this file). |
| R6 | claims for "one per raise" ride per-raise player slots (`ctx::slot(player, "black.pass.<uid>")`) | A per-raise key is never cleaned; every pass/mark trigger adds a permanent state key. | Claim on **one fixed-key seat latch** holding the raise uid (`state::get(player, CLAIM) != trigger::uid()`): one key per card, overwritten in place, shared by every copy so two copies answer one raise once (§6.3, §6.4, §6.7). |
| R7 | `remove_from_game` drains the mark log "at its end" | `remove_from_game` returns `Vec<usize>`, not `Flow`, and its two callers do not propagate a raise. | It returns `Flow<Vec<usize>>`; both callers `?` (§5.3). |
| R8 | `counterMax`, `createdBy` (created_by), `effectiveCard` stamping | hina's substitute would be destroyed with hina's own instance when hina is banished (`:85`) — `created_by` alone kills the stand-in. | Lifecycle (`created_by` + `set_created_by_at`) moves to G08 and the substitute marker (`effective_card`, formerly `real_id`) to G05; the cap stays here (R11). |
| R9 | cited line numbers from the 2026-10-28 base | Master moved (landing-log, walk-pace, detour `linger`, sparkler props). Every reference is re-read below; item 6 is already fixed on master. | References updated; §6.6 records the landed fix. |
| R10 | "RECORD_VERSION stays 3" | It is **4** (`record.rs:48`). | Corrected (§7). |
| R11 | one group carried three unrelated concerns | `effective_card` is offer/skin (G05 already enumerates presses and resolves an effective card) and `created_by` is zone lifecycle (G08 owns the zone writer, attachments and destruction). Keeping them here made G01 report on three subsystems at once and coupled its landing to two others. | G01 keeps only identity and its three stores: raise uid, subject instance, counter cap. `effective_card` (then spelled `real_id`) moves to G05 §6.2, `created_by` to G08 §4.9; their `Op` ids 347–349 stay with those groups in place. |

All line references below are against `a0403648`.

---

## 2. Problem

### 2.1 `rui-x-write` — row 232

> 在此卡结算过程中，每当你的技能被触发后，立即将x设置为5

`rules/cards/card-morfonica/src/rui_devil.rs` writes X unconditionally three
times: after every outbound `ctx::transfer` (`:37`), after every inbound one
(`:43`), and once trailing with the log (`:50-54`). The card is `On::Play` and
never places itself. The TODO at `:47-49` states what is missing: *"skill-fire
signal: nothing reports whether a skill fired while this body ran"*. A stripped
skill, an immune payee or the trailing write therefore still leaves X at 5
(item `current`). The sheet keys the write on 「每当你的技能被触发后」 — one
write per fire, and none without a fire.

The skill that fires here is `skill:八潮瑠唯:正论暴击`
(`rules/skills/skill-personas/src/rui.rs`), and **`On::Action(Pay)`** — a
post-settlement hook body dispatched by `collect_hook_jobs`
(`crates/game-rules/src/wasm_rules.rs:5292`) from the money pipeline, not by
`use_skill`. `use_skill` raises `cardUsed` (`crates/game-core/src/engine/play.rs:1168-1176`)
only on a press, so hook-driven skill bodies have no fire signal at all. The
card also cannot ask "did this fire happen during *my* resolution": `Run` has
one `current_card` (`wasm_rules.rs:222-228`) which a nested `play_card`
replaces, and no stack. rui is also never placed, so neither its own field
presence nor `self_uid()` can scope the hook.

### 2.2 `starry-trigger-uid` — row 1039

> 此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金

The guard at `rules/cards/card-morfonica/src/starry_night.rs:117-128` filters
by `trigger::card_is(ID)` and `trigger::name() == counter::CRYSTALS`. Both are
name compares. `collect_hook_jobs` dispatches every placed instance of the id;
there is no per-uid dedupe. Two copies on one field therefore both pass the
guard when **one** copy's counter changes, and both pay (TODO at `:120-122`).

The engine *has* the instance at the raise: `Run::note_counter_changed` is
called with `uid` (`wasm_rules.rs:650`) but drops it when pushing to
`counter_log` (`:657-658`, tuple `(owner, card, name, change)`), and
`commit_after` has nothing to stamp (`:4892-4899`).

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
rows (`crates/game-core/src/engine/ops.rs:2124-2131`), so the borrowed rule
outlives its crystal forever.

The crystal mark is also created **before** the stand-in is placed
(`black_suits.rs:58-67` then `:84-90`), so `ctx::self_uid()` is still `-1` at
`place_mark`: the wrapper stamps `instance = None` and `src = None`
(`wasm_rules.rs:1442-1450`, `:1502`). The rows are bound to no card at all, so
even a correct purge could not find them from the instance.

### 2.4 `black-copy-crystals` — row 285

> 你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。

`pass_tile` (`black_suits.rs:142-158`) bumps by `-1` unconditionally (`:151`).
The pass hook dispatches one job per placed copy, so two copies remove two
crystals on one pass (TODO `:148-150`). The residual guard `pass_tile_guard`
(`:135-137`) counts the owner's crystals and admits **both** jobs; only the
trailing `own_crystals == 0` check (`:154-156`) files, and only for the copy
that ran last.

No per-raise identity exists on a `Pass` trigger, so the two jobs cannot tell
they answer the same pass.

### 2.5 `tomori-x-instance-scope` — rows 184, 152, 1091

> 将此卡置于场上并从1-20间选择并声明X，将其写下

`want_human` writes X to a **player** slot (`rules/cards/card-crychic/src/want_human.rs:61`,
key `:39`) and every reader/"reset" goes through `ctx::slot`
(`want_human.rs:95,110,139,145`; `tomori_inner_shout.rs:66,91`). The comment at
`want_human.rs:57-60` states the defect: the value survives the card and two
copies share it, where the value belongs to the instance. `boost` and
`abBefore` (`:79`, `:84`) have the same shape.

`tomori_inner_shout` already implements the cross-instance half by hand:
`want_human_placed` scans `field_instances` (`:191-195`) precisely because the
slot cannot prove placement.

### 2.6 `detour-counter-hooks-dead` — rows 217, 950 (landed)

> 并将下一次的移动掷骰变更为1d6

The counter path sets the player latch `ctx::state::set(player_id, NEXT_ROLL, 1)`
(`rules/cards/card-ag/src/detour.rs:302`). The two entries that read it are a
`DiceRollPlan` filter (`:81`) and a `DiceRollAfter` consume (`:87`), and both
are **hook** entries. A counter's card is not placed, and hook dispatch only
reaches placed (or lingering) instances. Commit `0bb7e634` already added
`ctx::linger(player_id, 0)` at `:296`, binding the counter card as a turn-scoped
lingering instance, so both entries now dispatch. **No G01 work is owed
here**; §6.6 records the landed spell.

### 2.7 `hagumi-multi-standin-abi` — row 252

> 你经过育美标记时可在那格强制停下并获得2000资金

`hook` (`rules/cards/card-hhw/src/hagumi_marks.rs:147-197`) asks, gains 2000
(`:176`) and removes one mark (`:177`) per job. Two stand-ins mean two asks,
4000 and two marks for one pass. The cleanup `if !any_marks(player_id)
{ ctx::set_dest(…) }` (`:190-191`) files only the copy that reaches it after
the *last* removal (TODO `:187-189`).

### 2.8 `sparkler-per-instance-counter-cap` — row 342

> 放置2个奇迹水晶（上限2）

`ctx::add_crystals(2, 2)` (`rules/cards/card-mujica/src/sparkler.rs:50`) clamps
that one call; the TODO at `:46-49` states that `add_counter_at`'s `max` is
call-local and no per-instance cap is stored (`ops.rs:2150-2162`). Any external
add can take the count to 3, and `turn_end` then pays a third extra turn.

### 2.9 `umiri-user-key-per-instance` — row 445

> 将此卡放置于在此卡使用者下一名行动的玩家场上

The user is written to the **holder's** slot `umiri_user`
(`rules/cards/card-mujica/src/umiri_card.rs:83`, read at `:32,36,132,170`,
cleared at `:198`), so two copies on one holder overwrite each other. The TODO
at `:78-82` says per-instance state exists (`set_prop`) "but a pre cannot read
props" — and, as R4 notes, `card.counter` cannot distinguish the two copies
either.

### 2.10 `hina-clause-2-swap-missing` — rows 930, 931, 932, 936, 972

> （2）[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。

`rules/cards/card-pp/src/hina_sound.rs` has no marker on the placed substitute
and no surface to offer the swap. The card marks nothing after
`ctx::play_card` (`:71`) and leaves play (`:73`, `Banished`), so no later code
can find the substitute. The offer surface today only enumerates `skill:*` field
instances with an `On::Play` (`crates/game-core/src/engine/mod.rs:1416`); a
non-skill card cannot be pressed.

`[共鸣]` is the derived fuel card `PP:[衍生]共鸣`, spent by
`resonance::try_resonance` (`rules/cards/card-pp/src/resonance.rs:44-75`); the
gate spelling used elsewhere is `ctx::hand_count(player, "PP:[衍生]共鸣") > 0`
(`see_you_tomorrow.rs:51`).

**Where it went.** Both halves are offer-and-skin concerns, not identity:
the substitute marker (`effective_card`) and hina's two marker writes are G05
§6.2/§9.5, and the lifetime edge that keeps the stand-in alive (`created_by`,
`set_created_by_at`) is G08 §4.9. G01 supplies only the plumbing they read
(`trigger::uid()`, the instance uid).

### 2.11 `cs-crystal-cap-read` — rows 301, 1056

> 移动到另一张可以放置奇迹水晶的卡上。

`Slot::add` (`rules/cards/card-mujica/src/crystal_swap.rs:59-62`) passes
`max = 0` (uncapped) and the destination filter (`:158`) offers every card,
including one already at its printed 上限, then overfills it. The TODOs at
`:60-61` and `:154-157` ask for a per-uid cap read and a clamped add.

### 2.12 Shared root causes

1. **Raises drop identity.** `note_counter_changed` discards the uid
   (`wasm_rules.rs:650-659`); no raise carries a raise-id; `card_is` is a name
   compare; `collect_hook_jobs` is per-instance but nothing tells a guard which
   instance the raise is about.
2. **No counter-like change point for marks.** Mark mutations
   (`ops.rs:1921,1959,2002,2093,2112,2124`) are invisible to hooks, so a
   lifetime that follows a count cannot be maintained outside the card's own
   bodies.
3. **No lifetime edge between instances.** `place_card_on` creates a child row
   with no creator; destruction walks only mark/token units.
4. **The cap is call-local.** There is no per-instance maximum store.
5. **The condition reads are name-keyed.** A placed instance cannot be told
   apart from its copy in a pre (`card.*`); both resolve by `(player, id)`.
6. **Guest paths skip the host drive.** `play_card` / `invoke_skill` /
   `invoke_entry` run bodies through `call_entry` in the guest, so anything
   wired into the host drive alone misses them.

---

## 3. Capability

**Invariant (one generalization): every raise names the subject instance or
row, every counter-like store is addressed by uid, every instance read in a
condition answers for the running uid, and every instance may own other rows
for lifetime purposes.**

Concretely:

- A raise carries a monotonically unique `uid` and, where the raise is about
  one instance or row, an `instance` uid. `-1`/`0` mean "none".
- Every card body — host-dispatched or guest-invoked — enters and leaves a
  transient **resolution frame**. A frame is a hook-dispatch source and the
  vocabulary for "am I currently resolving".
- Mark mutations are logged and raised with the mark's `instance` (creator),
  owner, tile, kind and effective signed delta.
- `FieldCard` gains a per-counter maximum (`counter_max`), and adds clamp at
  the stored maximum.
- `CandidateCtx` carries the running uid so `card.*` answers for the instance,
  not the first copy of its name.

**Why per-holder / per-card stores are insufficient.** A holder slot is shared
by two copies of one card on one field (items §2.2, §2.4, §2.5, §2.9) and
survives the card (item §5). A card-id key is not an identity either: `card_is`
admits every copy (§2.2), `card.counter` reads the first copy (§2.9), and a
`place_mark` issued before placement stamps no instance at all (§2.3). Only an
engine-assigned instance/raise id distinguishes the copies that the rulebook
distinguishes (「该卡」 / 「每个」 / 「此卡」).

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
`crates/card-sdk/src/ctx/sys_kani.rs` (one `Op` arm each, §9.2).

```rust
// trigger payload (new; hook reads only)
ruleset_trig_uid() -> i32;              // the raise's unique id; 0 outside a raise
ruleset_trig_instance() -> i32;         // the subject instance/row uid; -1 none

// resolution stack (new)
ruleset_body_enter(ptr, len) -> i32;    // 1 run the body, 0 the fire was cancelled
ruleset_body_leave();                   // close the body scope (the frame
                                        // stays up; its raise drops it)

// per-instance counter maximum (new)
ruleset_counter_max_at(uid, ptr, len) -> i32;      // 0 = uncapped / absent
ruleset_set_counter_max_at(uid, ptr, len, max) -> i32; // 0 clears; returns stored
```

`ptrs` are guest pointers, `len` bytes of UTF-8, the established two-call /
`write_str` pattern (`hostfns.rs:3415-3420`). `uid` of a non-existent instance
reads `-1`/`0` and all writers return `0`.

### 4.2 Guest wrappers — `crates/card-sdk/src/ctx.rs`

```rust
// in `pub mod trigger`
pub fn uid() -> i32;          // raise id
pub fn instance() -> i32;     // subject instance, -1

// module scope (called by `rt::on`; not by card code)
pub fn body_enter(card: &str) -> i32;   // 0 = cancelled fire, skip the body
pub fn body_leave();

pub fn counter_max_at(uid: i32, name: &str) -> i32;
pub fn set_counter_max_at(uid: i32, name: &str, max: i32) -> i32;
```

Parameter semantics:

- `trigger::uid()` — unique per raise, stable for every job the raise
  dispatches. `0` outside a raise, and on a trigger a test built by hand.
  Because the whole action replays from its saved pre-action state, a replayed
  action re-assigns the same ids in the same order, so a claim keyed by uid
  holds (§7).
- `trigger::instance()` — the instance whose counter changed
  (`counterChanged`), the mark's creating instance (`markChanged`), or the
  instance the hook job runs as (`-1` when neither names one). It is **not**
  overwritten per job: copy A must be able to ignore a raise about copy B.
- `body_enter` / `body_leave` — called by the one guest body dispatcher;
  the host owns the frames they bracket (§5.4).
- `counter_max_at` / `set_counter_max_at` — a hard per-instance ceiling for one
  named counter. `0` = uncapped. `add_counter_at` / `add_counter` /
  `add_crystals` clamp to it in addition to their call-local `max`.

### 4.3 Wire enums and kinds — `crates/card-sdk/src/abi.rs`, `kinds.rs`

```rust
// engine trigger ids (104/107 retired; 105–110 taken; the README allocates
// the next free pair to this group, which lands first)
pub const CardFired: Self = Self::engine(111);    // wire "cardFired", hook-only
pub const MarkChanged: Self = Self::engine(112);  // wire "markChanged", hook-only
```

- `Action::CardFired(pre, guard, body)`,
  `Action::MarkChanged(pre, guard, body)` — added to the `action_kinds!` engine
  table (`crates/card-sdk/src/kinds.rs:638`), one `EngineKind` row each in
  `crates/game-kinds/src/lib.rs` with `hook_only: true`,
  `is_filter: false`, plus `from_wire` arms. Both are hook-only so no
  counteraction window opens.
- `Trigger` (core `game_core::engine::rules::Trigger` and the bridge mirror
  `game_rules::world::Trigger:35`) gains `pub uid: i32` and
  `pub instance: i32` (defaults `0`/`-1`); wire accessors `ruleset_trig_uid` /
  `ruleset_trig_instance`.
- `kind_table_pin`, `game-kinds`'s `every_row_round_trips` /
  `retired_ids_are_unnamed` / `filter_and_hook_sets`, and `hostops_smoke` are
  extended with the two names.
- **No `Actions` kind and no `entry::ACTION`.** G05's `On::Play(...).field()`
  plus `entry::PRESS` are the offer vocabulary (R1). `kind_table_pin`'s kind-id
  ceiling stays 112; G08 continues at 113.

### 4.4 Condition vocabulary — `crates/cond`

```cel
resolving('card:id') -> 0 | 1
```

`Scope::Func { arity: 1, cand: false }`, `Fx::StrInt` over a new
`WindowCtx.resolving: Vec<i64>` table (ids, `id_of` encoding; same shape as
`_move_tags`). `fill_window` (`condition.rs:1325`) fills it from the world's
resolution stack; `SnapSrc` gains `fn resolving(&self) -> Vec<String>` with an
empty default so fixture worlds compile. Used by `rui_devil`'s pre (§6.1) —
it must sit in the **window**, not the candidate, because the resolving card is
not the trigger's card.

### 4.5 Candidate identity — `crates/cond/src/ctx.rs`, `crates/game-rules/src/condition.rs`

`CandidateCtx` gains `pub uid: i64` (`-1` = no instance). A new
`fill_candidate_uid(world, owner, card, uid)` fills `card_placed`,
`card_counters` and `card_tile` through that uid (`SnapSrc::counter_names_at` /
`CardWorld::counter_at` / `card_tile_at`); `fill_candidate` keeps its
signature and forwards `uid = -1`. The sites where the candidate **is** the
running card (`host.rs:1590`, `:1882`, `:2042`, `:2098`) pass
`world.self_uid()`; target-probing sites keep `-1` until G07.

### 4.6 Client / view wire

**None.** G01 lands no client field of its own; G05's `SkillAction.instance` is
the carrier once it lands. `effective_card` never reaches the client directly
(G05's effective-card resolution reads it host-side).

### 4.7 ManifestOn changes

None. Both kinds reuse the existing `On::Action` manifest encoding; only the
enum value and wire name are new. `on` entry order and existing ids are
unchanged.

### 4.8 Replaced / extended exports (host side)

| export | state | change |
|---|---|---|
| `ruleset_add_counter_at` | extended | clamps at the instance's stored maximum (`counter_max`) after the call-local `max` |
| `ruleset_place_card` / `ruleset_place_card_on` | extended | split `counter_max.*` props into the per-instance cap store (§5.5) |
| `ruleset_trig_name` | extended | also set on `markChanged` (the mark kind) |
| `ruleset_trig_value` | extended | mark delta |
| `ruleset_trig_tile` / `trig_target` | extended | mark tile / mark owner |

No signature is removed and no existing byte layout changes.

---

## 5. Engine wiring

### 5.1 Raise identity (`uid`)

- `MatchState.trigger_uid: i32` — `#[serde(default)]`, monotone, never reused.
  It is the analogue of `next_mark_uid` (`ops.rs:2099`), including the floor
  at the largest uid in a restored world (`ops.rs:1980-1984` shape).
- The **single** assignment point is `Ruleset::counteract`
  (`wasm_rules.rs:10309`), the one function both `Match::counteract`
  (`play.rs:5918`) and `Run::raise_core` (`wasm_rules.rs:5862-5875`) reach:
  `if t.uid == 0 { t.uid = cx.world_mut().next_trigger_uid(); }`. A trigger
  built directly by a test keeps `0` and reads as "no identity"; a resumed
  half-trigger keeps the value it was born with.
- The bridge copies `t.uid` into the guest `Trigger` next to `t.step` and the
  other scalar fields, and `hostfns::trig_uid` returns it. All jobs of one
  raise share the value because the same `Trigger` is cloned to every drive.

### 5.2 Subject instance (`instance`)

- `counterChanged`: `note_counter_changed` extends `counter_log` to
  `(owner, card, uid, name, change)`; `commit_after` (`:4892`) stamps
  `t.instance = Some(uid)`.
- `markChanged`: the drain stamps the row's `instance` (§5.3).
- `cardFired`: unused (`-1`).
- The field is only ever the **raise's** subject. It is not restamped per hook
  job; `self_uid()` remains the job's instance.

### 5.3 Mark mutation log and `markChanged`

- `World.mark_log: Vec<MarkLog>` — `#[serde(skip)]`, drained at commit.
  Appended by every mutator: `place_mark` / `place_mark_pair` (per row),
  `bump_mark` (delta **actually applied**, `new - old`), `remove_marks` (per
  row, `-count`), `remove_mark_by_uid` (`-count`), and
  `destroy_instance_units` (per removed row).
  A row is `{ uid, tile, kind, instance, owner, delta, card }`; `card` is
  the row's provenance (`TileMark.card`), captured at mutation time because
  the creating instance may be gone by the drain.
- `commit_after` drains after `counter_log`, one raise per row (wire
  `markChanged`, `player_id = owner`, `tile`, `name = kind`, `value = delta`,
  `t.instance = instance`, `t.card` = the row's `card` provenance). A row with
  no creator still raises with `instance = -1`.
- **Dispatch.** `collect_hook_jobs` adds, in addition to the field walk:
  1. one job for `t.instance` when it is `>= 0` (even if already unplaced —
     a destroyed instance's own hook must still resync/discard), using
     `t.card` for the id (a dead instance's card comes from the row's
     provenance, §5.3);
  2. one job per resolution frame whose card declares the kind, with the
     frame's uid.
  All passes are deduped by uid against what this walk already drove, and the
  resolving pass runs last: a frame and a dead subject for the same uid are
  the same job, and a body that is also on the field or the board is driven
  once rather than hearing its own raise twice.
- `remove_from_game` (`play.rs:5107`) becomes `Flow<Vec<usize>>` and drains
  the log after its clears, so a purge's removals are raised even when no
  further run commits. Both call sites (`:4402`, `:4436`) propagate with `?`.
  Its field is cleared *before* `destroy_instance_units` (`:4488-4497`), which
  is why the subject job (§5.3) — not the field walk — is what reaches the
  destroyed card.
- Re-entrancy: a `markChanged` body that writes marks raises again. The chain
  depth bounds it, and the contract requires the listener's own write to be
  idempotent per raise; the claim spell in §6.3/§6.4/§6.7 makes that true.

### 5.4 Resolution stack, body wrapper and `cardFired`

- The stack is a host **thread-local** (`RESOLVING`), not a world field: a
  raise nested inside a body reads it, and every body commit swaps the
  world copy, so no `World`/`Cx` copy could carry it. One host thread per
  match; it dies with the stack. A frame is `(card, uid, owner seat, raise
  uid)`.
- The host owns the frames end to end; the guest wrapper in `card_sdk::rt::on`
  only announces scope (`body_enter` / `body_leave`).
  - `play_card` / `event` stamp the trigger's uid and push the frame before
    the drive; `drive_hook` pushes for the card it is about to drive (seat
    from the call, `-1` for a hand play's not-yet-placed servo).
  - A body returns from its guest call **before** the hooks its money moves
    raised are walked (handler answers replay inline afterwards), so the body
    cannot drop its own frame on return. A frame dies with the raise that
    carried it: `raise_core` (and `counteract_orig`, which writes the same
    lifetime) truncates every frame whose raise uid is the trigger it is
    settling, on success and on a non-suspended error alike.
  - An already-resolving instance keeps one frame; a repeated walk does not
    stack a second.
- Host side of `enter`:
  1. unless the drive **is** a `cardFired` hook job, raise `cardFired`
     (fresh `CoreTrigger::new("cardFired", call.player_id())`, `card`,
     `by_card = owner`, `instance = -1`) through the normal counteract
     choke. Every body announces -- a play, a hook, a counteraction, a skill
     alike; a listener after skills narrows with the `card.is_skill`
     precondition rather than the raise being skill-only. The owner is the
     **frame's seat** -- the seat the drive was dispatched for -- not the
     enclosing raise's `player_id`, because on a payment that is the payer:
     a payee's 「你的技能」 hook would otherwise miss every leg paid *to* it.
     The raise's own cause is stamped there too (`t.by_card`, which the guard
     reads as `by == owner`). A counter may cancel it; a cancelled fire
     returns `0` and the guest skips the body;
  2. if the raise halts (`packed == EXIT_NEED_INPUT`), return the sentinel —
     the body has not started, and the replayed action re-enters;
  3. otherwise return `1`.
- A hook's own body does not announce (step 1's skip): its entry *is* the
  announcement, and two cards watching each other's fires would otherwise
  raise forever. Termination is one level deep: a fire heard as a hook is
  never announced again.
- `card.is_skill` (window-scope Bool, `card_is_skill` flat) is the
  precondition 「技能」: the trigger's `card` is a `skill:` id.
- The fire is itself a visible outcome: its raise posts the firing card's
  chain event (`kind = "hook"` for an in-play body). A body that produced
  nothing else therefore still has that event — the lazy 「效果适用」 header
  (Ruling 2a) stays for a normal entry, and a `QuietWhenEmpty` entry still
  retracts its header around it.
- `leave` is a no-op (debug trace only): the frame deliberately stays up.
- Dispatch reads the stack: the conjoined unit-subject pass and the last
  pass of every walk drive the resolving bodies, resolving frames **last**
  and deduped against what the earlier passes drove, so a body that is also
  on the field or the board hears its own raise once.
- The guard for a cancelled fire is the existing `is_cancelled()` read on the
  raised trigger, exactly as `use_skill` treats a cancelled `cardUsed`
  (`play.rs:1181-1183`).
- One raise per body entry, not per ask pass: the raise is outside the
  learn/commit `loop` (`wasm_rules.rs:3157`), so the commit pass re-runs guest
  writes but does not re-raise.
- `collect_hook_jobs` reads the stack (§5.3); `fill_window` snapshots it
  (§4.4). The stack is mirrored nowhere else.

### 5.5 Counter maximum and per-instance candidate reads

- `FieldCard.counter_max: BTreeMap<String, i32>` — `#[serde(default,
  skip_serializing_if = "…::is_empty")]`.
- Static caps are declared as card props with the reserved prefix
  `counter_max.` (`abi::prop::COUNTER_MAX_PREFIX = "counter_max."`, e.g.
  `("counter_max.crystals", 2)`). In `Run::place_card*` the prefixed keys are
  split out of `props_of` into `counter_max`; all other props still land in
  `props`.
- `World::add_counter_at` clamps, in order: at 0, at the call-local `max` when
  `> 0`, at the stored cap when `> 0` (`ops.rs:2150-2162`).
  `set_counter_max_at` writes the store (`0` removes the key). The Kani arm
  mirrors both.
- `CandidateCtx.uid` / `fill_candidate_uid`: §4.5.

### 5.6 Kani model representation

- All six ops are **stubs** for now (`rules/kani/src/host.rs`, the
  `trig_uid` arm onward), following the existing convention for an op the
  model has not grown: the documented defaults, not a loud failure
  (`trig_uid` 0, `trig_instance` -1, `body_enter` 1, `body_leave` 0,
  `counter_max_at` 0, `set_counter_max_at` 0).
  `hostops_smoke::identity_and_cap_stubs` pins those totals, so replacing a
  stub with a model must keep them.
- Consequence to keep in mind when reading a card proof: a body that reads
  `trigger::uid()` / `trigger::instance()` or the stored cap sees the stub,
  so anything keyed on a specific non-zero id is not exercised symbolically
  yet (in particular `add_counter_at`'s stored-cap clamp).
- What a model must still represent: symbolic `trig_uid` / `trig_instance`,
  a bounded `resolving` in the frame stack, and `counter_max` in the
  per-field counter area. Its invariants: `add_counter_at` never exceeds a
  positive stored cap; a counter set with a cap and then set uncapped is no
  longer clamped; a frame pushed by the host survives a cancelled fire (it
  dies with the raise); `resolving(id)` holds from the push until the raise
  finishes, `body_leave` notwithstanding; `set_dest` applies to `last_uid`.

---

## 6. Card usage

### 6.1 `rui-x-write` (before → after)

```rust
// before (rui_devil.rs:29-54)
for &o in &others {
    ctx::transfer(player_id, o, 100, &reason)?;
    ctx::set_slot(player_id, X, 5);          // unconditional
}
…
ctx::set_slot(player_id, X, 5);              // trailing, unconditional

// after: the transfer legs stay; the writes and the log move into a hook
On::Action(Action::CardFired(
    "resolving('Mor:（Rui）正论恶魔') && card.is_skill && by == owner",
    None, on_skill))
…
fn on_skill(player_id: i32) -> Asked {
    ctx::set_slot(player_id, X, 5);
    ctx::log(player_id, &Msg::new(key!("rui_devil_crit")));
    Ok(())
}
```

`resolving(…)` keeps the fire inside this resolution; `actor == owner` (the
skill's owner is this card's user) says 「你的技能」; `card_is` narrows to the
persona skill. `owner`/`self_uid()` are useless as a scope because rui is never
placed.

### 6.2 `starry-trigger-uid`

```rust
// before (starry_night.rs:117-128)
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && trigger::name() == counter::CRYSTALS
        && (trigger::value() < 0 || ctx::crystals() == 0)

// after
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::instance() == ctx::self_uid()
        && trigger::name() == counter::CRYSTALS
        && (trigger::value() < 0 || ctx::crystals() == 0)
```

`trigger::card_is(ID)` is redundant once the instance matches (the instance's
card is ID) but harmless; it stays for readability.

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
    "tile.id == tile_named('弦卷集团')",
    Some(changed_is_my_crystal), resync))

fn changed_is_my_crystal(_p: i32) -> bool {
    // The subject instance is the stand-in itself; for a destroyed instance
    // `card.placed` is already false, so the guard does not check it.
    trigger::name() == MARK && trigger::instance() == ctx::self_uid()
}
fn resync(_p: i32) -> Asked { sync_circle_instance(trigger::tile()); Ok(()) }
```

```rust
// pass body: one removal per raise, every copy files
fn pass_tile(player_id: i32) -> Asked {
    let t = trigger::tile();
    if ctx::state::get(player_id, CLAIM) != trigger::uid() {
        ctx::state::set(player_id, CLAIM, trigger::uid());
        ctx::bump_mark(t, &MarkFilter::any().kind(MARK).owner(player_id), -1);
    }
    sync_circle_instance(t);
    if own_crystals(t, player_id) == 0 { ctx::set_dest(ctx::Dest::Graveyard); }
    Ok(())
}
```

`CLAIM` is a fixed seat-state key (`"black.claimed"`). `trigger::uid()` is
nonzero inside a raise; the value is a single int overwritten per raise, so
the key never accumulates and both copies read the same latch. The
guard `pass_tile_guard` and its residual `count_marks` check are dropped (the
identity does the work). `sync_circle_instance`'s child dies with the stand-in through G08's
`created_by`, so its own `unplace_at` call is optional once that lands; it
stays in the meantime.

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

### 6.6 `detour-counter-hooks-dead` (demoted to A — landed)

Commit `0bb7e634` added `ctx::linger(player_id, 0)` (`detour.rs:296`) beside
the latch (`:302`). The existing `DiceRollPlan` / `DiceRollAfter` entries
dispatch to the lingering instance and the `slot('detour_next_roll') != 0`
pres still own the gate. Nothing is owed; the lingering entry is cleared at
turn start (`TurnCtx.lingering`), so no cleanup is added. The `state` latches
stay because the linger carries no payload.

### 6.7 `hagumi-multi-standin-abi` (demote to A)

```rust
// before (hagumi_marks.rs:170-191) — every job asks and pays
ctx::gain(player_id, 2000, …)?;
ctx::bump_mark(t, &mark_filter(player_id), -1);
if !any_marks(player_id) { ctx::set_dest(ctx::Dest::Graveyard); }

// after — claim per raise, every copy files itself
if ctx::state::get(player_id, CLAIM) != trigger::uid() {
    ctx::state::set(player_id, CLAIM, trigger::uid());
    if !ctx::ask_yes(…) { return Ok(()); }
    if !ctx::gate(…) { return Ok(()); }
    if trigger::move_remaining() > 0 { ctx::plan::set_stop_at(t); }
    ctx::gain(player_id, 2000, …)?;
    ctx::bump_mark(t, &mark_filter(player_id), -1);
}
if !any_marks(player_id) { ctx::set_dest(ctx::Dest::Graveyard); }
```

The claim is written **before** the ask. When the ask pauses, the action
replays from its saved pre-action state, so the claim is written again on the
replay; within one action the commit pass re-reads the base world and runs it
again, also idempotently. `set_dest` already applies to the drive's
`current_uid` (the acting copy) at `commit_after` (`wasm_rules.rs:4953`), so
no per-uid dest import is needed.

### 6.8/6.11 `sparkler` and `crystal_swap`

```rust
// sparkler: the printed cap is a declared static prop
.props(&[(bandori_std::marker::TURNS_LEFT, 2), ("counter_max.crystals", 2)])
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

`Slot::add` keeps `max = 0`; the stored cap is the clamp. An instance placed
without the prop has no cap and still accepts overflow (unchanged).

### 6.9 `umiri-user-key-per-instance` (demote to A)

```rust
// before (umiri_card.rs:83, 32)
ctx::set_slot(next, USER_KEY, player_id + 1);
On::Action(Action::TurnStart("card.placed && actor == slot('umiri_user') - 1", …))

// after — an on-card counter, read for the running uid
ctx::set_counter(USER_KEY, player_id + 1);
On::Action(Action::TurnStart(
    "card.placed && card.counter('umiri_user') - 1 == actor", …))
```

`card.counter` now resolves through the job's uid (§4.5), so two copies keep
separate users and the value dies with the instance. `turn_start` / `turn_end`
swap `ctx::slot` for `ctx::counter` / `ctx::set_counter`, and the hop re-sets
the counter on the new instance.

## 7. Save / record / replay

| added field | serde | SAVE_VERSION |
|---|---|---|
| `FieldCard.counter_max` | `default`, skip-if-empty | unchanged |
| `MatchState.trigger_uid` | `default` | unchanged |
| `World.mark_log` | `skip` (rebuilt from re-raise; rows are transient) | unchanged |
| `World.resolving` | `skip` (transient stack) | unchanged |

`SAVE_VERSION` stays **12**. Every saved field is `#[serde(default)]`, which
`docs/ENGINE.md:602-603` defines as no bump; the new map/option fields also
skip when empty, so an untouched match's save bytes are unchanged. The
transient logs are safe to drop because a save happens at a routine boundary:
a pending open chain keeps its trigger in `pending`, and the uid of an
already-raised trigger has no saved consumer. `trigger_uid` itself is saved
because it must not repeat after a restore; it is defaulted so pre-v105 saves
load and the allocator floors at any larger existing value.

`RECORD_VERSION` stays **4** (`record.rs:48`). The record is an input log;
G01 changes no input shape. Triggers are transient and never recorded.
Replays ride archived engine bundles, so an older record replays under the
v104 engine that knows nothing of the new kinds; no record migration is owed.

Replay determinism is the reason uid assignment is monotone state, not a
per-process counter: an action replays from its saved pre-action state, so the
allocator is restored with it and hands out the same ids in the same order.
A claim keyed by uid therefore holds across a replay, and a whole action's
effects are redone (not doubled) because nothing of the failed attempt
survives. Checkpoint hashes are A/B'd as usual (`examples/ckpt_equiv.rs`);
`trigger_uid` is derived state and adds no input dependence.

---

## 8. Versioning

`ABI_VERSION` goes 104 → **105** (this group lands first in the README order;
any reordering takes the next free number instead — the README renumber rule).
The reason is additive but incompatible: a v105 ruleset imports
`ruleset_trig_uid`, `ruleset_trig_instance`, `ruleset_body_enter`,
`ruleset_body_leave`, `ruleset_counter_max_at` and
`ruleset_set_counter_max_at`, uses kind ids 111–112, and expects
`add_counter_at`'s stored-cap clamp and `place_card_on`'s `counter_max.*`
split. A v104 host has none of them. No existing import signature or byte layout
changes.

The exact doc line to add immediately below `ABI_VERSION` and above the newest
note (notes are newest-first under the constant):

```rust
/// v105: **per-instance identity and state**. Raises carry a unique `uid`
///    (`ruleset_trig_uid`) and a subject instance (`ruleset_trig_instance`);
///    every card body opens a resolution frame (`ruleset_body_enter` /
///    `ruleset_body_leave`) that the host pins to the body's raise, readable
///    as the `resolving('id')` predicate; `counterChanged` / `markChanged`
///    name their instance;
///    a per-instance counter maximum is stored (`ruleset_counter_max_at` /
///    `ruleset_set_counter_max_at`) and `add_counter_at` clamps at it; a
///    `counter_max.*` placement prop fills that store. New hook kinds
///    `cardFired` (111) and `markChanged` (112); `card.*` condition reads
///    answer for the running uid.
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
| 6 | (landed) `ag.rs::detour_counter_arms_the_next_roll_after_settle` |
| 7 | `hhw.rs::hagumi_one_reward_per_mark_with_two_standins` |
| 8 | `mujica.rs::sparkler_cap_blocks_a_third_external_crystal` |
| 9 | `mujica.rs::umiri_two_copies_keep_their_own_user` |
| 10 | (moved to G05) `pp.rs::hina_swap_marks_the_substitute_and_it_outlives_hina` |
| 11 | `mujica.rs::crystal_swap_skips_a_full_destination` |

Test 1 covers both directions: a fire during the resolution writes X, and a
resolution with no fire (all payees immune or a stripped skill) leaves X
untouched; it also pins the **guest-invoked** path (`ctx::invoke_skill` of
`正论暴击` inside a probe card names the same resolution). Test 3 exercises an
external `bump_mark` and a bankruptcy purge, and asserts the borrowed
`tile:circle` is gone. Test 4 runs the pass under a replay with a prompt open
to pin claim stability.

### 9.2 Kani obligations

`kani_host::Op` gains six append-only ids in the README band 341–346:
`trig_uid = 341`, `trig_instance = 342`, `body_enter = 343`,
`body_leave = 344`, `counter_max_at = 345`, `set_counter_max_at = 346`.
347–349 stay free; G05 (`effective_card_at` / `set_effective_card_at`) and G08
(`set_created_by_at`) take them from their own docs without a renumber.
Every new `Op` also lands in `hostops_smoke.rs`, `sys_kani.rs`,
`native_shims.rs` and the `host.rs` dispatch.

- `proofs/mor.rs::C-MOR-rui-demon-03` — `resolving` + `cardFired` ⇒ exactly
  one X write per fire; no fire ⇒ X unchanged; a cancelled fire ⇒ no write.
- `proofs/mor.rs::C-MOR-starry-night-02` — the guard admits iff
  `trig_instance == last_uid`.
- `proofs/hhw.rs::C-HHW-black-suits-02` — a `markChanged` add/remove drives
  attach/drop, including a removal raised from the purge.
- `proofs/hhw.rs::C-HHW-black-suits-03` — two drives with one raise id produce
  one `BumpMark(-1)` and two `set_dest` rows; a replay of the same raise keeps
  one bump.
- `proofs/crychic.rs::C-CRY-want-human-04` — X written on instance A is not
  readable from B, and disappears after A leaves.
- `proofs/ag.rs::C-AG-detour-02` — a lingering instance hears `DiceRollPlan`
  and the plan base becomes 1d6 (regression).
- `proofs/hhw.rs::C-HHW-hagumi-03` — one mark, two drives, one `Gain`, two
  dest writes.
- `proofs/mujica.rs::C-MUJICA-sparkler-02` — with the cap set, an external
  `add_counter_at` stops at the cap; clearing the cap unclamps.
- `proofs/mujica.rs::C-MUJICA-umiri-02` — two instances keep separate
  `counter(USER_KEY)` values, read through the uid-bound candidate.
- `proofs/mujica.rs::C-MUJICA-crystal-swap-03` — the destination list excludes
  a full instance.

Bounded-stub invariants per new op are in §5.6. `kind_table_pin` pins the two
names and ids.

---

## 10. Alternatives considered

1. **A holder/card-id-keyed dedupe in the engine** ("one payout per
   `(card_id, owner)` per raise"). Rejected: two copies of the same id on one
   field are precisely the failing case (§2.2, §2.4, §2.7), and it cannot
   express "this copy's counter".
2. **Per-card slots with a card-instance suffix key** (the tomori latch
   pattern). Rejected as the general answer: the key needs `self_uid()` in
   every reader/writer, which a **pre** for umiri/black cannot call; and
   item 5's whole defect is that slots survive the card and are shared. Kept
   nowhere: claims use props (§6.3).
3. **Dedupe by `MarkFilter::src`** for the black pass. Rejected: `src` is
   provenance, not a lifetime owner, and the marks are currently stamped with
   no instance at all; `instance` is the owner the purge already uses.
4. **A per-uid `set_dest_at` import** for hagumi/black. Rejected after
   checking `commit_after`: `set_dest` already applies to the drive's
   `current_uid` (`wasm_rules.rs:4953`), and a hook job binds `current_uid` to
   the acting copy, so per-copy filing is a card change.
5. **A boolean `mark_added` flag on `counterChanged`.** Rejected: a mark
   mutation is not necessarily a counter write (pairs, removals by filter,
   destruction) and the payload (tile/owner/instance) is exactly what a
   listener needs.
6. **A `Message`/`send`-based handshake for hina** (now G05's item). Rejected:
   the swap must be *offered* to the player with an enabled/reason state on the
   view.
7. **Storing the counter cap in `props` only** (`props["max.crystals"]`) and
   clamping there. Rejected: `add_counter_at` is engine-side and would have to
   know the prop prefix anyway; a typed `counter_max` map keeps the clamp
   local and the save field skippable.
8. **G01 defines its own `Actions` offer wire (the draft).** Rejected in
   favour of G05's press surface (R1); a second channel would need a parallel
   client act, a reserved entry id and a second `why_not` arm for the same
   cards.
9. **A resolving stack on `Cx` only** (host reads via the bridge). Rejected:
   conditions are evaluated from a `WindowCtx` built over the world, so a
   `Cx`-only stack is invisible to every `pre`.
10. **Raising `cardFired` in `drive_inner_body` only.** Rejected: guest
    `play_card` / `invoke_skill` / `invoke_entry` never reach it (R2).
11. **A skill-only kind (`skillFired`).** Rejected: every card body is a
    firing, and the skill carve-out is a precondition a listener states
    (`card.is_skill`), not a second kind. It also lets a listener hear a
    played card's own body without a second signal.
12. **Restamping `trigger.instance` to the job uid at dispatch.** Rejected:
    it destroys starry's ability to ignore a raise about the other copy.
    `self_uid()` already names the job.

---

## 11. Open questions

1. **Winner among two `want_human` copies.** `tomori_inner_shout` resets *one*
   X. Which instance when two are on the field — first by uid, or a pick? The
   sheet does not say. Design assumes the first (lowest uid) and leaves a pick
   open.
2. **Purge ownership of the black crystal kind.** `purge_markers_owned_by`
   (`play.rs:4527-4547`) deletes by `marker_owner` name across all owners, so
   a bankrupt black player also removes the *other* players' crystals of the
   same kind. Is that intended? Not ruled.
3. **Cancelled `cardFired`.** Design: a counter may cancel it, and a
   cancelled fire skips the body. Confirm that a listener may negate a
   skill's fire, not only rewrite X.
4. **`markChanged` re-entrancy.** A listener that writes marks raises again;
   the chain depth bounds it, and the claim spell makes a per-copy listener
   idempotent. The contract "listener writes must be idempotent per raise" is
   a ruling the doc assumes.
5. **(G05)** `effective_card` scope: the marker does not change the card id, so
   counters, props and `dest` stay the instance's. Confirm that a marked
   substitute should also keep its own `On::Play` for a non-press re-fire.
6. **(G05)** Hina clause (2) frequency — once per turn, once per resolution, or
   unlimited while resonance is held? The sheet gives none.
7. **(G05)** Hina's inner play refusal: if the newly played substitute refuses
   and stays in hand, the host instance is already unplaced and the crystals
   are lost. Re-place and refund, or treat it as legal?
8. **(G05)** `effective_card` on a card that is itself a stand-in for two ids —
   is a chain ever needed? Not ruled; one level assumed.
9. **`trigger::uid()` on synthetic triggers.** A test-built trigger has `0`.
   A claim keyed on `0` matches every synthetic raise in the test. Confirm
   that is acceptable, or reserve `0` as "no identity" and skip claiming.
