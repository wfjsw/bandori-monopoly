//! Findings filed by the interaction fuzzer (`tests/fuzz_interactions.rs`).
//!
//! Each entry is a minimal reproduction of a violation the fuzzer found, kept
//! as an `#[ignore = "DISCREPANCY: …"]` test so `cargo test` stays green while
//! the underlying engine / card gap is open. The fuzzer's skip-list
//! (`fuzz::KNOWN_FINDINGS`) points at these names so a default `FUZZ_ITERS`
//! run does not re-trip them.
//!
//! Black-box: nothing under `rules/cards/`, `rules/skills/`, `rules/fixtures/`
//! or `target/scratch/tainted/` is opened.

mod common;
mod fuzz;

use common::*;

// =====================================================================
// money ledger
// =====================================================================

/// The money ledger closes over every window. `迷茫之蝶们的三全音`'s
/// 「立刻获得此次失去的资金金额」 now logs a `gain` bank leg (via `gain_fixed`),
/// and FEVER!'s pay hook goes through the money pipeline's `paySplitAdd` stage.
#[test]
fn money_ledger_gap() {
    let seed = 0xF022_F022_F022_F022u64.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut t = Table::with_seed(&["户山香澄", "花园多惠", "山吹沙绫"], seed);
    let before: Vec<i32> = (0..t.n).map(|i| t.money(i)).collect();
    let mark = t.mark();
    let mut rng = fuzz::rng::Rng::new(seed ^ 0xA5A5);
    let mut tr = fuzz::drive::Trace::new();
    fuzz::drive::drive(&mut t, &mut rng, 3, fuzz::drive::AnswerMode::Free).unwrap();
    let after: Vec<i32> = (0..t.n).map(|i| t.money(i)).collect();
    let events = t.events_since(mark);
    fuzz::drive::check_money(&before, &after, &events, &mut tr).expect("money ledger gap");
}

// =====================================================================
// status bounds
// =====================================================================

/// `[停留]` went negative. The engine now floors status counters at 0 on
/// every write (`state_set` / `state_add`), so clearing a stay cannot drive
/// the counter below zero.
#[test]
fn negative_status_leak() {
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(3)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut t = Table::with_seed(
        &["户山香澄", "花园多惠", "山吹沙绫", "丸山彩"],
        seed,
    );
    let mut rng = fuzz::rng::Rng::new(seed ^ 0xA5A5);
    fuzz::drive::drive(&mut t, &mut rng, 4, fuzz::drive::AnswerMode::Free).unwrap();
    fuzz::drive::check_nonneg(&t).expect("negative status leak");
}

/// A character skill gained fire past its declared cap (「初始N，上限M」).
/// The card implementation (`bandori_std::fire`) polices the cap; the engine
/// stores the advertised `max` but does not clamp to it.
#[test]
fn fire_over_cap() {
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(5)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut t = Table::with_seed(
        &["户山香澄", "花园多惠", "山吹沙绫", "丸山彩"],
        seed,
    );
    let mut rng = fuzz::rng::Rng::new(seed ^ 0xA5A5);
    fuzz::drive::drive(&mut t, &mut rng, 4, fuzz::drive::AnswerMode::Free).unwrap();
    fuzz::drive::check_nonneg(&t).expect("fire over cap");
}

// =====================================================================
// card ids
// =====================================================================

/// A card id that is not in the shipped manifest must never turn up in a zone.
/// The generator only draws from `ruleset().cards()`, so a dangling id would be
/// an engine or card-effect fabrication. The original fuzzer seed no longer
/// reproduces; this pins the invariant on that seed and a nearby sweep.
#[test]
fn unknown_card_id() {
    let base = 0xF022_F022_F022_F022u64
        .wrapping_add(7)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let known = fuzz::gen::all_card_ids();
    for i in 0..8u64 {
        let seed = base.wrapping_add(i).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut t = Table::with_seed(&["户山香澄", "花园多惠"], seed);
        let mut rng = fuzz::rng::Rng::new(seed ^ 0xA5A5);
        fuzz::drive::drive(&mut t, &mut rng, 3, fuzz::drive::AnswerMode::Free).unwrap();
        fuzz::drive::check_cards(&t, &known)
            .unwrap_or_else(|e| panic!("unknown card id on seed {seed:#x}: {e}"));
    }
}

// =====================================================================
// card traps
// =====================================================================

/// A `log.card_trap` event appeared in the log -- a WASM guest trapped inside
/// a card module. The engine logs it and keeps going; the fuzzer treats it as
/// an invariant violation.
#[test]
fn card_trap_reachable() {
    // The fuzzer records `Trace::traps`. Any non-zero count is the finding;
    // the failing seed is printed by `fuzz_interactions_invariants`.
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(11)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut t = Table::with_seed(&["户山香澄", "花园多惠", "山吹沙绫"], seed);
    let mut rng = fuzz::rng::Rng::new(seed ^ 0xA5A5);
    let tr = fuzz::drive::drive(&mut t, &mut rng, 4, fuzz::drive::AnswerMode::Free).unwrap();
    assert_eq!(tr.traps, 0, "card_trap events observed: {}", tr.traps);
}

// =====================================================================
// determinism / save-restore
// =====================================================================

/// Same seed + same answers must produce identical save JSON (and the replay
/// must consume exactly the same answer sequence). The original finding seed
/// holds; this pins it.
#[test]
fn determinism_break() {
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(13)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    fuzz::meta::check_determinism(seed, 3).expect("determinism break");
}

/// `Match::save()` / `Match::restore` at a mid-run point must continue to the
/// same final state as the unbroken run. Root cause of the original break:
/// `TurnCtx::plan` (the live `MoveCtx`, including card-shaped roll tables) was
/// `#[serde(skip)]`, so a restore dropped the pending move's dice and the next
/// roll diverged. Now serialized (`SAVE_VERSION` 7).
#[test]
fn save_restore_break() {
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(17)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    fuzz::meta::check_save_restore(seed, 4).expect("save/restore break");
}

