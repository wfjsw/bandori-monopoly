# G07 — Targeting pipeline and distance

Status: **draft for review, design only** (2026-11-03 revision of the
2026-10-27 draft). No source is edited by this document.
Worktree `/home/monopoly/web/.claude/worktrees/g07-design-review`, branch
`worktree-g07-design-review`. Fits **on top of ABI v112** (G01, G08, G02, G04,
G03, G05 and G06 have landed); it does not redo them.
Group: **G07**. Family: the designation pipeline (player, self and tile
targeting), the between-turns designation counter, and the 「距离最近」
predicate with its per-pair waiver.

Names are spelled out (G03 R25's rule, G06's header): a **designation** is one
admission through the targeting pipeline; **counteraction** is the [反击]
mechanism; a **hook** is a passive listener; the engine-side carrier of one
raise is a **trigger**, its module-side mirror a **link**. The distance metric
is `DistanceMetric` with the values `Forward` (clockwise steps, the book's
「正向」) and `Absolute` (undirected ring distance, the book's 「绝对距离」);
the waiver is `nearest_exemption`, never “exempt”.

Ground truth is `docs/rulebook/cards.json`; every quoted fragment below was
re-read there and is identified by its card id beside it. The **row ids are
the triage sheet rows** — the same ids `BACKLOG.md` section B uses.

Reading order: `docs/GUARDS.md` §4.2 (the window vocabulary), G06 §4.1 (the
`effect`/`target`/`targeted` stage order this group slots into) and §6.1
(the kind contract), G12 §5.1 (the immunity gate), `docs/rulebook/spec/cards/
mor.md:191-224` (C-MOR-centrifuge-01/02), `docs/rulebook/spec/cards/r.md:
465-523` (C-R-必然的联系-01..04) and `:987-1040` (C-R-慈爱女神-02/03).

## 0. Review record

This revision came out of a full review of the 2026-10-27 draft. Every cited
site was re-read against master, every claim about the engine was re-derived
from the code, and the band arithmetic was re-run from the groups that landed
before this one. What the draft got wrong:

| # | Draft defect | Correction |
|---|---|---|
| R1 | The header said the document was written in worktree `card-rulebook-review` and "fits on top of ABI v111"; G06 has since landed, so master is ABI 112 / SAVE 16 / RECORD 4 / the last Kani `Op` 432. | Header and §11 carry the real base and landing numbers; the README's G07 row (112→113, Op 441–450) is honoured. |
| R2 | Nearly every engine citation was stale by 300–1000 lines (`wasm_rules.rs:5654-5738` for `target_player`, `:5745-5783` for `target_tile`, `play.rs:580-589` for the reset, `world.rs:661-665` for `targeted`, `abi.rs:845` for the version entry, `ctx.rs:14` for the re-export). | Every site is re-cited against master: `target_player` 6730-6831, `target_tile` 6832-6873, the reset `play.rs:719-728`, `World.targeted` `world.rs:897-900`, the version block `abi.rs:840-863`, the re-export `ctx.rs:14`. |
| R3 | §4.3's tile row said "increment iff `owner != by`" while the invariant and the next table also demanded the between-turns window. | §7.3 is one `note_designation` helper used by both entry points; the predicate is exactly `named != by_player && named != st.turn` for players and tiles alike. |
| R4 | §6 said `RECORD_VERSION` stays 3; G06 landed it at 4. | §9 keeps RECORD_VERSION 4. |
| R5 | §4.6 said "G03 takes 341-410 ahead of this group", ignoring G06's 411–426 and the landed G05/G13 ops 431–440. | The ops are 441–442 (§11); the Kani `Op` enum now ends at `invoke_frame = 432`. |
| R6 | §3.2 called the exemption `NEAREST_EXEMPT` / `nearest_exempt` / `grant_nearest_exempt` — three abbreviations of one word. | `prop::NEAREST_EXEMPTION`, `ctx::nearest_exemption`, `ctx::grant_nearest_exemption` (§5.3, §5.4). |
| R7 | §3.2's enum was named `Nearest`, which collides with the query function `nearest` and does not say what it is. | `DistanceMetric`; the values are the book's own words `Forward` / `Absolute` (§5.2). |
| R8 | §4.5's pseudo-code called `self.others_at(by)` as if it were an iterator and called a `World` measure the engine does not have (the ring length lives on `GameData`). | `nearest` is a `CardWorld` method on `Run`, which owns both the board data and the world; it iterates `self.world.others(by)` and measures with `self.dist` / `self.tile_forward` (§7.5). |
| R9 | §4.5 ignored `nearest_exemption`'s presence read: the Kani `NameMap::get` cannot distinguish a grant by seat 0 from no grant. | The scan is `props.get(key) == Some(&by)` (engine) / `get_opt` (Kani), never `unwrap_or(0)` (§7.6, §8.4). |
| R10 | §5.5 kept the skill's first-found loop as the expected result, so the regression test could not distinguish "all ties" from "first only". | The test compares `nearest(Forward)[0]` with the old loop and separately asserts the whole tied set (§10.1). |
| R11 | §5.4's `uid > 0` guard is right but unexplained; `place_card_at` answers `-1` on failure and uids start at 1 (`state.rs:274`). | §8.4 states the guard and the grant order; a refused gate places nothing. |
| R12 | §5.3 did not say that `NEAREST_EXEMPTION` needs the `game_core::state::prop` mirror, like `DESIGNATES` / `IMMUNE` (`state.rs:455-460`); without it the engine read has no constant. | §5.3 adds the mirror. |
| R13 | §9.1 named tests but no file; G06 established one named file per group. | One new `rules/tests/tests/g07_targeting.rs` (§10.1). |
| R14 | §8.2 said `kani/r.md` "gains a LISA-BOND-EXEMPT row" without saying the row is protocol-level and the proof is on the pure scan. | §10.4 names the model proof; the card row stays PROTOCOL-LEVEL. |
| R15 | The draft had no necessity section and no clause match (G06 added them after review). | §12 and §13 exist. |
| R16 | §2 called the count "the same invariant" as the pipeline but did not say what a **named-but-negated** designation does, although the card consequence depends on it. | §4 states it: named counts, refused does not; §7.3 increments before the window. |
| R17 | §5.2's welcome usage said "delete the TODO" and nothing else; the card also heads the candidate list with the user, which the TODO exists to explain. | The comment names the full pipeline and the self half; the TODO is gone only because §7.1 makes its claim true (§8.2). |
| R18 | §4.6's Kani note moved `Op::targeted_count` off `arm_stub_zero` but did not say the existing `arm_target` counts self and tiles unconditionally, so a conformance run would silently bless the old bug. | §7.8 rewrites both `arm_target` branches and moves `targeted_count` to a real reader (§7.8, §10.4). |
| R19 | §10 alternative A4 said a static prop "cannot ride the grantor seat", but did not mention that `place_card_at` stamps static props at placement. | A4 keeps the reason and cites `wasm_rules.rs:1268-1285` / `ops.rs:1370-1420`. |
| R20 | §2's "every designation enters one pipeline" omitted `target_all`'s per-seat loop; a list containing the actor would have been an unstated edge. | §7.8 states that `target_all` already loops through `sys::target`, so the actor's seat simply runs the self branch. |

## 1. Items

| # | slug(s) | sev | rows | disposition |
|---|---------|-----|------|-------------|
| 1 | `target-count-pipeline` — merged `centrifugal-between-turns-counter`, `centrifugal-tile-targeting`, `welcome-self-target-pipeline` | medium | 274, 1037, 1061, 1294 | **demote to A**: one pipeline, one count predicate, doc text and one sugar wrapper; no manifest field, no import |
| 2 | `lisa-distance-exemption` (merged `lisa-bond-exemption-gap`) | medium | 691, 1451 | B: `DistanceMetric`, `prop::NEAREST_EXEMPTION`, two imports, three wrappers |

Item 1 is **demoted to A** on the surface test, not on effort: every fix is in
the host pipeline and its documentation, and one guest wrapper is sugar over
the existing `target` import. Item 2 needs a durable per-pair grant (a new
property) and a host query (two new imports) and stays B. Only item 2 carries
the batch's wire movement.

Group coverage against `BACKLOG.md` section B: `target-count-pipeline`
(1, with its three merged sub-slugs) and `lisa-distance-exemption` (2, with
its merged sub-slug). Item 1 has three independent facets (the window, the
count grain, the self tail) but one pipeline; item 2 has one mechanism.

## 2. Problem, per item

Each entry names the verbatim fragment, the current site, what the book
requires and the root cause. Root causes are **R1-R4** defined in §3.

### 2.1 `centrifugal-between-turns-counter` — Mor:离心力，不为所动, row 274

> [反击]当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时，你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果

The gate is `targeted_count(owner) >= 2` (`centrifugal.rs:34`). The counter is
zeroed only at the owner's **own turn start** (`play.rs:719-728`) and every
non-self player designation increments it (`wasm_rules.rs:6798-6807`). There
is no “current turn is not the owner's” predicate, so a designation during the
owner's own turn counts toward the window. Two honest examples on master:
`AG:宣战布告` (`declare_war.rs:77`) answers a play by targeting its caster,
and `R:必然的联系（莉莎）` (`lisa_bond.rs:50`) designates the placer's
same-tile pick. Play any two cards and answer both with 宣战布告 inside your
own turn: the second counter targets you, `targeted_count` reaches 2, and
离心力 is offered with no turn boundary in between. The in-file TODO at
`centrifugal.rs:23-33` records it. `mor.md:202-204` reads the clause as
"since the end of the user's previous turn and before the start of their
next".

The spec's reading also settles the other end: the count that the window
accumulates must **start from zero** at the previous turn's end. Keeping the
storage reset at turn start and gating the increment on “not the named seat's
turn” gives exactly that observable behaviour without a second reset point
(§12 A1).

### 2.2 `centrifugal-tile-targeting` — same card, rows 1037 and 1061

The same book clause says 「其他角色技能或卡牌的目标」. A tile designation is
a designation of the tile's owner in the engine's own notion: `target_tile`
(`wasm_rules.rs:6832-6873`) raises `targeted` / `effect` / `target` with
`target = owner` and `tile = tile` (`target_window`, `:6883-6919`), and the
card review at row 1037 records that 「离心力 is offered even though the owner
was never a player target (e.g. `AG:ONE OF US`'s `target_tile`,
`one_of_us.rs:145/149`)」. But `target_tile` never touches `targeted` — the only
increment is in `target_player` (`:6798-6807`) — so the same event that opens
the window does not advance 「第二次」. The card can only be offered off an
unrelated player-target count. The in-file TODO is at `centrifugal.rs:30-33`.

The fix is not “require `tile < 0` on the card”: that would make every tile
designation count zero and leave the asymmetry in the engine for the next
reader.

### 2.3 `welcome-self-target-pipeline` — Mujica:欢迎来到ave mujica的世界, rows 1294 and 274

Book (clause (1)):

> 转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）；

「任意一名玩家」 includes the user, and the body already offers the user first
(`welcome_mujica.rs:41-43`). The pick then calls `ctx::target(who)`
(`:59`) — but `target_player` answers at `p == by_player`
(`wasm_rules.rs:6783-6785`) immediately after `redirect` (`:6762-6781`). A
self designation therefore skips `Untargetable`, the count (which must skip it
anyway, §7.3), the whole `target` window [`targeted` hooks, the `effect`
[反击] chain, the `target` settlement hook] and `ImmuneAll`. The in-file
`TODO(ABI)` is at `welcome_mujica.rs:53-58`; the same shape hits
`rain.rs:39` (通用:雨, which designates the user first by rule) and any
agent that answers a self-designated link.

Note the exact current order, because it bounds the fix: `redirect` already
runs for a self pick, so `RAS:EXIST`'s parenthetical 「包括其他玩家指向自身的
卡」 (`exist.rs:61`) already works — a redirect that re-names onto a third seat
escapes the short-circuit and runs the full pipeline. Only the early return's
**tail** is missing. The card review's claim that the redirect itself is
skipped (CSV row 2035) was written before this order landed; it is fixed
already and §15.2 clause 3 / §15.5 pin it.

### 2.4 `lisa-distance-exemption` — R:必然的联系（莉莎）, rows 691 and 1451

Book:

> 【反击】当你使用技能进行传送后，你可以打出此卡并指定一个和你在同一地块的角色，将此卡放在对方的游戏区，你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆。

The placement half works: the counter designates a co-located player through
the full gate and, on success, places a copy on the hit player's field
(`lisa_bond.rs:50-57`). The exemption half is inert. The phrase is live in
four bodies — this card, 今井莉莎's skill 2, 濑田薰's skill 2
(`kaoru.rs:126-140`) and 惊喜对决 (`surprise_duel.rs:70-86`) — and none can
see the placed instance. The only 「距离最近」 **limit** (a targeting
restricted to the nearest character) is lisa's skill 2 (「传送到距离你正向
距离最近的角色所在的格子」, `lisa.rs:69-95`), and it picks the nearest with an
inline first-best loop (`:71-84`); the other two re-derive a nearest on their
own (a ring direction, a tie-enumerating pick). The TODO is at
`lisa_bond.rs:65`; BACKLOG ABI-10 (`docs/rulebook/BACKLOG.md:522-530`)
records it.

