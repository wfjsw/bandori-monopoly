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

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const CRUSH_DRUM: CardDef = CardDef::new("RAS:（MASKING）CRUSH ON THE DRUM!!!", &[
    On::Play(Some(cant_play), play)]);

/// C# `CardCrushDrum.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[手]: 「移动阶段前打出此卡」 -- refuses the play once the main move is spent.
    ctx::cant_move(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「本回合主要移动掷骰额外添加Xd20，X为你弃牌堆的卡数」
    // -- C# `H._turnCtx.Plan.Dice.Add((discard.Count, 20, ...))`.
    let count = ctx::discard_size(player_id);
    if count > 0 {
        // 规则书[手]: 「本回合主要移动掷骰额外添加Xd20」 -- C#
        // `H._turnCtx.Plan.Dice.Add((count, 20, "（CRUSH ON THE DRUM!!!）"))`.
        ctx::plan::add_extra_dice(count, 20, "（CRUSH ON THE DRUM!!!）");
    }
    ctx::log(
        player_id,
        &Msg::new(key!("crush_drum_roll")).i("n", count as i64),
    );
    // 规则书[特]: 「打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡不受任何其他效果影响。」
    // -- C# sets `H._turnCtx.DiceCardsImmune = true`, and the play flow then
    // skips `React` for any `def.AddsDice` card of this player (MatchHost.cs
    // ~18912).
    // TODO(规则书)[judgement](ABI): needs a turn-level dice-immunity flag (C#
    //   the clause under-specifies -- see the note above it
    // `H._turnCtx.DiceCardsImmune`) and a way for the engine to know a card is
    // dice-adding. The Trigger payload of a card play is only
    // `{ Kind: Card, Player, Card, Play }` -- it does not expose `Play.Def.AddsDice`,
    // `t.ByCard`, or `t.Play.Def`, so a card-side guard cannot read "this is a
    // dice-adding card" either.
    Ok(())
}
