# ABI design G15 — Kani verification host support

**Status: draft for user review. Design only — no source edits, no commit.**

Worktree `/home/monopoly/web/.claude/worktrees/card-rulebook-review`, branch
`worktree-card-rulebook-review`. Group G15, family "Kani verification host
support".

**Closed slug and row:**

| # | slug | sev | rows | class |
|---|------|-----|------|-------|
| 1 | `kani-owned-livehouse` (merge of `blue-rose-dead-spec`) | low | 623 | B |

Ground truth: `docs/rulebook/cards.json`, id `R:蓝玫瑰的骄傲`, re-read
2026-10-28. The clause fragments below are verbatim from that cell.

| field | value |
|---|---|
| `rulebook_text` | 「（1）若你拥有的Livehouse格子多于或等于你拥有的其他格子，获得相当于这些Livehouse格子地契价值20%的资金 。」 |
| `current` | `Op::tile_in("buyable")` is `is_property && owner < 0` in the Kani host, so `live_deeds()` is always empty; no proof references the `blue_rose` functions. |
| `expected` | Iff/no-op/mutant harnesses over a world where the player owns printed-price Livehouses. |

Design scope: **model-only**. The production ABI is untouched; every change is
under `rules/kani` (`cfg(kani)` model + harnesses). No import is added, no
`Op` is added, no wire byte changes.

**This design does not touch the card body.** `play_kani` (added in
`rules/cards/card-roselia/src/blue_rose.rs:149-153`) is the only entry the
proofs call, exactly like the other `R` unit proofs.

---

## 2. Problem

### 2.1 What the book says and which part is dead

Clause (1) of `R:蓝玫瑰的骄傲` (row 623):

> 「（1）若你拥有的Livehouse格子多于或等于你拥有的其他格子，获得相当于这些Livehouse格子地契价值20%的资金 。」

Clause (2) teleports; clause (1) pays. The Kani unit carries the clause-(1)
reference helpers in `rules/kani/src/spec/r.rs:372-415`:

```rust
pub fn blue_rose_rich(n_live: i32, n_other: i32) -> bool { n_live >= n_other }
pub fn blue_rose_payout(prices: &[i32]) -> i32 { /* 20%, -1 if non-integral */ }
pub fn blue_rose_payout_ceil10(prices: &[i32]) -> i32 { /* the body's CeilTo */ }
pub fn blue_rose_payout_log_ok(effects: &[Effect], player: i32, amount: i32) -> bool { … }
pub fn blue_rose_payout_mutant(prices: &[i32]) -> i32 { /* 25% */ }
```

…and nothing calls them. `grep -rn blue_rose rules/kani/src/proofs` returns
nothing; `proofs/r.rs` has no blue_rose section and no `host_subset!` for the
body. The unit table already records the consequence
(`docs/rulebook/spec/kani/r.md:79`):

> `| C-R-蓝玫瑰的骄傲-01 | 20% of Livehouse deeds | (pending more setup) | (spec in spec::r) | — | …`

### 2.2 Why the harness cannot simply be written today

The card's definition of a Livehouse deed
(`rules/cards/card-roselia/src/blue_rose.rs:34-43`) is:

```rust
fn livehouses(player_id: i32) -> Vec<i32> {
    (0..ctx::tile_count())
        .filter(|&t| ctx::is_buyable(t) && ctx::is_color(player_id, t, 6))
        .collect()
}
```

`ctx::is_buyable(t)` is `tile_in(-1, t, "buyable")`
(`rules/bandori-std/src/lib.rs:169-171`). The Kani host answers that class as
`property && owner < 0` (`rules/kani/src/host.rs:423-433`, duplicated at
`:3088-3100`):

```rust
// ABI: tile_in(player_id, tile, class_ptr, class_len)
// This model keeps the historical quirks so proofs stay put:
// buyable = unowned property; livehouse = tiles 31/50.
…
(board::is_property(t) && owned < 0) as i64
```

Production `tile_class::tile_in` is ownership-independent: the registered
class `buyable` requires `TileData::is_buyable()` = `price > 0`
(`crates/game-core/src/data.rs:95-102`, `crates/game-rules/src/tile_class.rs:101-127`).
So the moment a tile has an owner, the model stops calling it buyable.

Chain of vacuity for the clause-(1) harness:

