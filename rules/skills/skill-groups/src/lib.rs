//! Band skills as card rules -- one file per skill, one crate for the lot.
//!
//! Separated from `rules/cards/` for the same reason as
//! [`skill-personas`](../skill_personas): these are abilities a band comes
//! with, not cards you draw. Same rule language ([`CardDef`]), separate crate.
//!
//! The rule text lives in the roster band row (`skill` is the name, `text` /
//! `simple` the wording). A band skill's [奇迹水晶] live on its own field
//! instance (`FieldCard::crystals`) -- `ctx::crystals` / `ctx::add_crystals`
//! from inside this rule, `ctx::group_crystals` / `ctx::add_group_crystals` from
//! any card meaning 「乐队卡 / 团卡」. One pool, no keyed-state side store.
//!
//! Hooks: the same two the character skills use, no third kind. A half the
//! player presses is [`On::Play`] with its gate; a half a field event calls is
//! [`On::Action`] (the skill is a field card, C# `Fx`). See the module docs in
//! `skill-personas` for the shape and for what is still missing (id
//! resolution, not hooks).

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use card_sdk::CardDef;

pub mod ave_mujica;
use ave_mujica::AVE_MUJICA;
pub mod afterglow;
use afterglow::AFTERGLOW;
pub mod roselia;
use roselia::ROSELIA;
pub mod morfonica;
use morfonica::MORFONICA;
pub mod ras;
use ras::RAS;
pub mod mygo;
use mygo::MYGO;
pub mod sumimi;
use sumimi::SUMIMI;
pub mod pastel;
use pastel::PASTEL;
pub mod fans;
pub mod hhw;
use hhw::HHW;
pub mod circle_staff;
use circle_staff::CIRCLE_STAFF;
pub mod poppin;
use poppin::POPPIN;
pub mod crychic;
use crychic::CRYCHIC;
pub mod roster_table;

// The 规则书 text for these is a Google Sheet (see `skill-rulebook-sheets` in
// the project memory, or fetch `export?format=csv&gid=1398197110` on the
// `1xZ3avBsNBXbl3bQ74lmPs0YQFgZkPD0Sdzmx7ZGGEDY` document), column `技能`.
// the roster band row (`text`) is a faithful copy of it -- 12/12 rows match modulo
// the sheet's literal backslash-n escapes -- so the JSON is safe to code against.
//
// All 12 band skills are written and bound (`bind_skills`). What is left on
// each is its own `TODO(规则书)` / `TODO(ABI)` note -- 4 of the 12 carry one
// (2026-10-06).
//
pub const CARDS: &[CardDef] = &[
    AVE_MUJICA,
    AFTERGLOW,
    ROSELIA,
    MORFONICA,
    RAS,
    MYGO,
    SUMIMI,
    PASTEL,
    HHW,
    CIRCLE_STAFF,
    POPPIN,
    CRYCHIC,
];

/// 「星星贴纸」 owed by the running press as the `PPP:Returns` [持续]（3）
/// activation tax: 1 for the borrowed copy, 0 for the owner's own band card.
///
/// `PPP:Returns` places the borrowed band and records its uid in
/// `returns.borrow_uid`; the owner's own band card sits beside it (ruling
/// 2026-10-06) and its presses are free. Single source of truth — every
/// caller derives from this, and effects that spend the same token budget it
/// on top of their own cost.
pub fn copy_sticker_cost(player_id: i32) -> i32 {
    // (3) is a [持续] of `PPP:Returns`: once Returns has left the field (or is
    // face down) the borrowed copy stays, but the tax does not -- the same
    // reason (1) scans the field instead of keeping a token.
    if bandori_std::ctx::find_card(player_id, "PPP:Returns").is_none()
        || bandori_std::ctx::card_face_down(player_id, "PPP:Returns")
    {
        return 0;
    }
    let borrow = bandori_std::ctx::tok(player_id, "returns.borrow_uid");
    if borrow != 0 && bandori_std::ctx::self_uid() == borrow {
        1
    } else {
        0
    }
}

/// Play-gate half of 「使用因此卡获得的团卡的主动效果时需要支付1星星贴纸」.
///
/// 「需要支付」 is a **cost of activation**: with too few stickers the borrowed
/// band's 主动 effect must not run. `Err(Prompt)` is the pause sentinel
/// (`card-sdk`), not a refusal — the refusal belongs on the press's `cant_play`.
pub fn copy_sticker_gate(player_id: i32) -> Option<card_sdk::Msg> {
    let cost = copy_sticker_cost(player_id);
    if cost > 0 && bandori_std::ctx::tok(player_id, "星星贴纸") < cost {
        Some(card_sdk::Msg::new(card_sdk::key!("skill_band_no_sticker")))
    } else {
        None
    }
}

/// Charge the tax once the press is payable (`copy_sticker_gate` is the
/// refusal). Skipping this would make a payable press free.
pub fn spend_copy_sticker(player_id: i32) -> card_sdk::Asked {
    let cost = copy_sticker_cost(player_id);
    if cost > 0 {
        bandori_std::ctx::add_tok(player_id, "星星贴纸", -cost, i32::MAX)?;
    }
    Ok(())
}

/// `Sumimi:#L11`'s [奇迹水晶] cover: 「每当乐队技能需要移除[奇迹水晶]时，可移除此卡
/// 上的一个[奇迹水晶]代替」. Read by the gates so a skill with an empty own pool
/// is still payable. Every copy on the field counts -- `find_card` is a name
/// resolver and would only ever see the first.
fn l11_cover(player_id: i32) -> i32 {
    let mut n = 0;
    for (uid, id) in bandori_std::ctx::field_instances(player_id) {
        if id == "Sumimi:#L11" {
            n += bandori_std::ctx::counter_at(uid, card_sdk::abi::counter::CRYSTALS).max(0);
        }
    }
    n
}

/// [奇迹水晶] a band-skill cost may draw on right now: the skill's own plus
/// the `Sumimi:#L11` cover. Gate on this, not on `ctx::crystals()`.
pub fn cover_crystals(player_id: i32) -> i32 {
    bandori_std::ctx::crystals() + l11_cover(player_id)
}

/// Pay `n` [奇迹水晶] of a band-skill cost: `Sumimi:#L11`'s cover first (every
/// copy, in placement order), the skill's own pool for the rest in one write.
/// Callers gate on [`cover_crystals`] first.
pub fn spend_crystals(player_id: i32, n: i32) -> card_sdk::Asked {
    let mut left = n;
    for (uid, id) in bandori_std::ctx::field_instances(player_id) {
        if left <= 0 {
            break;
        }
        if id != "Sumimi:#L11" {
            continue;
        }
        while left > 0 {
            let have = bandori_std::ctx::counter_at(uid, card_sdk::abi::counter::CRYSTALS);
            if have <= 0 {
                break;
            }
            bandori_std::ctx::add_counter_at(uid, card_sdk::abi::counter::CRYSTALS, -1, i32::MAX);
            left -= 1;
        }
    }
    if left > 0 {
        bandori_std::ctx::add_crystals(-left, i32::MAX)?;
    }
    Ok(())
}
