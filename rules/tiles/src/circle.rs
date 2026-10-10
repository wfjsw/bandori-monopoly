//! `tile:circle` -- CiRCLE's [结算] and its [经过] reward.
//!
//! 规则书（data/rules.txt, 「基础[结算]规则」, lines 95–96）:
//! > · CiRCLE和江户川乐器店的[结算]是：抽取一张手卡。
//! > 　· [经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]。
//!
//! and 「专有名词」:
//! > · [CiRCLE奖励]：[获得]2000资金或抽1张卡。
//!
//! Two entries. The **landing** half is `On::Settle` -- one draw, the
//! 「[结算]是：抽取一张手卡」 clause. The **pass** half (「[经过]CiRCLE」) is
//! `On::Hook(&[HookKind::PassTile], "")` -- `ctx::settle_circle_reward`, the reward
//! choice the walk used to run as `circle_reward` in `play.rs`. `docs/TILES.md`.
//!
//! Suppression is `prop::NO_REWARD` on *this* instance (`H.CircleReward` reads
//! it): a rule that 「无法获取[CiRCLE奖励]」 / 「[经过]CiRCLE时不获得[CiRCLE奖励]」
//! / 「首次经过CiRCLE不获得经过奖励」 sets the prop and clears it when its own
//! clause ends. The source owns the arming and the disarming; the reader is the
//! tile instance, not a per-player latch.
//!
//! 「[移动起点]不为CiRCLE」 is enforced in the reward step itself
//! (`play.rs::circle_reward`, `H.CircleReward`): the engine hands it the move's
//! 移动起点 and it declines the reward when the move began on CiRCLE. That covers
//! this body's `ctx::settle_circle_reward` call and the walk's built-in fallback
//! alike, so a pass whose 移动起点 is CiRCLE (a same-tile teleport onto CiRCLE)
//! pays nothing.
//!
//! TODO(规则书): the stunned player's card-only reward is a ruling, not book
//! text (`回合階段&註釋` `C44` blocks the money half; `团技能` `D13` forces the
//! card half).

use card_sdk::abi::{prop, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

pub const CIRCLE: CardDef = CardDef::new(
    "tile:circle",
    &[
        On::Settle("", None, settle),
        On::Hook(&[HookKind::PassTile], "", Some(passes_here), on_pass),
    ],
);

/// Only the tile this instance governs (the dispatch already filters board
/// instances by tile; this is the belt to that brace).
fn passes_here(_player_id: i32) -> bool {
    let me = ctx::self_tile().unwrap_or(-1);
    me >= 0 && trigger::tile() == me
}

/// 规则书: 「CiRCLE的[结算]是：抽取一张手卡。」
fn settle(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new("log.land_circle").player_id("who", player_id),
    );
    // 规则书: 「[结算]是：抽取一张手卡」 -- `H.DrawR`.
    ctx::draw(player_id, 1)?;
    Ok(())
}

/// 规则书: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]」 --
/// `H.CircleReward`, the whole reward step (suppression, the choice, the
/// `circleAffected` window, the payout).
fn on_pass(_owner: i32) -> card_sdk::Asked {
    // `prop::NO_REWARD` is read (and consumed) inside the reward step itself,
    // so a suppressing rule arms it on this instance before this entry runs.
    let _ = ctx::prop(prop::NO_REWARD);
    // The reward goes to the player who is passing, not to the owner of this
    // board-owned instance (that is `BOARD_OWNER`).
    let mover = trigger::player_id();
    if mover < 0 {
        return Ok(());
    }
    // 「获得」 wording differs for a stop on CiRCLE as against a pass over it.
    let landing = trigger::move_resolve() && trigger::move_remaining() <= 0;
    ctx::settle_circle_reward(mover, landing)?;
    Ok(())
}