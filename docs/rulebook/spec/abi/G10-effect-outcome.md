# ABI design G10 — Effect outcome, cause and settle facts

**Status:** draft for user review · design only, no source touched.
**Worktree:** `.claude/worktrees/card-rulebook-review` (branch `worktree-card-rulebook-review`).
**Group:** G10, family "Effect outcome, cause and settle facts".
**Ground truth:** `docs/rulebook/cards.json`; every fragment below is verbatim from it.
**Row ids** are the review-sheet row ids carried by `G10.json` (they are not
`cards.json` indices).

Terminology: **counteraction** = the [反击] mechanism (chain / window).
**Hook** = a passive listener (`On::Action` / `On::Filter`). Never "reaction".
**Outcome** = what a settle actually did, as a fact, not as a re-derivation.

**Closed slugs and rows:** all 5 items are covered; one half-item is demoted.

| # | slug | severity | rows | verdict |
|---|---|---|---|---|
| 1 | `event-defer` (`arisa-clause-2-event-defer`, `arisa-clause-1-cause-filter`) | medium | 567, 568 | B |
| 2 | `settle-outcome-flags` (`door-settle-body-replace-fact`, `council-check-cancelled-settle`) | medium | 792, 1428, 1449 | B; the `council` half is **demote to A** |
| 3 | `host01-journal` (`be-strongest-host01-nested-settle-ownership`) | medium | 724, 728 | B (engine-only, no export) |
| 4 | `redirect-latch` (`exist-influence-latch-scope`) | medium | 781, 1465 | B |
| 5 | `world-snapshot` (`nobreakup-revert-half`) | medium | 782, 1077 | B |

No item is dropped. The only A-class part is the council cleanup re-home
(§1.2.2, §6.3): it can ship today with the existing surface and is
**demoted to A**; the item stays B because Tomorrow's Door still needs the
`paid` fact.

---

## 1. Problem, per item

### 1.1 `event-defer` (rows 567, 568)

Book (`PPP:（有咲）等等等一下`):

> （1）任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。

> （2）事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。

Both fragments are verbatim from `cards.json#PPP:（有咲）等等等一下` (verified);
they are one card's two clauses, and the sheet cell holds both.

Current (`rules/cards/card-ppp/src/arisa_wait.rs`): the card registers only
`On::Play` and `On::Action(Action::EventAfter(…))`. Nothing suspends an event.
`draw_event` (`crates/game-core/src/engine/play.rs:4943`) raises `event`,
binds the event's rule instance, runs its body, files the card and raises
`eventAfter` — one synchronous pass with no suspension point and no
re-resolution path.

Two further gaps make clause (2) impossible on the current surface, and one
makes clause (1) wrong:

* **No deferral record.** There is no place to park “this event card, for this
  drawer, due at this owner's next turn start”.
* **No event cause.** `Trigger.by_card` is a **seat**
  (`crates/card-sdk/src/abi.rs:23`, v11). Clause (1)'s 「非因为此卡导致」 is
  about *this card instance*, not about its seat: two copies of the arisa
  card on one field would each have to exclude only their own releases. There
  is no instance cause (`by_card_id`), so today every `eventAfter` banks a
  crystal and the TODO at `arisa_wait.rs:76-79` says so.
* **No attribution for the extra draw.** The extra event is 「视为其他玩家在
  “流星堂”抽取」. `ctx::draw_event(player_id)` carries a player only, and
  `draw_event` leaves `tile` at `-1`, so the station is not on the trigger.

The station matters for a second card: the persona skill
`rules/skills/skill-personas/src/arisa.rs:27` hooks
`On::Action(Action::Event("actor != owner && tile.id == tile_named('流星堂')", …))`
for 「每次有其他玩家在"流星堂"抽取事件时获得一个[火罐]」. With `tile = -1`
on every `event` raise that hook can never match, and bonsai's own clause (2)
does not even draw an event (it calls `ctx::draw`, a **hand** draw,
`arisa.rs:62`). `draw_event_as(player, tile)` repairs both: it is
the primitive for “an event drawn as X at tile Y”.

### 1.2 `settle-outcome-flags` (rows 792, 1428, 1449)

#### 1.2.1 Tomorrow's Door (row 792)

Book (`PPP:Tomorrow's Door`):

> （3）此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。

Current (`rules/cards/card-ppp/src/tomorrows_door.rs`): the tax has two
halves — a `paySplitAdd` add on the rent payment (`pay_add`) and a
`settleBody` entry for the shapes no rent payment carries (`settle_body`,
guarded by `trigger::cancelled()`). The `settleBody` half is wrong in two
directions the card's own TODOs name:

* `trigger::cancelled()` in the settleBody hook is the **settleBody link's**
  cancellation, i.e. “someone else replaced the body” — so a body replacer
  (Parking Space, `rules/cards/card-general/src/parking_space.rs`) that runs
  *before* Door's hook drops the tax, although the settle still resolves and
  the book still charges it.
* Whether rent ran is guessed from `mortgaged_of(at)`; a body-replaced settle
  on a mortgaged tile therefore charges or not by hook order rather than by
  whether rent was actually paid.

The settle body stage cannot see a fact that is decided *after* it, and
`settleAfter` cannot see whether rent was paid. The card needs the settle's
own outcome bits.

#### 1.2.2 Student council check (rows 1428, 1449) — **demote to A**

Book (`R:学生会的检查`):

> 若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金。

Current (`rules/cards/card-roselia/src/council_check.rs`): the refund/cleanup
runs in `settle_after` (line 244) and clears `SLOT_STOP`. A settle cancelled
at `settle` jumps straight to `TileResolved`
(`play.rs:2163-2175`), so `settleAfter` never runs; the card stays on the
tile with `SLOT_STOP` armed and `SLOT_PAID` refundable — 「未结算」 never pays
back.

This half is fixable without new ABI: `settle_after` already clears
`SLOT_STOP`, so a new `tileResolved` hook guarded by `slot(SLOT_STOP) != 0`
can do the same cleanup on the cancelled path (a normal settle cleared the
slot first, so no double run). That is **demote to A** — ship it with the
latch, no surface.

### 1.3 `host01-journal` (rows 724, 728)

Book (`RAS:成为最强`):

> 若你没有Livehouse格子，传送到“Live House”

