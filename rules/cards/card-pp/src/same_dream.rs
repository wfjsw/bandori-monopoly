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
//! then flip every face-down fan back up. Fan moves go through the band-skill
//! `fans::flip_up` (state dict + `FansFlip` raise).

use bandori_std::ctx;
use card_sdk::{key, CardDef, Msg, On};
use skill_groups::fans;

pub const SAME_DREAM: CardDef = CardDef::new("PP:同一个梦想", &[On::Play("", None, None, same_dream)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"同一个梦想"#)
        .raw_name(r#"同一个梦想"#)
        .tags(&[r#"手"#])
        .text(r#"[手]：
依次进行以下操作：
1. 为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]；
2. 将所有正面[P✽P粉丝]变反，[获得]变反数量乘100的资金；
3. 将自己的所有反面[P✽P粉丝]变正，如果[共鸣]则此效果对所有Pastel✽Palettes角色生效。"#)
        // sheet cell D3
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]));

fn same_dream(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]1: 「为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    ctx::add_group_crystals(player_id, 3, i32::MAX);
    // 规则书[手]2: 「将所有正面[P✽P粉丝]变反，[获得]变反数量乘100的资金」
    let up = fans::up(player_id);
    if up > 0 {
        let flipped = fans::flip_down(player_id, up)?;
        if flipped > 0 {
            ctx::log(
                player_id,
                &Msg::new(key!("same_dream_down"))
                    .player_id("who", player_id)
                    .i("n", flipped as i64),
            );
            ctx::gain(
                player_id,
                flipped * 100,
                &Msg::new(key!("same_dream_why")).i("n", flipped as i64),
            )?;
        }
    }
    // 规则书[手]3: 「将自己的所有反面[P✽P粉丝]变正，如果[共鸣]则此效果对所有
    // Pastel✽Palettes角色生效」 -- the resonance half widens the target from this
    // player to the whole band, and that is all it changes. X = the face-down
    // count (「所有」), so band clause （2） overflow is 0.
    let widen = crate::resonance::try_resonance(player_id)?;
    for p in 0..ctx::player_count() {
        if p != player_id && !(widen && ctx::in_group(p, "Pastel✽Palettes")) {
            continue;
        }
        let down = fans::down(p);
        if down > 0 {
            let flipped = fans::flip_up(p, down)?;
            if flipped > 0 {
                ctx::log(
                    p,
                    &Msg::new(key!("same_dream_up"))
                        .player_id("who", p)
                        .i("n", flipped as i64),
                );
            }
        }
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn same_dream_kani(player_id: i32) -> card_sdk::Asked {
    same_dream(player_id)
}
