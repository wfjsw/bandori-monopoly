//! STA-02: the [不可阻挡] holder's per-effect choice
//! (「处于该状态时可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效」).
//!
//! Source: `docs/rulebook/cards.json`, glossary 14 ([不可阻挡]) / `data/rules.txt`
//! 51. Design: G12 §5.1 / §9.1 and `docs/rulebook/spec/abi/ab-unstoppable-choice.md`.
//!
//! The engine raises `abnormalChoice` on the holder whenever the
//! `abnormalGate` closed for a *received* movement abnormal (kinds 4–7); the
//! `status:unstoppable` body asks the holder yes/no:
//!
//! * yes -- the effect lands (a teleport moves, a forced move walks, a stop /
//!   reverse lands its marker), no `abnormalRefused`;
//! * no -- today's refusal path: the effect does not land and
//!   `log.unstoppable_skip` is logged;
//! * a self-caused move (`by == actor`) never prompts.
//!
//! Every case answers the open prompt explicitly (the replay re-runs the body
//! and re-reads the recorded answer), rather than relying on `skip_all`.

use rules_spec::{forall, Table};

const UNSTOPPABLE: &str = bandori_std::state_key::UNSTOPPABLE;

/// The prompt title the choice window raises.
const CHOICE_TITLE: &str = "ask.unstoppable_choice.title";

/// Drain every open prompt (the fallback is 「否」 for the choice too).
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// End `who`'s turn without a main move, then drain to the next seat.
fn pass_turn(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

/// Did the abnormal refusal hook fire after `mark`?
fn refused(t: &Table, mark: i32) -> bool {
    t.keys_since(mark)
        .iter()
        .any(|k| k.contains("abnormalRefused"))
}

/// Did the refusal's skip line fire after `mark`?
fn skipped(t: &Table, mark: i32) -> bool {
    t.keys_since(mark)
        .iter()
        .any(|k| k.contains("log.unstoppable_skip"))
}

/// Player 0 holds [不可阻挡]; hand the turn to player 1, who will deliver.
fn unstoppable_victim() -> Table {
    let mut t = Table::vanilla(2);
    t.set_state(0, UNSTOPPABLE, 1);
    t.set_pos(0, 5);
    t.set_pos(1, 10);
    pass_turn(&mut t, 0);
    t
}

/// The choice prompt is open on the holder and nothing else.
fn expect_choice(t: &Table) {
    let p = t.expect_prompt();
    assert_eq!(
        p.title.key(),
        CHOICE_TITLE,
        "expected the [不可阻挡] choice prompt: {}",
        t.dump_prompt()
    );
    assert_eq!(t.asked(), vec![0], "the choice is the holder's only");
    assert_eq!(p.options.len(), 2, "yes / no");
}

// =====================================================================
// received [传送] (kind 4)
// =====================================================================

/// 「可选择受到的[传送]…效果是否生效」 -- yes lets the received teleport land.
#[test]
fn received_teleport_yes_lands() {
    forall(4, 0xa5a2_0001, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:teleporter"]);
        let mark = t.mark();
        t.play(1, "TEST:teleporter")
            .map_err(|e| format!("play rejected: {e}"))?;
        expect_choice(&t);
        t.answer_one(0).map_err(|e| format!("yes: {e}"))?;
        drain(&mut t);
        if t.pos(0) == 5 {
            return Err(format!(
                "teleport did not land: pos {} (events {:?})",
                t.pos(0),
                t.recent_keys(20)
            ));
        }
        if refused(&t, mark) {
            return Err(format!(
                "the allowed teleport still refused: {:?}",
                t.recent_keys(20)
            ));
        }
        Ok(())
    })
    .expect("STA-02 teleport yes");
}

