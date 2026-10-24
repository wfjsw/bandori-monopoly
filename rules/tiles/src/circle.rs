//! `tile:circle` -- the start square's [结算] and its [经过] reward.
//!
//! 规则书（data/rules.txt, 「基础[结算]规则」, lines 95–96）:
//! > · the start square和江户川乐器店的[结算]是：抽取一张手卡。
//! > 　· [经过]CiRCLE且[移动起点]不为CiRCLE时获得[pass-reward]。
//!
//! and 「专有名词」:
//! > · [pass-reward]：[获得]2000资金或抽1张卡。
//!
//! Two entries. The **landing** half is `On::Settle` -- one draw, the
//! 「[结算]是：抽取一张手卡」 clause. The **pass** half (「[经过]CiRCLE」) is
//! `On::Action(&[ActionKind::PassTile], "")` -- [`settle_reward`], the reward
//! choice. A third, `On::Message`, is the cross-card face of that same reward
//! body: `event:协助CiRCLE重建` re-homes 「CiRCLE原本的所有效果」 onto
//! 「CiRCLE咖啡厅」 and pays the reward by messaging this rule rather than
//! reaching into its Rust. `docs/TILES.md`.
//!
//! Suppression is `prop::NO_REWARD` on *this* instance: a rule that
//! 「无法获取[pass-reward]」 / 「[经过]CiRCLE时不获得[pass-reward]」 /
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
use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{prop, text_slot, ActionKind, TriggerKind, REWARD_CARD, REWARD_MONEY};
use card_sdk::{Action, CardDef, Msg, On};

/// 「[获得]2000资金」 (专有名词). Matches `game_core::engine::world::CIRCLE_MONEY`.
pub const CIRCLE_MONEY: i32 = 2000;

pub const CIRCLE: CardDef = CardDef::new(
    "tile:circle",
    &[
        On::Settle("", None, settle),
        On::Action(Action::PassTile("", Some(pass_pays), on_pass)),
        // The cross-card face of the same reward body. No `pass_pays` here:
        // that residual is the [经过] arm's applicability (tile match, exile
        // gate, 「[移动起点]不为CiRCLE」). A message is a card asking for the
        // reward outright, and [`settle_reward`] carries its own suppression.
        On::Message(&["settleReward"], "", None, on_message_settle_reward),
    ],
)
.board_role("tile")
// The start square: the walk flushes its approach before this tile's
// pass-reward prompt (`prop::START`).
.props(&[(card_sdk::abi::prop::START, 1)])
// Deed-popup text (`docs/TILES.md` 「Tile text」): the two halves the book
// splits (line 95 settle draw, line 96 [经过] reward) speak for one slot each.
// `event:协助CiRCLE重建` replaces both while it is up.
.texts(&[
    (0, text_slot::SETTLE, "", "deed.note.circleSettle"),
    (1, text_slot::PASS, "", "deed.note.circlePass"),
]);

/// The [经过] arm's applicability. Only the tile this instance governs (the
/// dispatch already filters board instances by tile; this is the belt to that
/// brace), and only when the pass actually earns the reward.
///
/// 规则书: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[pass-reward]」 --
/// a move that began on the start square (a same-tile teleport onto the start
/// square, a 0-move that ends there) pays nothing. `start < 0` = no move in
/// flight (a card calling the reward outright): no 移动起点 to compare, so the
/// clause does not bar it.
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
    if rules_status::exile::get(mover) > 0 {
        return false;
    }
    let start = trigger::move_from();
    if start >= 0 && ctx::is_start(start) && ctx::is_start(me) {
        return false;
    }
    // A walk-scoped / per-player veto on one of the mover's own field
    // instances is read but not consumed -- the source disarms it. Pure
    // predicate before the reward prompt: suppress the entry (no flash) when
    // the mover cannot earn [pass-reward] at all.
    for (uid, _) in ctx::field_instances(mover) {
        if ctx::prop_at(uid, prop::NO_REWARD) > 0 {
            return false;
        }
    }
    true
}

/// 规则书: 「CiRCLE的[结算]是：抽取一张手卡。」
fn settle(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new("log.land_start").player_id("who", player_id),
    );
    // 规则书: 「[结算]是：抽取一张手卡」 -- `H.DrawR`.
    ctx::draw(player_id, 1)?;
    Ok(())
}

