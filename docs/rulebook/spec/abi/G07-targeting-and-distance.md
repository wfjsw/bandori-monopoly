# ABI design G07 — Targeting pipeline and distance

**Status:** draft for user review · design only, no source touched.
**Worktree:** `.claude/worktrees/card-rulebook-review` (branch `worktree-card-rulebook-review`).
**Group:** G07, family "Targeting pipeline and distance".
**Ground truth:** `docs/rulebook/cards.json`; every fragment below is verbatim from it.

Terminology: **designation** = one admission through the targeting pipeline
(`ctx::target` / `target_all` / `ctx::target_tile`, and the `[指定]` notion
behind 「成为…目标」). **Counteraction** = the [反击] mechanism (chain / window).
**Hook** = a passive listener. Never "reaction".

**Closed slugs and rows:**

| # | slug(s) | severity | rows | class |
|---|---|---|---|---|
| 1 | `target-count-pipeline` — merged `centrifugal-between-turns-counter`, `centrifugal-tile-targeting`, `welcome-self-target-pipeline` | medium | 274, 1037, 1061 (centrifugal), 1294 (welcome) | **demote to A** |
| 2 | `lisa-distance-exemption` (merged `lisa-bond-exemption-gap`) | medium | 691, 1451 | B |

Item 1 is **demoted to A**: every sub-item is a host-side pipeline fix on the
existing `target` import plus one SDK sugar wrapper — no manifest field, no
import signature change. Item 2 needs new capability (a durable per-instance
grant and a host-side nearest query) and stays B. The batch bump is carried by
item 2 only.

---

## 1. Problem, per item

### 1.1 `target-count-pipeline` (rows 274, 1037, 1061, 1294)

Book (`Mor:离心力，不为所动`):

> [反击]当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时，你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果

Three separate facts are wrong around one counter.

**(a) `centrifugal-between-turns-counter`.** The gate is
`targeted_count(owner) >= 2` (`centrifugal.rs:38`). The counter is zeroed only
at the owner's **own turn start** (`play.rs:580-589`, the `turn_start` entry)
and every `target_player` increments it (`wasm_rules.rs:5716-5725`). A targeting
during the owner's own turn therefore counts toward the window, and a second
one inside the same turn offers the card although the book only counts
「两个你的回合之间（回合1结束，回合2开始）」. The in-file TODO at
`centrifugal.rs:29-32` records it. `docs/rulebook/spec/cards/mor.md:204-206`
reads the same clause as "since the end of the user's previous turn and before
the start of their next".

**(b) `centrifugal-tile-targeting`.** `target_tile` (`wasm_rules.rs:5745-5783`)
does open the `targeted` / `effect` / `target` raises for the owner
(`target_window` at `:5775`), so 「成为…目标」 is already true at the card gate —
but it never touches `targeted` (the only increment is at `:5718-5725` inside
`target_player`). The same event that opens the window does not advance
「第二次」; the card can only ever be offered off a player-target count. Same
TODO, `centrifugal.rs:34-37`.

**(c) `welcome-self-target-pipeline`.** Book (`Mujica:欢迎来到ave mujica的世界`,
clause (1)):

> 转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）；

「任意一名玩家」 includes the user, and the body already offers the user first
(`welcome_mujica.rs:43-45`). The pick then calls `ctx::target(who)`
(`:66`) — but `target_player` answers at `p == by` (`wasm_rules.rs:5701-5704`)
immediately after the `redirect` hook. A self designation therefore skips
`Untargetable`, the count, the whole `target` window (`targeted` hooks, the
`effect` [反击] chain, the `target` settlement hook) and `ImmuneAll`. The
in-file `TODO(ABI)` is at `welcome_mujica.rs:61-65`; the same shape hits
`rain.rs:41` (`通用:雨`, which designates the user first by rule) and any
`declare_war` counter played against the owner's own link.

Note the exact current order, because it bounds the fix: the per-pair cancel
(`:5664`), `OnBoard` (`:5667`) and `redirect` (`:5681-5700`) already run for a
self pick; only the early return's *tail* is missing. A `redirect` that re-names
onto a third seat already escapes the short-circuit and runs the full pipeline —
the early return is reachable only when the final name is still `by`.

### 1.2 `lisa-distance-exemption` (rows 691, 1451)

Book (`R:必然的联系（莉莎）`):

