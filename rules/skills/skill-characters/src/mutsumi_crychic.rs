//! `skill:若叶睦（CRYCHIC）:精致的人偶`
//!
//! 规则书（skill sheet, 若叶睦（CRYCHIC））:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
//! > （2）开局时指定一名其他玩家，你的移动阶段开始时，可消耗1火罐，使本次的移动
//! > 掷骰结果为那名玩家上一次的移动掷骰结果；那名玩家破产时，可重新指定一名玩家。
//! > （3）乐队技能为Ave Mujica时，任意时刻手牌大于等于3时进入状态2，严格小于3时
//! > 进入状态1，但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶。
//!
//! （2） 「那名玩家上一次的移动掷骰结果」 is the turn-roll history (`_turnCtx.Rolls`)
//! read for that player's last face -- which the history carries as a flat list,
//! so the read is "the last roll recorded while they were the mover".
//!
//! （3） is the two-state mechanic the Ave Mujica band skill also uses; the state
//! axis is `skillState`, and 「只有洗牌时可以为你的乐队技能卡上添加奇迹水晶」
//! carves a class of crystal gains the engine does not tag.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// The named other player, or -1.
const OTHER: &str = "skill.mutsumiCrychic.other";
/// Their last move face, latched at their `RollAfter`.
const THEIRS: &str = "skill.mutsumiCrychic.theirs";

pub const MUTSUMI_CRYCHIC: CardDef = CardDef::new(
    "skill:若叶睦（CRYCHIC）:精致的人偶",
    &[
        On::Play(Some(can_use), use_skill, ""),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], None, at_start, ""),
        On::Hook(&[HookKind::Pass], None, on_pass, card_sdk::pre::MINE),
        On::Hook(&[HookKind::RollAfter], Some(other), latch, ""),
    ],
)
    .legacy(&[(2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn other(player_id: i32) -> bool {
    ctx::trigger::player_id() == state::get(player_id, OTHER)
}

/// 「初始1，上限1」 + （2）「开局时指定一名其他玩家」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 1);
    if state::get(player_id, OTHER) >= 0 {
        return Ok(());
    }
    let others: alloc::vec::Vec<i32> = (0..ctx::player_count())
        .filter(|&p| p != player_id && !ctx::player_out(p))
        .collect();
    if others.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("mutsumi_crychic_title")),
        &Msg::new(key!("mutsumi_crychic_ask")),
        &others
            .iter()
            .map(|&p| Msg::new(key!("mutsumi_crychic_option")).player_id("who", p))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    if let Some(&who) = others.get(pick) {
        state::set(player_id, OTHER, who);
        ctx::log(
            player_id,
            &Msg::new(key!("mutsumi_crychic_named")).player_id("who", who),
        );
    }
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("mutsumi_crychic_gain")))?;
    Ok(())
}

/// 「那名玩家上一次的移动掷骰结果」 -- latched as they roll.
fn latch(player_id: i32) -> card_sdk::Asked {
    let face = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    state::set(player_id, THEIRS, face);
    Ok(())
}

/// （2） 「你的移动阶段开始时，可消耗1火罐」.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("mutsumi_crychic_no_fire")));
    }
    if state::get(player_id, THEIRS) < 0 {
        return Some(Msg::new(key!("mutsumi_crychic_no_face")));
    }
    None
}

/// （2）「使本次的移动掷骰结果为那名玩家上一次的移动掷骰结果」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let face = state::get(player_id, THEIRS);
    if face < 0 {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("mutsumi_crychic_spend")))? {
        return Ok(());
    }
    ctx::set_fixed_roll(face);
    ctx::log(
        player_id,
        &Msg::new(key!("mutsumi_crychic_fixed")).i("n", face as i64),
    );
    Ok(())
}

// （3）「乐队技能为Ave Mujica时，任意时刻手牌大于等于3时进入状态2，严格小于3时
// 进入状态1」 -- the two-state mechanic the Ave Mujica band skill owns. This
// skill only restates the hand-size trigger; the state axis and the 「只有洗牌时
// 可以为你的乐队技能卡上添加奇迹水晶」 carve-out live there.
// TODO(规则书)[judgement]: （3） -- the clause under-specifies -- the state axis
//   is shared with the band skill and 「只有洗牌时可以…添加奇迹水晶」 carves a
//   class of crystal gains the engine does not tag.
