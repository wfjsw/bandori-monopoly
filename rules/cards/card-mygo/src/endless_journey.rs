//! `MyGO:哪怕这旅程没有终点` -- C# `CardEndlessJourney` (MatchHost.cs:6971-7009):
//! plant this card on the current tile with 4 miracle crystals; settles pay
//! out 60 per tile walked, and a long main move burns a crystal.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:哪怕这旅程没有终点`）:
//! > 哪怕这旅程没有终点：
//! >  [手] 将此卡放置于当前格子上并为其放置4个奇迹水晶，每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶。当此卡奇迹水晶数量为0时，将此卡置入弃牌堆。
//! > （2）[持续] 触发结算时，获得X*60资金，X为你此次主要移动[经过]的格数
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const ENDLESS_JOURNEY: CardDef = CardDef {
    id: "MyGO:哪怕这旅程没有终点",
    play: Some(endless_journey),
    can_react: None,
    react: None,
    why_not: None,
};

fn endless_journey(seat: i32) {
    // 规则书[手]: 「将此卡放置于当前格子上」 -- C# `H.PlaceFromPlay(c, c.Seat, pos, 4)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "MyGO:哪怕这旅程没有终点", &Msg::new(key!("endless_journey_note")));
    ctx::log(seat, &Msg::new(key!("endless_journey_placed")).seat("who", seat));
    // TODO(规则书)[手]: 「将此卡放置于当前格子上」 -- the placement is bound to the
    // seat's current tile (`Card.Tile`); needs field-card tile placement.
    // TODO(规则书)[手]: 「并为其放置4个奇迹水晶」 -- needs a per-card crystal counter
    // (`H.PlaceFromPlay(..., crystals: 4)`); `place_card` has no crystal argument.
    // TODO(规则书)[手]: 「每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶。当此卡奇迹
    // 水晶数量为0时，将此卡置入弃牌堆。」 -- needs the Fx.TurnEnd hook plus the
    // turn's main-move total (C# `H._turnCtx.LastMain > 6` -> `AddCrystals(-1)`
    // and `H.Unplace(this, "discard", ...)` at 0).
    // TODO(规则书)（2）: 「[持续] 触发结算时，获得X*60资金，X为你此次主要移动[经过]的格数」
    // -- needs the Fx.SettleAfter hook (C# `CardEndlessJourney.SettleAfter`) and
    // the move's path length (C# `m.Path.Count`); the money itself is
    // `ctx::gain(seat, x * 60, ...)` once those two are readable.
}