1. `w.tiles[t].owner = user` (the only way to own a deed in the model);
2. `ctx::is_buyable(t)` → `board::is_property(t) && owner < 0` → **false**;
3. `livehouses()` drops the owned tile;
4. `live_deeds()` (`blue_rose.rs:56-62`) is always empty;
5. `play()` computes `live.len() = 0` and either pays 0 (0/0) or always takes
   the teleport branch — the 20% sum is never exercised.

### 2.3 The second, independent blocker

Even if only step 2 were fixed, the host's colour class is a hard-coded index
pair (`host.rs:441-451` and `:3108-3111`):

```rust
// Live House is board.json index 32 (agent) / 51 (property
// Live House Galaxy) → engine positions 31 / 50.
(a1 == 31 || a1 == 50) as i64
```

Engine index 31 is the 「Live House」 **agent** (kind 2, price 0); it is colour
6 but not buyable, so under a corrected `buyable` every group-6 **deed** is an
owned Livehouse (the four RiNG tiles, 21, 40, 42, 50) and only the agent is
excluded. A single-deed payout does not prove the clause's
**sum over the owned Livehouse deeds**, so the colour class must become
data-derived (real group-6 deeds: the four RiNG tiles, 21 DUB MUSIC
EXPERIMENT, 40 武道馆, 42 Space, 50 Live House Galaxy) as part of this item.

`board::tile_in` already contains the correct static definitions
(`rules/kani/src/board.rs:600-615`), but no host arm calls it. The fix is
therefore mostly delegation, not new semantics.

### 2.4 Review rows closed

| source | id | point |
|---|---|---|
| group | 623 | the capability gap itself |
| review | `R:蓝玫瑰的骄傲` / 1443 (`misleading_comment`) | no `#[kani::proof]` references any `blue_rose_*` predicate; the "executable contract pins …" claim is never machine-checked |
| review | `C-R-蓝玫瑰的骄傲-01` row text | the executable pin is the integer case (`1000 → 200`); the harness must pin the same |

The native tests (`rules/tests/tests/spec_card_r.rs:1078`,
`rules/tests/tests/roselia.rs:323`) already own a RiNG and assert 200. They
prove the real engine; they do not make the Kani specs non-vacuous.

---

## 3. Capability as an invariant

The capability is stated and enforced as four invariants. Every blue_rose
harness machine-checks the applicable ones before it calls the body, so a
model regression fails loudly instead of making the proof vacuous. (IN-2 is
existential and checked by every fixture that requests a Livehouse; the
no-deed 0/0 world checks the exact count instead.)

### IN-1 — model fidelity (static class table)

For every tile `t ∈ 0..60`, player `p ∈ {-1, 0}`, and class
`c ∈ {"buyable", "shop", "livehouse", "color:<g>"}`, in a prop-free world:

```
host::tile_in_model(w, p, t, c) == board::tile_in(&TILES[t], p, c)
board::tile_in(&TILES[t], p, "buyable") == (TILES[t].price > 0)
```

The second line must hold **with and without an owner** on `t` (ownership
independence).

### IN-2 — the owned-Livehouse witness

There is a bounded world where seat `p` owns tile 50 (Live House Galaxy,
group 6, printed price 1400) and:

```
owned_count(p) == 1
owned_at(p, 0) == 50
tile_in(p, 50, "livehouse") == 1
tile_in(-1, 50, "buyable") == 1      // the card's is_buyable(-1) call
tile_in(p, 50, "color:6") == 1
tile_in(-1, 50, "livehouse") == 1
```

The card's own two-part filter, evaluated in the body order, must yield `[50]`.

### IN-3 — fixture bound

A blue_rose fixture owns at most 8 tiles, the `cfg(kani)` cap inside
`ctx::owned_tiles` (`crates/card-sdk/src/ctx.rs:342-354`). Every harness
asserts the exact owned/livehouse/other counts, so a silently truncated deed
list cannot pass.

### IN-4 — rounding pin