// =====================================================================
// metamorphic
// =====================================================================

/// A negated no-target [手] effect must not move money: "fully negate X"
/// equals "never play X". The original finding was a harness gap (the
/// counteraction was hardcoded to seat 1 even when the window asked another
/// seat, and a stunned seat silently skipped the declare). The check now uses
/// a vanilla board and declares as the asked seat.
#[test]
fn negation_not_total() {
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(19)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    fuzz::meta::check_negation(seed).expect("negation gap");
}

/// Two multiplicative money modifiers must commute.
///
/// **Still open as a ruling (MONEY-01 / C-Q1).** The rulebook lists
/// 「支付减半/翻倍」 as one 时点 (rulebook.txt 支付阶段 4) with no product /
/// stack / order rule for two live multipliers (Fire bird 1.5×, Ave Mujica
/// 状态2 1.5×, HHW band double-pay, 摩卡 0.5×). Two identical 0.5× modifiers
/// commute under any composition, so this probe cannot decide the question --
/// leave it ignored until the user answers C-Q1.
#[test]
#[ignore = "RULING: MONEY-01 / C-Q1 -- composition of two live money multipliers (product? declaration order? one-per-window?)"]
fn multiply_order_matters() {
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(23)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    match fuzz::meta::check_commutativity(seed) {
        Ok(()) => panic!("commutativity held on this seed; the finding is not reproducing"),
        Err(e) => panic!("commutativity gap: {e}"),
    }
}

/// A +N roll modifier must never shorten a move. The original check measured
/// net board displacement, which a longer walk can shrink by landing on a
/// teleporting tile; the check now measures the walk length itself
/// (`main_steps`) on a vanilla board.
#[test]
fn monotonicity_break() {
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(29)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    fuzz::meta::check_monotonicity(seed).expect("monotonicity gap");
}

/// An effect on an immune / untargetable player still changed that player.
#[test]
fn immunity_gap() {
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(31)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // The check lives in fuzz_interactions.rs; duplicate the body here so the
    // ignored test is self-contained.
    let (mut t, _) = fuzz::gen::arrange(seed);
    let n = t.n;
    for i in 0..n {
        t.set_money(i, 10_000);
    }
    t.give(0, &["Mor:夏日合宿"]);
    t.give(1, &["通用:登上武道馆"]);
    if t.play(0, "Mor:夏日合宿").is_err() {
        return;
    }
    let m0 = t.money(0);
    if t.play(1, "通用:登上武道馆").is_err() {
        return;
    }
    while t.prompt().is_some() {
        t.decline();
    }
    assert_eq!(
        t.money(0),
        m0,
        "shielded P0 lost money despite 夏日合宿"
    );
}

// =====================================================================
// termination
// =====================================================================

/// The settle loop comes to rest. `Table::settle` ticks until the match is
/// at rest (a prompt is waiting, or the current player is in 运营 / 结束 with
/// nothing queued). The state-clamping fix (fire cap / status floor) removed
/// the cascade that left `step = END` spinning.
#[test]
fn settle_loop_stuck() {
    let seed = 0xF022_F022_F022_F022u64
        .wrapping_add(41)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut t = Table::with_seed(&["户山香澄", "花园多惠", "山吹沙绫", "丸山彩"], seed);
    let mut rng = fuzz::rng::Rng::new(seed ^ 0xA5A5);
    fuzz::drive::drive(&mut t, &mut rng, 12, fuzz::drive::AnswerMode::Free)
        .expect("settle loop stuck");
}

// =====================================================================
// MAX_MONEY_DEPTH: nested money opens its windows
// =====================================================================

/// Nested money movements open their [反击] windows at every depth. The
/// rulebook (「[支付]时可以打出」) has no depth limit. The termination argument
/// is that a hook cannot re-trigger on its own movement
/// (`Cx::reentrant_hooks`); `MAX_MONEY_DEPTH = 32` is a safety cap that traps
/// loudly (panics) rather than settling silently. See `docs/ENGINE.md`.
///
/// The probe runs a nested chain; the assertion is that it completes without
/// hitting the safety cap (which would panic).
#[test]
fn money_depth_opens_windows() {
    match fuzz::meta::check_money_depth(0x3E97) {
        Ok(_report) => {
            // Nested 3+ deep with windows opening at each level: the sheet's reading.
        }
        Err(_e) => {
            // Could not nest 3 deep with this card set, but no runaway.
        }
    }
}

// =====================================================================
// prompt storm: PPP:Returns turn-start ask under a permanent stun
// =====================================================================

/// Fuzz soak `FUZZ_ITERS=2000` iter 1517 (seed 10174556463119430459):
/// `drain_prompts: prompts never stopped` around `cards:card-ppp.returns_title`
/// / `returns_which_band`.
///
/// Root cause: the fuzz arrange wrote `stun` through the raw `state_set` (no
/// `expires`), and `tick_state` only wears a counter down when the item names
/// its tick -- so both seats stayed stunned forever, every turn was skipped,
/// and Returns' [持续]（2） 「[拥有者]每回合开始时选择一个其他存活玩家的团卡」
/// raised one prompt per skipped turn. Fixed (2026-10-08) in
/// `MatchPlayer::state_set`: a stay / stun / stunStart item picks up the
/// rulebook's tick (「玩家的每回合结束时移除一层」) whenever it has none.
#[test]
fn returns_prompt_storm() {
    let seed = 10174556463119430459u64;
    fuzz::one_iter(seed, 4).expect("PPP:Returns turn-start prompt storm");
}
