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
//! * **「该效果生效时无法重复获得该效果」** -- TODO(规则书)（1）: today layers
//!   stack (`give_exile`). Conservative reading: an already-exiled player
//!   gains no new layer. Flip in Phase C.
//! * **「玩家的每回合开始前移除一层」** -- the tick runs at turn start,
//!   **before** `turnStart` (`play.rs` ordering table in
//!   `docs/STATUS-HOSTS.md` §4). One layer per start; if layers remain the
//!   turn is auto-skipped (`log.exiled_skip`); at 0 the return [传送] runs
//!   (`log.exile_over`) using `exileTo`.
//! * **「处于该状态时玩家技能和[场地]上的[持续]效果停止生效」** -- the act
//!   gate and the on-board gate.
//! * **`exileMain`** -- the return teleport is consumed as the turn's
//!   [主要移动] (`main_moved = true`), so the player cannot also roll.

use card_sdk::abi::{AbKind, GateKind, HookKind};
use card_sdk::ctx::trigger;
use card_sdk::{CardDef, On};

/// The board-owned [除外] rule. One instance on `BOARD_OWNER`, `tile = -1`.
pub const STATUS_EXILE: CardDef = CardDef::new(
    "status:exile",
    &[
        // 「任意玩家获得[除外]…时」 -- the landing clause (already raised from
        // the engine's `exile_log` as `TriggerKind::Exile`; this `Abnormal`
        // hook is the same moment from the abnormal path).
        On::Hook(&[HookKind::Abnormal], "", Some(exile_landed), on_exile_landed),
        On::Hook(&[HookKind::Exile], "", None, on_exile_granted),
        // rules.txt 50: 「处于该状态时[无法移动]，…无法使用[主动]技能和手卡的
        // [手]效果，无法收付款」 -- three gates, **CEL-only**.
        // TODO(规则书): 「不能被[指定]」 and the on-board clause are still
        // engine-side (`MatchPlayer::status_gates`), pending Untargetable /
        // on-board gate kinds.
        On::Gate(
            &[GateKind::ActGate, GateKind::PayGate, GateKind::MoveGate],
            EXILED,
            None,
            refuse,
        ),
    ],
);

/// The condition under which [除外] closes the act, pay and move gates.
pub const EXILED: &str = "exile(actor) > 0";

fn refuse(_owner: i32) -> card_sdk::Asked {
    trigger::set_cancelled();
    Ok(())
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

/// Give `n` [除外] layers; `to` is the return tile (-1 = none).
///
/// TODO(规则书)（1）: 「该效果生效时无法重复获得该效果」 (`data/rules.txt`
/// 50) -- today layers stack. Conservative reading: an already-exiled
/// player gains no new layer.
pub fn send(player_id: i32, n: i32, to: i32) {
    card_sdk::ctx::give_exile(player_id, n, to);
}

/// [除外] layers on the player.
pub fn get(player_id: i32) -> i32 {
    card_sdk::ctx::exile_of(player_id)
}

/// Drop every [除外] layer and forget the return tile.
pub fn clear(player_id: i32) {
    use card_sdk::ctx;
    let n = get(player_id);
    if n > 0 {
        ctx::state::add(player_id, KEY, -n);
    }
    ctx::state::set(player_id, KEY_TO, -1);
    ctx::state::set(player_id, KEY_MAIN, 0);
}
