//! `Mujica:（睦/mortis）表演的本能` -- C# `CardMortisInstinct` (MatchHost.cs:5342-5435):
//! copy the last non-placed card another player played.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（睦/mortis）表演的本能`）:
//! > （睦/mortis）表演的本能：
//! >  此卡打出时效果为场上任意其他玩家打出的上一张卡（不计入效果带有放置于场上的卡）（此卡复制[反击]卡时可在符合条件时打出）
//!
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const MORTIS_INSTINCT: CardDef = CardDef {
    id: "Mujica:（睦/mortis）表演的本能",
    play: Some(play),
    can_react: Some(can_react),
    react: Some(react),
    // C# `CardMortisInstinct.WhyNot` needs the copied card (`Copy(seat)` over
    // `H._playHistory`) and that card's own `WhyNot` -- 「其他玩家还没有打出过
    // 可以复制的卡」 / 「要复制的「…」只能在写明的时机打出」. TODO(规则书): the
    // `why_not` gate once the play-history + `H.NewCard` copy machinery below
    // is in the ABI.
    why_not: None,
};

fn play(seat: i32) {
    run(seat);
}

fn can_react(_seat: i32) -> bool {
    // TODO(规则书): 「（此卡复制[反击]卡时可在符合条件时打出）」 -- C#
    // `CardMortisInstinct.CanReact` delegates to `Copy(seat)?.CanReact(seat, t)`.
    // Needs the play-history copy machinery (C# `H._playHistory` / `H.NewCard`)
    // so the copied card's reaction window can be checked.
    false
}

fn react(seat: i32) {
    run(seat);
}

fn run(seat: i32) {
    // 规则书: 「此卡打出时效果为场上任意其他玩家打出的上一张卡（不计入效果带有放置于场上的卡）」
    // TODO(规则书): the copy itself -- C# `CardMortisInstinct.Copy` walks
    // `H._playHistory` backwards for the latest `(seat != me, !placed, id != me)`
    // entry, then `H.NewCard(id)` + nested `Play`/`React` (a full `PlayCtx` with
    // `Extrene`/`Doubled`/`Tags` forwarding). The ABI has no play-history query
    // and no cross-card `PlayCtx` nesting (`ctx::play_card` runs a card's `play`
    // without the copied card's identity / reaction mode).
    ctx::log(seat, &Msg::new(key!("mortis_instinct_nothing")).seat("who", seat));
}