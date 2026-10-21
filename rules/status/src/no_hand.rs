//! `status:no_hand` -- the `noHand` companion **owner**.
//!
//! Not a named 「标志物解释」 marker; `data/rules.txt` has no `noHand`
//! passage. The key is the continuous 「may hold no hand cards」 /
//! 「手牌数没有上限」 slot content invents (`MatchPlayer.state`, frozen wire).
//! TODO(规则书): source is card text (e.g. 「手卡上限数量减1」 neighbours);
//! the 2 -> 1 -> 0 dance is engine behaviour, not a quoted clause. Keep the
//! TODO: the dance is our reading of 「在下个自己的回合结束前不可使用手牌」
//! (event:飞鸟山之战), not a glossary rule.
//!
//! ## Behaviour (moved out of `play.rs`; this host owns the clause)
//!
//! * `noHand == 2` at turn start folds to `1` (「this turn only」 half spent);
//! * `noHand == 1` at turn end clears to `0`;
//! * `noHand > 0` closes the **play** gate (「不可使用手牌」, `err.play_no_hand`).
//!
//! ## Placement (ordering -- `docs/STATUS-HOSTS.md` §4)
//!
//! There is no hook exactly between `tick_state` and `turn_snap` where the
//! old inline dance sat. The host therefore hooks:
//!
//! * **fold (2 -> 1) on `TurnStart`** -- *after* `tick_state(TurnStart)` and
//!   the snapshot, not `TurnStartBefore`. `TurnStartBefore` would fold before
//!   the exile tick, so an exile-skipped turn (`log.exiled_skip` -> `end_turn`
//!   without ever reaching `turnStart`) would consume the fold and the clear
//!   would wipe the key mid-skip. `TurnStart` is not reached on that path,
//!   matching the old dance exactly. `noHand` is not in `turn_snap`, so the
//!   shift from "before snapshot" to "after snapshot" is unobservable.
//! * **clear (1 -> 0) on `TurnEnd`** -- hook-only (no counteraction window),
//!   raised after `tick_state(TurnEnd)`, the same slot the inline clear had.
//!
//! Both bodies act on `trigger::player_id()` (the turn seat), not the board
//! owner (`BOARD_OWNER`).

use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::ctx::trigger;
use card_sdk::{Action, CardDef, Filter, GateValue, On};

/// The board-owned `noHand` rule. One instance on `BOARD_OWNER`, tile = -1`.
/// Registered in [`crate::CARDS`]; `bind_tiles` places it.
pub const STATUS_NO_HAND: CardDef = CardDef::new(
    "status:no_hand",
    &[
        // Turn-start fold (2 -> 1). See the placement note above for why this
        // is `TurnStart` and not `TurnStartBefore`.
        On::Action(Action::TurnStart("", None, None, on_turn_start)),
        // Turn-end clear (1 -> 0). `TurnEnd` is hook-only.
        On::Action(Action::TurnEnd("", None, None, on_turn_end)),
        // 「不可使用手牌」 -- the play gate, **CEL-only**. No turn-skip: the
        // turn still runs; only hand-card plays are refused.
        On::Filter(Filter::PlayGate(NO_HAND_HELD, None, refuse)),
    ],
)
    .board_role("status")
    .gate_msgs(&[(FilterKind::PlayGate.gate_id(), "err.play_no_hand")]);

/// The condition under which `noHand` closes the play gate.
pub const NO_HAND_HELD: &str = "state(actor,'noHand') > 0";

fn refuse(_owner: i32, mut g: GateValue) -> GateValue {
    g.allowed = Some(false); // deny
    g
}

/// Turn-start fold (2 -> 1). Returns the new value.
pub fn tick_start(v: i32) -> i32 {
    if v == 2 {
        1
    } else {
        v
    }
}

/// Turn-end clear (1 -> 0). Returns the new value.
pub fn tick_end(v: i32) -> i32 {
    if v == 1 {
        0
    } else {
        v
    }
}

/// Is the hand limit lifted? (`noHand > 0`.)
pub fn lifts_hand_limit(v: i32) -> bool {
    v > 0
}

/// The `noHand` fold at the turn seat's `TurnStart`.
fn on_turn_start(_owner: i32) -> card_sdk::Asked {
    let who = trigger::player_id();
    if who < 0 {
        return Ok(());
    }
    let v = get(who);
    let folded = tick_start(v);
    if folded != v {
        card_sdk::ctx::state::set(who, KEY, folded);
        bandori_std::status::declare_companion(who, KEY);
    }
    Ok(())
}

/// The `noHand` clear at the turn seat's `TurnEnd`.
fn on_turn_end(_owner: i32) -> card_sdk::Asked {
    let who = trigger::player_id();
    if who < 0 {
        return Ok(());
    }
    let v = get(who);
    let cleared = tick_end(v);
    if cleared != v {
        card_sdk::ctx::state::set(who, KEY, cleared);
        bandori_std::status::declare_companion(who, KEY);
    }
    Ok(())
}

// ------------------------------------------------------------- canonical API

/// The state key this host owns.
pub const KEY: &str = "noHand";

pub fn add(player_id: i32, n: i32) {
    if n == 0 {
        return;
    }
    card_sdk::ctx::state::add(player_id, KEY, n);
    bandori_std::status::declare_companion(player_id, KEY);
}

pub fn get(player_id: i32) -> i32 {
    card_sdk::ctx::state::get(player_id, KEY)
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn on_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    on_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    on_turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn refuse_kani(player_id: i32, v: card_sdk::GateValue) -> card_sdk::GateValue {
    refuse(player_id, v)
}
