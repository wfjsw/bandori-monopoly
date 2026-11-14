//! Unit `markers`: clause proofs.
//!
//! ## Classification gate (see `docs/rulebook/spec/kani/tiles.md`)
//!
//! `mark:cp` has two entries.
//!
//! * **`on_land` (SettleBody)** — the book wants three writes: drop one tile
//!   [CP点], drop one on-card [CP点] of the mark's `src`, and [获得]800 to the
//!   settler. Under the current Kani host:
//!   * `gain` is modeled (observable `GainMoney`);
//!   * `bump_mark_f` is a no-op stub;
//!   * `mark_src_at` always returns −1, so the guard's
//!     「自己[场上]1个[CP点]」 conjunct can never admit and the body never
//!     reaches `add_counter_at`.
//!
//!   The **gain half** is proven below. The mark / counter half is
//!   **BLOCKED-HOST** on `bump_mark_f` + `mark_src_at`.
//!
//! * **`on_place` (Message `"place"`)** — **BLOCKED-HOST** on `msg_tile` /
//!   `msg_arg0` / `msg_sender_uid` (protocol stubs return 0), so the handler
//!   cannot see the target tile, the count, or the provenance card.
//!
//! ## fire kernel (`docs/MARKERS.md` §9)
//!
//! The enforcement clause wants **iff proofs on the kernel bodies** with the
//! exact effect set (one counter write + one 效果适用 log + the right window,
//! no money/pos side effects) and **I2 as an assert**. The kernel bodies are
//! `bandori_std::fire::{gain, spend}`; the harnesses below drive them directly
//! over a held-row world (a live `charge` row carries the ownership, so no
//! persona instance is needed) and assert `spec::markers::fire_*`.

use card_sdk::kani_host::{Args, Backend};
use card_sdk::Msg;

use crate::host;
use crate::intern;
use crate::invoke;
use crate::spec::markers as spec;
use crate::world::{
    self, Effect, Mark, WindowPoint, World, MAX_PLAYERS, WINDOW_GAIN, WINDOW_RAISE, WINDOW_SPEND,
};

/// Call the unmodified `on_land` body of `mark:cp` via its `CardDef` (no
/// registry scan — see `proofs::tiles::call_settle`).
fn call_mark_cp_land(player: i32) {
    let def = rules_markers::MARK_CP;
    for on in def.on.iter() {
        if let card_sdk::On::Action(e) = on {
            // The body is erased (`*const ()`); its `drive` owns the cast.
            invoke::unwrap_done((e.drive)(player, e.body));
            return;
        }
    }
    panic!("no On::Action on mark:cp");
}

fn any_seat() -> i32 {
    let p: i32 = kani::any();
    kani::assume(p >= 0 && p < 2);
    p
}

fn effects() -> ([Effect; world::MAX_LOG], usize) {
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|w| w.log);
    let n = world::with_world(|w| w.log_n);
    (logbuf, n)
}

/// World with one [CP点] mark already on `tile` (the placement path is
/// BLOCKED-HOST, so we install the mark directly — same shape `place_mark`
/// would have written).
fn symbolic_cp_world(settler: i32, tile: i32) -> World {
    crate::host::install();
    let mut w = World::new();
    w.n_players = 2;
    w.trig_player = settler;
    w.trig_tile = tile;
    w.trig_cancelled = false;
    w.money[settler as usize] = kani::any();
    kani::assume(w.money[settler as usize] >= 0 && w.money[settler as usize] <= 10_000);
    // One tile [CP点] on `tile`. `src_card` is the placing 该清CP了 instance;
    // the host's `mark_src_at` currently ignores it (returns −1).
    w.marks[0] = Mark {
        tile,
        key: crate::intern::NameId::INVALID,
        src_card: u16::MAX,
        instance: 0,
        n: 1,
    };
    w.marks_n = 1;
    w
}

/// The shipped `mark:cp` SettleBody guard (`lands_on_cp`), invoked through the
/// registry so we evaluate the real predicate, not a restated one.
fn mark_cp_guard(player: i32) -> bool {
    let card = rules_markers::MARK_CP;
    for on in card.on.iter() {
        if let card_sdk::On::Action(e) = on {
            if let Some(g) = e.guard {
                return g(player);
            }
        }
    }
    false
}

