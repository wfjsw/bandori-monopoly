# G10 — Effect outcome, cause and settle facts

Status: **design revision** (2026-11-04 rewrite of the 2026-10-27 draft; no
source touched by this document). Worktree
`/home/monopoly/web/.claude/worktrees/g10-design-review`, branch
`worktree-g10-design-review`. Fits **on top of the G09 landing** (ABI 114);
it starts from master's ABI 113 / SAVE 16 / RECORD 4 / highest engine kind 131
(`settleRentMul`) / highest Kani `Op` 442 (`nearest_with_prop`). It does not redo
any landed group.

Group: **G10**. Family: what an effect actually did — the outcome a settle
reached, the instance that caused a raise, the host work a torn card body was
in the middle of, and a card-scoped board checkpoint.

Names are spelled out (G03 R25's rule, G06's header, G07's header): a
**designation** is one admission through the targeting pipeline;
**counteraction** is the [反击] mechanism; a **hook** is a passive listener;
the engine-side carrier of one raise is a **trigger**, its module-side mirror
a **link**. `trigger` is not shortened to `trig` even though the landed
imports use that prefix, and `identifier` is not shortened to `id` in a new
name: the cause field is `by_card_instance`.

Ground truth is `docs/rulebook/cards.json`; every quoted fragment below was
re-read there and is identified by its card id beside it. Row ids are the
triage-sheet rows carried by `G10.json`; they are not `cards.json` indices.
Reading order: `docs/rulebook/spec/abi/G02-pay-pipeline.md` §4-§5 (the settle
frame, the stage machine's outcome and the pay pipeline), `docs/EVENTS.md`
(the event pipeline), `docs/ENGINE.md` §"Drive replay semantics" (HOST-01),
`docs/rulebook/SETTLE-STAGES.md` §7 / §9, and `docs/GUARDS.md` §4.2c (the window vocabulary).

## 0. Review record

This revision came out of a full review of the 2026-10-27 draft. Every cited
site was re-read against master, every claim about the engine re-derived from
the code, and the band arithmetic re-run from the groups that landed before
this one. What the draft got wrong:

| # | Draft defect | Correction |
|---|---|---|
| R1 | The header put the draft in worktree `card-rulebook-review` and based it on ABI 114 with kind 131. Master is ABI 113, kind 131 is already `settleRentMul`, and G09 lands first at 114. | Header, §9 and §11 carry the real base and the landing numbers; G10 is ABI 115 and takes kind **137**, not 131. |
| R2 | Nearly every citation was stale (`play.rs:4943` for the event draw, `:2163-2230` for the settle stages, `:4045-4054` for the balance write, `hostfns.rs:860` for the request shims, `wasm_rules.rs:5654` for `target_player`). The G06/G07 landings moved all of them. | Every site is re-cited against master: `draw_event` 5873-5940, the settle stages 2654-2804 (cancel 2670, body 2729-2741), the balance write 4660, `target_player` 6825-6920, `bridge_trigger` 9159, `inline.rs:215`. |
| R3 | Item 2's council half was designed as work. | G02 landed it: `council_check.rs` already runs `settle_after` on `Action::TileResolved` and `g02_council_cancelled_settle_refunds` pins it. §1.2.2 is now a **landed** note, not an item. |
| R4 | The fields were `by_card_id` / `trig_*` / `set_settle_facts` / `world_snap`. | `by_card_instance` (an instance is a uid, not an id), `trigger_*` imports, `settle_facts` carries a typed record, and `world_snapshot` / `world_restore` / `world_snapshot_drop` say the noun. |
| R5 | `SettleFacts` was a plain struct *field* on the trigger and the bits were derived inline; `cancelled` was `entry.outcome == Cancelled`, which is also true for a replaced body. | The trigger carries `Option<SettleFacts>`; `cancelled` excludes a replaced body (`outcome == Cancelled && !body_replaced`), so the two bits are independent (§5.1). |
| R6 | `SettleFacts` stored `at` / `owner` "copied at birth" from the entry, which made them a second copy of `settle_tile` / `target`. | The facts record is `{ cancelled, body_replaced, paid, paid_value }`; the tile and owner stay G02's `settle_tile` / `target` (§5.1). |
| R7 | `paid` was keyed on a `Pay::settle_tile` payload field that does not exist. | The predicate runs in `money_inner` against the raise's own `settle_tile` (`self.settle_tile()`) and `to`, i.e. exactly the values the `payAfter` / `paid` raises carry (§5.1). |
| R8 | §5.3 keyed the journal by a static answer *slot* and put the answer at that slot, but a nested request inside an in-flight request holds the **same** slot (the outer's own answer is absent), so the second insertion would displace the first. | Slots are dynamic: inserting an own answer shifts every entry's slot at or after the insertion point (and its reapply key) by one (§6.3). The resumed body re-applies the front entry at its own site before going on. |
| R9 | §5.3 let the guest hosts decide a reapply "and continue the drive loop", but the guest store cannot call `apply_host_request` (no bridge handle). | The restore + overlay + apply is done by the **drive** in its `NeedHost` arm; the guest only parks the request and marks its `JournalStart`, and the resumed body re-reaches the same site (§6.3). |
| R10 | §5.3's `Outcome::NeedHost` still pushed the answer, which is right only for a brand-new request; a resumed or recomputed one must insert or replace at its cursor. | `HostState` carries a `JournalStart` (`Push` / `Resume` / `Replace`) and the apply writes the answer at the entry's current slot; append is `Push` (§6.3). `Pending` carries the guest answer log, which the drive otherwise loses on a halt. |
| R11 | §4.7 gave `world_snap` the key `self_uid()`. A hand play has uid **-1** (`wasm_rules.rs:11919`), so every hand play would share one key and the instance-drop cleanup could never see it. | `world_snapshot()` allocates the key and the request answer is its identity, so a hand play needs no uid; `World.world_snapshot_owners` lets an instance drop clean up a placed card's keys (§5.7). |
| R12 | §6.6 made `no_breakup` restore a snapshot taken in its **play body** from a `tileResolved` hook and left the key to a local variable, which the hook cannot see. | The key rides a player slot, and the `settle_facts` reader must be a `tileResolved` entry on the same copy (§7.6). |
| R13 | §5.2's deferral record keyed the duplicate test on `(card, player)` with a *uid* exception but never said what a second copy does to its own crystals; the draft's `arisa_wait` spent crystals after the answer. | The claim is the record's existence for the pair; the losing copy answers 0 and spends nothing, the winner spends 3 (§6.2, §7.1). |
| R14 | §5.4's `draw_event` halts and re-runs by the ordinary replay, but the release path was never given a return value. | `resolve_event` answers `Deferred` / `Resolved`; the release raises `eventResolved` only on `Resolved` (§6.2). |
| R15 | §5.5 stamped `redirected_from` from a `landed` bool computed before the actor/self guards, so a self designation could stamp it. | `landed = live && to != p && to != by_player`, matching the fold's own condition (§6.4). |
| R16 | §9.2's ops were 431-449; G09's block (and `invoke_frame`) came after them. | §9 takes the nine ids after G09's block, **471-479**; the kind is **137**. |
| R17 | §4.1 called the cause field "not a save field" without saying why it needs no default-carrying reader. | `by_card_instance` is transient like `settle_tile` / `outcome`: every reader is a live raise, and a re-run re-raises (§8). |
| R18 | The tests named behaviour but no file. | One new file `rules/tests/tests/g10_effect_outcome.rs` (§10.1), per G06/G07. |
| R19 | The draft had no necessity section, no clause match and no naming section. | §4, §13 and §14 exist. |
| R20 | §5.3's `last` cap of 4 applies per slot, but the recompute can also lower the slot count; a cap hit kept the "last application" without saying which one. | §6.3 keeps the **last completed** application (delete the fresh attempt's writes by restoring the entry's `condition`), so a cap hit is a clean skip, not a half apply. |

## 1. Items

| # | slug(s) | sev | rows | disposition |
|---|---|---|---|---|
| 1 | `event-defer` — merged `arisa-clause-2-event-defer`, `arisa-clause-1-cause-filter` | medium | 567, 568 | B: `EventResolved` kind, `event_defer` / `draw_event_as` imports, `by_card_instance` |
| 2 | `settle-outcome-flags` — `door-settle-body-replace-fact`; `council-check-cancelled-settle` (landed by G02) | medium | 792, 1428, 1449 | B: `settle_facts` payload, `settle_paid_value` |
| 3 | `host01-journal` (`be-strongest-host01-nested-settle-ownership`) | medium | 724, 728 | B, engine-only: no guest import, no wire contribution |
| 4 | `redirect-latch` (`exist-influence-latch-scope`) | medium | 781, 1465 | B: `redirected_from` payload |
| 5 | `world-snapshot` (`nobreakup-revert-half`) | medium | 782, 1077 | B: three imports, saved world state |

Nothing is dropped. Item 2's council half landed with G02; the item stays
because Tomorrow's Door still needs `paid`.

### 1.1 `event-defer` (rows 567, 568)

Book (`PPP:（有咲）等等等一下`):

> （1）任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。

> （2）事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。

Current (`rules/cards/card-ppp/src/arisa_wait.rs`): the card registers only
`On::Play` and `On::Action(Action::EventAfter(…))`. Nothing suspends an event.
`draw_event` (`crates/game-core/src/engine/play.rs:5873`) pops, logs, waits,
raises `event` (the public reveal), binds, runs the body, files the card and
raises `eventAfter` — one synchronous pass with no suspension point and no second
resolution path.

Three further gaps make clause (2) impossible on the current surface, and one
makes clause (1) wrong:

* **No deferral record.** There is no place to park "this event card, for this
  drawer, due at this owner's next turn start". The trigger snapshot is
  transient and the deck is a plain `Vec<String>`; the record must outlive
  both a prompt and a save.
* **Cause is a seat, never an instance.** `Trigger.by_card` is `Option<i32>`
  and means a **seat** (`crates/game-core/src/engine/rules.rs:393`). Clause
  (1)'s 「非因为此卡导致」 is about *this copy*: two copies on one field each
  exclude only their own release. There is no instance cause, so today every
  `eventAfter` banks a crystal and the TODO at `arisa_wait.rs:64-66` says so.
* **No attribution for the extra draw.** The extra event is 「视为其他玩家在
  “流星堂”抽取」. `HostRequest::DrawEvent { player_id }` carries a player
  only, and `draw_event` leaves `tile` at `-1`, so the station is not on the
  trigger.

The station also matters to a different card: the persona skill
`rules/skills/skill-personas/src/arisa.rs:27` hooks
`On::Action(Action::Event("actor != owner && tile.id == tile_named('流星堂')", …))`
for 「每次有其他玩家在“流星堂”抽取事件时获得一个[火罐]」. With `tile = -1`
on every `event` raise that hook can never match, and bonsai's own clause (2)
does not draw an event at all (it calls `ctx::draw`, a **hand** draw).
`draw_event_as(player, tile)` is the primitive for "an event drawn as X at
tile Y"; the release uses it too.

### 1.2 `settle-outcome-flags` (rows 792, 1428, 1449)

#### 1.2.1 Tomorrow's Door (row 792)

Book (`PPP:Tomorrow's Door`):

> （3）此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。

Current (`rules/cards/card-ppp/src/tomorrows_door.rs`): the tax has two halves
— a `paySplitAdd` add to the rent payment (`pay_add`) and a `settleBody`
entry for the shapes no rent payment carries (`settle_body`, guarded by
`trigger::cancelled()`). The `settleBody` half is wrong in the two directions
the card's own TODOs name:

* `trigger::cancelled()` in the settleBody hook is the **settleBody link's**
  cancellation, i.e. "someone else replaced the body" — so a body replacer
  (`rules/cards/card-general/src/parking_space.rs`) that runs *before* Door's
  hook drops the tax, although the settle still resolves and the book still
  charges it.
* Whether rent ran is guessed from `mortgaged_of(at)`; a body-replaced settle
  on a mortgaged tile therefore charges or not by hook order rather than by
  whether rent was actually paid.

The settle-body stage cannot see a fact decided *after* it, and `settleAfter`
cannot see whether rent was paid. The card needs the settle's own outcome
bits. G02's `settle_outcome()` answers only `Settled` vs not and cannot say
"body replaced" — it is `Cancelled` for a replaced body too
(`play.rs:2741-2744`).

#### 1.2.2 Student council check (rows 1428, 1449) — **landed by G02**

Book (`R:学生会的检查`):

> 若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金。

G02 moved the cleanup onto `Action::TileResolved("card.placed", None,
settle_after)` (`council_check.rs:33`), which fires on the cancelled terminal
too, and `g02_council_cancelled_settle_refunds` pins it. G10 carries no
council surface; §7.3 only records it as a consumer of the new facts, and
`g10_council_uses_the_outcome_fact` keeps the two halves together.

### 1.3 `host01-journal` (rows 724, 728)

Book (`RAS:成为最强`):

> roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到“Live House”），若你没有Livehouse格子，传送到“Live House”。视为你的主要移动。

Current (`rules/cards/card-ras/src/be_strongest.rs:49-54`): the ownership read
(`owned_tiles(...).any(is_live_house_for)`) runs before the teleport. When the
teleport's settle buys a deed through the agent-tile offer, the master
(commit) re-run's read is not guaranteed to see it: a write inside a host
routine's own nested settle "may not be on live until that routine finishes
prompting" (`docs/ENGINE.md`, same section).

The exact mechanism, traced from `crates/game-rules/src/inline.rs:215` and
`crates/game-core/src/engine/mod.rs:1698`:

1. The body calls `run_planned_move`; the drive's `NeedHost` arm applies the
   `Move` and appends its answer. The move's settle reaches a prompt inside
   the nested settle.
2. `PlayCtx::ask` halts (`HaltKind::Ask`). `execute` stores
   `Pending { snapshot, answers, live }` with the **pre-routine** snapshot. The
   drive's local answer log is lost; only the engine's `Answered` log
   survives.
3. The prompt is answered. `complete_prompt` **pushes** its `Answered` and
   re-executes from the snapshot.
4. The body re-runs from the top. At the `Move` call the guest shim
   (`inline::request_or_pause`) finds a value at its cursor — the nested
   prompt's answer — returns it **without re-applying the move**, and advances.
   The settle (and the deed) never runs again; the player keeps a bogus answer
   and the ownership read stays stale.

Nothing card-side can fix this; it is engine plumbing. **No guest export**, no
contribution to the wire bump.

### 1.4 `redirect-latch` (rows 781, 1465)

Book (`RAS:EXIST`):

> 若在此期间此卡没有造成影响，抽1张卡

Current (`rules/cards/card-ras/src/exist.rs:65-72`): the latch is
`On::Action(Action::Effect("target == owner && actor != owner", …))`. But
`effect` is raised for **every** money movement too (`play.rs:4136-4140`) and
for any direct targeting, so:

* a plain rent payment to the owner — `target == owner`, `actor != owner` —
  marks the card as having had an influence;
* a bank print to the owner — same;
* an opponent's effect that directly names the owner, never redirected — same.

Only a redirect that **landed** is an influence. `redirect` is a Filter on the
declaration path (`wasm_rules.rs:6825-6920`, the fold at `:6855-6878`); a
Filter body may not write an instance prop (`refuse_if_filter`), and the
effect payload has no "redirected" flag, so the `effect` hook cannot tell
whether the rename happened. The provisional `state::add` inside the Filter
now also over-counts, because a probe pass and a counter can run the fold
several times (`exist.rs:68`).

### 1.5 `world-snapshot` (rows 782, 1077)

Book (`Sumimi:Sumimi不会解散哦`):

> 若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果

Current (`rules/cards/card-sumimi/src/no_breakup.rs`): the teleport settles
and consumes the main move, but nothing rolls the world back.
`ctx::turn_snapshot` exists, but it is captured at the player's **turn
start**, so it is stale for a play made during the turn: it would revert every
move and every settle since the turn began, not just this card's. What is
needed is a play-time, card-scoped checkpoint of every player's `pos` /
`state` / `money`, and the board's `owners` / `houses` / `mortgaged`.

The revert point is `tileResolved` (`docs/rulebook/SETTLE-STAGES.md` R3), the terminal that
also fires on a cancelled settle.

## 2. Shared root causes

1. **A settle is a stage machine but its outcome is not state.** Branch points
   (`play.rs:2654-2804`) read the return of each raise and throw the
   information away; later stages and terminal hooks see only the tile. Every
   consumer re-derives the outcome from proxies (mortgage bit, slot arms, hook
   order) and gets it wrong in a different way.
2. **Cause is carried as a seat, never as an instance.** `by_card` answers
   "whose card", which cannot separate two copies of one card. The same shape
   recurs in the event clauses: the card must exclude *itself* (arisa) and
   attribute a nested draw to itself (the extra event).
3. **The multi-pass drive assumes host routines apply atomically.** A routine
   that prompts is torn: its effect is partly on live, its answer slot is not,
   and the resumed pass's slot walk misreads the nested prompt answer as the
   routine's. Only the engine can restore the atomicity, and it must do so
   per request, not per body.
4. **Facts a hook needs are decided inside other subsystems.** "Did a redirect
   land" is decided by `target_player`'s fold; "did rent commit" by the money
   pipeline. A hook cannot observe either.

## 3. Capability as an invariant

**I-1 (deferral).** A deferred event card is neither lost nor duplicated:
exactly one record exists per event, it resolves exactly once — at the
deferring instance's owner's next turn start, or at the first turn start after
that owner leaves the game — and its resolution names the deferring **instance**
as the cause. The extra draw is an event draw attributed to a seat that is not
the owner and to the named tile.

**I-2 (settle outcome).** Every settle exposes its own outcome to every hook
from the body stage on: `cancelled` iff the `settle` link was negated (no body
could run), `body_replaced` iff the `settleBody` link was negated, `paid` / `paid_value` iff a rent payment to the
settle's owner for the settle's own tile actually committed. `cancelled` and
`body_replaced` are mutually exclusive. The bits are stable once the body
stage ends and are born with the settle frame, not with the tile.

**I-3 (journal).** For every effectful host request, the world the
resumed/landing pass reads is exactly the world that request produced: never a
partial application, never a duplicate. The request's own answer occupies its
own answer cursor, and a re-application removes the previous answer rather
than stacking on it.

**I-4 (redirect).** `redirected_from` is present on the `targeted`, `effect`
and `target` triggers of a designation iff a `redirect` hook moved the hit
from the declared seat to a different, live, non-acting seat — and absent on
every money-raised `effect`.

**I-5 (checkpoint).** `world_restore(key)` leaves the six covered families
(`pos`, keyed `state`, `money`, `owners`, `houses`, `mortgaged`) exactly as
`world_snapshot` stored them; it is idempotent; a missing key answers 0 and
changes nothing.

All five are generic mechanisms: no card names, no display text, no player
decision inside an import.

## 4. Names and naming decisions

| concept | name | why not the draft's |
|---|---|---|
| the instance that caused a raise | `by_card_instance` / `trigger_by_card_instance` / `trigger::by_card_instance()` | `by_card_id`: an instance is a uid, and `id` abbreviates |
| the settle's outcome record | `SettleFacts` / `settle_facts` / `trigger_settle_facts` | `trig_*` abbreviated `trigger`; G02 §12 already names the field `settle_facts` |
| the checkpoint primitives | `world_snapshot` / `world_restore` / `world_snapshot_drop` / `World.world_snapshots` | `world_snap` clipped the noun |
| the redirect's origin | `redirected_from` / `trigger_redirected_from` / `trigger::redirected_from()` | it reads as the source, and `redirected()` is derived |

`SettleFacts`' bit constants live in `card_sdk::abi::settle_fact`
(`CANCELLED = 1`, `BODY_REPLACED = 2`, `PAID = 4`) and are mirrored in
`game_core::state::settle_fact` like every other engine constant. The import
takes the bitset so one host call answers all three reads.

G02 §12's one line naming `trig_settle_facts` is updated at landing; the
numbering split it records (G02 owns the frame and `settle_outcome()`, G10
extends the frame) is followed unchanged.

## 5. Surface

### 5.1 `Trigger` — new fields

`crates/game-core/src/engine/rules.rs:367-460` and the guest mirror
`crates/game-rules/src/world.rs:35-145` both gain, in the same order:

```rust
/// The **instance** whose card caused this trigger, or `None`. `by_card` is
/// the seat; this is the identity that separates two copies of one card.
/// Set on the event raise, its after points and the deferral release.
/// Transient.
pub by_card_instance: Option<i32>,
/// The seat a `redirect` hook moved this designation from, on `targeted` /
/// `effect` / `target`, and only when the rename landed (I-4). Transient.
pub redirected_from: Option<i32>,
/// The settle this trigger belongs to, from `settleBody` on. `None` outside
/// a settle. `settle` itself stays G02's dotted `settle.tile` namespace,
/// which reads this trigger's `settle_tile`.
pub settle_facts: Option<SettleFacts>,
```

```rust
/// What a settle has done so far, born with its frame. G02's
/// `SettleEntry.outcome` distinguishes "no body" from "body replaced" only by
/// accident (`Cancelled` covers both), so the two are split here: `cancelled`
/// excludes `body_replaced`, and a hook can tell which happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SettleFacts {
    /// The settle ended before any body: the `settle` link was negated.
    pub cancelled: bool,
    /// The `settleBody` link was negated — the body was replaced.
    pub body_replaced: bool,
    /// A rent payment to this settle's owner, on this settle's tile,
    /// committed.
    pub paid: bool,
    /// What that payment moved (0 without `paid`).
    pub paid_value: i32,
}
```

Both structs get all three fields; `bridge_trigger` (`wasm_rules.rs:9159`)
copies them, or every guest reader sees the default. `SettleFacts` is `Copy`
and lives on the trigger, not in the world: it is re-derived from the frame
on every pass, like `settle_tile` and `outcome`. The engine writes it at the
raise sites only (§6.1) — no card body can set it.

### 5.2 Imports, one line each

Registered in `crates/game-rules/src/hostfns.rs` beside their neighbours;
externs in `crates/card-sdk/src/ctx/sys.rs`.

| import | signature | answers |
|---|---|---|
| `event_defer` | `() -> i32` | 1 = the event is deferred by this instance (the record exists now); 0 = not an `event` trigger, or another instance already deferred it |
| `draw_event_as` | `(player_id: i32, tile: i32) -> i32` | 1 (the whole event pipeline ran) |
| `trigger_by_card_instance` | `() -> i32` | the cause instance, or -1 |
| `trigger_settle_facts` | `() -> i32` | bitset `1 cancelled`, `2 body_replaced`, `4 paid`; 0 outside a settle hook |
| `trigger_settle_paid_value` | `() -> i32` | the committed rent amount, else 0 |
| `trigger_redirected_from` | `() -> i32` | the seat the rename moved from, else -1 |
| `world_snapshot` | `() -> i32` | the new key; -1 when no world (never, in practice) |
| `world_restore` | `(key: i32) -> i32` | 1 restored; 0 no snapshot for the key |
| `world_snapshot_drop` | `(key: i32) -> i32` | 1 dropped; 0 none |

The value reads are direct `hostfns` functions over `c.st()/st_mut()` and
`c.st().wr().trigger()`. `EventDefer`, `DrawEventAs`, `WorldSnapshot`,
`WorldRestore` and `WorldSnapshotDrop` are **host requests** (§5.5): a direct
host write goes to the caller's run copy, which a later prompt in the same
body drops, while a request is applied against live and replay-skipped. The
bridge's apply site supplies the `uid` / `card` / seat, so no `Run` signature
change is needed.

### 5.3 Sugar (`crates/card-sdk/src/ctx.rs`) — translates before the ABI

```rust
pub fn event_defer() -> Result<bool, Prompt>;              // asked(1/0) != 0
pub fn draw_event_as(player_id: i32, tile: i32) -> Result<(), Prompt>;
pub fn world_snapshot() -> Result<i32, Prompt>;
pub fn world_restore(key: i32) -> Result<bool, Prompt>;
pub fn world_snapshot_drop(key: i32) -> Result<(), Prompt>;
pub mod trigger {
    pub fn by_card_instance() -> Option<i32>;              // (v >= 0).then_some
    pub fn settle_cancelled() -> bool;                     // bit 1
    pub fn settle_body_replaced() -> bool;                 // bit 2
    pub fn settle_paid() -> bool;                          // bit 4
    pub fn settle_paid_value() -> i32;
    pub fn redirected_from() -> Option<i32>;
    pub fn redirected() -> bool;                           // from.is_some()
}
```

### 5.4 Kinds and entries

* `TriggerKind::EventResolved = Self::engine(137)` — **the first id after
  G09's block** (master ends at 131 `SettleRentMul`; G09 reserves 132-136, so 137 is
  reserved here and pinned at landing, §9).
* `ActionKind::EventResolved = Self::engine(137)`, an `action_kinds!` row
  after `EventAfter` (`crates/card-sdk/src/kinds.rs:685`), and an
  `EngineKind { id: 137, wire: "eventResolved", is_filter: false,
  hook_only: true }` row in `crates/game-kinds/src/lib.rs:718`.
* `EventResolved` is a plain field-walk action kind. The dispatch stamps
  `tile = rec.tile`, `card = Some(rec.deferrer_card)`,
  `by_card = Some(rec.by_card)`, `by_card_instance = Some(rec.deferrer_uid)`
  and `instance = rec.deferrer_uid`. The owning copy's residual guard is
  `trigger::by_card_instance() == Some(self_uid())` (there is no
  `trigger.instance` CEL field and no own-instance dispatch branch is needed:
  a hand copy is not `card.placed`).
* `EventResolved` is hook-only, never a counteraction window.

### 5.5 Host requests (`crates/game-rules/src/host.rs:389`)

```rust
DrawEventAs { player_id: i32, tile: i32 }   // draw_event_as
// The event facts, read off the live trigger by the shim (the store has the
// mirror); the bridge fills the uid / card / seat.
EventDefer { event: String, player_id: i32, tile: i32 }
WorldSnapshot                                // allocates and stores a key
WorldRestore { key: i32 }
WorldSnapshotDrop { key: i32 }
```

`HostRequest::DrawEvent` keeps its arity (`{ player_id }`) for already-built
modules; the plain `ctx::draw_event` maps to `DrawEventAs { tile: -1 }` and
`apply_host_request` resolves `-1` to the drawer's current position.

`HostRequest` and `Call` are not serializable and live in `game-rules`, while
the journal is saved state in `game-core` (§6.3). The journal therefore stores
a **`JournalRequest`** mirror in `game-core`:

```rust
// `PartialEq` (no `Eq`) for the recompute comparison; `MoveCtx` carries `f64`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum JournalRequest {
    Pay { from: i32, to: i32, amount: i32, src: Option<Msg>, total_stage: bool },
    PayTotal { from: i32, to: i32, amount: i32, src: Option<Msg> },
    Move { player_id: i32, plan: MoveCtx },
    SettleAt { player_id: i32, tile: i32, main: bool },
    Buy { player_id: i32, tile: i32, kind: i32 },
    Acquire { player_id: i32, from: i32, tile: i32, price: i32 },
    Teleport { player_id: i32, tile: i32 },
    TeleportAs { player_id: i32, tile: i32, resolve: bool, as_main: bool },
    Target { player_id: i32, tile: i32, single: bool },
    Gate { player_id: i32, kind: i32 },
    Draw { player_id: i32, n: i32 },
    DrawEvent { player_id: i32 },
    DrawEventAs { player_id: i32, tile: i32 },
    EventDefer { event: String, player_id: i32, tile: i32 },
    WorldSnapshot,
    WorldRestore { key: i32 },
    WorldSnapshotDrop { key: i32 },
    // …the remaining existing variants (OfferBuild, Build, Mortgage,
    // ApplyMortgage, ApplyRedeem, Agent, ChargeRent, PayRent, Linger, …),
    // …G09's `TileWrite` / `BuildAs` (it lands first; mirror them here).
}
```

**Every** `HostRequest` variant is journaled: the enum exists only for work
that touches the world, and any such work can be followed by a prompt in a
nested drive. `impl From<&HostRequest> for JournalRequest` and the reverse
live in `game-rules`; the replay site rehydrates and calls
`apply_host_request`.

### 5.6 Wire-breaking table

| change | class |
|---|---|
| 9 new imports (§5.2) | new wire |
| `Trigger` + `SettleFacts` fields (§5.1) | new guest-visible payload, read through the new imports |
| `TriggerKind` / `ActionKind` 137, spelling `eventResolved` | new wire value (manifest trigger spelling) |
| 5 new `HostRequest` variants | new request variants; reach a save only through their `JournalRequest` mirror |
| renumbered ids | none |
| `Outcome::NeedHost` / `Outcome::NeedInput` carry the guest log | host-internal |
| changed semantics of an existing id | none |

`draw_event_as` and `event_defer` can prompt indirectly (an event body may
ask), exactly like `draw_event`. No import contains a player decision.

### 5.7 World state (`world.rs`, `pending`)

```rust
// World, beside `event_deck`
#[serde(default)]
pub deferred_events: Vec<DeferredEvent>,
#[serde(default)]
pub world_snapshots: std::collections::BTreeMap<i32, WorldSnapshot>,
#[serde(default)]
pub world_snapshot_owners: std::collections::BTreeMap<i32, i32>, // uid -> keys
#[serde(default)]
pub next_world_snapshot_key: i32,

// Pending (the save's torn-routine slot), all `#[serde(default)]`
pub host_journal: Vec<HostJournalEntry>,
pub guest_answers: Vec<i32>,

// PlayCtx, transient, a copy of the pending values while a pass runs
pub host_journal: Vec<HostJournalEntry>,
pub guest_answers: Vec<i32>,
pub journal_resume: bool,
// HostState, transient per drive: one GuestLog (folded into `answers` /
// `next_answer`'s old fields, §6.3.5)
pub log: GuestLog,
// GuestLog
pub answers: Vec<i32>,
pub cursor: usize,
pub journal: Vec<HostJournalEntry>,
pub reapply: std::collections::BTreeMap<usize, usize>,  // cursor -> outstanding count
pub last: std::collections::BTreeMap<usize, HostJournalEntry>,
pub replace_count: std::collections::BTreeMap<usize, u8>,  // cap per slot (§6.3.5)
```

The journal is not world state: it exists only while a routine is torn, and a
save is only taken at a prompt, so it rides `Pending` (which `Saved` already
serializes) rather than `World`. `GuestOverlay`
(`crates/game-core/src/engine/cx.rs:674`) gains
`#[derive(Clone, Serialize, Deserialize)]` and crate visibility for its
fields; they are already save-shaped `StateVar`s.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeferredEvent {
    /// The event card taken out of the deck, unresolved.
    pub event: String,
    /// The drawer whose event this was; the release resolves for it.
    pub player: i32,
    /// The tile the draw stood on; the release resolves there.
    pub tile: i32,
    /// The deferring instance's owner; only that seat's turn start is due.
    pub due_owner: i32,
    /// The deferring instance: release cause and `eventResolved` subject.
    pub deferrer_uid: i32,
    pub deferrer_card: String,
    /// `by_card` on the release, kept beside the record rather than re-read.
    pub by_card: i32,
    /// Turn key when deferred; the release requires a strictly later one.
    pub after_turn_key: i32,
}
```

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub players: Vec<PlayerSnapshot>,   // pos / state / money, per seat
    pub owners: Vec<i32>,
    pub houses: Vec<i32>,
    pub mortgaged: Vec<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnapshot { pub pos: i32, pub state: Vec<(String, i32)>, pub money: i32 }
```

`DeferredEvent` / `HostJournalEntry` / `WorldSnapshot` live in
`crates/game-core/src/engine/world.rs`; all three are `Serialize,
Deserialize`. They sit inside `World` / `Pending`. `World` derives
`Debug, Clone, PartialEq` (`world.rs:836`), so `DeferredEvent` / `WorldSnapshot`
/ `PlayerSnapshot` must derive the same or the container stops compiling;
`Pending` derives `Debug, Clone, Serialize, Deserialize` (`mod.rs:310`), so
`HostJournalEntry` needs `Debug` and `Clone` (it also derives `PartialEq` for
the tests). `PlayerSnapshot` is `Copy` as well.

## 6. Engine wiring

### 6.1 Stage reporting (`play.rs`)

G02 already replaced the `settle_open` counter with `PlayCtx.settle_stack:
Vec<SettleEntry>`, pushed once per frame and popped on completion, and gave
the entry its terminal `outcome`. This group does not re-fix the counter; it
extends the entry with the facts the body stage cannot see:

* `SettleEntry` gains `body_replaced: bool`, `paid: bool`, `paid_value: i32`.
* `SettleStage::RaiseSettleBody` (`play.rs:2729`): on `si.is_cancelled()`
  (`:2741`), besides the existing `outcome = Cancelled`, set
  `settle_stack[frame.stack_idx].body_replaced = true`. `RaiseSettle`'s cancel
  branch (`:2670`) does **not** set it.
* In `money_inner`, between the balance write (`play.rs:4662`) and the
  `payAfter` raise (`:4695`), with the innermost entry (none open → skip):
  `if p.rent && loss > 0 && self.settle_tile() == entry.tile as i32
  && to == Some(entry.player as usize)` → `entry.paid = true; entry.paid_value =
  entry.paid_value.saturating_add(loss);` (saturating: one settle can carry
  several rent legs). A payment cancelled or reduced to 0 never reaches here.
  The test uses the raise's `settle_tile` (`:4706`) and `to`, not the
  payment's own `tile` — a redirect or a force-buy can make them differ.
* `RaiseSettleBody` and `RaiseSettleAfter` / `RaiseTileResolved` (`:2729`,
  `:2767`, `:2795`): the facts are built from the entry **before** the raise
  and passed as a field (`settle_facts = Some(SettleFacts { cancelled:
  entry.outcome == Cancelled && !entry.body_replaced, body_replaced:
  entry.body_replaced, paid: entry.paid, paid_value: entry.paid_value })`)
  exactly like `outcome`, because the hooks run inside the raise and a write
  to the returned trigger after it is too late. At `settleBody` the fields are
  still false (the link's own answer is what the raise asks); at
  `settleAfter` they are final.
* `tileResolved` keeps stamping `outcome` for G02's `settle_outcome()`
  readers; the facts are the finer, additive record.

### 6.2 Deferral

`event_defer` is a request. Its shim reads the live trigger's mirror
(`st.wr().trigger()`) and answers 0 without a request when
`kind != "event"`; otherwise it sends `EventDefer { event: trigger.card,
player_id: trigger.player_id, tile: trigger.tile }`. At the apply:

* If a record for `(event, player_id)` exists: `deferrer_uid == uid` answers
  **1** (the re-run of this same call) without pushing; any other uid answers
  **0**. The record's existence is the claim, so two copies both at ≥3
  crystals produce one record and one spend.
* Otherwise push `DeferredEvent { event, player, tile, due_owner: seat,
  deferrer_uid: uid, deferrer_card: card, by_card: seat, after_turn_key:
  turn_key() }` onto `world.deferred_events` and answer 1.

The record is the "deferred" signal — no trigger flag, which would be lost
when the hook drive halts and re-runs with the call skipped.

`draw_event` and the release share one routine:

```rust
enum EventResolution { Deferred, Resolved }
fn resolve_event(&mut self, card: String, player: usize, tile: i32,
                 by: Option<i32>, cause: Option<i32>) -> Flow<EventResolution>
```

1. `raise!(event, player, card = Some(card), tile = tile, by_card = by,
   by_card_instance = cause)`.
2. If a record now exists for `(card, player)` — a fresh deferral, including
   the one that suspended this very call — answer `Deferred`: no bind, no
   body, no filing, no `eventAfter`. The record is the whole signal, so the
   re-run after a prompt finds it again and stays deferred.
3. Otherwise run the existing resolution half verbatim: the `is_cancelled`
   branch (no bind, the activation flash marked negated), or `bind_event` +
   the body (including its expire/file paths); then file when neither the
   body nor the cancel did, raise `eventAfter` with the same `card` / `tile` /
   `by_card` / `by_card_instance`, and answer `Resolved`.

`draw_event` pops, logs, waits, retains off `eventTop`, then calls
`resolve_event(id, i, pos(i), by, cause)` (which raises `event`); on `Deferred`
it **returns without the refill** — the card is parked, not filed, so the
refill waits for the release. On `Resolved` it does the existing
eventAfter/refill/wait tail.

The release (`turn_start`, `play.rs:793`, immediately after the `turnStart`
raise and before the `out(i)` return) drains records where `due_owner == i &&
turn_key() > after_turn_key`, plus records whose `due_owner` is out (any seat,
the first turn start after the leave). For each: re-check `out(i)`; remove the
record; `resolve_event(rec.event, rec.player as usize, rec.tile,
Some(rec.by_card), Some(rec.deferrer_uid))`; only on `Resolved`, and only with a still-live
deferrer, raise `eventResolved` as §5.4 says; then `refill_event_deck()` and
`wait(0.4)` for the release itself. A re-deferred release raises no
`eventResolved`; its extra draw follows the later resolution.

`draw_event_as` is the public form: `DrawEventAs { player_id, tile }` applies
`draw_event_at(player, tile)` where `-1` means the drawer's current position,
and answers 1 after the pipeline (`Deferred` or `Resolved`).

### 6.3 HOST-01 journal

`HostJournalEntry`:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostJournalEntry {
    /// The answer cursor this request owns. Dynamic: every own-answer
    /// insertion shifts the deeper entries by one (§6.3.4).
    pub slot: usize,
    /// The request payload, replayed verbatim on resume.
    pub request: JournalRequest,
    pub card: String,
    pub uid: i32,
    /// The seat the request was issued for.
    pub player: i32,
    /// Exact world before the request.
    pub condition: Box<World>,
    /// The card's turn-policy writes (`build_discount`, `lingering`, ...) at
    /// the call, which the blind re-apply cannot read back from the body.
    pub linger: std::collections::BTreeMap<String, i32>,
    /// The guest state the body had already cleared when it called (the same
    /// `GuestOverlay` the live `NeedHost` boundary pushes).
    pub guest: GuestOverlay,
}
```

The request decides how the apply relates to the journal (it is set by the
guards of §6.3.2 and read by `apply_host_request`):

```rust
pub enum JournalStart {
    /// A brand-new request: push a new entry at the current cursor.
    Push,
    /// An in-flight entry is being re-applied in place.
    Resume(usize),
    /// A completed entry whose arguments the body recomputed: replace it.
    Replace(usize),
}
```

The answer slot is **not** carried on this value. A nested insertion during
the apply shifts the entry's own slot, so the apply reads the entry's slot at
the moment it succeeds (`Replace` uses the same slot it replaced) instead of a
slot captured before the call.

#### 6.3.1 Request entry

`apply_host_request` takes, besides the request, a `JournalStart` (the drive
harvested it from `HostState.host_journal_start`), `uid: i32`, and `seat: i32`
in place of `call` (every `call` use inside it and in `pay_through` is
`call.player_id()`; the live caller passes the run's uid and that seat, a
replay passes the entry's `uid` / `player`). At the **entry
of every request** (they are all
effectful by construction; plain reads are separate imports):

* `Push`: push `HostJournalEntry { slot: st.next_answer, request:
  (&req).into(), card: card.to_string(), uid, player: seat, linger:
  linger.clone(), condition: Box::new(cx.world_copy()), guest:
  cx.overlay_guest_state() }` onto `cx.host_journal` (the capture now returns
  the overlay; the live boundary applies the returned value instead of a
  second call). `Resume(i)` updates entry `i` in place with the same fields;
  `Replace(slot)` pushes a new entry the same way and leaves `last[slot]` in
  place until the new answer overwrites it (a halt before then must still find
  a `last` row).
* on `Ok(v)`, insert `v` at the entry's **current** slot in the guest answer
  log, call `st.w().after_host(entry.slot)`, advance the cursor past the answer
  (`Push` / `Resume` / `Replace` alike — the answer is the one this call now
  returns), move the entry into `cx`-side `last[slot]`, and shift every later
  slot (§6.3.4). `Replace` overwrites `answers[slot]` instead and does not
  shift.
* on `Err` other than a suspension, leave the entry in flight; on `Err(Ask)`
  also seed its slot into the next pass's `reapply`.

The simulation path (`DriveInline::apply`, `wasm_rules.rs:3170`) shares the
push and the start: it passes `run.current_uid`, `self.call.player_id()` and
the harvested `JournalStart`, so a provider
that declines a prompt inside a request leaves the same journal behind and the
halt/replay path resumes it identically.

#### 6.3.2 The guest pause point

`request_or_pause` (`crates/game-rules/src/inline.rs:215`) becomes, in this
order (for brevity the sketch keeps the existing `st.answers` /
`st.next_answer`; both are fields of `st.log`, §6.3.5):

```text
st = c.st_mut(); at = st.next_answer;
1. if st.reapply.get_mut(&at) is Some(n) with *n > 0:  // one outstanding
       //   application at this cursor; see below
       // an in-flight request is re-entered at its cursor. Several in-flight
       // entries can share a cursor (an enclosing request's own answer is not
       // written yet); the reapply mark is a count, and the first entry still
       // at this cursor is the outermost, so take that one.
       decrement the count at `at` (remove the key at 0)
       index = st.host_journal.iter().position(|e| e.slot == at)
       entry = st.host_journal[index].clone()
       st.world = Some((*entry.condition).clone())     // see 6.3.3
       entry.guest.apply(st.w_mut())
       if entry.request != (&req).into() { replace entry.request; drop the
          entries after it and their reapply marks (their writes are undone by
          the restore) }
       st.host_request = Some(req); st.host_journal_start = Resume(index);
       return pause
2. if st.answers.get(at) is Some(x):
       if st.last.get(&at) is Some(e):
           if e.request == (&req).into() {
               st.w().after_host(at); advance; return Ok(x) }
           // the body recomputed its arguments from the post-request world
           if the cap at `at` is reached (4, §6.3.5):
               st.world = Some((*e.condition).clone()); e.guest.apply(st.w_mut())
               st.w().after_host(at); advance; return Ok(x)   // clean skip
           st.world = Some((*e.condition).clone()); e.guest.apply(st.w_mut())
           bump st.replace_count[at]
           st.host_request = Some(req); st.host_journal_start = Replace(at);
           return pause
       st.w().after_host(at)
       advance; return Ok(x)          // a dead nested answer
3. st.host_request = Some(req); st.host_journal_start = Push;
   return pause
```

`HostState` gains `host_journal_start: Option<JournalStart>`, written by the
guards above, harvested with the rest of the log at the halt or return, and
read (and cleared) by the drive before it calls `apply_host_request`. `finish` hands
back the store's `answers` on both halt shapes (`NeedInput` / `NeedHost`),
because the nested answers a torn request produced live only in the store
(`finish`, `host.rs:2455-2480`).

`GuestOverlay::apply(&mut W)` is the stored-overlay twin of
`PlayCtx::overlay_guest_state(&World)`: it writes the recorded decreases
without recomputing them (the same `state_set` path, so the `NEVER_NEGATIVE`
floor still applies), for the restore path only. `PlayCtx::overlay_guest_state`
becomes "capture into a `GuestOverlay`, then `apply` it".

The entry's restore/overlay happens **before** the pause, because the drive's
`NeedHost` arm mutates live and must start from the request's pre-state. The
arm's existing `adopt_turn_policy` / `overlay_guest_state` /
`overlay_guest_plan_tags` calls stay (its own overlay is the current pass's
equivalent of the entry's); the arm then applies and the apply itself writes
the answer at the entry's current slot (§6.3.1). The guest store's own
`answers` is the same log, so the drive's `answers` is re-taken from the store
on return instead of being written separately.

#### 6.3.3 Resume

The store is dropped when `finish` returns, so the log must cross on the halt
itself: `Outcome::NeedHost` and `Outcome::NeedInput` gain the store's
`GuestLog` (their `world` payload stays). On `Err(HaltKind::Ask)` from inside
a request, or a prompt halt, the drive:

* takes the returned log, moves its `journal` into `Pending.host_journal` and
  its `answers` into `Pending.guest_answers`, and seeds every in-flight
  entry's slot into `reapply`;
* lets the halt propagate unchanged.

`complete_prompt` passes both pending fields to `execute`, which installs them
on a fresh `PlayCtx` (`start` passes empty) and sets `journal_resume`. The
first drive start consumes the flag:

1. `answers = cx.guest_answers.take()`, `journal = cx.host_journal.take()`,
   `reapply` seeded with the count of in-flight entries per slot.
2. Run the body from the top. The prefix is ordinary replay: prompts before
   the first torn request consume their logged answers (step 2 of §6.3.2).
3. At the first request site that carries a reapply mark, step 1 of §6.3.2
   takes the outermost in-flight entry, restores its `condition` and overlay
   into the pass's world, and pauses with `Resume(index)`. The drive's
   `NeedHost` arm applies it against live as for any other request.

No special drive-start application is needed: the entry is re-applied exactly
where the body reached it, and its answer lands at its own slot, so the call
returns the value the live request produced. Reads before the request site see
the pre-request world, which is correct — the request has not been applied in
this pass yet.

This is what gives `be_strongest` its corrected ownership read: the body's
first pass runs **after** the move was re-applied against live. When the
recomputed destination differs, the later `Replace` branch re-applies from
the entry's `condition` and the first application is gone.

A save between the halt and the resume is exactly a save inside `Pending`;
both fields are `#[serde(default)]` so an older state loads empty.

#### 6.3.4 Slots are dynamic

`shift_slots(from)` bumps `slot` on every entry with `slot >= from`, the same
keys in `reapply` (a cursor -> count map, because an enclosing and a nested
entry can share a cursor), and the same keys in `last` and `replace_count`.
It runs on every `Push` / `Resume` own-answer insertion, at that answer's
slot; `Replace` writes over an existing slot and
does not shift. Any entry still in flight at or after the insertion
point must move by one: the live cursor is at `at + 1` after the insert, and a
later request that completes must land where the cursor is, not where it was.
This covers both a deeper nested request and an **enclosing** one whose own
answer is not yet written (the enclosing entry's slot equals the first nested
site's cursor while it is in flight). Without the shift, the second of two
requests nested at the same cursor would overwrite the first's answer (review
R8) and the enclosing answer would land before its own nested answers.

#### 6.3.5 Termination

The journal holds **only in-flight entries**: a completed request pops its
entry, so outside a torn stack it is empty. A per-slot cap of 4 `Replace`
applications bounds a pathological recompute; the fourth hit is checked at
the Replace branch before pausing, restores the completed `last[slot]`
condition (dropping the pass's writes back to it), and consumes the existing
answer — a clean skip, not a half apply.

`last` and `replace_count` survive only the drive's passes and are cleared
when the drive returns.
A frame abort (`Err` other than `Ask` / suspension) drops the entries it
pushed.

A recompute can leave *stale* nested answers after the replaced one when the
fresh request asks fewer prompts than the old one. They are dead-but-harmless:
the body's next prompts consume them in order, and the same-order assumption
already underlies the whole replay. A body whose path diverges *before* a
stale answer consumes it as the wrong prompt's answer; the recompute case that
matters (`be_strongest`) diverges at the request itself, before any nested
prompt.

`condition` stores a whole `World`; cloning is refcount-cheap for the event
tail and bounded by the effectful-request count in one body.

The per-call guest log now travels as one value, because every part must
move together: `GuestLog { answers: Vec<i32>, cursor: usize,
journal: Vec<HostJournalEntry>, reapply: BTreeMap<usize, usize>, last:
BTreeMap<usize, HostJournalEntry>, replace_count: BTreeMap<usize, u8> }`.
`HostState` holds one instead of the bare `answers` / `next_answer`;
`store()` / `HostState::new` take the log.

Two copies of the log exist for one pass: the `PlayCtx` fields and the live
store. The guest guards can reach only the store, and `apply_host_request`
only the `PlayCtx`, so they are synchronized at the store boundary exactly
like `next_answer` today — `store()` seeds `HostState.log` from the `PlayCtx`
fields, and `finish` (all three outcomes) and the halt arm replace the
`PlayCtx` fields with the log's before the drive reads them. A torn pass is
therefore consistent: the guest's own edits are harvested before the drive
applies anything.

Nested `play_card` moves the log into the nested store and adopts it back at
the same places `next_answer` moves today (`hostfns.rs:3261-3296`), and
`Admitted::Yes { next_answer }` becomes the log's advanced cursor.

### 6.4 Redirect fact (`wasm_rules.rs:6825-6920`)

In `target_player`, the fold reassigns `let mut p = p` in place, so the
declared seat must be copied **before** the loop (`let declared = p`). Inside
the loop the fold already requires `to != p && to != by_player && live`; on
each accepted mutation set `landed = Some(declared);` (idempotent — a
second redirect still reports the declared seat, not the intermediate one).
Pass `landed` into `target_window` (`:6896`), which stamps `t.redirected_from`
on `targeted`, `effect` and `target`; a self pick (`p == by_player`) leaves it
`None` because the fold's own condition excludes it. The money-raised `effect`
(`play.rs:4136-4140`) leaves it `None`; `target_tile` has no redirect fold and
passes `None`.

A redirect link whose mutation applied but which a counter then cancelled is
"landed" in behaviour (the rename stands); the fact mirrors the rename, not
the link's negation. Recorded as Q3.

### 6.5 World checkpoint (`world.rs`, `play.rs`)

* `world_snapshot()`: `key = world.next_world_snapshot_key; ++` (the counter
  starts at `1`, so `0` means "no snapshot" to a caller); store
  `WorldSnapshot` of `players[*].pos`, `players[*].state` (key/value pairs, the
  projection `TurnSnapshot` uses), `players[*].money`, `st.owners`,
  `st.houses`, `st.mortgaged`; record `world_snapshot_owners[uid]` for cleanup;
  answer the key.
* `world_restore(key)`: write them back, absolute (idempotent). Positions are
  written directly, not via `teleport_as`, so no abnormal gate and no move
  bookkeeping (a revert is not a move). `turn_start_pos` / `turn_snapshot` are
  untouched.
* `world_snapshot_drop(key)`: remove the snapshot and the owner row.
* All three are requests (§5.5), so they land on live and are not lost when
  the calling hook prompts; a restore is a plain value write, never a move.
* `destroy_instance_units` (`ops.rs:2380`) additionally drops every key under
  `world_snapshot_owners[uid]` (a stale key otherwise leaks the world clone
  into every later save). Hand plays have uid `-1` and no cleanup; the card
  drops its key itself (§7.6).

The restore at `tileResolved` runs while the frame is still on the work stack;
restoring `pos` does **not** retarget the settle (the frame's `at` is fixed,
`docs/rulebook/SETTLE-STAGES.md` §7). The main-move latch is not restored (Q6).

### 6.6 Kani model (`crates/card-sdk/src/kani_host.rs`, `ctx/sys_kani.rs`,
`rules/kani/src/host.rs`)

Nine new `Op` rows, 471–479 (§9; the enum currently ends at
`nearest_with_prop = 442` and G09 reserves 451–470); models:

* `event_defer` → 1 when the modelled trigger kind is `event` and the modelled
  instance has not claimed it, else 0; sets the claim.
* `draw_event_as` → 1; appends a modelled draw at `tile` for `player`.
* `trigger_by_card_instance` → the model's cause instance;
  `trigger_redirected_from` → its redirect source; `trigger_settle_facts` →
  its bitset; `trigger_settle_paid_value` → its value.
* `world_snapshot` / `world_restore` / `world_snapshot_drop` → a `BTreeMap`
  in the model with the same allocate / idempotent restore / drop semantics.

No op may answer a silent 0 when unmodeled (house rule).

## 7. Card usage

### 7.1 `arisa_wait`

Before: only `On::Play` + `EventAfter` banking unconditionally.

After:

```rust
On::Play("", None, arisa_wait),
// （2）「事件触发时…」 — placement and count are the applicability.
On::Action(Action::Event("card.placed && counter('crystals') >= 3", None, defer)),
// （1）「任何非因为此卡导致的事件结算时…」
On::Action(Action::EventAfter("card.placed", Some(foreign), event_after)),
// release callback（「此后额外抽取…」）
On::Action(Action::EventResolved("card.placed", Some(resolved_mine), resolved)),
```

`defer`:

```rust
let ok = ctx::event_defer()?;
if !ok { return Ok(()); }          // another copy claimed this event
ctx::set_crystals(ctx::crystals() - 3);
ctx::log(player_id, &Msg::new(key!("arisa_wait_deferred")));
```

`resolved_mine`:

```rust
fn resolved_mine(_: i32) -> bool {
    // Only the copy whose deferral this is hears the callback.
    ctx::trigger::by_card_instance() == Some(ctx::self_uid())
}
```

`foreign`:

```rust
fn foreign(_: i32) -> bool {
    // 规则书（1）: 「任何非因为此卡导致的事件结算时」 — `None` is a board draw.
    ctx::trigger::by_card_instance() != Some(ctx::self_uid())
}
```

`resolved` picks the next live seat after the owner (Q1) and calls
`ctx::draw_event_as(other, ctx::tile_named("流星堂"))?`.

The two TODOs at `arisa_wait.rs:64-66` and `:53-54` are deleted: the cause
reader makes both claims true.

### 7.2 `tomorrows_door`

Before: `settleBody` entry with `trigger::cancelled()` and the mortgage
heuristic.

After: drop the `settleBody` entry; add

```rust
On::Action(Action::SettleAfter(
    "card.placed && actor != owner && tile.id >= 0 && (settle.owner == owner || settle.tile == tile_named('梦开始的地方'))",
    Some(tax_due_guard), settle_after))
```

with the guard `in_play_area() || prop(PROP_STEP) >= ROUTE.len()` (the
existing residual), `!player_out(payer)`, `!ctx::trigger::settle_paid()`,
`tax_due() > 0`; the body is the same `transfer`. `pay_add` keeps the rent case (it composes with `paySplitMul`), and
`settle_paid` makes the two halves mutually exclusive: a committed rent
payment (which `pay_add` already taxed) is `paid`, so the settleAfter entry
skips; a dream tile, a mortgaged owner tile, a replaced body, or a cancelled
rent is not `paid`, so it charges. The guard no longer reads `cancelled()` and
no longer guesses from `mortgaged_of`. The payer's `settle_paid_value()` is
the log figure.

A `cancelled` settle (the `settle` link negated) never reaches `settleAfter`
(G02 jumps to `tileResolved`), so a fully-negated settle charges nothing.
Recorded as Q8.

### 7.3 `council_check` — landed

No change. Its `tileResolved` handler already reads
`trigger::settle_outcome()`; `g10_council_uses_the_outcome_fact` just keeps it
in the G10 test file so the two consumers of a cancelled settle stay
co-pinned.

### 7.4 `be_strongest`

No change to the body: the engine re-applies the torn `Move` when the body
re-reaches the same call, so the ownership read at `:52` already sees the deed
the nested settle bought. The read answers the livehouse face, the fresh
`run_planned_move` carries a different plan, and the journal re-applies the
move from `condition`, so the player ends on the destination the corrected
table names. Delete the
`TODO(ABI) HOST-01` comment there.

### 7.5 `exist`

Before: `Action::Effect("target == owner && actor != owner", …)` with the
`state::add` latch inside the Filter.

After:

```rust
On::Action(Action::Effect(
    "target == owner && actor != owner", Some(landed_redirect), mark_used))
```

```rust
fn landed_redirect(_: i32) -> bool { ctx::trigger::redirected() }
```

The latch write moves out of `redirect` into `mark_used` (`state::add` on the
same per-instance key, unchanged), so it runs once per settled declaration and
no longer on probe passes. A rent or a direct effect to the owner has no
`redirected_from` and no longer latches.

### 7.6 `no_breakup`

`const SLOT_SHOT: &str = "nb_shot";`

At the play's start, before the ask:

```rust
let key = ctx::world_snapshot()?;
ctx::set_slot(player_id, SLOT_SHOT, key);
```

Add two `tileResolved` entries on the same copy -- the failure and the
success terminal -- both keyed to this settle's own tile:

```rust
On::Action(Action::TileResolved(
    "slot('nb_shot') > 0 && tile.id >= 0 && settle.tile == tile.id",
    Some(failed_shot), revert)),
On::Action(Action::TileResolved(
    "slot('nb_shot') > 0 && tile.id >= 0 && settle.tile == tile.id",
    Some(kept_shot), settle_after)),
```

with

```rust
fn failed_shot(player_id: i32) -> bool {
    !ctx::player_out(player_id)
        && !repeated_digits(ctx::money_of(player_id))
}
fn kept_shot(player_id: i32) -> bool { !failed_shot(player_id) }   // the same predicate, inverted

fn take_key(player_id: i32) -> i32 {
    let key = ctx::slot(player_id, SLOT_SHOT);
    ctx::set_slot(player_id, SLOT_SHOT, 0);            // 0 = no shot armed
    key
}
fn revert(player_id: i32) -> card_sdk::Asked {
    let key = take_key(player_id);
    ctx::world_restore(key)?;
    ctx::world_snapshot_drop(key)?;
    ctx::log(player_id, &Msg::new(key!("no_breakup_reverted")));
    Ok(())
}
fn settle_after(player_id: i32) -> card_sdk::Asked {
    ctx::world_snapshot_drop(take_key(player_id))?;
    Ok(())
}
```

The key rides a slot because the `tileResolved` hook is a different body from
the play body (review R12); the request answer keeps it stable across a
re-run. The slot starts at `0` (its absent read) and `world_snapshot` keys
start at `1`, so `> 0` is the armed test. Both entries clear the slot, so only
one runs and a successful card does not leak a world clone.
`destroy_instance_units` covers the placed case.

## 8. Save / record / replay

* **SAVE_VERSION 16 → 17** (the next bump after G09's). `World.deferred_events`,
  `World.world_snapshots`, `World.world_snapshot_owners` and
  `World.next_world_snapshot_key` (all outlive a pass),
  `Pending.host_journal` and `Pending.guest_answers` (a save is taken
  mid-prompt, with a torn routine) are all `#[serde(default)]`. Old saves are
  refused by `restore_saved`'s exact-version check (`mod.rs:534`) anyway; the
  defaults keep hand-edited or migrated state loading.
* **RECORD_VERSION stays 4.** `Answered` is unchanged; the new fields ride
  world/pending state. A record replays through its archived engine bundle, so
  an older engine never sees a v115 key.
* `SettleFacts`, `by_card_instance`, `redirected_from`, `last`, `reapply` and
  `replace_count` are transient: all are re-derived on a re-run, exactly like
  `settle_tile` / `outcome` / `settle_stack`. The journal is transient in the
  same sense (it exists only while a routine is torn and is cleared when it
  completes) but is saved on `Pending` because a resume needs it.
* A save inside a settle re-runs the routine; the settle work stack
  (`ContinueSettleWork`) is not saved today either, so nothing new is lost.
* Replay determinism: the deferral queue and the snapshot map are world
  state, so a replayed turn start releases the same record in the same order.
  The journal is rebuilt by the replay's own passes; a re-applied request
  recomputes its answer from the same restored world and the same live state,
  and the nested prompt answers it re-consumes come from the engine's own log.

## 9. Versioning and band arithmetic

Base: master ABI **113**, SAVE **16**, RECORD **4**, highest kind **131**,
highest `Op` **442**. G09 lands first at ABI **114** and takes the band
README.md now reserves for it: kinds **132–136**, ops **451–470**. G10:

* ABI **114 → 115** — the import table, the trigger payload and the manifest
  kind spelling all change; the host refuses a different version, so an
  additive change is still a bump.
* SAVE **16 → 17** (§8).
* kind **137** — the exact id is pinned at landing in
  `kind_table_pin.rs:168-181` and `game-kinds/src/lib.rs:710-718`.
* Kani `Op` **471–479**, after G09's reservation.
* RECORD unchanged.

Entry inserted immediately below the constant, above the newest note (newest
first); the v104 turn-snapshot entry stays below:

```rust
pub const ABI_VERSION: i32 = 115;   // was 114 (eleventh group)
/// v115: **outcome, cause and world facts** (G10).
///    `event_defer` suspends an `event` link; the record releases at the
///    deferring instance's owner's next turn start (new hook
///    `TriggerKind::EventResolved`, its residual guard comparing
///    `trigger::by_card_instance()` with the copy's own uid) and
///    `draw_event_as(player, tile)` attributes an
///    event draw to another seat and tile (`HostRequest::DrawEventAs`).
///    `Trigger.by_card_instance` / `trigger_by_card_instance` add the causing
///    **instance** beside `by_card`'s seat. `Trigger.settle_facts` /
///    `trigger_settle_facts` (bitset) / `trigger_settle_paid_value` publish a
///    settle's `cancelled` / `body_replaced` / `paid` outcome from the body
///    stage on. `Trigger.redirected_from` / `trigger_redirected_from` name
///    the seat a landed redirect moved a hit from. `world_snapshot` /
///    `world_restore` / `world_snapshot_drop` checkpoint and restore
///    pos/state/money/owners/houses/mortgaged. No id renumbered.
///    SAVE_VERSION 16 → 17 (deferred events, host journal, snapshots);
///    RECORD_VERSION unchanged.
```

**Merge caveat:** G06 already appended the `subject_card` / `join` /
`answers_declared` payload and the guest mirror, and G07 the designation
window. The `Trigger` change is a field-by-field addition on top of G06's
shape; do not paste a whole struct over it. The entry blocks are disjoint and
only the number moves in the ordinary case.

## 10. Tests and Kani obligations

### 10.1 Per item — one new file `rules/tests/tests/g10_effect_outcome.rs`

| item | test | assertion |
|---|---|---|
| 1 | `g10_arisa_defers_at_three_crystals` | ≥3 crystals + an event draw → the event is not resolved, 3 crystals are gone, one record exists |
| 1 | `g10_deferred_event_resolves_at_the_owner_turn_start` | at the owner's next `turnStart` the body ran and the card is filed |
| 1 | `g10_deferred_release_banks_no_crystal` | the card's own release adds 0 crystals (`by_card_instance`) |
| 1 | `g10_foreign_event_banks_one_crystal` | any non-self `eventAfter` adds exactly 1 |
| 1 | `g10_two_copies_defer_once` | both at ≥3 → one record, 3 crystals spent total, the loser spends nothing |
| 1 | `g10_deferred_extra_draw_is_an_event_at_the_station` | after the release an `event` is drawn for another seat with `tile == 流星堂`; bonsai gains a fire pot |
| 1 | `g10_deferred_record_survives_save_restore` | save mid-deferral, restore, the turn start releases |
| 2 | `g10_settle_facts_carry_cancelled_to_tile_resolved` | counter-cancelled settle → `tileResolved` reads cancelled, no `settleAfter` |
| 2 | `g10_settle_facts_split_cancelled_and_replaced` | a replaced body reads `body_replaced` and not `cancelled` |
| 2 | `g10_settle_facts_carry_paid_and_value` | rent settle → `paid` true, `paid_value` is the moved figure |
| 2 | `g10_door_tax_runs_after_a_body_replace` | Parking Space replaced the body → the tax still transfers |
| 2 | `g10_door_tax_skips_after_a_paid_rent` | ordinary rent → exactly one surcharge |
| 2 | `g10_door_tax_on_the_start_tile` | settle on 梦开始的地方 → tax due, no rent |
| 2 | `g10_council_uses_the_outcome_fact` | cancelled forced settle → card discarded, placement cost refunded (co-pin with G02) |
| 3 | `g10_ownership_read_sees_the_deed_bought_by_the_move` | agent-tile offer buys a livehouse → the commit pass's read sees it and the face-mapped destination wins |
| 3 | `g10_journal_survives_nested_prompt` | one nested prompt → the world ends as if the move ran once; the deed and settle exist once |
| 3 | `g10_journal_answers_do_not_shift` | a body prompt after the torn request gets its own answer |
| 3 | `g10_recomputed_request_reapplies_from_pre` | the second pass computes a different destination → the move re-applies from `condition`; the first application is gone |
| 3 | `g10_unchanged_request_is_not_double_applied` | the second pass computes the same request → one move, one settle, one payment |
| 3 | `g10_journal_three_deep` | a request inside a played card inside a request → in-flight slots shift, all three re-apply in order |
| 4 | `g10_exist_latch_needs_a_landed_redirect` | an opponent's single-target effect redirected → latched, no draw at turn start |
| 4 | `g10_exist_latch_ignores_a_plain_rent_payment` | rent to the owner → not latched, draws |
| 4 | `g10_exist_latch_ignores_a_direct_target` | an effect already naming the owner → not latched |
| 4 | `g10_exist_latch_follows_a_negated_redirect` (Q3) | redirect renamed, then counteracted → the rename stands, latched |
| 5 | `g10_no_breakup_reverts_a_failed_teleport` | a digit-less amount → pos/money/owners restored, card discarded |
| 5 | `g10_no_breakup_keeps_a_successful_one` | repeated digits → no restore |
| 5 | `g10_world_restore_is_idempotent` | two restores of one key equal one |
| 5 | `g10_world_snapshot_survives_a_prompt` | snapshot, prompt, save/restore, restore still lands the pre-state |

Regression/adjacent: `effect_log_cards::be_strongest_*`,
`door_starts_on_ryuseido_and_walks_the_sequence`,
`ix_door_surcharge_on_rent`, `ppp::*` arisa tests, the settle-stage suite,
`zz_review_door_storm`,
`effect_applied_log::reread_after_host_effect_sees_the_updated_value`.

### 10.2 Kani obligations

`rules/kani/src/proofs/{ppp,ras,sumimi,hostops}.rs` and `spec/`:

| proof | obligation |
|---|---|
| `g10_settle_facts_split` | the cancel branch ⇒ `cancelled && !body_replaced && !paid`; the replace branch ⇒ `body_replaced && !cancelled`; paid ⇒ the body ran |
| `g10_settle_paid_only_own_tile` | a rent payment to another tile or another payee leaves `paid` false |
| `g10_redirect_fact_iff` | `redirected_from.is_some()` ⇔ the fold changed a live non-actor target, over a symbolic `to` |
| `g10_event_cause_propagates` | a cause stamped on `event` is on its `eventAfter` |
| `g10_defer_is_once` | a second instance's `event_defer` on the same pair answers 0 and pushes no record; the same instance's re-run answers 1 and pushes none |
| `g10_release_due_iff` | a record releases at a turn start ⇔ due owner and strictly later key, or owner out |
| `g10_snapshot_restore_iff` | `restore(snapshot(w))` equals `w` on the six families; restore twice equals once; a missing key answers 0 |
| `g10_journal_answer_layout` | after a replay every in-flight entry's answer sits at its own shifted slot and no nested answer is orphaned |
| `g10_journal_apply_once` | an equal re-encounter applies zero more times; a changed one restores `condition` before applying |

Each new op also gets a `hostops_smoke` presence proof so an unmodeled op
cannot answer silently.

## 11. Alternatives

1. **Deferral as a guest-held record.** Rejected: the record must outlive the
   card (a save, a leave), so it belongs in the world.
2. **Event deferral as a new `On::Schedule` entry.** `On::Schedule` already
   has an id and a due-turn mode. Rejected: a schedule is card-owned future
   work with the card as the owner; an event is an event-deck unit with a
   drawer and a tile, and the release must re-enter the event pipeline
   (`bind_event`, filing, `eventAfter`), which no schedule entry does.
3. **Settle outcome as one `settleOutcome` hook instead of facts.** Rejected:
   both consumers also need the tile/actor payload `settleAfter` already
   gives, and a hook alone cannot serve a Filter or a guard inside
   `settleAfter`.
4. **`paid` from a `payAfter` hook's slot.** Rejected: a hook is a card body
   with its own store; the fact must exist before any card's guard runs, and
   the money pipeline is the only place that knows the commitment.
5. **HOST-01 as "rebase the snapshot only".** Rejected: the torn request's
   *own* answer slot is still wrong, so the guest skips the request on the
   first re-run; the world diverges permanently.
6. **HOST-01 as "roll back only".** Rejected: the request's effect is lost
   entirely.
7. **HOST-01 in the guest store** (let `request_or_pause` call the bridge).
   The store has no bridge handle; it would need a third raw-pointer host and
   would run `PlayCtx` code under a guest sandbox borrow. The drive already
   owns both, so the apply stays there.
8. **Redirect latch as a prop write from the `Redirect` Filter.** Would need
   `refuse_if_filter` lifted. Rejected in G06-adjacent work: Filter bodies are
   pure by contract, and the latch wants the settled declaration.
9. **World snapshot as a second `turn_snapshot` variant.** Rejected: the
   turn-start arrays are stale mid-turn, and a second capture point on the
   player collides with `ran_as_usual`, which already restores from them.
10. **`by_card_instance` as a `String` card id.** Readable but not identity:
    two copies share the id. Rejected for the uid; the id rides as the
    dispatch key only.

## 12. Open questions

1. **「其他玩家」 for the extra draw.** The book does not name the seat. V1
   takes the next live seat after the owner; alternatives are the next seat in
   turn order at release time (the same thing) or a chosen seat.
2. **Crystal timing on a deferred event.** Clause (1) banks at resolution, so
   a deferred event banks only when it resolves, and never for the card's own
   releases. Should a *negated* release still bank? Today's hook banks on
   `eventAfter` unconditionally; an event-level `cancelled` fact would answer
   precisely (out of this surface).
3. **Redirect link cancelled but its rename stood.** `target_player` reads
   `r.target` regardless of `r.is_cancelled()`, so a counter on the redirect
   link cannot undo the rename. The fact mirrors the rename (influence
   happened). Is that the ruling, or should a cancelled redirect restore the
   declared seat?
4. **Deferral when the arisa card leaves.** V1 resolves the event anyway and
   skips the callback (no extra draw). Alternatives: resolve immediately on
   leave, or drop the event back to the discard.
5. **Deferral when the owner leaves.** V1 releases at the first turn start
   after the leave, without the callback. Confirm.
6. **The main move after a revert.** Confirm 「视为你的主要移动」 stays
   consumed after 「取消所有受到的效果」.
7. **`no_breakup` and cards played during the settle.** The six-family
   snapshot leaves hand/deck/marks/field untouched. Is 「取消所有受到的效果」
   confined to the six families, or must a card that got played during the
   failed settle come back too?
8. **`settle_paid` when rent is replaced rather than paid.** `paid` follows
   the money, not the clause; confirm the Door tax still applies when the rent
   link was negated but the settle ran.
9. **Journal depth.** The replay covers nested requests; is a three-deep torn
   stack in scope for the first implementation, or may it assert-and-fall-back
   to today's behaviour?
10. **Does `world_snapshot` need a card-chosen key at all?** The allocating
    form keeps the card free of identity; a card wanting two checkpoints holds
    two answers. Confirm no caller wants an externally named key.
11. **The release's tile.** The record resolves at the tile the original draw
    stood on, where the event's body reads `trigger.tile`; the drawer may have
    moved since. The alternative — the drawer's position at release — moves
    the event's locality to the wrong turn's board. Confirm.
12. **Re-applying a recomputed request.** §6.3 replaces an application when
    the body's fresh payload differs, which is what lets `be_strongest`
    correct its destination after the nested settle. That undoes and re-runs
    the first settle. Confirm the post-settle read is the book's intent, and
    that replace (not a second, cumulative application) is the right shape.

## 13. Necessity

### 13.1 The group

The group is one hole with five faces: the engine has no way to say what an
effect *did* (outcome, cause) or what a torn body was *doing*, and no
card-scoped board memory. Item 2's `paid` and item 3's journal also sit on the
same HostRequest boundary: a request is the unit both the settle facts and the
tear/replay have to agree on.

Striking an item is possible and spelled below, but two pairs are genuinely
coupled: item 1 needs both `by_card_instance` and `draw_event_as` (a cause
without a re-draw cannot satisfy clause (2)); item 3 is the machinery that
makes `draw_event_as` and every other nested request safe when they prompt.

### 13.2 Per item

* **1 (defer).** Today the card is inert for both clauses. The smallest
  alternative — a guest record — cannot survive a save or a leave (A1).
  Necessity: one world vector, one kind, two imports; nothing card-side can
  park an event.
* **2 (settle facts).** Today the tax is wrong in both directions its own
  TODOs name, and no hook can tell a replaced body from a cancelled one:
  G02's `outcome` is `Cancelled` for both. Necessity: three bits on the
  already-open frame; A3/A4 each lose a consumer.
* **3 (journal).** Today a nested prompt through a request loses the request's
  effect and corrupts the answer cursor. Necessity: `Pending` fields and one
  restore/apply at a resume; A5/A6 each leave half the bug.
* **4 (redirect fact).** Today every money movement and every direct effect
  latches the card, so 「没有造成影响」 is almost never true. Necessity: one
  transient field stamped where the rename is decided; the Filter cannot write
  a prop (A8).
* **5 (checkpoint).** Today nothing reverts. Necessity: three requests and one
  world map; the turn-start snapshot is stale mid-turn (A9).

### 13.3 Surface, field by field

| surface | forced by | if removed |
|---|---|---|
| `by_card_instance` | 1 (「非因为此卡」) | every `eventAfter` banks, including the card's own release |
| `draw_event_as` | 1 (「视为其他玩家在流星堂」) | the extra draw has no seat and no tile; the arisa skill stays dead |
| `event_defer` | 1 | no suspension point |
| `EventResolved` | 1 (「此后」) | the callback has no point after the resolution |
| `settle_facts` / `settle_paid_value` | 2 | the tax keeps guessing; `cancelled` conflates replaced |
| `redirected_from` | 4 | the latch keeps counting rents and direct targets |
| `world_snapshot` / `world_restore` / `world_snapshot_drop` | 5 | no revert point exists |
| the journal fields and `JournalStart` | 3 | the request is skipped on resume and the settle is lost |

Nothing in §5 is speculative: every field is read by at least one item, and
every item's test in §10.1 reads the field it exists for. `body_replaced` is
the one bit with no card consumer: it is kept because it is the only way to
separate "replaced" from "nothing ran", and the Door guard and the log tests
read it.

### 13.4 The breaks are the price, and it is small

* **ABI 115.** Nine append-only imports and one kind; a pre-bump module
  cannot instantiate against the new host, which is the point of the version.
* **SAVE 17.** Two world collections and two pending collections, all
  defaulted; RECORD unchanged.
* **No new display text, no decision in an import.** Every decision stays an
  `ask_*` in a body.
* **No game word in the SDK.** The kind's spelling is the manifest's own
  `eventResolved`; the facts are generic.
* **Landing independence.** Item 2 needs G02; item 3 needs G02's request
  split; items 1, 4 and 5 are independent of each other. All land in one
  group because the surface bump is shared.

### 13.5 What would argue against the group

* *"Fix the cards only."* The Door keep-guessing and the arisa inertness are
  both surface holes; no card edit reaches them.
* *"Do HOST-01 by serialising more of the pass instead of a journal."*
  Serialising the whole torn stack is what the journal is; storing every
  prompt answer is half of it and still misreads the request slot.
* *"Defer item 5."* Legitimate: `no_breakup`'s revert is independent. Striking
  it leaves the card's displayed note false and costs three imports.

## 14. Clause match

Every affected clause is quoted with the book's own numbering; each is matched
to the design element, the test and a verdict. **matched** = expressible
exactly and pinned; **matched, assumption** = the design reads an unruled
detail and §12 carries it; **was gap** = this pass found the design short and
the fix is now in the body.

### 14.1 `PPP:（有咲）等等等一下`

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]」 | `by_card_instance` on `event` / `eventAfter` and the `foreign` guard (§6.2, §7.1) | `g10_foreign_event_banks_one_crystal`, `g10_deferred_release_banks_no_crystal` | matched |
| 2a | 「事件触发时如果此卡拥有至少3个[奇迹水晶]」 | the `Event` entry's condition `card.placed && counter('crystals') >= 3` | `g10_arisa_defers_at_three_crystals` | matched |
| 2b | 「则移除此卡3个[奇迹水晶]」 | the body after a claimed `event_defer` (§7.1) | `g10_two_copies_defer_once` | matched |
| 2c | 「那个事件在[拥有者]回合开始时结算」 | the deferral record and the `turn_start` drain (§6.2) | `g10_deferred_event_resolves_at_the_owner_turn_start`, `g10_deferred_record_survives_save_restore` | matched |
| 2d | 「此后额外抽取一个视为其他玩家在“流星堂”抽取的事件」 | `EventResolved` + `draw_event_as(other, 流星堂)` (§5.4, §7.1) | `g10_deferred_extra_draw_is_an_event_at_the_station` | matched, assumption (Q1) |

### 14.2 `PPP:Tomorrow's Door` (3)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「此卡在自身游玩区域时」 | the existing `in_play_area()` guard, kept | existing `door_*` | matched |
| 2 | 「[拥有者]以外的玩家…[结算]时」 | the condition `actor != owner` and the `settleAfter` stage | `g10_door_tax_runs_after_a_body_replace` | matched |
| 3 | 「在[拥有者]拥有的格子或梦开始的地方」 | the condition `settle.owner == owner || settle.tile == tile_named('梦开始的地方')` | `g10_door_tax_on_the_start_tile` | matched |
| 4 | 「额外支付…星之鼓动山丘上房子数量×100的资金」 | `pay_add` for the rent case; the `settleAfter` transfer for the rest, split by `settle_paid` (§6.1, §7.2) | `g10_door_tax_skips_after_a_paid_rent`, `g10_settle_facts_carry_paid_and_value` | matched, assumption (Q8) |

### 14.3 `R:学生会的检查` (refund half)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「若此卡进入弃牌堆时」 | `Discarded` body, unchanged (G02/G10) | `g02_*` | matched |
| 2 | 「结算获得的资金小于本卡原应拿到的资金或未结算」 | the `TileResolved` cleanup reading `settle_outcome()`; G10 only co-pins it | `g10_council_uses_the_outcome_fact` | matched (landed) |
| 3 | 「则获得发动此卡消耗的资金」 | the refund of `SLOT_PAID`, unchanged | `g02_council_cancelled_settle_refunds` | matched (landed) |

### 14.4 `RAS:成为最强`

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「roll 1d10」 | existing `ctx::roll`, unchanged | existing | matched |
| 2 | 「传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到“Live House”）」 | the existing table; the journal makes the post-settle face read and the move replay (§6.3) | `g10_ownership_read_sees_the_deed_bought_by_the_move` | was gap (now covered) |
| 3 | 「若你没有Livehouse格子，传送到“Live House”」 | the existing ownership read; the read is now correct after a nested offer | `g10_journal_survives_nested_prompt` | was gap (now covered) |
| 4 | 「视为你的主要移动」 | the existing `cant_play` / plan flag | existing | matched |

### 14.5 `RAS:EXIST`

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「场上及打出的所有对单一玩家生效的手卡…的目标将改为你」 | the existing `Redirect` Filter, unchanged | existing `exist_*` | matched |
| 2 | 「若在此期间此卡没有造成影响」 | the `Effect` entry's `redirected()` guard (§6.4, §7.5) | `g10_exist_latch_needs_a_landed_redirect`, `g10_exist_latch_ignores_a_plain_rent_payment` | matched, assumption (Q3) |
| 3 | 「你的下回合开始时将其翻入弃牌堆」 | the existing `TurnStart` body, unchanged | existing | matched |

### 14.6 `Sumimi:Sumimi不会解散哦`

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「当你本回合未进行过抵押/赎回操作且资金不含有相同数字时可打出」 | existing `cant_play`, unchanged | existing | out of group |
| 2 | 「进行一次可使你的资金变动为拥有相同数字的传送」 | the existing ask and plan, unchanged | existing | out of group |
| 3 | 「若传送并触发结算后未能使资金变为拥有相同数字」 | the `tileResolved` guard and the post-settle money read (§7.6) | `g10_no_breakup_reverts_a_failed_teleport` | matched |
| 4 | 「回到原处并取消所有受到的效果」 | `world_snapshot` / `world_restore` over the six families (§6.5) | `g10_no_breakup_reverts_a_failed_teleport`, `g10_world_restore_is_idempotent` | matched, assumption (Q7) |

### 14.7 Neighbours the design must not break

| card / site | clause | how it stays true |
|---|---|---|
| `通用:停车场` (body replacer) | replaces the settle body | the facts record `body_replaced`; the Door tax now still runs |
| the arisa skill | 「每次有其他玩家在“流星堂”抽取事件时」 | `draw_event_as` stamps the tile, so the hook matches the extra draw and any other event drawn there |
| `AG:像往常一样` | 「回到起始地点并取消所有受到的效果」 | `world_restore` writes positions directly (`teleport_as` untouched); the turn snapshot is a different store |
| every existing `settleAfter` / `tileResolved` reader | outcome payload | `outcome` still stamps; the facts are additive |
| the ask/replay suites (`docs/BOT.md` §3.2) | replay equivalence | the journal replays the same answers; the new requests are journaled like every other |
| every existing `event` / `eventAfter` hook | no tile / no cause | `tile` is now the draw's position and `by_card_instance` is set only on event raises; `eventAfter` carries the same four values as its `event` |

### 14.8 Result of the match

Three clauses were short and are now covered: the two `be_strongest`
destination branches and the `no_breakup` revert. Every other clause maps to
an existing behaviour plus one named design element, and every mapping has a
test in §10.1 or an existing suite.
