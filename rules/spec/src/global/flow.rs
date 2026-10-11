//! G-FLOW -- phase order, turn order, the main move once per turn.
//!
//! Owned by unit 6. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Note: the 回合流程 table rows 1-3 are corrupt in the book; use
//! **行动阶段** (`rulebook-doc.md:199+`) for the turn's action order.

use game_core::state::{key, stage};
use spec_kit::{Clause, Step, StepAct};

/// Turn-flow clauses. Each entry is one book sentence, checked as a predicate
/// over one observed transition. Setup-only clauses no-op when the trigger is
/// absent (a soak never runs 开始游戏 through the observer).
pub const CLAUSES: &[Clause] = &[
    G_FLOW_01,
    G_FLOW_02,
    G_FLOW_03,
    G_FLOW_04,
    G_FLOW_05,
    G_FLOW_06,
    G_FLOW_07,
    G_FLOW_08,
    G_FLOW_09,
    G_FLOW_10,
    G_FLOW_11,
    G_FLOW_12,
    G_FLOW_13,
    G_FLOW_14,
    G_FLOW_15,
    G_FLOW_16,
    G_FLOW_17,
    G_FLOW_18,
];

// ---------------------------------------------------------------- helpers

/// The act is a player command with this `act` name.
fn is_act(step: &Step<'_>, name: &str) -> bool {
    matches!(step.act, StepAct::Act { msg, .. } if msg.act == name)
}

/// Message keys of this step's events (the log lines a player sees).
fn keys(step: &Step<'_>) -> Vec<String> {
    step.event_keys()
}

/// Does the window contain a log line whose key is exactly `key` (or starts
/// with it, for the `log.part.*` nest)?
fn has_key(step: &Step<'_>, key: &str) -> bool {
    step.events.iter().any(|e| {
        let k = spec_kit::event_key(e);
        k == key || k.starts_with(key)
    })
}

/// The seat a `log.turn` / `log.turn_extra` line names, if this window started
/// a turn. `player_id` on a `turn`-typed event is the seat.
fn turn_began(step: &Step<'_>) -> Option<usize> {
    turns_began(step).into_iter().next()
}