Current (`rules/cards/card-ras/src/be_strongest.rs:59-61`): the ownership read
(`owned_tiles(...).any(is_live_house_for)`) runs before the teleport; when the
teleport's settle buys a deed through the agent-tile offer, the master
(commit) re-run's read is not guaranteed to see it. `docs/ENGINE.md` §"Drive
replay semantics" states the gap precisely: a write inside a host routine's
own nested settle “may not be on live until that routine finishes
prompting”.

The mechanism: `Match::execute` (`crates/game-core/src/engine/mod.rs:1571`)
halts on `HaltKind::Ask` and stores `Pending { snapshot, answers, … }`
**with the pre-routine snapshot**; `complete_prompt` (mod.rs:1704) re-executes
from it. During the aborted pass the nested settle appended its prompt's
answer at the next answer slot although the enclosing `HostRequest` never
returned. On resume, the guest host-call shim
(`crates/game-rules/src/hostfns.rs:860, 1626, 1961, 2047`) sees an answer at
the enclosing request's slot, returns it **without re-applying the request**,
and the nested settle (with the deed) never runs again. The player keeps a
bogus answer value and the world loses the settle.

Nothing on the guest side can fix this; it is engine plumbing. **No guest
export**, no contribution to the wire bump.

### 1.4 `redirect-latch` (rows 781, 1465)

Book (`RAS:EXIST`):

> 若在此期间此卡没有造成影响，抽1张卡

Current (`rules/cards/card-ras/src/exist.rs:24`): the latch is
`On::Action(Action::Effect("target == owner && actor != owner", …))`. But
`effect` is raised for **every** money movement too
(`play.rs:3445-3460`) and by any direct targeting, so:

* a plain rent payment to the owner — `target == owner`, `actor != owner` —
  marks the card as having had an influence;
* a bank print to the owner — same;
* an opponent's effect that directly names the owner (never redirected) —
  same.

Only a redirect that **landed** is an influence. `redirect` is a Filter on
the declaration path (`wasm_rules.rs:5668-5690`); a Filter body cannot write a
prop (`set_prop` is `refuse_if_filter`'d, `exist.rs:22`), and the effect
payload has no “redirected” flag. The hook that can write is `effect`, and it
cannot tell whether the rename happened.

### 1.5 `world-snapshot` (rows 782, 1077)

Book (`Sumimi:Sumimi不会解散哦`):

> 若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果

Current (`rules/cards/card-sumimi/src/no_breakup.rs`): the teleport settles
and consumes the main move, but nothing rolls the world back. `ctx::turn_snap`
(v104) exists, but it is captured at the player's **turn start**, so it is
stale for a play made during the turn: it would revert every move and every
settle since the turn began, not just this card's. What is needed is a
play-time, card-scoped checkpoint of every player's `pos` / `state` /
`money`, and the board's `owners` / `houses` / `mortgaged`.

The revert point is `tileResolved` (`SETTLE-STAGES.md` R3), the terminal that
also fires on a cancelled settle.

---

## 2. Shared root causes

1. **A settle is a stage machine but its outcome is not state.** Stage
   branches (`play.rs:2163-2230`) read the return of each raise and throw the
   information away; later stages and terminal hooks see only the tile. Every
   consumer re-derives outcome from proxies (mortgage bit, slot arms, hook
   order) and gets it wrong in a different way.
2. **Cause is carried as a seat, never as an instance.** `by_card` answers
   “whose card”, which cannot separate two copies of one card. The same
   shape recurs in G10's event clauses: the card must exclude *itself*
   (arisa), and it must attribute a nested draw to itself (the extra event).
3. **The multi-pass drive assumes host routines apply atomically.** A routine
   that prompts is torn: its effect is partially on live, its answer slot is
   not, and the resumed pass's slot walk misreads the nested prompt answer as
   the routine's. Only the engine can restore the atomicity.
4. **Outcome facts that a hook needs are decided inside other subsystems.**
   “Did a redirect land” is decided by `target_player`'s fold; “did rent
   commit” by the money pipeline. A hook cannot observe either.

---

## 3. Capability as an invariant

**I-1 (deferral).** A deferred event card is neither lost nor duplicated:
exactly one deferred record exists per event, it resolves exactly once — at
the deferring instance's owner's next turn start, or at the first turn start
after that owner leaves the game — and its resolution names the deferring
instance as the cause. The extra draw is an event draw attributed to a seat
that is not the owner and to the named tile.

**I-2 (settle outcome).** Every settle exposes its own outcome to every hook
that runs after its body stage: `cancelled` iff no body could run (the
`settle` link was negated or the mover was out), `body_replaced` iff the
`settleBody` link was negated, `paid`/`paid_value`
iff a rent payment to the settle's owner for the settle's own tile actually
committed. The bits are stable once the body stage ends and are born with the
settle frame, not with the tile.

**I-3 (journal).** For every effectful host request, the world the
resumed/landing pass reads is exactly the world that request produced: never
a partial application, never a duplicate. The request's own answer occupies
its own answer slot, and a re-application replaces the previous one rather
than stacking on it.

**I-4 (redirect).** `redirected_from` is present on the `targeted`, `effect`
and `target` triggers of a designation iff a `redirect` hook moved the hit
from the declared seat to a different, live, non-acting seat — and absent on
every money-raised `effect`.

**I-5 (checkpoint).** `world_restore(k)` leaves the six covered families
(`pos`, keyed `state`, `money`, `owners`, `houses`, `mortgaged`) exactly as
`world_snap(k)` stored them; it is idempotent; a missing key is a no-op that
answers 0.

All five are generic mechanisms: no card names, no display text, no player
decision inside an import.

---

## 4. Surface

### 4.1 `Trigger` — new fields (`crates/game-core/src/engine/rules.rs:231`)

```rust
/// The **instance** whose card caused this trigger, or `None`. `by_card` is
/// the seat; this is the identity that separates two copies of one card.
/// Set only where the raiser is a running card (event draws and their
/// after points). Not a save field.
pub by_card_id: Option<i32>,
/// The seat a `redirect` hook moved this designation from, on `targeted` /
/// `effect` / `target` only, and only when the rename landed (I-4).
pub redirected_from: Option<i32>,
/// The settle this trigger belongs to, on `settleBody` / `settleAfter` /
/// `tileResolved` only.
pub settle: Option<SettleFacts>,
```

```rust
/// The outcome of one settle, born with its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SettleFacts {
    /// The settle's tile and owner, copied from the frame at birth so the
    /// `paid` predicate can read them without the work stack.
    pub at: usize,
    pub owner: i32,
    /// The settle ended before any body: the `settle` link was negated, or
    /// the mover was already out. No `settleBody`, no `settleAfter`.
    /// Derived from G02's `SettleScope.outcome` (`Cancelled`).
    pub cancelled: bool,
    /// The `settleBody` link was negated — the body was replaced.
    pub body_replaced: bool,
    /// A rent payment to this settle's owner, on this settle's tile, committed.
    pub paid: bool,
    /// What that payment moved (0 without `paid`).
    pub paid_value: i32,
}
```

### 4.2 Imports, one line each (`crates/game-rules/src/host.rs` register +
`crates/card-sdk/src/ctx/sys.rs` externs)

