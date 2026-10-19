//! `status:exile` -- the [除外] **owner**.
//!
//! 规则书（`data/rules.txt` 50）:
//! > · [除外]：处于该状态时[无法移动]，不能被[指定]，无法使用[主动]技能和手卡的[手]效果，
//! >   无法收付款；玩家棋子不失为存在于任何格子上。该效果生效时无法重复获得该效果，
//! >   玩家的每回合开始前移除一层。处于该状态时玩家技能和[场地]上的[持续]效果停止生效。
//! >   获得该状态的效果会写明状态结束后在玩家回合开始时要[传送]到的格子。
//!
//! MyGO:无路矢's 「视为当回合的主要移动」 (`docs/rulebook/cards.json`,
//! id `MyGO:无路矢`) adds `exileMain`: the return teleport **is** that turn's
//! [主要移动].
//!
//! ## Clause-by-clause
//!
//! * **「处于该状态时[无法移动]，不能被[指定]，无法使用[主动]技能和手卡的[手]效果，
//!   无法收付款」** -- generic move / targeted / act / pay gates.
//! * **「玩家棋子不失为存在于任何格子上」** -- generic **on-board** gate;
//!   `present_from` / `players_on` / targeting skip exiled seats.
//! * **「该效果生效时无法重复获得该效果」** -- an already-exiled player gains
//!   no new layer ([`grant`]). A multi-layer first grant (MyGO:无路矢
//!   「获得2层[除外]」) is one acquisition and still lands.
//! * **「玩家的每回合开始前移除一层」** -- the [`on_turn_start_before`] tick
//!   runs at `turnStartBefore`, **before** `turnStart` (`docs/STATUS-HOSTS.md`
//!   §4). One layer per start; if layers remain the turn is auto-skipped
//!   (`log.exiled_skip` + `end_turn_now`); at 0 the return [传送] runs
//!   (`log.exile_over`) using `exileTo`.
//! * **Duration grants (「获得[除外]直至X」)** (user ruling 2026-10-18,
//!   `docs/rulebook/spec/global/status.md` G-STATUS-07) -- a continuous exile
//!   whose expiry is the `until` clause, not the turn-start layer tick.
//!   Represented as one layer plus `exileHold`; while the marker is set the
//!   tick does not peel (the turn is still auto-skipped,
//!   `log.exiled_skip_hold`), and the granting effect ends it via [`clear`].
//! * **[不可阻挡] 「无法获得新的[除外]层数」** (`data/rules.txt` 51) -- [`grant`]
//!   refuses new layers while `unstoppable > 0` (STA-01).
//! * **「处于该状态时玩家技能和[场地]上的[持续]效果停止生效」** -- the act
//!   gate and the on-board gate.
//! * **`exileMain`** -- the return teleport is consumed as the turn's
//!   [主要移动] (`teleport_as(..., as_main = true)`), so the player cannot
//!   also roll.

use card_sdk::abi::{AbKind, ActionKind, FilterKind};
use card_sdk::ctx::trigger;
use card_sdk::{CardDef, On};

/// The board-owned [除外] rule. One instance on `BOARD_OWNER`, `tile = -1`.
pub const STATUS_EXILE: CardDef = CardDef::new(
    "status:exile",
    &[
        // 「任意玩家获得[除外]…时」 -- the landing clause (already raised from
        // the engine's `exile_log` as `TriggerKind::Exile`; this `Abnormal`
        // hook is the same moment from the abnormal path).
        On::Action(&[ActionKind::Abnormal], "", "", Some(exile_landed), on_exile_landed),
        On::Action(&[ActionKind::Exile], "", "", None, on_exile_granted),
        // 「玩家的每回合开始前移除一层」 / the duration-hold skip. Runs as a
        // field hook collected last among the `turnStartBefore` walk so
        // sibling hooks still see pre-tick state (`docs/STATUS-HOSTS.md` §4).
        On::Action(
            &[ActionKind::TurnStartBefore],
            "",
            "",
            Some(turn_start_peels_or_holds),
            on_turn_start_before,
        ),
        // rules.txt 50: 「处于该状态时[无法移动]，…无法使用[主动]技能和手卡的
        // [手]效果，无法收付款」 -- three gates, **CEL-only**.
        // TODO(规则书): 「不能被[指定]」 and the on-board clause are still
        // engine-side (`MatchPlayer::status_gates`), pending Untargetable /
        // on-board gate kinds.
        On::FilterFlag(
            &[FilterKind::ActGate, FilterKind::PayGate, FilterKind::MoveGate],
            EXILED,
            None,
            refuse,
        ),
    ],
)
    .board_role("status");

