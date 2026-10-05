//! `RAS:（MASKING）CRUSH ON THE DRUM!!!` -- C# `CardCrushDrum` (MatchHost.cs:10066-10088):
//! [手] +Xd20 from the discard pile and [特] immunity for dice-adding cards.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（MASKING）CRUSH ON THE DRUM!!!`）:
//! > （MASKING）CRUSH ON THE DRUM!!!：
//! > [特]：
//! > 打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡不受任何其他效果影响。
//! > [手]：
//! > 移动阶段前打出此卡，本回合主要移动掷骰额外添加Xd20，X为你弃牌堆的卡数
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const CRUSH_DRUM: CardDef = CardDef {
    id: "RAS:（MASKING）CRUSH ON THE DRUM!!!",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // 规则书[手]: 「移动阶段前打出此卡」 -- C# `WhyNot => H.MoveWhyNot(seat)`
    // refuses the play once the main move is spent.
    // TODO(ABI): `H.MoveWhyNot` is still missing (`_turnCtx.MainMoved`,
    // `State.skipMove`), so the play window is not gated here.
    // 规则书[手]: 「本回合主要移动掷骰额外添加Xd20，X为你弃牌堆的卡数」
    // -- C# `H._turnCtx.Plan.Dice.Add((discard.Count, 20, ...))`.
    let count = ctx::discard_size(seat);
    if count > 0 {
        // TODO(规则书): the move dice plan (`MoveCtx.Plan.Dice`) to add Xd20 is
        // still missing; the count is read but cannot yet be injected.
    }
    ctx::log(
        seat,
        &Msg::new(key!("crush_drum_roll")).i("n", count as i64),
    );
    // 规则书[特]: 「打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡不受任何其他效果影响。」
    // -- C# sets `H._turnCtx.DiceCardsImmune = true`, and the play flow then
    // skips `React` for any `def.AddsDice` card of this seat (MatchHost.cs
    // ~18912).
    // TODO(ABI): needs a turn-level dice-immunity flag (C#
    // `H._turnCtx.DiceCardsImmune`) and a way for the engine to know a card is
    // dice-adding. The Trigger payload of a card play is only
    // `{ Kind: Card, Seat, Card, Play }` -- it does not expose `Play.Def.AddsDice`,
    // `t.ByCard`, or `t.Play.Def`, so a card-side guard cannot read "this is a
    // dice-adding card" either.
}
