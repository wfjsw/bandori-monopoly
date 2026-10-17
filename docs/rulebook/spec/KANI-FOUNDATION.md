# Kani foundation — unit-author template

**Audience:** ~20 parallel unit authors writing clause proofs.
**Ground fact:** the 规则书 clause docs. Never derive a spec from engine
behaviour. C# is not a spec.

**Companion docs**

| doc | what |
|---|---|
| [`KANI.md`](KANI.md) | pilot write-up, toolchain, cliffs, what Kani cannot prove (§6) |
| [`KANI-COVERAGE.md`](KANI-COVERAGE.md) | full per-card/skill/event op-coverage table |
| `tools/kani/README.md` | how to run a unit's harnesses |

**Fail-closed.** No `#[ignore]`, no skip list, no silent zeros on unmodeled
ops. A harness that cannot be proven yet is listed as **PROTOCOL-LEVEL** in
your unit table — it is not deleted, weakened, or vacuously proven.

---

## 1. Architecture

```
guest card / skill / event body  (unmodified Rust, `fn(i32) -> Asked`)
        |
        |  ctx::*  →  ctx::sys::*  →  card_sdk::kani_host::call(Op, [i32;6])
        v
rules/kani::host::dispatch   game vocabulary lives HERE (not in card-sdk)
        |
        v
rules/kani::world            symbolic world + effect log (bounded)
        ^
rules/kani::intern           u16 names (no String/memcmp under cfg(kani))
        ^
rules/kani::board            static 60-tile board from data/board.json
```

| layer | file | role |
|---|---|---|
| **world** | `rules/kani/src/world.rs` | symbolic players / piles / tiles / keys / effect log / snapshots |
| **board** | `rules/kani/src/board.rs` | static tile table (kind / group / price / house cost) |
| **intern** | `rules/kani/src/intern.rs` | u16 name ids (`MAX_INTERN=256`); `intern_or_fail` panics under Kani when exhausted |
| **host** | `rules/kani/src/host.rs` | `ruleset_*` backend: one `dispatch(Op, Args) -> i64`. Unmodeled → loud fail |
| **invoke** | `rules/kani/src/invoke.rs` | registry lookup of shipped `CardDef` / skill / event; calls the unmodified `On` body |
| **harness kit** | `rules/kani/src/harness.rs` | `clause_proof!`, `clause_mutant!`, `assert_frame!`, `cover_both!`, `any_player`, `any_in` |
| **shim** | `rules/kani/src/shim.rs` | `guard!` / `guard_id!` — real single-source guard verdict; `guard_assumed!` only for non-lowerable leftovers (greppable) |
| **guard IR** | `cond_irs` (generated into `$OUT_DIR` by `rules/kani/build.rs`, not committed) | 139 lowered CEL guards + const ids (`BY_ID`) |
| **ir_fill** | `rules/kani/src/ir_fill.rs` | general World → `IrCtx` (every field filled or documented) |
| **spec units** | `rules/kani/src/spec/<unit>.rs` | reference specs FROM the clause docs only |
| **proof units** | `rules/kani/src/proofs/<unit>.rs` | `#[kani::proof]` harnesses |
| **generic SDK** | `rules/card-sdk/src/kani_host.rs`, `ctx/sys_kani.rs` | `cfg(kani)` dispatch of every import; no game words |

`rules-kani` is a **ROOT** workspace member (crate name `rules-kani`). It is
deliberately not in the `rules/` wasm workspace. Run from the worktree root:

```sh
export CARGO_TARGET_DIR=/home/monopoly/kani-target-f   # NOT kani-target
export CARGO_BUILD_JOBS=8
cargo kani -p rules-kani --harness <name> --default-unwind 80 --no-unwinding-checks
# or, with the runner:
tools/kani/run.sh --unit pp --jobs 4 --timeout 900
```

Under `cfg(kani)` the real `extern "C-unwind"` host imports become ordinary
Rust funnels (`card_sdk::kani_host`). Production / wasm / `rules-native` are
unchanged (`cfg(not(kani))`).

---

## 2. Surface inventory (what guests actually call)

Scanned `rules/cards`, `rules/skills`, `rules/events`, `rules/tiles`,
`rules/tile_marks`, `rules/fixtures`: **306 files**, **219 unique ctx names**,
**4357 call-sites**. Full frequency + ctx→Op map: the generation notes below
and `/tmp` snapshots from the foundation pass; the **coverage table**
([`KANI-COVERAGE.md`](KANI-COVERAGE.md)) is the live per-id view.

### Top 40 by call-site (must-model first)