// ===========================================================================
// mark:cp on_land — observable gain half
// 「…[获得]800资金。」
// ===========================================================================

/// IFF (observable half): when the body runs for `settler`, exactly one
/// `GainMoney{settler, 800}` and nothing else in the effect log.
///
/// Isolation: the host's `mark_src_at` is a stub (always −1), so this harness
/// invokes the unmodified `on_land` body directly (the registry guard can
/// never admit while `mark_src_at` is stubbed). The full three-write book
/// shape is `spec::mark_cp_land_effects_book` and stays BLOCKED-HOST.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(48)]
pub fn mark_cp_land_gain_iff() {
    let settler = any_seat();
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    let w = symbolic_cp_world(settler, tile);
    let pre_money = w.money[settler as usize];
    let condition = {
        world::reset_world(w);
        world::snapshot()
    };

    call_mark_cp_land(settler);

    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    kani::cover!(log_n == 1, "exactly one observable effect (gain 800)");
    crate::kassert!(
        spec::mark_cp_land_gain_ok(effects, settler),
        "mark:cp on_land: observable effect must be exactly GainMoney{settler, 800}"
    );
    crate::kassert!(
        post.money[settler as usize] == pre_money + spec::CP_REWARD,
        "mark:cp on_land: settler gains exactly 800"
    );
    // Frame: no other seat's money moves; positions / fire / stay untouched.
    for p in 0..MAX_PLAYERS {
        if p != settler as usize {
            crate::kassert!(
                post.money[p] == condition.money[p],
                "mark:cp on_land: frame — other seats' money unchanged"
            );
        }
    }
    crate::kassert!(world::arr4_eq(post.pos, condition.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, condition.charge), "frame: fire");
    crate::kassert!(post.stay == condition.stay, "frame: stay");
}

/// MUTANT: the reference wants 900. Must be distinguishable.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(48)]
#[kani::should_panic]
pub fn mark_cp_land_mutant_900() {
    let settler = any_seat();
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    let w = symbolic_cp_world(settler, tile);
    world::reset_world(w);
    call_mark_cp_land(settler);
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::mark_cp_land_gain_ok_mutant_900(effects, settler),
        "mutant: the reward is 800, not 900"
    );
}

/// MUTANT: the reference pays another seat.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(48)]
#[kani::should_panic]
pub fn mark_cp_land_mutant_wrong_seat() {
    let settler = any_seat();
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    let w = symbolic_cp_world(settler, tile);
    world::reset_world(w);
    call_mark_cp_land(settler);
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::mark_cp_land_gain_ok_mutant_wrong_seat(effects, settler),
        "mutant: the 800 goes to the settling player"
    );
}

// ===========================================================================
// mark:cp on_land — guard (the real `lands_on_cp` predicate)
// ===========================================================================

/// NO-OP / guard: with no [CP点] on the tile the entry does not fire.
/// We evaluate the shipped guard; the host then does not call the body
/// (the effect log of a *not-called* body is empty by construction).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn mark_cp_land_guard_rejects_without_tile_cp() {
    let settler = any_seat();
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    let mut w = symbolic_cp_world(settler, tile);
    // Remove the only mark: the tile carries no [CP点].
    w.marks_n = 0;
    world::reset_world(w);

    let admitted = mark_cp_guard(settler);
    kani::cover!(!admitted, "guard rejects: no [CP点] on the tile");
    crate::kassert!(
        !admitted,
        "mark:cp guard must reject when the tile carries no [CP点]"
    );
    // Host would not call the body. Nothing to assert about effects.
}

/// NO-OP / guard: a cancelled settle (「将该次结算改为…」) skips the entry.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn mark_cp_land_guard_rejects_when_cancelled() {
    let settler = any_seat();
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    let mut w = symbolic_cp_world(settler, tile);
    w.trig_cancelled = true;
    world::reset_world(w);

    let admitted = mark_cp_guard(settler);
    kani::cover!(!admitted, "guard rejects: settle cancelled");
    crate::kassert!(!admitted, "mark:cp guard must reject on a cancelled settle");
}