> 【反击】当你使用技能进行传送后，你可以打出此卡并指定一个和你在同一地块的角色，将此卡放在对方的游戏区，你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆。

The placement half works: the counter designates a co-located player through
the full gate and, on success, `ctx::place_card_at(hit, ID, …)`
(`lisa_bond.rs:63-68`). The exemption half is inert. Grep over `crates/` and
`rules/` finds 「距离最近」 only in this file; no skill consults the placed
instance. The only live 「距离最近」 user is 今井莉莎's own skill 2
(「传送到距离你正向距离最近的角色所在的格子」, `lisa.rs:71-90`), and it
computes the nearest with an inline loop (`:74-87`) that cannot see the card.
The TODO is at `lisa_bond.rs:76-78`; recorded as BACKLOG ABI-10
(`docs/rulebook/BACKLOG.md:827-836`).

The blocker is that the placed instance cannot even say **who** granted it: `World::place_card` sets `owner = user = player_id`
(`ops.rs:1230-1242`), i.e. both fields are the *designated* player. C#'s
`PlaceCard(owner = r.index, user = c.Seat)` keeps the caster in `user`; the
Rust port drops it. No player decision is missing — only a durable grantor
fact and a query the skill can call.

---

## 2. Capability as an invariant

> **Every designation — a player target, a self-designation, and a
> tile-targeting — enters one pipeline and is counted exactly where the sheet
> counts it.** The pipeline is: the per-pair cancel check, the board gate,
> declaration re-naming (`redirect`, single player targets only),
> `Untargetable`, the between-turns count **when the named seat is not the
> actor**, the `targeted` hook / `effect` [反击] chain / `target` hook, and —
> for other-source designations only — `ImmuneAll`. A designation that was
> **named** counts even when a later step (the window, `ImmuneAll`) negates it.
> A designation of nobody (an unowned tile) names nobody.
>
> **The between-turns counter advances only while the named seat's own turn is
> not the current turn**, and is zeroed at that seat's own turn start.
>
> **The nearest-by-distance predicate is a host query**, returning the whole
> tied set (never a first-found seat), with the waiver set for skill targeting
> as a second query. A waiver is a durable fact on a placed instance, readable
> by the grantor seat.

The three parts are one invariant: a pipeline that is skipped for self and a
count that is advanced outside the sheet's window make each other wrong; a
nearest predicate that cannot be extended by a grant is the same hole one level
up.

---

## 3. Surface

Everything is generic mechanism: no display text, no player decisions inside
imports. Nothing here names a card.

### 3.1 `card_sdk::ctx` — pipeline wrappers (item 1, no wire change)

```rust
/// Target **yourself** through the full pipeline (「指定」 yourself).
/// `Some(hit)` is the seat actually named — a `redirect` may still move it
/// elsewhere; `None` when the designation was refused or cancelled.
/// Sugar over [`target`]; the engine path is identical.
#[cfg_attr(kani, inline(always))]
pub fn target_self(player_id: i32) -> Option<i32> {
    target(player_id)
}
```

`sys::target(player_id, tile, single)` keeps its three arguments and its wire
signature. The caster is `call.player_id()`, already what `apply_host_request`
passes as `by` (`wasm_rules.rs:3770-3781`).

### 3.2 `card_sdk::abi` — distance metric and the waiver property (item 2)

```rust
/// How the 「最近」 predicate measures distance. Wire `i32`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub enum Nearest {
    /// Steps strictly ahead around the ring. Co-located (`0`) is **not**
    /// ahead and is not a forward candidate.
    #[default]
    Forward = 0,
    /// Undirected ring distance. Co-located is distance `0` and is the nearest.
    Absolute = 1,
}
```

```rust
pub mod prop {
    // …existing keys…
    /// 「你对对方使用技能将可以无视【距离最近】」: on a **placed instance's**
    /// `props`, the value is the **grantor** seat whose skills may ignore the
    /// nearest limit when targeting this instance's holder. Written at runtime
    /// (like `RENT_HOUSES`), gone with the instance. Presence is the fact, so
    /// the read is `props.get` — seat `0` is a valid grantor.
    pub const NEAREST_EXEMPT: &str = "nearestExempt";
}
```

### 3.3 `card_sdk::ctx` — nearest queries and waiver writes (item 2)

