//! `PP:[衍生]共鸣` -- the [共鸣] fuel card.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]共鸣`）:
//! > [衍生]共鸣：
//! > [特]：
//! > 拥有此卡的玩家的卡可发动[共鸣]时将此卡放入弃卡区，发动那张卡的[共鸣]效果并为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]。
//!
//! Not a hand play (no `Play`): the [特] is what every other card's [共鸣] branch
//! calls, and it is [`try_resonance`] below. The clause is one sentence with
//! three parts and they all happen together:
//!
//! - 「将此卡放入弃卡区」 -- the cost is this card leaving the hand, so a player
//!   holding none cannot offer it at all.
//! - 「发动那张卡的[共鸣]效果」 -- the caller's own branch; this module only
//!   decides whether it runs.
//! - 「为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]」 -- paid on top
//!   of the branch, to the owner's Pastel✽Palettes band card.

use bandori_std::ctx;
use card_sdk::{key, CardDef, Msg};
use card_sdk::{para, sec, CardText};

/// The [共鸣] fuel card. Empty `On`: it has no hand play and no hook of its own
/// -- it is fuel for other cards' [共鸣] branches, offered by [`try_resonance`].
pub const RESONANCE: CardDef = CardDef::new("PP:[衍生]共鸣", &[]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"共鸣"#)
        .raw_name(r#"[衍生]共鸣"#)
        .derived(true)
        .tags(&[r#"特"#, r#"衍生"#])
        .text(CardText::of(&[
            sec(r#"[特]："#, &[
                para(r#"拥有此卡的玩家的卡可发动[共鸣]时将此卡放入弃卡区，发动那张卡的[共鸣]效果并为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]。"#),
            ]),
        ]))
        // sheet cell D8
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]));

/// Offer the [共鸣] cost. Returns whether the player paid it, which is the
/// caller's signal to run its own [共鸣] branch.
///
/// `false` when they decline or hold none -- the cost is this card, not some
/// abstract point. On success the card is discarded and, for the owner's
/// Pastel✽Palettes band card, 2 [奇迹水晶] are added (「添加2[奇迹水晶]」).
pub fn try_resonance(player_id: i32) -> Result<bool, card_sdk::Prompt> {
    // 规则书: 「拥有此卡的玩家的卡可发动[共鸣]时」
    if ctx::hand_count(player_id, RESONANCE.id) <= 0 {
        return Ok(false);
    }
    let yes = ctx::ask_yes(
        player_id,
        &Msg::new(key!("resonance_title")),
        &Msg::new(key!("resonance_text")).card("card", "PP:[衍生]共鸣"),
    )?;
    if !yes {
        return Ok(false);
    }
    // 规则书: 「拥有此卡的玩家的卡可发动[共鸣]时将此卡放入弃卡区」 -- one primitive:
    // `take_card` + `to_discard` would remove a second copy of the fuel.
    if !ctx::discard_from_hand(player_id, RESONANCE.id) {
        return Ok(false);
    }
    // TODO(规则书)（发动那张卡的[共鸣]效果并为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]）:
    //   the book resolves the branch first and the 2 crystals after; we add them
    //   here, so every caller's branch always runs with them already on the card.
    // 规则书: 「并为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]」 -- the
    // clause names the owner's band card, not the persona's band, so the write is
    // addressed to the Pastel✽Palettes band instance itself.
    for (uid, id, _) in ctx::group_skills(player_id) {
        if id.as_str().starts_with("skill:Pastel✽Palettes:") {
            ctx::add_crystals_at(uid, 2, i32::MAX);
        }
    }
    // 规则书: 「发动那张卡的[共鸣]效果」
    Ok(true)
}
