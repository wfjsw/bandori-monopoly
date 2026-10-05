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

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const SAME_DREAM: CardDef = CardDef::new("PP:同一个梦想", &[On::Play(same_dream)]);

/// The C# `H.FansUp` / `H.FansDown` token names (`P✽P粉丝` faces).
const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

fn same_dream(player_id: i32) {
    // 规则书[手]1: 「为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    ctx::add_band_crystals(player_id, 3, i32::MAX);
    // 规则书[手]2: 「将所有正面[P✽P粉丝]变反，[获得]变反数量乘100的资金」
    let up = ctx::tok(player_id, FANS_UP);
    if up > 0 {
        ctx::add_tok(player_id, FANS_UP, -up, i32::MAX);
        ctx::add_tok(player_id, FANS_DOWN, up, i32::MAX);
        ctx::log(player_id, &Msg::new(key!("same_dream_down")).player_id("who", player_id).i("n", up as i64));
        ctx::gain(player_id, up * 100, &Msg::new(key!("same_dream_why")).i("n", up as i64));
    }
    // 规则书[手]3: 「将自己的所有反面[P✽P粉丝]变正」
    let down = ctx::tok(player_id, FANS_DOWN);
    if down > 0 {
        ctx::add_tok(player_id, FANS_DOWN, -down, i32::MAX);
        ctx::add_tok(player_id, FANS_UP, down, i32::MAX);
        ctx::log(player_id, &Msg::new(key!("same_dream_up")).player_id("who", player_id).i("n", down as i64));
    }
    // TODO(规则书): 「如果[共鸣]则此效果对所有Pastel✽Palettes角色生效」 -- needs
    // H.TryResonance (discard 「PP:[衍生]共鸣」 from hand; +2 band crystals) and
    // H.InBand (which players play Pastel✽Palettes) to run the flip for the band.
}