//! Unit `skills_crychic`: reference specs FROM the clause docs only.
//!
//! Quotes: `docs/rulebook/spec/cards/crychic.md` (C-CRYCHIC-{tomoriskill,
//! soyoskill,rikiskill,sakiskill,mutsumiskill,band}-*), live sheets
//! `data/characters.json` / `data/bands.json`.
//!
//! Host dual-store note (see `spec/crychic.rs`): `give_stay`/`spend_fire` write
//! `World::stay`/`fire`; `state::{get,set,add}` on `state_key::{STAY,FIRE}`
//! write `player_keys`. Specs name the store the body uses.

use crate::world::Effect;

// ---------------------------------------------------------------------------
// C-CRYCHIC-tomoriskill-01 — 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
// Walk-past is PROTOCOL-LEVEL. Leaf: the cap declaration + the Pass body.
// ---------------------------------------------------------------------------

/// Cap declaration: fire pot initial 1, cap 1.
pub fn tomoriskill_01_cap() -> (i32, i32) {
    (1, 1)
}

/// Pass body: +1 fire (unlogged `gain_fire`). Post `w.fire` +1.
/// Effect log is empty in the Kani host (gain_fire is unlogged).
pub fn tomoriskill_01_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-tomoriskill-02 — 「（2）你回合内的任意时刻，可消耗1火罐清除自身一层[停留]」
// ---------------------------------------------------------------------------

/// Play body: `spend_fire(1)` + `state::add(STAY, -1)`. Both unlogged.
/// Post: `w.fire` -1 (the spend store) and `player_keys[STAY]` -1.
pub fn tomoriskill_02_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// MUTANT: the body is not a no-op when fire and stay allow it.
pub fn tomoriskill_02_log_ok_mutant(effects: &[Effect]) -> bool {
    effects.len() == 1
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-tomoriskill-03 — 「（3）当你的移动掷骰小于等于6时，立刻获得一层[停留]…」
// CLASS: `trigger::move_roll` is unmodeled (`trig_move_roll`) → BLOCKED-HOST
// for the roll hook. Deferred settle is PROTOCOL-LEVEL.
// ---------------------------------------------------------------------------

/// Roll body when face <= 6: GiveStay{1} + `before_turn_end` stub (unlogged).
pub fn tomoriskill_03_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::GiveStay { player: p, n: 1 } if p == player)
}

/// Face > 6: no stay.
pub fn tomoriskill_03_noop_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-soyoskill-01 — 「当其他玩家在属于你的格子上触发结算时，你可取消那次支付…」
// ---------------------------------------------------------------------------

/// Yes branch: waive the pay + arm the one-shot teleport latch.
/// `tile_key` / `armed_key` are interned `skill.soyoCrychic.tile` / `.armed`.
pub fn soyoskill_01_log_ok(
    effects: &[Effect],
    tile_key: u16,
    armed_key: u16,
    tile: i32,
) -> bool {
    effects.len() == 3
        && matches!(effects[0], Effect::SetPayAmount { to: 0 })
        && matches!(
            effects[1],
            Effect::SetState { key, n, .. } if key == tile_key && n == tile
        )
        && matches!(
            effects[2],
            Effect::SetState { key, n: 1, .. } if key == armed_key
        )
}

/// No branch: nothing happens.
pub fn soyoskill_01_noop_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// MUTANT: waive to 1.
pub fn soyoskill_01_log_ok_mutant(effects: &[Effect], tile_key: u16, armed_key: u16, tile: i32) -> bool {
    effects.len() == 3
        && matches!(effects[0], Effect::SetPayAmount { to: 1 })
        && matches!(
            effects[1],
            Effect::SetState { key, n, .. } if key == tile_key && n == tile
        )
        && matches!(
            effects[2],
            Effect::SetState { key, n: 1, .. } if key == armed_key
        )
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-rikiskill-01 — 「每当你的移动掷骰结果严格小于…」
// CLASS: blocked-by `trig_move_roll` (KANI-COVERAGE).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// C-CRYCHIC-sakiskill-01 — 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限2）」
// ---------------------------------------------------------------------------

/// Cap declaration: initial 1, cap 2.
pub fn sakiskill_01_cap() -> (i32, i32) {
    (1, 2)
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-mutsumiskill-01 — 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
// ---------------------------------------------------------------------------

pub fn mutsumiskill_01_cap() -> (i32, i32) {
    (1, 1)
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-band-01 — 「你的手牌数没有上限」
// ---------------------------------------------------------------------------

/// `HAND_LIMIT = i32::MAX` means 「没有上限」.
pub fn band_01_hand_limit_value() -> i32 {
    i32::MAX
}

/// Turn-start body: SetState{handLimit, i32::MAX} + SetState{SEEN, hand}.
pub fn band_01_log_ok(effects: &[Effect], player: i32, seen_key: u16, limit_key: u16, hand: i32) -> bool {
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::SetState { player: p, key, n } if p == player && key == limit_key && n == i32::MAX
        )
        && matches!(
            effects[1],
            Effect::SetState { player: p, key, n } if p == player && key == seen_key && n == hand
        )
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-band-01b — 「任何时刻拥有手牌数大于等于6时…无法从手中打出任何牌」
// FAILS(LESS): no body implements the play gate. See unit table.
// ---------------------------------------------------------------------------

/// The book's standing claim: when hand >= 6, a play from hand must be refused.
/// `play_accepted` is what the engine did. Spec is the negation.
pub fn band_01b_play_must_be_refused(play_accepted: bool) -> bool {
    !play_accepted
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-band-02 — turn-end transform. CLASS: PROTOCOL-LEVEL (multi-zone).
// Leaf: the X*500 cash-out that runs before the remove sweep.
// ---------------------------------------------------------------------------

/// 「当此卡移除时，你获得X*500资金」 — cash-out amount.
pub fn band_02_cashout_amount(x: i32) -> i32 {
    x * 500
}

/// Cash-out leaf log: GainMoney{x*500} when x > 0; empty when x == 0.
pub fn band_02_cashout_log_ok(effects: &[Effect], player: i32, x: i32) -> bool {
    if x <= 0 {
        effects.is_empty()
    } else {
        effects.len() == 1
            && matches!(
                effects[0],
                Effect::GainMoney { player: p, n } if p == player && n == x * 500
            )
    }
}

// ---------------------------------------------------------------------------
// C-CRYCHIC-band-03 — 「每当你的手牌数从4增加至5，为此卡放置一个奇迹水晶（上限10）」
// ---------------------------------------------------------------------------

/// Crystal on a true 4→5 crossing: +1, cap 10.
pub fn band_03_crystal_delta() -> i32 {
    1
}

pub fn band_03_crystal_cap() -> i32 {
    10
}

/// Drawn-hook leaf when SEEN == 4 and hand >= 5: AddCounter{+1} (instance
/// crystals via `add_crystals`). Cap clamp is the host's `add_counter`.
pub fn band_03_crystal_log_ok(effects: &[Effect], player: i32, key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter { owner, key: k, n: 1 } if owner == player && k == key
        )
}

/// Not a 4→5 crossing (SEEN != 4): no crystal.
pub fn band_03_noop_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// MUTANT: two crystals.
pub fn band_03_crystal_log_ok_mutant(effects: &[Effect], player: i32, key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter { owner, key: k, n: 2 } if owner == player && k == key
        )
}