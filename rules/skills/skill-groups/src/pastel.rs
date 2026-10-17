//! `skill:Pastel✽Palettes:与偶像一起`
//!
//! 规则书（band sheet, Pastel✽Palettes）:
//! > （1）游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张
//! > Pastel✽Palettes乐队卡时此效果不重复发动）。（2）你因任意原因受到"将X个反面
//! > [P✽P粉丝]变正"的效果且X大于拥有的反面的[P✽P粉丝]时可为此卡添加等量溢出的
//! > [奇迹水晶]（最多10个）。（3）[经过]CiRCLE时不获得[CiRCLE奖励]。（4）回合开始时
//! > 此卡添加1个[奇迹水晶]，然后可选择移除此卡5个[奇迹水晶]并抽1张卡。
//!
//! Fan markers `P✽P粉丝(正)/(反)` are units of **this** card's counter
//! (`docs/CARDS.md` → 「Pastel fans owned by the band skill」). Only this rule
//! creates or moves them; every other card sends `fansGrant` / `fansFlip`
//! ([`crate::fans`]). No band-skill instance on the field → no markers.

use card_sdk::abi::HookKind;
use ruleset_core::ctx;
use card_sdk::{key, CardDef, Msg, On};
use crate::fans;

const ID: &str = "skill:Pastel✽Palettes:与偶像一起";

pub const PASTEL: CardDef = CardDef::new(
    "skill:Pastel✽Palettes:与偶像一起",
    &[
        On::Action(&[HookKind::DeckAtGameStart], "", None, at_start),
        On::Action(&[HookKind::TurnStartBefore], card_sdk::pre::MINE, None, at_turn_start),
        // Fan ownership. `fansGrant` adds face-up / face-down to a holder;
        // `fansFlip` moves `X` in one direction and applies clause （2）.
        On::Message(&[fans::MSG_GRANT, fans::MSG_FLIP], "", None, on_fans),
    ],
);

/// （1）「游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张
/// Pastel✽Palettes乐队卡时此效果不重复发动）」 -- the band card's own game-start
/// clause; it hits every non-Pastel✽Palettes character on the board, not just
/// the card's owner. Direct write (this IS the owning rule).
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
        ctx::set_tok(p, fans::DOWN, ctx::tok(p, fans::DOWN) + 1);
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

/// `fansGrant` / `fansFlip`. Payload: `seat` = holder, `a`/`b` per
/// [`crate::fans`]. Direct `set_tok` (C# `H.GainFans` is a count write) so a
/// grant nested in an open pay pipeline does not open a `markerGain` window
/// (same reason `PP:再次闪耀` used `set_tok` before this split).
fn on_fans(player_id: i32) -> card_sdk::Asked {
    let seat = ctx::message::seat();
    if seat < 0 || ctx::player_out(seat) {
        ctx::message::reply(0);
        return Ok(());
    }
    let name_grant = ctx::message::name_is(fans::MSG_GRANT);
    if name_grant {
        let up = ctx::message::a().max(0);
        let down = ctx::message::b().max(0);
        if up > 0 {
            ctx::set_tok(seat, fans::UP, ctx::tok(seat, fans::UP) + up);
        }
        if down > 0 {
            ctx::set_tok(seat, fans::DOWN, ctx::tok(seat, fans::DOWN) + down);
        }
        // Wire reply = 1 + units (see `fans::MSG_FLIP`) so "handled, 0 moved"
        // is distinct from "no receiver".
        ctx::message::reply(up + down + 1);
        return Ok(());
    }
    if !ctx::message::name_is(fans::MSG_FLIP) {
        ctx::message::reply(0);
        return Ok(());
    }
    let x = ctx::message::a();
    let dir = ctx::message::b();
    if x <= 0 {
        // Handled, nothing to do — still non-zero so the sender stops.
        ctx::message::reply(1);
        return Ok(());
    }
    let (from, to) = if dir == fans::DIR_DOWN {
        (fans::UP, fans::DOWN)
    } else {
        (fans::DOWN, fans::UP)
    };
    let held = ctx::tok(seat, from);
    let flip = x.min(held).max(0);
    // Pre-flip count of the from-face, for clause （2） below.
    let pre_from = held;
    if flip > 0 {
        ctx::set_tok(seat, from, held - flip);
        ctx::set_tok(seat, to, ctx::tok(seat, to) + flip);
    }
    // （2）「你因任意原因受到"将X个反面[P✽P粉丝]变正"的效果且X大于拥有的
    // 反面的[P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）」 --
    // 「你」 is **this instance's owner**. Overflow is `X - pre` face-downs,
    // taken before the counters move. Every down→up flip site goes through
    // here (the retired FlipEffect/FlipAfter pair only covered `together_here`).
    if dir == fans::DIR_UP && seat == player_id && x > pre_from {
        let over = (x - pre_from).min(10);
        if over > 0 {
            let _ = ctx::add_crystals(over, 10);
            ctx::log(
                player_id,
                &Msg::new(key!("pastel_over")).i("n", over as i64),
            );
        }
    }
    ctx::message::reply(flip + 1);
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
pub fn on_fans_kani(player_id: i32) -> card_sdk::Asked {
    on_fans(player_id)
}