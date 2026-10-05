//! `PP:[若宫伊芙]属于我的武士道！` -- C# `CardEveBushido`: stay in play, roll 12d4
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[若宫伊芙]属于我的武士道！`）:
//! > [若宫伊芙]属于我的武士道！：
//! > [手]：
//! > 将此卡放置在[使用者]场上并投掷12d4，获得投掷结果*60的资金。
//! > [持续]：
//! >
//! > （1）[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）。
//! >
//! > （2）[结算]前如果此卡上有[奇迹水晶]且[拥有者]所在格子的地契主人为其他玩家则使用1个[奇迹水晶]并依次进行以下操作：
//! > 1. 地契主人投掷1d6；
//! > 2. [拥有者]投掷3d4；
//! > 3. 投掷点数低的玩家[支付]投掷点数高的玩家[拥有者]所在格子的房屋数量加1×100（如果平局则双方互不支付），如果[共鸣]则[支付]金额改为“投掷点差”×50。
//!
//! for money. The [持续] duel is TODO below.

use card_sdk::{ctx, key, CardDef, Msg};

pub const EVE_BUSHIDO: CardDef = CardDef {
    id: "PP:[若宫伊芙]属于我的武士道！",
    play: Some(eve_bushido),
    can_react: None,
    react: None,
    why_not: None,
};

fn eve_bushido(seat: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PP:[若宫伊芙]属于我的武士道！", &Msg::new(key!("eve_bushido_note")));
    // 规则书[手]: 「并投掷12d4，获得投掷结果*60的资金」
    // TODO: C# uses H.CardRoll, which honours PlayCtx.Extreme (forced max/min dice).
    let n = ctx::roll(seat, 12, 4);
    ctx::gain(seat, n * 60, &Msg::new(key!("eve_bushido_why")).i("n", n as i64));
    // TODO(规则书): [持续]（1）「[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）」 -- needs
    // the Fx.Drew hook (C# `Card.Drew`) and the field-card crystal counter
    // (`H.AddCrystals` on this placed card, cap 3).
    // TODO(规则书): [持续]（2）「[结算]前……地契主人投掷1d6；[拥有者]投掷3d4；投掷点数低的玩家
    // [支付]投掷点数高的玩家[拥有者]所在格子的房屋数量加1×100（如果平局则双方互不支付），
    // 如果[共鸣]则[支付]金额改为“投掷点差”×50」 -- needs the Fx.SettleBefore hook
    // (C# `Card.SettleBefore(MoveCtx)`) and the field-card crystal counter to
    // spend one. House counts are `ctx::houses_of` now; the amount is
    // `(houses + 1) * 100`. H.TryResonance still missing for the 「点差 ×50」 swap.
}