For every Livehouse tile's printed price `p`, the book's integral 20% and the
body's `CeilTo(sum*0.2, 10)` agree: `blue_rose_payout([p]) ==
blue_rose_payout_ceil10([p])`. All printed prices are multiples of 100, so 20%
is a multiple of 20; the pin is total on the board, not a lucky sample.

### What the proofs quantify over

Ownership (owned / unowned, and counts 0..=3), colour (real group-6 deed vs
non-group-6 deed), deed state (bought; houses and mortgage are unread by the
clause), and the acting seat (0 and 1 — every clause here is self-facing).
Fixtures use real board ids and real printed prices.

Out of scope, stated so the boundary is explicit: colour **props**
(`anyColor`, `colorFor:<p>`, `class:*`, `classFor:*`), more than 8 deeds, and
the clause-(2) destination (move-plan stubs). The colour-prop gap is tracked
as `toko-color` in `docs/rulebook/BACKLOG.md`.

---

## 4. Surface

### 4.1 Production ABI — none

| path | change |
|---|---|
| `crates/card-sdk/src/ctx/sys.rs` (+ `sys_kani.rs`) | none — `tile_in(player, tile, class)` keeps its signature |
| `crates/card-sdk/src/kani_host.rs` | none — `Op::tile_in = 18` keeps its payload `[player, tile, cp, cl, 0, 0]` |
| `crates/game-rules/src/hostfns.rs` / `host.rs` / `tile_class.rs` | none |
| `crates/card-sdk/src/ctx.rs` | none |
| `rules/kani/src/world.rs` | none — `TileState.owner` already exists; fixtures write owners directly, the same way `proofs/ag.rs:957-958` does |

No new import, no new op, no production cfg path, so the three-path obligation
(`sys.rs` / `hostfns` / `ctx`) is satisfied vacuously: all three still route
the same op. Only the **model's answer** changes, and only under `cfg(kani)`.

Because the answer table is shared by every proof, "only the model changes" is
not the same as "no proof is affected": proofs that install an owned tile and
lean on `buyable`/`is_shop` currently being false for it **start firing** once
the table is corrected (e.g. `proofs/ag.rs:1046-1047` sets an owned shop tile,
`proofs/pp.rs:1507` speaks of "one buyable owned tile"). Landing this item
therefore includes a full `cargo kani` re-run with the affected proof list
recorded in the change, and `docs/rulebook/spec/kani/ag.md:21` ("`board.rs`
shared, not editable") is amended to name this one sanctioned edit.

### 4.2 `rules/kani/src/board.rs` — static class table (hand-written helpers)

The `TILES` array stays generated; the helpers are hand-written and are the
single source of class answers under Kani.

```rust
/// `TileData::is_buyable` — price > 0. Ownership-independent.
pub fn is_buyable(t: &TileData) -> bool { t.price > 0 }

/// Colour group 6 (production `is_color(_, t, 6)` with no colour props).
fn is_color_6(t: &TileData) -> bool { t.group == 6 }

/// Production `tile_class::class_matches`: the ruleset's registered classes
/// (`buyable` / `shop` / `livehouse`) then the generic `color:<g>` class.
/// `player` is accepted for the future colour-prop layer (current model:
/// the tile's own group; `ALL_COLORS` = -2).
pub fn tile_in(t: &TileData, player: i32, class: &str) -> bool {
    let _ = player;
    match class {
        "buyable" => is_buyable(t),
        "shop" => is_shop(t),                    // buyable && group == 10
        "livehouse" => is_buyable(t) && is_color_6(t),
        _ => match class.strip_prefix("color:").and_then(|g| g.parse::<i32>().ok()) {
            Some(g) => t.group == g || t.group == -2,   // ALL_COLORS
            None => false,
        },
    }
}
```

`is_property` / `is_deed` / `is_agent` are unchanged (they are `tile_is`
kind helpers, not classes). `is_shop` keeps its current definition
(`is_buyable && group == 10`, `board.rs:552-555`) and becomes
ownership-independent through `is_buyable`.
The stale comment "Property or Ring (game-core `data.rs`)" is corrected to
`price > 0`; a data proof (M1) asserts `price > 0 ⇔ kind ∈ {1, 3}` over the
whole generated table so a board regeneration that breaks the equivalence is
caught.

### 4.3 `rules/kani/src/host.rs` — one model entry

The two duplicated arms collapse into one function; both call sites delegate:

```rust
/// Production `tile_class::tile_in` over the model world.
/// Missing tile → false (production passes `None`). Props are not modelled.
pub fn tile_in_model(_w: &World, player: i32, tile: i32, class: &str) -> bool {
    match board::data_at(tile) {
        Some(t) => board::tile_in(t, player, class),
        None => false,
    }
}
```

* `Op::tile_in => arm_tile_in(a)` in [`dispatch`] (replaces `host.rs:420-452`);
* `arm_tile_in` decodes `[player, tile, cp, cl]` and calls the function
  (replaces the duplicate at `:3088-3123`);
* `livehouse` and `color:6` no longer special-case 31/50;
* `owned` is no longer read.

The explicit `&World` argument (unused today) keeps the call sites stable if
Alternative A (props) lands later.

### 4.4 Harness preconditions — `rules/kani/src/proofs/r.rs`

Unit-local fixtures and one assertion helper (the kit stays generic):

```rust
/// Group-6 buyable deeds, real ids/prices (name, engine index, price).
/// 50 Live House Galaxy 1400 · 42 Space 1200 · 40 武道馆 3400 · 8 RiNG 1 1000
const LIVE: [i32; 4] = [50, 42, 40, 8];
/// Non-group-6 deeds. 2 购物中心 3000 · 3 天文馆 2600 · 4 水族馆 3200
const OTHER: [i32; 3] = [2, 3, 4];

