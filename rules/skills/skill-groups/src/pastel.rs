//! `skill:Pastel✽Palettes:与偶像一起`
//!
//! 规则书（band sheet, Pastel✽Palettes）:
//! > （1）游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张
//! > Pastel✽Palettes乐队卡时此效果不重复发动）。（2）你因任意原因受到"将X个反面
//! > [P✽P粉丝]变正"的效果且X大于拥有的反面的[P✽P粉丝]时可为此卡添加等量溢出的
//! > [奇迹水晶]（最多10个）。（3）[经过]CiRCLE时不获得[CiRCLE奖励]。（4）回合开始时
//! > 此卡添加1个[奇迹水晶]，然后可选择移除此卡5个[奇迹水晶]并抽1张卡。
//!
//! Fan markers live in the per-player state dict ([`crate::fans`]), owned by
//! no card. Clause （2） is an `On::Action` listener on
//! [`ruleset_core::kind::FansFlip`] (「…时」 state clause = event handler).

use card_sdk::abi::ActionKind;
use ruleset_core::ctx;
use ruleset_core::kind;
use card_sdk::{key, CardDef, Msg, On};
use crate::fans;

const ID: &str = "skill:Pastel✽Palettes:与偶像一起";

pub const PASTEL: CardDef = CardDef::new(
    ID,
    &[
        On::Action(&[ActionKind::DeckAtGameStart], "", None, at_start),
        On::Action(&[ActionKind::TurnStartBefore], card_sdk::pre::MINE, None, at_turn_start),
        // （2） overflow crystals. `FansFlip` is raised with the effect's X
        // before the dict writes, so `fans::down` is still the pre-flip count.
        // 「你」 is this card's owner (`actor == owner`).
        On::Action(&[kind::FansFlip], "actor == owner", None, on_fans_flip),
    ],
);

/// （1）「游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张
/// Pastel✽Palettes乐队卡时此效果不重复发动）」 -- the band card's own game-start
/// clause; it hits every non-Pastel✽Palettes character on the board, not just
/// the card's owner.
fn at_start(player_id: i32) -> card_sdk::Asked {
    // 「如有场上有多张…乐队卡时此效果不重复发动」 -- only the first copy on
    // the board (lowest seat holding one) deals.
    let first = (0..ctx::player_count()).find(|&p| ctx::placed_cards(p).iter().any(|c| c == ID));
    if first != Some(player_id) {
        return Ok(());
    }
    for p in 0..ctx::player_count() {
        if ctx::player_out(p) || ctx::in_group(p, "Pastel✽Palettes") {
            continue;
        }
        let _ = fans::grant(p, 0, 1)?;
    }
    Ok(())
}

/// （3）「[经过]CiRCLE时不获得[CiRCLE奖励]」 and （4）「回合开始时此卡添加1个
/// [奇迹水晶]…」. （3） is `prop::NO_REWARD` on this instance (`docs/TILES.md`)
/// -- the source owns the arming and the disarming, and the CiRCLE-reward step
/// reads it off the passing player's field. Set every turn start so the prop
/// tracks the card's presence.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    ctx::set_prop(card_sdk::abi::prop::NO_REWARD, 1);
    ctx::add_crystals(1, i32::MAX)?;
    if ctx::crystals() < 5 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("pastel_title")),
        &Msg::new(key!("pastel_draw")),
    )? {
        return Ok(());
    }
    ctx::add_crystals(-5, i32::MAX)?;
    ctx::draw(player_id, 1)?;
    Ok(())
}

/// （2）「你因任意原因受到"将X个反面[P✽P粉丝]变正"的效果且X大于拥有的反面的
/// [P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）」 --
/// `trigger::value()` is the effect's intended X; `fans::down` is still the
/// pre-flip count (the raise happens before the dict writes). 「可为此卡添加」
/// is the book's permissive may -- the overflow is applied.
///
/// 「最多10个」 caps this card's **TOTAL** crystals at 10 (user ruling
/// 2026-10-17), not the overflow amount: grant `X - down`, clamped by the
/// room left under 10. A pile already at/over 10 (clause （4）'s uncapped
/// tick can overshoot) gets nothing — never force-reduce.
fn on_fans_flip(player_id: i32) -> card_sdk::Asked {
    let x = ctx::trigger::value();
    let down = fans::down(player_id);
    if x <= down {
        return Ok(());
    }
    let before = ctx::crystals();
    if before >= 10 {
        return Ok(());
    }
    let after = ctx::add_crystals(x - down, 10)?;
    let added = after - before;
    if added <= 0 {
        return Ok(());
    }
    // Log the amount actually added (clamped by the total-10 cap), not the
    // requested overflow.
    ctx::log(
        player_id,
        &Msg::new(key!("pastel_over")).i("n", added as i64),
    );
    Ok(())
}

/// Roster band row for this band skill (ABI v63).
pub const BAND: ruleset_core::roster::GroupDef =
ruleset_core::roster::GroupDef::new(r#"Pastel✽Palettes"#)
    .short_name(r#"PP"#)
    .boot_art(r#"004"#)
    .color(r#"#33DDAA"#)
    .skill(r#"与偶像一起"#)
    .text(r#"（1）游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张Pastel✽Palettes乐队卡时此效果不重复发动）。（2）你因任意原因受到“将X个反面[P✽P粉丝]变正”的效果且X大于拥有的反面的[P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）。
（3）[经过]CiRCLE时不获得[CiRCLE奖励]。
（4）回合开始时此卡添加1个[奇迹水晶]，然后可选择移除此卡5个[奇迹水晶]并抽1张卡。"#)
    .logo(r#"004"#);

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_start_kani(player_id: i32) -> card_sdk::Asked {
    at_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_fans_flip_kani(player_id: i32) -> card_sdk::Asked {
    on_fans_flip(player_id)
}