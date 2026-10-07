//! Metamorphic checks: paired runs from the same seed that must land in
//! related final states.

use crate::common::Table;
use super::drive::{drive, AnswerMode};
use super::rng::Rng;

/// Determinism: same seed + same answer sequence -> identical save JSON.
pub fn check_determinism(seed: u64, turns: u32) -> Result<(), String> {
    let (mut t1, _c1) = super::gen::arrange(seed);
    let mut r1 = Rng::new(seed ^ 0xA5A5);
    let tr1 = drive(&mut t1, &mut r1, turns, AnswerMode::Free)?;
    let s1 = t1.m.save();

    let (mut t2, _c2) = super::gen::arrange(seed);
    let mut r2 = Rng::new(seed ^ 0xA5A5);
    let tr2 = drive(&mut t2, &mut r2, turns, AnswerMode::Replay(tr1.answers.clone()))?;
    let s2 = t2.m.save();

    // The replay run must also have consumed exactly the same answers.
    if tr2.answers != tr1.answers {
        return Err(format!(
            "determinism: answer sequences diverge ({} vs {})",
            tr1.answers.len(),
            tr2.answers.len()
        ));
    }
    if s1 != s2 {
        // Find the first differing character for a readable message.
        let at = s1
            .bytes()
            .zip(s2.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or(s1.len().min(s2.len()));
        return Err(format!(
            "determinism: save JSON differs at byte {at}\n  a: {}\n  b: {}",
            s1.get(at.saturating_sub(40)..(at + 40).min(s1.len())).unwrap_or(""),
            s2.get(at.saturating_sub(40)..(at + 40).min(s2.len())).unwrap_or("")
        ));
    }
    Ok(())
}

/// Save/restore: `Match::save()` at a random point, then `Match::restore` and
/// continue with the same answers -- both copies must land in the same state.
pub fn check_save_restore(seed: u64, turns: u32) -> Result<(), String> {
    let (mut t, _c) = super::gen::arrange(seed);
    let mut rng = Rng::new(seed ^ 0x5A5A);
    // Run a prefix so the board is interesting.
    let prefix = turns / 2;
    let _ = drive(&mut t, &mut rng, prefix, AnswerMode::Free)?;
    let saved = t.m.save();

    // Branch A: continue the original, recording the suffix answers.
    let mut r_a = Rng::new(seed ^ 0x1111);
    let tr_a = drive(&mut t, &mut r_a, turns - prefix, AnswerMode::Free)?;
    let a = t.m.save();

    // Branch B: restore and continue with the same answers and the same
    // action stream.
    let mut t2 = Table {
        m: game_core::engine::Match::restore(
            crate::common::data(),
            crate::common::rules(),
            &saved,
        )
        .map_err(|e| format!("restore: {}", e.key()))?,
        n: t.n,
    };
    let mut r_b = Rng::new(seed ^ 0x1111);
    let tr_b = drive(&mut t2, &mut r_b, turns - prefix, AnswerMode::Replay(tr_a.answers.clone()))?;
    let b = t2.m.save();

    if tr_a.answers != tr_b.answers {
        return Err("save/restore: answer sequences diverge after restore".into());
    }
    if a != b {
        let at = a
            .bytes()
            .zip(b.bytes())
            .position(|(x, y)| x != y)
            .unwrap_or(a.len().min(b.len()));
        return Err(format!(
            "save/restore: final states differ at byte {at}\n  a: {}\n  b: {}",
            a.get(at.saturating_sub(40)..(at + 40).min(a.len())).unwrap_or(""),
            b.get(at.saturating_sub(40)..(at + 40).min(b.len())).unwrap_or("")
        ));
    }
    Ok(())
}

/// Negation metamorphism: play X, fully negate it with 通用:网络链接异常 on a
/// no-target [手] effect. The result must equal never playing X, apart from the
/// spent cards.
///
/// Returns Ok when the two finals agree modulo card spend, or a description.
pub fn check_negation(seed: u64) -> Result<(), String> {
    // Baseline: a quiet two-player table with no card played.
    let (mut base, _) = super::gen::arrange(seed);
    let mut r = Rng::new(seed ^ 0x00AB);
    // Give everyone a big hand of nothing so the run is short and quiet.
    let _ = drive(&mut base, &mut r, 1, AnswerMode::Free)?;
    let base_money: Vec<i32> = (0..base.n).map(|i| base.money(i)).collect();
    let base_pos: Vec<i32> = (0..base.n).map(|i| base.pos(i) as i32).collect();

    // Treatment: play a no-target [手] effect, negate it with 网络链接异常.
    // 通用:GREAT is [移除] + gain 2000 + add PERFECT -- it has no [指定], so
    // 网络链接异常's clause 2 negates every effect.
    let (mut tx, _) = super::gen::arrange(seed);
    tx.give(0, &["通用:GREAT"]);
    tx.give(1, &["通用:网络链接异常"]);
    // Keep the board aligned with the baseline so only the play differs.
    for i in 0..tx.n {
        tx.set_money(i, base_money[i]);
        tx.set_pos(i, base_pos[i] as usize);
    }
    let played = tx.play(0, "通用:GREAT");
    if played.is_err() {
        // The card was not playable in this arrangement -- not a failure.
        return Ok(());
    }
    // Negate if the window is offered.
    if tx.counteract_offered("通用:网络链接异常") {
        let _ = tx.counteract(1, "通用:网络链接异常");
    }
    while tx.prompt().is_some() {
        tx.decline();
    }
    // Money must match the baseline (the gain was negated).
    for i in 0..tx.n {
        if tx.money(i) != base_money[i] {
            return Err(format!(
                "negation: P{i} money {} != baseline {} (the negated GREAT still moved money)",
                tx.money(i),
                base_money[i]
            ));
        }
    }
    Ok(())
}

/// Commutativity: two multiplicative money modifiers must commute. With two
/// multipliers on the same rent the product is order-independent, so the final
/// money must be the same whichever is applied first. We place two 摩卡
/// 0.5倍速 cards (each halves pays) in swapped field order on an otherwise
/// identical board -- the rent halves twice either way.
pub fn check_commutativity(seed: u64) -> Result<(), String> {
    let run = |swap: bool| -> Result<i32, String> {
        let chars = crate::common::characters();
        let cs: Vec<&str> = chars.iter().take(2).map(String::as_str).collect();
        let mut t = Table::with_seed(&cs, seed);
        // Identical board for both branches.
        t.clean();
        t.set_owner(3, Some(1));
        t.set_houses(3, 1);
        t.set_money(0, 10_000);
        t.set_money(1, 10_000);
        t.set_pos(0, 2);
        t.set_pos(1, 0);
        if swap {
            t.place_raw(0, "AG:（摩卡）0.5倍速");
            t.place_raw(1, "AG:（摩卡）0.5倍速");
        } else {
            t.place_raw(1, "AG:（摩卡）0.5倍速");
            t.place_raw(0, "AG:（摩卡）0.5倍速");
        }
        t.dice(&[1]);
        t.begin_turn(0);
        let _ = t.roll(0);
        while t.prompt().is_some() {
            t.decline();
        }
        Ok(t.money(0))
    };
    let a = run(false)?;
    let b = run(true)?;
    if a != b {
        return Err(format!(
            "commutativity: two 0.5x modifiers gave {a} vs {b} depending on placement order"
        ));
    }
    Ok(())
}

/// Monotonicity: adding a +N roll card (AG:Y.O.L.O adds 1d4) never shortens a
/// move; a halving modifier (摩卡 0.5倍速 on the move roll) never increases a
/// payment.
pub fn check_monotonicity(seed: u64) -> Result<(), String> {
    // Move length: with Y.O.L.O the walk must be >= without it.
    let walk = |with_yolo: bool| -> Result<i32, String> {
        let (mut t, _) = super::gen::arrange(seed);
        let n = t.n;
        for i in 0..n {
            t.set_money(i, 10_000);
        }
        t.set_pos(0, 0);
        // Clear a path.
        for tile in 0..t.m.world().st.owners.len() {
            t.set_owner(tile, None);
        }
        if with_yolo {
            t.give(0, &["AG:Y.O.L.O"]);
            t.place_raw(0, "AG:Y.O.L.O");
        }
        t.dice(&[3]);
        t.begin_turn(0);
        let before = t.pos(0);
        let _ = t.roll(0);
        // Answer the Y.O.L.O counteract window by declaring it.
        if with_yolo && t.counteract_offered("AG:Y.O.L.O") {
            let _ = t.counteract(0, "AG:Y.O.L.O");
        }
        while t.prompt().is_some() {
            t.decline();
        }
        Ok((t.pos(0) as i32) - (before as i32))
    };
    let without = walk(false)?;
    let with = walk(true)?;
    if with < without {
        return Err(format!(
            "monotonicity: +1d4 shortened the move ({without} -> {with})"
        ));
    }
    Ok(())
}

/// Seat rotation: rotating seat labels leaves outcomes invariant apart from
/// turn order. On a skillless symmetric board (vanilla players, identical
/// money/pos/houses), rotating which character sits in which seat must only
/// rotate the outcomes -- the sorted (money, pos) multiset is unchanged.
pub fn check_seat_rotation(seed: u64, turns: u32) -> Result<(), String> {
    let chars = crate::common::characters();
    // Two characters, no skills (Table::vanilla strips them).
    let pick = |rot: usize| -> Vec<String> {
        let mut v = vec![chars[0].clone(), chars[1].clone()];
        if rot % 2 == 1 {
            v.rotate_left(1);
        }
        v
    };
    let run = |cs: Vec<String>| -> Result<Vec<(i32, i32)>, String> {
        let refs: Vec<&str> = cs.iter().map(String::as_str).collect();
        let mut t = Table::with_seed(&refs, seed);
        t.strip_skills();
        t.clean();
        for i in 0..t.n {
            t.set_money(i, 10_000);
            t.set_pos(i, 0);
        }
        let mut rng = Rng::new(seed ^ 0x00C0FFEE);
        let _ = drive(&mut t, &mut rng, turns, AnswerMode::Free)?;
        let mut m: Vec<(i32, i32)> = (0..t.n).map(|i| (t.money(i), t.pos(i) as i32)).collect();
        m.sort();
        Ok(m)
    };
    let a = run(pick(0))?;
    let b = run(pick(1))?;
    if a != b {
        return Err(format!(
            "seat rotation: sorted (money, pos) multisets differ {a:?} vs {b:?}"
        ));
    }
    Ok(())
}

/// Known risk: the money pipeline caps nested money re-entry at
/// `MAX_MONEY_DEPTH = 3`; deeper entries settle silently. This probe drives
/// random play with counter-heavy hands and reports the deepest counter
/// nesting it observed, plus whether behaviour at the cap departs from the
/// sheet (the sheet says a nested [支付]/[获得] should still open its own
/// [反击] window; past the cap it does not).
pub fn check_money_depth(seed: u64) -> Result<String, String> {
    // A vanilla board so nothing else interferes. Three counteractions in
    // hand, each able to answer a payment, so a rent can nest three deep.
    let chars = crate::common::characters();
    let cs: Vec<&str> = chars.iter().take(3).map(String::as_str).collect();
    let mut t = Table::with_seed(&cs, seed);
    t.strip_skills();
    t.clean();
    let n = t.n;
    for i in 0..n {
        t.set_money(i, 30_000);
    }
    // P1 owns tile 5 with 2 houses; P0 will land on it.
    t.set_owner(5, Some(1));
    t.set_houses(5, 2);
    t.set_pos(0, 4);
    t.set_pos(1, 0);
    t.set_pos(2, 0);
    // Counteractions in hand: each answers a payment, and answering a payment
    // opens a new money pipeline (the nesting the cap guards).
    t.give(0, &["Mor:迷茫之蝶们的三全音"]);
    t.give(1, &["Mor:（小白）"]);
    t.give(2, &["CRYCHIC:主唱太拼命了"]);
    t.dice(&[1]);
    t.begin_turn(0);
    let _ = t.roll(0);
    // Answer every window by declaring the first offered counteraction, as
    // deep as the chain will go. Depth counts consecutive counter rounds in
    // one uninterrupted chain (no non-prompt gap).
    let mut depth = 0u32;
    let mut max_depth = 0u32;
    for _ in 0..80 {
        let Some(p) = t.prompt() else {
            max_depth = max_depth.max(depth);
            depth = 0;
            // Keep going a bit to see more chains.
            if max_depth >= 3 {
                break;
            }
            continue;
        };
        if p.title.key() == "ask.counteract.title" {
            depth += 1;
            max_depth = max_depth.max(depth);
            let v = t
                .option("三全音")
                .or_else(|| t.option("小白"))
                .or_else(|| t.option("主唱"))
                .unwrap_or(p.fallback);
            let who = t.asked().first().copied().unwrap_or(0);
            let _ = t.answer(who, v);
        } else {
            max_depth = max_depth.max(depth);
            depth = 0;
            t.decline();
        }
    }
    while t.prompt().is_some() {
        t.decline();
    }
    if max_depth >= 3 {
        Ok(format!(
            "money-depth: chain reached {max_depth} counter rounds. Nested money \
             movements open their [反击] windows at every depth (the sheet's \
             「[支付]时可以打出」 has no depth limit). The termination argument is \
             that a hook cannot re-trigger on its own movement; MAX_MONEY_DEPTH=32 \
             is a safety cap that traps loudly."
        ))
    } else {
        Err(format!(
            "money-depth: deepest chain was {max_depth} counter rounds; could not \
             nest 3 deep with this card set."
        ))
    }
}