/// Guard / BLOCKED-HOST note: even with a [CP点] on the tile the shipped
/// guard's 「自己[场上]1个[CP点]」 conjunct reads `mark_src_at`, which the
/// Kani host currently stubs to −1 — so the guard cannot admit and the full
/// three-write IFF is vacuous until that op lands. This harness pins the
/// *earlier* conjuncts (tile CP present ⇒ the prefix of the guard is true
/// once `mark_src_at` works) by checking `count_marks` sees the mark.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn mark_cp_land_tile_cp_visible_to_count_marks() {
    let settler = any_seat();
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    let w = symbolic_cp_world(settler, tile);
    world::reset_world(w);

    // `count_marks` is modeled: the mark we installed must be visible.
    let n = card_sdk::ctx::count_marks(tile, &card_sdk::abi::MarkFilter::any());
    crate::kassert!(
        n >= 1,
        "the installed [CP点] must be visible to count_marks"
    );
    kani::cover!(n == 1, "exactly one unit");

    // The full guard still cannot admit while `mark_src_at` is a stub.
    let admitted = mark_cp_guard(settler);
    kani::cover!(!admitted, "guard still rejects (mark_src_at stub)");
}

// ===========================================================================
// mark:cp on_place — BLOCKED-HOST (`msg_tile` / `msg_arg0` / `msg_sender_uid`)
//
// Specs are in `spec/markers.rs`. No harness: the message payload ops are
// protocol stubs that return 0, so `place_ok` always sees `(tile=0, count=0)`
// and rejects — proving anything about placement would be vacuous or would
// require faking the payload. Flip on when the host models the message ops.
// ===========================================================================

// ===========================================================================
// fire kernel — `bandori_std::fire::gain`
// `data/rules.txt` 126: 「· 火罐：技能指示物，在任意时刻，您持有的火罐数量不能超过
// 持有上限」-- gain is partial at the cap and reports actuals
// (MARKERS.md §4(1), §8 I2/I4).
// ===========================================================================

/// Ops the gain kernel reaches: the owner lookup (no persona is bound in these
/// worlds — the live `charge` row carries ownership), the row reads, the
/// capped refusal narration, and the `markerGain` write verb.
crate::host_subset!(fire_gain_dispatch {
    persona_skill => arm_persona_skill,
    count_held | count_held_name => arm_count_held,
    held_max => arm_tok_max,
    log | effect => arm_log,
    gain_marker => arm_gain_marker,
});

/// `spend_marker` with the engine's 效果适用 line mirrored into the effect log
/// (`wasm_rules::spend_marker` posts the caller's message with value `-n`; the
/// Kani arm itself writes silently).
fn fire_spend_logged(a: Args) -> i64 {
    let [a0, a1, a2, a3, ..] = a;
    let _ = (a1, a2);
    let ret = host::arm_spend_marker(a);
    if ret == 1 {
        let key = intern::intern(bandori_std::fire::KEY);
        let n = a3;
        world::with_world_mut(|w| {
            w.push(Effect::AddToken {
                player: a0,
                key: key.0,
                n: -n,
            })
        });
    }
    ret
}

/// Ops the spend kernel reaches: owner lookup + row reads, the `ChargePaying`
/// raise over the whole cost, the temp read, and the `markerSpend` verb.
/// Hand-written (not `host_subset!`) because `spend_marker` maps to the local
/// log mirroring arm above.
fn fire_spend_dispatch(op: card_sdk::kani_host::Op, a: Args) -> i64 {
    use card_sdk::kani_host::Op;
    match op {
        Op::persona_skill => host::arm_persona_skill(a),
        Op::count_held | Op::count_held_name => host::arm_count_held(a),
        Op::held_max => host::arm_tok_max(a),
        Op::raise => host::arm_raise(a),
        Op::spend_marker => fire_spend_logged(a),
        Op::log | Op::effect => host::arm_log(a),
        other => host::outside_subset(other),
    }
}

fn windows() -> ([WindowPoint; world::MAX_WINDOWS], usize) {
    world::with_world(|w| (w.windows, w.windows_n))
}