```rust
/// The nearest other present player(s) by `metric`, seat order on ties.
/// `Forward` excludes co-located seats; `Absolute` includes them.
pub fn nearest(by: i32, metric: crate::abi::Nearest) -> Vec<i32>;

/// `nearest`, plus every present seat holding an instance whose
/// `prop::NEAREST_EXEMPT` is `by` (the pair's 「距离最近」 waiver), in
/// distance-then-seat order, deduplicated. The raw nearest set stays first.
/// This is the candidate list of a 「距离最近」 **skill**.
pub fn nearest_skill_targets(by: i32, metric: crate::abi::Nearest) -> Vec<i32>;

/// Does `candidate` hold an instance granting `by` the waiver?
/// (Per-pair form for a skill that already has its candidate.)
pub fn nearest_exempt(by: i32, candidate: i32) -> bool;

/// Mark the instance at `uid` as granting `by` the waiver on its holder.
/// `ctx::set_prop_at(uid, prop::NEAREST_EXEMPT, by)`; returns the stored seat.
pub fn grant_nearest_exempt(uid: i32, by: i32) -> i32;
```

All three are reads except `grant_nearest_exempt`, which is a wrapper over the
existing `set_prop_at` import — no new write op.

### 3.4 Imports (`card_sdk::ctx::sys`, host `hostfns`, `World` trait)

```rust
// ctx/sys.rs
#[link_name = "ruleset_nearest"]
pub fn nearest(by: i32, metric: i32, skill: i32, buf: i32, cap: i32) -> i32; // postcard Vec<i32>
#[link_name = "ruleset_nearest_exempt"]
pub fn nearest_exempt(by: i32, candidate: i32) -> i32;

// game-rules/src/world.rs (CardWorld trait, next to `dist` (`:427`) and
// `tile_forward` (`:429`) -- these are world measures, not rules data)
/// Nearest present seats for `by` under `metric`; `skill` adds the waiver set.
fn nearest(&self, by: i32, metric: i32, skill: bool) -> Vec<i32>;
fn nearest_exempt(&self, by: i32, candidate: i32) -> bool;
```

`nearest` uses the `designations` out-buffer shape (postcard, `ctx::out_buf`,
cap 4096 — a body of 4–6 players can never approach it). `ctx.rs:14` adds
`Nearest` beside `AbKind, CardPile`; without the re-export `ctx::Nearest` does
not resolve in a card body.

### 3.5 Not changed

`ctx::designations` / `DESIGNATES` / `designation_cancelled` keep their
meaning: `designations` is the play's **static declared recipient set** (a
prop-driven list `others(actor)`), not a record of pipeline admissions, and a
per-pair `cancel_designation` is still checked at stage 2 of both pipelines.
`ctx::dist` / `ctx::tile_forward` stay the raw measure; `nearest` is the
selection built on them.

### 3.6 Classification

| item | NEW | EXTENDED | WIRE-BREAK | NO-WIRE |
|---|---|---|---|---|
| 1 | `ctx::target_self` (sugar) | `target_player` / `target_tile` stage order; `targeted_count` doc + semantics | — | **yes** |
| 2 | `Nearest`, `prop::NEAREST_EXEMPT`, 2 imports, 3 wrappers | `CardWorld` trait, `World` impl | new op ids (append-only) | — |

Two new append-only op ids are the only wire change; the `ManifestOn` /
`Trigger` / `HostRequest::Target` shapes are untouched.

---

## 4. Engine wiring

### 4.1 `target_player` — stages (items 1a, 1c)

`wasm_rules.rs:5654-5738`. Replace the early return with "the tail is not
skipped"; the stage list becomes:

| # | stage | today | after |
|---|---|---|---|
| 1 | seat / out check | runs | unchanged |
| 2 | per-pair cancel | runs | unchanged |
| 3 | `OnBoard` gate | runs | unchanged |
| 4 | `redirect` (single only) | runs | unchanged |
| 5 | `Untargetable` | **skipped when `p == by`** | runs |
| 6 | count increment | **skipped when `p == by`** | runs iff `p != by` |
| 7 | `target_window` (`targeted` hook → `effect` chain → `target` hook) | **skipped when `p == by`** | runs |
| 8 | `ImmuneAll` | **skipped when `p == by`** | runs iff `p != by` |
| 9 | answer `p` | — | unchanged |

