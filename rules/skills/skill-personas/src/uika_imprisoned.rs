//! `skill:三角初华:Imprisoned XII`（Ave Mujica）
//!
//! 规则书（skill sheet, 三角初华）:
//! > 状态1：移动改为10+1d10，每当你经过CiRCLE的回合结束后，若你自从上一次经过CiRCLE
//! > 后未在任何[回忆地块]（小豆岛，武道馆，旧古河庭园）触发结算则可在你的下回合开始
//! > 时进入状态2。若满足前述条件，你可在你经过CiRCLE的回合结束后立刻获得一个新的回合。
//! > 状态2：（火罐上限3）获得[不可阻挡]，并且在此状态下主动移动经过任何玩家都将向
//! > 其收取200资金，每次成功收取后可选择在[触发结算]前额外移动1d6（最多4次）
//!
//! 「状态1 / 状态2」 is the axis the Ave Mujica band skill owns
//! ([`skill_groups::ave_mujica`]); this skill writes it and reads it back.
//!
//! 状态1's 「移动改为10+1d10」 is the plan's dice table. 「未在任何[回忆地块]…触发结算」
//! is a latch a `Settle` onto one of the three named tiles sets, and the entry
//! into 状态2 is gated on it having stayed clear since the last CiRCLE pass.
//!
//! 状态2's 「获得[不可阻挡]」 is the `unstoppable` key the abnormal gate already
//! reads. 「主动移动经过任何玩家都将向其收取200资金」 is `PassPlayer`, and
//! 「额外移动1d6（最多4次）」 is a dice bonus capped at four.

use card_sdk::abi::{state_key, HookKind};
use ruleset_core::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「回忆地块」 -- the three the clause names.
const MEMORY: [&str; 3] = ["小豆岛", "武道馆", "旧古河庭园"];
/// Settled on a memory tile since the last CiRCLE pass.
const DIRTY: &str = "skill.uikaImprisoned.dirty";
/// The same latch under a neutral name, so a *card* that asks the same question
/// (`Mujica:（初华）我，无畏悲伤`) can read it without knowing this skill.
pub const MEMO_DIRTY: &str = "memory.dirty";
/// 1d6 bonuses taken in 状态2 this move. Cap 4.
const BONUS: &str = "skill.uikaImprisoned.bonus";

pub const UIKA_IMPRISONED: CardDef = CardDef::new(
    "skill:三角初华:Imprisoned XII",
    &[
        // 状态1's 「未在任何[回忆地块]…触发结算则可在你的下回合开始时进入状态2」
        // -- the offer exists only while the memory latch stayed clear.
        // 状态2's cap write still has to run whenever state is 2, so the
        // residual is the disjunction. Dynamic latch, residual.
        On::Hook(
            &[HookKind::TurnStartBefore],
            card_sdk::pre::MINE,
            Some(state2_or_clean),
            at_turn_start,
        ),
        // 状态1 「移动改为10+1d10」 -- only while this player is in 状态1.
        On::Hook(
            &[HookKind::RollPlan],
            "actor == owner && slot('skillState') != 2",
            None,
            on_plan,
        ),
        On::Hook(&[HookKind::Pass], ruleset_core::guards::OWN_START, None, on_pass),
        // 「若你自从上一次经过CiRCLE后未在任何[回忆地块]…触发结算」 -- the
        // settle that marks the latch is the owner's own settle onto one of the
        // three named tiles. Both facts are CEL (`actor == owner` and the three
        // `tile_named` ids), so a reject runs no body (and so posts no flash).
        On::Hook(
            &[HookKind::Settle],
            "actor == owner && (tile.id == tile_named('小豆岛') || tile.id == tile_named('武道馆') || tile.id == tile_named('旧古河庭园'))",
            None,
            on_settle,
        ),
        // 状态2 「主动移动经过任何玩家」 -- 行动阶段 12 [经过]
        // (`SETTLE-STAGES.md` §4 M4): the step onto a tile a player stands on,
        // not the end-tile [重叠]. The other player is read off the tile.
        On::Hook(
            &[HookKind::PassTile],
            "actor == owner && move.main && slot('skillState') == 2",
            None,
            on_pass_player,
        ),
        // 状态2 「每次成功收取后可选择在[触发结算]前额外移动1d6」.
        // 「每次成功收取后可选择在[触发结算]前额外移动1d6（最多4次）」 -- the
        // 4-shot cap is `slot('skill.uikaImprisoned.bonus') < 4`.
        On::Hook(
            &[HookKind::SettleBefore],
            "slot('skillState') == 2 && slot('skill.uikaImprisoned.bonus') < 4",
            None,
            before_settle,
        ),
    ],
)
    .silent(&[2, 3]);

/// Residual guard for [`at_turn_start`]: 状态2 (the cap write must run) or
/// the memory latch is clear (the 状态2 offer). Dynamic key.
fn state2_or_clean(player_id: i32) -> bool {
    state::get(player_id, state_key::SKILL_STATE) == 2
        || state::get(player_id, DIRTY) == 0
}

