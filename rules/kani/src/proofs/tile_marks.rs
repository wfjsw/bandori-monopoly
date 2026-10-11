//! Unit `tile_marks`: clause proofs.
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
//!   `msg_a` / `msg_sender_uid` (protocol stubs return 0), so the handler
//!   cannot see the target tile, the count, or the provenance card.

use crate::invoke;
use crate::spec::tile_marks as spec;
use crate::world::{self, Effect, Mark, World, MAX_PLAYERS};


/// Call the unmodified `on_land` body of `mark:cp` via its `CardDef` (no
/// registry scan — see `proofs::tiles::call_settle`).
fn call_mark_cp_land(player: i32) {
    let def = rules_tile_marks::MARK_CP;
    for on in def.on.iter() {
        if let card_sdk::On::Hook(_, _, _, f) = on {
            invoke::unwrap_done(f(player));
            return;
        }
    }
    panic!("no On::Hook on mark:cp");
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
    let card = rules_tile_marks::MARK_CP;
    for on in card.on.iter() {
        if let card_sdk::On::Hook(_, _, Some(g), _) = on {
            return g(player);
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
#[kani::unwind(48)]
pub fn mark_cp_land_gain_iff() {
    let settler = any_seat();
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    let w = symbolic_cp_world(settler, tile);
    let pre_money = w.money[settler as usize];
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    call_mark_cp_land(settler);

    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    kani::cover!(log_n == 1, "exactly one observable effect (gain 800)");
    assert!(
        spec::mark_cp_land_gain_ok(effects, settler),
        "mark:cp on_land: observable effect must be exactly GainMoney{settler, 800}"
    );
    assert!(
        post.money[settler as usize] == pre_money + spec::CP_REWARD,
        "mark:cp on_land: settler gains exactly 800"
    );
    // Frame: no other seat's money moves; positions / fire / stay untouched.
    for p in 0..MAX_PLAYERS {
        if p != settler as usize {
            assert!(
                post.money[p] == pre.money[p],
                "mark:cp on_land: frame — other seats' money unchanged"
            );
        }
    }
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
    assert!(post.stay == pre.stay, "frame: stay");
}

/// MUTANT: the reference wants 900. Must be distinguishable.
#[kani::proof]
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
    assert!(
        spec::mark_cp_land_gain_ok_mutant_900(effects, settler),
        "mutant: the reward is 800, not 900"
    );
}

/// MUTANT: the reference pays another seat.
#[kani::proof]
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
    assert!(
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
    assert!(
        !admitted,
        "mark:cp guard must reject when the tile carries no [CP点]"
    );
    // Host would not call the body. Nothing to assert about effects.
}

/// NO-OP / guard: a cancelled settle (「将该次结算改为…」) skips the entry.
#[kani::proof]
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
    assert!(!admitted, "mark:cp guard must reject on a cancelled settle");
}

/// Guard / BLOCKED-HOST note: even with a [CP点] on the tile the shipped
/// guard's 「自己[场上]1个[CP点]」 conjunct reads `mark_src_at`, which the
/// Kani host currently stubs to −1 — so the guard cannot admit and the full
/// three-write IFF is vacuous until that op lands. This harness pins the
/// *earlier* conjuncts (tile CP present ⇒ the prefix of the guard is true
/// once `mark_src_at` works) by checking `count_marks` sees the mark.
#[kani::proof]
#[kani::unwind(16)]
pub fn mark_cp_land_tile_cp_visible_to_count_marks() {
    let settler = any_seat();
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    let w = symbolic_cp_world(settler, tile);
    world::reset_world(w);

    // `count_marks` is modeled: the mark we installed must be visible.
    let n = card_sdk::ctx::count_marks(tile, &card_sdk::abi::MarkFilter::any());
    assert!(n >= 1, "the installed [CP点] must be visible to count_marks");
    kani::cover!(n == 1, "exactly one unit");

    // The full guard still cannot admit while `mark_src_at` is a stub.
    let admitted = mark_cp_guard(settler);
    kani::cover!(!admitted, "guard still rejects (mark_src_at stub)");
}

// ===========================================================================
// mark:cp on_place — BLOCKED-HOST (`msg_tile` / `msg_a` / `msg_sender_uid`)
//
// Specs are in `spec/tile_marks.rs`. No harness: the message payload ops are
// protocol stubs that return 0, so `place_ok` always sees `(tile=0, count=0)`
// and rejects — proving anything about placement would be vacuous or would
// require faking the payload. Flip on when the host models the message ops.
// ===========================================================================
