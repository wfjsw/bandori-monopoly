//! `MyGO:即使迷茫着` -- C# `CardEvenLost` (MatchHost.cs:6259-6307): [反击] place
//! this card in play when another player's card hits you; a placed action later
//! discards it to move as many tiles as your hand size.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:即使迷茫着`）:
//! > 即使迷茫着：
//! >
//! > （1）[反击] 当你被其他人的卡的效果影响时，你将此卡放置在自己场上。
//! > （2）[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。
//!

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const EVEN_LOST: CardDef = CardDef::new("MyGO:即使迷茫着", &[
    On::React(&[ChainKind::Effect], can_react, react),
]);

/// 规则书[反击]（1）: 「当你被其他人的卡的效果影响时」 -- C# `H.HitByOtherCard`.
fn can_react(player_id: i32) -> bool {
    // C# `H.HitByOtherCard(t, seat)` = `t.ByCard >= 0 && t.ByCard != seat` and
    // (kind "target"/"abnormal" -> `t.Target == seat`, kind "pay" -> `t.Pay.from == seat`).
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

fn react(player_id: i32) {
    // 规则书[反击]（1）: 「你将此卡放置在自己场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "MyGO:即使迷茫着", &Msg::new(key!("even_lost_note")));
    ctx::log(player_id, &Msg::new(key!("even_lost_placed")).player_id("who", player_id));
    // TODO(规则书)（2）: 「[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次
    // 主要移动格数为你当前手牌张数」 -- needs the placed-card `Actions` hook
    // (C# `CardEvenLost.Actions` / `Go`) and the immediate main-move routine
    // (`H.Unplace` then `H.MainMoveAs(Seat, new MoveCtx { Steps = hand.Count })`).
    // The step count itself is readable now (`ctx::hand_size(player_id)` =
    // `H._hidden[Player].hand.Count`); the walk that consumes the main move is
    // still landing (held with the H.CardMove / main-move family).
}