/// 「可选择受到的[传送]…效果是否生效」 -- no keeps the refusal and its skip line.
#[test]
fn received_teleport_no_refuses_and_logs() {
    forall(4, 0xa5a2_0002, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:teleporter"]);
        let mark = t.mark();
        t.play(1, "TEST:teleporter")
            .map_err(|e| format!("play rejected: {e}"))?;
        expect_choice(&t);
        t.answer_one(1).map_err(|e| format!("no: {e}"))?;
        drain(&mut t);
        if t.pos(0) != 5 {
            return Err(format!("teleport landed despite no: pos {}", t.pos(0)));
        }
        if !refused(&t, mark) {
            return Err(format!("no abnormalRefused: {:?}", t.recent_keys(20)));
        }
        if !skipped(&t, mark) {
            return Err(format!("no unstoppable skip line: {:?}", t.recent_keys(20)));
        }
        Ok(())
    })
    .expect("STA-02 teleport no");
}

// =====================================================================
// received [强制移动] (kind 5)
// =====================================================================

/// 「可选择受到的[强制移动]…效果是否生效」 -- yes walks the forced move.
#[test]
fn received_forced_move_yes_walks() {
    forall(4, 0xa5a2_0003, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:forcer"]);
        let mark = t.mark();
        t.play(1, "TEST:forcer")
            .map_err(|e| format!("play rejected: {e}"))?;
        expect_choice(&t);
        t.answer_one(0).map_err(|e| format!("yes: {e}"))?;
        drain(&mut t);
        if t.pos(0) == 5 {
            return Err(format!(
                "forced move did not walk: pos {} (events {:?})",
                t.pos(0),
                t.recent_keys(20)
            ));
        }
        if refused(&t, mark) {
            return Err(format!(
                "the allowed forced move still refused: {:?}",
                t.recent_keys(20)
            ));
        }
        Ok(())
    })
    .expect("STA-02 forced yes");
}

/// 「可选择受到的[强制移动]…效果是否生效」 -- no keeps the walk unmade.
#[test]
fn received_forced_move_no_refuses_and_logs() {
    forall(4, 0xa5a2_0004, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:forcer"]);
        let mark = t.mark();
        t.play(1, "TEST:forcer")
            .map_err(|e| format!("play rejected: {e}"))?;
        expect_choice(&t);
        t.answer_one(1).map_err(|e| format!("no: {e}"))?;
        drain(&mut t);
        if t.pos(0) != 5 {
            return Err(format!("forced move walked despite no: pos {}", t.pos(0)));
        }
        if !refused(&t, mark) {
            return Err(format!("no abnormalRefused: {:?}", t.recent_keys(20)));
        }
        if !skipped(&t, mark) {
            return Err(format!("no unstoppable skip line: {:?}", t.recent_keys(20)));
        }
        Ok(())
    })
    .expect("STA-02 forced no");
}

// =====================================================================
// received [强制停下] (kind 6) / reverse (kind 7): the caller-side gate
// =====================================================================

/// 「可选择受到的[强制停下]…效果是否生效」 -- yes lands the caller's stop.
#[test]
fn received_stop_yes_lands() {
    forall(4, 0xa5a2_0005, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:stopper"]);
        let mark = t.mark();
        t.play(1, "TEST:stopper")
            .map_err(|e| format!("play rejected: {e}"))?;
        expect_choice(&t);
        t.answer_one(0).map_err(|e| format!("yes: {e}"))?;
        drain(&mut t);
        if t.state(0, "TEST:stopped") != 1 {
            return Err(format!(
                "stop did not land: marker {} (events {:?})",
                t.state(0, "TEST:stopped"),
                t.recent_keys(20)
            ));
        }
        if refused(&t, mark) {
            return Err(format!(
                "the allowed stop still refused: {:?}",
                t.recent_keys(20)
            ));
        }
        Ok(())
    })
    .expect("STA-02 stop yes");
}

