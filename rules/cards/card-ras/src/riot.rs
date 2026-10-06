//! `RAS:R. I. O. T.` -- C# `CardRiot` (MatchHost.cs:9286-9320): [反击] everyone
//! discards their hand and redraws the same count.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:R. I. O. T.`）:
//! > R. I. O. T.：
//! > [反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。
//!

use alloc::vec::Vec;

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger, CardPile};
use card_sdk::{CardDef, On};

pub const RIOT: CardDef = CardDef::new("RAS:R. I. O. T.", &[
    On::CounterAct(&[ChainKind::Effect], can_react, react),
]);

/// 规则书[反击]: 「当你被其他人的卡效果影响时打出此卡」 -- C# `H.HitByOtherCard`.
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

fn react(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡」
    // -- C# `H.DiscardFromHand` over every hand, then `H.DrawR(p, count + (p ==
    // player ? 1 : 0))` for each player_id still in the game.
    // Every hand goes to the discard first, then everyone redraws -- so nobody
    // redraws a card another player just discarded mid-pass.
    let mut redraw: Vec<(i32, i32)> = Vec::new();
    for p in 0..ctx::player_count() {
        if ctx::player_out(p) {
            continue;
        }
        let hand = ctx::cards_in(p, CardPile::Hand);
        for id in &hand {
            ctx::discard_from_hand(p, id);
        }
        redraw.push((p, hand.len() as i32 + (p == player_id) as i32));
    }
    for (p, n) in redraw {
        if n > 0 {
            ctx::draw(p, n);
        }
    }
    Ok(())
}