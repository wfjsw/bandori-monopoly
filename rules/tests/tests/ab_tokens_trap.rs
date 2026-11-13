//! HAG-01 (held-token `markerChanged` event) and DECLARE-TRAP-LOG (every
//! "the entry did not fire" trap logs).
//!
//! 规则书 (skill sheet, 北泽育美):
//! > 其他角色拥有4个可乐饼时自动移除所有可乐饼停留一回合。
//!
//! Held-token writes raise the post-commit `markerChanged` event; the overflow
//! clause is a handler on it, filtered to another holder (`actor != owner`).
//! A transfer to another player trips the sweep exactly once; the owner's own
//! gain does not.
//!
//! docs/ACTIVATION.md §2: a trapping residual guard / `OP_DECLARE` declines
//! the entry, leaves the card untouched, and logs one `log.card_trap` line.

mod common;
use common::*;
use game_core::msg::Arg;
use game_core::state::Counter;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// Push one unbound held-token row (no instance, no cap) -- the arrangement
/// the `hhw` suite uses for 可乐饼.
fn set_held(t: &mut Table, who: usize, name: &str, value: i32) {
    t.m.world_mut().st.players[who].tokens.push(Counter {
        name: name.into(),
        value,
        instance: None,
        max: 0,
    });
}

/// `log.card_trap` lines at or after `mark`.
fn trap_lines(t: &Table, mark: i32) -> Vec<String> {
    t.events_since(mark)
        .into_iter()
        .filter(|e| e.msg.key().ends_with("log.card_trap"))
        .map(|e| match e.msg.args.get("detail") {
            Some(Arg::Text(d)) => d.clone(),
            _ => String::new(),
        })
        .collect()
}

fn key_count(t: &Table, mark: i32, key: &str) -> usize {
    t.keys_since(mark)
        .iter()
        .filter(|k| k.ends_with(key))
        .count()
}

// -- DECLARE-TRAP-LOG: residual guard ---------------------------------------

