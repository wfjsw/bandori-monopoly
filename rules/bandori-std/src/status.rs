//! Shared status verbs (「异常状态」) for this ruleset.
//!
//! The `status:*` host cards in `rules/status` own the standing hooks, ticks
//! and policy clauses (规则书 quotes live there). The **grant / read / clear
//! verbs** are shared by the hosts plus cards and events, so they live here
//! over the generic engine services (`apply_abnormal`, `state::*`,
//! `skip_main_move` / `clear_move_skip`, `end_turn_now`, `teleport_as`,
//! `log_as`). `rules/status` re-exports these so `rules_status::exile::…`
//! call sites keep working.
//!
//! Behaviour is a relocation of the verbs that used to sit in
//! `rules/status/src/{exile,stay,stun}.rs` -- no policy change.

use card_sdk::ctx::{self, state};

use crate::kind::abnormal;
use crate::state_key;

// --------------------------------------------------------------- [停留]

/// [停留] (rules.txt 48): 「处于该状态时[无法移动]。重复受到该效果时可叠加，
/// 玩家的每回合结束时移除一层。」
pub mod stay {
    use super::*;

    /// Add `n` [停留] layers (negative removes; floors at 0). Wear-off
    /// `Tick::TurnEnd`, declared here. Un-latching the turn's move gate on the
    /// last layer is [`unlatch`] / [`add`]'s own latch call, not a side effect
    /// the engine invents. [不可阻挡] does **not** refuse stay layers (it
    /// 「无视」s the gate, not the grant).
    ///
    /// The move latch is turn-flow, not status policy: a layer gained mid-turn
    /// locks the turn's main move immediately; dropping the last layer
    /// un-skips it. **exile is not part of this latch** (closeout-2 ruling):
    /// it grants [无法移动] (glossary 13) but not 「跳过主要移动阶段」.
    pub fn add(player_id: i32, n: i32) {
        ctx::apply_abnormal(player_id, abnormal::Stay, n, -1);
        if n != 0 {
            state::set_expires(player_id, state_key::STAY, state::TURN_END);
        }
        // 「stay：处于该状态时[无法移动]」 and step 7 「停留效果发动：跳过主要
        // 移动阶段」 -- the skip is a *consequence* of the state. 「[不可阻挡]」:
        // 「处于该状态时无视已存在的stay」 -- an unstoppable holder walks through
        // the stay rather than losing the move.
        let after = get(player_id);
        let unstoppable = state::get(player_id, state_key::UNSTOPPABLE) > 0;
        ctx::skip_main_move(player_id, after > 0 && !unstoppable);
    }

    /// [停留] count-changed clause (rules.txt 48): when the last stay layer (or
    /// an [不可阻挡] grant) leaves the player free to walk, the turn's main-move
    /// latch is released. exile is deliberately not part of it (see [`add`]).
    pub fn unlatch(player_id: i32) {
        let stay_after = get(player_id);
        let unstoppable = state::get(player_id, state_key::UNSTOPPABLE);
        if unlatch_skip_move(stay_after, 0, unstoppable) {
            ctx::clear_move_skip(player_id);
        }
    }

    /// Pure predicate: is the turn's main move free of the stay gate?
    /// `exile_after` is accepted for the documented call shape (the engine's
    /// interim clause reads it) but **does not** hold the latch -- closeout-2.
    pub fn unlatch_skip_move(stay_after: i32, _exile_after: i32, unstoppable: i32) -> bool {
        stay_after <= 0 || unstoppable > 0
    }

    /// [停留]'s contribution to the generic move gate.
    pub fn blocks_move(stay: i32, unstoppable: i32) -> bool {
        // 「处于该状态时[无法移动]」 vs [不可阻挡] 「无视已存在的[停留]」.
        stay > 0 && unstoppable <= 0
    }

    /// [停留] does **not** close the act or pay gates (only [晕眩] / [除外] do).
    pub fn blocks_act(_stay: i32) -> bool {
        false
    }

    pub fn blocks_pay(_stay: i32) -> bool {
        false
    }

    /// [停留] layers on the player.
    pub fn get(player_id: i32) -> i32 {
        state::get(player_id, state_key::STAY)
    }

    /// Drop every [停留] layer.
    pub fn clear(player_id: i32) {
        let n = get(player_id);
        if n > 0 {
            add(player_id, -n);
        }
    }
}

// --------------------------------------------------------------- [晕眩]

/// [晕眩] (rules.txt 49): 「处于该状态时[无法移动]，无法使用[主动]技能和手卡的
/// [手]效果，无法收付款。重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
pub mod stun {
    use super::*;

    /// Add `n` [晕眩] layers (floors at 0). Wear-off `Tick::TurnEnd`, declared
    /// here. [不可阻挡] 「晕眩效果也不会获得新的层数」: a positive grant while
    /// `unstoppable > 0` is refused before the engine service, so no abnormal
    /// [反击] window opens (STA-01). Negative `n` always runs (strip layers).
    pub fn add(player_id: i32, n: i32) {
        if n > 0 && crate::status::unstoppable_refuses(player_id) {
            return;
        }
        ctx::apply_abnormal(player_id, abnormal::Stun, n, -1);
        if n != 0 {
            state::set_expires(player_id, state_key::STUN, state::TURN_END);
        }
    }

    /// Add `n` `stunStart` layers -- the ones that only start counting next turn
    /// (`Tick::TurnStart`). Same [不可阻挡] refusal as [`add`]. No abnormal
    /// window today (raw state write; the engine service opens one only for
    /// `stun`).
    pub fn add_start(player_id: i32, n: i32) {
        if n > 0 && crate::status::unstoppable_refuses(player_id) {
            return;
        }
        if n == 0 {
            return;
        }
        state::add(player_id, state_key::STUN_START, n);
        state::set_expires(player_id, state_key::STUN_START, state::TURN_START);
    }