| import | signature | answers |
|---|---|---|
| `event_defer` | `() -> i32` | 1 = a deferral record exists (created now, or created by this same call and re-run); 0 = not an `event` trigger, or another instance already deferred this event |
| `draw_event_as` | `(player_id: i32, tile: i32) -> i32` | 1 (the whole event pipeline ran) |
| `trig_by_card_id` | `() -> i32` | the cause instance uid, or -1 |
| `trig_settle_facts` | `() -> i32` | bitset `1 cancelled`, `2 body_replaced`, `4 paid`; 0 outside a settle hook |
| `trig_settle_paid_value` | `() -> i32` | the committed rent amount, else 0 |
| `trig_redirected_from` | `() -> i32` | the seat the rename moved from, else -1 |
| `world_snap` | `(key: i32) -> i32` | 1 stored; 0 a snapshot already exists for `key` (insert-if-absent) |
| `world_restore` | `(key: i32) -> i32` | 1 restored; 0 no snapshot |
| `world_snap_drop` | `(key: i32) -> i32` | 1 dropped; 0 none |

`HostState` gains two transient fields, `reapply: BTreeSet<usize>` and
`last: BTreeMap<usize, HostJournalEntry>` (§5.3); the value reads are direct
`hostfns` functions over `c.st()/st_mut()` and `c.st().wr().trigger()` -- the
**guest** `Trigger` mid in `game-rules/src/world.rs`. Every new field of 4.1
therefore goes on that mirror as well as the engine struct, and
`bridge_trigger` (`wasm_rules.rs:7270-7331`) copies it, or every reader sees
the default. Every **writing** import
here (`event_defer`, `draw_event_as`, `world_snap`, `world_restore`,
`world_snap_drop`) is a host request (§4.5) instead: a direct host write goes
to the caller's run copy, which a later prompt in the same body drops, while
a request is applied against live and replay-skipped. The bridge's apply site
supplies the `uid` / `card` / seat, so no `CardWorld` / `Run` signature change
is needed.

### 4.3 Sugar (`crates/card-sdk/src/ctx.rs`) — translates before the ABI

```rust
pub fn event_defer() -> Result<bool, Prompt>;                 // asked(1/0) != 0
pub fn draw_event_as(player_id: i32, tile: i32) -> Result<(), Prompt>;
pub mod trigger {
    pub fn by_card_id() -> Option<i32>;                       // (v >= 0).then_some
    pub fn settle_cancelled() -> bool;                        // bit 1
    pub fn settle_body_replaced() -> bool;                    // bit 2
    pub fn settle_paid() -> bool;                             // bit 4
    pub fn settle_paid_value() -> i32;
    pub fn redirected_from() -> Option<i32>;
    pub fn redirected() -> bool;                              // from.is_some()
}
pub fn world_snap() -> i32;                                   // key = self_uid()
pub fn world_restore() -> i32;
pub fn world_snap_drop() -> i32;
```

The request apply site supplies the running `uid` / card / seat, so the guest
passes nothing for `event_defer`. `world_snap` sugar passes `ctx::self_uid()`;
the ABI takes an explicit key, so a card can hold several.

### 4.4 Kinds and entries

* `TriggerKind::EventResolved = Self::engine(131)` (README band; 110 is
  `PaySplitMulBefore`, 94 retired).
* `ActionKind::EventResolved = Self::engine(131)` with the doc line and an
  `action_kinds!` entry (`crates/card-sdk/src/kinds.rs`, `EventAfter;` +
  `EventResolved;`).
* The release dispatches **own-instance**, like `Drawn`
  (`wasm_rules.rs:5021-5040`): `t.card = deferrer_card`,
  `t.cards = [deferrer_card]`, uid-filtered to `deferrer_uid`, via
  `drive_hook(..., deferrer_uid, ...)`.
* `EventResolved` is hook-only (`kind_flags::hook_only`), never a
  counteraction window.

### 4.5 Host requests (`crates/game-rules/src/host.rs:383`)

```rust
DrawEvent { player_id: i32 }                       // unchanged arity for v104 modules
DrawEventAt { player_id: i32, tile: i32 }          // new: draw_event_as
EventDefer                                         // unit; the bridge fills uid/card
WorldSnap { key: i32 }                             // world_snap
WorldRestore { key: i32 }                          // world_restore
WorldSnapDrop { key: i32 }                         // world_snap_drop
```

`HostRequest::DrawEvent`'s three new facts (`tile`, `by_card`, `by_card_id`)
arrive **as a new request `DrawEventAt`** rather than by changing
`DrawEvent`'s construction sites: `apply_host_request` fills `by_card` from
its `seat` parameter and `by_card_id` from its own `uid`; `tile` is the
request's, and -1 means “the drawer's current pos”. The plain
`ctx::draw_event` becomes `DrawEventAt { tile: -1 }`.

`HostRequest` and `Call` are not serializable and live in `game-rules`, while
the journal is saved state in `game-core` (§5.3). The journal therefore
stores a **`JournalRequest`** mirror, in `game-core`. **Every**
`HostRequest` variant is journaled — the enum exists only for work that
touches the world, and any such work can be followed by a prompt in a nested
drive. Representative shape:

```rust
// `PartialEq` (no `Eq`) for the recompute comparison; `MoveCtx` carries `f64`.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
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
    DrawEventAt { player_id: i32, tile: i32 },
    EventDefer,
    WorldSnap { key: i32 },
    WorldRestore { key: i32 },
    WorldSnapDrop { key: i32 },
    // …the remaining existing variants (OfferBuild, Build, Mortgage,
    // ApplyMortgage, ApplyRedeem, Agent, ChargeRent, PayRent, Linger, …),
    // …G09's `TileWrite` / `BuildAs` (it lands first; mirror them here), and
    //    G14's `Draw.over_hand_limit` when it lands after (add the field).
    // same fields as `HostRequest`.
}
```