| rank | ctx name | count | notes |
|---:|---|---:|---|
| 1 | `log` | 589 | narration counter — **not** part of the effect frame |
| 2 | `trigger` | 246 | `trigger::*` module (pay / move / deal getters+setters) |
| 3 | `Dest` | 161 | enum, not an op |
| 4 | `plan` | 153 | `plan::*` move-plan structure (mostly PROTOCOL stubs) |
| 5 | `set_dest` | 144 | |
| 6 | `player_pos` | 106 | |
| 7 | `set_slot` | 90 | keyed state sugar |
| 8 | `player_out` | 88 | |
| 9 | `place_card` | 87 | → `place_card_at` |
| 10 | `add_tok` | 78 | → `add_held` |
| 11 | `slot` | 77 | |
| 12 | `ask_yes` | 77 | → `ask` (symbolic answers) |
| 13 | `gain` | 75 | |
| 14 | `tile_named` | 75 | |
| 15 | `tile_count` | 63 | |
| 16 | `state` | 60 | `state::{get,set,add,…}` |
| 17 | `tok` | 59 | → `count_held_name` |
| 18 | `is_placed` | 58 | |
| 19 | `tile_owner` | 54 | |
| 20 | `others` | 51 | → `others_count` + `others_at` |
| 21 | `roll` | 51 | symbolic dice |
| 22 | `crystals` | 49 | → `counter_self` |
| 23 | `add_crystals` | 48 | → `add_counter_self` |
| 24 | `card_move` | 48 | |
| 25 | `ask_pick` | 47 | |
| 26 | `draw` | 46 | |
| 27 | `player_count` | 46 | |
| 28 | `owned_tiles` | 45 | → `owned_count` + `owned_at` |
| 29 | `set_prop` | 45 | |
| 30 | `gain_fire` | 44 | |
| 31 | `transfer` | 42 | → `pay_to` |
| 32 | `spend_fire` | 41 | |
| 33 | `cards_in` | 40 | `[n, id…]` layout under Kani (no postcard) |
| 34 | `cant_move` | 34 | |
| 35 | `in_band` | 34 | |
| 36 | `skill_blocked` | 34 | keyed state (`skillBlock` token) |
| 37 | `pay` | 33 | |
| 38 | `ask_tile` | 30 | |
| 39 | `teleport_to` | 29 | |
| 40 | `money_of` | 27 | → `Op::money` |

**Types (not ops):** `Dest`, `CardPile`, `DeckPos`, `TriggerKind`, `PromptKind`.
Constants (`state::TURN_END`, `state::VALUE`, …) are arguments.

### Modeled vs unmodeled

* **Modeled** = an explicit `Op::*` arm in `host.rs::dispatch` that does *not*
  fall through to `unmodeled()`.
* **Unmodeled** = catch-all `other => unmodeled(other)` →
  `kani::assert(false, "unmodeled host op")` / native panic. **19** ops today:
  `rent_of`, `tile_at`, `set_tile_at`, `trig_price`, `trig_seller`,
  `trig_buy_kind`, `trig_roll_source`, `trig_move_kind`, `trig_move_resolve`,
  `trig_move_tag`, `trig_move_main`, `trig_move_dir`, `trig_move_remaining`,
  `trig_move_from`, `trig_move_total`, `trig_move_roll`,
  `trig_deal_owner`, `trig_deal_houses`, `trig_deal_mortgaged`.

### Hostops-2 model (2026-10-11) — see `kani/HOSTOPS-ROUND2.md`

* **Indexed invoke (required in proofs).** `invoke::play_idx(card_id!("…"), p)`
  and friends. `invoke::find` / string `play` / `entry` are **native-only** —
  the `RULESET` string scan is a CBMC unwinding cliff. Regenerate the id table
  with `tools/kani/gen_card_ids.py`; rewrite call sites with
  `tools/kani/migrate_invoke_idx.py`.
* **Per-card body trampolines (Kani, build-time).** `invoke::entry_idx` goes
  through `call_body_idx` → `call_card_N`, generated by `rules/kani/build.rs`
  (`gen_body_dispatch`) into `$OUT_DIR/body_dispatch_fn.rs` and `include!`d
  from `src/invoke.rs`. **Never committed** (same rule as `cond_irs.rs`; see
  `.gitignore`). A single shared `body_of(on)(player)` `code*` site makes
  CBMC expand an IF-chain over every address-taken `fn(i32) -> Asked`
  (874–4313 targets) — the `duobang_01_02` SAT cliff. Adding a card /
  skill / event / tile / mark only needs `gen_card_ids.py` + `BAND_START`;
  the trampolines regenerate on the next build.
