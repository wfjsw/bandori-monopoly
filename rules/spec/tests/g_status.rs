//! Scenario tests for G-STATUS: [停留] / [晕眩] / [除外] / [不可阻挡] /
//! [无法移动] layering, decay timing, and what each forbids.
//!
//! Derived from `docs/rulebook/rulebook-doc.md` 移动相关名词 items 10-15 and
//! the 行动阶段 rows they cite -- never from engine behaviour. A discrepancy
//! is a plain failing test.
//!
//! Book quotes are in `docs/rulebook/spec/global/status.md`. Every scenario
//! arms the area's per-act [`CLAUSES`] via [`Table::observe`], so a broken
//! transition fails on the clause id as well as on the scenario assertion.

use rules_spec::global::status::{ABNORMAL_MOVE_EFFECTS, CLAUSES};
use rules_spec::Table;

/// Arm the area's per-act clauses. From here on every observed input is
/// checked (and panics on the first violation).
fn with_clauses(t: &mut Table) {
    t.observe(CLAUSES);
}

/// Finish `who`'s turn: take the [主要移动] if one is still owed (the engine
/// refuses `end` with `err.roll_first` otherwise), then end. A [停留] /
/// [晕眩] / [除外] player's roll is itself the skip under test -- if it is
/// refused, `end` goes through on the skip latch.

/// Put `key` on `who` and re-run their turn start so the engine's skip latch
/// matches a status that is already in force when the [主要移动] opens (the
/// book's 「处于该状态时」). A status gained *mid-turn* is OPEN-停留-中途.
fn arm_status_before_turn(t: &mut Table, who: usize, key: &str, layers: i32) {
    t.set_state(who, key, layers);
    t.begin_turn(who);
}

fn finish_turn(t: &mut Table, who: usize) {
    if t.turn() != who {
        return;
    }
    let _ = t.roll(who);
    let _ = t.end(who);
}


// ---------------------------------------------------------------------------
// G-STATUS-01 / G-STATUS-10: definitions
// ---------------------------------------------------------------------------

/// [异常移动效果] is exactly the book's seven (G-STATUS-10).
#[test]
fn abnormal_move_effect_set() {
    assert_eq!(
        ABNORMAL_MOVE_EFFECTS,
        [
            "[传送]",
            "[停留]",
            "[晕眩]",
            "[除外]",
            "[强制移动]",
            "[强制停下]",
            "反方向移动"
        ],
        "G-STATUS-10: set must match rulebook-doc.md:100"
    );
}

/// The clause table is the doc's ten, in order.
#[test]
fn clause_ids_match_doc() {
    let ids: Vec<&str> = CLAUSES.iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        [
            "G-STATUS-01",
            "G-STATUS-02",
            "G-STATUS-03",
            "G-STATUS-04",
            "G-STATUS-05",
            "G-STATUS-06",
            "G-STATUS-07",
            "G-STATUS-08",
            "G-STATUS-09",
            "G-STATUS-10",
        ]
    );
}