`HostRequest::DrawEvent { player_id }` (still emitted by v104 modules) maps to
`DrawEventAt { player_id, tile: -1 }`.

`impl From<&HostRequest> for JournalRequest` and the reverse in
`game-rules`; the replay site rehydrates and calls `apply_host_request`.

### 4.6 New / extended / wire-breaking

| change | class |
|---|---|
| 9 new imports (§4.2) | new wire |
| `Trigger` + `SettleFacts` fields (§4.1) | new guest-visible payload, read through the new imports |
| `TriggerKind` / `ActionKind` 131, spelling `eventResolved` | new wire value (manifest trigger spelling) |
| `HostRequest::DrawEventAt` | new request variant; reaches a save only through its `JournalRequest` mirror, not record-visible |
| `EventDefer`, `WorldSnap`, `WorldRestore`, `WorldSnapDrop` requests | same |
| renumbered ids | none |
| changed semantics of an existing id | none |

`draw_event_as` and `event_defer` can prompt indirectly (an event body may
ask), exactly like `draw_event`. No import contains a player decision.

### 4.7 World state

```rust
// World, beside `event_deck`
#[serde(default)]
pub deferred_events: Vec<DeferredEvent>,
#[serde(default)]
pub world_snaps: std::collections::BTreeMap<i32, WorldSnap>,

// Pending (the save's torn-routine slot), `#[serde(default)]`
pub host_journal: Vec<HostJournalEntry>,
// Cx, transient, a copy of the pending stack while a pass runs
pub host_journal: Vec<HostJournalEntry>,
pub journal_resume: bool,
// HostState, transient per pass
pub reapply: BTreeSet<usize>,
pub last: BTreeMap<usize, HostJournalEntry>,
```

The journal is not world state: it exists only while a routine is torn, and
a save is only taken at a prompt, so it rides `Pending` (which `Saved`
already serializes) rather than `World`. `GuestOverlay` gains
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
    /// The deferring instance: release cause and `eventResolved` dispatch.
    pub deferrer_uid: i32,
    pub deferrer_card: String,
    /// `by_card` on the release, kept beside the record rather than re-read.
    pub by_card: i32,
    /// Turn key when deferred; the release requires a strictly later one.
    pub after_turn_key: i32,
}
```

`DeferredEvent` / `HostJournalEntry` / `WorldSnap` live in
`crates/game-core/src/engine/world.rs`; all three are `Serialize,
Deserialize`. They sit inside `World` / `Pending`, which derive
`Debug, Clone, PartialEq` (`world.rs:615`, `mod.rs:294`), so every one of them
must derive `Debug` and `PartialEq` too or the containers stop compiling.
`PlayerSnap` is `Copy` as well.

---

## 5. Engine wiring

### 5.1 Stage reporting (`play.rs`)

* G02 (which lands before this group) already replaced the `settle_open`
  counter with `Cx.settle_scopes: Vec<SettleScope>`, pushed once per frame and
  popped on completion, and gave the scope its terminal `outcome`. This group
  does **not** re-fix the counter; it extends that scope.
* `SettleScope` gains `paid: bool` and `paid_value: i32`; `SettleFacts` is a
  snapshot of the scope, `at`/`owner` read from `scope.tile`/`scope.player`
  (which is why both fields are redundant on the scope and are not stored
  twice). `cancelled` is `scope.outcome == Cancelled` — no second bool.
* `RaiseSettleBody`: `if si.is_cancelled()` — set a new
  `scope.body_replaced: bool`.
* In `money_inner`, after the balances are written (play.rs:4045-4054) and
  before the `paid` raise: with the innermost scope (none open → skip), if
  `p.rent && loss > 0 && scope.player >= 0 && p.settle_tile == scope.tile as i32 &&
  p.to == Some(scope.player)`, set `paid` and add `loss` to `paid_value`. The
  test uses G02's `p.settle_tile` payload field, not the payment's own
  `p.tile`. A payment cancelled or reduced to 0 never reaches it.
* `RaiseSettleAfter` / `RaiseTileResolved`: build `t.settle` from the frame's
  scope (`SettleFacts { at: scope.tile, owner: scope.player as i32,
  cancelled: scope.outcome == Cancelled, body_replaced: scope.body_replaced,
  paid: scope.paid, paid_value: scope.paid_value }`).
  `settleBody` carries the same snapshot with `paid = false`, `body_replaced =
  false` (facts so far).
* The bits are readable through the bitset import; `paid_value` in the same
  call.

### 5.2 Deferral queue

* `event_defer` is a **host request** (`EventDefer`), not a direct write: a
  direct host write goes to the caller's run copy and is lost when a hook
  prompts, while a host request is applied against live and is replay-skipped.
  `apply_host_request` requires `trigger.kind == "event"`; an existing record
  for `(trigger.card, trigger.player_id)` answers 0 unless its
  `deferrer_uid == uid`, which answers 1 without pushing (the re-run of this
  same call). Otherwise it pushes
  `DeferredEvent { event: trigger.card, player: trigger.player_id,
  tile: trigger.tile, due_owner: seat, deferrer_uid: uid,
  deferrer_card: card, by_card: seat,
  after_turn_key: turn_key() }` into `cx.world_mut().deferred_events` and
  answers 1. Outside an `event` raise, answer 0.
  **The record itself is the “deferred” signal** — no trigger flag, which
  would be lost when the hook drive halts and re-runs with the call skipped.
* `draw_event` and the release share one routine,
  `resolve_event(card, player, tile, by, cause)`:
  1. `raise!(event, player, card = Some(card), tile = tile, by_card = by,
     by_card_id = cause)`.
  2. If a record now exists for `(card, player)` — a fresh deferral, which
     includes the deferral that suspended this very call — return
     “deferred”: no bind, no body, no filing, no `eventAfter`. The record is
     the whole signal, so the re-run after a prompt finds it again and stays
     deferred. (Two copies both at ≥3 crystals: the first `event_defer`
     answers 1, the duplicate rule makes the second answer 0, so exactly one
     record and one 3-crystal payment.)
  3. Otherwise bind, run the body, file, raise `eventAfter` with the same
     cause; return “resolved”.