The blocker is that the placed instance cannot even say **who** granted it:
`World::place_card_on` sets `owner = user = player_id` (`ops.rs:1388-1420`),
i.e. both fields are the *designated* player. No player decision is missing —
only a durable grantor fact and a query the skill can call.

`R.md`'s C-R-必然的联系-04 (`r.md:510-523`) calls the limit “only named, never
defined as a keyword”; the one live user is the same character's skill, so the
group names the predicate rather than inventing a keyword.

## 3. Shared root causes

1. **Two entry points with divergent tails.** `target_player` and
   `target_tile` are near-copies and their tails have already drifted: one
   counts unconditionally, one never; both skip the tail for a self name. The
   same clause then reads differently depending on which entry the effect used.
2. **The counter is “since own turn start”, not “between turns”.** The
   storage is per seat and reset at own turn start; nothing conditions the
   increment on the turn boundary. A second counter/reset point is not needed
   — the predicate is — but only the pipeline owns the moment.
3. **A placement carries no grantor fact.** `FieldCard` has `owner` and
   `user`, both meaning the designated player (`ops.rs:1411-1412`). A rule
   that means “this instance was placed by X for a waiver on its holder” has
   nowhere to put X.
4. **The nearest predicate is re-derived in every card.** `my_own_problem`
   (`:43-57`), `lisa.rs` (`:71-84`), `nfo` (`:76-98`), `kaoru.rs` (`:126-140`)
   and `surprise_duel.rs` (`:70-86`) each hand-roll distance, tie and skip
   rules. None has an extension point, so no waiver can reach any of them.

## 4. Capability as an invariant

> **Every designation — a player target, a self-designation and a
> tile-targeting — enters one pipeline with one stage set.** The pipeline is:
> the per-pair cancel check, the board gate, declaration re-naming
> (`redirect`, single player targets only), `Untargetable`, the between-turns
> count **iff the named seat is not the actor**, the `targeted` hook / `effect`
> [反击] chain / `target` hook, the post-window retraction check, and —
> for other-source designations only — `ImmuneAll`. A designation that was
> **named** counts even when a later step (the window, `ImmuneAll`) negates
> it; a designation refused before naming does not. A designation of nobody
> (an unowned tile) names nobody and opens nothing.
>
> **The between-turns counter advances only while the named seat's own turn is
> not the current turn**, and is zeroed at that seat's own turn start.
>
> **The nearest-by-distance predicate is a host query** returning the whole
> tied set (never a first-found seat), with the waiver set for skill targeting
> computed into the same query. A waiver is a durable fact on a placed
> instance, readable by the grantor seat.

Derived obligations:

* **Stage exactness.** For any `(named, actor, single)` the stage set is
  exactly the invariant's. `named == actor` includes `UNTARGETABLE | WINDOW`
  and excludes `COUNT | IMMUNE`. A mutant restoring the early return fails.
