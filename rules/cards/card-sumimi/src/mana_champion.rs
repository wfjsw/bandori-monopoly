//! `Sumimi:（真奈）歌唱大赛5连冠` -- C# `CardManaChampion` (MatchHost.cs:11529-11591):
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:（真奈）歌唱大赛5连冠`）:
//! > （真奈）歌唱大赛5连冠：
//! > [反击]当你或你的格子即将受到来自你以外的效果影响时打出此卡，此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你。若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡。若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。
//!
//! [反击] that lets the other players react as if they were the target.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const MANA_CHAMPION: CardDef = CardDef::new("Sumimi:（真奈）歌唱大赛5连冠", &[
    On::React(&[TriggerKind::Pay, TriggerKind::Abnormal, TriggerKind::Target], can_react, react),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「当你或你的格子即将受到来自你以外的效果影响时打出此卡」
    // C# `H.HitByOtherCard(t, seat)` = `t.ByCard >= 0 && t.ByCard != seat` and
    // (kind "target"/"abnormal" -> `t.Target == seat`, kind "pay" -> `t.Pay.from == seat`).
    if !trigger::by_card().is_some_and(|by| by != player_id) {
        return false;
    }
    match trigger::kind() {
        TriggerKind::Target | TriggerKind::Abnormal => trigger::target() == player_id,
        TriggerKind::Pay => trigger::player_id() == player_id,
        _ => false,
    }
}

fn react(player_id: i32) {
    // 规则书[反击]: 「此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中
    // 针对打出玩家自身的效果改为你。若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡。
    // 若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。」
    // C# `CardManaChampion.React` walks `H.Others(i)`, copies the trigger with
    // `Target = p`, and yields `H.React(copy, p)` so each other player may play
    // their own [反击] as the (rewritten) target; a cancelling reaction shields
    // `i` and draws 1 for the reactor, and if nobody reacts `i` draws 1.
    // TODO(ABI): nested reaction runs (`H.React` / `H.React(copy, p)`) and the
    // trigger-copy payload (`t.Play` / `t.Ab` / `t.ByCard` / `t.Cancelled` /
    // `t.Reacted`) are not in the vocabulary, so neither the shield nor the draw
    // rewards can be expressed. Until then this [反击] only logs.
    ctx::log(player_id, &Msg::new(key!("mana_champion_log")).player_id("who", player_id));
}