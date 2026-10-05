//! `Mor:蝴蝶飞舞的星月夜` -- C# `CardStarryNight` (MatchHost.cs:4666-4778): pay X
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:蝴蝶飞舞的星月夜`）:
//! > 蝴蝶飞舞的星月夜：
//! >
//! > （1） 支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]，此卡没有[奇迹水晶]时加入弃牌堆; 此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次
//! >
//! > （2）当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个[奇迹水晶]，此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]。
//! >
//! > （3）此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。
//!
//! ×1000 to stock X crystals; each removal pays the owner 1,000.

use card_sdk::{ctx, key, CardDef, Msg};

pub const STARRY_NIGHT: CardDef = CardDef {
    id: "Mor:蝴蝶飞舞的星月夜",
    play: Some(starry_night),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // C# `CardStarryNight.WhyNot` refuses the play with less than 1,000
    // (`资金不够 1,000`).
    if ctx::money(seat) < 1000 {
        return Some(Msg::new(key!("starry_night_why_not")));
    }
    None
}

fn starry_night(seat: i32) {
    // 规则书（1）: 「支付X次1000的的资金」 -- C# `H.AskNumber(..., 1, max)` with
    // `max = Math.Max(1, Math.Min(5, money / 1000))`.
    let money = ctx::money(seat);
    let max = (money / 1000).clamp(1, 5);
    let x = ctx::ask_number(
        seat,
        &Msg::new(key!("starry_night_ask_title")),
        &Msg::new(key!("starry_night_ask_text")),
        1,
        max,
    );
    // 规则书（1）: 「支付X次1000的的资金」 -- C# `PayCtx { amount = 1000 * x, kind = "pay", must = false }`.
    let paid = ctx::pay(seat, 1000 * x, &Msg::new(key!("starry_night_why")).i("n", x as i64));
    if paid < 1000 * x {
        // C# `c.Effective = false` when the payment does not go through.
        // TODO(ABI): `PlayCtx.Effective` is not writable; the card is spent anyway.
        return;
    }
    // 规则书（1）: 「将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Mor:蝴蝶飞舞的星月夜", &Msg::new(key!("starry_night_note")).i("n", x as i64));
    ctx::log(seat, &Msg::new(key!("starry_night_placed")).seat("who", seat).i("n", x as i64));
    // TODO(规则书)（1）: 「并在此卡上放置X个[奇迹水晶]，此卡没有[奇迹水晶]时加入弃牌堆」 -- needs
    //   the field-card crystal counter (`H.PlaceFromPlay(c, -1, -1, x)` / `H.AddCrystals`).
    // TODO(规则书)（1）: 「此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次」 -- needs
    //   the Fx.RollAfter hook (C# `CardStarryNight.RollAfter` -> `Reroll`, `H.DoMoveRoll`).
    // TODO(规则书)（2）: 「当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个
    //   [奇迹水晶]」 -- needs the same Fx.RollAfter hook (C# `Attack`) plus the crystal counter.
    // TODO(规则书)（2）: 「此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]」 -- needs the Fx.PassTile
    //   hook on the "circle"-kind tile (C# `CardStarryNight.PassTile`).
    // TODO(规则书)（3）: 「此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金」 -- needs the
    //   crystal-removal payout (C# `CardStarryNight.Remove` -> `H.GainR(Seat, 1000, ...)`).
}