fn narration_n() -> usize {
    world::with_world(|w| w.narration_n)
}

/// In-place 2-seat world with a **live** fire row on `user`
/// (`value = pre`, declared cap `cap`). A live row makes `writable` true
/// without a persona-skill instance; `cap >= 1` keeps `row_live` true even at
/// `pre == 0`.
///
/// The world is `World::new()` (already the `WORLD` initializer) plus the row,
/// **not** `sparse_play_world`: the fire kernels read only the charge row, the
/// seat positions and `out`, so the symbolic fans / `key_set` props of the
/// play world are pure symex cost here (the spend pair timed out at 1500/1800 s
/// on it, cf. `rules/kani/timeout_table.toml`). Money is still symbolic on both
/// seats so the frame check has something to move.
///
/// `World::new` ships `charge_max = 3` as a struct default, so both halves are
/// cleared first — a declared seat is a **row**, not the default.
fn fire_world(backend: Backend, user: i32, pre: i32, cap: i32) {
    debug_assert!(user >= 0 && user < 2);
    debug_assert!(pre >= 0 && pre <= cap);
    host::install_backend(backend);
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.charge = [0; MAX_PLAYERS];
        w.charge_max = [0; MAX_PLAYERS];
        w.charge[user as usize] = pre;
        w.charge_max[user as usize] = cap;
        // Both seats symbolic, zero-loop (the frame check compares them).
        w.money[0] = kani::any();
        kani::assume(w.money[0] >= 0 && w.money[0] <= 5000);
        w.money[1] = kani::any();
        kani::assume(w.money[1] >= 0 && w.money[1] <= 5000);
        let pos: i32 = kani::any();
        kani::assume(pos >= 0 && pos < 60);
        w.pos[user as usize] = pos;
    });
}

/// Frame: nobody's money / position moves, and the declared cap does not move.
fn assert_no_money_pos_cap_moved(condition: &world::Snapshot, cap: i32, user: i32) {
    let post = world::snapshot();
    for p in 0..MAX_PLAYERS {
        crate::kassert!(
            post.money[p] == condition.money[p],
            "fire kernel: frame — money unchanged"
        );
        crate::kassert!(post.pos[p] == condition.pos[p], "frame: pos");
    }
    crate::kassert!(
        world::with_world(|w| w.charge_max[user as usize]) == cap,
        "fire kernel: frame — the declared cap does not move"
    );
}

/// IFF: with room under the cap, `gain(n)` lands exactly `min(n, cap - have)`:
/// one `GainFire` of the actual, one `markerGain` window, no narration, I2
/// after, and no money / position side effects.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_gain_iff() {
    let user = any_seat();
    // Slim symbolic grid: cap is 1..=3 above `pre`, the request 1..=4, so both
    // the fits and the clamped branch are reachable without a 7×8×8 product
    // (symex volume, see `rules/kani/timeout_table.toml`).
    let pre = crate::harness::any_in(0, 2);
    let cap = crate::harness::any_in(pre + 1, pre + 3);
    let n = crate::harness::any_in(1, 4);
    fire_world(fire_gain_dispatch, user, pre, cap);
    let condition = world::snapshot();

    let landed = bandori_std::fire::gain(user, n, &Msg::new("proof")).unwrap();
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let (wins, win_n) = windows();
    let want = spec::fire_gain_actual(n, pre, cap);

    kani::cover!(want == n, "gain fits under the cap");
    kani::cover!(want < n, "gain is clamped by the cap");
    crate::kassert!(landed == want, "gain returns the actual landed");
    crate::kassert!(
        spec::fire_gain_ok(effects, user, pre, cap, n, post.charge[user as usize]),
        "gain: exactly one GainFire of the clamped actual"
    );
    // I2 (「在任意时刻」), as an assert.
    crate::kassert!(
        spec::fire_i2_holds(post.charge[user as usize], cap),
        "I2: the pool never sits above the declared cap"
    );
    // The `markerGain` window opens once, for the actual.
    crate::kassert!(
        spec::fire_one_window(&wins[..win_n], WINDOW_GAIN, user, bandori_std::fire::KEY, want),
        "gain: exactly one markerGain window with name + actual"
    );
    crate::kassert!(narration_n() == 0, "a landed gain narrates nothing");
    assert_no_money_pos_cap_moved(&condition, cap, user);
}