* `state_set` field column: `a3` = 0 VALUE / 1 MAX / 2 MIN / 3 EXPIRES,
  `a4` = value (the old model wrote the field id as the value).
* `play_doubled` default is `-1` (no doubling); `ctx::n(k,v)` only doubles
  when the slot equals `k`.
* No-arg `trig_set_cancelled` / `trig_set_negate_effect`: the call **is** the
  effect (`sys_kani` packs `[0;6]` — never read `a0`).
* `tile_named` resolves every `board.json` name (`board::TILE_NAMES`).
* `neighbor` returns a **living seat** in turn order, not a board tile.
* stay / stun / exile are three fields (`key::STAY` / `STUN` / `EXILE` +
  `EXILE_TO`). `give_extra_turn` only records; scheduling is PROTOCOL.
* `add_counter*` clamps `(was+n).max(0)` then `min(max)` when `max > 0`;
  `gain_fire` clamps at `fire_max`.
* `is_shop` = buyable (property|ring) ∧ `group == 10`.
* `shuffle_into_deck` is a multiset move of hand/discard into draw
  (order symbolic / irrelevant).
* `place_card_on` ABI is `[player, tile, cp, cl, ptr, len]` — card name at
  `(a2,a3)`, landing tile at `a1`.
* **Protocol stubs** = modeled arms that only return `0` (move plan, offers /
  buy / build, multi-turn settle). They never hit `unmodeled`, so coverage
  counts them as modeled — but a clause that needs their *real* scheduling
  semantics is **PROTOCOL-LEVEL** (§6). Do not fake it.

---

## 3. How to write a harness FROM a clause doc

### Checklist

1. **Open the clause** in `docs/rulebook/spec/cards/<group>.md`,
   `events.md`, or the skills section. Copy the **Quote** into your spec file
   as a module doc. Clause id like `C-PP-aya-haruka-02` stays in the name.
2. **Write the spec first** in `rules/kani/src/spec/<unit>.rs` as a pure
   function over `(pre, effect_log, post) -> bool` (or the pieces: `*_x`,
   `*_amount`, `*_log_ok`, `*_post_ok`). Never look at engine behaviour.
   Trigger / Condition / Effect / Frame become the four obligations.
3. **Check coverage** in [`KANI-COVERAGE.md`](KANI-COVERAGE.md). If the id is
   `blocked-by` a nonempty set, either (a) add the host arm first, or (b) list
   the clause **PROTOCOL-LEVEL** / **blocked** in your unit table. Do not
   invent a zero.
