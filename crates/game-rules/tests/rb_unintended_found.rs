//! Findings filed by the Q4 unintended-interaction sweep
//! (`tests/q4_unintended.rs`).
//!
//! Each entry is a minimal reproduction of an unintended card / skill / tile
//! interaction. An entry whose gap is open stays `#[ignore = "DISCREPANCY:
//! unintended: …"]` so `cargo test` stays green, and the Q4 skip list
//! (`q4::KNOWN_FINDINGS`) points at its name so a default run does not
//! re-trip it. An entry whose gap is fixed is un-ignored and pins the fix;
//! its `KNOWN_FINDINGS` row goes with it.
//!
//! Black-box: nothing under `rules/cards/`, `rules/skills/`, `rules/fixtures/`
//! or `target/scratch/tainted/` is opened.

mod common;
mod fuzz;

use common::*;

// =====================================================================
// turn.play_from_hand leaks past the play and past the turn
// =====================================================================

/// `TurnCtx.play_from_hand` (C# `PlayCtx.FromDeck`) is set by
/// `play_from_hand` and only cleared by a nested `ctx::play_card`. It is
/// never reset when the play ends or when the turn ends, so after any hand
/// press the flag stays `true` for the rest of the match. 「若此卡从手牌
/// 以外的地方打出」 reads it, so a later card that should see `false`
/// (played from the draw pile / discard / a hook) sees the stale `true`
/// and takes the wrong branch -- an unintended coupling between every
/// hand-press and every later `play_from_hand` reader.
///
/// Repro: play one card, roll the turn over, read the flag back. The flag is
/// scoped to the play (cleared when the play ends), so it is already false
/// once `t.play` returns; the turn lap is the belt to that brace.
#[test]
fn play_from_hand_flag_leaks_past_the_turn() {
    let mut t = Table::vanilla(3);
    t.give(0, &["通用:GREAT"]);
    let _ = t.play(0, "通用:GREAT");
    assert!(
        !t.m.world().turn.play_from_hand,
        "flag is scoped to the play: cleared when the play ends"
    );
    // A full turn lap: every player's end / start boundary fires once. `end`
    // is refused before the main move (「err.roll_first」), so roll first.
    while t.prompt().is_some() {
        t.decline();
    }
    for _ in 0..3 {
        let who = t.turn();
        t.dice(&[3, 3, 3, 3, 3, 3, 3, 3]);
        let _ = t.roll(who);
        while t.prompt().is_some() {
            t.decline();
        }
        let _ = t.end(who);
    }
    assert!(
        !t.m.world().turn.play_from_hand,
        "turn.play_from_hand is still true after a full turn lap"
    );
}

// =====================================================================
// turn.abnormal is not reset for a one-shot rule
// =====================================================================

/// `TurnCtx.abnormal` is documented as "abnormal effects that got through
/// to each player this turn (indexed by player; a new turn starts them all
/// at 0)". `Mujica:心の雨` is a one-shot `[手]` play with no turn-start /
/// turn-end hook: it makes every player within 20 tiles roll and grants
/// [停留] on a low face. The [停留] is an 异常移动效果, so the engine
/// stamps `turn.abnormal` -- and the stamp must not survive a full turn lap.
/// A later reader of `turn.abnormal` (「本回合受到过异常移动效果」) then
/// sees a stale entry and fires when it should not.
#[test]
fn kokoro_no_ame_leaves_turn_abnormal() {
    let mut t = Table::vanilla(3);
    // Everyone close together so 「前方20格内」 catches the others.
    t.set_pos(0, 10);
    t.set_pos(1, 12);
    t.set_pos(2, 14);
    // Faces <= 12 grant [停留]; loaded dice guarantee it.
    t.dice(&[1, 1, 1, 1, 1, 1, 1, 1]);
    t.give(0, &["Mujica:心の雨"]);
    let _ = t.play(0, "Mujica:心の雨");
    // A full turn lap: every player's end / start boundary fires once. `end`
    // is refused before the main move (「err.roll_first」), so roll first; a
    // stayed player's roll is refused but its `end` still runs the boundary.
    while t.prompt().is_some() {
        t.decline();
    }
    for _ in 0..3 {
        let who = t.turn();
        t.dice(&[3, 3, 3, 3, 3, 3, 3, 3]);
        let _ = t.roll(who);
        while t.prompt().is_some() {
            t.decline();
        }
        let _ = t.end(who);
    }
    let ab = &t.m.world().turn.abnormal;
    assert!(
        ab.iter().all(|&v| v == 0),
        "turn.abnormal still shows a hit after the turn boundary: {ab:?}"
    );
}

// =====================================================================
// targeted is not reset past a full turn lap
// =====================================================================

