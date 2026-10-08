//! `skill:椎名立希（CRYCHIC）:克服劣等感`
//!
//! 规则书（skill sheet, 椎名立希（CRYCHIC））:
//! > 每当你的移动掷骰结果严格小于你以外玩家中上一次进行的移动掷骰结果且那次移动掷骰
//! > 结果不大于20，若你本回合未使用技能，你下回合可在移动前重骰一次并选择第二次的
//! > 结果使用。
//!
//! Three latches carry it:
//!
//! - 「你以外玩家中上一次进行的移动掷骰结果」 is the last move-roll anyone else
//!   made. The `rollAfter` hook sees every player's roll, so this player's state
//!   holds the last one that was not theirs.
//! - 「若你本回合未使用技能」 is a per-turn flag the `skillUsed` trigger sets.
//! - 「你下回合可在移动前重骰一次」 arms *next* turn; the offer fires at this
//!   player's own `rollAfter`, which is before the walk runs, and the choice is
//!   `trigger::set_move_roll` between the two totals.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「你以外玩家中上一次进行的移动掷骰结果」.
const LAST_OTHER: &str = "skill.takiCrychic.lastOther";
/// Set when the comparison lands; consumed at the next turn start.
const ARMED: &str = "skill.takiCrychic.armed";
/// 「若你本回合未使用技能」.
const USED: &str = "skill.takiCrychic.used";

pub const TAKI_CRYCHIC: CardDef = CardDef::new(
    "skill:椎名立希（CRYCHIC）:克服劣等感",
    &[
        On::Hook(&[HookKind::TurnStartBefore], card_sdk::pre::MINE, None, at_turn_start),
        On::Hook(&[HookKind::RollAfter], "", None, on_roll),
        On::Hook(&[HookKind::SkillUsed], card_sdk::pre::MINE, None, on_skill_used),
    ],
)
    .legacy(&[(0, legacy_mine), (2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, USED, 0);
    // 「你下回合可在移动前重骰一次」 -- the arm from last turn becomes this
    // turn's offer.
    if state::get(player_id, ARMED) != 0 {
        state::set(player_id, ARMED, 0);
        state::set(player_id, "skill.takiCrychic.reroll", 1);
    }
    Ok(())
}

fn on_skill_used(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, USED, 1);
    Ok(())
}

fn on_roll(player_id: i32) -> card_sdk::Asked {
    let roller = ctx::trigger::player_id();
    let roll = ctx::trigger::move_roll().unwrap_or(0);
    if roller != player_id {
        // 「你以外玩家中上一次进行的移动掷骰结果」 -- just remember it.
        state::set(player_id, LAST_OTHER, roll);
        return Ok(());
    }
    // The offer for *this* turn's move, before the walk runs.
    if state::get(player_id, "skill.takiCrychic.reroll") != 0 {
        state::set(player_id, "skill.takiCrychic.reroll", 0);
        offer_reroll(player_id, roll)?;
        return Ok(());
    }
    // 「严格小于…且那次移动掷骰结果不大于20」
    let other = state::get(player_id, LAST_OTHER);
    if other <= 0 || other > 20 {
        return Ok(());
    }
    if roll >= other {
        return Ok(());
    }
    // 「若你本回合未使用技能」
    if state::get(player_id, USED) != 0 {
        return Ok(());
    }
    state::set(player_id, ARMED, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("taki_crychic_armed")).i("n", other as i64),
    );
    Ok(())
}

/// 「重骰一次并选择第二次的结果使用」.
fn offer_reroll(player_id: i32, first: i32) -> card_sdk::Asked {
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("taki_crychic_title")),
        &Msg::new(key!("taki_crychic_ask")).i("n", first as i64),
    )? {
        return Ok(());
    }
    // Re-roll the same shape the move rolled, and let the player pick.
    let second = ctx::do_move_roll(player_id);
    let keep = ctx::ask_yes(
        player_id,
        &Msg::new(key!("taki_crychic_title")),
        &Msg::new(key!("taki_crychic_pick"))
            .i("a", first as i64)
            .i("b", second as i64),
    )?;
    let chosen = if keep { first } else { second };
    ctx::trigger::set_move_roll(chosen);
    ctx::log(
        player_id,
        &Msg::new(key!("taki_crychic_kept")).i("n", chosen as i64),
    );
    Ok(())
}