* `draw_event` calls it with `tile = pos(i)` and `by`/`cause` from the
  request and returns. The release calls it with the record's fields and
  first replays the draw's public head (the `log.event` line, `wait(3.0)`,
  and the reveal — the card is off the deck and off `eventTop`, so the pop
  and retain steps are skipped; the later file/refill steps are the normal
  ones).
* `turn_start` (`play.rs:580`), immediately after the `turnStart` raise and
  before `take_end_turn_request`: drain records where
  `due_owner == i && turn_key() > after_turn_key`, plus records whose
  `due_owner` is out (any seat, first turn start after the leave, **without**
  `EventResolved`). For each: remove the record, `resolve_event(rec.event,
  rec.player, rec.tile, Some(rec.by_card), Some(rec.deferrer_uid))`, and only
  on “resolved”, with a still-live deferrer,
  `raise!(eventResolved, rec.player, card = Some(rec.deferrer_card), cards =
  vec![rec.deferrer_card], by_card = Some(rec.by_card), by_card_id =
  Some(rec.deferrer_uid))` — the own-instance dispatch of §4.4. A re-deferred
  release raises no `EventResolved`; its extra draw follows the later
  resolution. Re-check `out(i)` between records.
* The release never re-raises `settleBefore`; an event resolves where the
  original draw stood, so its body uses `rec.player`'s position.

### 5.3 HOST-01 journal

`HostJournalEntry`:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostJournalEntry {
    /// The answer slot this request owns.
    pub slot: usize,
    /// The request payload, replayed verbatim on resume.
    pub request: JournalRequest,
    pub card: String,
    pub uid: i32,
    /// The seat the request was issued for.
    pub player: i32,
    /// Exact world before the request.
    pub pre: Box<World>,
    /// The guest state the body had already cleared when it called (the same
    /// `GuestOverlay` the live `NeedHost` boundary pushes).
    pub guest: GuestOverlay,
    /// Answers generated inside the request, in the flight layout (own answer
    /// absent) — `slot ..`.
    pub nested: std::ops::Range<usize>,
}
```

* `apply_host_request` gains `uid: i32`, `trigger: &mut Trigger` and takes
  `seat: i32` in place of `call` (the body only ever reads `call.player_id()`;
  the live caller passes `call.player_id()`, the replay passes the entry's
  `player`), plus the `guest` overlay. At entry of **every** `HostRequest`
  (they are all effectful by construction; plain reads are separate imports)
  push an entry with `slot = st.next_answer`, `pre = Box::new(cx.world_copy())`,
  `nested = slot..slot` onto `cx.host_journal`. On normal return insert the
  answer at `slot` (the caller saves `st.next_answer` before the call instead
  of pushing at the end) and pop the entry.
* **Guest overlay.** `overlay_guest_state` becomes a capture-and-push
  returning the `GuestOverlay` (`can_pay` needs a clear the body just made;
  壱雫空). The live call stores that overlay in the entry, and a replay
  overlays an entry's `guest` for the duration of its re-application and
  restores after — the same discipline the live pass uses.
* On `Err(Halt::Ask)` inside the request, leave the entry in place and set
  `nested.end = answers.len()`. `Pending` gains
  `#[serde(default)] host_journal: Vec<HostJournalEntry>`; the Ask arm moves
  the Cx stack into it and clears the Cx. `Pending.snapshot` stays the
  **routine** snapshot: the body re-runs from the top, so engine direct
  writes that precede the request (the event-deck pop, `wait`s) are undone
  and replayed exactly once.
* **Answer layout.** A request owns one answer slot but pushes it only when
  it returns; the answers of prompts raised *inside* it start at that slot.
  While an entry is in flight its own answer is therefore absent from the
  vector, and its `nested` range names the answers it produced. The replay
  keeps that invariant: before applying an entry it removes any stale own
  answer at `slot`; after it completes it inserts the fresh answer at `slot`.
* **Replay.** On resume, `complete_prompt` passes `p.host_journal` to
  `execute`, which installs it as `cx.host_journal` (`start` passes empty),
  seeds every in-flight entry's slot into `HostState.reapply`, and sets a
  one-shot `cx.journal_resume`. The first drive's start consumes the flag,
  clears the front entry's mark, restores the entry, overlays its `guest`,
  applies its request, restores the overlay and continues into the body — so
  the first body pass runs on the post-request world. That is what gives
  `be_strongest` its corrected read at the top of the body and, when the
  recomputed destination differs, its re-applied move.
* A replay that re-enters a marked slot takes the stack's front entry instead
  of pushing a new one: the wrapper does `reapply.remove(&slot)` first, then
  the restore/overlay/apply above, and continues the drive loop so the body
  runs again rather than returning the stale answer.