/// Layer counters are non-negative (G-STATUS-01) across a turn end.
#[test]
fn status_layers_never_negative() {
    let mut t = Table::vanilla(3);
    with_clauses(&mut t);
    for who in 0..3 {
        t.set_state(who, "stay", 1);
        t.set_state(who, "stun", 1);
        t.set_state(who, "exile", 1);
    }
    // Re-run player 0's turn start so their statuses are in force at the move.
    t.begin_turn(0);
    finish_turn(&mut t, 0);
    for who in 0..3 {
        for key in ["stay", "stun", "stunStart", "exile"] {
            assert!(
                t.state(who, key) >= 0,
                "G-STATUS-01: player {who} {key} went negative"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// G-STATUS-03: [停留] decay at end of own turn
// ---------------------------------------------------------------------------

/// 「[停留]：…玩家的每回合结束时移除一层。」 -- one layer, not all of them.
#[test]
fn stay_decays_one_layer_at_end_of_own_turn() {
    for pre_layers in [1, 2, 3] {
        let mut t = Table::vanilla(3);
        with_clauses(&mut t);
        arm_status_before_turn(&mut t, 0, "stay", pre_layers);
        finish_turn(&mut t, 0);
        let got = t.state(0, "stay");
        assert_eq!(
            got,
            pre_layers - 1,
            "G-STATUS-03: [停留] {pre_layers} -> {got} over the owner's turn end (want {})",
            pre_layers - 1
        );
    }
}

/// Stacking: two layers stay two until a turn end takes one (「可叠加」).
#[test]
fn stay_stacks_and_decays_one() {
    let mut t = Table::vanilla(3);
    with_clauses(&mut t);
    arm_status_before_turn(&mut t, 0, "stay", 1);
    t.set_state(0, "stay", 2);
    assert_eq!(t.state(0, "stay"), 2, "G-STATUS-03: layers stack");
    finish_turn(&mut t, 0);
    assert_eq!(t.state(0, "stay"), 1, "G-STATUS-03: one layer off, one left");
}

// ---------------------------------------------------------------------------
// G-STATUS-05: [晕眩] decay + row 4 skip
// ---------------------------------------------------------------------------

/// 「[晕眩]：…玩家的每回合结束时移除一层。」 + row 4 auto-skip.
#[test]
fn stun_decays_one_layer_and_skips_turn() {
    for pre_layers in [1, 2] {
        let mut t = Table::vanilla(3);
        t.set_state(1, "stun", pre_layers);
        with_clauses(&mut t);
        // Ending player 0 starts player 1's turn; row 4 auto-skips it.
        finish_turn(&mut t, 0);
        let got = t.state(1, "stun");
        assert_eq!(
            got,
            pre_layers - 1,
            "G-STATUS-05: [晕眩] {pre_layers} -> {got} over the owner's turn end (want {})",
            pre_layers - 1
        );
        // The turn moved on past the stunned player (row 4: skip 运营 and
        // 主要移动 -- the engine auto-skips the whole active window).
        assert_ne!(
            t.turn(),
            1,
            "G-STATUS-05: stunned player's turn should auto-skip (row 4)"
        );
    }
}

/// 「无法使用…手卡的[手]效果」 -- a mid-turn [晕眩] blocks the play.
#[test]
fn stun_forbids_hand_play() {
    let mut t = Table::vanilla(3);
    t.give(0, &["通用:GREAT"]);
    t.set_state(0, "stun", 1);
    with_clauses(&mut t);
    let r = t.play(0, "通用:GREAT");
    assert!(
        r.is_err(),
        "G-STATUS-04: stunned player's [手] play must be refused, got {r:?}"
    );
}

// ---------------------------------------------------------------------------
// G-STATUS-07 / 08: [除外] decay, return [传送], skip
// ---------------------------------------------------------------------------

/// 「玩家的每回合开始前移除一层」 -- one layer before the turn starts.
#[test]
fn exile_decays_one_layer_before_turn_start() {
    for pre_layers in [1, 2, 3] {
        let mut t = Table::vanilla(3);
        t.set_state(1, "exile", pre_layers);
        t.set_state(1, "exileTo", -1);
        with_clauses(&mut t);
        finish_turn(&mut t, 0);
        let got = t.state(1, "exile");
        assert_eq!(
            got,
            pre_layers - 1,
            "G-STATUS-07: [除外] {pre_layers} -> {got} before the owner's turn start (want {})",
            pre_layers - 1
        );
    }
}

/// 「状态结束后在玩家回合开始时要[传送]到的格子」 -- the return [传送].
#[test]
fn exile_expiry_teleports_to_named_tile() {
    let mut t = Table::vanilla(3);
    let dest = rules_spec::tile("银河拉面馆");
    t.set_state(1, "exile", 1);
    t.set_state(1, "exileTo", dest as i32);
    with_clauses(&mut t);
    finish_turn(&mut t, 0);
    assert_eq!(t.state(1, "exile"), 0, "G-STATUS-08: the last layer ticked out");
    assert_eq!(
        t.pos(1),
        dest,
        "G-STATUS-08: pawn landed on the named return tile"
    );
}

/// Still-exiled turns are skipped (「[除外]：…[无法移动]」 + the auto-skip).
#[test]
fn exile_skips_remaining_turns() {
    let mut t = Table::vanilla(3);
    t.set_state(1, "exile", 2);
    t.set_state(1, "exileTo", -1);
    with_clauses(&mut t);
    finish_turn(&mut t, 0);
    assert_ne!(t.turn(), 1, "G-STATUS-06: exiled player's turn auto-skips");
    assert_eq!(t.state(1, "exile"), 1, "G-STATUS-07: one layer off");
}

/// 「无法使用…手卡的[手]效果」 under [除外].
#[test]
fn exile_forbids_hand_play() {
    let mut t = Table::vanilla(3);
    t.give(0, &["通用:GREAT"]);
    t.set_state(0, "exile", 1);
    t.set_state(0, "exileTo", -1);
    with_clauses(&mut t);
    let r = t.play(0, "通用:GREAT");
    assert!(
        r.is_err(),
        "G-STATUS-06: exiled player's [手] play must be refused, got {r:?}"
    );
}

// ---------------------------------------------------------------------------
// G-STATUS-02: [停留] skips the main move
// ---------------------------------------------------------------------------

/// Row 7 「停留效果发动：跳过主要移动阶段」 / item 10 「[移动终点]视为当前
/// 所在格子且不[结算]」.
#[test]
fn stay_skips_main_move() {
    let mut t = Table::vanilla(3);
    t.set_state(1, "stay", 1);
    with_clauses(&mut t);
    finish_turn(&mut t, 0);
    assert_eq!(t.turn(), 1, "stay skips the move, not the whole turn");
    let r = t.roll(1);
    assert!(
        r.is_err(),
        "G-STATUS-02: [停留] player's [主要移动] must be skipped, roll got {r:?}"
    );
}

// ---------------------------------------------------------------------------
// G-STATUS-09: [不可阻挡]
// ---------------------------------------------------------------------------

/// 「处于该状态时无视已存在的[停留]」 -- the move goes through.
#[test]
fn unstoppable_ignores_stay() {
    let mut t = Table::vanilla(3);
    t.set_state(1, "stay", 1);
    t.set_state(1, "unstoppable", 1);
    t.dice(&[5, 5, 5, 5]);
    with_clauses(&mut t);
    finish_turn(&mut t, 0);
    assert_eq!(t.turn(), 1);
    t.roll(1)
        .map_err(|e| {
            format!(
                "G-STATUS-09: unstoppable player must be able to [主要移动], roll refused: {e}"
            )
        })
        .unwrap();
}

/// 「[晕眩]效果也不会获得新的层数」 / 「无法获得新的[除外]层数」 -- while
/// [不可阻挡], the counters must not rise. The per-act clause G-STATUS-09
/// watches every soak window; this pins a clean no-grant window.
#[test]
fn unstoppable_gains_no_new_layers_in_a_quiet_window() {
    let mut t = Table::vanilla(3);
    with_clauses(&mut t);
    t.set_state(0, "unstoppable", 1);
    t.set_state(0, "stun", 1);
    t.set_state(0, "exile", 1);
    t.begin_turn(0);
    // Ending the turn decays (「依旧掉层」) but must not raise.
    finish_turn(&mut t, 0);
    assert!(
        t.state(0, "stun") <= 1,
        "G-STATUS-09: unstoppable player's [晕眩] rose"
    );
    assert!(
        t.state(0, "exile") <= 1,
        "G-STATUS-09: unstoppable player's [除外] rose"
    );
}

// ---------------------------------------------------------------------------
// G-STATUS-04: [晕眩] 「无法收付款」
// ---------------------------------------------------------------------------

/// 「[晕眩]：…无法收付款」 -- rent owed to a stunned owner must not land on
/// them (the engine *does* gate the [晕眩] payee, unlike [除外]).
#[test]
fn stun_owner_cannot_receive_rent() {
    let mut t = Table::vanilla(3);
    let tile = rules_spec::tile("银河拉面馆");
    t.set_owner(tile, Some(1));
    t.set_houses(tile, 1);
    t.set_money(0, 50_000);
    t.set_money(1, 0);
    with_clauses(&mut t);
    t.set_state(1, "stun", 1);
    t.set_pos(0, tile.saturating_sub(1).max(0));
    t.dice(&[1, 1, 1, 1]);
    t.begin_turn(0);
    finish_turn(&mut t, 0);
    assert_eq!(
        t.money(1),
        0,
        "G-STATUS-04: stunned owner received rent (「无法收付款」)"
    );
}

// ---------------------------------------------------------------------------
// G-STATUS-06: [除外] 「无法收付款」 -- cannot receive
// ---------------------------------------------------------------------------

/// 「[除外]：…无法收付款」 -- rent owed to an exiled owner must not land on
/// them. (MyGO:无路矢's 「[除外]期间本应获得的格子收入由此前指定的那名玩家
/// 获得」 is a *redirect* to someone else, not an exception that pays the
/// exiled player.)
///
/// **Genuine discrepancy (stays failing):** the engine pays the exiled owner.
/// See `docs/rulebook/spec/global/status.md` § Findings (G-STATUS-06).
#[test]
fn exile_owner_cannot_receive_rent() {
    let mut t = Table::vanilla(3);
    let tile = rules_spec::tile("银河拉面馆");
    t.set_pos(0, 0);
    t.set_owner(tile, Some(1));
    t.set_houses(tile, 1);
    t.set_money(0, 50_000);
    t.set_money(1, 0);
    with_clauses(&mut t);
    // Exile the owner *without* starting their turn (a turn start would tick
    // the layer off). Player 0 then lands on their tile and pays rent.
    t.set_state(1, "exile", 1);
    t.set_state(1, "exileTo", -1);
    t.set_pos(0, tile.saturating_sub(1).max(0));
    t.dice(&[1, 1, 1, 1]);
    t.begin_turn(0);
    finish_turn(&mut t, 0);
    assert_eq!(
        t.state(1, "exile"),
        1,
        "setup: player 1 should still be exiled after player 0's turn"
    );
    assert_eq!(
        t.money(1),
        0,
        "G-STATUS-06: exiled owner received rent (「无法收付款」)"
    );
}

// ---------------------------------------------------------------------------
// G-STATUS-02: [晕眩] in force at the roll
// ---------------------------------------------------------------------------

/// 「[晕眩]：处于该状态时[无法移动]」 + row 4 「跳过…主要移动阶段」 -- once
/// stunned, the [主要移动] must not walk. Mid-运营 [晕眩] is enough (present
/// tense).
///
/// **Genuine discrepancy (stays failing):** the engine still rolls and walks.
/// See `docs/rulebook/spec/global/status.md` § Findings (G-STATUS-02).
#[test]
fn stun_in_force_at_roll_does_not_walk() {
    let mut t = Table::vanilla(3);
    t.dice(&[5, 5, 5, 5]);
    with_clauses(&mut t);
    let before = t.pos(0);
    // Mid-运营 [晕眩]: 「处于该状态时[无法移动]」.
    t.set_state(0, "stun", 1);
    let r = t.roll(0);
    if r.is_ok() {
        assert_eq!(
            t.pos(0),
            before,
            "G-STATUS-02: stunned player's [主要移动] landed elsewhere (起点 {before}, now {})",
            t.pos(0)
        );
    }
    // A refused roll is also fine (the skip form of the same clause).
}
