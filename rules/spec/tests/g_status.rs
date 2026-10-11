//! Scenario + iff tests for G-STATUS ([停留] / [晕眩] / [除外] /
//! [不可阻挡] / [无法移动] / [异常移动效果]).
//!
//! Standard (spec-brief-addendum): a contract passes **iff** the engine does
//! exactly what the book says -- not less and not more. Each contract is a
//! pure `verify_*` (in `rules_spec::global::status`) plus a driver; the
//! `*_verifier_rejects_mutants` tests prove the predicate is two-sided.
//!
//! Book quotes: `docs/rulebook/spec/global/status.md`.
//! A genuine discrepancy is a plain failing test (no `#[ignore]`).

use game_core::engine::World;
use game_core::msg::Msg;
use game_core::state::MatchEvent;
use rules_spec::global::status::{
    MoneyGate, verify_exile_return, verify_exile_turn_start, verify_layers_nonneg,
    verify_main_move_skipped, verify_no_money_status, verify_stay_turn_end,
    verify_stun_turn_end, verify_unstoppable, ABNORMAL_MOVE_EFFECTS, CLAUSES,
};
use rules_spec::{frame, Table};

// ---------------------------------------------------------------------------
// drivers / helpers
// ---------------------------------------------------------------------------

/// Arm the area's per-act clauses.
fn with_clauses(t: &mut Table) {
    t.observe(CLAUSES);
}

/// Put `key` on `who` and re-run their turn start so the skip latch matches a
/// status already in force when the [主要移动] opens (book: 「处于该状态时」).
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

/// A `"text"` `log.end_turn` line for `who` (the window's turn-end mark).
fn end_turn_event(who: usize) -> MatchEvent {
    let mut e = MatchEvent::default();
    e.r#type = "text".into();
    e.player_id = who as i32;
    e.msg = Msg::new("log.end_turn");
    e
}

/// A `"turn"` `log.turn` line for `who` (the window's turn-start mark).
fn turn_start_event(who: usize) -> MatchEvent {
    let mut e = MatchEvent::default();
    e.r#type = "turn".into();
    e.player_id = who as i32;
    e.msg = Msg::new("log.turn");
    e
}

/// A `"status"` `log.stunned_skip` line for `who`.
fn stunned_skip_event(who: usize) -> MatchEvent {
    let mut e = MatchEvent::default();
    e.r#type = "status".into();
    e.player_id = who as i32;
    e.msg = Msg::new("log.stunned_skip");
    e
}

/// A `"pay"` / `"rent"` leg of `amount` between `from` and `to`.
fn money_event(kind: &str, from: i32, to: i32, amount: i32) -> MatchEvent {
    let mut e = MatchEvent::default();
    e.r#type = kind.into();
    e.player_id = from;
    e.other = to;
    e.value = amount;
    e.msg = Msg::new(format!("log.{kind}"));
    e
}

/// A `log.outcome.stay` grant line targeting `who`.
fn grant_event(kind: &str, who: usize) -> MatchEvent {
    let mut e = MatchEvent::default();
    e.r#type = "text".into();
    e.player_id = who as i32;
    e.msg = Msg::new(format!("log.outcome.{kind}")).player_id("target", who as i32);
    e
}

/// Empty two-player worlds for hand-built triples (vanilla arrange, then
/// snapshot). `n` players, all at tile 0, money 1000.
fn empty_worlds(n: usize) -> (World, World) {
    let t = Table::vanilla(n);
    let pre = t.snapshot_world();
    (pre.clone(), pre)
}

/// Write a layer count **raw** (bypass `state_set`'s floor-at-0 so a mutant
/// can carry a negative).
fn set_layer(w: &mut World, who: usize, key: &str, v: i32) {
    let e = w.st.players[who].state.entry(key.to_string()).or_default();
    e.value = v;
}

fn set_money(w: &mut World, who: usize, v: i32) {
    w.st.players[who].money = v;
}

