//! `AG:朝同一片天空迈进` -- C# `CardSameSky` (MatchHost.cs:1547-1582):
//! [反击] auto-plays when drawn: money by hand size, else draw.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:朝同一片天空迈进`）:
//! > 朝同一片天空迈进：
//! > [反击]抽出此卡时立刻打出，如果你手牌数大于等于3，获得手牌数*600的资金，如果你的手牌数小于3，抽一张卡（开局时抽到此卡洗回）
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const SAME_SKY: CardDef = CardDef {
    id: "AG:朝同一片天空迈进",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「抽出此卡时立刻打出」 -- C# `CardSameSky.Drawn` auto-plays it
    // as a reaction; there is no window prompt.
    // TODO(ABI): the Fx.Drawn hook (auto-play on draw) is not in the vocabulary.
    // `TriggerKind::DrawOut` is the closest raise the engine might use; until the
    // hook exists this reaction stays dormant.
    trigger::kind() == TriggerKind::DrawOut && trigger::target() == seat
}

fn react(seat: i32) {
    // C# `Drawn` pulls the card out of hand first (`hand.Remove(Id)`).
    // TODO(ABI): hand removal for a play (not `discard_from_hand`, which discards)
    // is not in the vocabulary.
    // TODO(规则书[反击]): 「如果你手牌数大于等于3，获得手牌数*600的资金」 -- needs
    // the total hand size (C# `H._hidden[seat].hand.Count`); `ctx::hand_count` only
    // counts copies of one card id.
    // TODO(规则书[反击]): 「如果你的手牌数小于3，抽一张卡」 -- same total-hand-size
    // query picks the branch.
    ctx::log(seat, &Msg::new(key!("same_sky_pending")).seat("who", seat));
    // TODO(规则书[反击]): 「（开局时抽到此卡洗回）」 -- needs the
    // `Card.ReturnAtOpening` flag (C# `CardSameSky.ReturnAtOpening`) so an
    // opening-draw shuffles it back instead of resolving.
}