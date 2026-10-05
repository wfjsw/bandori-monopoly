//! `RAS:R. I. O. T.` -- C# `CardRiot` (MatchHost.cs:9286-9320): [反击] everyone
//! discards their hand and redraws the same count.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:R. I. O. T.`）:
//! > R. I. O. T.：
//! > [反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::trigger;
use card_sdk::CardDef;

pub const RIOT: CardDef = CardDef {
    id: "RAS:R. I. O. T.",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// 规则书[反击]: 「当你被其他人的卡效果影响时打出此卡」 -- C# `H.HitByOtherCard`.
fn can_react(seat: i32) -> bool {
    // TODO(ABI): C# `H.HitByOtherCard` also requires `t.ByCard >= 0 && t.ByCard
    // != seat` (the trigger is raised by a card that is not this seat's own);
    // the Trigger payload carries no `ByCard`.
    match trigger::kind() {
        // 规则书[反击]: 「被其他人的卡效果影响」 -- C# kinds "target"/"abnormal"
        // with `t.Target == seat`.
        TriggerKind::Target | TriggerKind::Abnormal => {
            trigger::target() == seat && trigger::seat() != seat
        }
        // 规则书[反击]: 「被其他人的卡效果影响」 -- C# kind "pay" with
        // `t.Pay.from == seat` (this seat is the one paying).
        TriggerKind::Pay => trigger::seat() == seat && trigger::target() != seat,
        _ => false,
    }
}

fn react(seat: i32) {
    // 规则书[反击]: 「所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡」
    // -- C# `H.DiscardFromHand` over every hand, then `H.DrawR(p, count + (p ==
    // seat ? 1 : 0))` for each seat still in the game.
    // TODO(ABI): needs hand inspection (`H._hidden[p].hand` enumeration) so the
    // "same count" redraw can match; `ctx::discard_from_hand` can discard a named
    // card but not the whole hand, and there is no hand-size query.
    let _ = seat;
}