/// The condition under which [除外] closes the act, pay and move gates.
pub const EXILED: &str = "state(actor,'exile') > 0";

fn refuse(_owner: i32, _v: i32) -> bool {
    false // deny
}

fn exile_landed(_owner: i32) -> bool {
    trigger::abnormal_kind() == Some(AbKind::Exile)
}

fn on_exile_landed(_owner: i32) -> card_sdk::Asked {
    Ok(())
}

/// 「任意玩家获得[除外]…时」 (`TriggerKind::Exile`) -- raised from
/// `exile_log` after the grant commits. Cards that mean 「获得[除外]时」
/// listen here rather than at each grant site (memory: *state clauses belong
/// on events*).
fn on_exile_granted(_owner: i32) -> card_sdk::Asked {
    Ok(())
}

/// Pure guard for the turn-start tick: only when the turn player is exiled.
/// (`trigger::player_id()` is the turn player; the instance owner is
/// `BOARD_OWNER`.)
fn turn_start_peels_or_holds(_owner: i32) -> bool {
    let p = trigger::player_id();
    p >= 0 && get(p) > 0
}

/// 「玩家的每回合开始前移除一层」 -- the turn-start layer tick, plus the
/// duration-hold skip (「获得[除外]直至X」, G-STATUS-07 ruling 2026-10-18).
///
/// Ordering (STATUS-HOSTS.md §4): this runs inside the `turnStartBefore`
/// raise, after the hand counteraction window and after sibling field hooks.
/// The engine then honors `end_turn_now` (or continues into `tick_state` /
/// `turnStart`).
fn on_turn_start_before(_owner: i32) -> card_sdk::Asked {
    use card_sdk::ctx;
    let p = trigger::player_id();
    if p < 0 || get(p) <= 0 {
        return Ok(());
    }
    // Duration grant: the hold marker suppresses the layer tick. The granting
    // effect ends the exile (and runs any return teleport itself if it owes
    // one); until then the turn is auto-skipped with no layer countdown.
    if ctx::state::get(p, KEY_HOLD) != 0 {
        ctx::log_as(
            p,
            "status",
            &card_sdk::Msg::new("log.exiled_skip_hold").player_id("who", p),
        );
        ctx::end_turn_now(p);
        return Ok(());
    }
    // 「玩家的每回合开始前移除一层」
    ctx::state::add(p, KEY, -1);
    let left = get(p);
    if left > 0 {
        ctx::log_as(
            p,
            "status",
            &card_sdk::Msg::new("log.exiled_skip")
                .player_id("who", p)
                .i("left", left as i64),
        );
        ctx::end_turn_now(p);
        return Ok(());
    }
    // Ticked out: 「状态结束后在玩家回合开始时传送」 + `exileMain`.
    let to = ctx::state::get(p, KEY_TO);
    let as_main = ctx::state::get(p, KEY_MAIN) != 0;
    // Drop every marker so nothing stale survives into the next acquisition.
    ctx::state::set(p, KEY_TO, -1);
    ctx::state::set(p, KEY_MAIN, 0);
    ctx::state::set(p, KEY_HOLD, 0);
    ctx::log_as(
        p,
        "status",
        &card_sdk::Msg::new("log.exile_over").player_id("who", p),
    );
    if to >= 0 {
        // No abnormal window: the return is this status's own clause.
        ctx::teleport_as(p, to, /* resolve */ false, as_main);
    }
    Ok(())
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
// ------------------------------------------------------------- canonical API

/// The state key this host owns (frozen webui wire).
pub const KEY: &str = "exile";
/// Tile the return [传送] lands on, or -1 for none.
pub const KEY_TO: &str = "exileTo";
/// 「视为当回合的主要移动」 -- the return teleport **is** that turn's
/// [主要移动] (MyGO:无路矢).
pub const KEY_MAIN: &str = "exileMain";
/// Duration grant (「获得[除外]直至X」) marker. While set, the turn-start
/// layer tick does not peel [除外].
pub const KEY_HOLD: &str = "exileHold";

/// Policy + service for one acquisition. Returns whether the grant landed.
///
/// 「该效果生效时无法重复获得该效果」 (`data/rules.txt` 50): an already-exiled
/// player gains no new layer. [不可阻挡] 「无法获得新的[除外]层数」
/// (`data/rules.txt` 51, STA-01): the same. A counteraction that cancels the
/// abnormal window writes no markers.
fn grant(player_id: i32, n_layers: i32, to: i32, hold: bool) -> bool {
    use card_sdk::ctx;
    if get(player_id) > 0 {
        return false;
    }
    if crate::unstoppable::get(player_id) > 0 {
        return false;
    }
    let before = get(player_id);
    // Engine service: the abnormal [反击] window + the layer write. `to` is
    // unused there (markers are this host's job).
    ctx::give_exile(player_id, n_layers, -1);
    if get(player_id) <= before {
        return false; // cancelled by counteraction
    }
    ctx::state::set(player_id, KEY_TO, to);
    ctx::state::set(player_id, KEY_HOLD, if hold { 1 } else { 0 });
    true
}

/// Give `n` [除外] layers; `to` is the return tile (-1 = none).
/// Layered grants (「获得N层[除外]」) use this and still tick per turn start.
/// Negative `n` is a removal (「移除一层[除外]」) and always applies.
pub fn send(player_id: i32, n: i32, to: i32) {
    use card_sdk::ctx;
    if n > 0 {
        grant(player_id, n, to, false);
        return;
    }
    if n < 0 {
        let left = (get(player_id) + n).max(0);
        ctx::state::set(player_id, KEY, left);
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
    card_sdk::ctx::exile_of(player_id)
}

/// Drop every [除外] layer and forget the return tile / hold marker. This is
/// the canonical "the duration ended" path (the `until` clause of a held
/// grant, or a manual clear).
pub fn clear(player_id: i32) {
    use card_sdk::ctx;
    let n = get(player_id);
    if n > 0 {
        ctx::state::add(player_id, KEY, -n);
    }
    clear_markers(player_id);
}

/// Forget `exileTo` / `exileMain` / `exileHold` without touching the layer
/// count (the tick-out path already has `exile == 0`).
fn clear_markers(player_id: i32) {
    use card_sdk::ctx;
    ctx::state::set(player_id, KEY_TO, -1);
    ctx::state::set(player_id, KEY_MAIN, 0);
    ctx::state::set(player_id, KEY_HOLD, 0);
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn on_exile_landed_kani(player_id: i32) -> card_sdk::Asked {
    on_exile_landed(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_exile_granted_kani(player_id: i32) -> card_sdk::Asked {
    on_exile_granted(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_turn_start_before_kani(player_id: i32) -> card_sdk::Asked {
    on_turn_start_before(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn refuse_kani(owner: i32, v: i32) -> bool {
    refuse(owner, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_start_peels_or_holds_kani(owner: i32) -> bool {
    turn_start_peels_or_holds(owner)
}