* An **unmarked** slot with an answer is the ordinary re-encounter after an
  earlier pass: the wrapper compares the fresh request with the one kept for
  the slot (`last`). Equal → return the answer. Different → the body
  recomputed its arguments from the post-request world; restore `pre`,
  overlay, apply, and continue the loop, replacing the prior answer (and the
  prior application's effects, which `pre` undoes). A per-pass cap of 4
  applies per slot bounds a pathological recompute; hitting it keeps the
  last application.
* If a later prompt aborts the body again, the next re-run re-encounters a
  slot whose request did not change and the call is skipped, so the player
  does not move (or pay, or settle) twice.
* After the last in-flight entry completes, its answer sits at its own slot,
  the nested answers are dead, and the guest body re-runs from the top against
  the post-request world. The top-of-body ownership read sees the deed.
* The vector therefore holds **only in-flight entries**: a completed request
  pops its entry, so outside a torn stack it is empty.
* A frame abort (`Err` other than suspend) drops the entries it pushed.
* `last` keeps a completed entry only for the passes that may re-encounter
  it; it is cleared when the drive returns.

`pre` stores a whole `World`; cloning is refcount-cheap for the event tail
and bounded by the effectful-request count in one body.

### 5.4 World checkpoint (`world.rs`, `play.rs`)

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldSnap {
    pub players: Vec<PlayerSnap>,   // pos / state / money, per seat
    pub owners: Vec<i32>,
    pub houses: Vec<i32>,
    pub mortgaged: Vec<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnap { pub pos: i32, pub state: Vec<(String, i32)>, pub money: i32 }
```

* `world_snap(key)`: insert-if-absent; snapshot `players[*].pos`,
  `players[*].state` (the `state` map with values, the same projection
  `TurnSnap` uses), `players[*].money`, `st.owners`, `st.houses`,
  `st.mortgaged`.
* `world_restore(key)`: write them back, absolute (idempotent). Positions are
  written directly, not via `teleport_as`, so no abnormal gate and no move
  bookkeeping (a revert is not a move). `turn_start_pos` / `turn_snap` are
  untouched.
* `world_snap_drop(key)`.
* All three run as `HostRequest`s (§4.5), so the snapshot/restore lands on
  live and is not lost when the calling hook prompts; a restore is a plain
  value write, never a move.
* Keys are opaque i32; `ctx::self_uid()` is the conventional key, so the
  entry dies with the instance when the card drops it.
  `destroy_instance_units` (`ops.rs:2123`) also drops the instance's key (a
  stale key otherwise leaks the world clone into every later save).

The restore at `tileResolved` runs while the frame is still on the work stack;
restoring `pos` does **not** retarget the settle (the frame's `at` is fixed,
`SETTLE-STAGES.md` §7). The main-move latch is **not** restored — an open
question (§11 Q6).

### 5.5 Redirect fact (`wasm_rules.rs:5654`)

In `target_player`: keep the declared `p` as `orig`; after the `redirect`
fold compute `landed = p != orig`. Pass `landed.then_some(orig)` into
`target_window`, which stamps `t.redirected_from` on `targeted`, `effect`
and `target`. The money-raised `effect` (`play.rs:3448`) leaves it `None`.
`target_tile` has no redirect fold and passes `None`.

A redirect link whose mutation applied but which a counter then cancelled is
“landed” in behaviour (the rename stands); the fact mirrors the rename, not
the link's negation. Recorded as Q3.

### 5.6 Kani model (`crates/card-sdk/src/kani_host.rs`, `ctx/sys_kani.rs`,
`rules/kani/src/host.rs`)

Ops 431–449 in order of §4.2. Models:

* `event_defer` → 1 when the modelled trigger kind is `event` and not yet
  deferred, else 0; sets the flag in the model.
* `draw_event_as` → 1; appends a modelled draw at `tile` for `player`.
* `trig_by_card_id` → the model's cause uid; `trig_redirected_from` → its
  redirect source; `trig_settle_facts` → its bitset; `trig_settle_paid_value`
  → its value.
* `world_snap`/`world_restore`/`world_snap_drop` → a `BTreeMap` in the model
  with the same insert-if-absent / idempotent restore semantics.

No op may answer a silent 0 when unmodeled (house rule).

---

## 6. Card usage

### 6.1 `arisa_wait`

Before: only `On::Play` + `EventAfter` banking unconditionally.

After:

```rust
On::Play("", None, arisa_wait),
// （2）「事件触发时…」 -- placement and count are one pre (applicability).
On::Action(Action::Event("card.placed && card.counter('crystals') >= 3", None, defer)),
// （1）「任何非因为此卡导致的事件结算时…」
On::Action(Action::EventAfter("card.placed", Some(foreign), event_after)),
// release callback（「此后额外抽取…」）
On::Action(Action::EventResolved("card.placed", None, resolved)),
```

`defer`:

```rust
let ok = ctx::event_defer()?;
if !ok { return Ok(()); }
ctx::set_crystals(ctx::crystals() - 3);
ctx::log(player_id, &Msg::new(key!("arisa_wait_deferred")));
```

`foreign`: `trigger::by_card_id() != Some(ctx::self_uid())` (`None` = a
board/engine draw, which is foreign). `resolved`: pick the next live seat
after the owner as 「其他玩家」, `ctx::draw_event_as(other,
tile_named("流星堂"))?`.

### 6.2 `tomorrows_door`

Before: `settleBody` entry with `trigger::cancelled()` and the mortgage bit.

After: drop the `settleBody` entry; add

```rust
On::Action(Action::SettleAfter(
    "card.placed && actor != owner && tile.id >= 0 && (tile.owner == owner || tile_is(tile.id, \"start\"))",
    Some(tax_due_guard), settle_after))
```

with `let due = tax_due(); if due > 0 && !ctx::trigger::settle_paid() { transfer }`.
`pay_add` keeps handling the rent case (it composes with `paySplitMul`); the
`settleAfter` entry covers the start tile, a mortgaged owner tile, a
body-replaced settle, and a settle whose rent was cancelled. The payer's
`settle_paid_value()` gives the log figure if wanted. `tax_due_guard` is the
old guard minus the cancellation/mortgage heuristics plus
`!ctx::trigger::settle_paid()`.

### 6.3 `council_check` — demoted half

Add

```rust
On::Action(Action::TileResolved(
    "card.placed && slot('council_stop') != 0", Some(settle_cancelled), cancelled_cleanup))
```

doing what `settle_after` does: unplace to graveyard, refund `SLOT_PAID`
(「未结算」), clear the slots. `settle_cancelled` is
`fn(_: i32) -> bool { ctx::trigger::settle_cancelled() }`. No double run:
`settle_after` cleared `SLOT_STOP` on the normal path.

### 6.4 `be_strongest`

No change to the body: the engine replays the torn `Move` before the resumed
pass, so the ownership read at line 59 already sees the deed the nested
settle bought. It answers the roll's livehouse face, the fresh `card_move`
carries a different plan, and the journal re-applies the move from `pre`
(§5.3), so the player ends on the rolled tile. Delete the `TODO(ABI) HOST-01`
comment at lines 56-58; the read and everything else stay where they are.

### 6.5 `exist`

Before: `Action::Effect("target == owner && actor != owner", …)`.

After:

```rust
On::Action(Action::Effect(
    "target == owner && actor != owner", Some(landed_redirect), mark_used))
```

with `fn landed_redirect(_: i32) -> bool { ctx::trigger::redirected() }`.

### 6.6 `no_breakup`

At the play's start, before the ask:
`ctx::world_snap();` (key = self uid). After `card_move`, at
`tileResolved`, if `!repeated_digits(money_of(owner)) && !player_out(owner)`:
`ctx::world_restore();`. Then `ctx::world_snap_drop();` on both branches.
The restore is the 「回到原处并取消所有受到的效果」 half.

---

## 7. Save / record / replay

* **SAVE_VERSION 16 → 17** (next SAVE bump after the landing order above G10; take the next free one). `World.deferred_events` (outlives turns),
  `World.world_snaps` (a save can be taken between a card's snapshot and its
  restore) and `Pending.host_journal` (a save is taken mid-prompt, with a
  torn routine), all `#[serde(default)]`. Old saves are refused by
  `restore_saved`'s exact-version check (mod.rs:491) anyway; the defaults
  exist so a hand-edited or field-migrated state loads.
* **RECORD_VERSION stays 3.** `Answered` is unchanged; the new fields ride
  world/pending state. A record replays through its archived engine bundle,
  so an older engine never sees v114 keys.
* `Cx.settle_facts`, `Trigger.*` are transient (the routine re-derives them
  on a re-run, exactly like `settle_open`).
* A save inside a settle re-runs the routine; the settle work stack
  (`ContinueSettleWork`) is not saved today either, so nothing new is lost.
* Replay determinism: the deferral queue is world state, so a replayed turn
  start releases the same record in the same order. Answer ordinals are
  the record's own; no renumbering.

---

## 8. Versioning

Entry inserted immediately below the constant, above the newest note (newest first); the v104
turn-snap entry stays below it:

```rust
pub const ABI_VERSION: i32 = 114;   // was 113 (eleventh group)
/// v114: **outcome, cause and world facts** (G10).
///    `event_defer` suspends an `event` link; the record releases at the
///    deferring instance's owner's next turn start (new hook
///    `TriggerKind::EventResolved = 131`, own-instance dispatch) and
///    `draw_event_as(player, tile)` attributes an event draw to another
///    seat and tile (`HostRequest::DrawEventAt`). `Trigger.by_card_id` /
///    `trig_by_card_id` add the causing **instance** beside `by_card`'s
///    seat. `Trigger.settle` / `trig_settle_facts` (bitset) /
///    `trig_settle_paid_value` publish a settle's `cancelled` /
///    `body_replaced` / `paid` outcome to `settleAfter` / `tileResolved`.
///    `Trigger.redirected_from` / `trig_redirected_from` name the seat a
///    landed redirect moved a hit from. `world_snap` / `world_restore` /
///    `world_snap_drop` checkpoint and restore pos/state/money/owners/
///    houses/mortgaged. No id renumbered; new `Op` 431..=449.
///    SAVE_VERSION 16 → 17 (deferred events, host journal, snapshots);
///    RECORD_VERSION unchanged.
```

`ABI_VERSION` is bumped because the import table, the trigger payload and the
manifest kind spelling all change; the host refuses a different version, so
additive is still a bump. `TriggerKind::EventResolved = 131`;
`kind_table_pin.rs:167` (the pin rows end at `(110, …)`) gains `(131, "EventResolved")`.

**Merge caveat:** G06 also appends `Trigger` payload (its `subject_card` /
`join` / `answers_declared`) and lands before this group. Besides the number,
the **struct** merges field-by-field on G06's shape; take its `Trigger` and add
4.1's fields, do not paste a whole struct over it. The entry blocks are
disjoint and only the number moves in the ordinary case.

---

## 9. Tests and Kani obligations

### 9.1 Per item (`rules/tests/tests/`)

| item | test | assertion |
|---|---|---|
| 1 | `g10_arisa_defers_at_three_crystals` | ≥3 crystals + an event draw → event not resolved, 3 crystals gone, one deferred record |
| 1 | `g10_deferred_event_resolves_at_the_owner_turn_start` | after the owner's next `turnStart` the event body ran and the card filed |
| 1 | `g10_deferred_release_banks_no_crystal` | a release of arisa's own deferral adds 0 crystals (the `by_card_id` filter) |
| 1 | `g10_foreign_event_banks_one_crystal` | any non-self eventAfter adds exactly 1 |
| 1 | `g10_deferred_extra_draw_is_an_event_at_the_station` | after the release, an `event` is drawn for another seat with `tile == 流星堂`; bonsai gains a fire pot |
| 1 | `g10_deferred_record_survives_save_restore` | save mid-deferral, restore, turn start releases |
| 2 | `g10_settle_facts_carry_cancelled_to_tile_resolved` | counter-cancelled settle → `tileResolved` reads cancelled, no `settleAfter` |
| 2 | `g10_settle_facts_carry_paid_and_value` | rent settle → `paid` true, `paid_value` = the moved figure |
| 2 | `g10_door_tax_runs_after_a_body_replace` | Parking Space replaces the body → the tax still transfers |
| 2 | `g10_door_tax_skips_after_a_paid_rent` | ordinary rent → exactly one surcharge, not two |
| 2 | `g10_door_tax_on_the_start_tile` | settle on 梦开始的地方 → tax due, no rent |
| 2 | `g10_council_cleans_up_on_a_cancelled_settle` (A) | cancelled forced settle → card discarded and placement cost refunded |
| 3 | `g10_ownership_read_sees_the_deed_bought_by_the_move` | agent-tile offer buys a livehouse → the commit pass's read sees it and the face-mapped destination wins |
| 3 | `g10_journal_survives_nested_prompt` | one nested prompt → the world ends as if the move ran once, the deed and the settle exist once |
| 3 | `g10_journal_answers_do_not_shift` | a body prompt after the torn request gets its own answer |
| 3 | `g10_recomputed_request_reapplies_from_pre` | the second pass computes a different destination → the move re-applies from `pre` and the first application's effects are gone |
| 3 | `g10_unchanged_request_is_not_double_applied` | the second pass computes the same request → one move, one settle, one payment |
| 4 | `g10_exist_latch_needs_a_landed_redirect` | opponent's single-target effect redirected → latched, no draw at turn start |
| 4 | `g10_exist_latch_ignores_a_plain_rent_payment` | rent to the owner → not latched, draws |
| 4 | `g10_exist_latch_ignores_a_direct_target` | effect already naming the owner → not latched |
| 4 | `g10_exist_latch_follows_a_negated_redirect` (Q3) | redirect renamed, then the link is counteracted → the rename stands, latched, no draw |
| 5 | `g10_no_breakup_reverts_a_failed_teleport` | settle leaves a digit-less amount → position/money/owners restored, card discarded |
| 5 | `g10_no_breakup_keeps_a_successful_one` | repeated digit → no restore |
| 5 | `g10_world_restore_is_idempotent` | two restores of one key equal one |
| 5 | `g10_world_snap_survives_a_prompt` | snapshot, prompt, save/restore, restore still lands the pre-state |

Regression/adjacent: `effect_log_cards::be_strongest_*`,
`door_starts_on_ryuseido_and_walks_the_sequence`,
`ix_door_surcharge_on_rent`, `ppp::*` arisa tests, the settle-stage suite,
`zz_review_door_storm`, `effect_applied_log::reread_after_host_effect_sees_the_updated_value`.

### 9.2 Kani obligations (`rules/kani/src/proofs/{ppp,ras,sumimi,hostops}.rs` + `spec/`)

| proof | obligation |
|---|---|
| `g10_settle_facts_monotone` | cancel branch ⇒ `!body_replaced && !paid`; paid ⇒ body ran |
| `g10_settle_paid_only_own_tile` | a rent payment to another tile or another payee leaves `paid` false |
| `g10_redirect_fact_iff` | `redirected_from.is_some()` ⇔ the fold changed a live non-actor target, over a symbolic `to` |
| `g10_event_cause_propagates` | a cause stamped on `event` is on its `eventAfter` |
| `g10_defer_is_once` | another instance's second `event_defer` on the link answers 0 and pushes no record; the same instance's re-run answers 1 and pushes none |
| `g10_release_due_iff` | a record releases at a turn start ⇔ due owner and strictly later key, or owner out |
| `g10_snap_restore_iff` | `restore(snap(w))` equals `w` on the six families; restore twice equals once |
| `g10_journal_answer_layout` | after replay, every in-flight entry's answer sits at its own slot and no nested answer is orphaned |
| `g10_journal_apply_once` | an equal re-encounter applies zero more times; a changed one restores `pre` before applying |

Each new op also gets a `hostops_smoke` presence proof so an unmodeled op
cannot answer silently.

---

## 10. Alternatives

1. **Event deferral as a guest-held card (`IEventDefer` exactly as C#).**
   The card keeps a tuple and the engine calls `Resolved` on it. Rejected:
   the record must outlive the card (a save, a leave), so it belongs in the
   world; a guest-held record cannot be released after the instance is gone.
2. **Event deferral as a new `On::Schedule` entry.** `On::Schedule` already
   has an id and a due-turn mode. Rejected: a schedule is *card-owned
   future work* with the card as the owner; an event is an event-deck unit
   with a drawer and a tile, and the release must re-enter the event
   pipeline (`bind_event`, filing, `eventAfter`), which no schedule entry
   does.
3. **Settle outcome as one `settleOutcome` hook instead of facts.** One hook
   fired after the body carrying the bits. Rejected: both consumers also
   need the *tile/actor* payload `settleAfter` already gives, and a hook
   alone cannot serve a Filter or a guard inside `settleAfter`.
4. **HOST-01 as “rebase the snapshot only”.** Store the post-halt world as
   the resume snapshot. Rejected: the torn request's *own* answer slot is
   still wrong, so the guest skips the request on the first re-run; the world
   diverges permanently.
5. **HOST-01 as “roll back only”.** Restore the pre-request world and let the
   answer slot skip. Rejected: the request's effect is lost entirely.
6. **Redirect latch as a prop write from the Redirect Filter.** Would need
   `refuse_if_filter` lifted for `set_prop`. Rejected in G06-adjacent work:
   Filter bodies are pure by contract, and the latch wants to run on the
   settled declaration, not on the proposal.
7. **World snapshot as a second `turn_snap` variant.** Reuse the turn-start
   arrays. Rejected: turn-start arrays are stale mid-turn, and a second
   capture point on the player would collide with cards that already restore
   from it (`ran_as_usual`).
8. **`by_card_id` as a `String` card id.** Readable but not identity: two
   copies share the id. Rejected for the uid; the id rides as the dispatch
   key only.

---

## 11. Open questions

1. **「其他玩家」 for the extra draw.** The book does not name the seat. The
   design takes the next live seat after the owner; alternatives are the
   next seat in turn order at release time (same thing) or a chosen seat.
2. **Crystal timing on a deferred event.** Clause (1) banks at *resolution*,
   so a deferred event banks only when it resolves, and never for the card's
   own releases. Should a *negated* event still bank? Today's hook banks on
   `eventAfter` unconditionally; a `settle`-style `cancelled` fact on the
   event would be the precise answer (out of this group's surface).
3. **Redirect link cancelled but its rename stood.** `target_player` reads
   `r.target` regardless of `r.is_cancelled()`, so a counter on the redirect
   link cannot undo the rename. The fact mirrors the rename (influence
   happened). Is that the intended ruling, or should a cancelled redirect
   restore the declared seat?
4. **Deferral when the arisa card leaves.** V1 resolves the event anyway and
   skips the callback (no extra draw). Alternatives: resolve immediately on
   leave, or drop the event back to the discard. The C# `Card.Detach` note
   suggests retirement.
5. **Deferral when the owner leaves.** V1 releases at the first turn start
   after the leave, without the callback. Confirm.
6. **`no_breakup` and the main-move latch.** The C# snapshot does not
   include the move bookkeeping; a reverted teleport keeps the main move
   consumed. Confirm the book's 「视为你的主要移动」 stays consumed after
   「取消所有受到的效果」.
7. **`no_breakup` and cards played during the settle.** The six-family
   snapshot leaves hand/deck/marks/field untouched. Is 「取消所有受到的
   效果」 confined to the six families, or must a card that got played during
   the failed settle also come back?
8. **`settle.paid` when rent is replaced rather than paid** (a hook moved the
   rent into an abnormal or a force-buy). `paid` follows the money, not the
   clause; confirm.
9. **Journal depth.** The stack replay covers nested requests. Is a
   three-deep torn stack (a request inside a played card inside a request)
   in scope for the first implementation, or may it assert-and-fall-back to
   the current behaviour?
10. **Does `world_snap` belong on the instance uid only?** A card wanting two
    checkpoints must pass explicit keys; the instance drop cleanup then
    misses them. Confirm the convention.
11. **Re-applying a recomputed request.** §5.3 replaces an application when
    the body's fresh payload differs, which is what lets `be_strongest`
    correct its destination after the nested settle. That undoes and re-runs
    the first settle. Confirm the post-settle read is the book's intent, and
    that replace (not a second, cumulative application) is the right shape.
