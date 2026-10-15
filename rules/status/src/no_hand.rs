//! `status:no_hand` -- the `noHand` companion **owner**.
//!
//! Not a named 「标志物解释」 marker; `data/rules.txt` has no `noHand`
//! passage. The key is the continuous 「may hold no hand cards」 /
//! 「手牌数没有上限」 slot content invents (`MatchPlayer.state`, frozen wire).
//! TODO(规则书): source is card text (e.g. 「手卡上限数量减1」 neighbours);
//! the 2 -> 1 -> 0 dance is engine behaviour, not a quoted clause.
//!
//! ## Behaviour (kept from `play.rs`)
//!
//! * `noHand == 2` at turn start folds to `1` (「this turn only」 half spent);
//! * `noHand == 1` at turn end clears to `0`;
//! * `noHand > 0` lifts the hand limit to [`card_sdk::abi::state_key::NO_HAND_LIMIT`].

use card_sdk::CardDef;

/// The board-owned `noHand` rule. One instance on `BOARD_OWNER`, `tile = -1`.
pub const STATUS_NO_HAND: CardDef = CardDef::new("status:no_hand", &[])
    .board_role("status");

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
// ------------------------------------------------------------- canonical API

/// The state key this host owns.
pub const KEY: &str = "noHand";

pub fn add(player_id: i32, n: i32) {
    card_sdk::ctx::state::add(player_id, KEY, n);
}

pub fn get(player_id: i32) -> i32 {
    card_sdk::ctx::state::get(player_id, KEY)
}
