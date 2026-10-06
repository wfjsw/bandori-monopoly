//! `Sumimi:一人两个甜甜圈` -- C# `CardTwoDonuts` (MatchHost.cs:11119-11138): exile
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:一人两个甜甜圈`）:
//! > 一人两个甜甜圈：
//! >
//! > （1）获得[除外]直至你原本所在格子被其他玩家经过。
//! >
//! > （2）你原本所在格子被其他玩家经过时，可在那名玩家触发结算后选择传送至你原本所在格子（不包括）与那名玩家本次移动终点间的任一格并触发结算，之后你们各获得2火罐（超出上限的每个火罐转化为500资金）
//!
//! until the original tile is passed by someone else, then a settle-teleport and
//! 2 fire each. The C# `AiPlay` always returns false (no `H.AiPlay` hook).
//!
//! C# arms `H.ExtraOf<DonutFx>(i)` (a player attachment, not a placed card). The
//! hook surface only dispatches to *placed* cards, so the play body places this
//! card as the `DonutFx` stand-in and files it to the discard pile when the
//! effect finishes -- the same carrier pattern as the other `Fx` cards.

use alloc::vec::Vec;

use card_sdk::abi::{HookKind, MoveKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const TWO_DONUTS: CardDef = CardDef::new(
    "Sumimi:一人两个甜甜圈",
    &[
        On::Play(None, two_donuts),
        On::Hook(&[HookKind::PassTile, HookKind::SettleAfter], fx_guard, fx),
    ],
);

const ID: &str = "Sumimi:一人两个甜甜圈";
/// C# `DonutFx.Orig` -- the tile the exile waits on.
const SLOT_ORIG: &str = "two_donuts_orig";
/// C# `DonutFx._by` -- the passer whose settle opens the return window (-1 = none).
const SLOT_BY: &str = "two_donuts_by";

fn two_donuts(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    // 规则书（1）: 「获得[除外]直至你原本所在格子被其他玩家经过」
    // C# `H.GiveExile(i, 99, pos, i, CardName)` -- 99 layers so the per-turn
    // layer tick does not expire it; `DonutFx` clears the exile early.
    ctx::give_exile(player_id, 99, pos);
    ctx::set_slot(player_id, SLOT_ORIG, pos);
    ctx::set_slot(player_id, SLOT_BY, -1);
    // C# `H.ExtraOf<DonutFx>(i).Orig = pos` -- place as the live-effect carrier
    // so the `PassTile` / `SettleAfter` hooks below run (see the module doc).
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("two_donuts_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("two_donuts_exile"))
            .player_id("who", player_id)
            .tile("tile", pos),
    );
    // C# `CardTwoDonuts.AiPlay` returns false -- CardDef has no H.AiPlay hook.
    Ok(())
}

/// C# `DonutFx.PassTile` / `DonutFx.SettleAfter` -- run through the Fx hook
/// dispatch while this card is placed.
/// Pure guard for [`fx`] -- the activation gate. `false`
/// means the card is not activated at all.
fn fx_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn fx(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        // 规则书（1）: 「直至你原本所在格子被其他玩家经过」 -- C# `DonutFx.PassTile`
        // records the first other player that steps on `Orig`.
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
        }
        // 规则书（2）: 「你原本所在格子被其他玩家经过时，可在那名玩家触发结算后…」
        // -- C# `DonutFx.SettleAfter` -> `Back(m)` when that passer lands.
        TriggerKind::SettleAfter => {
            let by = ctx::slot(player_id, SLOT_BY);
            if by < 0 || trigger::player_id() != by {
                return Ok(());
            }
            back(player_id, by, trigger::move_dir());
        }
        _ => {}
    }
    Ok(())
}

/// C# `DonutFx.Back` -- end the exile, return to `Orig`, offer the in-between
/// settle-teleport, hand out 2 fire each, drop the attachment.
fn back(player_id: i32, by: i32, dir: i32) -> card_sdk::Asked {
    ctx::set_slot(player_id, SLOT_BY, -1);
    let orig = ctx::slot(player_id, SLOT_ORIG);
    // C# `me.exile = 0; me.exileTo = -1; me.pos = Orig` -- a plain position set,
    // no settle (`ctx::teleport_to` is `H.ForceTeleport(..., resolve: false)`).
    ctx::give_exile(player_id, -99, -1);
    ctx::teleport_to(player_id, orig);
    ctx::log(
        player_id,
        &Msg::new(key!("two_donuts_ended"))
            .player_id("who", player_id)
            .player_id("by", by),
    );
    // 规则书（2）: 「可在那名玩家触发结算后选择传送至你原本所在格子（不包括）与那名玩家
    // 本次移动终点间的任一格并触发结算」 -- C# `H.AskTileOf(..., allowNone: true)`
    // over the tiles from `Orig` (exclusive) along the passer's direction to
    // their end (`for i in 1..=n { t = (Orig + dir*i) mod n; ... break at their end }`),
    // then `H.Teleport(Seat, r.index, resolve: true)`. `allowNone` has no ctx
    // counterpart, so a yes/no stands in for the 「不传送」 branch (the
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
        // C# `H.AskTileOf(..., allowNone: true)` -- a yes/no stands in for allowNone.
        if ctx::ask_yes(player_id, &title, &Msg::new(key!("two_donuts_yes")))? {
            let to = ctx::ask_tile(
                player_id,
                &title,
                &Msg::new(key!("two_donuts_ask")).player_id("by", by),
                &tiles,
            )?;
            // C# `H.Teleport(Seat, r.index, resolve: true, ...)` (MatchHost.cs
            // DonutFx.Back) = `set_teleport_to(to)` + `set_resolve(true)` +
            // `card_move(player_id)`. C# calls `H.Teleport` (TeleportMove) rather
            // than `H.CardMove` (MainMoveAs), so this settle-teleport does not
            // consume the main move; `card_move` is the closest shape and only
            // marks `MainMoved` when the owner is the turn player.
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
        let got = ctx::gain_fire(who, 2, &Msg::new(key!("two_donuts_fire")));
        let missed = 2 - got;
        if missed > 0 {
            ctx::gain(
                who,
                missed * 500,
                &Msg::new(key!("two_donuts_overflow")).i("n", missed as i64),
            );
        }
    }
    // C# `H.RemoveExtra(this)` -- the stand-in leaves the field for the discard.
    ctx::set_dest(ctx::Dest::Graveyard);
    Ok(())
}
