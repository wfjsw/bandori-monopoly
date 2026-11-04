//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（喵梦）`）:
//! > （喵梦）
//! >
//! > （1）将此卡放置于场上
//!
//! > （2）
//! > 1. 每当此卡将从正面翻至背面时可选择失去2个火罐。
//! > 2. 每当此卡将从背面翻至正面时此卡拥有者抽一张卡。
//!
//! Both flip clauses ride the flip hooks: `cardFaceFlipped` (before the face
//! changes, where the old face is still readable) offers the fire-pot cost, and
//! `cardFaceFlippedAfter` draws on a turn to the face-up side. The card's own
//! hooks run while it is face-down (the subject pass has no face guard).

use card_sdk::abi::ZoneChangeCause;
use card_sdk::{ctx, key, Action, CardDef, Msg, On};
use card_sdk::{it, it_sub, CardText};

const ID: &str = "Mujica:（喵梦）";

pub const NYAMU_CARD: CardDef = CardDef::new(
    "Mujica:（喵梦）",
    &[
        On::Play("", None, nyamu_card),
        // 规则书: 「每当此卡将从正面翻至背面时可选择失去2个火罐」
        On::Action(Action::CardFaceFlipped("card.placed", None, flip_down)),
        // 规则书: 「每当此卡将从背面翻至正面时此卡拥有者抽一张卡」
        On::Action(Action::CardFaceFlippedAfter("card.placed", None, flip_up)),
    ],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name("")
        .raw_name(r#"（喵梦）"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"将此卡放置于场上"#),
            it_sub(
                r#""#,
                &[
                    r#"每当此卡将从正面翻至背面时可选择失去2个火罐。"#,
                    r#"每当此卡将从背面翻至正面时此卡拥有者抽一张卡。"#,
                ],
            ),
        ]))
        // sheet cell J15
        .fields(&[("group", r#"Ave Mujica"#), ("owner", r#"祐天寺若麦"#)]),
);

/// 「将从正面翻至背面」 -- before the face changes, the old face is still the
/// live one; `trigger::value()` is the face it turns to.
///
/// 规则书: 「每当此卡将从正面翻至背面时可选择失去2个火罐」
fn flip_down(player_id: i32) -> card_sdk::Asked {
    // The kind is an instance kind, so another copy sees this raise too.
    if ctx::trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    if ctx::self_face_down() || ctx::trigger::value() != 1 {
        return Ok(());
    }
    if bandori_std::fire::get(player_id) < 2 {
        return Ok(());
    }
    // 规则书: 「可选择失去2个火罐」 -- the loss is the player's choice.
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("nyamu_flip_title")),
        &Msg::new(key!("nyamu_flip_ask")),
    )? {
        return Ok(());
    }
    // 规则书: 「失去2个火罐」 -- a loss, not a cost: the pots leave the pool.
    bandori_std::fire::spend(player_id, 2, &Msg::new(key!("nyamu_flip_lost")))?;
    ctx::log(player_id, &Msg::new(key!("nyamu_flip_lost")));
    Ok(())
}

/// 「将从背面翻至正面」 -- the after hook, so the new face is the live one
/// (`trigger::value() == 0`).
///
/// 规则书: 「每当此卡将从背面翻至正面时此卡拥有者抽一张卡」
fn flip_up(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    if ctx::trigger::value() != 0 {
        return Ok(());
    }
    ctx::draw(player_id, 1)?;
    ctx::log(player_id, &Msg::new(key!("nyamu_flipped_up")));
    Ok(())
}

fn nyamu_card(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "Mujica:（喵梦）", &Msg::new(key!("nyamu_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("nyamu_placed")).player_id("who", player_id),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn nyamu_card_kani(player_id: i32) -> card_sdk::Asked {
    nyamu_card(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn flip_down_kani(player_id: i32) -> card_sdk::Asked {
    flip_down(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn flip_up_kani(player_id: i32) -> card_sdk::Asked {
    flip_up(player_id)
}
