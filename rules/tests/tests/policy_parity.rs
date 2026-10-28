//! `bot-core::policy::act_for` == `Match::bot_action` on the same live state.
//!
//! S1 keeps the engine's copy next to the new policy module, so a parity test
//! pins the two together before the engine copy is deleted: same command, same
//! card / value / card list. Both sides decide on a throwaway clone of the same
//! world, so the RNG streams are identical and every surface must agree.
//!
//! Covered surfaces: a rich and a low-cash [pass-reward] prompt, a turn start
//! in 运营 (roll), a `skip_move` surface (end) and an over-hand discard.

use bot_core::policy::act_for;
use game_core::state::{stage, BotMentality};
use rules_spec::global::movement::BOARD_TILES;
use rules_spec::Table;

/// Make `who` a held bot seat: a standard bot whose `ai` is off, so nothing
/// answers its prompts during a tick and the two implementations decide.
fn hold(t: &mut Table, who: usize) {
    t.make_bot(who, BotMentality::Standard);
    t.m.world_mut().st.players[who].ai = false;
}

/// Both entries must produce the exact same command for `who`.
fn assert_same(t: &Table, who: usize, label: &str) {
    let member = t.m.state().players[who].member;
    let want = t.m.bot_action(member);
    let got = act_for(&t.m, member);
    assert_eq!(got, want, "{label}: act_for diverged from bot_action");
}

/// Roll past CiRCLE from `money` and stop with the [pass-reward] prompt open.
fn pass_reward_prompt(money: i32) -> Table {
    let mut t = Table::vanilla(2);
    hold(&mut t, 0);
    t.set_money(0, money);
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!"]);
    t.set_pos(0, BOARD_TILES - 2);
    t.set_pos(1, 10);
    t.begin_turn(0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    t.tick_until("the [pass-reward] prompt opens", |t| {
        t.prompt()
            .is_some_and(|p| p.title.key().contains("pass_reward"))
    });
    t
}

fn card_and_money_options(t: &Table) -> (usize, usize) {
    let p = t.prompt().expect("pass-reward prompt open");
    let card = p
        .options
        .iter()
        .position(|o| o.key().contains("pass_reward.card"))
        .expect("a card option");
    let money = p
        .options
        .iter()
        .position(|o| o.key().contains("pass_reward.money"))
        .expect("a money option");
    (card, money)
}

#[test]
fn rich_pass_reward_parity() {
    let t = pass_reward_prompt(100_000);
    let (card, _money) = card_and_money_options(&t);
    let member = t.m.state().players[0].member;
    let got = act_for(&t.m, member).expect("an answer");
    assert_eq!(got.act, "answer");
    assert_eq!(got.value as usize, card, "above the cap the card wins");
    assert_same(&t, 0, "rich pass-reward");
}

#[test]
fn low_cash_pass_reward_parity() {
    let t = pass_reward_prompt(4_000);
    let (_card, money) = card_and_money_options(&t);
    let member = t.m.state().players[0].member;
    let got = act_for(&t.m, member).expect("an answer");
    assert_eq!(got.act, "answer");
    assert_eq!(got.value as usize, money, "below the ramp the money wins");
    assert_same(&t, 0, "low-cash pass-reward");
}

#[test]
fn turn_start_roll_parity() {
    let mut t = Table::vanilla(2);
    hold(&mut t, 0);
    t.begin_turn(0);
    t.set_hand(0, &[]);
    let member = t.m.state().players[0].member;
    assert_eq!(act_for(&t.m, member).map(|m| m.act), Some("roll".into()));
    assert_same(&t, 0, "turn start");
}

#[test]
fn skip_move_end_parity() {
    let mut t = Table::vanilla(2);
    hold(&mut t, 0);
    t.begin_turn(0);
    t.set_hand(0, &[]);
    t.m.world_mut().st.skip_move = true;
    let member = t.m.state().players[0].member;
    assert_eq!(act_for(&t.m, member).map(|m| m.act), Some("end".into()));
    assert_same(&t, 0, "skip_move");
}

#[test]
fn over_hand_discard_parity() {
    let mut t = Table::vanilla(2);
    hold(&mut t, 0);
    t.begin_turn(0);
    t.set_hand(0, &vec!["通用:GREAT!"; 12]);
    {
        let w = t.m.world_mut();
        w.st.turn = 0;
        w.st.step = stage::END;
        w.st.skip_move = false;
    }
    let member = t.m.state().players[0].member;
    let got = act_for(&t.m, member).expect("a discard");
    assert_eq!(got.act, "discard");
    assert_same(&t, 0, "over-hand");
}
