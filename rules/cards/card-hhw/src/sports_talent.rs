//! `HHW:运动的天赋` -- C# `CardSportsTalent` (MatchHost.cs:4025-4091).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:运动的天赋`）:
//! > 运动的天赋：
//! >  将此卡放置于自己场上并放置3个奇迹水晶，每回合结束时失去一个，为0时置入弃牌堆。此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100。每次移动掷骰时，重骰移动掷骰直至结果为10以上为止。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const SPORTS_TALENT: CardDef = CardDef {
    id: "HHW:运动的天赋",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // 规则书: 「将此卡放置于自己场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "HHW:运动的天赋", &Msg::new(key!("sports_talent_note")));
    ctx::log(seat, &Msg::new(key!("sports_talent_placed")).seat("who", seat));
    // TODO(规则书): 「并放置3个奇迹水晶，每回合结束时失去一个，为0时置入弃牌堆」
    // -- the placement's crystal charge and its turn-end decay need card_crystals
    // on a placed card (C# `DecayCard`, `H.PlaceFromPlay(c, -1, -1, 3)`) plus the
    // Fx.TurnEnd hook.
    // TODO(规则书): 「此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100」
    // -- needs the Fx.RollAfter hook (C# `CardSportsTalent.RollAfter` -> `Reward`)
    // firing on every dice roll.
    // TODO(规则书): 「每次移动掷骰时，重骰移动掷骰直至结果为10以上为止」
    // -- needs the same Fx.RollAfter hook's reroll loop (C#
    // `CardSportsTalent.Rolls`, `H.DoMoveRoll`).
}