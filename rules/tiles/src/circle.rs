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
//! `On::Hook(&[HookKind::PassTile], "")` -- [`settle_reward`], the reward
//! choice the walk used to run as `circle_reward` in `play.rs`. `docs/TILES.md`.
//! The engine keeps a built-in copy (`Play::circle_reward`, the walk/teleport
//! `tile_rule_instances(at).is_empty()` fallback) for `StubRules`.
//!
//! Suppression is `prop::NO_REWARD` on *this* instance: a rule that
//! 「无法获取[CiRCLE奖励]」 / 「[经过]CiRCLE时不获得[CiRCLE奖励]」 /
//! 「首次经过CiRCLE不获得经过奖励」 sets the prop and clears it when its own
//! clause ends. The source owns the arming and the disarming; the reader is the
//! tile instance, not a per-player latch. A veto on one of the mover's own
//! field instances is read but **not** consumed (the source holds that one).
//!
//! Preconditions live on the entry, not at the body top (`docs/GUARDS.md` §0):
//! the [经过] arm's residual guard takes the tile match, the exile gate and
//! 「[移动起点]不为CiRCLE」. The `NO_REWARD` check stays in the body because
//! the tile-side arm is **consumed** -- an early-out after a side effect.
//!
//! TODO(规则书): the stunned player's card-only reward is a ruling, not book
//! text (`回合階段&註釋` `C44` blocks the money half; `团技能` `D13` forces the
//! card half).

use card_sdk::abi::{prop, HookKind, TriggerKind, REWARD_CARD, REWARD_MONEY};
use card_sdk::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

/// 「[获得]2000资金」 (专有名词). Matches `game_core::engine::world::CIRCLE_MONEY`.
pub const CIRCLE_MONEY: i32 = 2000;

pub const CIRCLE: CardDef = CardDef::new(
    "tile:circle",
    &[
        On::Settle("", None, settle),
        On::Hook(&[HookKind::PassTile], "", Some(pass_pays), on_pass),
    ],
);

/// The [经过] arm's applicability. Only the tile this instance governs (the
/// dispatch already filters board instances by tile; this is the belt to that
/// brace), and only when the pass actually earns the reward.
///
/// 规则书: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]」 --
/// a move that began on CiRCLE (a same-tile teleport onto CiRCLE, a 0-move
/// that ends there) pays nothing. `start < 0` = no move in flight (a card
/// calling the reward outright): no 移动起点 to compare, so the clause does
/// not bar it.
///
/// An exiled mover earns nothing (回合階段&註釋: [除外] players are out of the
/// income loop). TODO(规则书): the exile bar is engine bookkeeping, not a
/// 「[经过]CiRCLE」 clause.
fn pass_pays(_owner: i32) -> bool {
    let me = ctx::self_tile().unwrap_or(-1);
    if me < 0 || trigger::tile() != me {
        return false;
    }
    let mover = trigger::player_id();
    if mover < 0 {
        return false;
    }
    if ctx::exile_of(mover) > 0 {
        return false;
    }
    let start = trigger::move_from();
    if start >= 0 && ctx::is_circle(start) && ctx::is_circle(me) {
        return false;
    }
    true
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
/// the whole reward step (suppression, the choice, the `circleAffected`
/// window, the payout). Also the body `event:协助CiRCLE重建` re-homes onto
/// 「CiRCLE咖啡厅」 (「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅」).
fn on_pass(_owner: i32) -> card_sdk::Asked {
    // The reward goes to the player who is passing, not to the owner of this
    // board-owned instance (that is `BOARD_OWNER`).
    let mover = trigger::player_id();
    if mover < 0 {
        return Ok(());
    }
    // 「获得」 wording differs for a stop on CiRCLE as against a pass over it.
    let landing = trigger::move_resolve() && trigger::move_remaining() <= 0;
    settle_reward(mover, landing)
}

/// The [CiRCLE奖励] step: 「[获得]2000资金或抽1张卡」, between the pick and the
/// payout. `landing` selects the stop-on-CiRCLE wording (the landing also
/// draws its own hand card from [`settle`]).
pub fn settle_reward(mover: i32, landing: bool) -> card_sdk::Asked {
    // The tile the reward is being paid for -- the mover's square. When
    // `event:协助CiRCLE重建` re-homes the reward onto 「CiRCLE咖啡厅」 this is
    // the cafe, not `self_tile` (the event instance sits on no tile).
    let at = ctx::player_pos(mover);
    // `prop::NO_REWARD` on this tile's rule instance -- consumed here, so a
    // stale arm cannot veto the next player (`event:协助CiRCLE重建` re-arms).
    if at >= 0 && ctx::tile_prop(at, prop::NO_REWARD) > 0 {
        ctx::set_tile_prop(at, prop::NO_REWARD, 0);
        return Ok(());
    }
    // A walk-scoped / per-player veto on one of the mover's own field
    // instances is read but not consumed -- the source disarms it.
    for (uid, _) in ctx::field_instances(mover) {
        if ctx::prop_at(uid, prop::NO_REWARD) > 0 {
            return Ok(());
        }
    }
    let pick = if ctx::stun_of(mover) > 0 {
        // TODO(规则书): card-only while stunned is a ruling (`C44` / `D13`),
        // not a 「[CiRCLE奖励]」 clause.
        ctx::log(
            mover,
            &Msg::new("log.circle_card_only").player_id("who", mover),
        );
        REWARD_CARD
    } else {
        // 「[获得]2000资金或抽1张卡」 -- the wording differs for a stop on
        // CiRCLE (the landing also draws) as against a pass over it.
        let (text, money, card) = if landing {
            (
                "ask.circle.text_landed",
                "ask.circle.money_landed",
                "ask.circle.card_landed",
            )
        } else {
            ("ask.circle.text", "ask.circle.money", "ask.circle.card")
        };
        let pick = ctx::ask_pick(
            mover,
            &Msg::new("ask.circle.title"),
            &Msg::new(text).player_id("who", mover),
            &[
                Msg::new(money).n("money", CIRCLE_MONEY as i64),
                Msg::new(card),
            ],
        )?;
        pick as i32
    };
    // `circleAffected` -- the reward has been picked but not paid out. A
    // field card can rewrite it or cancel it outright and pay something else
    // (Morfonica replaces the money half), so this raises between the pick and
    // the payout: after the pick, because a hook has to see which option was
    // taken; before the payout, because it has to be able to change it.
    //
    // `value` is which option the reward resolved to: 0 = money, 1 = card.
    // The stunned path forces the card, so it raises with `value = 1` --
    // a money-only clause ("when the money option is chosen") checks
    // `value == 0` and correctly does not fire there. Raised on both paths
    // rather than only the choice, so a reward rewrite applies however the
    // option was arrived at.
    if ctx::raise(mover, TriggerKind::CircleAffected, pick) {
        if pick == REWARD_CARD {
            ctx::log(
                mover,
                &Msg::new("log.circle_card").player_id("who", mover),
            );
            ctx::draw(mover, 1)?;
        } else {
            // 「[获得]2000资金」 -- a bank print through the money pipeline
            // (`Pay::new(_, "gain")`), event type `pass`, logged as
            // `log.circle_money`. The 「支付」 scalars do not reach a print.
            ctx::gain_typed(
                mover,
                CIRCLE_MONEY,
                "pass",
                &Msg::new("log.circle_money").player_id("who", mover),
            )?;
        }
    }
    Ok(())
}