/// Every seat whose turn began in this window (a cascade of auto-skips can
/// start several).
fn turns_began(step: &Step<'_>) -> Vec<usize> {
    step.events
        .iter()
        .filter(|e| e.r#type == "turn")
        .map(|e| e.player_id as usize)
        .collect()
}

/// Did a turn end in this window? Covers the player-chosen `end` and the two
/// auto-skips (stun / exile), which call `end_turn` without the `end_turn` log.
fn turn_ended(step: &Step<'_>) -> bool {
    has_key(step, "log.end_turn")
        || has_key(step, "log.stunned_skip")
        || has_key(step, "log.exiled_skip")
}

/// Exile layers of player `i`.
fn exile(step_pre: bool, step: &Step<'_>, i: usize) -> i32 {
    let p = if step_pre {
        step.pre.st.players.get(i)
    } else {
        step.post.st.players.get(i)
    };
    p.map(|p| p.state_get(key::EXILE)).unwrap_or(0)
}

fn stay_of(pre: bool, step: &Step<'_>, i: usize) -> i32 {
    let p = if pre {
        step.pre.st.players.get(i)
    } else {
        step.post.st.players.get(i)
    };
    p.map(|p| p.state_get(key::STAY)).unwrap_or(0)
}

fn stun_of(pre: bool, step: &Step<'_>, i: usize) -> i32 {
    let p = if pre {
        step.pre.st.players.get(i)
    } else {
        step.post.st.players.get(i)
    };
    p.map(|p| p.state_get(key::STUN)).unwrap_or(0)
}

/// Card / skill activations in the window -- a hook may have layered a status
/// on top of the tick, so an exact -1 is only required on a quiet window.
fn has_card_events(step: &Step<'_>) -> bool {
    step.events
        .iter()
        .any(|e| matches!(e.r#type.as_str(), "card" | "play" | "skill"))
}

// ------------------------------------------------------------- G-FLOW-01

/// The main move happens at most once per turn.
///
/// * **ID:** G-FLOW-01
/// * **Source:** `rulebook-doc.md:132` / 行动阶段 row 7
/// * **Quote:** 「移动阶段中需进行1次[主要移动]。」 / 「一回合只能触发一次［主要移动］效果,多次触发［主要移动］时无效」
/// * **Trigger:** an observed input that is a `roll` command (the player
///   pressing the main-move button), or any input that touches the
///   `turn.main_moved` latch.
/// * **Condition:** `pre.turn.main_moved` is already true in the same turn
///   (the main move already happened).
/// * **Effect:** a second `roll` command does not move the player (the engine
///   must refuse it / no-op it). The latch may clear at a turn boundary --
///   including an extra turn for the same seat -- which is not a violation.
/// * **Frame:** n/a (read of the turn latch and `pos`).
/// * **Rulings & OPEN:** 「移动阶段中需进行1次」 is the "exactly one" that
///   makes a second attempt illegal. Card-driven dice (a card's 「投掷3d10」)
///   are **not** a `roll` command and are out of scope -- they are not
///   [主要移动]. Card-driven extra moves are 「[强制移动]」 / 「[传送]」 and
///   are unit 4 and unit 6's shared edge.
/// * **Executable:** [`g_flow_01`].
const G_FLOW_01: Clause = Clause::new(
    "G-FLOW-01",
    "rulebook-doc.md:132",
    "玩家在运营阶段可在非执行任何效果的时候进入移动阶段。移动阶段中需进行1次[主要移动]。",
    g_flow_01,
);

/// Executable form of [`G-FLOW-01`].
pub fn g_flow_01(step: &Step<'_>) -> Result<(), String> {
    let pre_moved = step.pre.turn.main_moved;
    let pre_turn = step.pre.st.turn;

    // A second [主要移动] attempt is the player pressing `roll` again while
    // the latch is already set. The engine must refuse it: `pos` does not
    // move. (A card's own dice is not a `roll` command.) An extra turn for
    // the same seat legitimately clears `main_moved` inside one settle -- that
    // is a turn boundary, not a second main move -- so the latch alone is not
    // a violation.
    // A second [主要移动] is a `roll` with the latch already set. A card's
    // 「视为[主要移动]」 that re-spends the move logs `log.main_move_used` and
    // must not walk -- checked below on that key. (Forced / teleport moves are
    // not [主要移动] and are out of scope.)
    let is_roll_cmd = is_act(step, "roll");
    if is_roll_cmd && pre_moved {
        let seat = step.pre.turn.player_id;
        let pos_before = step
            .pre
            .st
            .players
            .get(seat)
            .map(|p| p.pos)
            .unwrap_or(-1);
        let pos_after = step
            .post
            .st
            .players
            .get(seat)
            .map(|p| p.pos)
            .unwrap_or(-1);
        if pos_before != pos_after {
            return Err(format!(
                "second [主要移动] moved player {seat} {pos_before} -> {pos_after} in turn {pre_turn}"
            ));
        }
    }
    // 「多次触发［主要移动］时无效」 -- a card that re-spends the move logs
    // `log.main_move_used` and must not have walked.
    if has_key(step, "log.main_move_used") {
        let seat = step.pre.turn.player_id;
        let pos_before = step
            .pre
            .st
            .players
            .get(seat)
            .map(|p| p.pos)
            .unwrap_or(-1);
        let pos_after = step
            .post
            .st
            .players
            .get(seat)
            .map(|p| p.pos)
            .unwrap_or(-1);
        if pos_before != pos_after {
            return Err(format!(
                "invalid second [主要移动] still moved player {seat} {pos_before} -> {pos_after}"
            ));
        }
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-02

/// The main move is required exactly once: `end` before it is refused.
///
/// * **ID:** G-FLOW-02
/// * **Source:** `rulebook-doc.md:132`
/// * **Quote:** 「移动阶段中需进行1次[主要移动]。…[主要移动]和所需[结算]完成后进入结束阶段。」
/// * **Trigger:** an observed `end` command.
/// * **Condition:** the actor is in 运营 (`step == OPS`) with the main move
///   still owed (`!turn.main_moved`) and the turn is not a skip
///   (`!st.skip_move` -- [停留] / [除外] / [晕眩] legitimately skip it).
/// * **Effect:** the `end` is refused (a no-op). Accepting it would enter
///   结束阶段 without the required [主要移动].
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 「需进行1次」 is the "exactly one" that makes a
///   pre-move `end` illegal. [晕眩] skips both 运营 and the move (G-FLOW-09)
///   and never reaches this gate; [停留] sets `skip_move` (G-FLOW-10) and may
///   `end` without rolling.
/// * **Executable:** [`g_flow_02`].
const G_FLOW_02: Clause = Clause::new(
    "G-FLOW-02",
    "rulebook-doc.md:132",
    "移动阶段中需进行1次[主要移动]。基础[主要移动]为**投掷1d20**…[主要移动]和所需[结算]完成后进入结束阶段。",
    g_flow_02,
);

/// Executable form of [`G-FLOW-02`].
pub fn g_flow_02(step: &Step<'_>) -> Result<(), String> {
    if !is_act(step, "end") {
        return Ok(());
    }
    let who = step.player_of_act();
    // The gate is about the actor's own turn in 运营 with the move still owed.
    if step.pre.st.turn != who as i32 {
        return Ok(());
    }
    // 「[主要移动]和所需[结算]完成后进入结束阶段」 -- `end` during the move
    // or its settle is also illegal.
    if step.pre.st.step == stage::MOVE {
        return Err(format!(
            "player {who} ended the turn during 移动阶段 (settle still owed)"
        ));
    }
    if step.pre.st.step != stage::OPS {
        return Ok(());
    }
    if step.pre.st.skip_move || step.pre.turn.main_moved {
        return Ok(());
    }
    // Refused `end` is a pure no-op: the turn does not advance.
    let accepted = step.post.st.turn != step.pre.st.turn
        || step.post.st.step != step.pre.st.step
        || turn_ended(step);
    if accepted {
        return Err(format!(
            "player {who} ended the turn at 运营 without the required [主要移动] \
             (main_moved={}, skip_move={})",
            step.pre.turn.main_moved, step.pre.st.skip_move
        ));
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-03

/// Every player starts on 1号CiRCLE with 10000 money.
///
/// * **ID:** G-FLOW-03
/// * **Source:** `rulebook-doc.md:125`
/// * **Quote:** 「每局游戏开始的时候，如无技能或卡效果影响，每名玩家的初始格子为1号CiRCLE，拥有10000初始资金并抽取2张手卡。」
/// * **Trigger:** the match-start log (`log.match_start`), which carries the
///   initial money and hand size.
/// * **Condition:** no start-position / start-money effect (the quote's 「如无
///   技能或卡效果影响」). The log's `money` arg is the engine's start money.
/// * **Effect:** the log announces 10000 money; every seat's `pos` is tile 0
///   (1号CiRCLE) and `money` is 10000.
/// * **Frame:** n/a (read of the opening state).
/// * **Rulings & OPEN:** 「1号CiRCLE」 is board index 0 (`data/board.json`
///   name `CiRCLE`). A `deckBeforeGame` hook may move a start position or
///   change the opening hand size -- those are skill/card effects and out of
///   scope here; the clause then requires the *announced* money only.
/// * **Executable:** [`g_flow_03`].
const G_FLOW_03: Clause = Clause::new(
    "G-FLOW-03",
    "rulebook-doc.md:125",
    "每局游戏开始的时候，如无技能或卡效果影响，每名玩家的初始格子为1号CiRCLE，拥有10000初始资金并抽取2张手卡。",
    g_flow_03,
);

/// Executable form of [`G-FLOW-03`].
pub fn g_flow_03(step: &Step<'_>) -> Result<(), String> {
    if !has_key(step, "log.match_start") {
        return Ok(());
    }
    // The announce carries the engine's start money (10000).
    for e in step.events {
        if spec_kit::event_key(e) == "log.match_start" {
            let announced = spec_kit::int_arg(&e.msg, "money").unwrap_or(0);
            if announced != 10_000 {
                return Err(format!(
                    "log.match_start announced {announced} money, book says 10000"
                ));
            }
        }
    }
    // 「如无技能或卡效果影响」 -- a start effect (deckBeforeGame / a [特] in
    // the opening deck) may move a seat or grant money. Only pin the absolute
    // start state on a quiet window.
    if has_card_events(step) {
        return Ok(());
    }
    let n = step.post.st.players.len();
    for i in 0..n {
        let money = step.post.st.players[i].money;
        if money != 10_000 {
            return Err(format!(
                "player {i} starts with {money} money, book says 10000"
            ));
        }
        let pos = step.post.st.players[i].pos;
        if pos != 0 {
            return Err(format!(
                "player {i} starts on tile {pos}, book says 1号CiRCLE (tile 0)"
            ));
        }
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-04

/// Opening hand of 2, plus one optional redraw of the whole hand.
///
/// * **ID:** G-FLOW-04
/// * **Source:** `rulebook-doc.md:125` / 开始游戏 rows 5-6
/// * **Quote:** 「并抽取2张手卡。仅一次，可将抽取的手卡全部洗回抽卡区并重抽2张。」
/// * **Trigger:** the opening deal (`log.match_start` window) and a mulligan
///   (`log.mulligan`).
/// * **Condition:** the opening hand size is the default 2 (a `deckBeforeGame`
///   effect may cut it -- out of scope).
/// * **Effect:** after the deal each seat holds 2 cards. A `log.mulligan` line
///   means that seat returned the opening hand and drew a fresh 2 -- and only
///   once (`st.players[i].mulligan`).
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 「仅一次」 is the once-only mulligan. The redraw is
///   of the *whole* opening hand (「全部洗回…重抽2张」), not a partial keep.
/// * **Executable:** [`g_flow_04`].
const G_FLOW_04: Clause = Clause::new(
    "G-FLOW-04",
    "rulebook-doc.md:125",
    "每名玩家抽取2张手卡 | 初始重抽 | 每名玩家可独立选择洗回自己初始抽卡获得的手卡并重新抽取2张手卡",
    g_flow_04,
);

/// Executable form of [`G-FLOW-04`].
pub fn g_flow_04(step: &Step<'_>) -> Result<(), String> {
    // Opening deal: the match_start announce carries the hand size.
    if has_key(step, "log.match_start") {
        for e in step.events {
            if spec_kit::event_key(e) == "log.match_start" {
                let hand = spec_kit::int_arg(&e.msg, "hand").unwrap_or(0);
                if hand != 2 {
                    return Err(format!(
                        "log.match_start announced opening hand {hand}, book says 2"
                    ));
                }
            }
        }
    }
    // A mulligan: the seat's hand is back to 2 and the flag is set once.
    if has_key(step, "log.mulligan") {
        for e in step.events {
            if spec_kit::event_key(e) != "log.mulligan" {
                continue;
            }
            let who = e.player_id as usize;
            let n = spec_kit::int_arg(&e.msg, "n").unwrap_or(0);
            if n != 2 {
                return Err(format!("mulligan of player {who} drew {n}, book says 2"));
            }
            let before = step
                .pre
                .st
                .players
                .get(who)
                .map(|p| p.mulligan)
                .unwrap_or(false);
            if before {
                return Err(format!(
                    "mulligan of player {who} ran twice (「仅一次」)"
                ));
            }
            let mulliganed = step
                .post
                .st
                .players
                .get(who)
                .map(|p| p.mulligan)
                .unwrap_or(false);
            if !mulliganed {
                return Err(format!(
                    "mulligan of player {who} did not set the once-only flag"
                ));
            }
            // 重抽2张: the redraw count is `n`. The live hand is only pinned
            // on a quiet window -- a card's [特] may touch the hand afterwards
            // (the quote's 「如无技能或卡效果影响」).
            if !has_card_events(step) {
                let held = step.post.hidden.get(who).map(|h| h.hand.len()).unwrap_or(0);
                if held != 2 {
                    return Err(format!(
                        "after mulligan player {who} holds {held} cards, book says 重抽2张"
                    ));
                }
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-05

/// Turn stage order: 开始 → 运营 → 移动 → 结束.
///
/// * **ID:** G-FLOW-05
/// * **Source:** `rulebook-doc.md:127`
/// * **Quote:** 「每回合的阶段顺序从头到尾为：开始阶段，运营阶段，移动阶段，结束阶段。」
/// * **Trigger:** any observed transition that stays inside one turn (no
///   `turn`-typed boundary event in the window).
/// * **Condition:** `pre.st.turn == post.st.turn >= 0`.
/// * **Effect:** `post.st.step >= pre.st.step` -- the stage never runs
///   backwards inside a turn. (Skips jump forward: 晕眩 开始 → 结束.)
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** OPEN-回合流程 -- the 回合流程 table rows 1-3 are
///   corrupt; the **行动阶段** table is the source of the turn's action order.
///   A turn boundary (including an extra turn for the same seat) legitimately
///   resets the stage to 开始 and is skipped here.
/// * **Executable:** [`g_flow_05`].
const G_FLOW_05: Clause = Clause::new(
    "G-FLOW-05",
    "rulebook-doc.md:127",
    "每回合的阶段顺序从头到尾为：开始阶段，运营阶段，移动阶段，结束阶段。",
    g_flow_05,
);

/// Executable form of [`G-FLOW-05`].
pub fn g_flow_05(step: &Step<'_>) -> Result<(), String> {
    // A `turn` event is a turn boundary (new turn, or an extra turn for the
    // same seat): the stage resets to 开始 and the monotonic rule does not
    // apply across it.
    if turn_began(step).is_some() {
        return Ok(());
    }
    if step.pre.st.turn < 0 || step.pre.st.turn != step.post.st.turn {
        return Ok(());
    }
    let (a, b) = (step.pre.st.step, step.post.st.step);
    if b < a {
        return Err(format!(
            "stage ran backwards inside turn {}: {} -> {} (book order 开始,运营,移动,结束)",
            step.pre.st.turn, a, b
        ));
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-06

/// The next player's turn begins immediately after a turn ends.
///
/// * **ID:** G-FLOW-06
/// * **Source:** `rulebook-doc.md:127`
/// * **Quote:** 「**每名玩家的回合结束后立刻进入行动顺序中下一位玩家的回合。**」
/// * **Trigger:** an observed window in which a turn ended (`log.end_turn`,
///   or an auto-skip that calls `end_turn`).
/// * **Condition:** the match is still in play afterwards.
/// * **Effect:** the same settled window also contains the next `log.turn` /
///   `log.turn_extra`, for the next [存活] seat in action order.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 「立刻」 is the settle of one `end` input running on
///   into the next turn -- no intervening player input. Extra turns
///   (`log.turn_extra`) are 「额外回合」 and count as that seat's next turn.
/// * **Executable:** [`g_flow_06`].
const G_FLOW_06: Clause = Clause::new(
    "G-FLOW-06",
    "rulebook-doc.md:127",
    "每名玩家的回合结束后立刻进入行动顺序中下一位玩家的回合。",
    g_flow_06,
);

/// Executable form of [`G-FLOW-06`].
pub fn g_flow_06(step: &Step<'_>) -> Result<(), String> {
    if !turn_ended(step) {
        return Ok(());
    }
    // Match over (everyone out / last standing) -- no next turn to demand.
    if step.post.st.phase != "play" {
        return Ok(());
    }
    // A turn-end hook may open a decision (「结束阶段后」 effects). The next
    // turn begins immediately once that window closes -- the settle stopped
    // mid-decision, so demand the next turn only at rest.
    if step.post_prompt_open || step.post.st.busy {
        return Ok(());
    }
    let began = turn_began(step);
    let Some(began) = began else {
        return Err(format!(
            "a turn ended but no next turn began in the same settle \
             (phase={}, turn={}, keys={:?})",
            step.post.st.phase,
            step.post.st.turn,
            keys(step)
        ));
    };
    // 「下一位玩家的回合」 -- the next [存活] seat in action order, not just
    // any turn. `pre.st.turn` is the seat that just ended.
    let n = step.post.st.players.len();
    if n == 0 {
        return Ok(());
    }
    let from = step.pre.st.turn;
    if from < 0 {
        return Ok(());
    }
    let mut expect = None;
    for k in 1..=n {
        let c = ((from as usize + k) % n) as i32;
        if !step.post.st.players[c as usize].out() {
            expect = Some(c as usize);
            break;
        }
    }
    // An extra turn for the same seat is also legal (「额外回合」) and logs
    // `log.turn_extra` for that seat.
    let extra = has_key(step, "log.turn_extra") && began == from as usize;
    if !extra {
        if let Some(expect) = expect {
            if began != expect {
                return Err(format!(
                    "turn ended for seat {from} but next turn began for seat {began}, \
                     book says the next [存活] seat ({expect})"
                ));
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-07

/// 回合开始前 removes one [除外] layer.
///
/// * **ID:** G-FLOW-07
/// * **Source:** `rulebook-doc.md:98` / 行动阶段 row 1
/// * **Quote:** 「玩家的每回合开始前移除一层。」 / 「负面效果：移除一层[除外]」
/// * **Trigger:** a window in which player `i`'s turn begins (`log.turn` /
///   `log.turn_extra` for `i`) and `pre` exile of `i` is already > 0.
/// * **Condition:** no card activation in the window that could layer more
///   [除外] on top of the tick (those windows are skipped -- a hook may add).
/// * **Effect:** `post` exile of `i` is exactly `pre - 1`.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** The tick is 「回合开始前」 -- before the turn's own
///   开始阶段 effects. If the layer was the last one, the return [传送]
///   (「状态结束后在玩家回合开始时要[传送]到的格子」) is unit 4 / G-STATUS.
/// * **Executable:** [`g_flow_07`].
const G_FLOW_07: Clause = Clause::new(
    "G-FLOW-07",
    "rulebook-doc.md:98",
    "该效果生效时无法重复获得该效果，玩家的每回合开始前移除一层。",
    g_flow_07,
);

/// Executable form of [`G-FLOW-07`].
pub fn g_flow_07(step: &Step<'_>) -> Result<(), String> {
    let seats = turns_began(step);
    // Skip a window in which the turn-start routine did not complete (a
    // `turnStartBefore` hook prompted and paused it before the exile tick).
    // Engine-level: `post_prompt_open`, never the `"card"` presentation
    // event -- a lazy header must not decide whether this clause runs.
    // (The old `has_card_events` skip was a presentation dependency that
    // masked the check on master whenever any bookkeeping hook flashed.)
    if seats.is_empty() || step.post_prompt_open {
        return Ok(());
    }
    for i in seats {
        let before = exile(true, step, i);
        if before <= 0 {
            continue;
        }
        let after = exile(false, step, i);
        if after != before - 1 {
            return Err(format!(
                "回合开始前 of player {i}: [除外] {before} -> {after}, book says 移除一层 ({})",
                before - 1
            ));
        }
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-08

/// 结束阶段 removes one [晕眩] layer and one [停留] layer.
///
/// * **ID:** G-FLOW-08
/// * **Source:** `rulebook-doc.md:96-97` / 行动阶段 row 18
/// * **Quote:** 「玩家的每回合结束时移除一层。」（[停留] / [晕眩]）/ 「负面效果：移除一层[眩晕]/[停留]」
/// * **Trigger:** a window in which player `i`'s turn ended.
/// * **Condition:** the layer was already > 0 in `pre`, and no card
///   activation in the window (a hook may layer a fresh status on top of the
///   tick -- those windows are skipped).
/// * **Effect:** each of [停留] / [晕眩] is exactly `pre - 1`.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** The tick is 「每回合结束时」 -- 行动阶段 row 18,
///   before row 19 (结束阶段后). `stunStart` (「本回合获得的晕眩」) ticks at
///   the *next* turn's start and is G-STATUS's business, not this row.
/// * **Executable:** [`g_flow_08`].
const G_FLOW_08: Clause = Clause::new(
    "G-FLOW-08",
    "rulebook-doc.md:96",
    "[停留]：…重复受到该效果时可叠加，玩家的每回合结束时移除一层。",
    g_flow_08,
);

/// Executable form of [`G-FLOW-08`].
pub fn g_flow_08(step: &Step<'_>) -> Result<(), String> {
    if !turn_ended(step) || has_card_events(step) {
        return Ok(());
    }
    // Whose turn ended? The pre-turn owner (`end` of that seat), plus every
    // auto-skip in the window (stun / exile skip calls `end_turn` for that
    // seat). A seat whose turn merely *began* has not worn off yet.
    let mut seats: Vec<usize> = vec![];
    if step.pre.st.turn >= 0 {
        seats.push(step.pre.st.turn as usize);
    }
    if is_act(step, "end") {
        seats.push(step.player_of_act());
    }
    for e in step.events {
        let k = spec_kit::event_key(e);
        if k == "log.stunned_skip" || k == "log.exiled_skip" {
            seats.push(e.player_id as usize);
        }
    }
    seats.sort_unstable();
    seats.dedup();
    let mut errs = Vec::new();
    for i in seats {
        for (name, before, after) in [
            ("[停留]", stay_of(true, step, i), stay_of(false, step, i)),
            ("[晕眩]", stun_of(true, step, i), stun_of(false, step, i)),
        ] {
            if before <= 0 {
                continue;
            }
            if after != before - 1 {
                errs.push(format!(
                    "{name} of player {i}: {before} -> {after}, book says 移除一层"
                ));
            }
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs.join("; "))
    }
}

// ------------------------------------------------------------- G-FLOW-09

/// 晕眩 skips 运营 and the main move.
///
/// * **ID:** G-FLOW-09
/// * **Source:** `rulebook-doc.md:97` / 行动阶段 row 4
/// * **Quote:** 「[晕眩]：处于该状态时[无法移动]，无法使用[主动]技能和手卡的[手]效果…」 /
///   「眩晕效果发动：跳过经营和主要移动阶段」
/// * **Trigger:** a window containing `log.stunned_skip` for player `i`.
/// * **Condition:** `i` is stunned at their turn start.
/// * **Effect:** no [主要移动] runs for `i` that turn (`turn.main_moved`
///   stays false / no `roll` event), and the turn goes straight to its end.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 行动阶段 row 4 names the skip of 经营 *and* 主要移动.
///   [无法移动] (「[主要移动]的[移动终点]视为当前所在格子且不[结算]」) is the
///   standing definition; the skip is the turn-flow consequence.
/// * **Executable:** [`g_flow_09`].
const G_FLOW_09: Clause = Clause::new(
    "G-FLOW-09",
    "rulebook-doc.md:204",
    "眩晕效果发动：跳过经营和主要移动阶段",
    g_flow_09,
);

/// Executable form of [`G-FLOW-09`].
pub fn g_flow_09(step: &Step<'_>) -> Result<(), String> {
    if !has_key(step, "log.stunned_skip") {
        return Ok(());
    }
    // A stunned skip must not also walk a [主要移动].
    if step.post.turn.main_moved {
        return Err(
            "晕眩 skip ran a [主要移动] (main_moved=true); book says 跳过经营和主要移动阶段"
                .into(),
        );
    }
    if has_key(step, "log.roll") {
        return Err("晕眩 skip window contains a roll; book says 跳过经营和主要移动阶段".into());
    }
    // 跳过经营 -- no hand plays / skills / buys / builds in the skip window.
    for e in step.events {
        let k = spec_kit::event_key(e);
        if e.r#type == "play" || e.r#type == "skill" || k == "log.buy" || k == "log.build" {
            return Err(format!(
                "晕眩 skip window contains 经营 activity ({k}); book says 跳过经营和主要移动阶段"
            ));
        }
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-10

/// 停留 skips the main move (but not 运营).
///
/// * **ID:** G-FLOW-10
/// * **Source:** `rulebook-doc.md:96` / 行动阶段 row 7
/// * **Quote:** 「[停留]：处于该状态时[无法移动]。」 / 「停留效果发动：跳过主要移动阶段」
/// * **Trigger:** a window containing `log.stay_skip` for the turn player.
/// * **Condition:** the turn player has [停留] layers and is not [不可阻挡].
/// * **Effect:** the turn's [主要移动] is skipped (`st.skip_move`); a `roll`
///   for that turn is refused; the player may still act in 运营 and then `end`.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** [不可阻挡] 「无视已存在的[停留]」 lifts the skip --
///   that path logs no `stay_skip` and is out of scope here (G-STATUS).
/// * **Executable:** [`g_flow_10`].
const G_FLOW_10: Clause = Clause::new(
    "G-FLOW-10",
    "rulebook-doc.md:207",
    "停留效果发动：跳过主要移动阶段",
    g_flow_10,
);

/// Executable form of [`G-FLOW-10`].
pub fn g_flow_10(step: &Step<'_>) -> Result<(), String> {
    if !has_key(step, "log.stay_skip") {
        return Ok(());
    }
    if step.post.turn.main_moved {
        return Err("停留 skip ran a [主要移动]; book says 跳过主要移动阶段".into());
    }
    if has_key(step, "log.roll") {
        return Err("停留 skip window contains a roll; book says 跳过主要移动阶段".into());
    }
    // The skip latch itself (「跳过主要移动阶段」).
    if !step.post.st.skip_move {
        return Err("停留 skip did not set skip_move; book says 跳过主要移动阶段".into());
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-11

/// 进入移动阶段 only when no effect is executing.
///
/// * **ID:** G-FLOW-11
/// * **Source:** `rulebook-doc.md:132`
/// * **Quote:** 「玩家在运营阶段可在非执行任何效果的时候进入移动阶段。」
/// * **Trigger:** an observed `roll` command (the input that enters 移动阶段).
/// * **Condition:** an effect is executing -- `pre.st.busy` (a routine is
///   pending) or a prompt is open (`pre_prompt_open`).
/// * **Effect:** the `roll` does not run (no move, no `main_moved`).
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 「非执行任何效果的时候」 is read as "no pending
///   routine and no open decision". A [反击] window is an open decision, so
///   rolling through it is illegal.
/// * **Executable:** [`g_flow_11`].
const G_FLOW_11: Clause = Clause::new(
    "G-FLOW-11",
    "rulebook-doc.md:132",
    "玩家在运营阶段可在非执行任何效果的时候进入移动阶段。",
    g_flow_11,
);

/// Executable form of [`G-FLOW-11`].
pub fn g_flow_11(step: &Step<'_>) -> Result<(), String> {
    if !is_act(step, "roll") {
        return Ok(());
    }
    let busy = step.pre.st.busy || step.pre_prompt_open;
    if !busy {
        return Ok(());
    }
    // The roll must not have run. Compare the actor's own pos (the turn
    // player may differ if a non-turn seat sent the command).
    let who = step.player_of_act();
    let pos_before = step
        .pre
        .st
        .players
        .get(who)
        .map(|p| p.pos)
        .unwrap_or(-1);
    let pos_after = step
        .post
        .st
        .players
        .get(who)
        .map(|p| p.pos)
        .unwrap_or(-1);
    if pos_before != pos_after || step.post.turn.main_moved {
        return Err(format!(
            "roll entered 移动阶段 while an effect was executing \
             (busy={}, prompt={}; pos {pos_before} -> {pos_after}, main_moved={})",
            step.pre.st.busy,
            step.pre_prompt_open,
            step.post.turn.main_moved
        ));
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-12

/// 娱乐 mode seats 3 to 10 players.
///
/// * **ID:** G-FLOW-12
/// * **Source:** `rulebook-doc.md:107`
/// * **Quote:** 「游玩人数为3到10人。」
/// * **Trigger:** the match-start log of a Casual (娱乐) match.
/// * **Condition:** `mode == Casual` and the window announces match start.
/// * **Effect:** the seat count is in `3..=10`.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** `MatchMode::Casual` is 娱乐; `MatchMode::Ranked` is
///   正规 (G-FLOW-14). Solo is a practice table and is not the book's 娱乐
///   mode -- its seat count is out of scope.
/// * **Executable:** [`g_flow_12`].
const G_FLOW_12: Clause = Clause::new(
    "G-FLOW-12",
    "rulebook-doc.md:107",
    "游玩人数为3到10人。",
    g_flow_12,
);

/// Executable form of [`G-FLOW-12`].
pub fn g_flow_12(step: &Step<'_>) -> Result<(), String> {
    if !has_key(step, "log.match_start") {
        return Ok(());
    }
    if step.post.st.mode != game_core::MatchMode::Casual as i32 {
        return Ok(());
    }
    let n = step.post.st.players.len();
    if !(3..=10).contains(&n) {
        return Err(format!(
            "娱乐 mode seated {n} players, book says 3到10人"
        ));
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-13

/// Turn order: 1d20 each, high to low; ties re-roll among the tied.
///
/// * **ID:** G-FLOW-13
/// * **Source:** `rulebook-doc.md:109`
/// * **Quote:** 「游戏开始时每名玩家投掷1d20，根据出目从大到小决定一局游戏的行动顺序（最大的先开始）。如果多名玩家出目一致则那些玩家再投一次1d20来决定在那些玩家中的顺序，直到排完。」
/// * **Trigger:** the order log (`log.order` / `log.order_rerolled`).
/// * **Condition:** the window announces the seat rolls.
/// * **Effect:** the announced rolls are strictly decreasing across seats
///   (high to low, no ties left); each roll is in `1..=20`. A
///   `log.order_rerolled` key means at least one tie re-roll happened.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** The engine logs only the *final* rolls per seat, so
///   "only the tied players re-rolled" cannot be checked from the event
///   stream alone -- the scenario test pins the final order and the
///   re-roll key. OPEN-平局重投: the book's 「那些玩家再投一次」 among tied
///   players is the requirement; the log shape cannot distinguish "everyone
///   re-rolled" from "only the tied re-rolled".
/// * **Executable:** [`g_flow_13`].
const G_FLOW_13: Clause = Clause::new(
    "G-FLOW-13",
    "rulebook-doc.md:109",
    "游戏开始时每名玩家投掷1d20，根据出目从大到小决定一局游戏的行动顺序（最大的先开始）。如果多名玩家出目一致则那些玩家再投一次1d20来决定在那些玩家中的顺序，直到排完。",
    g_flow_13,
);

/// Executable form of [`G-FLOW-13`].
pub fn g_flow_13(step: &Step<'_>) -> Result<(), String> {
    use game_core::msg::Arg;
    let Some(e) = step
        .events
        .iter()
        .find(|e| matches!(spec_kit::event_key(e).as_str(), "log.order" | "log.order_rerolled"))
    else {
        return Ok(());
    };
    let key = spec_kit::event_key(e);
    // `rolls` is a list of `log.part.player_roll` messages, one per seat, in
    // the final (high-to-low) seat order.
    let Some(Arg::List(rolls)) = e.msg.a.get("rolls") else {
        return Err(format!("{key}: missing rolls list"));
    };
    let mut faces: Vec<i64> = Vec::new();
    for item in rolls {
        let Arg::Msg(inner) = item else {
            return Err(format!("{key}: rolls item is not a message"));
        };
        let n = spec_kit::int_arg(inner, "n").unwrap_or(-1);
        if !(1..=20).contains(&n) {
            return Err(format!("{key}: roll {n} outside 1d20"));
        }
        faces.push(n);
    }
    if faces.len() != step.post.st.players.len() {
        return Err(format!(
            "{key}: {} rolls for {} players",
            faces.len(),
            step.post.st.players.len()
        ));
    }
    for w in faces.windows(2) {
        if w[0] <= w[1] {
            return Err(format!(
                "{key}: rolls not strictly high-to-low: {faces:?} (book: 从大到小, ties re-roll until 排完)"
            ));
        }
    }
    // The live seats also carry those faces, in order.
    for (i, &face) in faces.iter().enumerate() {
        let seat_roll = step.post.st.players[i].roll as i64;
        if seat_roll != face {
            return Err(format!(
                "{key}: seat {i} roll {seat_roll} != announced {face}"
            ));
        }
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-14

/// 正规 mode seats 5 to 6 players.
///
/// * **ID:** G-FLOW-14
/// * **Source:** `rulebook-doc.md:117`
/// * **Quote:** 「游玩人数须在5到6人。」
/// * **Trigger:** the match-start log of a Ranked (正规) match.
/// * **Condition:** `mode == Ranked` and the window announces match start.
/// * **Effect:** the seat count is in `5..=6`.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** `MatchMode::Ranked` is 正规. The engine's Table
///   harness allows 2..=10 seats for any mode; a Ranked match outside 5-6 is
///   a book violation the scenario tests pin.
/// * **Executable:** [`g_flow_14`].
const G_FLOW_14: Clause = Clause::new(
    "G-FLOW-14",
    "rulebook-doc.md:117",
    "游玩人数须在5到6人。",
    g_flow_14,
);

/// Executable form of [`G-FLOW-14`].
pub fn g_flow_14(step: &Step<'_>) -> Result<(), String> {
    if !has_key(step, "log.match_start") {
        return Ok(());
    }
    if step.post.st.mode != game_core::MatchMode::Ranked as i32 {
        return Ok(());
    }
    let n = step.post.st.players.len();
    if !(5..=6).contains(&n) {
        return Err(format!("正规 mode seated {n} players, book says 5到6人"));
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-15

/// 正规 ban: at most one character each, reverse turn order, banned unpickable.
///
/// * **ID:** G-FLOW-15
/// * **Source:** `rulebook-doc.md:118-120` / 开始游戏 row 1
/// * **Quote:** 「选择角色前每名玩家有机会禁用最多一名角色，禁用的角色本局不可被任何玩家选择。」
///   「禁用角色的顺数为行动顺序从后到前」 / 「次阶段只有正规模式中使用」
/// * **Trigger:** a `ban` command (or the ban-phase log).
/// * **Condition:** `mode == Ranked`, phase is `ban`.
/// * **Effect:** bans are taken from the last seat forward (turn index counts
///   down); each seat bans at most once (`ban_done`); a banned name goes on
///   `st.bans` and cannot be picked later.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 「最多一名」 includes zero (skip the ban). The
///   engine records a skip as `log.no_ban` and an empty `ban` string.
/// * **Executable:** [`g_flow_15`].
const G_FLOW_15: Clause = Clause::new(
    "G-FLOW-15",
    "rulebook-doc.md:119",
    "选择角色前每名玩家有机会禁用最多一名角色，禁用的角色本局不可被任何玩家选择。",
    g_flow_15,
);

/// Executable form of [`G-FLOW-15`].
pub fn g_flow_15(step: &Step<'_>) -> Result<(), String> {
    if step.pre.st.mode != game_core::MatchMode::Ranked as i32 {
        return Ok(());
    }
    // Ban-phase turn order: after a ban the engine walks the turn index down.
    // A rejected ban is a no-op (no `ban` event) and is not a second ban.
    let ban_accepted = is_act(step, "ban")
        && step.events.iter().any(|e| e.r#type == "ban");
    if ban_accepted {
        let who = step.player_of_act();
        // 「禁用角色的顺数为行动顺序从后到前」 -- the turn index counts down.
        // The last ban opens the pick phase (turn becomes 0), so only pin the
        // walk while the ban phase is still open.
        let pre_turn = step.pre.st.turn;
        let post_turn = step.post.st.turn;
        if step.post.st.phase == "ban" && pre_turn >= 0 && post_turn >= 0 && post_turn >= pre_turn {
            return Err(format!(
                "ban turn index {pre_turn} -> {post_turn}, book says 从后到前 (counts down)"
            ));
        }
        // The banner is the seat whose turn it is (the engine enforces this);
        // after a successful ban, ban_done is set at most once.
        let done_before = step
            .pre
            .st
            .players
            .get(who)
            .map(|p| p.ban_done)
            .unwrap_or(false);
        if done_before {
            return Err(format!("player {who} banned twice (最多一名)"));
        }
        let done_after = step
            .post
            .st
            .players
            .get(who)
            .map(|p| p.ban_done)
            .unwrap_or(false);
        if !done_after {
            return Err(format!("ban of player {who} did not record ban_done"));
        }
    }
    // A banned character cannot be picked afterwards. A rejected pick is a
    // no-op (no `pick` event).
    let pick_accepted = is_act(step, "pick")
        && step.events.iter().any(|e| e.r#type == "pick");
    if pick_accepted {
        let c = match step.act {
            StepAct::Act { msg, .. } => msg.character.clone(),
            _ => String::new(),
        };
        if !c.is_empty() && step.pre.st.bans.contains(&c) {
            return Err(format!("picked banned character {c:?}"));
        }
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-16

/// Character pick runs in forward turn order, after the ban phase.
///
/// * **ID:** G-FLOW-16
/// * **Source:** `rulebook-doc.md:120` / 开始游戏 rows 1-2
/// * **Quote:** 「选择角色的顺序为行动顺序从前到后。」
/// * **Trigger:** a `pick` command in the pick phase.
/// * **Condition:** `mode == Ranked` (or Casual -- both pick; only Ranked
///   bans first), phase is `pick`.
/// * **Effect:** picks are taken from the first seat forward (turn index
///   counts up). Each seat picks exactly one character; no two seats share one.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 开始游戏 row 1 (禁用角色) is Ranked-only; row 2
///   (选角色) runs in every mode. Casual goes straight to pick (no ban).
/// * **Executable:** [`g_flow_16`].
const G_FLOW_16: Clause = Clause::new(
    "G-FLOW-16",
    "rulebook-doc.md:120",
    "选择角色的顺序为行动顺序从前到后。",
    g_flow_16,
);

/// Executable form of [`G-FLOW-16`].
pub fn g_flow_16(step: &Step<'_>) -> Result<(), String> {
    // A rejected pick is a no-op (no `pick` event).
    if !is_act(step, "pick") || !step.events.iter().any(|e| e.r#type == "pick") {
        return Ok(());
    }
    let who = step.player_of_act();
    // 「选择角色的顺序为行动顺序从前到后」 -- the turn index counts up.
    // The last pick opens the deck phase (turn becomes -1), so only pin the
    // walk while the pick phase is still open.
    let pre_turn = step.pre.st.turn;
    let post_turn = step.post.st.turn;
    if step.post.st.phase == "pick" && pre_turn >= 0 && post_turn >= 0 && post_turn <= pre_turn {
        return Err(format!(
            "pick turn index {pre_turn} -> {post_turn}, book says 从前到后 (counts up)"
        ));
    }
    let c = match step.act {
        StepAct::Act { msg, .. } => msg.character.clone(),
        _ => String::new(),
    };
    if c.is_empty() {
        return Err(format!("pick of player {who} named no character"));
    }
    // Unique characters (「每名玩家选择各不同的邦多利角色」 for 娱乐; the
    // engine's `pickable` enforces it in every mode).
    let holders = step
        .post
        .st
        .players
        .iter()
        .filter(|p| p.character == c)
        .count();
    if holders != 1 {
        return Err(format!(
            "character {c:?} held by {holders} players after pick (book: 各不同的角色)"
        ));
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-17

/// 正规 mode: the last [存活] player wins.
///
/// * **ID:** G-FLOW-17
/// * **Source:** `rulebook-doc.md:121`
/// * **Quote:** 「仅剩一名[存活]玩家时游戏结束，那名玩家获胜。」
/// * **Trigger:** the match-end log (`log.match_end`) with reason `last`.
/// * **Condition:** `mode == Ranked` and the end reason is `last`.
/// * **Effect:** the winner is the sole [存活] seat (`winner`); the match is
///   `ended`.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** [存活] is 「非[破产]状态且未离开游戏的玩家」
///   (`rulebook-doc.md:63`). 娱乐 mode ends by agreement (「可在所有玩家同意时
///   结束游戏」) and lists 「仅剩一名[存活]玩家」 only as a *reference* time --
///   the engine auto-ends every mode at last standing. That auto-end in
///   娱乐 is recorded under Findings / OPEN rather than as a hard clause on
///   Casual.
/// * **Executable:** [`g_flow_17`].
const G_FLOW_17: Clause = Clause::new(
    "G-FLOW-17",
    "rulebook-doc.md:121",
    "仅剩一名[存活]玩家时游戏结束，那名玩家获胜。",
    g_flow_17,
);

/// Executable form of [`G-FLOW-17`].
pub fn g_flow_17(step: &Step<'_>) -> Result<(), String> {
    if !has_key(step, "log.match_end") {
        return Ok(());
    }
    if step.post.st.mode != game_core::MatchMode::Ranked as i32 {
        return Ok(());
    }
    if step.post.st.end_reason != "last" {
        return Ok(());
    }
    if step.post.st.phase != "ended" {
        return Err(format!(
            "match_end 'last' but phase is {:?}",
            step.post.st.phase
        ));
    }
    let winner = step.post.st.winner;
    if winner < 0 {
        return Err("match_end 'last' with no winner".into());
    }
    // The winner is a [存活] seat; everyone else is out.
    let alive: Vec<usize> = (0..step.post.st.players.len())
        .filter(|&i| !step.post.st.players[i].out())
        .collect();
    if alive != [winner as usize] {
        return Err(format!(
            "match_end 'last': winner {winner} is not the sole [存活] seat (alive {alive:?})"
        ));
    }
    Ok(())
}

// ------------------------------------------------------------- G-FLOW-18

/// 开始游戏 sequence: ban (正规 only) → pick → deck → 游戏开始前 → draw 2 →
/// redraw → 游戏开始时.
///
/// * **ID:** G-FLOW-18
/// * **Source:** `rulebook-doc.md:168-176`
/// * **Quote:** 「| 1 | 禁用角色 | … | 次阶段只有正规模式中使用 |」
///   「| 2 | 选角色 | …」 「| 3 | 选卡 | …」 「| 4 | 游戏开始前 | 发动游戏开始前发动的角色技能和卡的[特]效果 |」
///   「| 5 | 初始抽卡 | 每名玩家抽取2张手卡 |」 「| 6 | 初始重抽 | …」
///   「| 7 | 游戏开始时 | 发动游戏开始时发动的角色技能和卡的[特]效果 |」
/// * **Trigger:** the setup stream (ban / pick / deck / match_start events).
/// * **Condition:** the window carries one of the setup steps.
/// * **Effect:** phases appear in the table's order -- `ban` (Ranked only)
///   before `pick` before `deck` before `log.match_start`. The two [特]
///   points (`deckBeforeGame` / `deckAtGameStart`) bracket the deal.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** OPEN-回合流程 -- use **行动阶段** for the turn;
///   开始游戏's own table is intact and is this clause's source. The engine
///   raises `deckBeforeGame` before the deal and `deckAtGameStart` after it
///   (and after the mulligan), matching rows 4 and 7.
/// * **Executable:** [`g_flow_18`].
const G_FLOW_18: Clause = Clause::new(
    "G-FLOW-18",
    "rulebook-doc.md:168",
    "开始游戏 | 1 禁用角色（正规模式）→ 2 选角色 → 3 选卡 → 4 游戏开始前 → 5 初始抽卡 → 6 初始重抽 → 7 游戏开始时",
    g_flow_18,
);

/// Executable form of [`G-FLOW-18`].
pub fn g_flow_18(step: &Step<'_>) -> Result<(), String> {
    // Rank the setup steps this window touched. 0 = ban, 1 = pick, 2 = deck,
    // 3 = match_start (rows 4-7 live inside that one settle).
    let mut seen: Vec<(u8, String)> = Vec::new();
    for e in step.events {
        let k = spec_kit::event_key(e);
        // Only accepted setup acts log their event type; a rejected command
        // is a no-op and must not be scored as a step.
        let rank = match e.r#type.as_str() {
            "ban" => Some(0),
            "pick" => Some(1),
            "deck" => Some(2),
            "text" if k == "log.match_start" => Some(3),
            _ => None,
        };
        if let Some(r) = rank {
            seen.push((r, k));
        }
    }
    for w in seen.windows(2) {
        if w[0].0 > w[1].0 {
            return Err(format!(
                "开始游戏 steps out of order: {} then {} (book: 禁用→选角色→选卡→游戏开始前→…)",
                w[0].1, w[1].1
            ));
        }
    }
    // Ranked must not pick before its ban phase is over. A pick in a window
    // that also still has an unbanned seat with `!ban_done` is a mid-ban pick.
    if step.post.st.mode == game_core::MatchMode::Ranked as i32 {
        if seen.iter().any(|(r, _)| *r == 1) {
            // Every seat has finished ban (possibly a skip) before the first pick.
            let unbanned = step
                .pre
                .st
                .players
                .iter()
                .filter(|p| !p.ban_done)
                .count();
            // `pre` of a pick act is mid-pick-phase: bans are done. A window
            // that contains a pick while some seat has not ban_done would mean
            // ban was skipped.
            if unbanned > 0 && !seen.iter().any(|(r, _)| *r == 0) {
                return Err(format!(
                    "pick ran with {unbanned} seats still owing a ban and no ban in the window"
                ));
            }
        }
    }
    Ok(())
}
