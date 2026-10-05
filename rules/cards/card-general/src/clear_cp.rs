//! `通用:该清CP了` -- C# `CardCP`: one [CP点] mark on an empty tile, 6 on you.
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

use card_sdk::{ctx, key, CardDef, Msg};
use alloc::vec::Vec;

pub const CLEAR_CP: CardDef = CardDef {
    id: "通用:该清CP了",
    play: Some(clear_cp),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// 规则书[特]（1）: 「此卡的[手]效果只有在自己[场上]拥有的小等于2个[CP点]时才可发动」
/// -- C# `CardCP.WhyNot` refuses the card when `H.Tok(seat, "CP点") > 2`
/// (`场上的 [CP点] 超过 2 个时不能发动`). The counter this port writes is the
/// same `key!("clear_cp_tok")` the play adds.
fn why_not(seat: i32) -> Option<Msg> {
    if ctx::tok(seat, key!("clear_cp_tok")) <= 2 {
        return None;
    }
    Some(Msg::new(key!("clear_cp_too_many")))
}

fn clear_cp(seat: i32) {
    // 规则书[手]: 「在任意一个没有角色和[CP点]的格子上」
    let n = ctx::tile_count();
    let seats = ctx::seat_count();
    let mut free = Vec::new();
    for t in 0..n {
        let taken = (0..seats).any(|s| !ctx::seat_out(s) && ctx::seat_pos(s) == t);
        if !taken && ctx::count_marks(t, key!("clear_cp_mark"), -2) == 0 {
            free.push(t);
        }
    }
    if free.is_empty() {
        return;
    }
    // 规则书[手]: 「添加1个[CP点]」
    let tile = ctx::ask_tile(
        seat,
        &Msg::new(key!("clear_cp_ask_title")),
        &Msg::new(key!("clear_cp_ask_text")),
        &free,
    );
    ctx::add_mark(tile, seat, key!("clear_cp_mark"), &Msg::new(key!("clear_cp_mark_note")));
    // 规则书[手]: 「并在自己[场上]添加6个[CP点]」
    ctx::add_tok(seat, key!("clear_cp_tok"), 6, i32::MAX);
    ctx::log(seat, &Msg::new(key!("clear_cp_placed")).tile("tile", tile).seat("who", seat));
    // TODO(规则书)（2）: 「[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]
    // 及其产物将在相邻的没有[CP点]的格子添加1个[CP点]」 -- needs the `Fx.TurnStart`
    // hook (C# `CPControl.TurnStart`, Spread = 2).
    // TODO(规则书)[手]: 「在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]
    // 1个[CP点]，[获得]800资金」 -- needs the `Fx.SettleAfter` hook (C#
    // `CPControl.SettleAfter` -> `Clean`).
}