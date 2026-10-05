//! `PP:TITLE IDOL` -- C# `CardTitleIdol` (MatchHost.cs:7610-7628): +2 band
//! crystals, then +1 (or +2 with [共鸣]) on every field card whose text mentions
//! 「奇迹水晶」.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:TITLE IDOL`）:
//! > TITLE IDOL：
//! > [手]：
//! > 依次进行以下效果：
//! > 1. 为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]；
//! > 2. 为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]，如果[共鸣]则改为添加2个。
//!

use card_sdk::{ctx, CardDef, On};

pub const TITLE_IDOL: CardDef = CardDef::new("PP:TITLE IDOL", &[
    On::Play(title_idol),
]);

fn title_idol(player_id: i32) {
    // 规则书[手]1: 「为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]」
    ctx::add_band_crystals(player_id, 2, i32::MAX);
    // TODO(规则书)[手]2: 「如果[共鸣]则改为添加2个」 -- needs H.TryResonance (discard
    // 「PP:[衍生]共鸣」 from hand) to upgrade the count from 1 to 2.
    // 规则书[手]2: 「为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]」 -- the
    // Pastel✽Palettes band card's text contains 「奇迹水晶」 (C# adds `n` to the band
    // card for exactly that reason: 「团卡的效果也包含奇迹水晶」); the other matching
    // field cards are TODO below.
    ctx::add_band_crystals(player_id, 1, i32::MAX);
    // TODO(规则书)[手]2: 「所有效果包含[奇迹水晶]的卡」 beyond the band card -- needs
    // placed-card enumeration (C# `H.PlacedOf(i)` filtered on
    // `!p.FaceDown && !p.Immune && (H.Db.Card(p.Id)?.text ?? "").Contains("奇迹水晶")`)
    // and the per-card crystal counter (`item.AddCrystals(n, "TITLE IDOL")`).
}