/// 状态1's entry offer, and 状态2's cap.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        // 「（火罐上限3）」
        state::set_bounds(player_id, state_key::CHARGE, 0, 3);
        // 「获得[不可阻挡]」
        state::set(player_id, state_key::UNSTOPPABLE, 1);
        return Ok(());
    }
    // 「可在你的下回合开始时进入状态2」 -- `state2_or_clean` already promised
    // the memory latch is clear.
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("uika_imprisoned_title")),
        &Msg::new(key!("uika_imprisoned_enter")),
    )? {
        return Ok(());
    }
    state::set(player_id, state_key::SKILL_STATE, 2);
    ctx::log(player_id, &Msg::new(key!("uika_imprisoned_two")));
    Ok(())
}

/// 状态1: 「移动改为10+1d10」.
fn on_plan(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「移动改为10+1d10」 -- the d20 becomes a d10 and a flat 10 rides
    // along; the summed roll is the walk's length (`set_steps` would pin the
    // walk to 10 and drop the die).
    plan::set_base_dice(1, 10, "Imprisoned XII");
    plan::add_extra_dice(10, 0, "Imprisoned XII");
    Ok(())
}

/// 「每当你经过CiRCLE的回合结束后…」 -- the pass clears the memory latch.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, DIRTY, 0);
    Ok(())
}

/// 「若你自从上一次经过CiRCLE后未在任何[回忆地块]…触发结算」 -- settling on one
/// sets the latch. `actor == owner` and the three named tiles are the condition.
fn on_settle(player_id: i32) -> card_sdk::Asked {
    debug_assert!(ctx::trigger::player_id() == player_id);
    debug_assert!(MEMORY.iter().any(|&n| ctx::trigger::tile() == ctx::tile_named(n)));
    state::set(player_id, DIRTY, 1);
    state::set(player_id, MEMO_DIRTY, 1);
    Ok(())
}

/// 状态2: 「主动移动经过任何玩家都将向其收取200资金」.
///
/// `SETTLE-STAGES.md` §4 M4: 「主动移动经过任何玩家」 is the mover's step onto
/// a tile a player stands on (行动阶段 12 [经过]). The other player is read off
/// the tile -- a `passTile` payload has no `target`. (It used to sit on
/// `passPlayer` and read `trigger::player_id()` as "the other", which is the
/// mover -- the guard already pinned that to `player_id`, so the body bailed
/// and the charge never fired.)
fn on_pass_player(player_id: i32) -> card_sdk::Asked {
    let at = ctx::trigger::tile();
    for other in ctx::players_on(at, player_id) {
        ctx::transfer(
            other,
            player_id,
            200,
            &Msg::new(key!("uika_imprisoned_fee")),
        )?;
        ctx::log(
            player_id,
            &Msg::new(key!("uika_imprisoned_charged")).player_id("who", other),
        );
    }
    Ok(())
}

/// 状态2: 「每次成功收取后可选择在[触发结算]前额外移动1d6（最多4次）」.
fn before_settle(player_id: i32) -> card_sdk::Asked {
    // `slot('skill.uikaImprisoned.bonus') < 4` is the pre.
    debug_assert!(state::get(player_id, BONUS) < 4);
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("uika_imprisoned_title")),
        &Msg::new(key!("uika_imprisoned_extra")),
    )? {
        return Ok(());
    }
    let d = ctx::roll(player_id, 1, 6).max(0);
    state::set(player_id, BONUS, state::get(player_id, BONUS) + 1);
    ctx::plan::set_extra_steps(d);
    ctx::log(
        player_id,
        &Msg::new(key!("uika_imprisoned_moved")).i("n", d as i64),
    );
    Ok(())
}

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: ruleset_core::roster::PersonaDef =
ruleset_core::roster::PersonaDef::new(r#"三角初华"#)
    .display(r#"三角初华"#)
    .group(r#"Ave Mujica"#)
    .color(r#"#BB9955"#)
    .skill(r#"Imprisoned XII"#)
    .text(r#"状态1：移动改为10+1d10，每当你经过CiRCLE的回合结束后，若你自从上一次经过CiRCLE后未在任何[回忆地块]（小豆岛，武道馆，旧古河庭园）触发结算则可在你的下回合开始时进入状态2。若满足前述条件，你可在你经过CiRCLE的回合结束后立刻获得一个新的回合。
状态2：（火罐上限3）获得[不可阻挡]，并且在此状态下主动移动经过任何玩家都将向其收取200资金，每次成功收取后可选择在[触发结算]前额外移动1d6（最多4次）"#)
    .cn_id("")
    .art(r#"337"#)
    .costume(r#"337_event_297_story_01"#)
    .exclusive_cards(&[r#"我，无畏悲伤"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_player_kani(player_id: i32) -> card_sdk::Asked {
    on_pass_player(player_id)
}
