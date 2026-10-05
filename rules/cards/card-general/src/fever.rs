//! `通用:[衍生]FEVER!` -- C# `CardFever`: stays in play, money to the owner +X.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:[衍生]FEVER!`）:
//! > [衍生]FEVER!：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]。
//! > [持续]：
//! >
//! > （1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。
//! >
//! > （2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const FEVER: CardDef = CardDef {
    id: "通用:[衍生]FEVER!",
    play: Some(fever),
    can_react: None,
    react: None,
    why_not: None,
};

fn fever(seat: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "通用:[衍生]FEVER!", &Msg::new(key!("fever_note")));
    // TODO(规则书)（1）: 「[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，
    // [拥有者]场上每拥有一张卡则X降低200（可小于0）」 -- needs the `Fx.PayAdd` hook
    // (C# `CardFever.PayAdd`) and a placed-card count (`H.PlacedOf`) for X.
    // `PlayCtx.N(0, 600)` (the doubled base) is not in the ABI either, so the
    // base 600 is not stored on the card.
    // TODO(规则书)（2）: 「[拥有者]回合开始时将此卡放入[使用者]弃卡区」 -- needs the
    // `Fx.TurnStart` hook (C# `CardFever.TurnStart` -> `H.Unplace`).
}