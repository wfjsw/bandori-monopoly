//! `Mor:勇气展翅高飞之时` -- C# `CardCourageWings` (MatchHost.cs:4779-4803).
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:勇气展翅高飞之时`）:
//! > 勇气展翅高飞之时：
//! >  掷骰3d20，结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半，若为地产商地块，获得1000资金
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const COURAGE_WINGS: CardDef = CardDef {
    id: "Mor:勇气展翅高飞之时",
    play: Some(courage_wings),
    can_react: None,
    react: None,
    why_not: None,
};

fn courage_wings(seat: i32) {
    // 规则书: 「掷骰3d20，结果对应序号格子」
    // TODO(规则书): the C# rolls with H.CardRoll (PlayCtx.Extreme can force
    // max/min; the card is a RangeCard).
    let roll = ctx::roll(seat, 3, 20);
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    let tile = (roll - 1) % n;
    // 规则书: 「若为地产商地块，获得1000资金」
    // TODO(规则书): 「若为地产商地块，获得1000资金」 -- needs a tile-kind query
    // (C# `H._tiles[t].kind == "agent"`); `buy_price`/`rent_of` cannot tell an
    // agent tile from the other zero-priced tiles, so that branch is dropped and
    // an agent roll falls through to 「没有别的主人」 below.
    // 规则书: 「所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半」
    let owner = ctx::tile_owner(tile);
    if owner < 0 || owner == seat || ctx::seat_out(owner) {
        // C#: 掷到了 …：没有别的主人，没有效果
        ctx::log(seat, &Msg::new(key!("courage_wings_none")).tile("tile", tile));
        return;
    }
    // 「购买价格+地块已有房子的建造价格总额」 is `buy_price` (land + houses);
    // 「…的一半」 is the C# `(price + houses * house) / 2`.
    let amount = ctx::buy_price(tile) / 2;
    ctx::transfer(owner, seat, amount, &Msg::new(key!("courage_wings_why")));
}