//! `PP:同一个梦想` -- C# `CardSameDream`: +3 band crystals, flip fans for money,
//!
//! 规则书（docs/rulebook/cards.json, id `PP:同一个梦想`）:
//! > 同一个梦想：
//! > [手]：
//! > 依次进行以下操作：
//! > 1. 为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]；
//! > 2. 将所有正面[P✽P粉丝]变反，[获得]变反数量乘100的资金；
//! > 3. 将自己的所有反面[P✽P粉丝]变正，如果[共鸣]则此效果对所有Pastel✽Palettes角色生效。
//!
//! then flip every face-down fan back up. The `3.` resonance half (the whole
//! Pastel✽Palettes band) is TODO below.

use card_sdk::{ctx, key, CardDef, Msg};

pub const SAME_DREAM: CardDef = CardDef { id: "PP:同一个梦想", play: Some(same_dream), can_react: None, react: None, why_not: None };

/// The C# `H.FansUp` / `H.FansDown` token names (`P✽P粉丝` faces).
const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

fn same_dream(seat: i32) {
    // 规则书[手]1: 「为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    ctx::add_band_crystals(seat, 3, i32::MAX);
    // 规则书[手]2: 「将所有正面[P✽P粉丝]变反，[获得]变反数量乘100的资金」
    let up = ctx::tok(seat, FANS_UP);
    if up > 0 {
        ctx::add_tok(seat, FANS_UP, -up, i32::MAX);
        ctx::add_tok(seat, FANS_DOWN, up, i32::MAX);
        ctx::log(seat, &Msg::new(key!("same_dream_down")).seat("who", seat).i("n", up as i64));
        ctx::gain(seat, up * 100, &Msg::new(key!("same_dream_why")).i("n", up as i64));
    }
    // 规则书[手]3: 「将自己的所有反面[P✽P粉丝]变正」
    let down = ctx::tok(seat, FANS_DOWN);
    if down > 0 {
        ctx::add_tok(seat, FANS_DOWN, -down, i32::MAX);
        ctx::add_tok(seat, FANS_UP, down, i32::MAX);
        ctx::log(seat, &Msg::new(key!("same_dream_up")).seat("who", seat).i("n", down as i64));
    }
    // TODO(规则书): 「如果[共鸣]则此效果对所有Pastel✽Palettes角色生效」 -- needs
    // H.TryResonance (discard 「PP:[衍生]共鸣」 from hand; +2 band crystals) and
    // H.InBand (which seats play Pastel✽Palettes) to run the flip for the band.
}