/// 规则书: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[pass-reward]」 --
/// the whole reward step (suppression, the choice, the `passRewardAffected`
/// window, the payout). `event:协助CiRCLE重建` re-homes that step onto
/// 「CiRCLE咖啡厅」 (「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅」) by messaging
/// this rule (see [`on_message_settle_reward`]), not by calling in here.
fn on_pass(_owner: i32) -> card_sdk::Asked {
    // The reward goes to the player who is passing, not to the owner of this
    // board-owned instance (that is `BOARD_OWNER`).
    let mover = trigger::player_id();
    if mover < 0 {
        return Ok(());
    }
    // 「获得」 wording differs for a stop on the start square as against a pass over it.
    let landing = trigger::move_resolve() && trigger::move_remaining() <= 0;
    settle_reward(mover, landing)
}

/// Cross-card `"settleReward"` -- pay this square's [pass-reward] to `seat`.
///
/// 规则书: 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」. The
/// event must not reach into this rule's Rust body: it messages the card, and
/// the reward runs **as this instance**, with its own choice prompt, its own
/// `passRewardAffected` window and its own `prop::NO_REWARD` read. That is the
/// whole point of the channel (`docs/CARDS.md`) -- a card that wants another
/// card's effect asks for it; it does not call that card's function.
///
/// Payload: `seat` = the player the reward is paid to; `b` = 1 when this is a
/// landing on the square (「获得」 wording -- the landing also draws its own
/// hand card from [`settle`]); `tile` = the square being paid for.
fn on_message_settle_reward(_owner: i32) -> card_sdk::Asked {
    let mover = ctx::message::seat();
    if mover < 0 {
        return Ok(());
    }
    let landing = ctx::message::b() != 0;
    settle_reward(mover, landing)
}

/// The [pass-reward] step: 「[获得]2000资金或抽1张卡」, between the pick and the
/// payout. `landing` selects the stop-on-the-start-square wording (the landing
/// also draws its own hand card from [`settle`]).
///
/// Private on purpose: the only entry points are this rule's own `PassTile`
/// arm and its `"settleReward"` message. Another card that wants the reward
/// asks for it (`ctx::send`); it does not call across into this module.
fn settle_reward(mover: i32, landing: bool) -> card_sdk::Asked {
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
    // instances is read but not consumed -- the source disarms it. The
    // `PassTile` entry's residual (`pass_pays`) already rejects this, so the
    // prompt never appears on that path; this copy keeps the landing and the
    // `"settleReward"` message honest.
    for (uid, _) in ctx::field_instances(mover) {
        if ctx::prop_at(uid, prop::NO_REWARD) > 0 {
            return Ok(());
        }
    }
    let pick = if rules_status::stun::get(mover) > 0 {
        // TODO(规则书): card-only while stunned is a ruling (`C44` / `D13`),
        // not a 「[pass-reward]」 clause.
        ctx::log(
            mover,
            &Msg::new("log.pass_reward_card_only").player_id("who", mover),
        );
        REWARD_CARD
    } else {
        // 「[获得]2000资金或抽1张卡」 -- the wording differs for a stop on
        // the start square (the landing also draws) as against a pass over it.
        let (text, money, card) = if landing {
            (
                "ask.pass_reward.text_landed",
                "ask.pass_reward.money_landed",
                "ask.pass_reward.card_landed",
            )
        } else {
            (
                "ask.pass_reward.text",
                "ask.pass_reward.money",
                "ask.pass_reward.card",
            )
        };
        let pick = ctx::ask_pick(
            mover,
            &Msg::new("ask.pass_reward.title"),
            &Msg::new(text).player_id("who", mover),
            &[
                Msg::new(money).n("money", CIRCLE_MONEY as i64),
                Msg::new(card),
            ],
        )?;
        pick as i32
    };
    // `passRewardAffected` -- the reward has been picked but not paid out. A
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
    if ctx::raise(mover, bandori_std::kind::PassRewardAffected, pick)? {
        if pick == REWARD_CARD {
            ctx::log(
                mover,
                &Msg::new("log.pass_reward_card").player_id("who", mover),
            );
            ctx::draw(mover, 1)?;
        } else {
            // 「[获得]2000资金」 -- a bank print through the money pipeline
            // (`Pay::new(_, "gain")`), event type `pass`, logged as
            // `log.pass_reward_money`. The 「支付」 scalars do not reach a print.
            ctx::gain_typed(
                mover,
                CIRCLE_MONEY,
                "pass",
                &Msg::new("log.pass_reward_money").player_id("who", mover),
            )?;
        }
    }
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn settle_kani(player_id: i32) -> card_sdk::Asked {
    settle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_message_settle_reward_kani(player_id: i32) -> card_sdk::Asked {
    on_message_settle_reward(player_id)
}
