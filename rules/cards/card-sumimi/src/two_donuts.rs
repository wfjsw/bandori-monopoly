//! `Sumimi:一人两个甜甜圈` -- 一人两个甜甜圈
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:一人两个甜甜圈`）:
//! > 一人两个甜甜圈：
//! >
//! > （1）获得[除外]直至你原本所在格子被其他玩家经过。
//! > （2）你原本所在格子被其他玩家经过时，可在那名玩家触发结算后选择传送至你原本所在格子（不包括）与那名玩家本次移动终点间的任一格并触发结算，之后你们各获得2火罐（超出上限的每个火罐转化为500资金）
//!
//! （1） is a **duration** grant (「获得[除外]直至X」): a continuous exile
//! until the original tile is passed, not a layer stack. The turn-start layer
//! tick does not apply to it (`rules_status::exile::hold`); the pass ends it
//! (`rules_status::exile::clear`). Then (2) opens the settle-teleport after
//! the passer's 触发结算, and the fire pots land unexiled.
//!
//! The hook surface only dispatches to *placed* cards, so the play body places
//! this card as the live-effect carrier and files it to the discard pile when
//! the effect finishes -- the same carrier pattern as the other `Fx` cards.

use alloc::vec::Vec;

use card_sdk::abi::{ActionKind, TriggerKind};
use bandori_std::ctx::{self, trigger};
use card_sdk::{Action, CardDef, key, Msg, On};

pub const TWO_DONUTS: CardDef = CardDef::new(
    "Sumimi:一人两个甜甜圈",
    &[
        On::Play("", None, None, two_donuts),
        // `card.placed` is the uniform gate; per-kind clauses (the other-passer
        // latch, the `move_dir` read) stay in the arms below.
        On::Action(Action::PassTile("card.placed", None, None, fx)),
        On::Action(Action::SettleAfter("card.placed", None, None, fx)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"一人两个甜甜圈"#)
        .raw_name(r#"一人两个甜甜圈"#)
        .tags(&[])
        .text(r#"（1）获得[除外]直至你原本所在格子被其他玩家经过。（2）你原本所在格子被其他玩家经过时，可在那名玩家触发结算后选择传送至你原本所在格子（不包括）与那名玩家本次移动终点间的任一格并触发结算，之后你们各获得2火罐（超出上限的每个火罐转化为500资金）"#)
        // sheet cell L4
        .fields(&[("group", r#"Sumimi"#), ("owner", "")]));

const ID: &str = "Sumimi:一人两个甜甜圈";
/// The tile the exile waits on.
const SLOT_ORIG: &str = "two_donuts_orig";
/// The passer whose settle opens the return window (-1 = none).
const SLOT_BY: &str = "two_donuts_by";

fn two_donuts(player_id: i32) -> card_sdk::Asked {
    // 「该效果生效时无法重复获得该效果」 bars the (1) acquisition while the
    // user is already exiled.
    // TODO(规则书) DONUT-VOID-01: the book does not say what the card does when
    // the acquisition is barred; we treat the whole effect as void (no carrier,
    // no slots, no log). The card is still spent (the play itself is done).
    if rules_status::exile::get(player_id) > 0 {
        return Ok(());
    }
    let pos = ctx::player_pos(player_id);
    // 规则书（1）: 「获得[除外]直至你原本所在格子被其他玩家经过」 -- a duration
    // grant (held exile, no layer tick). The pass ends it (see `fx`).
    rules_status::exile::hold(player_id, pos);
    ctx::set_slot(player_id, SLOT_ORIG, pos);
    ctx::set_slot(player_id, SLOT_BY, -1);
    // Place as the live-effect carrier so the `PassTile` / `SettleAfter` hooks
    // below run (see the module doc).
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("two_donuts_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("two_donuts_exile"))
            .player_id("who", player_id)
            .tile("tile", pos),
    );
    Ok(())
}

/// The `PassTile` / `SettleAfter` hook pair, while this card is placed.
fn fx(player_id: i32) -> card_sdk::Asked {
    // `card.placed` is the pre.
    match trigger::kind() {
        // 规则书（1）: 「直至你原本所在格子被其他玩家经过」 -- the exile ends the
        // moment the original tile is passed, not at the passer's settle. Latch
        // the first other player that steps on `Orig` and end the exile here.
        TriggerKind::PassTile => {
            if trigger::player_id() == player_id {
                return Ok(());
            }
            if trigger::tile() != ctx::slot(player_id, SLOT_ORIG) {
                return Ok(());
            }
            if ctx::slot(player_id, SLOT_BY) >= 0 {
                return Ok(());
            }
            ctx::set_slot(player_id, SLOT_BY, trigger::player_id());
            rules_status::exile::clear(player_id);
            ctx::log(
                player_id,
                &Msg::new(key!("two_donuts_ended"))
                    .player_id("who", player_id)
                    .player_id("by", trigger::player_id()),
            );
        }
        // 规则书（2）: 「你原本所在格子被其他玩家经过时，可在那名玩家触发结算后…」
        // -- when that passer lands, offer the settle-teleport.
        TriggerKind::SettleAfter => {
            let by = ctx::slot(player_id, SLOT_BY);
            if by < 0 || trigger::player_id() != by {
                return Ok(());
            }
            back(player_id, by, trigger::move_dir())?;
        }
        _ => {}
    }
    Ok(())
}

/// 规则书（2）: the optional settle-teleport onto a square between `Orig`
/// (exclusive) and the passer's 移动终点, then the fire pots of (3) for both
/// players. The exile already ended at the pass (clause (1) 「直至」), so the
/// teleport and the money run unexiled.
fn back(player_id: i32, by: i32, dir: i32) -> card_sdk::Asked {
    ctx::set_slot(player_id, SLOT_BY, -1);
    let orig = ctx::slot(player_id, SLOT_ORIG);
    // The holder sat out the exile on `Orig` (「无法移动」); put them back there
    // without settle so the (2) window and its stream start from that square.
    ctx::teleport_to(player_id, orig);
    // 规则书（2）: 「可在那名玩家触发结算后选择传送至你原本所在格子（不包括）与那名玩家
    // 本次移动终点间的任一格并触发结算」 -- `allowNone` has no ctx counterpart,
    // so a yes/no stands in for the 「不传送」 branch (the
    // `card-general/tsugu_ycm` pattern).
    let n = ctx::tile_count();
    let mut tiles: Vec<i32> = Vec::new();
    if n > 0 && orig >= 0 {
        let end = ctx::player_pos(by);
        let d = if dir < 0 { -1 } else { 1 };
        for i in 1..=n {
            let t = ((orig + d * i) % n + n) % n;
            tiles.push(t);
            if t == end {
                break;
            }
        }
    }
    if !tiles.is_empty() {
        let title = Msg::new(key!("two_donuts_title"));
        if ctx::ask_yes(player_id, &title, &Msg::new(key!("two_donuts_yes")))? {
            let to = ctx::ask_tile(
                player_id,
                &title,
                &Msg::new(key!("two_donuts_ask")).player_id("by", by),
                &tiles,
            )?;
            // `card_move` is the closest shape to `H.Teleport(..., resolve: true)`
            // and only marks `MainMoved` when the owner is the turn player, so
            // this settle-teleport does not consume the main move.
            ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
            ctx::plan::set_teleport_to(to);
            ctx::plan::set_resolve(true);
            ctx::card_move(player_id);
            ctx::log(
                player_id,
                &Msg::new(key!("two_donuts_moved"))
                    .player_id("who", player_id)
                    .tile("tile", to),
            );
        }
    }
    // 规则书（2）: 「之后你们各获得2火罐（超出上限的每个火罐转化为500资金）」
    for who in [player_id, by] {
        if ctx::player_out(who) {
            continue;
        }
        let got = bandori_std::fire::gain(who, 2, &Msg::new(key!("two_donuts_fire")))?;
        let missed = 2 - got;
        if missed > 0 {
            ctx::gain(
                who,
                missed * 500,
                &Msg::new(key!("two_donuts_overflow")).i("n", missed as i64),
            )?;
        }
    }
    // The stand-in leaves the field for the discard.
    ctx::set_dest(ctx::Dest::Graveyard);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn two_donuts_kani(player_id: i32) -> card_sdk::Asked {
    two_donuts(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn fx_kani(player_id: i32) -> card_sdk::Asked {
    fx(player_id)
}
