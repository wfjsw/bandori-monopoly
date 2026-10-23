//! Unit `markers`: reference specs FROM the clause docs only.
//!
//! Sources:
//! * `data/rules.txt` 125 「标志物解释」: 「· CP点：放置于路面上的指示物」
//! * `docs/rulebook/cards.json` id `通用:该清CP了` [手] / （1） settle clause:
//!   「在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，
//!    [获得]800资金。」
//! * the user ruling recorded in `rules/markers/src/cp.rs` (2026-10-07):
//!   tile [CP点] vs on-card [CP点]; the 800 goes to the settling player.
//!
//! Never derived from engine behaviour.

use crate::world::Effect;

// ===========================================================================
// mark:cp — landing settle
// 「在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，
//  [获得]800资金。」
// ===========================================================================

/// The reward (the card text's fixed 800).
pub const CP_REWARD: i32 = 800;

/// The clause's guard, from the quote:
/// * the tile actually carries a [CP点];
/// * the mark's attached 该清CP了 instance (`src`) has ≥1 on-card [CP点]
///   (「自己[场上]1个[CP点]」 to spend);
/// * the settle is not cancelled (「将该次结算改为…」 skips every entry).
pub fn mark_cp_land_guard(
    cancelled: bool,
    settler: i32,
    tile: i32,
    tile_cp: i32,
    src: i32,
    src_cp: i32,
) -> bool {
    !cancelled && settler >= 0 && tile >= 0 && tile_cp > 0 && src >= 0 && src_cp > 0
}

/// Full book effect of one [结算] on a CP tile:
/// 1. remove one tile [CP点] (`bump_mark(..., -1)`);
/// 2. remove one on-card [CP点] of the mark's `src`;
/// 3. [获得]800资金 to the settling player.
///
/// `pre_tile_cp` / `pre_src_cp` / `post_tile_cp` / `post_src_cp` / money deltas
/// close the frame. Under the current Kani host `bump_mark` is a no-op and
/// `mark_src_at` always returns −1, so only the gain is observable — see
/// `mark_cp_land_gain_ok` and the unit table (BLOCKED-HOST).
pub fn mark_cp_land_effects_book(
    effects: &[Effect],
    settler: i32,
    pre_money: i32,
    post_money: i32,
    pre_tile_cp: i32,
    post_tile_cp: i32,
    pre_src_cp: i32,
    post_src_cp: i32,
) -> bool {
    post_tile_cp == pre_tile_cp - 1
        && post_src_cp == pre_src_cp - 1
        && post_money == pre_money + CP_REWARD
        && effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == settler && n == CP_REWARD)
}

/// Observable half under the current host model: exactly one `GainMoney`
/// of 800 to the settler. The mark / counter writes are unobservable
/// (`bump_mark_f` = 0, `mark_src_at` = −1).
pub fn mark_cp_land_gain_ok(effects: &[Effect], settler: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == settler && n == CP_REWARD)
}

/// MUTANT: the reference wrongly wants 900.
pub fn mark_cp_land_gain_ok_mutant_900(effects: &[Effect], settler: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == settler && n == 900)
}

/// MUTANT: the reference wrongly pays another seat.
pub fn mark_cp_land_gain_ok_mutant_wrong_seat(effects: &[Effect], settler: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p != settler && n == CP_REWARD)
}

// ===========================================================================
// mark:cp — placement (the cross-card `"place"` message)
// 「在任意一个没有角色和[CP点]的格子上添加1个[CP点]…」
//
// The placement gate (no character, no [CP点]) is the **placer's**; this
// owner only stamps category + provenance. Under the Kani host the message
// payload ops (`msg_tile` / `msg_a` / `msg_sender_uid`) are protocol stubs
// that return 0, so the handler cannot see the target tile or count —
// BLOCKED-HOST. Spec kept for when they land.
// ===========================================================================

/// Placement is legal when the payload names a live tile and a positive count.
/// (The 「没有角色和[CP点]」 gate is the placer's, not this owner's.)
pub fn mark_cp_place_guard(tile: i32, count: i32, tile_count: i32) -> bool {
    tile >= 0 && tile < tile_count && count > 0
}

/// One placement: a [CP点] mark of `count` units lands on `tile`, neutral
/// owner, provenance = the sending 该清CP了 instance.
pub fn mark_cp_place_effects_ok(
    effects: &[Effect],
    tile: i32,
    pre_tile_cp: i32,
    post_tile_cp: i32,
    count: i32,
) -> bool {
    post_tile_cp == pre_tile_cp + count
        && effects.len() == 1
        && matches!(effects[0], Effect::PlaceMark { tile: t, .. } if t == tile)
}