* **Count exactness.** The player and tile increment predicate is exactly
  `named != actor && named != turn_player`. A mutant dropping the turn guard
  reproduces 2.1; one counting self reproduces 2.3's count half; one never
  counting a tile reproduces 2.2.
* **Named precedes negation.** The increment happens after `Untargetable`
  passes and before the window; a counter that negates the window leaves the
  count advanced.
* **Nearest exactness.** The query answers exactly every present non-actor
  seat at the minimal metric distance, in seat order, and — with the skill
  flag — exactly the union with the exempt seats, deduplicated,
  distance-then-seat ordered.
* **Exemption exactness.** The scan is true iff some instance held by the
  candidate carries `props[NEAREST_EXEMPTION] == Some(grantor)`. A grant by
  another seat does not count; the value `0` is a valid grantor.
* **Instance lifetime.** The exemption dies with the instance; a refused gate
  places no instance and grants nothing.

## 5. Surface

Everything is generic mechanism: no display text, no player decision inside an
import.

### 5.1 `card_sdk::ctx` — self wrapper (item 1, no wire change)

```rust
/// Target **yourself** through the full pipeline (「指定」 yourself).
/// `Some(hit)` is the seat actually named — a `redirect` may still move it
/// elsewhere; `None` when the designation was refused or cancelled.
/// Sugar over [`target`]; the engine path is identical.
pub fn target_self(player_id: i32) -> Option<i32> {
    target(player_id)
}
```

`sys::target(player_id, tile, single)` keeps its three arguments and its wire
signature. The caster is `call.player_id()`, already what
`apply_host_request` passes as `by_player` (`wasm_rules.rs:4505-4515`).

### 5.2 `card_sdk::abi` — the metric

```rust
/// How the 「最近」 predicate measures distance. Wire `i32`; an unknown value
/// falls back to `Forward` (the v107 unknown-value rule).
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DistanceMetric {
    /// Steps strictly ahead around the ring. Co-located (`0`) is **not**
    /// ahead and is not a forward candidate.
    #[default]
    Forward = 0,
    /// Undirected ring distance (「绝对距离」). Co-located is distance `0`
    /// and is the nearest.
    Absolute = 1,
}
```

### 5.3 `card_sdk::abi::prop` and the engine mirror — the waiver

```rust
pub mod prop {
    // …existing keys…
    /// 「你对对方使用技能将可以无视【距离最近】」: on a **placed
    /// instance's** `props`, the value is the **grantor** seat whose skills may
    /// ignore the nearest limit when targeting this instance's holder.
    /// Written at runtime (like `RENT_HOUSES`), gone with the instance.
    /// Presence is the fact, so the read is `props.get` — seat `0` is a valid
    /// grantor.
    pub const NEAREST_EXEMPTION: &str = "nearestExemption";
}
```

`game_core::state::prop::NEAREST_EXEMPTION` mirrors it (`state.rs:455-466`
style), because the engine read cannot depend on the guest crate.

### 5.4 `card_sdk::ctx` — nearest queries and the grant

```rust
/// The nearest other present player(s) by `metric`, seat order on ties.
/// `Forward` excludes co-located seats; `Absolute` includes them.
pub fn nearest(by: i32, metric: crate::abi::DistanceMetric) -> Vec<i32>;

/// `nearest`, plus every present seat holding an instance whose
/// `prop::NEAREST_EXEMPTION` is `by` (the pair's 「距离最近」 waiver), in
/// distance-then-seat order, deduplicated. The raw nearest set stays first.
/// This is the candidate list of a 「距离最近」 **skill**.
pub fn nearest_skill_candidates(
    by: i32,
    metric: crate::abi::DistanceMetric,
) -> Vec<i32>;

/// Does `candidate` hold an instance granting `by` the waiver?
/// (Per-pair form for a skill that already has its candidate.)
pub fn nearest_exemption(by: i32, candidate: i32) -> bool;

/// Mark the instance at `uid` as granting `by` the waiver on its holder.
/// `ctx::set_prop_at(uid, prop::NEAREST_EXEMPTION, by)`; returns the stored
/// seat.
pub fn grant_nearest_exemption(uid: i32, by: i32) -> i32;
```

All three reads are imports (`nearest` and `nearest_skill_candidates` are the
same `ruleset_nearest` import with the skill flag `0` / `1`); the grant is
sugar over the existing `set_prop_at` (`ctx.rs:1490-1495`) — no new write op.

### 5.5 Imports (`card_sdk::ctx::sys`, host `hostfns`, `CardWorld`)

```rust
// ctx/sys.rs, beside `dist` (`:277`) and `tile_forward` (`:282`); the same
// `#[cfg_attr(any(not(target_arch = "wasm32"), feature = "native-rules"),
// link_name = …)]` wrapper the neighbours use.
pub fn nearest(by: i32, metric: i32, skill: i32, buf: i32, cap: i32) -> i32; // postcard Vec<i32>
pub fn nearest_exemption(by: i32, candidate: i32) -> i32;

// game-rules/src/world.rs (CardWorld trait, beside `dist` (`:674`) and
// `tile_forward` (`:676`) -- these are world measures, not rules data)
/// Nearest present seats for `by` under `metric`; `skill` adds the waiver set.
fn nearest(&self, by: i32, metric: i32, skill: bool) -> Vec<i32>;
fn nearest_exemption(&self, by: i32, candidate: i32) -> bool;
```

`nearest` uses the `designations` out-buffer shape (`hostfns.rs:2333-2346`:
postcard, `ctx::out_buf`, cap 4096 — a body of 4–6 players can never approach
it). `ctx.rs:14` adds `DistanceMetric` beside `AbKind, CardPile`; without the
re-export, `ctx::DistanceMetric` does not resolve in a card body.

Both new `CardWorld` methods need the non-`Run` implementors: `NullWorld`
(`host.rs:6326`, sandbox feature), `rules/tests/tests/testworld.rs:112`,
`crates/game-rules/tests/testworld.rs:109` and `rules/kani/tests/
conf_model.rs:322` (§11).

### 5.6 Not changed

`ctx::designations` / `DESIGNATES` / `designation_cancelled` keep their
meaning: `designations` is the play's **static declared recipient set** (a
prop-driven list `others(actor)`), not a record of pipeline admissions, and a
per-pair `cancel_designation` is still checked at both the entry and the
post-window point. `ctx::dist` / `ctx::tile_forward` stay the raw measures;
`nearest` is the selection built on them. `mark::NO_TARGET` keeps its gate.
`HostRequest::Target` keeps `{ player_id, tile, single_target }`
(`host.rs:435-439`).

### 5.7 Classification

| item | NEW | EXTENDED | WIRE-BREAK | NO-WIRE |
|---|---|---|---|---|
| 1 | `ctx::target_self` (sugar) | `target_player` / `target_tile` stage order; `note_designation`; `targeted_count` doc + semantics | — | **yes** |
| 2 | `DistanceMetric`, `prop::NEAREST_EXEMPTION` (+ engine mirror), 2 imports, 3 wrappers | `CardWorld` trait, every implementor, `World` free functions | new op ids (append-only) | — |

Two new append-only op ids are the only wire change; the `ManifestOn` /
`Trigger` / `HostRequest::Target` shapes are untouched.

## 6. `targeted_count` semantics in the vocabulary

The cond native function (`cond/src/vocab.rs:1214-1226`, `view.rs:263-265`,
`ctx.rs:51`) keeps its name and `i64` shape. Its doc line changes to
"designations of `p` by other sources since `p`'s own turn ended (not
counted during `p`'s turn)". `GUARDS.md` §4.2's list entry
(`GUARDS.md:78`, `:167`, `:495`, `:579`, `:752`) needs the same wording; no
window variable is added.

## 7. Engine wiring

### 7.1 `target_player` — stages (items 1a, 1c)

`wasm_rules.rs:6730-6831`. Delete the early return and let the tail run; the
stage list becomes:

| # | stage | today | after |
|---|---|---|---|
| 1 | seat / out check | runs | unchanged |
| 2 | per-pair cancel | runs | unchanged |
| 3 | `OnBoard` gate | runs | unchanged |
| 4 | `redirect` (single only) | runs | unchanged |
| 5 | `Untargetable` | **skipped when `p == by_player`** | runs |
| 6 | count | **skipped when `p == by_player`; otherwise unconditional** | `note_designation(p, by_player)` |
| 7 | `target_window` (`targeted` hook → `effect` chain → `target` hook) | **skipped when `p == by_player`** | runs |
| 8 | post-window `cancelled_designations` check | runs for non-self only | runs |
| 9 | `ImmuneAll` | **skipped when `p == by_player`** | runs iff `p != by_player` |
| 10 | answer `p` | — | unchanged |

```rust
// wasm_rules.rs:6781-6788, before → after
-        // A self-target no redirect moved (EXIST's 「包括其他玩家指向自身的卡」
-        // included): already named, nothing to chain.
-        if p == by_player {
-            return Ok(p);
-        }
         // `Untargetable` governs who may be *named*. …
