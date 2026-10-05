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
//! The ladder body runs off `cards_in(Field)` for the placed-card count.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const RANGER: CardDef = CardDef::new("PP:[衍生]魔法战队Pastel✽Ranger", &[
    On::Play(Some(cant_play), ranger)]);

/// The C# `H.FansUp` / `H.FansDown` token names (`P✽P粉丝` faces).
const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[特]: 「仅在你的绝对距离30-“粉丝数量”格内有你拥有房的格子时可使用」
    // C# `CardRanger.WhyNot`: `range = 30 - H.Fans(player_id)`, refuse unless some
    // owned tile has houses and `H.Dist(pos, t) <= range`.
    let range = 30 - (ctx::tok(player_id, FANS_UP) + ctx::tok(player_id, FANS_DOWN));
    let pos = ctx::player_pos(player_id);
    let ok = ctx::owned_tiles(player_id)
        .into_iter()
        .any(|t| ctx::houses_of(t) > 0 && ctx::dist(pos, t) <= range);
    if !ok {
        return Some(Msg::new(key!("ranger_why_not")).i("n", range.max(0) as i64));
    }
    None
}

/// 规则书[手]: 「根据场上你拥有的卡数量」 -- C# `H.PlacedOf(i).Count`.
fn placed_count(player_id: i32) -> i32 {
    ctx::cards_in(player_id, ctx::CardPile::Field).len() as i32
}

fn ranger(player_id: i32) {
    // 规则书[手]: 「根据场上你拥有的卡数量」
    let mut n = placed_count(player_id);
    // 规则书[手]: 「如果[共鸣]则视为数量加1」 -- the cost is the discard, and it
    // buys a fatter count rather than anything on its own.
    if crate::resonance::try_resonance(player_id) {
        n += 1;
    }
    ctx::log(player_id, &Msg::new(key!("ranger_count")).i("n", n as i64));
    // 规则书[手]1: 「数量至少为1则[获得]500资金」
    if n >= 1 {
        ctx::gain(player_id, 500, &Msg::new(key!("ranger_why")).i("n", n as i64));
    }
    // 规则书[手]2: 「数量至少为3则为Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    if n >= 3 {
        ctx::add_band_crystals(player_id, 3, i32::MAX);
    }
    // 规则书[手]3: 「数量至少为4则移除Pastel✽Palettes乐队卡4个[奇迹水晶]并抽1张卡」
    if n >= 4 {
        ctx::add_band_crystals(player_id, -4, i32::MAX);
        ctx::draw(player_id, 1);
    }
    // 规则书[手]4: 「数量至少为5则获得1层状态“失去2000资金，下次盖房时减免2000（可溢出），
    // 盖房后减少1层”」 -- the 2,000 loss is `H.LoseR(i, 2000, CardName)`.
    if n >= 5 {
        ctx::pay(player_id, 2000, &Msg::new(key!("ranger_why")).i("n", n as i64));
        // 规则书[手]4: 「下次盖房时减免2000（可溢出），盖房后减少1层」 -- a
        // layered cut on the build cost; the engine refunds the 「可溢出」 half
        // and pops one layer per build.
        ctx::set_build_discount(2000, 1);
    }
    // 规则书[手]5: 「数量至少为6则抽1张卡」
    if n >= 6 {
        ctx::draw(player_id, 1);
    }
}