/// DEBUG (temporary): the same symbolic grid as `fire_gain_iff`, one assert
/// per window conjunct, so a failure names the wrong field. No snapshot /
/// effect reads -- the symptom is the window only, and `World` copies are the
/// bulk of the CBMC cost. Both debug harnesses are cheap enough for the unit;
/// they exist because `mark_*` harnesses sit under the six allocator
/// false-positive checks (see `docs/rulebook/spec/kani/tiles.md`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_gain_window_debug() {
    let user = any_seat();
    let pre = crate::harness::any_in(0, 2);
    let cap = crate::harness::any_in(pre + 1, pre + 3);
    let n = crate::harness::any_in(1, 4);
    fire_world(fire_gain_dispatch, user, pre, cap);
    let _ = bandori_std::fire::gain(user, n, &Msg::new("proof")).unwrap();
    let (wins, win_n) = windows();
    let want = spec::fire_gain_actual(n, pre, cap);
    kani::cover!(want == n, "fits");
    kani::cover!(want < n, "clamped");
    let w = &wins[..win_n];
    crate::kassert!(win_n == 1, "debug: window count");
    crate::kassert!(w[0].kind == WINDOW_GAIN, "debug: window kind");
    crate::kassert!(w[0].player == user, "debug: window player");
    crate::kassert!(
        w[0].name == intern::intern(bandori_std::fire::KEY).0,
        "debug: window name id"
    );
    crate::kassert!(
        intern::id_eq_str(intern::NameId(w[0].name), bandori_std::fire::KEY),
        "debug: window name bytes"
    );
    crate::kassert!(w[0].value == want, "debug: window value");
}

/// DEBUG (temporary): `fire_gain_iff` with **concrete** args, splitting the
/// failing name conjunct into (a) the stored id vs `intern(KEY)` and (b) the
/// byte compare, so a failure names which half is wrong. Full concrete is the
/// cheapest harness here; it was the one that pinned the old intern model's
/// byte mangle under the symbolic-seat path.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_gain_name_concrete_debug() {
    // Fully concrete world (no `kani::any` at all) so CBMC collapses to one
    // path: the point is the name value, not the grid.
    host::install_backend(fire_gain_dispatch);
    world::with_world_mut(|w| {
        w.n_players = 2;
        w.charge = [0; MAX_PLAYERS];
        w.charge_max = [0; MAX_PLAYERS];
        w.charge[0] = 1;
        w.charge_max[0] = 2;
        w.pos = [10, 20, 0, 0];
        w.money = [1000; MAX_PLAYERS];
    });
    let landed = bandori_std::fire::gain(0, 2, &Msg::new("proof")).unwrap();
    let (wins, win_n) = windows();
    let key = intern::intern(bandori_std::fire::KEY);
    crate::kassert!(landed == 1, "debug: the one point of room lands");
    crate::kassert!(win_n == 1, "debug: one window");
    // Covers, not asserts: an unreachable cover names a wrong value without
    // stopping the run. `charge` is the third `known_init` insert (bucket 49,
    // nothing else in the probe window).
    kani::cover!(wins[0].name == 49, "debug: name is charge's known_init slot");
    kani::cover!(wins[0].name == 0, "debug: name is slot 0");
    crate::kassert!(wins[0].name == key.0, "debug: stored name is the key id");
    crate::kassert!(
        intern::resolve(intern::NameId(wins[0].name)).len() == 6,
        "debug: stored name has the key length"
    );
    crate::kassert!(
        intern::id_eq_str(intern::NameId(wins[0].name), bandori_std::fire::KEY),
        "debug: bytes"
    );
}

