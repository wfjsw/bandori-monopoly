//! `skill:若宫伊芙:天下统一`
//!
//! 规则书（skill sheet, 若宫伊芙）:
//! > （1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得若宫伊芙
//! > 的（2）技能
//! > （2）当你因任意原因投掷前可选择将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加
//! > Yd4；如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正，
//! > 否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正
//!
//! （1）'s grant is the skill rule placed on the grantee's field (see
//! [`super::aya_with`]).
//!
//! （2） 「因任意原因投掷前」 is every roll, not just the move dice -- `DiceRollAfter`
//! is the moment the face exists, and 「为此次投掷结果增加Yd4」 adds Yd4 to it.

use card_sdk::abi::HookKind;
use ruleset_core::ctx;
use card_sdk::{key, CardDef, Msg, On};
use skill_groups::fans;

pub const EVE_UNIFY: CardDef = CardDef::new(
    "skill:若宫伊芙:天下统一",
    &[
        On::Action(
            &[HookKind::DeckAtGameStart],
            "group_is(owner, 'Pastel✽Palettes')",
            None,
            at_start,
        ),
        On::Action(
            &[HookKind::DiceRollAfter],
            "actor == owner && tok('P✽P粉丝(正)') >= 1",
            Some(roll_ok),
            on_roll,
        ),
    ],
);

/// Residual guard for （2） -- `skill_blocked` (band-specific) and `fixed_roll`
/// (derived plan state) stay here (not yet in the condition vocabulary).
/// `actor == owner && tok('P✽P粉丝(正)') >= 1` is the pre.
fn roll_ok(player_id: i32) -> bool {
    if ruleset_core::ctx::skill_blocked(player_id, "Pastel✽Palettes") {
        return false;
    }
    ctx::fixed_roll().is_none()
}

/// （1）「游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得若宫伊芙的
/// （2）技能」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    // （1） belongs to the skill's own Pastel✽Palettes character, not to
    // the grantees of the (2) below -- the `group_is(owner, …)` pre owns that.
    let _ = fans::grant(player_id, 5, 0)?;
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) || ctx::in_group(p, "Pastel✽Palettes") {
            continue;
        }
        ctx::place_card(
            p,
            "skill:若宫伊芙:天下统一",
            &Msg::new(key!("eve_unify_granted")),
        );
    }
    Ok(())
}

/// （2）「将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加Yd4」. The fixed-face
/// and has-fans clauses are the residual guard / pre.
fn on_roll(player_id: i32) -> card_sdk::Asked {
    let up = ctx::tok(player_id, fans::UP);
    let y = ctx::ask_number(
        player_id,
        &Msg::new(key!("eve_unify_title")),
        &Msg::new(key!("eve_unify_ask")),
        0,
        up,
    )?;
    if y < 1 {
        return Ok(());
    }
    let mut y = y;
    let _ = fans::flip_down(player_id, y)?;
    // 「为此次投掷结果增加Yd4」
    let add = ctx::roll(player_id, y, 4);
    let before = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    ctx::trigger::set_move_roll(before + add);
    if ctx::in_group(player_id, "Pastel✽Palettes") {
        for p in 0..ctx::player_count() {
            if y <= 0 {
                break;
            }
            if p == player_id || ctx::player_out(p) || ctx::in_group(p, "Pastel✽Palettes") {
                continue;
            }
            let down = ctx::tok(p, fans::DOWN).min(y);
            if down > 0 {
                let _ = fans::flip_up(p, down)?;
                y -= down;
            }
        }
    } else {
        for p in 0..ctx::player_count() {
            if ctx::player_out(p) || !ctx::in_group(p, "Pastel✽Palettes") {
                continue;
            }
            // Always send -- the effect still hits a seat with 0 face-downs, so
            // band clause （2） may grant an overflow crystal.
            let _ = fans::flip_up(p, 1)?;
        }
    }
    ctx::log(
        player_id,
        &Msg::new(key!("eve_unify_done")).i("n", add as i64),
    );
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_start_kani(player_id: i32) -> card_sdk::Asked {
    at_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_roll_kani(player_id: i32) -> card_sdk::Asked {
    on_roll(player_id)
}
