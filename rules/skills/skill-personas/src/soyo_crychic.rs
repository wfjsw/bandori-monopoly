//! `skill:长崎素世（CRYCHIC）:雨中祈晴`
//!
//! 规则书（skill sheet, 长崎素世（CRYCHIC））:
//! > 当其他玩家在属于你的格子上触发结算时，你可取消那次支付，使你下回合的主要移动
//! > 变为传送至触发此技能的那格。
//!
//! One clause, two halves: the pay is voided, and the next main move is a
//! teleport back to that tile. 「取消那次支付」 is the pay chain's own cancel
//! (`set_pay_amount(0)`); 「下回合的主要移动变为传送至…」 is an armed one-shot
//! the next `DiceRollPlan` consumes -- the same shape as detour's 「下一次的移动掷骰
//! 变更为1d6」.
//!
//! The latch is a presence flag plus a tile memory (the detour / soyo
//! idiom), not a signed tile: an unset key reads 0, and tile 0 is a real tile,
//! so a bare tile index cannot say "nothing owed" without a sentinel. The flag
//! gates the read and the CEL `slot('skill.soyoCrychic.armed')` condition, so
//! the residual guard is gone.

use bandori_std::ctx::{self, state};
use card_sdk::abi::MoveKind;
use card_sdk::{key, Action, CardDef, Msg, On};

/// The teleport is armed (0/1). Presence only -- the tile to teleport to rides
/// in [`TILE`], so no value doubles as "nothing owed".
const ARMED: &str = "skill.soyoCrychic.armed";
/// The tile 「触发此技能的那格」 to teleport back to. Only read while [`ARMED`]
/// is set; tile 0 is a real tile, not "nothing".
const TILE: &str = "skill.soyoCrychic.tile";
/// Set by a **borrower** (`CRYCHIC:春日影`) that has no own soyo row to answer
/// the armed latch for; an owning seat leaves it 0 and its own entries consume
/// the arm. The same stand-down key as `skill.tomoriPoem.borrowed` — the two
/// cards keep the spelling in sync (card-crychic does not depend on this crate).
const BORROWED: &str = "skill.soyoCrychic.borrowed";

pub const SOYO_CRYCHIC: CardDef = CardDef::new(
    "skill:长崎素世（CRYCHIC）:雨中祈晴",
    &[
        // 「当其他玩家在属于你的格子上触发结算时，你可取消那次支付」 -- the
        // body-top applicability checks are the condition now: another player
        // (`actor != owner`, not `MINE` -- the old `mine` guard was inverted
        // against the body and the rulebook), a tile we own, and a positive
        // amount. The opt-in ask stays in the body (a player choice).
        //
        // Rides `PaySplitChooseBefore` (v78), restoring the rulebook PaySplitChoose position:
        // the interactive cancel (`ask_yes` + `set_pay_amount(0)` + state writes)
        // sits immediately before the pure `PaySplitChoose` filter stage.
        On::Action(Action::PaySplitChooseBefore(
            "actor != owner && tile.owner == owner && value > 0",
            None,
            on_pay,
        )),
        // 「下回合的主要移动变为传送至…那格」 -- the owner's own plan consumes
        // the armed one-shot (`actor == owner` on `DiceRollPlan`). The latch is a
        // CEL `slot` presence flag (`skill.soyoCrychic.armed` is in SLOT_NAMES),
        // so the condition alone decides and no residual guard remains.
        // An `Action` (not a `Filter`) so a borrower can run it through
        // `invoke_entry`: only Action bodies are invocable. The `borrowed`
        // stand-down keeps this seat's own entry from double-consuming an arm
        // the borrower opened.
        On::Action(Action::DiceRollPlan(
            "actor == owner && slot('skill.soyoCrychic.armed') != 0 \
             && slot('skill.soyoCrychic.borrowed') == 0",
            None,
            on_plan,
        ))
        .id("plan"),
        // One-shot latch clear once the rewritten move has been taken. This is
        // a teleport plan, so `DiceRollAfter` never fires (no dice) -- `MoveAfter`
        // is the next event for both teleports and walks.
        On::Action(Action::MoveAfter(
            "actor == owner && slot('skill.soyoCrychic.armed') != 0 \
             && slot('skill.soyoCrychic.borrowed') == 0",
            None,
            on_plan_after,
        ))
        .id("after"),
    ],
)
.rule();

/// 「当其他玩家在属于你的格子上触发结算时，你可取消那次支付」.
/// Applicability is the entry's condition (`actor != owner && tile.owner ==
/// owner && value > 0`); the ask is the 「可」 opt-in.
/// Rides `PaySplitChooseBefore` (v78): the prompt and the `set_pay_amount(0)` cancel
/// sit at the rulebook PaySplitChoose position.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("soyo_crychic_title")),
        &Msg::new(key!("soyo_crychic_ask")).tile("tile", t),
    )? {
        return Ok(());
    }
    // 「取消那次支付」
    ctx::trigger::set_pay_amount(0);
    // 「使你下回合的主要移动变为传送至触发此技能的那格」
    state::set(player_id, TILE, t);
    state::set(player_id, ARMED, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("soyo_crychic_cancelled")).tile("tile", t),
    );
    Ok(())
}

/// 「下回合的主要移动变为传送至…那格」 -- consumed by the next plan. The latch
/// is the entry's condition (`slot('skill.soyoCrychic.armed') != 0`). The plan
/// is rewritten through `ctx::plan`, so a borrower's `invoke_entry` lands on
/// the same plan the owner's own entry would. The one-shot latch clear is
/// [`on_plan_after`] on `MoveAfter` (teleports skip `DiceRollAfter`).
fn on_plan(player_id: i32) -> card_sdk::Asked {
    let t = state::get(player_id, TILE);
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(t);
    ctx::plan::set_settles(true);
    ctx::log(
        player_id,
        &Msg::new(key!("soyo_crychic_moved")).tile("tile", t),
    );
    Ok(())
}

/// Drop the one-shot [`ARMED`] latch (and the borrower's [`BORROWED`] stand-down)
/// once the teleport move has been taken.
fn on_plan_after(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, ARMED, 0);
    state::set(player_id, BORROWED, 0);
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn on_pay_kani(player_id: i32) -> card_sdk::Asked {
    on_pay(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_plan_kani(player_id: i32) -> card_sdk::Asked {
    on_plan(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_plan_after_kani(player_id: i32) -> card_sdk::Asked {
    on_plan_after(player_id)
}