```

```rust
// wasm_rules.rs:6798-6807, replaced by one call
-        if let Ok(s) = usize::try_from(p) {
-            let mut w = cx.world_copy();
-            if w.targeted.len() <= s {
-                w.targeted.resize(s + 1, 0);
-            }
-            w.targeted[s] += 1;
-            cx.swap_world(w);
-        }
+        self.note_designation(cx, p, by_player);
```

The window at `:6808` loses its actor guard (`if p != by_player &&
!self.target_window(...)` -> `if !self.target_window(...)`); the immune call
at `:6821` gains one (`if p != by_player && self.immune(cx, p, by_player)?`).

**Why `ImmuneAll` stays other-source only:** the engine already treats
`by == player_id` as “your own effect” everywhere else — the abnormal gate
skips `ImmuneAll` for a self application (`abnormal_gate`, `:6673`), and the
payment gate does the same (`:4452`) — and `mor.md:223-224` leaves the
shield's scope OPEN. A self designation must still be *named* (so
the window and a redirect are right), but the owner's own shield must not eat
it. A resolver may rule the other way; the call sites are one condition each.

### 7.2 `target_tile` — stages (items 1b, 1c)

`wasm_rules.rs:6832-6873`. The `NO_TARGET` check and the unowned-tile no-op
stay. Inside the owner branch:

| # | stage | today | after |
|---|---|---|---|
| 1 | owner live check (`owner >= 0 && live`) | `owner != by_player` also in the gate | gate drops `owner != by_player` |
| 2 | `Untargetable` on the owner | only when `owner != by_player` | always (branch entered) |
| 3 | count | **never** | `note_designation(owner, by_player)` |
| 4 | `target_window` | only when `owner != by_player` | always (branch entered) |
| 5 | post-window cancel check | only when `owner != by_player` | always |
| 6 | `ImmuneAll` | only when `owner != by_player` | iff `owner != by_player` |

The outer `if owner >= 0 && owner != by_player && live` (`:6850`) becomes
`if owner >= 0 && live`, with the two `by_player` guards moved inside (stages
3 and 6). `tile` still rides the `targeted` / `effect` / `target` triggers
(`t.tile = tile`).

### 7.3 One increment helper and the count lifecycle

Both entries call one helper, so their grain cannot drift again:

```rust
/// Count one naming of `named` by `by_player`, if the sheet's window is open.
///
/// 规则书 (`Mor:离心力，不为所动`): 「当你在两个你的回合之间（回合1结束，
/// 回合2开始）…成为…目标」. A self designation is the same seat's own card and
/// never counts; a designation during the named seat's own turn is before the
/// window opens. The caller has already passed `Untargetable`, so a refused
/// name does not reach here (named counts, negated still counts).
fn note_designation(&self, cx: &mut PlayCtx, named: i32, by_player: i32) {
    let Ok(s) = usize::try_from(named) else { return };
    if named == by_player || cx.world().st.turn == named { return; }
    if s >= cx.state().players.len() { return; }
    let mut w = cx.world_copy();
    if w.targeted.len() <= s {
        w.targeted.resize(s + 1, 0);
    }
    w.targeted[s] += 1;
    cx.swap_world(w);
}
```

| event | `targeted[seat]` |
|---|---|
| seat's own `turn_start` (`play.rs:719-728`) | `= 0` (unchanged line) |
| player designation named, `p != by_player`, `st.turn != p` | `+= 1` |
| tile designation named, live other owner, `st.turn != owner` | `+= 1` |
| self designation (`p == by_player` / own tile) | unchanged |
| designation during the seat's own turn | unchanged |
| a later counteraction negates the window | already counted (named) |
| a designation refused by `Untargetable` / `NO_TARGET` | unchanged (never named) |
| unowned tile | unchanged |

`st.turn` is the turn player (`state.rs:139`; `-1` before the match); the
predicate "not the seat's own turn" is exactly "between that seat's turn end
and its next turn start" for every designation that can reach the counter. No
new saved field and no new reset point — see §12 A1.

The group's `expected` says "resets at the owner's turn end". Keeping the
reset at turn start and gating the increment on `st.turn != named`
**observably implements** that: the value carried into a turn is zeroed at
its start, no own-turn designation enters the count, and the count the
following between-turns window accumulates starts from 0 and holds only
between-turns designations (§4, §15.1 clause 1).

### 7.4 `ctx::targeted_count` consumers

Only `centrifugal.rs:34` reads it. The condition stays; its comment now cites
the window clause and records the `>= 2` reading with a `TODO(规则书)`
(§8.1). No window variable is added. `designates` / `DESIGNATES` and
`designation_cancelled` are untouched — see §5.6.

### 7.5 Nearest query (`CardWorld::nearest` on `Run`)

`Run` owns both the board data (`self.data.tiles.len()`) and the world. Add
next to `dist` (`wasm_rules.rs:2097-2112`):

```rust
fn nearest(&self, by: i32, metric: i32, skill: bool) -> Vec<i32> {
    use card_sdk::abi::DistanceMetric;
    // An unknown code falls back to `Forward` (the v107 unknown-value rule).
    let absolute = metric == DistanceMetric::Absolute as i32;
    let measure = |a: i32, b: i32| {
        if absolute {
            self.dist(a, b)
        } else {
            self.tile_forward(a, b)
        }
    };
    let at = self.world.player_pos(by);
    if at < 0 || self.data.tiles.len() == 0 {
        return Vec::new();
    }
    let mut best = i32::MAX;
    let mut raw: Vec<i32> = Vec::new();
    for p in self.world.others(by) {
        let there = self.world.player_pos(p);
        if there < 0 {
            continue; // a seat with no board position cannot be measured
        }
        let d = measure(at, there);
        if !absolute && d == 0 {
            continue; // co-located is not ahead
        }
        if d < best {
            best = d;
            raw.clear();
            raw.push(p);
        } else if d == best {
            raw.push(p);
        }
    }
    if !skill {
        return raw;
    }
    // The waiver set, distance-then-seat ordered, appended after the raw set.
    let mut extra: Vec<i32> = self
        .world
        .others(by)
        .into_iter()
        .filter(|&p| {
            self.world.player_pos(p) >= 0
                && !raw.contains(&p)
                && self.nearest_exemption(by, p)
        })
        .collect();
    extra.sort_by_key(|&p| (measure(at, self.world.player_pos(p)), p));
    raw.extend(extra);
    raw
}
```

`others` excludes only out seats (`ops.rs:411-416`), so an exiled seat is
still a candidate (§13.8). The first raw set stays first; ties are seat order
because `others` is seat order.

### 7.6 Exemption scan (`CardWorld::nearest_exemption` on `Run`)

```rust
fn nearest_exemption(&self, by: i32, candidate: i32) -> bool {
    self.world
        .player_id(candidate)
        .is_some_and(|s| {
            s.field.iter().any(|f| {
                f.props.get(game_core::state::prop::NEAREST_EXEMPTION) == Some(&by)
            })
        })
}
```

`props.get` is `Option` — presence is the fact. The instance is found by its
holder; if it later moves to another field, the waiver follows the instance
(§13.5). No board scan and no per-play cache — the set changes only on
placement / removal / movement.

### 7.7 `target_all`

`ctx::target_all` (`ctx.rs:3409-3418`) needs no code change: it calls
`sys::target(s, -1, 0)` per seat, so a list containing the actor now
pipelines that seat instead of short-circuiting, and still dedupes answers.
It will count every non-actor seat it names (§7.3).

### 7.8 Kani model

* `kani_host::Op` gains two append-only ids: `nearest = 441`,
  `nearest_exemption = 442` (after `invoke_frame = 432`).
* `sys_kani.rs` + `native_shims.rs` (`ruleset_nearest`,
  `ruleset_nearest_exemption`) + `host.rs` registration
  (`hostfns.rs:2333-2346` pattern) mirror the two imports.
* `rules/kani/src/host.rs`:
  * `arm_target` (`:3614-3656`) models the new stages: `actor =
    w.trig_player`; both the player and the tile branch pass when
    `named == actor`, and increment only when `named != actor &&
    named != w.turn_player`. (`w.turn_player` exists at `world.rs:833`.)
  * `Op::targeted_count` moves off the `arm_stub_zero` arm (`:1155`) to a
    real reader of `w.targeted_count[p]`.
  * `arm_nearest` computes the §7.5 set over `w.pos` / `w.n_players` /
    `w.out`, postcard-encodes it and writes it with
    `card_sdk::native::write` (`arm_placed_cards`, `:2815-2847`, is the
    pattern). `arm_nearest_exemption` scans `w.field[..w.field_n]` for
    `f.player == candidate && f.props.get_opt(key) == Some(by)`.
  * `Known` (`:47-78`) pre-interns `nearestExemption` so the compare is
    `u16` equality; `install_backend` asserts it is valid.
* `rules/kani/tests/conf_model.rs:1062` and `conf_fill_diff.rs:573` stop
  answering `targeted_count` 0 from the `ConfWorld`; the `:73` gap note drops
  `targeted_count`.
* The exact rules are proved on pure functions in
  `rules/kani/src/spec/g07_targeting.rs` (§10.4), so the stubs only need
  shape consistency.

## 8. Card usage

The mechanism is generic; the cards still have to say what they mean. Every
card below is named by its `cards.json` id.

### 8.1 `Mor:离心力，不为所动` — comment only

**Before / after:** the condition is identical. What changes is that the gate
is now truthful: own-turn designations no longer inflate the count, tile
designations advance it, and a self designation neither advances it nor is
offered against it (`by != owner`). Replace the two TODOs at
`centrifugal.rs:23-33` and `:40` with one citation of the window clause; keep the
`>= 2` comment and make it a `TODO(规则书)` for C-MOR-centrifuge-01's
"second (or later)" reading (`mor.md:202-204`).

### 8.2 `Mujica:欢迎来到ave mujica的世界` — comment only

**Before** (`welcome_mujica.rs:53-59`): `ctx::target(who)` with the TODO
saying a self pick skips the tail. **After:** same call; delete the
`TODO(ABI)` and state that `ctx::target` is the full pipeline for both
branches (the card may use `ctx::target_self` only for readability, but the
seat is a variable here).

### 8.3 `通用:雨` / `AG:宣战布告` — no change

`rain.rs:39` uses `ctx::target(player_id)`; it now runs the row's window like
every other designation (a counter may cancel the self designation via
`cancel_designation`, and its own `Untargetable` gate is asked with
`by == p`, which its condition excludes). `declare_war.rs:77` targets the
caster of the answered card; a self-counter pipelines instead of
short-circuiting. `declare_war`'s own condition filters `by != player_id`
(`:47`), so it still does not answer its owner's own card.

### 8.4 `R:必然的联系（莉莎）`

**Before** (`lisa_bond.rs:50-64`):

```rust
    if let Some(who) = hit {
        ctx::place_card_at(
            who,
            "R:必然的联系（莉莎）",
            &Msg::new(key!("lisa_bond_note")).player_id("who", player_id),
        );
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
            ctx::grant_nearest_exemption(uid, player_id);
        }
        ctx::log(…);
    }
