//! `Sumimi:Sumimi是二人一体的` -- C# `CardTwoInOne` (MatchHost.cs:11249-11312):
//! swap to the other Sumimi character (play or [反击] after own move roll).
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sumimi是二人一体的`）:
//! > Sumimi是二人一体的：
//! > （1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数
//! > （2）此卡可在你的移动掷骰后作为[反击]使用。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const TWO_IN_ONE: CardDef = CardDef::new("Sumimi:Sumimi是二人一体的", &[
    On::Play(play),
    On::React(&[TriggerKind::MoveRoll], can_react, react),
]);

fn play(player_id: i32) {
    // 规则书（1）: 「打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」
    swap(player_id);
}

fn can_react(player_id: i32) -> bool {
    // 规则书（2）: 「此卡可在你的移动掷骰后作为[反击]使用」 -- C#
    // `t.Kind == "moveRoll" && t.Seat == seat` (and the player's character is a
    // Sumimi one).
    trigger::kind() == TriggerKind::MoveRoll && trigger::player_id() == player_id
    // TODO(规则书)（1）: the C# `CanReact` also requires `Other(H.CharacterOf(player_id)) != null`
    // (the player's character is 三角初华（Sumimi） or 纯田真奈); the ABI has no
    // character query (`H.CharacterOf`).
}

fn react(player_id: i32) {
    // 规则书（2）: the [反击] is the same swap as the play effect (C# `React` -> `Swap`).
    swap(player_id);
}

/// C# `CardTwoInOne.Swap` -- flip 三角初华（Sumimi） <-> 纯田真奈, reset fire.
fn swap(player_id: i32) {
    // 规则书（1）: 「将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」
    // TODO(ABI): needs `H.CharacterOf` / `H.ReplaceSkill` (C# `CardTwoInOne.Other`
    // + `H.ReplaceSkill(i, other, CardName)`, which drops the old skill and lets
    // the new one's `Attach` set that character's initial fire / fire max --
    // 三角初华（Sumimi） 2/2, 纯田真奈 1/2). The ctx vocabulary has no character
    // query and no skill-replacement op.
    ctx::log(player_id, &Msg::new(key!("two_in_one_swap")).player_id("who", player_id));
}