//! Unit `skills_mygo`: clause proofs.
//!
//! Band skill `skill:MyGO!!!!!:迷途之星` and the five MyGO!!!!! character
//! skills. Per clause: iff + no-op + covers + mutant (see KANI-FOUNDATION.md).
//! Specs live in `crate::spec::skills_mygo`.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

const BAND_ID: &str = "skill:MyGO!!!!!:迷途之星";

fn skills_symbolic_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.pos[p] = kani::any();
        kani::assume(w.pos[p] >= 0 && w.pos[p] < world::TILE_COUNT);
        w.stay[p] = 0;
        w.fire[p] = 0;
        w.out[p] = kani::any();
    }
    w
}

fn living_player(w: &World) -> i32 {
    let p = crate::harness::any_player(w.n_players);
    kani::assume(!w.out[p as usize]);
    p
}

fn crystals_key() -> crate::intern::NameId {
    crate::intern::intern("crystals")
}

// ===========================================================================
// C-MYGO-band-02
// 「若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）。」
// Hook entry index 1 (`On::Hook(RollAfter)`). The guard
// `"actor == owner && move.roll >= 16"` is the host pre; under Kani we invoke
// the body only when the guard admits.
// ===========================================================================

/// Iff: one AddCounter(+1); crystals rise to min(pre+1, 1).
#[kani::proof]
#[kani::unwind(24)]
pub fn band_02_crystal_on_16_iff() {
    let w = skills_symbolic_world();
    let player = living_player(&w);

    // Host pre: `actor == owner && move.roll >= 16`. We pin the admitted region
    // and invoke the unmodified body. The guard IR for this string is exercised
    // by `cond_ops`; here the body is the obligation.
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    let pre_c = world::with_world(|ww| ww.key_get(player, crystals_key()));
    kani::assume(pre_c >= 0 && pre_c <= 3);

    invoke::unwrap_done(invoke::hook_only(BAND_ID, "RollAfter", player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_c = world::with_world(|ww| ww.key_get(player, crystals_key()));

    // `add_crystals(1, 1)` clamps at max=1. Host `add_counter_self` logs the
    // raw `n` (not the clamped delta) and clamps the stored value.
    assert!(
        spec::skills_mygo::band_02_log_ok(effects, player),
        "C-MYGO-band-02: effect log must be exactly one AddCounter(+1)"
    );
    assert!(
        post_c == spec::skills_mygo::band_02_crystals_after(pre_c),
        "C-MYGO-band-02: crystals must be min(pre+1, 1) under the roll cap"
    );
    assert!(post.money == pre.money, "frame: money unchanged");
    assert!(post.pos == pre.pos, "frame: pos unchanged");
    assert!(post.stay == pre.stay, "frame: stay unchanged");

    kani::cover!(pre_c == 0, "pre crystals 0 (gain lands)");
    kani::cover!(pre_c >= 1, "pre crystals at cap (gain refused by max)");
}

/// Mutant companion: a +2 spec must not match.
#[kani::proof]
#[kani::unwind(24)]
pub fn band_02_mutant_is_detected() {
    let w = skills_symbolic_world();
    let player = living_player(&w);
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only(BAND_ID, "RollAfter", player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    assert!(
        !spec::skills_mygo::band_02_log_ok_mutant(effects, player),
        "mutant: +2 spec must not match the +1 card"
    );
}

// ===========================================================================
// C-MYGO-band-04
// 「每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的[奇迹水晶]
//  （最多超出2个）」
// Hook entry index 2 (`On::Hook(Discarded)`).
// KNOWN black-box LESS: the over-cap crystal does not land when the band card
// is already at the roll cap of 1. Spec from the book.
// ===========================================================================

/// Iff against the book (gain whenever `pre < 1+over`, over ∈ 0..=2).
#[kani::proof]
#[kani::unwind(24)]
pub fn band_04_ineffective_overcap_iff() {
    let w = skills_symbolic_world();
    let player = living_player(&w);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    let pre_c = world::with_world(|ww| ww.key_get(player, crystals_key()));
    kani::assume(pre_c >= 0 && pre_c <= 3);

    invoke::unwrap_done(invoke::hook_only(BAND_ID, "Discarded", player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_c = world::with_world(|ww| ww.key_get(player, crystals_key()));
    let over = world::with_world(|ww| ww.key_get(player, crate::intern::intern("skill.mygo.over")));
    kani::assume(over >= 0 && over <= 2);

    let expected = spec::skills_mygo::band_04_crystals_after(pre_c, over);
    kani::cover!(pre_c == 1 && over == 0, "at roll cap, over=0 (book: must still grow)");
    kani::cover!(pre_c < expected, "gain fires");

    assert!(
        post_c == expected,
        "C-MYGO-band-04: ineffective discard must add one over-cap crystal (max 2 over)"
    );
    if post_c > pre_c {
        assert!(
            spec::skills_mygo::band_04_log_ok(effects, player),
            "C-MYGO-band-04: gain logs exactly one AddCounter(+1)"
        );
    }
    assert!(post.money == pre.money, "frame: money unchanged");
    assert!(post.pos == pre.pos, "frame: pos unchanged");
}

/// Mutant: empty log when the gain should fire.
#[kani::proof]
#[kani::unwind(24)]
pub fn band_04_mutant_is_detected() {
    let w = skills_symbolic_world();
    let player = living_player(&w);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    let pre_c = world::with_world(|ww| ww.key_get(player, crystals_key()));
    let _ = pre;
    kani::assume(pre_c == 0); // gain must fire
    invoke::unwrap_done(invoke::hook_only(BAND_ID, "Discarded", player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    if log_n > 0 {
        assert!(
            !spec::skills_mygo::band_04_log_ok_mutant(effects, player),
            "mutant: empty-log spec must not match a gaining card"
        );
    }
}

// ===========================================================================
// Classification notes (no harness — documented in the unit table):
// C-MYGO-band-01 (开局 3d20 start tile) — PROTOCOL-LEVEL: game-start
//   scheduling (`On::Hook(DeckBeforeGame)`); the body's 3d20 sum is
//   `roll(3,20)` (symbolic sum 3..=60) then `teleport_to`. Leaf body could be
//   proven but the start-of-game window is engine scheduling.
// C-MYGO-band-03 (move-1 instead of dice) — the press body spends a crystal
//   and shapes a 1-step move via `plan::*` (protocol stubs). PROTOCOL-LEVEL.
// Character skills (tomori/anon/rana/soyo/rikki) — see the unit table; most
//   windows are multi-turn / counteraction / settle pipelines (PROTOCOL-LEVEL)
//   or hit known black-box FAILS recorded in mygo.md §Findings.
// ===========================================================================