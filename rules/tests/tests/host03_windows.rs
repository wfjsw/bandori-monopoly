//! HOST-03: the 支付阶段 windows have their own positions.
//!
//! 规则书 支付阶段 (data/rules.txt): window 1 「支付计算前」 precedes window 3
//! 「支付计算后」; window 7 「资金变动」 sees the settled figure. The probe
//! `TEST:payCalcProbe` logs window 1 (`payCalcBefore`) and window 3
//! (`paySplitMulBefore`); the order of the two lines is the assertion.

mod common;
use common::*;

/// Decline every open prompt.
fn drain(t: &mut Table) {
    for _ in 0..40 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
    panic!("drain: prompts never stopped: {}", t.dump_prompt());
}

/// Window 1 is raised before window 3 on one payment.
#[test]
fn pay_calc_before_precedes_pay_calc_after() {
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, "TEST:payCalcProbe");
    t.give_play(0, "TEST:lose1000").unwrap();
    drain(&mut t);
    let keys = t.recent_keys(32);
    let w1 = keys
        .iter()
        .position(|k| k.contains("pay_calc_w1"))
        .unwrap_or_else(|| panic!("no window-1 line: {keys:?}"));
    let w3 = keys
        .iter()
        .position(|k| k.contains("pay_calc_w3"))
        .unwrap_or_else(|| panic!("no window-3 line: {keys:?}"));
    assert!(w1 < w3, "window 1 precedes window 3: {keys:?}");
}

/// Window 5 「支付时」 is raised after the calculation windows and reads the
/// computed figure; the money actually moves by exactly that figure.
#[test]
fn pay_window_follows_the_calculation_windows() {
    let space = tile("Space");
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    t.own(1, &[space]);
    t.set_money(0, 10_000);
    t.set_money(1, 10_000);
    t.place_raw(0, "TEST:payCalcProbe");
    t.set_pos(0, space - 1);
    t.begin_turn(0);
    let mark = t.mark();
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    let keys = t.recent_keys(48);
    let w1 = keys
        .iter()
        .position(|k| k.contains("pay_calc_w1"))
        .unwrap_or_else(|| panic!("no window-1 line: {keys:?}"));
    let w3 = keys
        .iter()
        .position(|k| k.contains("pay_calc_w3"))
        .unwrap_or_else(|| panic!("no window-3 line: {keys:?}"));
    let w5 = keys
        .iter()
        .position(|k| k.contains("pay_calc_w5"))
        .unwrap_or_else(|| panic!("no window-5 line: {keys:?}"));
    assert!(w1 < w3 && w3 < w5, "the windows keep their order: {keys:?}");
    let seen: Vec<i64> = t
        .events_since(mark)
        .iter()
        .filter(|e| e.msg.key().contains("pay_calc_w5"))
        .filter_map(|e| int_arg(&e.msg, "value"))
        .collect();
    let paid = (10_000 - t.money(0)) as i64;
    assert!(paid > 0, "the rent was paid");
    assert_eq!(seen, vec![paid], "window 5 read the figure that moved");
    assert_eq!(t.money(1) as i64, 10_000 + paid, "the payee got the same figure");
}

/// `paid` (支付阶段 7) carries the settled figure on a receipt and on a loss.
#[test]
fn paid_fires_on_a_gain_at_the_settled_figure() {
    // A gain (a print): the seat is the payee and `player_id` is -1.
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, "TEST:paidProbe");
    let mark = t.mark();
    t.give_play(0, "TEST:gain1000").unwrap();
    drain(&mut t);
    let gain: Vec<i64> = t
        .events_since(mark)
        .iter()
        .filter(|e| e.msg.key().contains("paid_seen"))
        .filter_map(|e| int_arg(&e.msg, "value"))
        .collect();
    assert_eq!(gain, vec![1000], "the receipt fires `paid` with its gain");

    // A loss: the settled loss is the figure.
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, "TEST:paidProbe");
    let mark = t.mark();
    t.give_play(0, "TEST:lose1000").unwrap();
    drain(&mut t);
    let loss: Vec<i64> = t
        .events_since(mark)
        .iter()
        .filter(|e| e.msg.key().contains("paid_seen"))
        .filter_map(|e| int_arg(&e.msg, "value"))
        .collect();
    assert_eq!(loss, vec![1000], "the loss fires `paid` with its loss");
}