```rust
// wasm_rules.rs:5701-5705, before → after
-        // A self-target no redirect moved (EXIST's 「包括其他玩家指向自身的卡」
-        // included): already named, nothing to chain.
-        if p == by {
-            return Ok(p);
-        }
         // `Untargetable` governs who may be *named*. …
```

The count at `:5716-5725` gains the actor guard and keeps its position (after
naming, before the window):

```rust
if p != by {
    let mut w = cx.world_copy();
    if w.targeted.len() <= s { w.targeted.resize(s + 1, 0); }
    // 「两个你的回合之间」: the seat's own turn is not the window.
    if w.st.turn != p {
        w.targeted[s] += 1;
    }
    cx.swap_world(w);
}
```

The immune call at `:5734` becomes `if p != by && self.immune(cx, p, by)?`, and
the **window** call at `:5728` loses its actor guard, `if p != by &&
!self.target_window(...)` -> `if !self.target_window(...)`: with the early
return deleted, stage 7 is exactly what lets a self pick be redirected.

`ctx::target_all` (`ctx.rs:2792-2801`) needs no code change: it calls
`sys::target(s, -1, 0)` per seat, so a list containing the actor now pipelines
that seat instead of short-circuiting, and still dedupes the answers.

**Why skip `ImmuneAll` for self:** the engine already treats `by ==
player_id` as "your own effect" everywhere else (the abnormal gate `:5616`,
and its self spellings `:5553`, `:5579`), and `mor.md:222` / OPEN-无效化 leave the shield's scope
open. A self designation must still be *named* (so the count for a redirected
other-source designation and the window are right), but the owner's own shield
must not eat it. A resolver may later rule the other way; the call sites
(`:5734`, `:5778`) are one condition each, matching the abnormal gate
(`:5616`) and the payment gate (`:4392`, `x != call.player_id()`).

### 4.2 `target_tile` — stages (items 1b, 1c)

`wasm_rules.rs:5745-5783`. The `NO_TARGET` check and the unowned-tile
no-op stay. Inside the owner branch:

| # | stage | today | after |
|---|---|---|---|
| 1 | owner live check (`owner >= 0 && live`) | runs | unchanged |
| 2 | `Untargetable` on the owner | only when `owner != by` | always (branch entered) |
| 3 | count | **never** | increment iff `owner != by` |
| 4 | `target_window` | only when `owner != by` | always (branch entered) |
| 5 | `ImmuneAll` | only when `owner != by` | iff `owner != by` |

The outer `if owner >= 0 && owner != by && live` becomes `if owner >= 0 &&
live`, with the two `by` guards moved inside (stages 3 and 5). `tile` still
rides the `targeted` / `effect` / `target` triggers (`t.tile = tile`).

### 4.3 Count lifecycle

| event | `targeted[seat]` |
|---|---|
| seat's own `turn_start` (`play.rs:580-589`) | `= 0` (unchanged line) |
| player designation named, `p != by`, `st.turn != p` | `+= 1` |
| tile designation named, live other owner | `+= 1` |
| self designation (`p == by` / own tile) | unchanged |
| designation during the seat's own turn | unchanged |
| a later counteraction negates the window | already counted (named) |
| unowned / `NO_TARGET` tile | unchanged |

`st.turn` is the turn player (`play.rs:547`, `-1` before the match); the
predicate "not the seat's own turn" is exactly "between that seat's turn end
and its next turn start" for every targeting that can reach the counter. No new
saved field and no new reset point — see §7.

The group's `expected` says "resets at the owner's turn end". Keeping the reset
at turn start and gating the increment on `st.turn != p` **observably
implements** that: the value carried into a turn is zeroed at its start, no
own-turn targeting enters the count, and the count that the following
between-turns window accumulates starts from 0 and holds only between-turns
designations (§9 A1).

### 4.4 `ctx::targeted_count` consumers

The cond native function (`cond/src/vocab.rs:960-976`, `view.rs:212-214`,
`ctx.rs:51`) keeps its name and `i64` shape; only the doc line changes to
"designations of `p` since `p`'s turn ended (not counted during `p`'s turn)".
No window var is added. `designates` / `DESIGNATES` (the static recipient set)
and `designation_cancelled` are untouched — see §3.5.

### 4.5 Nearest query (`World::nearest`)

