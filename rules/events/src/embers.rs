//! `event:火种燃尽之后会怎么样呢？` -- 事件卡「火种燃尽之后会怎么样呢？」（中立事件 A29）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `火种燃尽之后会怎么样呢？`）:
//! > （1）将此卡的复制品放置于所有玩家所在格子上，触发事件的玩家回合开始时将此卡的复制品放置于所有玩家的前后各一格，触发结算时，格子上每有一张此卡的复制品，那名玩家失去10资金。任意玩家获得[除外]或破产时，移除所有此卡的复制品。

use bandori_std::state_key; use card_sdk::abi::{AbKind, ActionKind, MarkFilter, TriggerKind};
use bandori_std::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, keep};

const ID: &str = "火种燃尽之后会怎么样呢？";

/// The copy mark this event leaves on tiles (owner -1 = neutral).
const EMBER: &str = "ember";

/// The player who drew (「触发」) the event, on the instance's props.
const DRAWER: &str = "drawer";
/// Set once the drawer's turn-start spread has run.
const SPREAD: &str = "spread";

pub const EMBERS: CardDef = CardDef::new(
    "event:火种燃尽之后会怎么样呢？",
    &[
        On::Play("", "", None, play),
        // Safety net: 「任意玩家获得[除外]…时，移除所有此卡的复制品」 re-checked
        // at a turn boundary for grants that did not raise `exile`. Residual
        // admits only when someone is actually exiled, so a no-op turn start
        // does not activate.
        On::Action(&[ActionKind::TurnStartBefore], "", "", Some(any_exiled_guard), on_turn_start_clear),
        // 「触发事件的玩家回合开始时将此卡的复制品放置于所有玩家的前后各一格」
        // -- one-shot at the drawer's own turn start.
        On::Action(&[ActionKind::TurnStartBefore], "", "", Some(spread_applies), on_turn_start),
        // 「触发结算时」 is 行动阶段 15 -- an entry in the settle's effect list
        // (`SETTLE-STAGES.md` §4 M2), not a pre-settle write and not an after
        // hook. `settleBody` is the list; a field card that replaces the body
        // (`trigger::cancelled()`) skips this entry like every other.
        On::Action(&[ActionKind::SettleBody], "", "", Some(on_settle_applies), on_settle),
        On::Action(&[bandori_std::kind::Exile, ActionKind::Bankrupt, ActionKind::Abnormal], "", "", Some(on_out_applies), on_out),
    ],
)
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"火种燃尽之后会怎么样呢？"#)
            .text(r#"（1）将此卡的复制品放置于所有玩家所在格子上，触发事件的玩家回合开始时将此卡的复制品放置于所有玩家的前后各一格，触发结算时，格子上每有一张此卡的复制品，那名玩家失去10资金。任意玩家获得[除外]或破产时，移除所有此卡的复制品。"#)
            .cell(r#"A29"#)
            .derived(false));

/// Residual guard for [`on_turn_start_clear`]: someone is carrying [除外].
fn any_exiled_guard(_player_id: i32) -> bool {
    any_exiled()
}

/// Residual guard for the one-shot spread: the drawer's own turn start and
/// not yet spread.
fn spread_applies(_player_id: i32) -> bool {
    trigger::player_id() == ctx::prop(DRAWER) && ctx::prop(SPREAD) == 0
}

/// Residual guard for [`on_settle`]: the tile carries at least one copy.
/// Derived mark scan (docs/GUARDS.md §4.2).
fn on_settle_applies(_player_id: i32) -> bool {
    if trigger::kind() != TriggerKind::SettleBody || trigger::cancelled() {
        return false;
    }
    let at = trigger::tile();
    if at < 0 || trigger::player_id() < 0 {
        return false;
    }
    ctx::count_marks(at, &MarkFilter::any().kind(EMBER).owner(-1)) > 0
}

/// Residual guard for [`on_out`]: the raise is an exile or a bankruptcy
/// (an `Abnormal` only counts when its kind is `Exile`).
fn on_out_applies(_player_id: i32) -> bool {
    match trigger::kind() {
        // exile (ruleset 21) or bankruptcy
        k if k == bandori_std::kind::Exile.as_trigger() || k == TriggerKind::Bankrupt => true,
        TriggerKind::Abnormal => trigger::abnormal_kind() == Some(bandori_std::kind::abnormal::Exile),
        _ => false,
    }
}

/// 规则书: 「将此卡的复制品放置于所有玩家所在格子上」 -- keep the event and
/// drop one neutral copy mark on each player's tile.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::set_prop(DRAWER, player_id);
    ctx::set_prop(SPREAD, 0);
    let note = Msg::new("log.event.embers_on")
        .player_id("who", player_id)
        .card("event", ID);
    for p in all_players() {
        let at = ctx::player_pos(p);
        if at >= 0 {
            // One row per copy (`place_mark_new`): the old `add_mark` pushed a
            // fresh row, and tests pin separate rows when two players share a
            // tile.
            ctx::place_mark(at, EMBER, "", -1, ctx::self_uid(), 1, &note, card_sdk::abi::Stack::Fresh);
        }
    }
    ctx::log(player_id, &note);
    Ok(())
}