/// 「可选择受到的[强制停下]…效果是否生效」 -- no keeps the caller's stop off.
#[test]
fn received_stop_no_refuses_and_logs() {
    forall(4, 0xa5a2_0006, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:stopper"]);
        let mark = t.mark();
        t.play(1, "TEST:stopper")
            .map_err(|e| format!("play rejected: {e}"))?;
        expect_choice(&t);
        t.answer_one(1).map_err(|e| format!("no: {e}"))?;
        drain(&mut t);
        if t.state(0, "TEST:stopped") != 0 {
            return Err(format!(
                "stop landed despite no: marker {}",
                t.state(0, "TEST:stopped")
            ));
        }
        if !refused(&t, mark) {
            return Err(format!("no abnormalRefused: {:?}", t.recent_keys(20)));
        }
        if !skipped(&t, mark) {
            return Err(format!("no unstoppable skip line: {:?}", t.recent_keys(20)));
        }
        Ok(())
    })
    .expect("STA-02 stop no");
}

/// The reverse rides the same gate and the same choice.
#[test]
fn received_reverse_yes_lands() {
    forall(4, 0xa5a2_0007, |_rng, _i| {
        let mut t = unstoppable_victim();
        t.give(1, &["TEST:reverser"]);
        t.play(1, "TEST:reverser")
            .map_err(|e| format!("play rejected: {e}"))?;
        expect_choice(&t);
        t.answer_one(0).map_err(|e| format!("yes: {e}"))?;
        drain(&mut t);
        if t.state(0, "TEST:reversed") != 1 {
            return Err(format!(
                "reverse did not land: marker {} (events {:?})",
                t.state(0, "TEST:reversed"),
                t.recent_keys(20)
            ));
        }
        Ok(())
    })
    .expect("STA-02 reverse yes");
}

// =====================================================================
// self-caused moves stay unprompted
// =====================================================================

/// 「受到的」 excludes a move the holder caused itself: the forced walk
/// (`TEST:mover` runs its own `run_planned_move`) never opens the choice and
/// simply walks.
#[test]
fn self_caused_forced_move_does_not_prompt() {
    forall(4, 0xa5a2_0008, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_state(0, UNSTOPPABLE, 1);
        t.set_pos(0, 5);
        t.give(0, &["TEST:mover"]);
        t.play(0, "TEST:mover")
            .map_err(|e| format!("play rejected: {e}"))?;
        let mut saw_choice = false;
        while let Some(p) = t.prompt() {
            if p.title.key() == CHOICE_TITLE {
                saw_choice = true;
            }
            t.decline();
        }
        if saw_choice {
            return Err(format!(
                "self-caused move prompted: {:?}",
                t.recent_keys(20)
            ));
        }
        if t.pos(0) != 8 {
            return Err(format!(
                "self-caused 3-step walk did not land: pos {} (events {:?})",
                t.pos(0),
                t.recent_keys(20)
            ));
        }
        Ok(())
    })
    .expect("STA-02 self-caused");
}

/// A bare self teleport from a `passTile` hook (`TEST:pass_tele` jumps its
/// holder 3 ahead) is likewise chosen, not prompted.
#[test]
fn self_caused_teleport_does_not_prompt() {
    forall(4, 0xa5a2_0009, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_state(0, UNSTOPPABLE, 1);
        t.set_pos(0, 20);
        t.set_pos(1, 50);
        t.set_state(0, "TEST.pass_tele.done", 0);
        t.give_play(0, "TEST:pass_tele")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        let mut saw_choice = false;
        while let Some(p) = t.prompt() {
            if p.title.key() == CHOICE_TITLE {
                saw_choice = true;
            }
            t.decline();
        }
        if saw_choice {
            return Err(format!(
                "self teleport prompted: {:?}",
                t.recent_keys(20)
            ));
        }
        if t.pos(0) != 24 {
            return Err(format!(
                "the passTile jump did not land: pos {} (events {:?})",
                t.pos(0),
                t.recent_keys(20)
            ));
        }
        Ok(())
    })
    .expect("STA-02 self teleport");
}