4. **Build a symbolic pre-world** (`kani::any` + `assume`ed ranges) in
   `rules/kani/src/proofs/<unit>.rs`. Isolation assumes (e.g. "no
   `[共鸣]` in hand") must be stated in the module docs *and* `assume`d.
5. **Invoke the unmodified entry** via `invoke::` (`play`, `hook`, `entry`, …).
   Card sources are **not** copied or rewritten.
6. **Assert iff**: `post == spec(pre)` **and** `effect_log == exactly(spec_effects)`
   **and** (false trigger) log empty / frame unchanged.
7. **`kani::cover!` every branch** (poorest / not-poorest, pick A / pick B).
8. **MUTANT**: a harness whose spec is deliberately wrong (off-by-one,
   extra effect, two cards instead of one). Prefer a **reachable-disagreement**
   companion (`*_mutant_is_detected`: assert card ≠ wrong-spec) and/or
   `kani::cover!` of the disagreement; keep `#[kani::should_panic]` only if it
   actually reports SUCCESSFUL (panic as expected). If the mutant does *not*
   fail, the iff proof is vacuous — fix it.
9. **Run** `tools/kani/run.sh --unit <unit> --harness <name>` and paste the
   verdict into your unit table.

### Four proofs per clause

| # | name shape | obligation |
|---|---|---|
| 1 | `*_iff` | post == spec(pre) ∧ log exactly ∧ frame |
| 2 | `*_noop` / `*_empty_*` | trigger/condition false ⇒ log empty ∧ frame |
| 3 | covers | `kani::cover!` on each branch of the condition / pick |
| 4 | `*_mutant_*` | `#[kani::should_panic]` — spec wrong on purpose |

Use `clause_proof!` / `clause_mutant!` from `crate::harness` (they expand to
the attributes above) or write `#[kani::proof]` directly like the pilots.

---

## 4. Worked example A — money clause `C-PP-aya-haruka-02`

**Quote** (`docs/rulebook/spec/cards/pp.md`):

> 「（1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时
> 将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。」

**Card:** `PP:[丸山彩]憧憬的前方` — `On::Filter(PaySplitAdd)` (unmodified).
**Host pre** (`value > 0`) is the **single-source guard** `"actor == owner && value > 0"`.
Under Kani it is evaluated via the lowered IR (`guard_id!` + `guard!`) —
**never** `assume`d. **Isolation:** no `PP:[衍生]共鸣` in hand so clause (2)’s
stacked 1500 shave cannot fire.

### Spec (`rules/kani/src/spec/pilot.rs`, keep the shape for new units)

```rust
/// X = 100, or 200 once the owner holds >= 10 [P✽P粉丝] (either face).
pub fn aya_haruka_02_x(fans_pos: i32, fans_neg: i32) -> i32 {
    if fans_pos + fans_neg >= 10 { 200 } else { 100 }
}

/// Owner is the poorest living player (OPEN-ties reading).
pub fn is_poorest(money: &[i32; 4], out: &[bool; 4], n: i32, owner: i32) -> bool { /* … */ }

/// Post pay_amount the clause demands.
pub fn aya_haruka_02_amount(pre_amount: i32, x: i32) -> i32 {
    (pre_amount - x).max(0)
}

/// Exactly one effect: the pay-amount rewrite. Narration is not in the frame.
pub fn aya_haruka_02_log_ok(effects: &[Effect], expected_to: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::SetPayAmount { to } if to == expected_to)
}
```

### Iff + covers (`rules/kani/src/proofs/money_shave.rs`)

```rust
#[kani::proof]
#[kani::unwind(24)]
pub fn aya_haruka_02_shave_iff() {
    let w = symbolic_pay_world();          // N∈2..=4, money/out/fans symbolic
    let owner = w.trig_player;
    let pre_amount = w.pay_amount;

    // single-source guard: the real CEL string, const id, no assume
    let gid = guard_id!("actor == owner && value > 0");
    let ctx = ir_fill::fill_pay_add(&w, owner);
    let admitted = guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits");
    kani::cover!(!admitted, "guard rejects");
    if !admitted {
        // host would not call the body; world untouched
        return;
    }

    let pre = { world::reset_world(w); world::snapshot() };
    invoke::unwrap_done(invoke::aya_longing_pay_add(owner));   // unmodified body

    let post = world::snapshot();
    let effects = /* world log slice */;

    let x = spec::aya_haruka_02_x(pre.fans_pos[owner], pre.fans_neg[owner]);
    let poorest = spec::is_poorest(&pre.money, &pre.out, pre.n_players, owner);
    kani::cover!(poorest, "owner is poorest (shave applies)");
    kani::cover!(!poorest, "owner is not poorest (no-op)");

    if poorest {
        let expected = spec::aya_haruka_02_amount(pre_amount, x);
        assert!(post.pay_amount == expected, "pay amount must be max(pre-X, 0)");
        assert!(spec::aya_haruka_02_log_ok(effects, expected), "log exactly the rewrite");
    } else {
        assert!(log_n == 0, "non-poorest owner must produce no effects");
        assert!(post.pay_amount == pre_amount, "must not touch the amount");
    }
    // frame
    assert!(post.money == pre.money && post.pos == pre.pos && post.fire == pre.fire);
}
```

### No-op

The `else` branch above *is* the no-op proof (log empty + amount unchanged).
When the clause has a separate false-trigger path (entry not fired at all),
add a dedicated `*_noop` harness that does **not** call the entry and asserts
the log stays empty.

### MUTANT

```rust
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn aya_haruka_02_mutant_x_off_by_one() {
    // … same symbolic world …
    // spec wants X+1; the card does X. Kani must report FAIL.
    let x = spec::aya_haruka_02_x_mutant(pre.fans_pos[owner], pre.fans_neg[owner]);
    assert!(post.pay_amount == spec::aya_haruka_02_amount(pre_amount, x),
            "mutant: card must not match the off-by-one spec");
}
```

**Pilot result:** `aya_haruka_02_shave_iff` **SUCCESSFUL** (2/2 covers);
`aya_haruka_02_mutant_x_off_by_one` **SUCCESSFUL (panic as expected)** —
non-vacuous.

---

## 5. Worked example B — prompt clause `C-PP-aya-haruka-01`

**Quote:**

> 「[手]：将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。」

**Condition:** discard holds at least one card.
**Effect:** this card on the user’s field; exactly one discard card → hand.
**Prompt:** `ctx::ask_card` over the discard — Kani explores **every** index
via `w.symbolic_answers = true`.

### Spec

```rust
/// Place + exactly one pre-existing discard card into hand.
/// Any pick among the distinct discard cards is legal; the chosen one moves.
pub fn aya_haruka_01_effects_ok(
    effects: &[Effect], player: i32, this_card: u8, picked: u8,
    pre_discard: &[bool; MAX_CARDS],
) -> bool { /* PlaceCard + TakeFromPile + AddToHand, exactly */ }

pub fn aya_haruka_01_post_ok(/* … */) -> bool { /* hand/discard moved exactly `picked` */ }

/// Empty discard: place only.
pub fn aya_haruka_01_empty_discard_ok(effects: &[Effect], player: i32, this_card: u8) -> bool { /* … */ }
```

### Iff (all picks) + empty-discard + mutant

```rust
fn symbolic_play_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume((2..=4).contains(&w.n_players));
    // …
    w.symbolic_answers = true;   // every ask_* branch in one proof
    w
}

#[kani::proof]
#[kani::unwind(32)]
pub fn aya_haruka_01_prompt_all_picks_iff() {
    // pre: discard holds OTHER and/or EXTRA (symbolic)
    // invoke::aya_longing_play(player)
    // find `picked` from Effect::TakeFromPile
    kani::cover!(picked == 2, "pick OTHER");
    kani::cover!(picked == 3, "pick EXTRA");
    assert!(spec::aya_haruka_01_effects_ok(…), "place + exactly one discard->hand");
    assert!(spec::aya_haruka_01_post_ok(…), "post moves exactly the picked card");
}

#[kani::proof]
#[kani::unwind(24)]
pub fn aya_haruka_01_empty_discard() {
    // discard empty ⇒ place only
    assert!(spec::aya_haruka_01_empty_discard_ok(…), "empty discard: place only");
}

#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn aya_haruka_01_mutant_two_cards() {
    // reference wants two cards in hand — card must FAIL to match
    assert!(spec::aya_haruka_01_effects_ok_mutant(…), "mutant: not a two-card spec");
}
```

**Pilot status (general world):**
- `aya_haruka_01_empty_discard` **SUCCESSFUL** (~194 s, post-invoke reachable)
- `aya_haruka_01_prompt_all_picks_iff` **TIMEOUT** (1200 s) — `Vec<String>::contains` still memcmp-unrolls on the non-empty path
- `aya_haruka_01_mutant_two_cards` **TIMEOUT** (600 s) — same cliff

Strategy (a)+(e): frozen 2-card discard of distinct interned ids; pick
unrolled as two concrete worlds. Under `cfg(kani)`, `cards_in` returns a
1-byte interned-str facade (`rules/card-sdk/src/ctx.rs`) and `invoke::find`
returns the `CardDef` directly (a 200+ entry iterator scan never returned
under CBMC — that made earlier "SUCCESS" vacuous; fixed by reachability
bisect). Still blocked: heap `String` length stays symbolic so `memcmp` in
the unmodified `ids.contains` path unrolls to 30+. A still-blocked prompt is
a CBMC cliff, not evidence against the card.

---

## 6. Symbolic world API (`rules/kani/src/world.rs`)

```text
World
  players   n_players: i32          // assume 2..=4
            money/out/pos/fire/fire_max/stay/character_id/band_id
            player_keys: [NameMap; 4]     // tokens / held / keyed state
            band_crystals: [i32; 4]
            fans_pos / fans_neg           // P✽P粉丝(正)/(反) views
  piles     hand / deck / discard : [u16; MAX_CARDS] + _n   // interned ids
  field     field: [FieldCard; MAX_FIELD]  // uid, card, player, tile, crystals, props
            dest / last_uid / next_uid
  tiles     tiles: [TileState; 60]         // owner / houses / mortgaged / props
  marks     marks: [Mark; MAX_MARKS]       // tile, key, src_card, instance, n
  events    events: [u16; MAX_EVENTS]
  trigger   trig_player / trig_value / pay_amount / trig_kind / trig_target /
            trig_tile / trig_cancelled / trig_negate
  prompts   opt_n / next_answer / answers / symbolic_answers
  log       log: [Effect; MAX_LOG] + log_n     // the frame
            narration: [Narration; MAX_NARRATION]  // NOT the frame
  flags     unmodeled_op: i32
```

| helper | use |
|---|---|
| `World::new()` | empty world |
| `world::reset_world(w)` | install as the global cell |
| `world::snapshot()` | `Snapshot` for pre/post compare (pilot-compatible fields) |
| `world::with_world(_mut)` | read / write the global cell |
| `w.push(Effect::…)` | append to the effect log |
| `w.push_narration(player, key)` | narration only |
| `w.key_get/set/add` | generic named counters |
| `w.pile_push/take/count` | bounded pile ops |
| `w.sync_views()` | refresh `fans_*` from `player_keys` |
| `w.symbolic_answers = true` | every `ask_*` returns a fresh symbolic index |

### `Effect` log (the frame)

`SetPayAmount` · `Teleport` · `GainFire` · `GiveStay` · `GiveStun` ·
`GiveExile` · `PlaceCard` · `TakeFromPile` · `AddToHand` · `DiscardFromHand` ·
`Draw` · `GainMoney` · `PayMoney` · `PayTo` · `AddToken` · `SetToken` ·
`AddCounter` · `SetState` · `SetOwner` · `SetHouses` · `SetMortgaged` ·
`PlaceMark` · `EventExpire` · `UnplaceCard` · `MoveUnits`.

Specs assert **exactly** the listed effects, in order. Narration (`ctx::log`)
is a separate counter and is **not** part of the frame.

---

## 7. Bounds conventions

| bound | value | where |
|---|---|---|
| players | `n_players ∈ 2..=4` (slots 0..4) | `World::MAX_PLAYERS = 4` |
| pile / field arrays | `MAX_CARDS = 8`, `MAX_FIELD = 8` | `world.rs` |
| effect log | `MAX_LOG = 32` | `world.rs` |
| narration | `MAX_NARRATION = 8` | `world.rs` |
| named keys | `MAX_KEYS = 32` | `world.rs` |
| marks | `MAX_MARKS = 16` | `world.rs` |
| events | `MAX_EVENTS = 8` | `world.rs` |
| intern table | `MAX_INTERN = 256` | `card_sdk::intern` / `rules/kani/src/intern.rs` |
| unwind | `--default-unwind 80` + `--no-unwinding-checks` | `tools/kani/run.sh`, `KANI.md` §1 |
| money (typical) | `0..=10_000` | harness `assume` |
| fans (typical) | `0..=20` | harness `assume` |

`--no-unwinding-checks` is required for the fixed 64-byte `guest_eq` compare
and `memcmp` over `String::eq` inside unmodified card bodies. Every such loop
is statically bounded; do **not** disable checks on unbounded constructs.

Per-harness `#[kani::unwind(N)]` may be tighter than 80 (the money pilot uses
24). Prefer the smallest N that covers your loops; the runner still passes
`--default-unwind 80` as the floor for anything without an attribute.

---

## 8. When a clause is NOT Kani-expressible

**Do not fake it.** List it as **PROTOCOL-LEVEL** (or **blocked** by an
unmodeled op) in your unit table, with a one-line reason. Fail-closed means
the row stays visible.

| kind of clause | why | what would prove it | tag |
|---|---|---|---|
| multi-turn chains, [反击] windows, settle pipelines | engine scheduling is a concurrent replay-with-abort protocol | TLA+ / Alloy / SMT (Z3) over the chain/settle protocol (`docs/ENGINE.md`), with Kani leaf refinement | **PROTOCOL-LEVEL** |
| unbounded loops / “all future turns” | model is N≤4 and every loop is a fixed bound | inductive invariants (Kani loop contracts, experimental) | **PROTOCOL-LEVEL** |
| CEL guard strings | ~~compiled at ruleset build; Kani does not evaluate them~~ **resolved**: the lowered IR evaluates the same string under Kani | `guard_id!` + `guard!` against `ir_fill::fill_from_world` (no `assume`) | — (use `guard!`) |
| wasm-only paths (bump allocator, wasmi) | not modelled | native `rules-native` semantics are the stand-in (same Rust) | note in unit table |
| needs an unmodeled `Op` | `host.rs` catch-all | add a real `dispatch` arm (read `crates/game-rules/src/hostfns.rs` first) | **blocked-by** `Op` |
| needs a protocol stub’s real semantics | arm returns `0` on purpose | see `KANI.md` §6 | **PROTOCOL-LEVEL** |

Cite `KANI.md` §6 in the unit table row. Isolation assumes for stacked hooks
go in the module docs **and** as `kani::assume` — never hidden.

### `guard!` / `guard_id!` (the real path)

```rust
// Clause condition: `actor == owner && value > 0` (host pre) ∧ `poorest` (card residual).
let gid = guard_id!("actor == owner && value > 0"); // const id, compile-checked
let ctx = ir_fill::fill_from_world(&w, owner, card, placed, tile, &ov);
let admitted = guard!(gid, &ctx);   // real single-source verdict
if admitted {
    // unmodified body; assert post == spec(pre)
} else {
    // world untouched (no-op)
}
```

`guard_id!` resolves at **const time** via a literal-matching macro emitted
by `gen_cond_ir`; an unknown string is a **compile error**. `guard!` indexes
`generated::BY_ID[id]` — never a 139-way string scan (that is a CBMC
unwinding / OOM cliff). `guard_by_src` remains for host-side tests only.

### `guard_assumed!` (leftovers only)

```rust
// ONLY for a guard that gen_cond_ir cannot lower. List it here.
guard_assumed!(/* residual not in the CEL string */, "why");
```

Expands to `kani::assume` under `cfg(kani)`; native build is a no-op that
still typechecks. Greppable: `guard_assumed!` (marker
`crate::GUARD_ASSUMED_MARKER`). **Current leftover list: (none)** — every
inventory guard that the pilots need lowers. Do not use this to hide a
condition the proof is supposed to establish.

---

## 9. Harness kit reference (`rules/kani/src/harness.rs`)

| macro / fn | purpose |
|---|---|
| `clause_proof! { attrs; fn name() { body } }` | expand attrs + `#[kani::proof]` |
| `clause_mutant! { … }` | same + `#[kani::should_panic]` (prefer a reachable-disagreement companion) |
| `assert_frame!(pre, post, field…)` | every listed `Snapshot` field unchanged |
| `cover_both!(cond, "yes", "no")` | `kani::cover!` both sides |
| `any_player(n) -> i32` | symbolic player in `0..n` |
| `any_in(lo, hi) -> i32` | symbolic i32 in `lo..=hi` |
| `unit_mod!(a, b, …)` | declare unit modules (already done in `mod.rs`) |

`rules/kani/src/shim.rs`: `guard_id!("src")` (const id), `guard!(id, &ctx)`,
`guard_assumed!(cond, why)` (leftovers only), `GUARD_ASSUMED_MARKER`.

---

## 10. Unit table

Each unit author owns **only** `spec/<unit>.rs` + `proofs/<unit>.rs`.
`spec/mod.rs` and `proofs/mod.rs` are written once — do not edit them.

| unit | clause doc | crates | notes |
|---|---|---|---|
| `pp` | `docs/rulebook/spec/cards/pp.md` | `rules/cards/card-pp` | 19 cards |
| `ppp` | `docs/rulebook/spec/cards/ppp.md` | `rules/cards/card-ppp` | 17 cards |
| `r` | `docs/rulebook/spec/cards/r.md` | `rules/cards/card-roselia` | 17 cards |
| `ag` | `docs/rulebook/spec/cards/ag.md` | `rules/cards/card-ag` | 15 cards |
| `hhw` | `docs/rulebook/spec/cards/hhw.md` | `rules/cards/card-hhw` | 15 cards |
| `mor` | `docs/rulebook/spec/cards/mor.md` | `rules/cards/card-morfonica` | 15 cards |
| `ras` | `docs/rulebook/spec/cards/ras.md` | `rules/cards/card-ras` | 15 cards |
| `mygo` | `docs/rulebook/spec/cards/mygo.md` | `rules/cards/card-mygo` | 15 cards |
| `mujica` | `docs/rulebook/spec/cards/mujica.md` | `rules/cards/card-mujica` | 15 cards |
| `crychic` | `docs/rulebook/spec/cards/crychic.md` | `rules/cards/card-crychic` | 15 cards |
| `sumimi` | `docs/rulebook/spec/cards/sumimi.md` | `rules/cards/card-sumimi` | 11 cards |
| `general` | `docs/rulebook/spec/cards/general.md` | `rules/cards/card-general` | 14 cards |
| `events` | `docs/rulebook/spec/events.md` | `rules/events` | 28 events |
| `skills_bands` | skills sheets / `skills` section | `rules/skills/skill-bands` | 12 band skills |
| `skills_characters` | skills sheets / `skills` section | `rules/skills/skill-characters` | 54 character skills |
| `skills_pp` … `skills_sumimi`, `skills_general` | (per-band skill stubs) | — | empty module slots; put per-band skill proofs here only if you split them |
| `tiles` | tile rules / `data/board.json` | `rules/tiles` | 6 tile bodies |
| `tile_marks` | mark rules | `rules/tile_marks` | 1 mark (`cp`) |
| `money_shave` / `teleport` / `prompt` | pilot clauses | — | keep; do not rewrite pilots |

Pilot proofs (`money_shave`, `teleport`, `prompt`) stay where they are. New
clauses for PP / PPP go in `pp.rs` / `ppp.rs`.

---

## 11. Coverage summary

Full table: [`KANI-COVERAGE.md`](KANI-COVERAGE.md) (284 ids).
Definition: *fully modeled* = every host op the body reaches has an explicit
`dispatch` arm (does not hit `unmodeled`).

| kind | fully modeled | total | blocked |
|---|---:|---:|---:|
| card | 174 | 183 | 9 |
| skill | 51 | 66 | 15 |
| event | 28 | 28 | 0 |
| tile | 6 | 6 | 0 |
| mark | 1 | 1 | 0 |
| **all** | **260** | **284** | **24** |

### Blocked-by (unmodeled ops that actually appear)

| `Op` | rows | who |
|---|---:|---|
| `rent_of` | 9 | hhw, mygo, r, sumimi cards + 2 character skills |
| `trig_move_roll` | 8 | character skills (move-roll readers) |
| `tile_at` | 2 | hhw cards |
| `trig_move_main` | 2 | band + character skills |
| `trig_move_remaining` | 2 | character skills |
| `trig_price` | 1 | `skill:Roselia:对音乐的纯粹` |

Unblocking any of these means a new `host.rs` arm (read
`crates/game-rules/src/hostfns.rs` first) — never a silent `0` for something
the proof needs.

Per-unit blocked counts are in [`KANI-COVERAGE.md`](KANI-COVERAGE.md).

---

## 12. Runner

```sh
tools/kani/run.sh [--unit pp] [--harness name] [--jobs 4] [--timeout 900] [--out DIR]
```

* JSON + markdown results: harness, verdict `SUCCESSFUL|FAILED|TIMEOUT|UNMODELED`,
  time, unwind.
* Exits nonzero on any failure / timeout / unmodeled. Fail-closed.
* `CARGO_TARGET_DIR=/home/monopoly/kani-target-f`, `CARGO_BUILD_JOBS=8`.
* Details: `tools/kani/README.md`.

---

## 13. Rules of engagement (read once before your first PR)

1. **Spec from the book only.** Quote the clause in the spec module doc.
2. **Card bodies stay unmodified.** Invoke via `invoke::`.
3. **Iff + no-op + cover + mutant.** Four proofs. Mutant must panic.
4. **Frame is the effect log.** Narration is not the frame.
5. **Isolation assumes are visible.** Module docs + `kani::assume`.
6. **Unmodeled → loud fail.** Never a silent zero. `TODO(规则书)` if the book
   says more than the model.
7. **PROTOCOL-LEVEL is a real tag.** Multi-turn / chain-window / engine
   scheduling clauses go in the unit table with that tag (`KANI.md` §6).
8. **No `#[ignore]`.** Fail-closed. Fix or tag; do not hide.
9. **One unit, two files.** `spec/<unit>.rs` + `proofs/<unit>.rs` only.
10. **Run the runner** before you hand the unit back; paste verdicts into the
    unit table.

---

## Adding a host import (checklist — a miss breaks EVERY Kani proof)

A new `ruleset_*` import in `rules/card-sdk/src/ctx/sys.rs` must also be added
to the Kani side, or `card-sdk` stops compiling under `cfg(kani)` and every
harness FAILS in seconds (this happened with `join_redirect_target` /
`set_join_used`):

1. `rules/card-sdk/src/kani_host.rs` — an `Op` variant with the same slot.
2. `rules/card-sdk/src/ctx/sys_kani.rs` — the same-signature `pub fn` calling
   `kani_host::call_*` (`call_i32` / `call_unit` / ...).
3. `rules/kani/src/host.rs` — a faithful `dispatch` arm (follow
   `crates/game-rules/src/hostfns.rs`); engine-scheduling surfaces stay
   PROTOCOL-LEVEL and loud, never a silent `0`. Add the world field in
   `world.rs` if the op carries state.
4. A conformance test in `rules/kani/tests/conf_*.rs`.
5. Check: `bash tools/kani/run.sh --unit wrong_guard --harness real_guard_matches_clause_iff --jobs 1`
   must still verify. `tools/kani/central.sh` aborts the whole pass when a unit
   fails to compile in every harness.

Derived tables that are **generated at build time into `$OUT_DIR`** (never
committed — `.gitignore`):

| file | generator | include site |
|---|---|---|
| `cond_irs.rs` | `rules_cond::gen_ir::generate` via `rules/kani/build.rs` | `src/generated/mod.rs` |
| `body_dispatch_fn.rs` | `rules/kani/build.rs` `gen_body_dispatch` | `src/invoke.rs` |

`card_ids.rs` is still a committed generated file (`tools/kani/gen_card_ids.py`)
because `card_id!` must be a const macro visible to every crate that names a
card. Do not add more committed derived tables.