fn set_pos(w: &mut World, who: usize, v: i32) {
    w.st.players[who].pos = v;
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

/// G-STATUS-01 driver: non-negative across a turn end.
#[test]
fn status_layers_never_negative() {
    let mut t = Table::vanilla(3);
    with_clauses(&mut t);
    for who in 0..3 {
        t.set_state(who, "stay", 1);
        t.set_state(who, "stun", 1);
        t.set_state(who, "exile", 1);
    }
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

/// G-STATUS-01 iff: the pure verifier accepts 0 and rejects -1.
#[test]
fn layers_nonneg_verifier_rejects_mutants() {
    let (pre, mut post) = empty_worlds(2);
    set_layer(&mut post, 0, "stay", 0);
    verify_layers_nonneg(&post).expect("0 layers is fine");
    // LESS is not a thing here (the rule is a lower bound). WRONG: -1.
    set_layer(&mut post, 0, "stay", -1);
    assert!(
        verify_layers_nonneg(&post).is_err(),
        "G-STATUS-01: verifier must reject a negative layer"
    );
    let _ = pre;
}

// ---------------------------------------------------------------------------
// G-STATUS-03: [停留] decay at end of own turn (exact + frame + mutant)
// ---------------------------------------------------------------------------

/// 「[停留]：…玩家的每回合结束时移除一层。」 -- exactly one layer.
#[test]
fn stay_decays_one_layer_at_end_of_own_turn() {
    for pre_layers in [1, 2, 3] {
        let mut t = Table::vanilla(3);
        with_clauses(&mut t);
        arm_status_before_turn(&mut t, 0, "stay", pre_layers);
        let pre = t.snapshot_world();
        finish_turn(&mut t, 0);
        let post = t.snapshot_world();
        let got = t.state(0, "stay");
        assert_eq!(
            got,
            pre_layers - 1,
            "G-STATUS-03: [停留] {pre_layers} -> {got} over the owner's turn end (want {})",
            pre_layers - 1
        );
        // Exact frame: only P0's stay may move on a pure decay.
        frame(&pre, &post, &["state:P0:stay"]).expect("G-STATUS-03 frame");
    }
}

/// Stacking: two layers stay two until a turn end takes one.
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

/// Condition false (not their turn end): the decay is a no-op.
#[test]
fn stay_decay_is_noop_off_its_tick() {
    let (mut pre, _) = empty_worlds(2);
    set_layer(&mut pre, 0, "stay", 3);
    let post = pre.clone();
    // Quiet window with no turn-end mark: unchanged is accepted.
    let events: [MatchEvent; 0] = [];
    verify_stay_turn_end(&pre, &post, &events, 0).expect("no-op when condition false");
    assert_eq!(
        post.st.players[0].state_get("stay"),
        3,
        "G-STATUS-03: stay must be unchanged off its tick"
    );
    // LESS/WRONG: a shrink off the tick must be rejected.
    let mut shrank = post.clone();
    set_layer(&mut shrank, 0, "stay", 0);
    assert!(
        verify_stay_turn_end(&pre, &shrank, &events, 0).is_err(),
        "G-STATUS-03: reject a decay that fires off its tick"
    );
}

/// Mutants on a hand-built compliant stay-decay triple.
#[test]
fn stay_decay_verifier_rejects_mutants() {
    let mut pre = empty_worlds(2).0;
    set_layer(&mut pre, 0, "stay", 2);
    let mut post = pre.clone();
    set_layer(&mut post, 0, "stay", 1);
    let events = vec![end_turn_event(0)];

    // Genuine (hand-built compliant post) is accepted.
    verify_stay_turn_end(&pre, &post, &events, 0).expect("compliant decay");

    // LESS: no decay.
    let less = pre.clone();
    assert!(
        verify_stay_turn_end(&pre, &less, &events, 0).is_err(),
        "G-STATUS-03: reject missing decay"
    );

    // WRONG: double decay.
    let mut wrong = post.clone();
    set_layer(&mut wrong, 0, "stay", 0);
    assert!(
        verify_stay_turn_end(&pre, &wrong, &events, 0).is_err(),
        "G-STATUS-03: reject double decay"
    );

    // WRONG: +1 instead of -1.
    let mut up = post.clone();
    set_layer(&mut up, 0, "stay", 3);
    assert!(
        verify_stay_turn_end(&pre, &up, &events, 0).is_err(),
        "G-STATUS-03: reject a gain at turn end"
    );

    // MORE: unrelated path moved (other player's money).
    let mut more = post.clone();
    set_money(&mut more, 1, 1);
    // The decay verifier itself does not own money; frame() is the MORE
    // check. Assert frame rejects the leak.
    let fr = frame(&pre, &more, &["state:P0:stay"]);
    assert!(fr.is_err(), "G-STATUS-03 frame: reject unrelated money move");

    // A grant line makes the window non-quiet: exact -1 no longer applies
    // (`give_stay(p, 2)` logs one line for two layers).
    let mut ev2 = events.clone();
    ev2.push(grant_event("stay", 0));
    let mut with_grant = post.clone();
    set_layer(&mut with_grant, 0, "stay", 3);
    verify_stay_turn_end(&pre, &with_grant, &ev2, 0)
        .expect("grant window is not an exact-decay window");
}

// ---------------------------------------------------------------------------
// G-STATUS-05: [晕眩] decay + row 4 skip
// ---------------------------------------------------------------------------

/// 「[晕眩]：…玩家的每回合结束时移除一层。」 + row 4 auto-skip.
#[test]
fn stun_decays_one_layer_and_skips_turn() {
    for pre_layers in [1, 2] {
        let mut t = Table::vanilla(3);
        with_clauses(&mut t);
        t.set_state(1, "stun", pre_layers);
        let pre = t.snapshot_world();
        finish_turn(&mut t, 0);
        let post = t.snapshot_world();
        let got = t.state(1, "stun");
        assert_eq!(
            got,
            pre_layers - 1,
            "G-STATUS-05: [晕眩] {pre_layers} -> {got} over the owner's turn end (want {})",
            pre_layers - 1
        );
        assert_ne!(
            t.turn(),
            1,
            "G-STATUS-05: stunned player's turn should auto-skip (row 4)"
        );
        frame(&pre, &post, &["state:P1:stun", "state:P1:stunStart", "pos:P0"])
            .expect("G-STATUS-05 frame");
    }
}

/// Mutants on a compliant [晕眩] decay + skip triple.
#[test]
fn stun_decay_verifier_rejects_mutants() {
    let mut pre = empty_worlds(2).0;
    set_layer(&mut pre, 1, "stun", 2);
    let mut post = pre.clone();
    set_layer(&mut post, 1, "stun", 1);
    let events = vec![end_turn_event(1)];

    verify_stun_turn_end(&pre, &post, &events, 1).expect("compliant decay");

    // LESS: no decay.
    assert!(
        verify_stun_turn_end(&pre, &pre.clone(), &events, 1).is_err(),
        "G-STATUS-05: reject missing [晕眩] decay"
    );

    // WRONG: double decay.
    let mut wrong = post.clone();
    set_layer(&mut wrong, 1, "stun", 0);
    assert!(
        verify_stun_turn_end(&pre, &wrong, &events, 1).is_err(),
        "G-STATUS-05: reject double [晕眩] decay"
    );

    // Row 4: turn start while still stunned must skip. `stunned_skip` also
    // marks the turn end, so the compliant post has decayed one layer.
    let mut pre2 = empty_worlds(2).0;
    set_layer(&mut pre2, 1, "stun", 2);
    let mut post2 = pre2.clone();
    set_layer(&mut post2, 1, "stun", 1);
    let ev_skip = vec![turn_start_event(1), stunned_skip_event(1)];
    verify_stun_turn_end(&pre2, &post2, &ev_skip, 1).expect("skip present");
    // No skip line, but still stunned after the start: row 4 broken. Use a
    // window that only marks turn start (no end), so no decay is required.
    let mut pre3 = empty_worlds(2).0;
    set_layer(&mut pre3, 1, "stun", 1);
    let post3 = pre3.clone();
    let ev_noskip = vec![turn_start_event(1)];
    assert!(
        verify_stun_turn_end(&pre3, &post3, &ev_noskip, 1).is_err(),
        "G-STATUS-05: reject a stunned turn that did not skip (row 4)"
    );
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

/// Condition false: an un-stunned player's [手] play is allowed (the gate is
/// a no-op -- the engine must NOT block).
#[test]
fn stun_gate_is_noop_when_not_stunned() {
    let mut t = Table::vanilla(3);
    t.give(0, &["通用:GREAT"]);
    with_clauses(&mut t);
    t.play(0, "通用:GREAT")
        .expect("G-STATUS-04: play must work when not stunned (no-op gate)");
}

/// Mutants on the [晕眩] money gate.
#[test]
fn stun_money_verifier_rejects_mutants() {
    let mut pre = empty_worlds(2).0;
    set_layer(&mut pre, 1, "stun", 1);
    set_money(&mut pre, 1, 0);
    let mut post = pre.clone();
    let events = vec![money_event("rent", 0, 1, 760)];

    // Compliant: the rent leg exists but the stunned payee's money is unchanged
    // (the engine *should* have blocked it). Our verifier flags any amount on
    // a stunned seat -- so the *compliant* post for the BOOK is "no rent leg",
    // or a blocked window. Here we build the book-compliant shape: no money
    // leg involving the stunned seat.
    let quiet: [MatchEvent; 0] = [];
    verify_no_money_status(&pre, &post, &quiet, 1, MoneyGate::Stun).expect("quiet is fine");

    // MORE / WRONG: a rent leg that moved the stunned payee.
    set_money(&mut post, 1, 760);
    assert!(
        verify_no_money_status(&pre, &post, &events, 1, MoneyGate::Stun).is_err(),
        "G-STATUS-04: reject a rent landing on a stunned payee"
    );

    // LESS side of the book rule (cannot pay either): a pay leg from them.
    let pay = vec![money_event("pay", 1, 0, 760)];
    let mut post2 = pre.clone();
    set_money(&mut post2, 1, -760);
    set_money(&mut post2, 0, 760);
    assert!(
        verify_no_money_status(&pre, &post2, &pay, 1, MoneyGate::Stun).is_err(),
        "G-STATUS-04: reject a pay by a stunned payer"
    );

    // Condition false: an un-stunned pre is a no-op for this gate.
    let (mut free, _) = empty_worlds(2);
    set_money(&mut free, 1, 0);
    let mut free_post = free.clone();
    set_money(&mut free_post, 1, 760);
    assert!(
        verify_no_money_status(&free, &free_post, &events, 1, MoneyGate::Stun).is_ok(),
        "G-STATUS-04: no-op when not stunned"
    );
}

// ---------------------------------------------------------------------------
// G-STATUS-07: [除外] decay before turn start; no re-stack
// ---------------------------------------------------------------------------

/// 「玩家的每回合开始前移除一层」 -- exactly one layer.
#[test]
fn exile_decays_one_layer_before_turn_start() {
    for pre_layers in [1, 2, 3] {
        let mut t = Table::vanilla(3);
        with_clauses(&mut t);
        t.set_state(1, "exile", pre_layers);
        t.set_state(1, "exileTo", -1);
        let pre = t.snapshot_world();
        finish_turn(&mut t, 0);
        let post = t.snapshot_world();
        let got = t.state(1, "exile");
        assert_eq!(
            got,
            pre_layers - 1,
            "G-STATUS-07: [除外] {pre_layers} -> {got} before the owner's turn start (want {})",
            pre_layers - 1
        );
        frame(&pre, &post, &["state:P1:exile", "state:P1:exileTo", "pos:P0"])
            .expect("G-STATUS-07 frame");
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
    let pre = t.snapshot_world();
    finish_turn(&mut t, 0);
    let post = t.snapshot_world();
    assert_eq!(t.state(1, "exile"), 0, "G-STATUS-08: the last layer ticked out");
    assert_eq!(
        t.pos(1),
        dest,
        "G-STATUS-08: pawn landed on the named return tile"
    );
    // Frame: only the exile keys and the pawn's pos.
    frame(
        &pre,
        &post,
        &["state:P1:exile", "state:P1:exileTo", "pos:P1", "pos:P0"],
    )
    .expect("G-STATUS-08 frame");
}

/// Mutants on [除外] turn-start decay + no re-acquire.
#[test]
fn exile_decay_verifier_rejects_mutants() {
    let mut pre = empty_worlds(2).0;
    set_layer(&mut pre, 1, "exile", 2);
    let mut post = pre.clone();
    set_layer(&mut post, 1, "exile", 1);
    let events = vec![turn_start_event(1)];

    verify_exile_turn_start(&pre, &post, &events, 1).expect("compliant decay");

    // LESS: no decay.
    assert!(
        verify_exile_turn_start(&pre, &pre.clone(), &events, 1).is_err(),
        "G-STATUS-07: reject missing [除外] decay"
    );

    // WRONG: double decay.
    let mut wrong = post.clone();
    set_layer(&mut wrong, 1, "exile", 0);
    assert!(
        verify_exile_turn_start(&pre, &wrong, &events, 1).is_err(),
        "G-STATUS-07: reject double [除外] decay"
    );

    // MORE: re-acquisition while active (even if decay cancels it).
    let mut ev = events.clone();
    ev.push(grant_event("exile", 1));
    let mut reacq = pre.clone();
    set_layer(&mut reacq, 1, "exile", 2); // 2 -1 decay +1 grant = 2, or net 2
    assert!(
        verify_exile_turn_start(&pre, &reacq, &ev, 1).is_err(),
        "G-STATUS-07: reject re-acquiring [除外] while active"
    );
}

/// Return-[传送] mutants (G-STATUS-08).
#[test]
fn exile_return_verifier_rejects_mutants() {
    let mut pre = empty_worlds(2).0;
    set_layer(&mut pre, 1, "exile", 1);
    pre.st.players[1]
        .state
        .insert("exileTo".into(), Default::default());
    pre.st.players[1].state_set("exileTo", 7);
    set_pos(&mut pre, 1, 0);
    let mut post = pre.clone();
    set_layer(&mut post, 1, "exile", 0);
    set_pos(&mut post, 1, 7);
    let events = vec![turn_start_event(1)];

    verify_exile_return(&pre, &post, &events, 1).expect("compliant return");

    // WRONG: landed somewhere else.
    let mut wrong = post.clone();
    set_pos(&mut wrong, 1, 3);
    assert!(
        verify_exile_return(&pre, &wrong, &events, 1).is_err(),
        "G-STATUS-08: reject landing off the named tile"
    );

    // Condition false: exile did not tick out (still 1) -- no return required.
    let mut still = pre.clone();
    set_layer(&mut still, 1, "exile", 1);
    set_pos(&mut still, 1, 0);
    verify_exile_return(&pre, &still, &events, 1).expect("no return when not expired");
}

/// Still-exiled turns are skipped.
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
// G-STATUS-02: [停留] / [晕眩] skip the main move
// ---------------------------------------------------------------------------

/// Row 7 「停留效果发动：跳过主要移动阶段」.
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

/// Mutants on the [无法移动] main-move skip.
#[test]
fn cannot_move_roll_verifier_rejects_mutants() {
    let mut pre = empty_worlds(2).0;
    set_layer(&mut pre, 0, "stay", 1);
    pre.st.skip_move = true;
    set_pos(&mut pre, 0, 4);
    let post = pre.clone();
    // Compliant: stays put, no settle.
    let events: [MatchEvent; 0] = [];
    verify_main_move_skipped(&pre, &post, &events, 0, true).expect("compliant skip");

    // LESS / WRONG: walked away.
    let mut moved = post.clone();
    set_pos(&mut moved, 0, 9);
    assert!(
        verify_main_move_skipped(&pre, &moved, &events, 0, true).is_err(),
        "G-STATUS-02: reject a walk while [无法移动]"
    );

    // MORE: settled despite 「且不[结算]」.
    let mut ev = vec![MatchEvent::default()];
    ev[0].r#type = "text".into();
    ev[0].msg = Msg::new("log.land_unowned");
    assert!(
        verify_main_move_skipped(&pre, &post, &ev, 0, true).is_err(),
        "G-STATUS-02: reject a settle while [无法移动]"
    );

    // Condition false: not [无法移动] -- a walk is fine.
    let mut free = empty_worlds(2).0;
    set_pos(&mut free, 0, 4);
    let mut free_post = free.clone();
    set_pos(&mut free_post, 0, 9);
    verify_main_move_skipped(&free, &free_post, &events, 0, true)
        .expect("no-op when not [无法移动]");
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

/// Mutants: unstoppable forbids new [晕眩]/[除外] layers and a [停留] skip.
#[test]
fn unstoppable_verifier_rejects_mutants() {
    let mut pre = empty_worlds(2).0;
    set_layer(&mut pre, 0, "unstoppable", 1);
    set_layer(&mut pre, 0, "stun", 1);
    let post = pre.clone();
    // Compliant: no new layers (decay is a separate clause).
    let events: [MatchEvent; 0] = [];
    verify_unstoppable(&pre, &post, &events, 0).expect("compliant quiet");

    // MORE: a grant line for [晕眩].
    let ev = vec![grant_event("stun", 0)];
    assert!(
        verify_unstoppable(&pre, &post, &ev, 0).is_err(),
        "G-STATUS-09: reject a [晕眩] grant while unstoppable"
    );

    // MORE: counter rose with no line.
    let mut up = post.clone();
    set_layer(&mut up, 0, "stun", 5);
    assert!(
        verify_unstoppable(&pre, &up, &events, 0).is_err(),
        "G-STATUS-09: reject a [晕眩] rise"
    );

    // MORE: stay skip despite 「无视已存在的[停留]」.
    let mut skip = vec![MatchEvent::default()];
    skip[0].r#type = "status".into();
    skip[0].player_id = 0;
    skip[0].msg = Msg::new("log.stay_skip");
    assert!(
        verify_unstoppable(&pre, &post, &skip, 0).is_err(),
        "G-STATUS-09: reject a [停留] skip while unstoppable"
    );

    // Condition false: not unstoppable -- a grant is fine for this clause.
    let free = empty_worlds(2).0;
    assert!(
        verify_unstoppable(&free, &free.clone(), &ev, 0).is_ok(),
        "G-STATUS-09: no-op when not unstoppable"
    );
}

// ---------------------------------------------------------------------------
// G-STATUS-06 / 04: money gate (genuine finding stays failing)
// ---------------------------------------------------------------------------

/// 「[除外]：…无法收付款」 -- rent owed to an exiled owner must not land on
/// them.
///
/// **Genuine discrepancy (stays failing):** the engine pays the exiled owner.
/// Findings classification: **LESS** is wrong here -- the engine does *more*
/// (pays a seat the book says cannot receive). Class: **MORE** (money leaks
/// to a seat that must not receive).
#[test]
fn exile_owner_cannot_receive_rent() {
    let mut t = Table::vanilla(3);
    let tile = rules_spec::tile("银河拉面馆");
    t.set_owner(tile, Some(1));
    t.set_houses(tile, 1);
    t.set_money(0, 50_000);
    t.set_money(1, 0);
    with_clauses(&mut t);
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

/// 「[晕眩]：…无法收付款」 -- the [晕眩] payee *is* gated (contrast [除外]).
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

/// [晕眩] in force at the roll must not walk.
///
/// **Genuine discrepancy (stays failing).** Classification: **WRONG** (also
/// **MORE**) -- the destination is not the current tile, and the move+settle
/// run at all (the book forbids both). Matches status.md § Findings.
#[test]
fn stun_in_force_at_roll_does_not_walk() {
    let mut t = Table::vanilla(3);
    t.dice(&[5, 5, 5, 5]);
    with_clauses(&mut t);
    let before = t.pos(0);
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
}

// ---------------------------------------------------------------------------
// G-STATUS-06 exiled money verifier mutants (shared with 04)
// ---------------------------------------------------------------------------

/// Mutants: [除外] 「无法收付款」 (same verifier as [晕眩]).
#[test]
fn exile_money_verifier_rejects_mutants() {
    let mut pre = empty_worlds(2).0;
    set_layer(&mut pre, 1, "exile", 1);
    set_money(&mut pre, 1, 0);
    let mut post = pre.clone();
    set_money(&mut post, 1, 760);
    let events = vec![money_event("rent", 0, 1, 760)];

    verify_no_money_status(&pre, &post, &events, 1, MoneyGate::Exile)
        .expect_err("reject rent to an exiled payee");

    // Condition false: not exiled -- the same leg is a no-op for this clause.
    let (mut free, _) = empty_worlds(2);
    set_money(&mut free, 1, 0);
    let mut free_post = free.clone();
    set_money(&mut free_post, 1, 760);
    assert!(
        verify_no_money_status(&free, &free_post, &events, 1, MoneyGate::Exile).is_ok(),
        "G-STATUS-06: no-op when not exiled"
    );

    // Compliant: blocked window (no leg) or unchanged money.
    let quiet: [MatchEvent; 0] = [];
    verify_no_money_status(&pre, &pre.clone(), &quiet, 1, MoneyGate::Exile).expect("quiet ok");
}
