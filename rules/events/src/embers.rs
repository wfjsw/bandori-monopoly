//! `event:火种燃尽之后会怎么样呢？` -- 事件卡「火种燃尽之后会怎么样呢？」（中立事件 A29）.
//!
//! 事件文本（data/events.json, id `火种燃尽之后会怎么样呢？`）:
//! > （1）将此卡的复制品放置于所有玩家所在格子上，触发事件的玩家回合开始时将此卡的复制品放置于所有玩家的前后各一格，触发结算时，格子上每有一张此卡的复制品，那名玩家失去10资金。任意玩家获得[除外]或破产时，移除所有此卡的复制品。

use card_sdk::abi::{state_key, AbKind, HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
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
        On::Play(None, play),
        On::Hook(&[HookKind::TurnStartBefore], always, on_turn_start),
        On::Hook(&[HookKind::SettleBefore], always, on_settle),
        On::Hook(
            &[HookKind::Exile, HookKind::Bankrupt, HookKind::Abnormal],
            always,
            on_out,
        ),
    ],
);

fn always(_player_id: i32) -> bool {
    true
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
            ctx::add_mark(at, -1, EMBER, &note);
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
/// Also the 「任意玩家获得[除外]…时」 safety net: a grant that bypassed the
/// `exile` raise (a raw state write) is caught here at the next turn start.
fn on_turn_start(_player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::TurnStartBefore {
        return Ok(());
    }
    // 「任意玩家获得[除外]或破产时，移除所有此卡的复制品」 -- see `on_out`; this
    // is the same clause re-checked at a turn boundary for grants that did not
    // raise `exile` (the engine's own `give_exile` path raises it).
    if any_exiled() {
        clear_copies(trigger::player_id());
    }
    if trigger::player_id() != ctx::prop(DRAWER) || ctx::prop(SPREAD) != 0 {
        return Ok(());
    }
    ctx::set_prop(SPREAD, 1);
    let note = Msg::new("log.event.embers_spread");
    for p in all_players() {
        for steps in [1, -1] {
            let at = ctx::tile_steps_ahead(p, steps);
            if at >= 0 {
                ctx::add_mark(at, -1, EMBER, &note);
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
    for t in 0..ctx::tile_count() {
        ctx::remove_marks(t, EMBER, -2);
    }
    ctx::log(
        who,
        &Msg::new("log.event.embers_clear").player_id("who", who),
    );
}

/// 规则书: 「触发结算时，格子上每有一张此卡的复制品，那名玩家失去10资金」
/// -- at each settle, the settler pays 10 per copy standing on that tile.
fn on_settle(_owner: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::SettleBefore {
        return Ok(());
    }
    let at = trigger::tile();
    let who = trigger::player_id();
    if at < 0 || who < 0 {
        return Ok(());
    }
    let n = ctx::count_marks(at, EMBER, -1);
    if n <= 0 {
        return Ok(());
    }
    // 「失去10资金」 per copy.
    ctx::pay(who, n * 10, &Msg::new("log.event.embers_burn").i("n", n as i64))?;
    Ok(())
}

/// 规则书: 「任意玩家获得[除外]或破产时，移除所有此卡的复制品」 -- strip every
/// copy mark from the board on an exile or a bankruptcy.
/// TODO(规则书): the text only removes the copies; it does not say the event
/// itself leaves play (and gives no other expiry), so the event stays.
fn on_out(_owner: i32) -> card_sdk::Asked {
    let hit = match trigger::kind() {
        TriggerKind::Exile | TriggerKind::Bankrupt => true,
        TriggerKind::Abnormal => trigger::abnormal_kind() == Some(AbKind::Exile),
        _ => false,
    };
    if !hit {
        return Ok(());
    }
    clear_copies(trigger::player_id());
    Ok(())
}