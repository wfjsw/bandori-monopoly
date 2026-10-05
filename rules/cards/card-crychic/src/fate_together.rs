//! `CRYCHIC:一起演奏音乐的命运共同体` -- C# `CardFateTogether` (MatchHost.cs:3050-3061):
//! until your next turn start, hop to the next seat's tile on every rent collection.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:一起演奏音乐的命运共同体`）:
//! > 一起演奏音乐的命运共同体
//! > ：
//! > （1）[手] 直到你的下回合开始，每当场上任意格子发生一次收款时，你移动到你前方的下一个属于行动序列后一名玩家的格子（不触发结算）。
//! > （2）若此卡打出后
//! > （1）效果未产生作用，将此卡返回手牌（不触发乐队技能的
//! > （3）效果）。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const FATE_TOGETHER: CardDef = CardDef {
    id: "CRYCHIC:一起演奏音乐的命运共同体",
    play: Some(fate_together),
    can_react: None,
    react: None,
    why_not: None,
};

fn fate_together(seat: i32) {
    // 规则书（1）[手]: 「直到你的下回合开始，每当场上任意格子发生一次收款时，你移动到你
    // 前方的下一个属于行动序列后一名玩家的格子（不触发结算）。」 -- C# arms `FateFx`
    // (`H.ExtraOf<FateFx>(c.Seat)`, `Hit = false`, `Card = c.Id`).
    ctx::log(seat, &Msg::new(key!("fate_together_note")).seat("who", seat));
    // TODO(ABI): （1） 「每当场上任意格子发生一次收款时，你移动到你前方的下一个属于行动序列
    //   后一名玩家的格子（不触发结算）。」 -- needs the Fx.PayAfter persistent hook
    //   (C# `FateFx.PayAfter`: `p.paid && p.IsRent && p.finalGain > 0`, then the
    //   next tile forward owned by `H.Neighbor(Seat, 1)`, walked with
    //   `H.Walk(..., resolve: false)`). The neighbour seat is
    //   action-order-next; the walk is the missing movement routine.
    // TODO(ABI): （1） 「直到你的下回合开始」 -- needs the Fx.TurnStart hook
    //   (C# `FateFx.TurnStart` removes the effect at this seat's next turn start).
    // TODO(规则书)（2）: 「若此卡打出后（1）效果未产生作用，将此卡返回手牌（不触发乐队技能的
    //   （3）效果）。」 -- needs the same Fx.TurnStart hook plus a hand/discard
    //   lookup (C# `FateFx.TurnStart`: `if (!Hit) discard.Remove(Card) ->
    //   hand.Add(Card)`). `ctx::set_dest(ctx::Dest::Hand)` is ready once the "did
    //   (1) fire" flag and the discard lookup exist.
}