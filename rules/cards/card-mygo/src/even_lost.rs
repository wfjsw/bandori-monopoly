//! `MyGO:即使迷茫着` -- C# `CardEvenLost` (MatchHost.cs:6259-6307): [反击] place
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:即使迷茫着`）:
//! > 即使迷茫着： 
//! >  
//! > （1）[反击] 当你被其他人的卡的效果影响时，你将此卡放置在自己场上。
//! > （2）[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。
//!
//! this card in play when another player's card hits you; a placed action later
//! discards it to move as many tiles as your hand size.

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const EVEN_LOST: CardDef = CardDef::new(
    "MyGO:即使迷茫着",
    &[
        On::Play(Some(can_go), go),
        On::Counteract(&[ChainKind::Effect], can_counteract, counteract),
    ],
);

/// 规则书[反击]（1）: 「当你被其他人的卡的效果影响时」 -- C# `H.HitByOtherCard`.
fn can_counteract(player_id: i32) -> bool {
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

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]（1）: 「你将此卡放置在自己场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "MyGO:即使迷茫着",
        &Msg::new(key!("even_lost_note")),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("even_lost_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// （2）「[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要
/// 移动格数为你当前手牌张数」 -- a press: unplace, then walk `hand.Count` steps
/// as the turn's main move.
fn can_go(player_id: i32) -> Option<Msg> {
    if !ctx::is_placed() {
        return Some(Msg::new(key!("even_lost_not_placed")));
    }
    None
}

fn go(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    // The move below settles, so the card must be gone before it runs.
    ctx::send_to_dest(ctx::Dest::Graveyard);
    ctx::plan::set_steps(ctx::hand_size(player_id));
    ctx::plan::set_resolve(true);
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("even_lost_go")).i("n", ctx::hand_size(player_id) as i64),
    );
    Ok(())
}