```

`place_card_at` answers the new instance's uid, `-1` on failure (uids start at
1, `state.rs:274`); the guard makes a failed placement grant nothing. The grant
rides the placed instance: leaving the field, or the instance being destroyed,
ends it. A refused target gate places nothing and grants nothing.

### 8.5 `skill:今井莉莎:慈爱女神` (2)

**Before** (`lisa.rs:71-84`): the inline `tile_forward` loop, first strictly
smaller wins, no waiver. **After:**

```rust
    // 规则书: 「传送到距离你正向距离最近的角色所在的格子」
    let candidates = ctx::nearest_skill_candidates(
        player_id,
        ctx::DistanceMetric::Forward,
    );
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

The ask only appears when the waiver (or a tie) makes the set non-singleton,
so the no-waiver path is unchanged. `nearest` (raw) stays available for the
non-skill loops.

### 8.6 First adopters of the raw query

* `CRYCHIC:是我自己的问题` (`my_own_problem.rs:43-57`) — `nearest()` becomes
  `ctx::nearest(player_id, ctx::DistanceMetric::Absolute)`; the existing test
  `near[0]` use is unchanged.
* `R:与你前往遥远彼方` (`nfo.rs:76-98`) — `nearest_ahead()` becomes
  `ctx::nearest(player_id, ctx::DistanceMetric::Forward).first().copied()`
  with `unwrap_or(-1)`; the existing `p == pos` skip matches `Forward`'s
  co-located exclusion.
* `skill:濑田薰:梦幻的王子殿下` (2) (`kaoru.rs:126-140`) — the loop becomes
  `let near = ctx::nearest(player_id, ctx::DistanceMetric::Absolute);`; the
  existing tie enumeration and `ask_choice` stay. Without this the card cannot
  see a waiver either.
* `惊喜对决` (`surprise_duel.rs:70-86`) stays on the raw pair measure
  (`ctx::tile_forward` / `ctx::dist`): it compares one known pair, it does not
  select a seat.

### 8.7 Tile-designating neighbours

No body change; they simply gain the own-tile window and the other-owner
count: `AG:ONE OF US` (`one_of_us.rs:145/149`), `AG:无论是何种颜色的夕阳`
(`sunset.rs:101/114`), `R:交换` (`toko_swap.rs:100`), `MyGO:anon_tokyo`
(`anon_tokyo.rs:91`), `PP:strong_flower` (`strong_flower.rs:78`). `J11`'s
`Targeted` hook (`j11.rs:20`) becomes reachable for a self designation; its
guard still excludes its owner's own card (`:85-87`), which the card's
「其他卡」 clause supports.

## 9. Save, record and replay

* **SAVE_VERSION stays 16.** No saved field is added: `World.targeted`
  already exists with `#[serde(default)]` (`world.rs:897-900`), and the window
  is derived from `st.turn`, which is saved. The exemption is a `props` entry
  on an already-saved `FieldCard` (`state.rs:1150-1155`). An earlier save
  loads and behaves correctly; the count's meaning changes but its storage
  does not.
* **RECORD_VERSION stays 4.** `HostRequest::Target` keeps
  `{ player_id, tile, single_target }` (`host.rs:435-439`); the record stores
  the same answers. Old `.bdrec` files replay on their archived engine bundle
  and keep the old pipeline; nothing in the record needs rewriting.
* **Replay determinism.** The pipeline gains no new host prompt and no new
  random draw; the added stages are pure reads and raises. A replay recorded
  after the change re-runs the same raise sequence. The `nearest` read is
  deterministic (seat order on ties); `ask_player` is an existing recorded
  prompt.
* **ABI_VERSION** (§11) is the only stamp that moves; `save_version` in an
  `EngineStamp` is unchanged.

## 10. Tests and Kani obligations

### 10.1 Per item (`rules/tests/tests/g07_targeting.rs`)