/// NO-OP: at the cap the gain refuses before the write — nothing moves, no
/// window opens, and the refusal is narrated once (「不能超过持有上限」).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_gain_at_cap_noop() {
    let user = any_seat();
    let cap = crate::harness::any_in(1, 4);
    let n = crate::harness::any_in(1, 4);
    fire_world(fire_gain_dispatch, user, cap, cap);
    let condition = world::snapshot();

    let landed = bandori_std::fire::gain(user, n, &Msg::new("proof")).unwrap();
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let (_, win_n) = windows();

    kani::cover!(n > 1, "over-cap request");
    crate::kassert!(landed == 0, "a full pool lands nothing");
    crate::kassert!(log_n == 0, "a refusal logs no effect");
    crate::kassert!(post.charge[user as usize] == cap, "the pool did not move");
    crate::kassert!(win_n == 0, "no markerGain window without a write");
    crate::kassert!(narration_n() == 1, "the refusal is narrated once");
    assert_no_money_pos_cap_moved(&condition, cap, user);
    let _ = &logbuf;
}

/// NO-OP: a non-positive request returns 0 without a window or a log.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_gain_zero_request_noop() {
    let user = any_seat();
    let pre = crate::harness::any_in(0, 4);
    fire_world(fire_gain_dispatch, user, pre, 5);
    let condition = world::snapshot();

    let landed = bandori_std::fire::gain(user, 0, &Msg::new("proof")).unwrap();
    let (logbuf, log_n) = effects();
    let (_, win_n) = windows();
    crate::kassert!(landed == 0, "n == 0 lands nothing");
    crate::kassert!(log_n == 0, "n == 0 logs no effect");
    crate::kassert!(win_n == 0, "n == 0 opens no window");
    crate::kassert!(narration_n() == 0, "n == 0 narrates nothing");
    assert_no_money_pos_cap_moved(&condition, 5, user);
    let _ = &logbuf;
}

/// NO-OP / I1: a seat with no live row and no persona skill cannot gain.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_gain_undeclared_seat_noop() {
    let user = any_seat();
    // No row at all: `fire_world` clears both halves (`World::new` ships
    // `charge_max = 3` as a demo default, which would otherwise make the
    // "undeclared" seat writable) and `cap = 0` keeps `row_live` false.
    fire_world(fire_gain_dispatch, user, 0, 0);
    let landed = bandori_std::fire::gain(user, 2, &Msg::new("proof")).unwrap();
    let (logbuf, log_n) = effects();
    let (_, win_n) = windows();
    crate::kassert!(landed == 0, "no row / no skill lands nothing");
    crate::kassert!(log_n == 0 && win_n == 0, "no write, no window");
    let _ = &logbuf;
}

/// MUTANT: the reference forgets the cap. With 2/3 held, +5 must land 1, not 5.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn fire_gain_mutant_uncapped_is_detected() {
    let user = 0i32;
    fire_world(fire_gain_dispatch, user, 2, 3);
    let _ = bandori_std::fire::gain(user, 5, &Msg::new("proof")).unwrap();
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::fire_gain_mutant_uncapped(effects, user, 2, 5, post.charge[user as usize]),
        "mutant: the gain is clamped to the room, not pre + n"
    );
}

// ===========================================================================
// fire kernel — `bandori_std::fire::spend`
// MARKERS.md §3 / §10(2): spend is all-or-nothing, opens `ChargePaying` over
// the whole cost and then `markerSpend`, both before the write; temp first.
// ===========================================================================

/// The `ChargePaying` raise's wire spelling (`kind::ChargePaying`, wire 78).
const CHARGE_PAYING_NAME: &str = "r:bandori:78";

