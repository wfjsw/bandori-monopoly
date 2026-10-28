//! The standard bot's [pass-reward] choice (user ruling 2026-10-28): the
//! 2 000 money print at low cash, the card once the stack passes the ramp,
//! and always the card above the cap. The behaviour is engine-side
//! (`Cx::ai_answer_for_entry`), so it holds for server bots and timed-out
//! seats alike.

use game_core::state::BotMentality;
use rules_spec::global::movement::BOARD_TILES;
use rules_spec::Table;

/// Roll past CiRCLE from `money`, let the bot answer its own reward prompt,
/// and return the reward keys the move logged.
fn reward_keys(money: i32) -> Vec<String> {
    let mut t = Table::vanilla(2);
    t.make_bot(0, BotMentality::Standard);
    t.set_money(0, money);
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!"]);
    t.set_pos(0, BOARD_TILES - 2);
    t.set_pos(1, 10);
    t.begin_turn(0);
    let mark = t.mark();
    t.dice(&[3]); // wraps past CiRCLE (0) and lands on 1
    t.roll(0).unwrap();
    let mut seen = false;
    t.tick_until("the bot answers its [pass-reward]", |t| {
        if let Some(p) = t.prompt() {
            seen |= p.title.key().contains("pass_reward");
        }
        seen && t
            .prompt()
            .is_none_or(|p| !p.title.key().contains("pass_reward"))
    });
    t.keys_since(mark)
        .into_iter()
        .filter(|k| k.contains("pass_reward"))
        .collect()
}

#[test]
fn a_low_cash_bot_takes_the_money() {
    let keys = reward_keys(4_000);
    assert!(
        keys.iter().any(|k| k.contains("pass_reward_money")),
        "expected the money half: {keys:?}"
    );
    assert!(
        !keys.iter().any(|k| k.contains("pass_reward_card")),
        "no card below the ramp: {keys:?}"
    );
}

#[test]
fn a_rich_bot_takes_the_card() {
    let keys = reward_keys(100_000);
    assert!(
        keys.iter().any(|k| k.contains("pass_reward_card")),
        "expected the card half: {keys:?}"
    );
    assert!(
        !keys.iter().any(|k| k.contains("pass_reward_money")),
        "no money above the cap: {keys:?}"
    );
}

/// `Match::bot_action` -- the shared command 托管 sends -- must answer the
/// same half the scheduler's own answer path does. The seat is held (`ai`
/// cleared), so only `bot_action` decides here.
fn held_action(money: i32) -> (String, i32, Vec<String>) {
    let mut t = Table::vanilla(2);
    {
        let p = &mut t.m.world_mut().st.players[0];
        p.bot = true;
        p.mentality = BotMentality::Standard;
        p.ai = false;
    }
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
    let p = t.prompt().expect("prompt open");
    let keys: Vec<String> = p.options.iter().map(|o| o.key().to_string()).collect();
    let cmd =
        t.m.bot_action(1)
            .expect("the held seat's own prompt is a bot_action surface");
    (cmd.act, cmd.value, keys)
}

#[test]
fn bot_action_answers_the_pass_reward_the_same_way() {
    let (act, value, keys) = held_action(100_000);
    assert_eq!(act, "answer");
    let card = keys
        .iter()
        .position(|k| k.contains("pass_reward.card"))
        .expect("a card option");
    assert_eq!(
        value as usize, card,
        "above the cap the card wins: {keys:?}"
    );

    let (_act, value, keys) = held_action(4_000);
    let money = keys
        .iter()
        .position(|k| k.contains("pass_reward.money"))
        .expect("a money option");
    assert_eq!(
        value as usize, money,
        "below the ramp the money wins: {keys:?}"
    );
}

#[test]
fn the_chance_ramp_is_superlinear_and_bounded() {
    use game_core::engine::{
        pass_reward_card_chance, PASS_REWARD_CARD_MIN_MONEY, PASS_REWARD_CARD_MONEY,
    };
    assert_eq!(pass_reward_card_chance(PASS_REWARD_CARD_MIN_MONEY), 0.0);
    assert_eq!(pass_reward_card_chance(PASS_REWARD_CARD_MONEY), 1.0);
    let mid = (PASS_REWARD_CARD_MIN_MONEY + PASS_REWARD_CARD_MONEY) / 2;
    assert!(
        (pass_reward_card_chance(mid) - 0.25).abs() < 1e-9,
        "t^2 at the midpoint"
    );
    let mut prev = 0.0;
    let mut m = PASS_REWARD_CARD_MIN_MONEY;
    while m <= PASS_REWARD_CARD_MONEY {
        let c = pass_reward_card_chance(m);
        assert!(c >= prev, "monotonic at {m}");
        prev = c;
        m += 500;
    }
}
