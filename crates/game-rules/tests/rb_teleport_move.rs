//! Teleport + move sequencing: a move after a teleport must start at the
//! teleport destination, and a later move must not be clobbered back by the
//! replay of the teleport.
//!
//! Spec: the cards' own rulebook text (`docs/rulebook/cards.json`).
//! Shapes covered:
//!   (a) a teleport then a card move in the same effect
//!       -- 一人两个甜甜圈 (2), 纯真振翅;
//!   (b) a teleport effect before the main move roll
//!       -- 纯真振翅 「立刻进行移动掷骰」;
//!   (c) a teleport during a move-chain window, then the remaining move
//!       -- 是我自己的问题 (the settleBefore [反击] steps the mover 1 tile
//!          away from the nearest player, and the settle follows it).

mod common;
use common::*;

/// Answer every open prompt with its decline.
fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// Advance until it is `who`'s 运营 stage.
fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..60 {
        if t.turn() == who && t.step() == game_core::state::stage::OPS {
            return;
        }
        let cur = t.turn();
        if t.turn() == who {
            t.end(who).unwrap();
            drain(t);
            continue;
        }
        t.m.world_mut().st.skip_move = true;
        t.end(cur).unwrap();
        drain(t);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

// =====================================================================
// (c) teleport during a move-chain window, then the remaining move
// =====================================================================

// 规则书 (CRYCHIC:是我自己的问题) [反击]（1）:
// 「主要移动结束时，[触发结算]前打出此卡，使自己额外远离绝对距离最近的玩家一格
// （若距离最近的玩家在身前则向后移动，若与其他玩家重合则可选择任意方向）」.
#[test]
fn c_settle_before_step_away_settles_at_the_new_tile() {
    let mut t = Table::vanilla(2);
    t.set_pos(1, 10);
    t.give(0, &["CRYCHIC:是我自己的问题"]);
    t.set_pos(0, 5);
    t.dice(&[4]);
    t.roll(0).unwrap();
    // P0's main move ends at 9, next to P1 at 10; the counter steps to 8.
    let mut played = false;
    let mut prompts = Vec::new();
    loop {
        let Some(_p) = t.prompt() else { break };
        prompts.push(t.dump_prompt());
        if t.counteract_offered("CRYCHIC:是我自己的问题") {
            t.counteract(0, "CRYCHIC:是我自己的问题").unwrap();
            played = true;
        } else {
            t.decline();
        }
    }
    assert!(played, "the [反击] window offered the card: {prompts:?}");
    assert_eq!(
        t.pos(0),
        8,
        "moves 1 further from P1 (10) and settles there: pos={} keys={:?} prompts={:?}",
        t.pos(0),
        t.recent_keys(20),
        prompts
    );
}

// =====================================================================
// (a) teleport then a card move in the same effect
// =====================================================================

// 规则书 (Sumimi:一人两个甜甜圈)（2）:
// 「你原本所在格子被其他玩家经过时，可在那名玩家触发结算后选择传送至你原本所在
// 格子（不包括）与那名玩家本次移动终点间的任一格并触发结算」.
//
// The effect teleports the holder back to their original square (the exile
// ends) and *then* offers the settle-teleport onto a square between there and
// the passer's endpoint. The settle-teleport must land on the chosen square
// and settle there -- not be undone by the replay of the return jump.
#[test]
fn a_two_donuts_settle_teleport_lands_on_the_chosen_tile() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 10);
    t.give_play(0, "Sumimi:一人两个甜甜圈").unwrap();
    drain(&mut t);
    assert!(t.state(0, "exile") > 0, "P0 gets [除外]: {}", t.state(0, "exile"));
    // P1 walks from 5 past 10 (P0's original square) to 15.
    until_turn(&mut t, 1);
    t.set_pos(1, 5);
    t.dice(&[10]);
    t.roll(1).unwrap();
    // The offered squares run from 11 along the passer's direction to 15.
    let mut prompts = Vec::new();
    loop {
        let Some(p) = t.prompt() else { break };
        prompts.push(t.dump_prompt());
        if p.text.key().contains("two_donuts_yes") {
            // 「可选择传送至…」 -- take the offer (the `ask.yes` option).
            t.answer(0, 0).unwrap();
        } else if p.kind == "tile" {
            // 「传送至…任一格」 -- pick 12 (between the original 10 and the
            // endpoint 15).
            t.answer_tile(0, 12).unwrap();
        } else {
            t.decline();
        }
    }
    assert_eq!(
        t.pos(0),
        12,
        "the settle-teleport lands on the chosen square and is not undone: pos={} keys={:?} prompts={:?}",
        t.pos(0),
        t.recent_keys(20),
        prompts
    );
}

// =====================================================================
// (b) teleport effect before the main move roll
// =====================================================================

// 规则书 (Mor:纯真振翅):
// 「传送到移动方向20格后（不触发结算），立刻进行移动掷骰」.
//
// The jump is 「（不触发结算）」 and does not say 视为你的主要移动 -- it is a
// position write with no settle and no main-move consumption. 「立刻进行
// 移动掷骰」 then rolls the main move from the jump's destination.
#[test]
fn b_wing_roll_starts_from_the_teleport_destination() {
    let mut t = Table::vanilla(2);
    t.set_pos(0, 5);
    t.dice(&[3]);
    t.give_play(0, "Mor:纯真振翅").unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        5 + 20 + 3,
        "teleport 20 ahead, then the roll walks from there: pos={} keys={:?}",
        t.pos(0),
        t.recent_keys(20)
    );
}