```rust
fn nearest(&self, by: i32, metric: i32, skill: bool) -> Vec<i32> {
    let at = self.player_pos(by);
    let mut best = i32::MAX;
    let mut raw = Vec::new();
    for p in self.others_at(by) {
        let d = if metric == 1 { self.dist(at, self.player_pos(p)) }
                else { self.tile_forward(at, self.player_pos(p)) };
        if metric == 0 && d == 0 { continue; }        // co-located is not ahead
        if d < best { best = d; raw.clear(); raw.push(p); }
        else if d == best { raw.push(p); }
    }
    if !skill { return raw; }
    // The waiver set, distance-then-seat ordered, appended after the raw set.
    let mut extra: Vec<i32> = self.others_at(by)
        .into_iter()
        .filter(|&p| !raw.contains(&p) && self.nearest_exempt(by, p))
        .collect();
    extra.sort_by_key(|&p| (self.dist(at, self.player_pos(p)), p));
    raw.extend(extra);
    raw
}
```

`nearest_exempt(by, candidate)` scans `candidate`'s field for any instance
whose `props.get(prop::NEAREST_EXEMPT) == Some(&by)` (`get`, not
`unwrap_or(0)`). Both fields come from `FieldCard` (`state.rs:1096-1131`):
`props` carries the grant, the field array is the holder. No board scan and no
per-play cache — the set changes only on placement/removal.

### 4.6 Kani model

* `kani_host::Op` gains two append-only ids: `nearest = 401`,
  `nearest_exempt = 402` (340 is the current maximum; 401-410 is this group's
  README band).
* `sys_kani.rs` + `native_shims.rs` (`ruleset_nearest`,
  `ruleset_nearest_exempt`) + `host.rs` registration mirror the two imports.
* `rules/kani/src/host.rs`:
  * `arm_target` models the new stages: actor = `w.trig_player` (the harness
    convention already used by every proof), tile branch (`a1 >= 0`) resolves
    `w.tiles[a1].owner`, the count becomes
    `p != actor && p != w.turn_player`, self passes and counts nothing.
  * `Op::targeted_count` moves off the `arm_stub_zero` arm to a real reader of
    `w.targeted_count[p]`.
  * `arm_nearest` computes the §4.5 set over `w.pos` / `w.others`; the
    `NEAREST_EXEMPT` name is pre-interned in `Known`; `arm_nearest_exempt`
    scans `w.field[..w.field_n]` (`f.player == candidate && f.props.get(key) == by`).
* `rules/kani/tests/conf_model.rs` + `conf_fill_diff.rs`: `targeted_count`
  / `target` fixtures stop being "always 0" (`conf_fill_diff.rs:73` gap note
  drops).
* The exact rules are proved on pure functions in
  `rules/kani/src/spec/abi_g07.rs` (§8 obligations), so the stubs only need
  shape consistency.

---

## 5. Card usage

### 5.1 `Mor:离心力，不为所动` — no body change

**Before / after:** identical code. What changes is that the gate is now
truthful: own-turn targetings no longer inflate the count, tile targetings
advance it, and a self designation neither advances it nor is offered against
it (`by != owner`). Replace the two TODOs at `centrifugal.rs:29-37` with one
citation of the window clause; the `>= 2` comment keeps C-MOR-centrifuge-01's
"second (or later)" reading (`mor.md:204-206`).

### 5.2 `Mujica:欢迎来到ave mujica的世界` — comment only

**Before** (`welcome_mujica.rs:55-68`): `ctx::target(who)` with the TODO saying
a self pick skips the tail. **After:** same call; delete the `TODO(ABI)` and
state that `ctx::target` is the full pipeline for both branches (the card may
use `ctx::target_self` only for readability, but the seat is a variable here).

### 5.3 `通用:雨` / `AG:宣战布告` — no change

`rain.rs:41` uses `ctx::target(player_id)`; it now runs the row's window like
every other designation (a counter may cancel the self designation via
`cancel_designation`, and its own `Untargetable` gate is asked with `by == p`,
which its condition excludes). `declare_war.rs:92` targets the caster of the
answered card; a self-counter now pipelines instead of short-circuiting.

### 5.4 `R:必然的联系（莉莎）`

**Before** (`lisa_bond.rs:63-75`):

