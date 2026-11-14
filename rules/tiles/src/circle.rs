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
//! choice. Both are ordinary rule entries, so a **rule swap**
//! (`ctx::tile_swap_rules`, `docs/rulebook/spec/abi/ab-tile-swap.md`) moves
//! them wholesale: `event:协助CiRCLE重建` swaps this instance onto
//! 「CiRCLE咖啡厅」 (「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」)
//! and both halves then run there with no cross-card message. `docs/TILES.md`.
//!
//! Suppression is `prop::NO_REWARD` on *this* instance: a rule that
//! 「无法获取[pass-reward]」 / 「[经过]CiRCLE时不获得[pass-reward]」 /
//! 「首次经过CiRCLE不获得经过奖励」 sets the prop and clears it when its own
//! clause ends. The source owns the arming and the disarming; the reader is the
//! instance, not a per-player latch, and the read rides a rule swap with it. A
//! veto on one of the mover's own field instances is read but **not** consumed
//! (the source holds that one).
//!
//! Preconditions live on the entry, not at the body top (`docs/GUARDS.md` §0):
//! the [经过] arm's residual guard takes the exile gate and 「[移动起点]不为
//! CiRCLE」 (by kind -- a swap does not make the partner square the start
//! square); the governing-square match is the dispatch's. The `NO_REWARD`
//! check stays in the body because the instance-side arm is **consumed** --
//! an early-out after a side effect.
//!
//! TODO(规则书): the stunned player's card-only reward is a ruling, not book
//! text (`回合階段&註釋` `C44` blocks the money half; `团技能` `D13` forces the
//! card half).
use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{prop, text_slot, ActionKind, REWARD_CARD, REWARD_MONEY};
use card_sdk::{Action, CardDef, Msg, On};

/// 「[获得]2000资金」 (专有名词). Matches `game_core::engine::world::CIRCLE_MONEY`.
pub const CIRCLE_MONEY: i32 = 2000;

pub const CIRCLE: CardDef = CardDef::new(
    "tile:circle",
    &[
        // Deed-popup text (`docs/TILES.md` 「Tile text」): the two halves the
        // book splits (line 95 settle draw, line 96 [经过] reward) speak for
        // one slot each. `event:协助CiRCLE重建` replaces the settle while it is
        // up and suppresses the pass; a rule swap re-homes both texts with the
        // bodies (`World::governing_square`).
        On::Settle("", None, settle).texts(&[(text_slot::SETTLE, "", "deed.note.circleSettle")]),
        On::Action(Action::PassTile("", Some(pass_pays), on_pass)).texts(&[(
            text_slot::PASS,
            "",
            "deed.note.circlePass",
        )]),
    ],
)
.board_role("tile")
// The start square: the walk flushes its approach before this tile's
// pass-reward prompt (`prop::START`).
.props(&[(card_sdk::abi::prop::START, 1)]);

/// The [经过] arm's applicability: the pass actually earns the reward.
/// Whether the walked square is the one this instance is bound to is the
/// dispatch's own question -- the board's hook list is built from the tile's
/// **governing** instances (`World::tile_rule_instances`), so a rule swap
/// sends this entry with the square and no body-side re-check is needed. The
/// resolving pass (a replay after a prompt resume) narrows a board instance
/// the same way, so the walk's earlier steps do not pay this reward again.
///
/// 规则书: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[pass-reward]」 --
/// a move that began on the start square (a same-tile teleport onto the start
/// square, a 0-move that ends there) pays nothing. `start < 0` = no move in
/// flight: no 移动起点 to compare, so the clause does not bar it. The start
/// square is read by **kind** (`ctx::is_start`, the static board), never by
/// the instance's current square: 「CiRCLE原本的所有效果迁移至…」 migrates the
/// effect with this clause, not the square's identity (a pass of the partner
/// square from CiRCLE itself still reads 「移动起点为CiRCLE」).
///
/// An exiled mover earns nothing (回合階段&註釋: [除外] players are out of the
/// income loop). TODO(规则书): the exile bar is engine bookkeeping, not a
/// 「[经过]CiRCLE」 clause.
fn pass_pays(_owner: i32) -> bool {
    let me = ctx::self_tile().unwrap_or(-1);
    if me < 0 {
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
    if start >= 0 && ctx::is_start(ctx::tile_at(start)) && ctx::is_start(me) {
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
    ctx::draw(player_id, 1)?;
    Ok(())
}

/// 规则书: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[pass-reward]」 --
/// the whole reward step (suppression, the choice, the `passRewardAffected`
/// window, the payout). 规则书: 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅」 --
/// the swap moves this very entry with the rule instance, so the body needs
/// no cross-card message.
fn on_pass(_owner: i32) -> card_sdk::Asked {
    // The reward goes to the player who is passing, not to the owner of this
    // board-owned instance (that is `BOARD_OWNER`).
    let mover = trigger::player_id();
    if mover < 0 {
        return Ok(());
    }
    // 「获得」 wording differs for a stop on the start square as against a pass over it.
    let landing = trigger::move_settles() && trigger::move_remaining() <= 0;
    settle_reward(mover, landing)
}

/// The [pass-reward] step: 「[获得]2000资金或抽1张卡」, between the pick and the
/// payout. `landing` selects the stop-on-the-start-square wording (the landing
/// also draws its own hand card from [`settle`]).
///
/// Private on purpose: the only entry point is this rule's own `PassTile`
/// arm. Another card that wants the reward either swaps this rule instance to
/// its square (`ctx::tile_swap_rules`) or copies the clause; it does not call
/// across into this module.
fn settle_reward(mover: i32, landing: bool) -> card_sdk::Asked {
    // `prop::NO_REWARD` is a prop of **this rule instance** (`ctx::prop`), not
    // of whatever square it currently governs: the source owns the arming, the
    // instance is the reader, and 「…迁移至…」 carries the read with the
    // instance. Consumed here, so a stale arm cannot veto the next player
    // (`event:协助CiRCLE重建` re-arms today's other arms by square).
    if ctx::prop(prop::NO_REWARD) > 0 {
        ctx::set_prop(prop::NO_REWARD, 0);
        return Ok(());
    }
    // A walk-scoped / per-player veto on one of the mover's own field
    // instances is read but not consumed -- the source disarms it. The
    // `PassTile` entry's residual (`pass_pays`) already rejects this, so the
    // prompt never appears; this copy keeps the landing honest.
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
        let pick = ctx::ask_choice(
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
