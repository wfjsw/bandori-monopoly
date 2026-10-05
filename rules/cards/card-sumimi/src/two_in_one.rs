//! `Sumimi:Sumimi是二人一体的` -- C# `CardTwoInOne` (MatchHost.cs:11249-11312):
//! swap to the other Sumimi character (play or [反击] after own move roll).
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sumimi是二人一体的`）:
//! > Sumimi是二人一体的：
//! > （1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数
//! > （2）此卡可在你的移动掷骰后作为[反击]使用。
//!

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const TWO_IN_ONE: CardDef = CardDef::new("Sumimi:Sumimi是二人一体的", &[
    On::Play(None, play),
    On::CounterAct(&[ChainKind::MoveRoll], can_react, react)]);

fn play(player_id: i32) {
    // 规则书（1）: 「打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」
    swap(player_id);
}

fn can_react(player_id: i32) -> bool {
    // 规则书（2）: 「此卡可在你的移动掷骰后作为[反击]使用」 -- C#
    // `t.Kind == "moveRoll" && t.Seat == seat` (and the player's character is a
    // Sumimi one).
    trigger::kind() == TriggerKind::MoveRoll
        && trigger::player_id() == player_id
        // 规则书（1）: the player's character is one of the two Sumimi ones.
        && (ctx::character_is(player_id, "三角初华（Sumimi）") || ctx::character_is(player_id, "纯田真奈"))
}

fn react(player_id: i32) {
    // 规则书（2）: the [反击] is the same swap as the play effect (C# `React` -> `Swap`).
    swap(player_id);
}

/// The two the clause names, and their 「初始火罐数」.
const A: (&str, &str, i32, i32) = ("三角初华（Sumimi）", "skill:三角初华（Sumimi）:成为偶像", 2, 2);
const B: (&str, &str, i32, i32) = ("纯田真奈", "skill:纯田真奈:甜甜圈爱好者", 1, 2);

/// C# `CardTwoInOne.Swap` -- flip 三角初华（Sumimi） <-> 纯田真奈, reset fire.
fn swap(player_id: i32) {
    // 规则书（1）: 「将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」 --
    // `H.ReplaceSkill` is the old skill rule coming off the field and the new
    // one going on, and 「初始火罐数」 is the new character's cap being written.
    let (from, to) = if ctx::character_is(player_id, A.0) { (A, B) } else { (B, A) };
    ctx::unplace_card_named(player_id, from.1);
    ctx::place_card(player_id, to.1, &Msg::new(key!("two_in_one_note")));
    card_sdk::ctx::state::set_bounds(player_id, card_sdk::abi::state_key::FIRE, 0, to.3);
    card_sdk::ctx::state::set(player_id, card_sdk::abi::state_key::FIRE, to.2);
    ctx::log(player_id, &Msg::new(key!("two_in_one_swap")).player_id("who", player_id).card("card", to.1));
}