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
//! （2） is **two-part** and must run 「投掷前」 -- before the dice land:
//!
//! 1. **[`ActionKind::DiceRollBefore`] (Action)** -- the `ask_number` for Y, the
//!    self `fans::flip_down(Y)`, the other-seat `fans::flip_up` loop, and the
//!    log. Prompts are allowed here; this is the only window that is actually
//!    before the roll.
//! 2. **[`FilterKind::DiceRollPlan`] (FilterValue)** -- pure: reads the Y the
//!    Before action stashed in per-player state and appends `Y`d4 to the move's
//!    dice table (`add_base_count` / `add_base_sides`, same shape as
//!    [`super::kanon_lost`]'s 「1d20+1d4」). The host folds that into the plan
//!    before `roll_tables`, so 「为此次投掷结果增加Yd4」 is *part of* the roll
//!    result (not a post-hoc `set_move_roll` patch).
//!
//! Raise order at the main-move site (`play.rs` `main_move`):
//! `diceRollBefore` → `diceRollPlan` → `roll_tables` → `diceRollValue` →
//! `diceRollAfter`. The plan is re-seeded from the move *after* Before, so a
//! `plan::add_base_dice` call inside Before would be wiped; Y is therefore
//! stashed in state at Before and read by the pure Plan filter.
//! A silent `diceRollAfter` Action clears the stash so a later roll cannot
//! inherit it; Before always overwrites (including `Y = 0` on decline).
//! The Plan filter's *condition* is `actor == owner` (not `slot(…) >= 1`):
//! a `slot()` condition on FilterValue at DiceRollPlan did not observe a
//! stash written earlier in the same roll, while `state::get` in the body
//! does.
//!
//! TODO(规则书): 「因任意原因投掷前」 also covers card-driven rolls, but the
//! card_move site (`play.rs` ~743) never raises `diceRollBefore` (only
//! `diceRollPlan` / `diceRollValue` / `diceRollAfter` / `diceRollResult`), so
//! this skill cannot prompt before those dice. `DiceRollPlan` is Filter-only
//! (no prompts). Fixing that needs a `diceRollBefore` raise on the card_move
//! path.

use card_sdk::abi::{ActionKind, Dice, FilterKind};
use bandori_std::ctx::{self, state};
use card_sdk::{Action, CardDef, Filter, key, Msg, On, PlanValue};
use skill_groups::fans;

/// Y chosen at [`ActionKind::DiceRollBefore`], consumed by the Plan filter.
/// One-shot per roll: Before always writes it (0 on decline); a silent
/// After action clears it.
const PENDING_Y: &str = "evePendingY";

pub const EVE_UNIFY: CardDef = CardDef::new(
    "skill:若宫伊芙:天下统一",
    &[
        On::Action(Action::DeckAtGameStart("group_is(owner, 'Pastel✽Palettes')", "", None, at_start)),
        // （2） 「投掷前」 -- ask + flips. Action: prompts allowed.
        On::Action(Action::DiceRollBefore("actor == owner && slot('ppFansUp') >= 1", "", Some(roll_ok), on_before)),
        // （2） 「为此次投掷结果增加Yd4」 -- pure dice-table append.
        On::Filter(Filter::DiceRollPlan("actor == owner", None, on_plan)),
        // Bookkeeping: drop the one-shot Y so a later roll cannot inherit it.
        On::Action(Action::DiceRollAfter("actor == owner", "", None, clear_pending)),
    ],
)
// 2 (plan shape) and 3 (stash clear) are bookkeeping; 1 is the visible clause.
.silent(&[2, 3]);

/// Residual guard for （2） -- `skill_blocked` (band-specific) and `fixed_roll`
/// (derived plan state) stay here (not yet in the condition vocabulary).
/// `actor == owner && slot('ppFansUp') >= 1` is the pre.
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

/// （2） at 「投掷前」: ask Y, flip fans now, stash Y for the Plan filter.
/// The Yd4 itself is *not* rolled here -- it joins the move's dice table via
/// [`on_plan`] so the sum is part of the roll result.
fn on_before(player_id: i32) -> card_sdk::Asked {
    // Always write the stash (including 0) so a decline cannot inherit a
    // previous roll's Y.
    state::set(player_id, PENDING_Y, 0);
    let up = fans::up(player_id);
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
    let chosen = y;
    let mut left = y;
    let _ = fans::flip_down(player_id, chosen)?;
    // Stash for [`on_plan`]; the Plan filter is pure and cannot `state::set`.
    state::set(player_id, PENDING_Y, chosen);
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
    // `n` is the chosen Y (the dice count), not the Yd4 sum -- the dice have
    // not landed yet.
    ctx::log(
        player_id,
        &Msg::new(key!("eve_unify_done")).i("n", chosen as i64),
    );
    Ok(())
}

/// （2） 「为此次投掷结果增加Yd4」 -- pure PlanValue at [`FilterKind::DiceRollPlan`].
/// Appends `Y`d4 to the move's dice table (`add_base`), so `roll_tables`
/// includes it in the face. Reads the stash [`on_before`] wrote; never writes.
fn on_plan(player_id: i32, mut v: PlanValue) -> PlanValue {
    // NOTE: `slot('evePendingY')` as a *condition* on this FilterValue did not
    // see the stash `on_before` writes earlier in the same roll (the Action
    // `slot` conditions do). Gate on `actor == owner` and read the stash here
    // -- `state::get` is legal in filters and does see it.
    let y = state::get(player_id, PENDING_Y);
    if y >= 1 {
        v.add_base = Some(Dice { count: y, sides: 4 });
    }
    v
}

/// Drop the one-shot Y after the roll (including its `diceRollValue` reshape).
fn clear_pending(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, PENDING_Y, 0);
    Ok(())
}

//

//

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

#[cfg(kani)]
#[inline(always)]
pub fn on_plan_kani(player_id: i32, v: card_sdk::PlanValue) -> card_sdk::PlanValue {
    on_plan(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_pending_kani(player_id: i32) -> card_sdk::Asked {
    clear_pending(player_id)
}
