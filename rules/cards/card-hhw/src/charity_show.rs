//! `HHW:爱心义演` -- C# `CardCharityShow` (MatchHost.cs:4164-4177): this turn,
//! +2 steps the first time you pass each of your tiles, and half pay to others.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:爱心义演`）:
//! > 爱心义演：
//! > 打出此卡的回合内，你若进行掷骰移动，每初次经过一个属于你的格子，使你的总移动数+2，本回合中向其他玩家支付时你的付款减半（向上取整10）（若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子）
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const CHARITY_SHOW: CardDef = CardDef {
    id: "HHW:爱心义演",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // C# `CardCharityShow.Play` only arms the two turn-long effects and logs.
    ctx::log(seat, &Msg::new(key!("charity_show_played")).seat("who", seat));
    // 规则书: 「打出此卡的回合内，你若进行掷骰移动，每初次经过一个属于你的格子，
    // 使你的总移动数+2」 -- C# `CharityFx.PassTile` (seen-tile set, `m.ExtraSteps += 2`).
    // TODO(规则书): needs the Fx.PassTile hook plus the move's extra-steps field
    // (C# `CharityFx.PassTile`, `m.ExtraSteps`), scoped to this turn's dice move
    // (`!m.Teleport && !m.TeleportWalk && m.Steps >= 0`).
    // 规则书: 「本回合中向其他玩家支付时你的付款减半（向上取整10）」 -- C#
    // `H._turnCtx.HalfPayToOthers = true`, applied in `SettleFactor` as
    // `p.amount = CeilTo(p.amount / 2.0, 10)` for any pay-to-other of yours.
    // TODO(规则书): needs the turn-context half-pay flag (C# `TurnCtx.
    // HalfPayToOthers` + `SettleFactor` / `CeilTo`); the vocabulary has no
    // pay-multiplier hook.
    // 规则书: 「（若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子）」
    // -- C# `CharityFx.Mine` treats CiRCLE as yours when a `CardKokoroCircle`
    // of yours sits on it. TODO(规则书): that ownership probe needs placed-card
    // lookup by id and tile (C# `H._placed.Any(p => p is CardKokoroCircle &&
    // p.Seat == Seat && p.Tile == t)`); `is_placed` only asks about this card.
    // C# `CharityFx.TurnEndAfter` drops the extra at the end of the owner's turn.
    // TODO(规则书): 「打出此卡的回合内」 -- needs the Fx.TurnEndAfter hook to end
    // both effects with the turn (C# `H.RemoveExtra(this)`).
}