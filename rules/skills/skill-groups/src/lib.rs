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

/// Is the running instance the 「因此卡获得的团卡」 copy?
///
/// `PPP:Returns` places the borrowed band and records its uid in
/// `returns.borrow_uid`; the owner's own band card sits beside it (ruling
/// 2026-10-06) and its presses are free.
fn is_borrowed_copy(player_id: i32) -> bool {
    let borrow = ruleset_core::ctx::tok(player_id, "returns.borrow_uid");
    borrow != 0 && ruleset_core::ctx::self_uid() == borrow
}

/// Play-gate half of 「使用因此卡获得的团卡的主动效果时需要支付1星星贴纸」.
///
/// 规则书（`PPP:Returns` [持续]（3））: 「需要支付」 is a **cost of activation**:
/// with 0 stickers the borrowed band's 主动 effect must not run. `Err(Prompt)`
/// is the pause sentinel (`card-sdk`), not a refusal -- the refusal belongs on
/// the press's `cant_play` gate, keyed the same way as [`spend_copy_sticker`].
/// Returns `Some(msg)` only when the running instance **is** the borrowed copy
/// and the owner holds fewer than 1 「星星贴纸」.
pub fn copy_sticker_gate(player_id: i32) -> Option<card_sdk::Msg> {
    if !is_borrowed_copy(player_id) {
        return None;
    }
    if ruleset_core::ctx::tok(player_id, "星星贴纸") < 1 {
        return Some(card_sdk::Msg::new(card_sdk::key!(
            "skill_band_no_sticker"
        )));
    }
    None
}

/// Spend the 「星星贴纸」 「使用因此卡获得的团卡的主动效果时需要支付」.
///
/// Charges exactly 1 sticker when the borrowed copy's press is payable.
/// [`copy_sticker_gate`] is the refusal (0 stickers → the press never runs);
/// this body must still charge when it does, or a payable press would skip
/// the cost. Only the **borrowed** copy pays (see [`is_borrowed_copy`]).
pub fn spend_copy_sticker(player_id: i32) -> card_sdk::Asked {
    if is_borrowed_copy(player_id) {
        ruleset_core::ctx::add_tok(player_id, "星星贴纸", -1, i32::MAX)?;
    }
    Ok(())
}
