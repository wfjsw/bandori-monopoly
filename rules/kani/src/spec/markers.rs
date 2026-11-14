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

use crate::world::{Effect, WindowPoint};

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
// payload ops (`msg_tile` / `msg_arg0` / `msg_sender_uid`) are protocol stubs
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

// ===========================================================================
// fire kernel — gain / spend policy
//
// `data/rules.txt` 126: 「· 火罐：技能指示物，在任意时刻，您持有的火罐数量不能超过
// 持有上限」. MARKERS.md §7: 火罐 is instance-owned by the character skill that
// prints 「（初始N，上限M）」; every write goes through the kernel
// (`bandori_std::fire::gain` / `spend`), which applies the owner's policy and
// returns **actuals**. I2 (「在任意时刻」) is the cap; I4 is "every change
// passes the owner's policy exactly once (one 效果适用 log, one window point)".
//
// The clauses the kernel owns, in order: the `markerGain` / `markerSpend`
// window opens **before** the write (spend opens the `ChargePaying`
// substitution window first, over the whole cost); gain clamps to the cap and
// logs the landed amount; spend is all-or-nothing and draws temp pots first
// (§10(2)); neither touches money or positions.
// ===========================================================================

/// I2, exactly as 「在任意时刻」 states it: the pool sits in `0..=max` (a
/// missing declaration is cap 0, so the only legal count is 0).
pub fn fire_i2_holds(value: i32, cap: i32) -> bool {
    value >= 0 && value <= cap.max(0)
}

/// How many pots a gain may land under 「不能超过持有上限」.
pub fn fire_gain_actual(n: i32, have: i32, cap: i32) -> i32 {
    if n <= 0 || cap <= 0 {
        0
    } else {
        n.clamp(0, (cap - have).max(0))
    }
}

/// Exactly one `GainFire` for `player` of `n` — the counter write's log.
pub fn fire_gain_log_ok(effects: &[Effect], player: i32, n: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainFire { player: p, n: m } if p == player && m == n)
}

/// The whole book outcome of one `fire::gain`: the pool took exactly the
/// clamped `actual`, I2 holds after, and the effect log is that one counter
/// write.
pub fn fire_gain_ok(
    effects: &[Effect],
    player: i32,
    pre: i32,
    cap: i32,
    n: i32,
    post: i32,
) -> bool {
    let actual = fire_gain_actual(n, pre, cap);
    post == pre + actual && fire_i2_holds(post, cap) && fire_gain_log_ok(effects, player, actual)
}

/// MUTANT: the reference forgets the cap and adds the whole `n`.
pub fn fire_gain_mutant_uncapped(effects: &[Effect], player: i32, pre: i32, n: i32, post: i32) -> bool {
    post == pre + n && fire_gain_log_ok(effects, player, n)
}

/// Exactly one window point of `kind`, on `player`, carrying `name` with
/// payload `value`.
pub fn fire_one_window(
    points: &[WindowPoint],
    kind: u8,
    player: i32,
    name: &str,
    value: i32,
) -> bool {
    points.len() == 1
        && points[0].kind == kind
        && points[0].player == player
        && crate::intern::id_eq_str(crate::intern::NameId(points[0].name), name)
        && points[0].value == value
}

/// A spend opens the `ChargePaying` window over the whole cost, then the
/// `markerSpend` window — both **before** any pot moves (user ruling
/// 2026-10-07; §10(2) keeps temp first inside that order).
pub fn fire_spend_windows_ok(points: &[WindowPoint], player: i32, n: i32, pay_name: &str) -> bool {
    points.len() == 2
        && points[0].kind == crate::world::WINDOW_RAISE
        && points[0].player == player
        && points[0].value == n
        && crate::intern::id_eq_str(crate::intern::NameId(points[0].name), pay_name)
        && fire_one_window(&points[1..], crate::world::WINDOW_SPEND, player, bandori_std::fire::KEY, -n)
}

/// Exactly one `AddToken{player, "charge", -n}` — the engine's 效果适用 line
/// for a spend (`wasm_rules::spend_marker` posts the caller's message with
/// value `-n`).
pub fn fire_spend_log_ok(effects: &[Effect], player: i32, n: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::AddToken { player: p, key, n: m }
            if p == player
                && key == crate::intern::intern(bandori_std::fire::KEY).0
                && m == -n)
}

/// The whole book outcome of one `fire::spend`: the cost was paid in full,
/// the pool fell by exactly `n`, and the effect log is that one write.
pub fn fire_spend_ok(effects: &[Effect], player: i32, pre: i32, n: i32, post: i32, paid: bool) -> bool {
    paid && n > 0 && pre >= n && post == pre - n && fire_spend_log_ok(effects, player, n)
}

/// MUTANT: the reference takes one pot too few.
pub fn fire_spend_mutant_underspend(post: i32, pre: i32, n: i32) -> bool {
    post == pre - n + 1
}
