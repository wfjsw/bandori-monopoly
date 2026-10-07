//! Shared helpers for event bodies (`docs/EVENTS.md`).
//!
//! Every event is a thin [`card_sdk::CardDef`] over these: the engine keeps the
//! deck / draw / active list, so a body is a list of citations over `ctx`
//! primitives -- never a reimplementation of the shell.

use alloc::string::String;
use alloc::vec::Vec;

use card_sdk::ctx;

/// Every seat in turn order, skipping players already out of the game.
pub fn all_players() -> Vec<i32> {
    (0..ctx::player_count())
        .filter(|&p| !ctx::player_out(p))
        .collect()
}

/// The event's rule id (`event:<id>`), for `ctx::event_*` calls that name it.
#[allow(dead_code)]
pub fn rule_id(id: &str) -> String {
    alloc::format!("event:{id}")
}

/// Keep this event in play: the engine leaves it on the active list and its
/// rule instance bound until some clause calls [`expire`].
///
/// 规则书: 「将此卡放置于场地中央」 -- staying is what that means mechanically.
pub fn keep() {
    ctx::set_dest(ctx::Dest::Field);
}

/// 「放入事件弃牌」 -- expire the active event `id` into the discard.
pub fn expire(id: &str) {
    ctx::event_expire(id, false);
}

/// 「永久移除」 -- expire the active event `id` out of the game entirely.
pub fn expire_removed(id: &str) {
    ctx::event_expire(id, true);
}

/// `X = count d sides` from the match RNG, summed. No [反击] window; a roll
/// that 「掷骰结算前」 can be countered goes through `ctx::roll_ask`.
pub fn roll(player_id: i32, count: i32, sides: i32) -> i32 {
    ctx::roll(player_id, count, sides)
}