    /// [晕眩]'s contribution to the generic move gate.
    pub fn blocks_move(stun: i32) -> bool {
        stun > 0
    }

    /// 「无法使用[主动]技能和手卡的[手]效果」 -- act gate.
    pub fn blocks_act(stun: i32) -> bool {
        stun > 0
    }

    /// 「无法收付款」 -- pay gate (and the payee filter).
    pub fn blocks_pay(stun: i32) -> bool {
        stun > 0
    }

    /// [晕眩] does not close the on-board gate (the piece still stands on a tile).
    pub fn blocks_on_board(_stun: i32) -> bool {
        false
    }

    /// [晕眩] layers (`stun` + `stunStart`, what the view shows as 眩晕).
    pub fn get(player_id: i32) -> i32 {
        state::get(player_id, state_key::STUN)
    }

    /// Drop every [晕眩] layer (both counters).
    pub fn clear(player_id: i32) {
        let n = state::get(player_id, state_key::STUN);
        if n > 0 {
            state::add(player_id, state_key::STUN, -n);
        }
        let s = state::get(player_id, state_key::STUN_START);
        if s > 0 {
            state::add(player_id, state_key::STUN_START, -s);
        }
    }
}

// --------------------------------------------------------------- [除外]

/// [除外] (rules.txt 50). The host card in `rules/status` owns the turn-start
/// tick and the `status:*` hooks; these verbs are the shared grant / read /
/// clear surface (cards like MyGO:无路矢 and Sumimi:一人两个甜甜圈 call them).
pub mod exile {
    use super::*;

    /// Policy + service for one acquisition. Returns whether the grant landed.
    ///
    /// 「该效果生效时无法重复获得该效果」 (`data/rules.txt` 50): an already-exiled
    /// player gains no new layer. [不可阻挡] 「无法获得新的[除外]层数」
    /// (`data/rules.txt` 51, STA-01): the same. A counteraction that cancels the
    /// abnormal window writes no markers.
    fn grant(player_id: i32, n_layers: i32, to: i32, hold: bool) -> bool {
        if get(player_id) > 0 {
            return false;
        }
        if crate::status::unstoppable_refuses(player_id) {
            return false;
        }
        let before = get(player_id);
        // Engine service: the abnormal [反击] window + the layer write. `to` is
        // unused there (markers are this host's job).
        ctx::apply_abnormal(player_id, abnormal::Exile, n_layers, -1);
        if get(player_id) <= before {
            return false; // cancelled by counteraction
        }
        state::set(player_id, state_key::EXILE_TO, to);
        state::set(player_id, state_key::EXILE_HOLD, if hold { 1 } else { 0 });
        true
    }

    /// Give `n` [除外] layers; `to` is the return tile (-1 = none).
    /// Layered grants (「获得N层[除外]」) use this and still tick per turn start.
    /// Negative `n` is a removal (「移除一层[除外]」) and always applies.
    pub fn send(player_id: i32, n: i32, to: i32) {
        if n > 0 {
            grant(player_id, n, to, false);
            return;
        }
        if n < 0 {
            let left = (get(player_id) + n).max(0);
            state::set(player_id, state_key::EXILE, left);
            if left == 0 {
                clear_markers(player_id);
            }
        }
    }

    /// Duration grant (「获得[除外]直至X」): one layer held until the granting
    /// effect ends it. The turn-start layer tick does not peel it (`exileHold`);
    /// call [`clear`] when the `until` clause fires.
    pub fn hold(player_id: i32, to: i32) {
        grant(player_id, 1, to, true);
    }

    /// [除外] layers on the player.
    pub fn get(player_id: i32) -> i32 {
        state::get(player_id, state_key::EXILE)
    }

    /// Drop every [除外] layer and forget the return tile / hold marker. This is
    /// the canonical "the duration ended" path (the `until` clause of a held
    /// grant, or a manual clear).
    pub fn clear(player_id: i32) {
        let n = get(player_id);
        if n > 0 {
            state::add(player_id, state_key::EXILE, -n);
        }
        clear_markers(player_id);
    }

    /// Forget `exileTo` / `exileMain` / `exileHold` without touching the layer
    /// count (the tick-out path already has `exile == 0`).
    fn clear_markers(player_id: i32) {
        state::set(player_id, state_key::EXILE_TO, -1);
        state::set(player_id, state_key::EXILE_MAIN, 0);
        state::set(player_id, state_key::EXILE_HOLD, 0);
    }

    /// [除外]'s contribution to the generic move gate.
    pub fn blocks_move(exile: i32) -> bool {
        exile > 0
    }

    /// 「无法使用[主动]技能和手卡的[手]效果」 + 「玩家技能…停止生效」 -- act gate.
    pub fn blocks_act(exile: i32) -> bool {
        exile > 0
    }

    /// 「无法收付款」 -- pay gate.
    pub fn blocks_pay(exile: i32) -> bool {
        exile > 0
    }

    /// 「玩家棋子不失为存在于任何格子上」 -- on-board gate.
    pub fn blocks_on_board(exile: i32) -> bool {
        exile > 0
    }

    /// 「不能被[指定]」 -- targeted gate.
    pub fn blocks_targeted(exile: i32) -> bool {
        exile > 0
    }
}

/// [不可阻挡] (rules.txt 51, STA-01): 「晕眩效果也不会获得新的层数」 /
/// 「无法获得新的[除外]层数」. Stay layers are **not** refused (the holder
/// 「无视」s the gate, not the grant).
pub(crate) fn unstoppable_refuses(player_id: i32) -> bool {
    state::get(player_id, state_key::UNSTOPPABLE) > 0
}