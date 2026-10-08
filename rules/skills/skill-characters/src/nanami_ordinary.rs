//! `skill:广町七深:这是很普通的事吧？`
//!
//! 规则书（skill sheet, 广町七深）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）
//! > （2）当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记（当场上
//! > 存活玩家数量大于等于3时：每种角色标记上限1，该技能对被动技能存在3回合CD）
//! > （3）在时机合适时候，你可以使用手中的角色标记，视为使用该角色的技能（尽力补齐
//! > 资源消耗，比如双[火罐]消耗技能和翻转[P✽P粉丝]）。
//!
//! （2） 「该角色的标记」 is a counter named for the character whose skill is
//! running -- which the `skillUsed` trigger carries as the skill's id, so the
//! mark name is `角色标记:<skill id>` (the 「角色标记」 namespace NNM also spends).
//! 「每种角色标记上限1」 is a per-kind cap, and
//! 「该技能对被动技能存在3回合CD」 is a per-kind cooldown.
//!
//! （3） 「视为使用该角色的技能」 is `ctx::play_card(skill_id, player_id)?`, the
//! same invocation haruhikage uses; the skill id is the mark name minus the
//! `角色标记:` prefix. 「尽力补齐资源消耗」 is advice on paying the
//! invoked skill's cost, not a rule -- the sheet's `思路` column is play advice
//! and is not implemented.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// The 「角色标记」 namespace every consumer agrees on (NNM's `角色标记:*` too).
/// A mark's name is `角色标记:<skill id>`, so (3) can hand the id to `play_card`.
const MARK_PREFIX: &str = "角色标记:";

/// The skill id a mark stands for (`角色标记:<skill id>` -> `<skill id>`).
fn mark_skill(name: &str) -> &str {
    name.strip_prefix(MARK_PREFIX).unwrap_or(name)
}

/// The mark name for a skill id.
fn mark_name(id: &str) -> alloc::string::String {
    alloc::format!("{MARK_PREFIX}{id}")
}

/// Per-kind cooldown, keyed `skill.nanamiOrdinary.cd:<skill id>`.
fn cd_key(id: &str) -> alloc::string::String {
    alloc::format!("skill.nanamiOrdinary.cd:{id}")
}

pub const NANAMI_ORDINARY: CardDef = CardDef::new(
    "skill:广町七深:这是很普通的事吧？",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::Pass], card_sdk::pre::MINE, None, on_pass),
        On::Hook(&[HookKind::SkillUsed], "", Some(any), on_skill),
        On::Hook(&[HookKind::TurnEnd], card_sdk::pre::MINE, None, tick),
    ],
)
    .legacy(&[(2, legacy_mine), (4, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn any(_player_id: i32) -> bool {
    true
}

/// 「初始2，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 2, 2);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("nanami_ordinary_gain")))?;
    Ok(())
}

/// （2）「当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记」.
fn on_skill(player_id: i32) -> card_sdk::Asked {
    let src = ctx::trigger::player_id();
    if src == player_id {
        return Ok(());
    }
    let Some(id) = ctx::trigger::cards().into_iter().next() else {
        return Ok(());
    };
    if id.is_empty() {
        return Ok(());
    }
    let cd = cd_key(&id);
    // 「该技能对被动技能存在3回合CD」 -- a per-kind cooldown.
    if state::get(player_id, &cd) > 0 {
        return Ok(());
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Ok(());
    }
    // 「当场上存活玩家数量大于等于3时：每种角色标记上限1」
    let alive = (0..ctx::player_count())
        .filter(|&p| !ctx::player_out(p))
        .count() as i32;
    if alive >= 3 && ctx::tok(player_id, &mark_name(&id)) >= 1 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("nanami_ordinary_title")),
        &Msg::new(key!("nanami_ordinary_ask")),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("nanami_ordinary_spend")))? {
        return Ok(());
    }
    ctx::add_tok(player_id, &mark_name(&id), 1, i32::MAX)?;
    state::set(player_id, &cd, 3);
    ctx::log(
        player_id,
        &Msg::new(key!("nanami_ordinary_mark")).card("card", &id),
    );
    Ok(())
}

/// 「3回合CD」 counts down.
fn tick(player_id: i32) -> card_sdk::Asked {
    for name in ctx::tok_names(player_id, MARK_PREFIX) {
        let cd = cd_key(mark_skill(&name));
        if state::get(player_id, &cd) > 0 {
            state::set(player_id, &cd, state::get(player_id, &cd) - 1);
        }
    }
    Ok(())
}

/// （3） 「你可以使用手中的角色标记，视为使用该角色的技能」.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if ctx::tok_names(player_id, MARK_PREFIX).is_empty() {
        return Some(Msg::new(key!("nanami_ordinary_no_mark")));
    }
    None
}

/// （3）「视为使用该角色的技能」 -- `ctx::play_card` runs that skill's `On::Play`.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let marks = ctx::tok_names(player_id, MARK_PREFIX);
    if marks.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("nanami_ordinary_title")),
        &Msg::new(key!("nanami_ordinary_which")),
        &marks
            .iter()
            .map(|m| Msg::new(key!("nanami_ordinary_option")).card("card", mark_skill(m)))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(mark) = marks.get(pick).cloned() else {
        return Ok(());
    };
    let id = mark_skill(&mark);
    ctx::add_tok(player_id, &mark, -1, i32::MAX)?;
    ctx::play_card(id, player_id)?;
    ctx::log(
        player_id,
        &Msg::new(key!("nanami_ordinary_used")).card("card", id),
    );
    Ok(())
}
