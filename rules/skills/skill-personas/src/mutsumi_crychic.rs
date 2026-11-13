//! `skill:若叶睦（CRYCHIC）:精致的人偶`
//!
//! 规则书（skill sheet, 若叶睦（CRYCHIC））:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
//! > （2）开局时指定一名其他玩家，你的移动阶段开始时，可消耗1火罐，使本次的移动
//! > 掷骰结果为那名玩家上一次的移动掷骰结果；那名玩家破产时，可重新指定一名玩家。
//! > （3）乐队技能为Ave Mujica时，任意时刻手牌大于等于3时进入状态2，严格小于3时
//! > 进入状态1，但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶。
//!
//! （3） is the two-state mechanic the Ave Mujica band skill also uses; the state
//! axis is `skillState`. 「但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶」
//! moves the band skill's crystal add to reshuffles: it rides `Reshuffled` and
//! writes through the shuffle-tagged host op, and the band skill's own
//! third-turn add stands down while this face rides with it.

use bandori_std::ctx::{self, state};
use bandori_std::state_key;
use card_sdk::abi::{roll_source, ActionKind};
use card_sdk::{key, Action, CardDef, Msg, On};

/// The named other player, or -1.
const OTHER: &str = "skill.mutsumiCrychic.other";
/// Their last move face, latched at their `DiceRollAfter`.
const THEIRS: &str = "skill.mutsumiCrychic.theirs";

pub const MUTSUMI_CRYCHIC: CardDef = CardDef::new(
    "skill:若叶睦（CRYCHIC）:精致的人偶",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Action(Action::TurnStartBefore("", None, at_start)),
        On::Action(Action::DeckAtGameStart("", None, at_start)),
        On::Action(Action::Pass(bandori_std::guards::OWN_START, None, on_pass)),
        On::Action(Action::DiceRollAfter("", Some(other), latch)).silent(),
        // （3）「乐队技能为Ave Mujica时，任意时刻手牌大于等于3时进入状态2，严格
        // 小于3时进入状态1」 -- own hand changes only (`actor == owner`); the
        // band check is the residual (band id is outside the CEL vocabulary).
        On::Action(Action::HandChanged(
            "actor == owner",
            Some(mujica),
            sync_state,
        )),
        // 「但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶」 -- the carve-out:
        // the add rides `Reshuffled` and writes through the shuffle-tagged
        // host op, so it is distinguishable from every other crystal gain.
        On::Action(Action::Reshuffled("", Some(mujica_in_two), add_crystal)),
    ],
)
.rule()
// 「（初始1，上限1）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 1),
    (bandori_std::fire::FIRE_CAP, 1),
]);

fn other(player_id: i32) -> bool {
    ctx::trigger::player_id() == state::get(player_id, OTHER)
}

/// 「初始1，上限1」 + （2）「开局时指定一名其他玩家」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    if state::get(player_id, OTHER) >= 0 {
        return Ok(());
    }
    let others: alloc::vec::Vec<i32> = (0..ctx::player_count())
        .filter(|&p| p != player_id && !ctx::player_out(p))
        .collect();
    if others.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_choice(
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
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("mutsumi_crychic_gain")))?;
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
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if bandori_std::fire::get(player_id) < 1 {
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
    if !bandori_std::fire::spend_skill(player_id, 1, &Msg::new(key!("mutsumi_crychic_spend")))? {
        return Ok(());
    }
    ctx::set_fixed_roll(face, roll_source::SKILL);
    ctx::log(
        player_id,
        &Msg::new(key!("mutsumi_crychic_fixed")).i("n", face as i64),
    );
    Ok(())
}

/// The band skill that owns the 状态1 / 状态2 axis (（3）'s gate).
const MUJICA: &str = "skill:Ave Mujica:假面之下的真实";

/// 「乐队技能为Ave Mujica时」 -- an id check, outside the CEL vocabulary.
fn mujica(player_id: i32) -> bool {
    bandori_std::group_skill(player_id).as_deref() == Some(MUJICA)
}

/// `mujica` + 状态2, for the shuffle carve-out.
fn mujica_in_two(player_id: i32) -> bool {
    mujica(player_id) && state::get(player_id, state_key::SKILL_STATE) == 2
}

/// （3）「任意时刻手牌大于等于3时进入状态2，严格小于3时进入状态1」.
///
/// The state axis is the band skill's (`skillState`); this face only drives
/// the edge. Outside an Ave Mujica band the hand size means nothing here.
fn sync_state(player_id: i32) -> card_sdk::Asked {
    if !mujica(player_id) {
        return Ok(());
    }
    let want = if ctx::hand_size(player_id) >= 3 { 2 } else { 1 };
    if state::get(player_id, state_key::SKILL_STATE) != want {
        state::set(player_id, state_key::SKILL_STATE, want);
        ctx::log(
            player_id,
            &Msg::new(key!("mutsumi_crychic_state")).i("n", want as i64),
        );
    }
    Ok(())
}

/// 「只有洗牌时可以为你的乐队技能卡上添加奇迹水晶」 -- one per reshuffle, on
/// the band-skill instance (the same pool `group_crystals` reads).
fn add_crystal(player_id: i32) -> card_sdk::Asked {
    bandori_std::add_group_crystals_shuffled(player_id, 1, i32::MAX);
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_start_kani(player_id: i32) -> card_sdk::Asked {
    at_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn latch_kani(player_id: i32) -> card_sdk::Asked {
    latch(player_id)
}
