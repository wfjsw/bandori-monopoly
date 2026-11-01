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
//! [`super::aya`]).
//!
//! （2） is one **[`ActionKind::DiceRollBefore`] (Action)** body: the
//! `ask_number` for Y, the self `fans::flip_down(Y)`, the other-seat
//! `fans::flip_up` loop, the `Y`d4 throw and the log.
//!
//! Raise order per cast (G03 §5.2): `diceRollCast` → `diceRollBefore` →
//! `diceRollAdd` → `diceRollValue` → … The window is named for the
//! settlement, not the throw, so the d20 has already landed when this body
//! runs -- but `diceRollAdd` / `diceRollValue` have not, and a
//! `trigger::set_move_roll` write from the window reaches the face before
//! them (the same shape Y.O.L.O uses). The `Y`d4 is therefore thrown here
//! (`ctx::roll_direct`, skill-sourced) and added onto the running face, so
//! 「为此次投掷结果增加Yd4」 is part of *this* roll's face rather than a
//! separate transaction. A `diceRollPlan` filter cannot carry it any more:
//! Plan runs before the cast, and this window is the only place that can
//! prompt.
//!
//! TODO(规则书): 「因任意原因投掷前」 also covers card-driven rolls, but the
//! run_planned_move site (`play.rs` ~743) never raises `diceRollBefore` (only
//! `diceRollPlan` / `diceRollValue` / `diceRollAfter` / `diceRollResult`), so
//! this skill cannot prompt before those dice. `DiceRollPlan` is Filter-only
//! (no prompts). Fixing that needs a `diceRollBefore` raise on the run_planned_move
//! path.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{roll_source, ActionKind, RollPurpose};
use card_sdk::{key, Action, CardDef, Msg, On};
use skill_groups::fans;

pub const EVE: CardDef = CardDef::new(
    "skill:若宫伊芙:天下统一",
    &[
        On::Action(Action::DeckAtGameStart(
            "group_is(owner, 'Pastel✽Palettes')",
            None,
            at_start,
        )),
        // （2） 「投掷前」 -- ask + flips. Action: prompts allowed.
        On::Action(Action::DiceRollBefore(
            "actor == owner && slot('ppFansUp') >= 1",
            Some(roll_ok),
            on_before,
        )),
    ],
);
/// Residual guard for （2） -- `skill_blocked` (band-specific) and `fixed_roll`
/// (derived plan state) stay here (not yet in the condition vocabulary).
/// `actor == owner && slot('ppFansUp') >= 1` is the condition.
fn roll_ok(player_id: i32) -> bool {
    if bandori_std::ctx::skill_blocked(player_id, "Pastel✽Palettes") {
        return false;
    }
    ctx::fixed_roll().is_none()
}

/// （1）「游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得若宫伊芙的
/// （2）技能」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    // （1） belongs to the skill's own Pastel✽Palettes character, not to
    // the grantees of the (2) below -- the `group_is(owner, …)` condition owns that.
    let _ = fans::grant(player_id, 5, 0)?;
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) || ctx::in_group(p, "Pastel✽Palettes") {
            continue;
        }
        ctx::place_card(p, "skill:若宫伊芙:天下统一", &Msg::new(key!("eve_granted")));
    }
    Ok(())
}

/// （2） at the move's `diceRollBefore`: ask Y, flip fans, and add `Y`d4 onto
/// this cast's face. A declined ask (Y = 0) adds nothing, so a later roll
/// cannot inherit a stale boost.
fn on_before(player_id: i32) -> card_sdk::Asked {
    let up = fans::up(player_id);
    let y = ctx::ask_number(
        player_id,
        &Msg::new(key!("eve_title")),
        &Msg::new(key!("eve_ask")),
        0,
        up,
    )?;
    if y < 1 {
        return Ok(());
    }
    let chosen = y;
    let mut left = y;
    let _ = fans::flip_down(player_id, chosen)?;
    if ctx::in_group(player_id, "Pastel✽Palettes") {
        for p in 0..ctx::player_count() {
            if left <= 0 {
                break;
            }
            if p == player_id || ctx::player_out(p) || ctx::in_group(p, "Pastel✽Palettes") {
                continue;
            }
            let down = fans::down(p).min(left);
            if down > 0 {
                let _ = fans::flip_up(p, down)?;
                left -= down;
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
    // 「为此次投掷结果增加Yd4」 -- the dice are a real throw (skill source): its
    // `diceRollCast` pays per-cast listeners, and the sum is added onto the
    // face the window carries. `n` is the chosen Y; `add` the rolled sum.
    let add = ctx::roll_direct(player_id, chosen, 4, RollPurpose::Effect, roll_source::SKILL)?.max(0);
    let face = trigger::move_roll().unwrap_or_else(|| trigger::value());
    trigger::set_move_roll(face + add);
    ctx::log(
        player_id,
        &Msg::new(key!("eve_done"))
            .i("n", chosen as i64)
            .i("add", add as i64),
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
pub fn on_before_kani(player_id: i32) -> card_sdk::Asked {
    on_before(player_id)
}