```rust
if let Some(who) = hit {
    ctx::place_card_at(who, "R:必然的联系（莉莎）", &Msg::new(key!("lisa_bond_note")));
    ctx::log(…);
}
```

**After:**

```rust
if let Some(who) = hit {
    let uid = ctx::place_card_at(
        who,
        "R:必然的联系（莉莎）",
        &Msg::new(key!("lisa_bond_note")).player_id("who", player_id),
    );
    if uid > 0 {
        // 规则书: 「你对对方使用技能将可以无视【距离最近】这一限制」
        ctx::grant_nearest_exempt(uid, player_id);
    }
    ctx::log(…);
}
```

The grant rides the placed instance: leaving the field, or the instance being
destroyed, ends it. A refused target gate places nothing and grants nothing.

### 5.5 `skill:今井莉莎:慈爱女神` (2)

**Before** (`lisa.rs:74-89`): inline `tile_forward` loop, first strictly
smaller distance wins, no waiver. **After:**

```rust
// 规则书: 「传送到距离你正向距离最近的角色所在的格子」
let candidates = ctx::nearest_skill_targets(player_id, ctx::Nearest::Forward);
let to = match candidates.len() {
    0 => return Ok(()),
    1 => ctx::player_pos(candidates[0]),
    _ => ctx::player_pos(ctx::ask_player(
        player_id,
        &Msg::new(key!("lisa_title")),
        &Msg::new(key!("lisa_ask")),
        &candidates,
    )?),
};
```

The ask only appears when the waiver (or a tie) makes the set non-singleton, so
the no-waiver path is unchanged. `nearest` (raw) stays available for the four
non-skill 「最近」 loops (`my_own_problem.rs:44-59`, `kaoru.rs:127-137`,
`nfo.rs:81-98`, `surprise_duel.rs:65-84`); migrating them is optional (§10).

---

## 6. Save / record / replay

* **SAVE_VERSION stays 16.** No saved field is added: `World.targeted` already
  exists with `#[serde(default)]` (`engine/world.rs:661-665`), and the window
  is derived from `st.turn`, which is saved. The exemption is a `props` entry
  on an already-saved `FieldCard` (`state.rs:1111-1116`). An earlier save loads and
  behaves correctly; the count's meaning changes but its storage does not.
* **RECORD_VERSION stays 3.** `HostRequest::Target` keeps
  `{ player_id, tile, single }` (`host.rs:413-421`); the record stores the same
  answers. Old `.bdrec` files replay on their archived engine bundle and keep
  the old pipeline; nothing in the record needs rewriting.
* **Replay determinism.** The pipeline gains no new host prompt and no new
  random draw; the added stages are pure reads and raises. A replay recorded
  after the change re-runs the same raise sequence. The `nearest` read is
  deterministic (seat order on ties); `ask_player` is an existing recorded
  prompt.
* **ABI_VERSION** (§7) is the only stamp that moves; `save_version` in an
  `EngineStamp` is unchanged.

---

## 7. Versioning

`ABI_VERSION` 111 → **112** (ninth group in landing order). 104 is already taken in the working tree by the
in-flight `turn_snap` change (`abi.rs:845`); if another group in the batch
lands 105 first, renumber to the next free integer — the entry text is the
contract. Entry inserted immediately below the constant, above the newest note (the tree keeps notes newest-first):

```
/// v112: **designation pipeline and distance queries**. `ctx::target` now
///    runs a self-designation (`p == by`) through `Untargetable`, the
///    between-turns count and the `target` [反击] window, and skips only the
///    count and `ImmuneAll` (own-source); `ctx::target_tile` likewise runs the
///    window for an own tile and counts a live other owner's tile. The
///    `targeted_count` counter advances only while the seat's own turn is not
///    current (「两个你的回合之间」) and is zeroed at its own turn start; a
///    named-but-negated designation still counts. New `ctx::target_self`
///    sugar. New imports `ruleset_nearest` / `ruleset_nearest_exempt`
///    (`ctx::nearest`, `ctx::nearest_skill_targets`, `ctx::nearest_exempt`,
///    `abi::Nearest::{Forward,Absolute}`) and the runtime-written
///    `prop::NEAREST_EXEMPT` (`ctx::grant_nearest_exempt` sugar over
///    `set_prop_at`). SAVE_VERSION / RECORD_VERSION unchanged.
```

---

