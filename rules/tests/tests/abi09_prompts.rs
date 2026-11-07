//! ABI-09 / HOST-05: the free-amount money ask and the cosmetic skin list.
//!
//! `ask_money` is a prompt kind with **no option list**: the bound rides
//! `view.price`, the answer is a raw amount in `0..=max`. `persona_skins`
//! names the other personas of the player's base character (`variant_of`).

mod common;
use common::*;
use game_core::msg::Arg;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// The `n` argument of the first log line with `key` at or after `mark`.
fn logged_int(t: &Table, mark: i32, key: &str) -> Option<i64> {
    for e in t.events_since(mark) {
        if e.msg.key().ends_with(key) {
            if let Some(Arg::Integer(n)) = e.msg.args.get("n") {
                return Some(*n);
            }
        }
    }
    None
}

// 「支付选择数量的资金」 -- the ask has no options, carries the maximum and
// answers the raw amount.
#[test]
fn ask_money_carries_the_bound_and_answers_the_amount() {
    let mut t = Table::vanilla(2);
    let mark = t.mark();
    t.give_play(0, "TEST:askMoney").unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.kind, "money", "{}", t.dump_prompt());
    assert!(p.options.is_empty(), "a money ask lists no options");
    assert_eq!(p.price, 5_000, "the maximum rides the view");
    t.answer(0, 2_500).unwrap();
    drain(&mut t);
    assert_eq!(
        logged_int(&t, mark, "ask_money_value"),
        Some(2_500),
        "the raw answer reaches the body: {:?}",
        t.recent_keys(12)
    );
}

#[test]
fn ask_money_rejects_an_out_of_range_amount() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "TEST:askMoney").unwrap();
    let _ = t.expect_prompt();
    assert!(t.answer(0, -1).is_err(), "no amount is below the range");
    assert!(t.answer(0, 5_001).is_err(), "the amount is capped at the max");
    t.answer(0, 5_000).unwrap();
    drain(&mut t);
}

// A base character's skins are its `variant_of` rows; the current persona is
// excluded and an unrelated character has none.
#[test]
fn persona_skins_lists_the_variants_of_the_base_character() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丰川祥子");
    let mark = t.mark();
    t.give_play(0, "TEST:personaSkins").unwrap();
    drain(&mut t);
    assert_eq!(logged_int(&t, mark, "persona_skins_n"), Some(1));
    let names: Vec<String> = t
        .events_since(mark)
        .into_iter()
        .filter(|e| e.msg.key().ends_with("persona_skins_name"))
        .filter_map(|e| match e.msg.args.get("name") {
            Some(Arg::Persona(n)) => Some(n.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["丰川祥子（CRYCHIC）".to_string()]);
}

#[test]
fn persona_skins_is_empty_for_a_character_without_variants() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "千早爱音");
    let mark = t.mark();
    t.give_play(0, "TEST:personaSkins").unwrap();
    drain(&mut t);
    assert_eq!(logged_int(&t, mark, "persona_skins_n"), Some(0));
}