/// The G15 invariant over the installed world: every owned tile answers
/// `livehouse` iff it answers `buyable ∧ color:6`. Returns the owned
/// Livehouse count (0 is legal for the no-deed world; `deed_setup` compares
/// it to the requested fixture).
fn assert_owned_livehouse(user: i32) -> i32;

/// Install the backend + sparse world, clear tiles, set `user`'s owners and
/// concrete money, then run IN-2/IN-3. Panics on any drift.
fn deed_setup(user: i32, live: &[i32], other: &[i32]);

/// The fixture's printed prices, through the same `tile_price` host the body
/// uses (`live.iter().map(|&t| ctx::tile_price(t))`).
fn prices_of(live: &[i32]) -> Vec<i32>;

/// `world.narration_n` (the non-frame log counter).
fn narration_n() -> usize;
```

`deed_setup` asserts `live.len() + other.len() <= 8`, the exact owned count,
and `assert_owned_livehouse(user) == live.len()` (IN-2 holds whenever the
fixture has a Livehouse). Under Kani the assertions are CBMC properties
(`crate::kassert!`), not `debug_assert!`.

### 4.5 Tests that pin the model natively

* `rules/kani/tests/conf_hostops2.rs` — new engine-contract tests (installed
  over the file's `base()` world so the 8-tile pilot shortcut cannot fire):
  owned property stays `buyable`; owned tile 50 is `livehouse`; tile 31 is
  `color:6` but not `livehouse`; tile 8 (RiNG) is `livehouse`; non-group-6 is
  not `color:6`.
* `rules/kani/tests/conf_model.rs:358-373` — the hand-copied `tile_in` mock
  (which repeats the quirk and drifts to 1-based `32/51`) delegates to
  `rules_kani::board::tile_in`, so the differential test exercises the same
  table as the host.
* `rules/kani/tests/conf_fill_diff.rs:244-249` — a third hand copy
  (`board_is_buyable` / `board_is_venue`, already owner-independent and
  kind-based). It happens to agree; delegating to `board::is_buyable` /
  `board::tile_in(.., "livehouse")` removes the drift risk.

### 4.6 Classification

* **new** — model function, harness helpers, proofs (all `cfg(kani)`).
* **extended** — the answers of the existing `Op::tile_in`, and the static
  `board::is_buyable` / `board::tile_in`.
* **wire-breaking** — no. Payload, enum id, import set and SAVE/RECORD shape
  are unchanged.

---

## 5. Wiring

### 5.1 Model answer table (after)

| tile | `buyable` | `color:6` | `livehouse` |
|---|---|---|---|
| 50 Live House Galaxy (property, group 6) | true, owned or not | true | true |
| 8 / 16 / 36 / 53 RiNG (ring, group 6) | true, owned or not | true | true |
| 21 / 40 / 42 group-6 properties | true, owned or not | true | true |
| 31 Live House (agent, group 6, price 0) | false | true | false |
| 2 / 3 group-7 properties; 1 / 4 group-1 properties | true | false | false |
| 0 CiRCLE / 30 江户川乐器店 / 46 商店街 | false | false | false |

### 5.2 Harness body (exact order)

```rust
fn blue_rose_payout_iff_at(user: i32, live: &[i32], other: &[i32]) {
    deed_setup(user, live, other);              // install + owners + IN-2/IN-3
    let pre = world::snapshot();
    let pre_pos = world::with_world(|w| w.pos);
    invoke::unwrap_done(card_roselia::blue_rose::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let amount = spec::blue_rose_payout(&prices_of(live));

    kani::cover!(log_n == 1, "one payout effect");
    kani::cover!(log_n == 0, "zero payout (0/0 world)");
    crate::kassert!(spec::blue_rose_rich(live.len() as i32, other.len() as i32),
        "C-R-…-01: rich predicate holds");
    // `gain(0)` logs no effect, so the 0/0 world asserts emptiness instead.
    if amount > 0 {
        crate::kassert!(spec::blue_rose_payout_log_ok(effects, user, amount),
            "C-R-…-01: exactly GainMoney 20% of the live deed prices");
    } else {
        crate::kassert!(effects.is_empty(), "C-R-…-01: zero payout logs no effect");
    }
    crate::kassert!(spec::only_player_money_delta(&pre, &post, user, amount, 1),
        "C-R-…-01: only the user's money moves, by the payout");
    crate::kassert!(world::with_world(|w| w.pos) == pre_pos, "frame: pos");
    crate::kassert!(narration_n() == 0, "clause (1) does not log a move");
}
```

Every fixture price is a multiple of 100, so every sum is a multiple of 20 and
`blue_rose_payout` never returns the `-1` open-rounding sentinel (M6 proves the
same property over the whole livehouse price column). `deed_setup` sets
`w.money = [10_000; MAX_PLAYERS]` concrete so the gain can
never saturate; the payout is ≤ 1200 here. The harnesses also run the full
frame through `assert_frame!(pre, post, pos, charge, stay, hand, discard)` and
assert every fixture tile's `owner` is unchanged (a payout never rewrites
ownership).

The not-rich harness (`blue_rose_not_rich_noop_at`) asserts the negation:
effects empty, money unchanged for all seats, `narration_n() == 1` (the
teleport log; `arm_log` pushes a narration counter, `host.rs:1960-1964`), and
the frame unchanged. `pos` does not move under Kani because `card_move` is a
protocol stub (`host.rs:1462-1466`) — that stays PROTOCOL-LEVEL for clause (2),
but the running of clause (2) is now observable by the narration count and the
absence of a gain.

### 5.3 Per-body host-op subset

`blue_rose_dispatch` lists exactly what the unmodified body can reach:

```rust
crate::host_subset!(blue_rose_dispatch {
    player_pos => arm_player_pos,
    tile_count => arm_tile_count,
    tile_in => arm_tile_in,
    tile_price => arm_tile_price,
    tile_owner => arm_tile_owner,
    tile_forward | dist => arm_dist,
    tile_named => arm_tile_named,
    owned_count => arm_owned_count,
    owned_at => arm_owned_at,
    gain | gain_typed | gain_fixed => arm_gain,
    log | effect => arm_log,
    set_kind | set_teleport_to | set_resolve => arm_protocol_stub,
    card_move => arm_card_move,
});
```

`gain_typed` / `gain_fixed` / `effect` / `dist` are listed although this body
currently uses `gain`, `log`, `tile_forward` only, so an isolation break is a
loud FAIL (`outside_subset`) rather than a silent pass.

### 5.4 Which blue_rose proofs become non-vacuous

None exist today. After this design: all six model proofs (M1–M6) and all
clause-(1) proof pairs (B1–B10, §9) execute the real filter → body path, and
`vacuity is itself asserted`: `deed_setup` fails if the filter stops seeing the
fixture. The unit table row `C-R-蓝玫瑰的骄傲-01` moves from "(pending more
setup)" to PROVEN with pasted verdicts; rows -02/-03 stay PROTOCOL-LEVEL for
the move plan, with the note that their **trigger** is now exercised by the
not-rich harnesses.

---

## 6. Card/spec usage — before/after

### Before

```rust
// rules/kani/src/spec/r.rs:369-375
// 「地契价值」 = printed tile price. No harness yet -- these helpers are
// spec-only (`proofs/r.rs` has no blue_rose section).
```

`blue_rose_rich` / `payout` / `payout_ceil10` / `payout_log_ok` /
`payout_mutant` compile and are referenced by nothing. The only reader of the
Livehouse class under Kani is dead for owned tiles.

### After (spec text unchanged; proofs added)

```rust
// proofs/r.rs, clause (1) section
/// C-R-蓝玫瑰的骄傲-01 iff (1 livehouse, 1 other — equality boundary).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn blue_rose_payout_equality_iff() {
    blue_rose_payout_iff_at(0, &LIVE[..1], &OTHER[..1]);
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(80)]
pub fn blue_rose_payout_equality_user1_iff() {
    blue_rose_payout_iff_at(1, &LIVE[..1], &OTHER[..1]);
}
```

The card body is not modified. The spec helpers are not reworded; the only
spec-file change is the section comment and the executable doc: the `-1`
"rounding open" return stays, and IN-4 (proof B8) is what lets the harness
call with board prices without relying on luck.

---

## 7. Save / record / replay

None. The Kani host is not part of the engine, a save, a record or a replay
bundle:

* `SAVE_VERSION` unchanged — `rules/kani` has no durable state.
* `RECORD_VERSION` unchanged — no input shape.
* Archived engine bundles are unaffected; `card-sdk`'s `cfg(kani)` surface is
  not compiled into any bundle.
* `EngineStamp` hashes unchanged.

---

## 8. Versioning

* `ABI_VERSION` **unchanged (104 at the time of writing)**. No `Op` id, no
  payload, no import, no class definition changes.
* `SAVE_VERSION` / `RECORD_VERSION` unchanged (§7).
* The ruleset class table (`bandori_std::kind::class_defs`) is unchanged.
* Documentation touches: `docs/rulebook/spec/kani/r.md` (row -01 verdict,
  harness list), the `rules/kani/src/proofs/hostops_smoke.rs` module table,
  `docs/rulebook/spec/kani/ag.md` and `HOSTOPS-ROUND2.md` (the `is_shop` /
  `buyable` gap rows), `docs/rulebook/BACKLOG.md` (close the slug).
* `rules/kani/src/board.rs`'s `TILES` stays generated; only helper functions
  are edited.

---

## 9. Proofs

Every proof is `#[kani::proof]` with `#[kani::use_stub_set(LIB_COST_NOFMT)]`,
runs under `tools/kani/run.sh --unit r`, and has no `#[ignore]`. Model proofs
live in `proofs/hostops_smoke.rs` (the module's stated job is exactly "pin the
host model formulas"; conformance is `tests/conf_model.rs`). Body proofs live
in `proofs/r.rs` under their own section and use the seat helper pattern
(`*_iff_at(user)` called from two harnesses, per the module header).

`#[kani::unwind]`: 80 for every proof that runs the body or walks all 60 tiles
(the body scans `tile_count` = 60); 12 for fixed-tile proofs. Bounds are below
the runner default only where the loop is provably shorter.

Predicate → proof map (one obligation set per helper; no spec predicate is
left unasserted):

| spec predicate | proofs |
|---|---|
| `blue_rose_rich` | B1–B6, X2 |
| `blue_rose_payout` | B1–B3, B9, X1 |
| `blue_rose_payout_ceil10` | B8 (M6) |
| `blue_rose_payout_log_ok` | B7; asserted inside B1–B3 and X1 |
| `blue_rose_payout_mutant` | X1, X3 |

The model proofs install a four-op `tile_in_dispatch` (`tile_in` →
`arm_tile_in`, `owned_count` → `arm_owned_count`, `owned_at` →
`arm_owned_at`, `tile_count` → `arm_tile_count`) instead of the full
`dispatch`; every other op is a loud `outside_subset` FAIL. That keeps the
CBMC match small and turns "the proof needed an undeclared op" into a failure.

### 9.1 Model / capability (each maps to IN-1..IN-4)

| # | predicate | proof | assertion |
|---|---|---|---|
| M1 | IN-1 buyable | `tile_in_buyable_is_price_based` | 8×8 walk over all 60 tiles, owner −1 and owner 0: `dispatch(tile_in, "buyable") == (TILES[t].price > 0)`; `cover!` property / ring / agent; also asserts `price > 0 ⇔ kind ∈ {1,3}` |
| M2 | IN-1 livehouse | `tile_in_livehouse_is_buyable_and_colour_6` | for every tile: `livehouse == (buyable && color:6)`; covers 50 (true), 31 (false), 8 (true) |
| M3 | IN-1 colour | `tile_in_colour_class_uses_group` | `color:6` true for 31 and 50, false for 47; `color:10` true for 47; out-of-range tile → false |
| M4 | IN-2 witness | `owned_livehouse_witness` | tile 50 owner 0 → buyable / color:6 / livehouse all 1; `owned_count(0) == 1`, `owned_at(0, 0) == 50` |
| M5 | IN-2 end-to-end | `owned_deed_scan_yields_the_livehouse` | emulate the card's filter over `0..tile_count` with `is_buyable(-1)` + `is_color(user, ., 6)` intersected with `owned_at`: result is exactly `[50]` |
| M6 | IN-4 rounding | `ceil10_equals_exact_20pct_on_board_prices` | 8×8 walk: for every livehouse tile price `p`, `payout([p]) == payout_ceil10([p])`; plus symbolic `x > 0 ∧ x % 20 == 0 ⇒ ceil10(x) == x/5` with `x` in 20..=10_000 |

### 9.2 Clause (1) — spec `blue_rose_rich`

Each row is a pair: `…_iff` at seat 0 and `…_user1_iff` at seat 1.

| # | predicate / case | proof | assertion |
|---|---|---|---|
| B1 | rich, equality boundary (1 live, 1 other) | `blue_rose_payout_equality_iff` / `…_equality_user1_iff` | `blue_rose_rich(1,1)` true; exactly one `GainMoney{user, 280}`; money +280; no narration; frame |
| B2 | rich, majority (2 live, 1 other) | `blue_rose_payout_majority_iff` / `…_majority_user1_iff` | `blue_rose_rich(2,1)` true; exactly `GainMoney{user, 520}` (1400+1200 at 20%); frame |
| B3 | rich, sum over three deeds (3 live, 2 other) | `blue_rose_payout_sum_iff` / `…_sum_user1_iff` | `blue_rose_rich(3,2)` true; exactly `GainMoney{user, 1200}` (6000 at 20%) |
| B4 | rich, no deeds (0, 0) | `blue_rose_zero_deed_iff` / `…_zero_deed_user1_iff` | `blue_rose_rich(0,0)` true; **no** gain effect, money unchanged, narration 0 (proves the branch is (1), not (2)) |
| B5 | not rich, zero live (0 live, 2 other) | `blue_rose_not_rich_noop` / `blue_rose_not_rich_user1_noop` | `blue_rose_rich(0,2)` false; no GainMoney; money unchanged; narration 1 |
| B6 | not rich, one live / two other | `blue_rose_payout_below_threshold_noop` / `…_user1_noop` | `blue_rose_rich(1,2)` false; no GainMoney; money unchanged; narration 1 |

### 9.3 Clause (1) — spec `blue_rose_payout`, `payout_log_ok`, `payout_ceil10`

| # | predicate | proof | assertion |
|---|---|---|---|
| B7 | `payout` / `payout_log_ok` exactness | `blue_rose_payout_log_ok_is_exact` | pure, `n > 0`: `log_ok(effects, p, n)` is true only for exactly one `GainMoney{p, n}`; false for n±1, for the other seat, and for an extra `PayMoney` |
| B8 | `payout_ceil10` | `ceil10_equals_exact_20pct_on_board_prices` | M6 above: every livehouse price and the symbolic lemma |
| B9 | `payout` is the book's 20% | asserted inside B1–B3 | `amount == blue_rose_payout(&prices)` and `only_player_money_delta(..., amount)` |

### 9.4 Mutants (iff must be non-vacuous)

| # | mutant | proof | assertion |
|---|---|---|---|
| X1 | `blue_rose_payout_mutant` (25%) | `blue_rose_payout_mutant_is_detected` | [50] case: body pays 280, `!log_ok(effects, user, blue_rose_payout_mutant(&[1400]))` (350) and money delta ≠ 350 |
| X2 | strict `>` instead of `>=` | `blue_rose_rich_strict_mutant_is_detected` | equality case (1,1): `!rich_strict(1,1)` while the body logs the gain |
| X3 | `#[kani::should_panic]` amount mutant | `blue_rose_payout_mutant_350` | asserting the 25% spec against the body must FAIL (panic as expected) |

### 9.5 Native (non-Kani) obligations

`conf_hostops2` tests named in 4.5 plus the existing
`blue_rose_payout_contract` / `blue_rose_pride_pays_20pct_when_livehouses_dominate`
stay green. `conf_model`'s mock delegates to the shared table. These are
machine-checked under `cargo test`, not `#[ignore]`.

Verdicts are pasted into `docs/rulebook/spec/kani/r.md` exactly like the other
harnesses.

---

## 10. Alternatives

### A. Model the `class:` / `classFor:` tile-prop override; leave the static `buyable` quirk

Honour the production precedence (prop first, then the static table) and have
the harness stamp `class:buyable = 1` on the owned tile.

* **Pro:** exercises the documented per-player override (which the model
  ignores today), smallest change to the historical `buyable` answer, and it
  would also serve future class-stamp cards.
* **Con:** the static answer stays wrong, so every other card that reads
  `is_buyable` on an owned tile stays vacuous (`one_of_us` partner selection,
  savior, childhood — `kani/ag.md:21`). The proof world would be legal only
  because of a prop production does not need, which reads as a workaround. It
  also does not fix `color:6`, so multi-deed sums still need a second fix.
* **Verdict:** keep as the follow-up layer (open question OQ1) once the base
  table is right; not the vehicle for this item.

### B. New model-only op or world field (`Op::set_tile_buyable`, `World.buyable` override)

* **Pro:** explicit setup; no change to any existing class answer, so no other
  proof can move.
* **Con:** a new op that no guest body calls, plus a second meaning of
  "buyable" alongside `tile_in`; still needs the colour fix for sums; still
  leaves the production-vs-model divergence.
* **Verdict:** more surface than value for one static table.

### C. Model the purchase pipeline so the tile is acquired by playing a buy

* **Pro:** closest to how ownership arises in a real game.
* **Con:** `offer_buy` / `buy` / `acquire` are protocol stubs with no quoted
  price or ownership write (`kani/HOSTOPS-ROUND2.md`); this is a much larger
  item and the clause does not depend on how the deed was acquired.
* **Verdict:** separate future item.

### D. Demote the clause to PROTOCOL-LEVEL

* **Pro:** no model work.
* **Con:** clause (1) is pure arithmetic over an owned set — exactly what Kani
  is for — and the native tests already pin the engine. Leaving it
  PROTOCOL-LEVEL keeps the dead spec helpers.
* **Verdict:** rejected.

The recommended design is the static table correction (§4.2–4.3): it fixes the
root cause for every card, needs no new op, and is proven against the board by
M1–M3.

---

## 11. Open questions

1. **OQ1 — props now or later?** Should `class:` / `classFor:` and the colour
   props `anyColor` / `colorFor:<p>` be modelled in the same change? They are
   production precedence and would close the `toko-color` gap and enable the
   OPEN-BAND-PRETEND probe, but they widen the conformance proof beyond the
   prop-free restriction of IN-1. Recommendation: later, tracked.
2. **OQ2 — `buyable` definition.** Use `price > 0` (production
   `TileData::is_buyable`) or kind `property|ring`? They coincide on the
   current board; `price > 0` is production and is the recommendation, with M1
   asserting the equivalence so a future board change is visible.
3. **OQ3 — clause-(2) observability.** `card_move` is a stub, so a not-rich
   harness can only see "no gain + one narration". Is one narration assertion
   enough for the clause-(2) trigger, or should a later item make the move plan
   (destination, main-move latch) observable?
4. **OQ4 — the 8-deed cap.** `owned_tiles` truncates at 8 under Kani while
   `owned_count` counts all owners. A 9th owned non-Livehouse inflates `others`
   and a 9th Livehouse is invisible to `live_deeds`. Do we add a 9+ proof, or
   document the cap as a model bound (current plan)?
5. **OQ5 — card helper.** Should `livehouses()` switch to
   `ctx::is_live_house_for(player, t)`? Production semantics are identical, but
   the Kani subset would shrink from two ops to one. Card-side, so possibly
   outside G15.
6. **OQ6 — ruleset class customization.** Production's class membership is a
   ruleset-supplied table (`tile_class::set_class_defs`, seeded by
   `bandori_std::kind::class_defs`); the Kani model hard-codes the three
   registered classes. A custom ruleset that renames or adds a class would
   diverge. Is that a supported model target, or is the bandori-std table the
   fixed contract under Kani?