/// A trapping residual guard declines the entry -- but not silently. One
/// `log.card_trap` line naming the card and "guard trapped"; the card stays in
/// play and the body never runs. `TEST:trap_guard_ok` (same card, switch off)
/// runs the body with no trap line.
#[test]
fn trapping_guard_logs_and_leaves_the_card() {
    let mut t = Table::new(&["户山香澄", "千早爱音"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give_play(0, "TEST:trap_guard").unwrap();
    drain(&mut t);
    assert!(
        t.on_field(0, "TEST:trap_guard"),
        "the fixture placed: {:?}",
        t.field_ids(0)
    );
    t.set_pos(0, 0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.end(0).unwrap();
    drain(&mut t);
    let traps = trap_lines(&t, mark);
    assert_eq!(
        traps.len(),
        1,
        "one `log.card_trap` for the trapping guard: {:?}",
        t.recent_keys(40)
    );
    assert!(
        traps[0].contains("guard trapped"),
        "the line names the guard arm: {:?}",
        traps[0]
    );
    assert_eq!(
        key_count(&t, mark, "trap_guard_body"),
        0,
        "a declined entry runs no body: {:?}",
        t.recent_keys(40)
    );
    assert!(
        t.on_field(0, "TEST:trap_guard"),
        "a trapping guard never costs the card: {:?}",
        t.field_ids(0)
    );
}

#[test]
fn admitting_guard_logs_nothing() {
    let mut t = Table::new(&["户山香澄", "千早爱音"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give_play(0, "TEST:trap_guard_ok").unwrap();
    drain(&mut t);
    t.set_pos(0, 0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.end(0).unwrap();
    drain(&mut t);
    assert_eq!(
        trap_lines(&t, mark).len(),
        0,
        "the switch-off sibling does not trap: {:?}",
        t.recent_keys(40)
    );
    assert_eq!(
        key_count(&t, mark, "trap_guard_body"),
        1,
        "the body ran: {:?}",
        t.recent_keys(40)
    );
}

// -- DECLARE-TRAP-LOG: hook `OP_DECLARE` ------------------------------------

/// `TEST:trap_declare` is a placed card whose TurnEnd hook collector panics.
/// The trap declines the entry, logs one `log.card_trap` naming "declare
/// trapped", keeps the card in play, and never runs the body.
#[test]
fn trapping_hook_declare_logs_and_leaves_the_card() {
    let mut t = Table::new(&["户山香澄", "千早爱音"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.give_play(0, "TEST:trap_declare").unwrap();
    drain(&mut t);
    assert!(
        t.on_field(0, "TEST:trap_declare"),
        "the fixture placed: {:?}",
        t.field_ids(0)
    );
    t.set_pos(0, 0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    let mark = t.mark();
    t.end(0).unwrap();
    drain(&mut t);
    let traps = trap_lines(&t, mark);
    assert_eq!(
        traps.len(),
        1,
        "one `log.card_trap` for the trapping collector: {:?}",
        t.recent_keys(40)
    );
    assert!(
        traps[0].contains("declare trapped"),
        "the line names the declare arm: {:?}",
        traps[0]
    );
    assert_eq!(
        key_count(&t, mark, "trap_declare_body"),
        0,
        "the body never ran: {:?}",
        t.recent_keys(40)
    );
    assert!(
        t.on_field(0, "TEST:trap_declare"),
        "a trapping collector never costs the card: {:?}",
        t.field_ids(0)
    );
}

// -- DECLARE-TRAP-LOG: play `OP_DECLARE` ------------------------------------

/// A hand card whose play collector traps: the play declines, the card stays
/// in hand, one `log.card_trap` lands and the body does not run.
#[test]
fn trapping_play_declare_logs_and_stays_in_hand() {
    let mut t = Table::new(&["户山香澄", "千早爱音"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_hand(0, &["TEST:trap_play_declare"]);
    let mark = t.mark();
    t.play(0, "TEST:trap_play_declare").unwrap();
    drain(&mut t);
    let traps = trap_lines(&t, mark);
    assert_eq!(
        traps.len(),
        1,
        "one `log.card_trap` for the trapping collector: {:?}",
        t.recent_keys(40)
    );
    // The play declare goes through the standalone `declare_entry` collector,
    // so its detail is the bare wasm error -- non-empty is the point.
    assert!(
        !traps[0].is_empty(),
        "the line carries the trap detail: {:?}",
        traps[0]
    );
    assert_eq!(
        key_count(&t, mark, "trap_play_body"),
        0,
        "the body never ran: {:?}",
        t.recent_keys(40)
    );
    assert_eq!(
        t.hand(0),
        vec!["TEST:trap_play_declare".to_string()],
        "a trapping play collector leaves the card in hand"
    );
}

// -- HAG-01: markerChanged sweep -------------------------------------------

/// 规则书: 「其他角色拥有4个可乐饼时自动移除所有可乐饼停留一回合」 -- a
/// transfer to *another* player is that player's write (`actor = recipient`,
/// not the owner). The `markerChanged` handler sweeps them exactly once: all
/// croquettes gone, one [停留] layer, one outcome line.
#[test]
fn transfer_to_another_player_trips_the_sweep_once() {
    let mut t = Table::new(&["北泽育美", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    set_held(&mut t, 0, "可乐饼", 4);
    let mark = t.mark();
    t.give_play(0, "TEST:hagumiGive").unwrap();
    drain(&mut t);
    assert_eq!(
        t.token(1, "可乐饼"),
        0,
        "the sweep removed every croquette: P1={}",
        t.token(1, "可乐饼")
    );
    assert_eq!(t.state(1, "stay"), 1, "one [停留] layer");
    assert_eq!(
        key_count(&t, mark, "hagumi_overflow"),
        1,
        "exactly one sweep, not one per write: {:?}",
        t.recent_keys(40)
    );
    assert_eq!(
        t.token(0, "可乐饼"),
        0,
        "the transfer itself moved them off the owner"
    );
}

/// The owner's **own** gain to 4 is her write (`actor == owner`): the handler
/// is filtered out, so nothing is swept.
#[test]
fn owners_own_gain_to_four_does_not_sweep() {
    let mut t = Table::new(&["北泽育美", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let mark = t.mark();
    t.give_play(0, "TEST:hagumiGain").unwrap();
    drain(&mut t);
    assert_eq!(t.token(0, "可乐饼"), 4, "her gain stands");
    assert_eq!(
        key_count(&t, mark, "hagumi_overflow"),
        0,
        "no sweep for the owner's own write: {:?}",
        t.recent_keys(40)
    );
    assert_eq!(t.state(0, "stay"), 0, "no [停留] layer");
}
