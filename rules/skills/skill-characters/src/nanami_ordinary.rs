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
//! mark name is derived from it. 「每种角色标记上限1」 is a per-kind cap, and
//! 「该技能对被动技能存在3回合CD」 is a per-kind cooldown.
//!
//! （3） 「视为使用该角色的技能」 is `ctx::play_card(skill_id, player_id)?`, the
//! same invocation haruhikage uses. 「尽力补齐资源消耗」 is advice on paying the
//! invoked skill's cost, not a rule -- the sheet's `思路` column is play advice
//! and is not implemented.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// Per-kind cooldown, keyed `skill.nanamiOrdinary.cd:<skill id>`.
fn cd_key(id: &str) -> alloc::string::String {
    alloc::format!("skill.nanamiOrdinary.cd:{id}")
}

pub const NANAMI_ORDINARY: CardDef = CardDef::new("skill:广町七深:这是很普通的事吧？", &[
    On::Play(Some(can_use), use_skill),
    On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
    On::Hook(&[HookKind::Pass], mine, on_pass),
    On::Hook(&[HookKind::SkillUsed], any, on_skill),
    On::Hook(&[HookKind::TurnEnd], mine, tick),
]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn any(_player_id: i32) -> bool {
    true
}

/// 「初始2，上限2」.
fn declare_cap(player_id: i32) {
    state::set_bounds(player_id, state_key::FIRE, 0, 2);
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return;
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("nanami_ordinary_gain")));
}

/// （2）「当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记」.
fn on_skill(player_id: i32) {
    let src = ctx::trigger::player_id();
    if src == player_id {
        return;
    }
    let Some(id) = ctx::trigger::cards().into_iter().next() else { return; };
    if id.is_empty() {
        return;
    }
    let cd = cd_key(&id);
    // 「该技能对被动技能存在3回合CD」 -- a per-kind cooldown.
    if state::get(player_id, &cd) > 0 {
        return;
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return;
    }
    // 「当场上存活玩家数量大于等于3时：每种角色标记上限1」
    let alive = (0..ctx::player_count()).filter(|&p| !ctx::player_out(p)).count() as i32;
    if alive >= 3 && ctx::tok(player_id, &id) >= 1 {
        return;
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("nanami_ordinary_title")),
        &Msg::new(key!("nanami_ordinary_ask")),
    ) {
        return;
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("nanami_ordinary_spend"))) {
        return;
    }
    ctx::add_tok(player_id, &id, 1, i32::MAX);
    state::set(player_id, &cd, 3);
    ctx::log(player_id, &Msg::new(key!("nanami_ordinary_mark")).card("card", &id));
}

/// 「3回合CD」 counts down.
fn tick(player_id: i32) {
    for id in ctx::tok_names(player_id, "skill:") {
        let cd = cd_key(&id);
        if state::get(player_id, &cd) > 0 {
            state::set(player_id, &cd, state::get(player_id, &cd) - 1);
        }
    }
}

/// （3） 「你可以使用手中的角色标记，视为使用该角色的技能」.
fn can_use(player_id: i32) -> Option<Msg> {
    if ctx::tok_names(player_id, "skill:").is_empty() {
        return Some(Msg::new(key!("nanami_ordinary_no_mark")));
    }
    None
}

/// （3）「视为使用该角色的技能」 -- `ctx::play_card` runs that skill's `On::Play`.
fn use_skill(player_id: i32) {
    let marks = ctx::tok_names(player_id, "skill:");
    if marks.is_empty() {
        return;
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("nanami_ordinary_title")),
        &Msg::new(key!("nanami_ordinary_which")),
        &marks
            .iter()
            .map(|m| Msg::new(key!("nanami_ordinary_option")).card("card", m))
            .collect::<alloc::vec::Vec<_>>(),
    );
    let Some(id) = marks.get(pick).cloned() else { return; };
    ctx::add_tok(player_id, &id, -1, i32::MAX);
    ctx::play_card(&id, player_id);
    ctx::log(player_id, &Msg::new(key!("nanami_ordinary_used")).card("card", &id));
}