/// `World::targeted` is "per player, times other players' cards targeted it
/// since its own turn last started". `通用:登上武道馆` is a multi-target
/// pay that [指定]s every other player. After a full turn lap each
/// player's own turn start should zero its counter.
/// A reader of 「本回合被其他玩家的卡[指定]过」 then over-fires.
#[test]
fn budokan_leaves_targeted_counters() {
    let mut t = Table::vanilla(3);
    for who in 0..3 {
        t.set_money(who, 20_000);
    }
    t.give(0, &["通用:登上武道馆"]);
    let _ = t.play(0, "通用:登上武道馆");
    // Answer the multi-target prompt with its default (pay everyone).
    for _ in 0..16 {
        t.settle();
        if t.prompt().is_none() {
            break;
        }
        t.decline();
    }
    // A full turn lap: every player's end / start boundary fires once. `end`
    // is refused before the main move (「err.roll_first」), so roll first.
    for _ in 0..3 {
        let who = t.turn();
        t.dice(&[3, 3, 3, 3, 3, 3, 3, 3]);
        let _ = t.roll(who);
        while t.prompt().is_some() {
            t.decline();
        }
        let _ = t.end(who);
    }
    let tg = t.m.world().targeted.clone();
    assert!(
        tg.iter().all(|&v| v == 0),
        "targeted[] still shows hits after a full turn lap: {tg:?}"
    );
}

// =====================================================================
// (a mark left by `skill:北泽育美:全垒打！` is **not** a finding: the
// skill text says 「每个回合在北泽精肉店生成一个可乐饼」, so the mark is
// a standing resource, not a turn-scoped leftover. The sweep's expiry
// check learned to skip marks for rules with an ongoing hook.)
// =====================================================================

// =====================================================================
// footprint gaps: a rule writes keyed state its footprint does not declare
// =====================================================================

/// `HHW:爱心义演` declares `writes: [money, roll.result]` but stamps two
/// keyed-state scratch slots (`charity_extra_total`, `charity_show_turn`)
/// on its owner. Those names are not in the footprint and not in any other
/// rule's declared vocabulary -- if another rule ever reads a
/// `charity_*` slot it is colliding on an undeclared name.
#[test]
fn charity_writes_undeclared_state() {
    let mut t = Table::vanilla(3);
    t.give(0, &["HHW:爱心义演"]);
    let _ = t.play(0, "HHW:爱心义演");
    let p = t.p(0);
    let charity: Vec<_> = p
        .state
        .keys()
        .filter(|k| k.starts_with("charity_"))
        .cloned()
        .collect();
    assert!(
        charity.is_empty(),
        "HHW:爱心义演 stamps undeclared state keys {charity:?}"
    );
}

/// `RAS:EXIST` declares `writes: [targeting, field, discard.pile, hand]`
/// but stamps `exist_used` on its owner. Same shape as the charity gap.
#[test]
fn exist_writes_undeclared_state() {
    let mut t = Table::vanilla(3);
    t.give(0, &["RAS:EXIST"]);
    let _ = t.play(0, "RAS:EXIST");
    let p = t.p(0);
    assert!(
        !p.state.contains_key("exist_used"),
        "RAS:EXIST stamps undeclared state key exist_used"
    );
}

// =====================================================================
// shared-name registry: a P✽P fan-name variant
// =====================================================================

/// `PP:初次演出事故` suppresses Pastel✽Palettes (2) skills and records the
/// suppression as a token. The token is written under **two** names in the
/// same activation -- `skillBlock:Pastel✽Palettes` (the band's real name,
/// with the star glyph) and `skillBlock:PP2` (an abbreviation). Any later
/// reader that looks up one of the two will miss the other, and the star
/// glyph is a third spelling (`Pastel*Palettes` / `PastelPalettes`) a
/// different rule could easily use. This is the P✽P fan-name variant the
/// registry is meant to catch.
#[test]
fn stage_accident_writes_two_skillblock_names() {
    let mut t = Table::vanilla(3);
    t.give(0, &["PP:初次演出事故"]);
    let _ = t.play(0, "PP:初次演出事故");
    let names: Vec<String> = t.p(0).tokens.iter().map(|c| c.name.clone()).collect();
    let blocks: Vec<String> = names
        .iter()
        .filter(|n| n.starts_with("skillBlock"))
        .cloned()
        .collect();
    // One concept, one name. Two names for the same suppression is the bug.
    let mut stems: Vec<String> = blocks
        .iter()
        .map(|n| n.trim_start_matches("skillBlock:").to_string())
        .collect();
    stems.sort();
    stems.dedup();
    assert!(
        stems.len() <= 1,
        "PP:初次演出事故 writes {} distinct skillBlock stems {stems:?} -- a name collision",
        stems.len()
    );
}