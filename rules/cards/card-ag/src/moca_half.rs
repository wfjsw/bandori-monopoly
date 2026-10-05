//! `AG:（摩卡）0.5倍速` -- C# `CardMocaHalf` (MatchHost.cs:1161-1197):
//! placed card, 3 crystals decaying each own turn end; halve rolls and payments.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（摩卡）0.5倍速`）:
//! > （摩卡）0.5倍速：
//! > (1)  将此卡放置在场上并获得3个奇迹水晶，你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆。
//! > (2) 此卡在场时，你的移动掷骰的最终结算/2（向上取整）且你的所有资金支付与消耗减半。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const MOCA_HALF: CardDef = CardDef {
    id: "AG:（摩卡）0.5倍速",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // 规则书(1): 「将此卡放置在场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "AG:（摩卡）0.5倍速", &Msg::new(key!("moca_half_note")));
    ctx::log(seat, &Msg::new(key!("moca_half_placed")).seat("who", seat));
    // TODO(规则书(1)): 「并获得3个奇迹水晶，你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆」
    // -- the placement's crystal charge and its turn-end decay need card_crystals
    // on a placed card (C# `DecayCard`, `H.PlaceFromPlay(c, -1, -1, 3)`) plus the
    // Fx.TurnEnd hook (C# `DecayCard.TurnEnd` -> `AddCrystals(-1)` / `Unplace`).
    // TODO(规则书(2)): 「此卡在场时，你的移动掷骰的最终结算/2（向上取整）」 -- needs
    // the Fx.RollAfter hook (C# `CardMocaHalf.RollAfter`: `m.Roll = ceil(max(0, roll)/2)`).
    // TODO(规则书(2)): 「且你的所有资金支付与消耗减半」 -- needs the Fx.PayMul hook
    // (C# `CardMocaHalf.PayMul`: `p.amount = CeilTo(amount/2, 10)`).
}