Fixture probes go in `rules/fixtures/test-cards/src/lib.rs`: a
`TEST:target_self` body logging `ctx::target_self`'s answer, a
`TEST:target_tile` body taking the tile from `ask_tile` and calling
`ctx::target_tile`, a `TEST:nearest` body logging
`ctx::nearest(…, Forward)` and `(…, Absolute)`, a `TEST:target_counter`
`On::Counteract(Effect)` entry with `target == owner` that logs, and a
`TEST:targeted_probe` `On::Action(Action::Targeted("", …))` that logs
`trigger::target` / `trigger::tile`. The real cards are used where the clause
itself matters.

| item | test | assertion |
|---|---|---|
| 1a | `g07_centrifugal_own_turn_targeting_is_not_counted` | 宣战布告 twice inside the owner's own turn → not offered; after their turn ends one more targeting → offered |
| 1a | `g07_centrifugal_second_between_turns_targeting_offers` | another player plays 欢迎 at the owner twice during that player's turn → offered on the second |
| 1a | `g07_centrifugal_window_closes_at_own_turn_start` | one between-turns targeting, own turn starts, then a targeting in the own turn → no offer |
| 1a | `g07_centrifugal_count_is_per_seat` | two seats targeted during the same turn keep separate counts |
| 1a | `g07_named_then_negated_still_counts` | a counter cancels the designation inside the window → `targeted_count` still advanced |
| 1a | `g07_refused_designation_does_not_count` | another player's 夏日合宿 `Untargetable` refuses → count unchanged |
| 1b | `g07_centrifugal_tile_targeting_counts` | ONE OF US's `target_tile` on the owner's tile between turns bumps `targeted_count(owner)` by 1 |
| 1b | `g07_centrifugal_tile_targeting_offers` | the second tile hit between turns offers the card; the window carries `tile >= 0` |
| 1b | `g07_centrifugal_own_tile_does_not_count` | targeting the owner's own tile (owner == by) counts 0 but still raises `targeted` |
| 1b | `g07_tile_during_own_turn_does_not_count` | an own-turn tile designation of a seat's tile counts 0 |
| 1c | `g07_welcome_self_target_runs_the_target_window` | an opponent's net_error answers a self pick (the `effect` window exists with `target == by == user`) |
| 1c | `g07_welcome_self_target_skips_immunity` | under 离心力's shield the self toggle still lands (own source) |
| 1c | `g07_welcome_self_target_can_be_cancelled` | `cancel_designation(user)` in the window cancels the self designation |
| 1c | `g07_exist_redirects_a_self_designation` | EXIST on another seat re-names the self pick (the parenthetical, now through the running tail) |
| 1c | `g07_rain_self_designation_uses_the_pipeline` | the 雨 self pick raises the window and is not counted until an opponent answers |
| 1c | `g07_target_all_includes_the_actor_through_the_pipeline` | the fixture's `target_all` list containing the actor windows it and counts the others |
| 1c | `g07_tile_self_designation_can_be_cancelled` | `cancel_designation(owner)` in an own-tile window makes `target_tile` answer false |
| 1c | `g07_own_tile_raises_the_targeted_hook` | the fixture's `Targeted` probe fires for an own tile with `tile >= 0` |
| 2 | `g07_nearest_forward_matches_the_legacy_loop` | a fixture board: `nearest(Forward)[0]` equals the pre-change `lisa` loop result |
| 2 | `g07_nearest_absolute_returns_all_ties` | two seats at the same minimal distance are both returned, seat order |
| 2 | `g07_nearest_forward_excludes_co_located` / `g07_nearest_absolute_includes_co_located` | the two metrics' boundary |
| 2 | `g07_lisa_bond_grants_the_waiver` | place, then `nearest_skill_candidates(Forward)` of the placer contains the designated co-located seat |
| 2 | `g07_lisa_bond_waiver_is_per_grantor` | `nearest_exemption(other, designated)` is false; `nearest_exemption(placer, designated)` is true |
| 2 | `g07_lisa_bond_waiver_dies_with_the_instance` | sweep the placed instance to the discard → no longer exempt |
| 2 | `g07_lisa_bond_refused_gate_grants_nothing` | 夏日合宿 on the candidate, another-source refusal → no instance, no exemption |
| 2 | `g07_lisa_goddess_offers_the_designated_seat` | with the waiver, the press prompts and can teleport to the co-located seat |
| 2 | `g07_lisa_goddess_no_prompt_without_waiver` | plain board → single nearest, no prompt (regression) |

Existing tests that must stay green: `welcome_offers_self_as_well`,
`welcome_switches_designated_state`, `welcome_no_state2_is_noop`, `rain_*`,
`declare_war_*`, `inter_declare_war_answers_no_road_targeting`,
`cross_chain::c12_exist_vs_summer_camp`, `teleport_move::*`,
`roselia.rs:636` (lisa skill forward-nearest), `roselia.rs:671` (bond
placement), `unintended_found::budokan_leaves_targeted_counters`,
`engine::save_and_restore_continue_identically`, `record_codec` round-trips,
every `*_kani` `ppcost` export.

### 10.2 Query-matrix test

`g07_targeting.rs` also drives the fixture `TEST:nearest` on a fixed four-seat
board: for each `(metric, skill)` pair it asserts the exact sequence, including
the empty and singleton cases, the co-located boundary and a duplicate
exemption. This is the direct unit of §4's nearest obligations.

### 10.3 Static sweep

`g06_static_sweep.rs`-style, over `rules/cards/*/src/*.rs`: every
`place_card_at` whose card grants a waiver asserts a `grant_nearest_exemption`
call on its returned uid; every card body that hand-rolls a
`min`/`filter == best` over `ctx::dist`/`ctx::tile_forward` asserts a
`TODO(ABI)` naming `ctx::nearest` until migrated.

### 10.4 Kani obligations

`rules/kani/src/spec/g07_targeting.rs` models the pure rules;
`rules/kani/src/proofs/g07_targeting.rs` proves them (registered in
`spec/mod.rs` ungated and in `proofs/mod.rs` under
`#[cfg(feature = "band-general")]`, as `g06_windows` is):

* `pipeline_stages_iff` — for arbitrary `(by, p, single)` the stage set is
  exactly §7.1's; `p == by` **includes** `UNTARGETABLE | WINDOW` and
  **excludes** `COUNT | IMMUNE`. A mutant restoring the early return fails.
* `targeted_increment_iff` — the player increment is exactly
  `p != by && p != turn`; a mutant dropping the turn guard fails (the current
  bug), one counting self fails.
* `tile_increment_iff` — exactly `owner >= 0 && owner != by && owner != turn`;
  a mutant keeping the old "never" fails, one counting own tiles fails.
* `nearest_iff` — the returned set is exactly every present non-`by` seat at
  the minimal metric distance, seat order; mutants "first only" and "all
  candidates" fail.
* `nearest_skill_iff` — exactly `raw ∪ exempt` in distance-then-seat order; a
  mutant dropping exemptions fails, one admitting a non-exempt farther seat
  fails.
* `exemption_scan_iff` — true iff some instance of the candidate carries
  `props[NEAREST_EXEMPTION] == by`; a mutant matching any grantor fails, one
  using `unwrap_or(0)` fails on grantor 0 vs absent.
* `exemption_dies_with_instance` — after the instance unit is removed the scan
  is false.

Card bodies stay PROTOCOL-LEVEL: `kani/mor.md:56` keeps the centrifuge rows
PROTOCOL-LEVEL (the window is the protocol), the count becomes a modelled host
query; `kani/mujica.md:44` loses the BLOCKED-HOST note on C-MUJICA-欢迎-02
(`Op::target` admits and `arm_target` models the stages); `kani/r.md:78` keeps
C-R-必然的联系-01..04 PROTOCOL-LEVEL but names the exemption scan as its host
side.

## 11. Versioning and band arithmetic

`ABI_VERSION` 112 → **113**, entry inserted immediately below the v112
contract block (`abi.rs:840-863`):

