//! `Mor:夏日合宿` -- C# `CardSummerCamp` (MatchHost.cs:4584-4643): stay untargetable
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:夏日合宿`）:
//! > 夏日合宿：
//! >  打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌
//!
//! by others until your next turn, then draw if nothing was blocked.

use card_sdk::{ctx, key, CardDef, Msg};

pub const SUMMER_CAMP: CardDef = CardDef {
    id: "Mor:夏日合宿",
    play: Some(summer_camp),
    can_react: None,
    react: None,
    why_not: None,
};

fn summer_camp(seat: i32) {
    // C# `AiPlay => false` -- bots never play this card; `CardDef` has no AiPlay
    // hook yet, so a bot prompt will still offer it.
    // 规则书: 「打出此卡，直到下个自己的回合开始前」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Mor:夏日合宿", &Msg::new(key!("summer_camp_note")));
    ctx::log(seat, &Msg::new(key!("summer_camp_placed")).seat("who", seat));
    // TODO(规则书): 「你只会被自己发动的效果指定」 -- needs the Fx.Untargetable hook
    // (C# `CardSummerCamp.Untargetable` refuses any `by != Seat` and counts
    // `Mem["blocked"]`).
    // TODO(规则书): 「当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌」 -- needs
    // the Fx.TurnStart hook to end the effect on the owner's next turn
    // (C# `CardSummerCamp.TurnStart` -> `End`: `H.Unplace` and
    // `H.DrawR(Seat, 1, ...)` when `Blocked == 0`).
}