## 8. Tests and Kani obligations

### 8.1 Per item (`rules/tests/tests/`)

| item | test | assertion |
|---|---|---|
| 1a | `g07_centrifugal_own_turn_targeting_is_not_counted` | 宣战布告 twice inside the owner's own turn → not offered; after their turn ends one more targeting → offered |
| 1a | `g07_centrifugal_second_between_turns_targeting_offers` | two targetings during another player's turn → offered on the second |
| 1a | `g07_centrifugal_window_closes_at_own_turn_start` | one between-turns targeting, own turn starts, then a targeting in the own turn → no offer |
| 1a | `g07_centrifugal_count_is_per_seat` | two seats targeted during the same turn keep separate counts |
| 1b | `g07_centrifugal_tile_targeting_counts` | ONE OF US's `target_tile` on the owner's tile between turns bumps `targeted_count(owner)` by 1 |
| 1b | `g07_centrifugal_tile_targeting_offers` | the second tile hit between turns offers the card; the window carries `tile >= 0` |
| 1b | `g07_centrifugal_own_tile_does_not_count` | targeting the owner's own tile (owner == by) counts 0 but still raises `targeted` |
| 1c | `g07_welcome_self_target_runs_the_target_window` | an opponent's counter answers a self pick (the `effect` window exists with `target == by == user`) |
| 1c | `g07_welcome_self_target_skips_immunity` | under 离心力's shield the self toggle still lands (own source) |
| 1c | `g07_welcome_self_target_can_be_cancelled` | `cancel_designation(user)` in the window cancels the self designation |
| 1c | `g07_rain_self_designation_uses_the_pipeline` | the 雨 self pick raises the window and is not counted until an opponent answers |
| 2 | `g07_nearest_forward_matches_the_legacy_loop` | a fixture board: `nearest(Forward)` == the pre-change `lisa` loop result, seat order |
| 2 | `g07_nearest_forward_excludes_co_located` / `g07_nearest_absolute_includes_co_located` | the two metrics' boundary |
| 2 | `g07_nearest_absolute_returns_all_ties` | two seats at the same minimal distance are both returned |
| 2 | `g07_lisa_bond_grants_the_waiver` | place, then `nearest_skill_targets(Forward)` of the placer contains the designated co-located seat |
| 2 | `g07_lisa_bond_waiver_is_per_grantor` | `nearest_exempt(other, designated)` is false |
| 2 | `g07_lisa_bond_waiver_dies_with_the_instance` | sweep the placed instance to the discard → no longer exempt |
| 2 | `g07_lisa_bond_refused_gate_grants_nothing` | 夏日合宿 on the candidate, another-source refusal → no instance, `nearest_exempt` false |
| 2 | `g07_lisa_goddess_offers_the_designated_seat` | with the waiver, the press prompts and can teleport to the co-located seat |
| 2 | `g07_lisa_goddess_no_prompt_without_waiver` | plain board → single nearest, no prompt (regression) |

Existing tests that must stay green: `welcome_offers_self_as_well`,
`welcome_switches_designated_state`, `rain_*`, `declare_war_*`,
`cross_chain::c12_exist_vs_summer_camp`, `teleport_move::*`,
`roselia.rs:638` (lisa skill forward-nearest), `polyglot` save round-trips.

### 8.2 Kani obligations

`rules/kani/src/spec/abi_g07.rs` models the pure rules; `proofs/abi_g07.rs`
proves them:

* `pipeline_stages_iff` — for arbitrary `(by, p, single)` the stage set is
  exactly §4.1's; `p == by` **includes** `UNTARGETABLE | WINDOW` and
  **excludes** `COUNT | IMMUNE`. A mutant restoring the early return fails.
* `targeted_increment_iff` — the player increment is exactly
  `p != by && p != turn`; a mutant dropping the turn guard fails (the current
  bug), one counting self fails.
* `tile_increment_iff` — exactly `owner >= 0 && owner != by`; a mutant keeping
  the old "never" fails, one counting own tiles fails.
* `nearest_iff` — the returned set is exactly every present non-`by` seat at
  the minimal metric distance, seat order; mutants "first only" and "all
  candidates" fail.
* `nearest_skill_iff` — exactly `raw ∪ exempt` in distance-then-seat order; a
  mutant dropping exemptions fails, one admitting a non-exempt farther seat
  fails.