```
/// v113: **designation pipeline and distance queries**. `ctx::target` now
///    runs a self-designation (`p == by`) through `Untargetable`, the
///    between-turns count and the `target` [反击] window, and skips only the
///    count and `ImmuneAll` (own source); `ctx::target_tile` likewise runs the
///    window for an own tile and counts a live other owner's tile. The
///    `targeted_count` counter advances only while the seat's own turn is not
///    current (「两个你的回合之间」) and is zeroed at its own turn start; a
///    named-but-negated designation still counts. New `ctx::target_self`
///    sugar. New imports `ruleset_nearest` / `ruleset_nearest_exemption`
///    (`ctx::nearest`, `ctx::nearest_skill_candidates`,
///    `ctx::nearest_exemption`, `abi::DistanceMetric::{Forward,Absolute}`)
///    and the runtime-written `prop::NEAREST_EXEMPTION`
///    (`ctx::grant_nearest_exemption` sugar over `set_prop_at`).
///    SAVE_VERSION / RECORD_VERSION unchanged.
```

Band arithmetic from the README: G07 owns `ABI_VERSION` 112→113 and the Kani
`Op` band 441–450; it takes two ops (441–442) and no engine kind (the table's
kind column is “—”), and `SAVE_VERSION` stays 16 / `RECORD_VERSION` 4. The G06 landing consumed
111→112 and 411–426; G05+G13's 431–440 are landed on the other side of this
band.

## 12. Alternatives

**A1 — a separate `targeted_between` counter zeroed at turn end.** Matches the
group's first fix hint literally, but needs a new saved field and a reset
point inside the end sequence that is safe across the early out / ended
branches, and it still needs the "not your turn" guard (otherwise an own-turn
designation increments it before the end). Rejected; the `st.turn` predicate
is the same behaviour with no state.

**A2 — a dedicated `ctx::target_self` import that opens only the window.**
Cheaper than the full pipeline, but self designations still skip
`Untargetable` (matters for a redirect onto a shielded seat) and the stages
diverge per call kind — exactly the drift this item is about. Kept as sugar
over the common path instead.

**A3 — exemption as per-player state
(`state::set(candidate, "nearestExemptionBy:<by>", 1)`).** Survives the card
leaving the field, but needs dynamic-key cleanup, and two copies on one player
collapse. Rejected: the placed instance already exists and already dies at the
right moment.

**A4 — exemption as a static `CardDef` prop.** `place_card_at` stamps static
props from the card definition (`wasm_rules.rs:1268-1285`,
`ops.rs:1370-1420`), so the *grantor seat* — which is not a card constant —
cannot ride a static prop. Would need the `user` field instead; rejected only
because `props` avoids changing `place_card_at`'s signature.

**A5 — keep the inline nearest loops and give them a
`ctx::nearest_exemption` check.** One new import only, but every 「最近」 card
must re-derive ties and order, and lisa's skill would still be the only gate.
Rejected; the host query is the invariant.

**A6 — a single-seat `ctx::nearest`.** Simpler signature, but loses ties
(`my_own_problem` already enumerates them) and cannot express
nearest-plus-waiver. Rejected.

**A7 — let the card place a marker mark on the tile and scan marks for the
waiver.** A mark is a unit of a creating card and is removed with it, which
fits; but the per-pair value still has nowhere to live (a mark has no payload
beyond kind / owner / source), and it would need a second mark filter per
query. Rejected; `props` is already the per-instance key/value store.

**A8 — make the skill take the first candidate always and prompt only on
`Absolute`.** Preserves the old no-prompt behaviour but with a waiver the
player can then never pick the exempt target when a nearer one exists — the
opposite of 「无视」. Rejected.

**A9 — count a self designation too, because the engine's `target` window now
sees it.** Simpler predicate (`p != turn` only), but the book says 「其他角色
技能或卡牌的目标」 and the owner's own card is not another character's. A
player could then charge 离心力 by playing self-targeted cards. Rejected on the
clause; §8.1 keeps the card's `by != owner` guard redundant-but-explicit.

## 13. Open questions

1. **Own-turn-end boundary.** A designation that answers a link raised during
   the seat's own `turnEndAfter` (structurally after the turn "ended", but
   `st.turn` is still that seat) does not count. Is `st.turn != named` the
   right cut, or must `endTurnAfter` already count?
2. **Second offer.** `>= 2` keeps offering on the third and later
   between-turns designation (`mor.md:204-206` says "second (or later)"). Does
   the book's 「第二次」 allow a later re-offer if the second was declined?
3. **DESIGNATES plays.** A card that declares its recipient set via
   `prop::DESIGNATES` (「[指定]…所有玩家」) names players without a
   `ctx::target` call, so it never advances the count. Does such a
   declaration count as 「成为目标」? Design: no — only pipeline admissions
   count (OPEN-指定).
4. **Negated targetings count.** A designation refused by `Untargetable` does
   not count (never named); one named and then negated by the window does.
   Confirm the distinction is 「成为目标」 vs 「受到效果」.
5. **Waiver lifetime.** The sheet names no end for 「无视【距离最近】」; the
   spec reads it as live while the effect is live (`r.md:515-523`). Design:
   instance-bound. If the instance is move-adopted to a third player's field,
   the waiver follows the instance and applies to that holder.
6. **Skills only.** The waiver text says 「使用**技能**」; cards keep the raw
   `nearest`. Confirm a waiver never widens a card effect's nearest pick.
7. **Ties.** Is the 「距离最近」 limit satisfied by the whole tied set, or is
   the limit only "distance equal to the minimum" with the player free to
   pick? Design asks only when a waiver or a tie makes the set non-singleton;
   OPEN-目标计数.
8. **Exiled seats.** `others` includes exiled (not out) seats; a waiver holder
   who is exiled still counts as a candidate. Intended?
9. **Two grants on one holder.** Two copies of the bond card can grant the
   same `by` on one holder; design treats them as one (any instance matches).
   Distinct grantors stack independently.
10. **Other nearest loops.** Should `my_own_problem` / `nfo` / 薰's skill 2
    migrate in this batch (design says yes, §8.6), and should the tile-axis
    `want_to_grab` / `blue_rose` loops get a tile query later?
11. **A waiver and a skill with no other candidate.** With no live other seat
    the raw set is empty and only an exempt co-located seat can be a
    candidate; design keeps it. Confirm the co-located designated seat alone
    is a legal teleport landing.
12. **Escape from the game.** If the grantor leaves the game, the instances
    remain; the waiver is keyed by seat and still matches. Should it survive
    its grantor's elimination?

## 14. Necessity

This section answers one question: why does each piece of §5 exist — and why
could it not be done smaller? It is written item by item so a reviewer can
strike an item without striking the group, and surface by surface so a
reviewer can strike a field without losing sight of what it holds up.

### 14.1 The group

The group is one hole with two ends: the engine's designation notion is
inconsistent between its own entry points, and its nearest predicate has no
extension point. 离心力 is the only count consumer and the only card that can
expose the inconsistency; 必然的联系 is the only waiver source and the only
skill that can consume one. They belong together because the same `target`
call that 莉莎's counter uses is the one that has to be fixed for 离心力, and
because both need the designation to be one pipeline.

Without item 1, a clause that says 「两个回合之间」 is observable only by
accident of which entry point fired; without item 2, a card that says
「无视距离最近」 has no fact to read. Fixing either alone leaves the other's
tests passing on the wrong reason.

### 14.2 Per item

* **1a (count window).** Today two 宣战布告 in the owner's own turn offer the
  card. The smallest alternative is A1's second counter, which needs a saved
  field and an end-sequence reset. Necessity: one predicate on the existing
  counter; nothing card-side can observe the turn boundary from
  `targeted_count` alone.
* **1b (tile count).** Today the same event opens the window and does not
  advance the count, so the card is offered off an unrelated figure. The
  card-side alternative (`tile < 0` in the condition) closes tile hits
  entirely. Necessity: the increment in the branch that already raises the
  window.
* **1c (self tail).** Today a self pick skips `Untargetable`, the window and
  `ImmuneAll`; net_error cannot cancel a self designation, 离心力 cannot see
  one, and the `targeted` hook is dead for it. A2's window-only import keeps
  the untargetable divergence. Necessity: deleting one early return.
