//! `PP:[衍生]魔法战队Pastel✽Ranger` -- C# `CardRanger` (MatchHost.cs:7766-7814):
//! a ladder of rewards keyed on how many field cards the user has.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]魔法战队Pastel✽Ranger`）:
//! > [衍生]魔法战队Pastel✽Ranger：
//! > [特]：
//! > 仅在你的绝对距离30-“粉丝数量”格内有你拥有房的格子时可使用。
//! > [手]：
//! > 根据场上你拥有的卡数量并依次进行以下操作，如果[共鸣]则视为数量加1：
//! > 1. 数量至少为1则[获得]500资金；
//! > 2. 数量至少为3则为Pastel✽Palettes乐队卡添加3个[奇迹水晶]；
//! > 3. 数量至少为4则移除Pastel✽Palettes乐队卡4个[奇迹水晶]并抽1张卡；
//! > 4. 数量至少为5则获得1层状态“失去2000资金，下次盖房时减免2000（可溢出），盖房后减少1层”；
//! > 5.数量至少为6则抽1张卡。
//!
//! The ladder body is here; the card count it keys on is TODO(ABI) below.

use card_sdk::{ctx, key, CardDef, Msg};

pub const RANGER: CardDef = CardDef {
    id: "PP:[衍生]魔法战队Pastel✽Ranger",
    play: Some(ranger),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// The C# `H.FansUp` / `H.FansDown` token names (`P✽P粉丝` faces).
const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

fn why_not(seat: i32) -> Option<Msg> {
    // 规则书[特]: 「仅在你的绝对距离30-“粉丝数量”格内有你拥有房的格子时可使用」
    // C# `CardRanger.WhyNot`: `range = 30 - H.Fans(seat)`, refuse unless some
    // owned tile has houses and `H.Dist(pos, t) <= range`.
    let range = 30 - (ctx::tok(seat, FANS_UP) + ctx::tok(seat, FANS_DOWN));
    let pos = ctx::seat_pos(seat);
    let ok = ctx::owned_tiles(seat)
        .into_iter()
        .any(|t| ctx::houses_of(t) > 0 && ctx::dist(pos, t) <= range);
    if !ok {
        return Some(Msg::new(key!("ranger_why_not")).i("n", range.max(0) as i64));
    }
    None
}

/// 规则书[手]: 「根据场上你拥有的卡数量」 -- C# `H.PlacedOf(i).Count`.
///
/// TODO(ABI): the ABI has no placed-card enumeration (`is_placed` only knows
/// about the running card), so this returns 0 and none of the thresholds fire.
/// Fill this in when `H.PlacedOf` lands and the ladder below lights up.
fn placed_count(_seat: i32) -> i32 {
    0
}

fn ranger(seat: i32) {
    // 规则书[手]: 「根据场上你拥有的卡数量」
    let n = placed_count(seat);
    // TODO(规则书)[手]: 「如果[共鸣]则视为数量加1」 -- needs H.TryResonance (discard
    // 「PP:[衍生]共鸣」 from hand) to treat the count as one higher.
    // C# `H.Log("text", i, "场上的卡数：" + n)`.
    ctx::log(seat, &Msg::new(key!("ranger_count")).i("n", n as i64));
    // 规则书[手]1: 「数量至少为1则[获得]500资金」
    if n >= 1 {
        ctx::gain(seat, 500, &Msg::new(key!("ranger_why")).i("n", n as i64));
    }
    // 规则书[手]2: 「数量至少为3则为Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    if n >= 3 {
        ctx::add_band_crystals(seat, 3, i32::MAX);
    }
    // 规则书[手]3: 「数量至少为4则移除Pastel✽Palettes乐队卡4个[奇迹水晶]并抽1张卡」
    if n >= 4 {
        ctx::add_band_crystals(seat, -4, i32::MAX);
        ctx::draw(seat, 1);
    }
    // 规则书[手]4: 「数量至少为5则获得1层状态“失去2000资金，下次盖房时减免2000（可溢出），
    // 盖房后减少1层”」 -- the 2,000 loss is `H.LoseR(i, 2000, CardName)`.
    if n >= 5 {
        ctx::pay(seat, 2000, &Msg::new(key!("ranger_why")).i("n", n as i64));
        // TODO(规则书)[手]4: 「下次盖房时减免2000（可溢出），盖房后减少1层」 -- needs
        // the H.ExtraOf BuildDiscountFx attachment (C# `H.ExtraOf<BuildDiscountFx>(i)
        // .Add(2000, CardName)`: a layered `Fx.BuildCost` cut with overflow refund
        // and one layer popped per `Fx.Built`).
    }
    // 规则书[手]5: 「数量至少为6则抽1张卡」
    if n >= 6 {
        ctx::draw(seat, 1);
    }
}