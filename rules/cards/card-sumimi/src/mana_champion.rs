//! `Sumimi:（真奈）歌唱大赛5连冠` -- C# `CardManaChampion` (MatchHost.cs:11529-11591):
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:（真奈）歌唱大赛5连冠`）:
//! > （真奈）歌唱大赛5连冠：
//! > [反击]当你或你的格子即将受到来自你以外的效果影响时打出此卡，此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你。若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡。若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。
//!
//! [反击] that lets the other players react as if they were the target.

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const MANA_CHAMPION: CardDef = CardDef::new("Sumimi:（真奈）歌唱大赛5连冠", &[
    On::CounterAct(&[ChainKind::Effect], can_react, react),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「当你或你的格子即将受到来自你以外的效果影响时打出此卡」
    // C# `H.HitByOtherCard(t, seat)` (MatchHost.cs:19235-19255) =
    // `t.ByCard >= 0 && t.ByCard != seat` and (kind "target"/"abnormal" ->
    // `t.Target == seat`, kind "pay" -> `t.Pay.from == seat`).
    // Re-verified against the v26 pipeline: a tile hit lands as
    // `t.Target = owner` (H.TargetTile, MatchHost.cs:19178-19216), so
    // 「或你的格子」 is covered by `target() == player_id`.
    if !trigger::by_card().is_some_and(|by| by != player_id) {
        return false;
    }
    // 规则书[反击]: 「被其他人的卡的效果影响」 is one condition on the *effect*,
    // and now reads as one: any effect another player's card declared at me. It
    // used to be reconstructed by unioning `Target`/`Abnormal` (via `t.Target`)
    // with `Pay` (via `t.Pay.from`) and matching on two different fields.
    // `effect::hits` covers both because a payment touches its payer *and* its
    // payee.
    ctx::effect::hits(player_id)
}

fn react(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。」
    // C# `CardManaChampion.React` (MatchHost.cs:11543-11590) walks `H.Others(i)`
    // offering each a nested `H.React(copy, p)` with `Target = p`; `anyone` is
    // set only when a nested reaction lands, and `if (!anyone)` draws 1.
    ctx::log(player_id, &Msg::new(key!("mana_champion_log")).player_id("who", player_id));
    // 规则书[反击]: 「若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。」 --
    // C# `H.DrawR(i, 1, ...)` on `!anyone` (MatchHost.cs:11586-11589).
    ctx::draw(player_id, 1);
    // TODO(规则书)[judgement](ABI): nested reaction half (C# `H.React(copy, p)`,
    //   the clause under-specifies -- see the note above it
    // MatchHost.cs:11573) -- 「此时场上其他玩家可如同自身的对应目标被指定一般
    // 打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你。若以此种方式使你
    // 免于受到该影响，打出那张[反击]卡的玩家可抽一张卡。」 The copy trigger
    // (`Target = p`, `t.Play` / `t.Ab` / `t.ByCard` / `t.Cancelled` /
    // `t.Reacted`) and the nested reaction run are not in the vocabulary, so
    // the shield and the reactor's draw cannot be expressed; the fallback draw
    // above is what fires while that half is held.
    Ok(())
}