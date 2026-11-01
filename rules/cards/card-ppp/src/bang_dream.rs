//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Bang Dream!`）:
//! > Bang Dream!：
//! > [手]：
//! > 为[使用者]的团卡添加一个[奇迹水晶]，然后[传送]至任意[使用者]拥有的格子且可选择盖房。
//!

use bandori_std::ctx;
use card_sdk::{key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const BANG_DREAM: CardDef = CardDef::new(
    "PPP:Bang Dream!",
    &[On::Play("", Some(cant_play), bang_dream)],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"Bang Dream!"#)
        .raw_name(r#"Bang Dream!"#)
        .tags(&[r#"手"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"为[使用者]的团卡添加一个[奇迹水晶]，然后[传送]至任意[使用者]拥有的格子且可选择盖房。"#),
            ]),
        ]))
        // sheet cell B7
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]),
);

fn cant_play(player_id: i32) -> Option<Msg> {
    // TODO(规则书)（[传送]至任意[使用者]拥有的格子）:
    if ctx::owned_count(player_id) == 0 {
        return Some(Msg::new(key!("bang_dream_no_land")));
    }
    None // playable
}

fn bang_dream(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「为[使用者]的团卡添加一个[奇迹水晶]」
    ctx::add_group_crystals(player_id, 1, i32::MAX);
    ctx::log(
        player_id,
        &Msg::new(key!("bang_dream_crystal")).player_id("who", player_id),
    );
    let mine = ctx::owned_tiles(player_id);
    if mine.is_empty() {
        return Ok(());
    }
    // 规则书: 「[传送]至任意[使用者]拥有的格子」
    let to = ctx::ask_tile(
        player_id,
        &Msg::new(key!("bang_dream_title")),
        &Msg::new(key!("bang_dream_ask")),
        &mine,
    )?;
    ctx::teleport_to(player_id, to);
    ctx::log(
        player_id,
        &Msg::new(key!("bang_dream_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    // Skips silently when `to` cannot take one.
    ctx::card_offer_build(player_id, &[to]);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn bang_dream_kani(player_id: i32) -> card_sdk::Asked {
    bang_dream(player_id)
}