* **2 (waiver).** The placed copy is inert and the skill's loop cannot see it.
  A3's player state collapses copies and needs cleanup; A5 pushes the tie and
  order derivation into every card; A7's mark has no payload. Necessity: one
  property and one pair query, both already supported by existing storage.

### 14.3 Surface, field by field

| surface | forced by | if removed |
|---|---|---|
| `ctx::target_self` | 1c readability | callers use `ctx::target(player_id)`; no behaviour change |
| `DistanceMetric` | 2 (Forward) and §8.6 (Absolute) | the query cannot say which ring measure it means |
| `prop::NEAREST_EXEMPTION` + mirror | 2 | the instance cannot name its grantor |
| `nearest` import | 2, §8.6 | every card re-derives ties and the waiver |
| `nearest_exemption` import | 2 (per-pair form) | a card with a candidate cannot ask |
| `grant_nearest_exemption` | 2 | a raw `set_prop_at` call with a string literal |
| `nearest_skill_candidates` | 2, 8.5 | the skill must union raw and exempt itself |

Nothing in §5 is speculative: every field is read by at least one item, and
every item's test in §10.1 reads the field it exists for.

### 14.4 The breaks are the price, and it is small

* **ABI 113.** Two append-only imports; a module built before the bump cannot
  instantiate against the new host, which is the point of the version.
* **SAVE 16 / RECORD 4 unchanged.** The counter already saves; the grant rides
  an existing saved map.
* **No new display text, no decision in an import.** Every decision is still
  `ask_player` / `ask_tile` in a body; `nearest` returns a list.
* **Landing independence.** Item 1 compiles and lands alone; item 2's imports
  and card usage land as one piece. Neither needs a later group.

### 14.5 What would argue against the group

* *“Just change the card condition.”* It fixes neither the window skip (1c)
  nor the tile count (1b) and leaves the engine's two entry points divergent.
* *“A1 avoids the predicate.”* It adds state, a reset point and the same
  predicate in a different place.
* *“Compute the waiver in the body with `ctx::prop_at`.”* The body can see
  only the running card's instance, not the candidate's field; it would need
  `field_instances` + `prop_at` per candidate — the query again, per card.
* *“Defer item 2.”* Legitimate: item 1 is independent and 必然会联系's
  placement half already works. Striking item 2 leaves the card's displayed
  note false and costs the two imports.

## 15. Clause match

Every affected card is quoted clause by clause — the numbered branches the
book itself uses are kept — and each clause is matched to the design element
that satisfies it, the place it is tested, and a verdict. **matched** means
the clause is expressible exactly and pinned by a test; **matched, assumption**
means the design reads an unruled detail and §13 carries it; **was gap** means
this pass found the design short and the fix is now in the body.

### 15.1 `Mor:离心力，不为所动` (item 1)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「当你在两个你的回合之间（回合1结束，回合2开始）」 | `note_designation`'s `st.turn != named` and the turn-start reset (§7.3) | `g07_centrifugal_own_turn_targeting_is_not_counted`, `g07_centrifugal_window_closes_at_own_turn_start` | matched |
| 2 | 「第二次成为其他角色技能或卡牌的目标时」 | the count increments on a named player or tile designation, `named != by_player` (§7.1-§7.3); `>= 2` reading kept with `TODO(规则书)` | `g07_centrifugal_second_between_turns_targeting_offers`, `g07_centrifugal_tile_targeting_counts` | matched, assumption (§13.2) |
| 3 | 「你可以打出此卡」 | existing optional `On::Counteract` entry; unchanged | existing `morfonica` suites | matched |
| 4 | 「直到下个你的回合开始时」 | existing `Action::TurnStart` body; unchanged | existing | matched |
| 5 | 「无效化你受到的所有效果」 | existing `Filter::ImmuneAll`; a self designation skips the *own* shield (§7.1) | `g07_welcome_self_target_skips_immunity` | matched, assumption (§13.4) |

### 15.2 `Mujica:欢迎来到ave mujica的世界` clause (1) (item 1c)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「转换任意一名玩家的状态」 | the existing user-first candidate list and `ctx::target`; the pipeline now runs for the self half | `welcome_offers_self_as_well`, `g07_welcome_self_target_runs_the_target_window` | matched |
| 2 | 「（若指定了不存在状态2的玩家则无效果）」 | unchanged `switch_state` gate (CH-08 / C-Q30) | `welcome_no_state2_is_noop` | out of group |
| 3 | parenthetical of EXIST covering self | `redirect` already precedes the (removed) return; the running tail names the redirect destination | `g07_exist_redirects_a_self_designation`, `cross_chain::c12` | was gap (now reachable) |

### 15.3 `R:必然的联系（莉莎）` (item 2)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「【反击】当你使用技能进行传送后，你可以打出此卡」 | existing `MoveAfter` entry and condition; unchanged | `roselia.rs:671`, `roselia.rs:969` | matched |
| 2 | 「并指定一个和你在同一地块的角色」 | existing ask + `ctx::target`; the full gate, unchanged | `roselia.rs:671` | matched |
| 3 | 「将此卡放在对方的游戏区」 | `place_card_at` returning the uid (§8.4) | `g07_lisa_bond_grants_the_waiver` | matched |
| 4 | 「你对对方使用技能将可以无视【距离最近】这一限制」 | `grant_nearest_exemption` + `nearest_skill_candidates` (§5.3-§5.4, §7.5-§7.6, §8.4-§8.5) | `g07_lisa_goddess_offers_the_designated_seat` | was gap (now granted) |
| 5 | 「当你发动该效果后，此卡进入弃牌堆」 | the played copy's default `Dest::Graveyard`; the placed copy is the persistent marker | `roselia.rs:671` (discard assertion) | matched |

### 15.4 `skill:今井莉莎:慈爱女神` (2) (item 2)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「消耗一个火罐」 | existing `spend_skill`, after the candidate pick | existing `roselia.rs:636` | matched |
| 2 | 「传送到距离你正向距离最近的角色所在的格子」 | `nearest_skill_candidates(Forward)`; the raw set is the legacy result, the waiver appends the designated seat (§8.5) | `g07_nearest_forward_matches_the_legacy_loop`, `g07_lisa_goddess_no_prompt_without_waiver` | matched |
| 3 | 「触发结算」 | existing `plan::set_settles(true)`; unchanged | existing | out of group |
| 4 | 「本次传送触发结算前…此次传送不触发任何结算」 | existing second-fire `ask_yes`; unchanged | existing | out of group |

### 15.5 Neighbours the design must not break

| card | clause | how it stays true |
|---|---|---|
| `RAS:EXIST` | 「场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）」 | `redirect` runs before the removed return; a self pick now passes through it |
| `Mujica:#J11` | 「被其他卡指定的时刻」 | the `Targeted` hook now runs for self; the `by != player_id` guard keeps its owner's own card out |
| `AG:宣战布告` | 「当你或你拥有的格子被其他玩家的卡效果影响时」 | the `by != player_id` guard still excludes a self designation |
| `通用:雨` | 「（其中必须包括[使用者]）」 | the user goes through `ctx::target` first; a cancel is a permitted refusal (§13.4) |
| `AG:ONE OF US`, `AG:无论是何种颜色的夕阳`, `R:交换`, `MyGO:anon_tokyo`, `PP:strong_flower` | tile designations | the window now also fires for an own tile; no body change |
| `CRYCHIC:是我自己的问题`, `R:与你前往遥远彼方`, `skill:濑田薰:梦幻的王子殿下` (2) | 「绝对距离最近」 / 「前方最近」 / 「绝对距离最近」 | migrated to the raw query with the same measure and tie rule (§8.6) |
| `惊喜对决` | 「绝对距离最近的方向」 | stays on the raw pair measure; the direction is unchanged |

### 15.6 Result of the match

Two clauses were short and are now covered: the bond's waiver (15.3 clause 4)
and the reachable self window (15.1/15.2). Every other clause maps to an
existing behaviour plus one named design element, and every mapping has a test
in §10.1 or an existing suite.