* `exempt_scan_iff` — true iff some instance of the candidate carries
  `props[NEAREST_EXEMPT] == by`; a mutant matching any grantor fails.
* `exempt_dies_with_instance` — after `destroy_instance_units` the scan is
  false.

Card bodies stay PROTOCOL-LEVEL: `kani/mor.md:56` keeps the centrifuge rows
PROTOCOL-LEVEL (the window is the protocol), and the count becomes a modelled
host query; `kani/mujica.md:44` loses the BLOCKED-HOST note (the `Op::target`
model now admits); `kani/r.md` gains a LISA-BOND-EXEMPT row for the grant.

---

## 9. Alternatives

**A1 — a separate `targeted_between` counter zeroed at turn end.** Matches the
group's first fix hint literally, but needs a new saved field and a reset point
inside the end sequence that is safe across the early out/ended branches, and it
still needs the "not your turn" guard (otherwise an own-turn targeting increments
it before the end). Rejected; the `st.turn` predicate is the same behaviour
with no state.

**A2 — a dedicated `ctx::target_self` import that opens only the window.**
Cheaper than the full pipeline, but then self designations still skip
`Untargetable` (matters for a redirect back onto a shielded seat) and the
stages diverge per call kind — exactly the drift this item is about. Kept as
sugar over the common path instead.

**A3 — exemption as per-player state (`state::set(candidate, "nearestExemptBy:<by>", 1)`).**
Survives the card leaving the field, but needs dynamic-key cleanup, and two
copies on one player collapse. Rejected: the placed instance already exists and
already dies at the right moment.

**A4 — exemption as a static `CardDef` prop.** `place_card_at` stamps static
props from the card definition, so the *grantor seat* — which is not a card
constant — cannot ride a static prop. Would need the `user` field instead;
rejected only because `props` avoids changing `place_card_at`'s signature.

**A5 — keep the inline nearest loops and give them a `ctx::nearest_exempt`
check.** One new import only, but every 「最近」 card must re-derive ties and
order, and lisa's skill would still be the only gate. Rejected; the host query
is the invariant.

**A6 — a single-seat `ctx::nearest`.** Simpler signature, but loses ties
(kaoru/my_own_problem already enumerate ties) and cannot express
nearest-plus-waiver. Rejected.

---

## 10. Open questions

1. **Own-turn-end boundary.** A targeting that answers a link raised during the
   seat's own `turnEndAfter` (structurally after the turn "ended", but
   `st.turn` is still that seat) does not count. Is `st.turn != p` the right
   cut, or must `endTurnAfter` already count?
2. **Second offer.** `>= 2` keeps offering on the third and later
   between-turns designation (`mor.md:204-206` says "second (or later)"). Does
   the book's 「第二次」 allow a later re-offer if the second was declined?
3. **DESIGNATES plays.** A card that declares its recipient set via
   `prop::DESIGNATES` (「[指定]…所有玩家」) names players without a `ctx::target`
   call, so it never advances the count. Does such a declaration count as
   「成为目标」? Design: no — only pipeline admissions count (OPEN-指定).
4. **Negated targetings count.** A designation refused by `Untargetable` does
   not count (never named); one named and then negated by the window does.
   Confirm the distinction is 「成为目标」 vs 「受到效果」.
5. **Waiver lifetime.** The sheet names no end for 「无视【距离最近】」; the spec
   reads it as live while the effect is live (`r.md:509-515`). But the same
   sentence says 「此卡进入弃牌堆」 after activation — if the placed instance is
   later made to discard, the waiver dies immediately. Design: instance-bound.
6. **Skills only.** The waiver text says 「使用**技能**」; cards keep the raw
   `nearest`. Confirm a waiver never widens a card effect's nearest pick.
7. **Ties.** Is the 「距离最前/最近」 limit satisfied by the whole tied set (the
   design's ask), or is the limit only "distance equal to the minimum" with the
   player free to pick? Same outcome for the query; the prompt only exists with
   a waiver or a tie.
8. **Exiled seats.** `others` includes exiled (not out) seats; a waiver holder
   who is exiled still counts as a candidate. Intended?
9. **Other nearest loops.** Should `my_own_problem`, `kaoru`, `nfo`,
   `surprise_duel` migrate to `ctx::nearest` in the same batch, or after?
