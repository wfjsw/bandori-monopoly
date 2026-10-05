//! `Sumimi:一人两个甜甜圈` -- C# `CardTwoDonuts` (MatchHost.cs:11119-11138): exile
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:一人两个甜甜圈`）:
//! > 一人两个甜甜圈：
//! >
//! > （1）获得[除外]直至你原本所在格子被其他玩家经过。
//! >
//! > （2）你原本所在格子被其他玩家经过时，可在那名玩家触发结算后选择传送至你原本所在格子（不包括）与那名玩家本次移动终点间的任一格并触发结算，之后你们各获得2火罐（超出上限的每个火罐转化为500资金）
//!
//! until the original tile is passed by someone else, then a settle-teleport and
//! 2 fire each. The C# `AiPlay` always returns false (no `H.AiPlay` hook).

use card_sdk::{ctx, key, CardDef, Msg};

pub const TWO_DONUTS: CardDef = CardDef {
    id: "Sumimi:一人两个甜甜圈",
    play: Some(two_donuts),
    can_react: None,
    react: None,
    why_not: None,
};

fn two_donuts(seat: i32) {
    let pos = ctx::seat_pos(seat);
    // 规则书（1）: 「获得[除外]直至你原本所在格子被其他玩家经过」
    // C# `H.GiveExile(i, 99, pos, i, CardName)` -- 99 layers so the per-turn
    // layer tick does not expire it; `DonutFx` clears the exile early.
    ctx::give_exile(seat, 99, pos);
    ctx::log(
        seat,
        &Msg::new(key!("two_donuts_exile")).seat("who", seat).tile("tile", pos),
    );
    // TODO(规则书)（1）: 「直至你原本所在格子被其他玩家经过」 -- needs the
    // Fx.PassTile hook (C# `DonutFx.PassTile` records the passer) to end the
    // exile when another player steps on `pos`, instead of the 99-layer expiry.
    // TODO(规则书)（2）: 「你原本所在格子被其他玩家经过时，可在那名玩家触发结算后选择传送至
    // 你原本所在格子（不包括）与那名玩家本次移动终点间的任一格并触发结算，之后你们各获得2火罐
    // （超出上限的每个火罐转化为500资金）」 -- needs Fx.PassTile + Fx.SettleAfter
    // (C# `DonutFx.SettleAfter` / `Back`): after the passer's settle, `ask_tile`
    // over the tiles from `pos` (exclusive) along the passer's direction to their
    // end, `H.Teleport(..., resolve: true)`, then `H.GainFire` 2 each with the
    // overflow converted to 500 money per missing fire.
    // C# `CardTwoDonuts.AiPlay` returns false -- CardDef has no H.AiPlay hook.
}