/// 规则书: 「触发事件的玩家回合开始时将此卡的复制品放置于所有玩家的前后各一格」
/// -- one more wave of copies at the drawer's turn start, on each player's front
/// and back neighbour tiles. Runs once (「回合开始时」 names a single moment).
/// TODO(规则书): the text does not say whether the spread repeats on later turn
/// starts of the drawer; this body reads it as one-shot.
/// 「任意玩家获得[除外]…时」 safety net at a turn boundary -- see `on_out`.
/// Runs only when [`any_exiled_guard`] holds; the clear itself is visible.
fn on_turn_start_clear(_player_id: i32) -> card_sdk::Asked {
    clear_copies(trigger::player_id());
    Ok(())
}

/// Applicability is [`spread_applies`].
fn on_turn_start(_player_id: i32) -> card_sdk::Asked {
    ctx::set_prop(SPREAD, 1);
    let note = Msg::new("log.event.embers_spread");
    for p in all_players() {
        for steps in [1, -1] {
            let at = ctx::tile_steps_ahead(p, steps);
            if at >= 0 {
                // Fresh row per copy (`place_mark_new`): two players sharing a
                // neighbour must land as separate rows, not one stacked count.
                ctx::place_mark(at, EMBER, "", -1, ctx::self_uid(), 1, &note, card_sdk::abi::Stack::Fresh);
            }
        }
    }
    Ok(())
}

/// Is anyone carrying [除外] right now?
fn any_exiled() -> bool {
    all_players()
        .iter()
        .any(|&p| ctx::state::get(p, state_key::EXILE) > 0)
}

/// 「移除所有此卡的复制品」
fn clear_copies(who: i32) {
    // `MarkFilter::any()` keeps the old `owner: -2` "any owner" match; tile -1
    // is every tile, logged as one line.
    ctx::remove_marks(-1, &MarkFilter::any().kind(EMBER));
    ctx::log(
        who,
        &Msg::new("log.event.embers_clear")
            .player_id("who", who)
            .card("event", ID),
    );
}

/// 规则书: 「触发结算时，格子上每有一张此卡的复制品，那名玩家失去10资金」
/// -- at each settle, the settler pays 10 per copy standing on that tile.
/// 行动阶段 15 (`SETTLE-STAGES.md` §4 M2): an entry in the settle's effect
/// list, so a body replace skips it and a cancelled settle never reaches it.
fn on_settle(_owner: i32) -> card_sdk::Asked {
    let at = trigger::tile();
    let who = trigger::player_id();
    let n = ctx::count_marks(at, &MarkFilter::any().kind(EMBER).owner(-1));
    // 「失去10资金」 per copy. `n > 0` is [`on_settle_applies`].
    ctx::pay(who, n * 10, &Msg::new("log.event.embers_burn").i("n", n as i64))?;
    Ok(())
}

/// 规则书: 「任意玩家获得[除外]或破产时，移除所有此卡的复制品」 -- strip every
/// copy mark from the board on an exile or a bankruptcy.
/// TODO(规则书): the text only removes the copies; it does not say the event
/// itself leaves play (and gives no other expiry), so the event stays.
fn on_out(_owner: i32) -> card_sdk::Asked {
    // Applicability is [`on_out_applies`].
    clear_copies(trigger::player_id());
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_turn_start_clear_kani(player_id: i32) -> card_sdk::Asked {
    on_turn_start_clear(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    on_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_settle_kani(player_id: i32) -> card_sdk::Asked {
    on_settle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_out_kani(player_id: i32) -> card_sdk::Asked {
    on_out(player_id)
}
