//! `PP:TITLE IDOL` -- C# `CardTitleIdol` (MatchHost.cs:7610-7628): +2 band
//!
//! 规则书（docs/rulebook/cards.json, id `PP:TITLE IDOL`）:
//! > TITLE IDOL：
//! > [手]：
//! > 依次进行以下效果：
//! > 1. 为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]；
//! > 2. 为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]，如果[共鸣]则改为添加2个。
//!
//! crystals, then +1 (or +2 with [共鸣]) on every field card whose text mentions
//! 「奇迹水晶」.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const TITLE_IDOL: CardDef = CardDef::new("PP:TITLE IDOL", &[On::Play("", None, title_idol)]);

fn title_idol(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]1: 「为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]」
    ctx::add_band_crystals(player_id, 2, i32::MAX);
    // 规则书[手]2: 「如果[共鸣]则改为添加2个」 -- paying the [共鸣] cost upgrades
    // the count from 1 to 2.
    let n = if crate::resonance::try_resonance(player_id)? {
        2
    } else {
        1
    };
    // 规则书[手]2: 「为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]」 --
    // every placed card whose text mentions 「奇迹水晶」 and which is neither
    // face-down nor immune (C# `H.PlacedOf(i)` + `p.FaceDown` + `p.Immune`). The
    // Pastel✽Palettes band card's text contains 「奇迹水晶」 (「团卡的效果也包含
    // 奇迹水晶」), so it is in this loop too -- clause 1's +2 and clause 2's +n
    // are two separate adds to the same pool.
    for (uid, c) in ctx::field_instances(player_id) {
        if ctx::is_face_down_at(uid) || ctx::is_immune_at(uid) {
            continue;
        }
        if !ctx::card_text_mentions(&c, "奇迹水晶") {
            continue;
        }
        ctx::add_crystals_at(uid, n, 0);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("title_idol_crystals"))
            .player_id("who", player_id)
            .i("n", n as i64),
    );
    Ok(())
}
