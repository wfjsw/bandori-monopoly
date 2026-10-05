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

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const EVEN_LOST: CardDef = CardDef {
    id: "MyGO:即使迷茫着",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// 规则书[反击]（1）: 「当你被其他人的卡的效果影响时」 -- C# `H.HitByOtherCard`.
fn can_react(seat: i32) -> bool {
    // TODO(ABI): C# `H.HitByOtherCard` also requires `t.ByCard >= 0 && t.ByCard
    // != seat` (the trigger is raised by a card that is not this seat's own);
    // the Trigger payload carries no `ByCard`. The `trigger::seat() != seat`
    // arm below is a partial stand-in.
    match trigger::kind() {
        // 规则书[反击]（1）: 「被其他人的卡的效果影响」 -- C# kinds "target"/
        // "abnormal" with `t.Target == seat`.
        TriggerKind::Target | TriggerKind::Abnormal => {
            trigger::target() == seat && trigger::seat() != seat
        }
        // 规则书[反击]（1）: 「被其他人的卡的效果影响」 -- C# kind "pay" with
        // `t.Pay.from == seat` (this seat is the one paying).
        TriggerKind::Pay => trigger::seat() == seat && trigger::target() != seat,
        _ => false,
    }
}

fn react(seat: i32) {
    // 规则书[反击]（1）: 「你将此卡放置在自己场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "MyGO:即使迷茫着", &Msg::new(key!("even_lost_note")));
    ctx::log(seat, &Msg::new(key!("even_lost_placed")).seat("who", seat));
    // TODO(规则书)（2）: 「[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次
    // 主要移动格数为你当前手牌张数」 -- needs the placed-card `Actions` hook
    // (C# `CardEvenLost.Actions` / `Go`: `H.Unplace` then `H.MainMoveAs(Seat,
    // new MoveCtx { Steps = hand.Count })`) and a total hand-size query
    // (`H._hidden[Seat].hand.Count`); `hand_count(seat, id)` counts one id
    // only, and the vocabulary has no action list.
}