/// IFF: with enough pots, `spend(n)` opens the two windows, takes exactly `n`,
/// logs that one write, leaves no money/pos trail, and keeps I2.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_spend_iff() {
    let user = any_seat();
    // `pre` concrete: the pre range multiplies the symbolic `n` paths, and
    // this body does not consult `pre` beyond size. 3 keeps both covers
    // (`n == pre`, `n < pre`) satisfiable.
    let pre = 3;
    let n = crate::harness::any_in(1, pre);
    let cap = pre + 1;
    fire_world(fire_spend_dispatch, user, pre, cap);
    let condition = world::snapshot();

    let paid = bandori_std::fire::spend(user, n, &Msg::new("proof")).unwrap();
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let (wins, win_n) = windows();

    kani::cover!(n == pre, "spend the whole pool");
    kani::cover!(n < pre, "spend part of the pool");
    crate::kassert!(paid, "a covered spend succeeds");
    crate::kassert!(
        spec::fire_spend_ok(effects, user, pre, n, post.charge[user as usize], paid),
        "spend: exactly one AddToken of -n"
    );
    crate::kassert!(
        spec::fire_spend_windows_ok(&wins[..win_n], user, n, CHARGE_PAYING_NAME),
        "spend: ChargePaying then markerSpend, both before the write"
    );
    // I2 stays true after a spend.
    crate::kassert!(
        spec::fire_i2_holds(post.charge[user as usize], cap),
        "I2 after spend"
    );
    crate::kassert!(narration_n() == 0, "spend narrates nothing");
    assert_no_money_pos_cap_moved(&condition, cap, user);
}

/// NO-OP: a short pool refuses all-or-nothing — the windows still open, but
/// no pot moves and no effect is logged.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_spend_short_refused() {
    let user = any_seat();
    let pre = crate::harness::any_in(0, 4);
    let n = crate::harness::any_in(pre + 1, pre + 2);
    // A live cap keeps `writable` true even at pre == 0.
    let cap = pre + 2;
    fire_world(fire_spend_dispatch, user, pre, cap);
    let condition = world::snapshot();

    let paid = bandori_std::fire::spend(user, n, &Msg::new("proof")).unwrap();
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let (wins, win_n) = windows();

    kani::cover!(pre == 0, "empty pool");
    crate::kassert!(!paid, "a short pool refuses");
    crate::kassert!(log_n == 0, "a refusal logs no effect");
    crate::kassert!(post.charge[user as usize] == pre, "refusal moves nothing");
    crate::kassert!(
        spec::fire_spend_windows_ok(&wins[..win_n], user, n, CHARGE_PAYING_NAME),
        "the windows open even when the write refuses"
    );
    assert_no_money_pos_cap_moved(&condition, cap, user);
    let _ = &logbuf;
}

/// NO-OP: a non-positive spend returns true without windows or writes.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn fire_spend_zero_noop() {
    let user = any_seat();
    fire_world(fire_spend_dispatch, user, 3, 3);
    let paid = bandori_std::fire::spend(user, 0, &Msg::new("proof")).unwrap();
    let (logbuf, log_n) = effects();
    let (_, win_n) = windows();
    crate::kassert!(paid, "n == 0 is trivially paid");
    crate::kassert!(log_n == 0 && win_n == 0, "n == 0 writes and opens nothing");
    let _ = &logbuf;
}

/// MUTANT: the reference takes one pot too few.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn fire_spend_mutant_underspend_is_detected() {
    let user = 0i32;
    fire_world(fire_spend_dispatch, user, 2, 3);
    let _ = bandori_std::fire::spend(user, 1, &Msg::new("proof")).unwrap();
    let post = world::snapshot();
    crate::kassert!(
        spec::fire_spend_mutant_underspend(post.charge[user as usize], 2, 1),
        "mutant: 2 − 1 is 1, not 2"
    );
}

// ===========================================================================
// fire kernel — I2 over a symbolic pre/cap/request grid
// ===========================================================================

/// I2 as a total Kani assert over a symbolic pool, cap and request: whatever
/// the gain lands, the pool never exceeds the cap and never goes negative.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fire_i2_is_preserved_by_gain() {
    let user = any_seat();
    let pre = crate::harness::any_in(0, 2);
    let cap = crate::harness::any_in(pre, pre + 2);
    let n = crate::harness::any_in(1, 4);
    fire_world(fire_gain_dispatch, user, pre, cap);
    let _ = bandori_std::fire::gain(user, n, &Msg::new("proof")).unwrap();
    let post = world::snapshot();
    kani::cover!(post.charge[user as usize] == cap, "reached the cap");
    kani::cover!(post.charge[user as usize] < cap, "stopped below the cap");
    crate::kassert!(
        spec::fire_i2_holds(post.charge[user as usize], cap),
        "I2: value <= declared max at all times"
    );
}
