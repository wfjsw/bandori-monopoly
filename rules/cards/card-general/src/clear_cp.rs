//! `通用:该清CP了` -- C# `CardCP` + `CPControl`: one [CP点] mark on an empty
//! tile, 6 on you; the marks spread for 2 turn starts and pay out on settle.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:该清CP了`）:
//! > 该清CP了：
//! > [特]：
//! >
//! > （1）此卡的[手]效果只有在自己[场上]拥有的小等于2个[CP点]时才可发动。
//! >
//! > （2）[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]及其产物将在相邻的没有[CP点]的格子添加1个[CP点]。
//! > [手]：
//! > 在任意一个没有角色和[CP点]的格子上添加1个[CP点]并在自己[场上]添加6个[CP点]。在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，[获得]800资金。
//!

use alloc::vec::Vec;
use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const CLEAR_CP: CardDef = CardDef::new("通用:该清CP了", &[
    On::Play(Some(cant_play), clear_cp),
    On::Hook(&[HookKind::TurnStart, HookKind::SettleAfter], react_guard, react)]);

/// Where the spread countdown is written down (C# `CPControl.Spread`).
const SLOT_SPREAD: &str = "clear_cp_spread";

/// 规则书[特]（1）: 「此卡的[手]效果只有在自己[场上]拥有的小等于2个[CP点]时才可发动」
/// -- C# `CardCP.WhyNot` refuses the card when `H.Tok(seat, "CP点") > 2`
/// (`场上的 [CP点] 超过 2 个时不能发动`). The counter this port writes is the
/// same `key!("clear_cp_tok")` the play adds.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::tok(player_id, key!("clear_cp_tok")) <= 2 {
        return None;
    }
    Some(Msg::new(key!("clear_cp_too_many")))
}

fn clear_cp(player_id: i32) {
    // 规则书[手]: 「在任意一个没有角色和[CP点]的格子上」
    let n = ctx::tile_count();
    let players = ctx::player_count();
    let mut free = Vec::new();
    for t in 0..n {
        let taken = (0..players).any(|s| !ctx::player_out(s) && ctx::player_pos(s) == t);
        if !taken && ctx::count_marks(t, key!("clear_cp_mark"), -2) == 0 {
            free.push(t);
        }
    }
    if free.is_empty() {
        return;
    }
    // 规则书[手]: 「添加1个[CP点]」
    let tile = ctx::ask_tile(
        player_id,
        &Msg::new(key!("clear_cp_ask_title")),
        &Msg::new(key!("clear_cp_ask_text")),
        &free,
    );
    ctx::add_mark(tile, player_id, key!("clear_cp_mark"), &Msg::new(key!("clear_cp_mark_note")));
    // 规则书[手]: 「并在自己[场上]添加6个[CP点]」
    ctx::add_tok(player_id, key!("clear_cp_tok"), 6, i32::MAX);
    ctx::log(player_id, &Msg::new(key!("clear_cp_placed")).tile("tile", tile).player_id("who", player_id));
    // 规则书（2）[特] + 规则书[手]: the settle / spread bodies run in `react`,
    // which the Fx hook dispatch runs at `turnStart` / `settleAfter`. C#
    // carries them on a standalone `CPControl` in `H._fx[i].extra`; the port's
    // persistent-effect carrier is the placed card.
    if !ctx::is_placed(player_id) {
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(player_id, "通用:该清CP了", &Msg::new(key!("clear_cp_note")));
    }
    // C# `cPControl.Spread = 2` -- reset the countdown on every play.
    ctx::set_slot(player_id, SLOT_SPREAD, 2);
}

/// `CPControl.TurnStart` / `CPControl.SettleAfter` (C# `Fx` overrides).
/// Runs through the Fx hook dispatch, so this is a field effect, not a [反击].
/// Pure guard for [`react`] -- the activation gate. `false`
/// means the card is not activated at all.
fn react_guard(player_id: i32) -> bool {
    ctx::is_placed(player_id)
}

fn react(player_id: i32) {
    match trigger::kind() {
        // 规则书（2）[特]: 「[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]
        // 及其产物将在相邻的没有[CP点]的格子添加1个[CP点]」 -- C#
        // `CPControl.TurnStart` (Spread = 2).
        TriggerKind::TurnStart => {
            if trigger::player_id() != player_id {
                return;
            }
            let spread = ctx::slot(player_id, SLOT_SPREAD);
            if spread <= 0 {
                return;
            }
            ctx::set_slot(player_id, SLOT_SPREAD, spread - 1);
            let n = ctx::tile_count();
            let mut marked: Vec<i32> = Vec::new();
            for t in 0..n {
                if ctx::count_marks(t, key!("clear_cp_mark"), player_id) > 0 {
                    marked.push(t);
                }
            }
            let mut added = 0;
            for &t in &marked {
                for &adj in &[(t + 1) % n, (t - 1 + n) % n] {
                    if ctx::count_marks(adj, key!("clear_cp_mark"), -2) == 0 {
                        ctx::add_mark(adj, player_id, key!("clear_cp_mark"), &Msg::new(key!("clear_cp_mark_note")));
                        added += 1;
                        break;
                    }
                }
            }
            if added > 0 {
                ctx::log(player_id, &Msg::new(key!("clear_cp_spread")).i("n", added as i64));
            }
        }
        // 规则书[手]: 「在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]
        // 1个[CP点]，[获得]800资金」 -- C# `CPControl.SettleAfter` -> `Clean`.
        TriggerKind::SettleAfter => {
            if trigger::player_id() != player_id {
                return;
            }
            let tile = trigger::tile();
            if tile < 0 {
                return;
            }
            if ctx::count_marks(tile, key!("clear_cp_mark"), -2) <= 0 {
                return;
            }
            if ctx::tok(player_id, key!("clear_cp_tok")) <= 0 {
                return;
            }
            // C# `tileMark.count--` (at most one [CP点] per tile) then `AddTok(-1)`.
            ctx::remove_marks(tile, key!("clear_cp_mark"), -2);
            ctx::add_tok(player_id, key!("clear_cp_tok"), -1, i32::MAX);
            // C# `H.GainR(Seat, Reward, ...)`; `Reward = c.N(0, 800)` -- the
            // PlayCtx number is the card's printed 800 (the C# stores it on
            // `CPControl.Reward` at Play; equivalent while `PlayCtx.Doubled`
            // is unported).
            ctx::gain(player_id, ctx::n(0, 800), &Msg::new(key!("clear_cp_clean")));
        }
        _ => {}
    }
}
