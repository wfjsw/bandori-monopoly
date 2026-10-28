//! `CRYCHIC:一起演奏音乐的命运共同体` -- C# `CardFateTogether` (MatchHost.cs:3050-3061):
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:一起演奏音乐的命运共同体`）:
//! > 一起演奏音乐的命运共同体
//! >  ：
//! > （1）[手] 直到你的下回合开始，每当场上任意格子发生一次收款时，你移动到你前方的下一个属于行动序列后一名玩家的格子（不触发结算）。
//! > （2）若此卡打出后
//! > （1）效果未产生作用，将此卡返回手牌（不触发乐队技能的
//! > （3）效果）。
//!
//! until your next turn start, hop to the next player's tile on every collection
//! (rent legs only at present -- see the TODO at the hook).

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, TriggerKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const FATE_TOGETHER: CardDef = CardDef::new(
    "CRYCHIC:一起演奏音乐的命运共同体",
    &[
        On::Play("", None, fate_together),
        // 规则书（1）: 「每当场上任意格子发生一次收款时」 -- the placed card's
        // collection hook.
        On::Action(Action::PayAfter("", Some(counteract_guard), counteract)),
        // 规则书（1）: 「直到你的下回合开始」 -- (1) ends and (2) is decided at the
        // owner's next turn start.
        On::Action(Action::TurnStart("", Some(counteract_guard), counteract)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"一起演奏音乐的命运共同体"#)
        .raw_name(r#"一起演奏音乐的命运共同体"#)
        .tags(&[r#"手"#])
        .text(CardText::items(&[
            it(r#"[手] 直到你的下回合开始，每当场上任意格子发生一次收款时，你移动到你前方的下一个属于行动序列后一名玩家的格子（不触发结算）。"#),
            it(r#"若此卡打出后（1）效果未产生作用，将此卡返回手牌（不触发乐队技能的（3）效果）。"#),
        ]))
        // sheet cell M9
        .fields(&[("group", r#"CRYCHIC"#), ("owner", "")]));

const ID: &str = "CRYCHIC:一起演奏音乐的命运共同体";

/// C# `FateFx.Hit` -- did (1) produce a result? An instance prop, so two copies
/// each carry their own bit (a shared player slot made them share one).
const PROP_HIT: &str = "fate_hit";

fn fate_together(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）[手]: 「直到你的下回合开始，每当场上任意格子发生一次收款时，你移动到你
    // 前方的下一个属于行动序列后一名玩家的格子（不触发结算）。」 -- C# arms `FateFx`
    // (`H.ExtraOf<FateFx>(c.Seat)`, `Hit = false`, `Card = c.Id`). The free-floating
    // `FateFx` maps to this card in play: the `PayAfter` / `TurnStart` hooks below
    // fire while it is placed.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        ID,
        &Msg::new(key!("fate_together_note")).player_id("who", player_id),
    );
    // After `place_card`, so the running instance is the freshly placed one.
    ctx::set_prop(PROP_HIT, 0);
    Ok(())
}

/// C# `FateFx.PayAfter` / `FateFx.TurnStart`.
/// Pure guard for [`counteract`] -- the activation gate. `false`
/// means the card is not activated at all.
fn counteract_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        // 规则书（1）: 「每当场上任意格子发生一次收款时，你移动到你前方的下一个属于行动序列
        // 后一名玩家的格子（不触发结算）。」 -- C# `FateFx.PayAfter`:
        // `p.paid && p.IsRent && p.finalGain > 0`.
        TriggerKind::PayAfter => {
            // TODO(规则书)（每当场上任意格子发生一次收款时）: any collection on any
            //   tile -- we fire only on rent legs with value > 0 (OPEN-收款).
            if !trigger::pay_is_rent() || trigger::value() <= 0 {
                return Ok(());
            }
            if ctx::player_out(player_id) {
                return Ok(());
            }
            // The action-order-next player (C# `H.Neighbor(Seat, 1)`).
            let num = ctx::neighbor(player_id, 1);
            if num < 0 || num == player_id {
                return Ok(());
            }
            // 规则书（1）: 「你前方的下一个属于行动序列后一名玩家的格子」 -- the first
            // tile whose owner is `num`. The scan includes the player's own tile
            // (0) so a replayed pass -- the body re-runs from the landing tile and
            // the `card_move` answer is only replayed, not run again -- still finds
            // the destination the first pass picked instead of falling out of the
            // loop.
            let n = ctx::tile_count();
            let pos = ctx::player_pos(player_id);
            if n <= 0 || pos < 0 {
                return Ok(());
            }
            // TODO(规则书)（你前方的下一个属于行动序列后一名玩家的格子）:
            //   OPEN-行动序列后一名 does not say what happens when that player owns no
            //   tile ahead -- we walk the whole ring and do nothing on no match.
            let Some((i, tile)) = (0..n)
                .map(|i| (i, (pos + i).rem_euclid(n)))
                .find(|&(_, t)| ctx::tile_owner(t) == num)
            else {
                return Ok(());
            };
            // 规则书（1）: 「（不触发结算）」 -- C# `H.Walk(Seat, i, resolve: false)`
            //   = `set_steps(i)` + `set_resolve(false)` + `card_move(player_id)`.
            ctx::plan::set_steps(i);
            ctx::plan::set_resolve(false);
            // 规则书（2）: 「（1）效果未产生作用」 -- record the hit *before* the move:
            // the body re-runs from the landing tile and re-finds it at 0 above,
            // so the re-run reaches this write again.
            ctx::set_prop(PROP_HIT, 1);
            // TODO(规则书)(ABI)（你移动到你前方的下一个属于行动序列后一名玩家的格子）:
            //   the hop is an effect move, not the main move -- `card_move`
            //   treats it as the main move (refused once the main move is
            //   spent, and it consumes the main move when it is not). Its bool
            //   is the abnormal-gate answer, not whether the move ran, so a
            //   spent main move still reads as a hit. Needs a `plan::set_forced`
            //   export.
            if !ctx::card_move(player_id) {
                // The abnormal gate blocked the move: (1) produced nothing.
                ctx::set_prop(PROP_HIT, 0);
                return Ok(());
            }
            ctx::log(
                player_id,
                &Msg::new(key!("fate_together_moved"))
                    .player_id("who", player_id)
                    .tile("tile", tile),
            );
        }
        // 规则书（1）: 「直到你的下回合开始」 -- C# `FateFx.TurnStart` removes the effect at
        // this player's next turn start.
        // 规则书（2）: 「若此卡打出后（1）效果未产生作用，将此卡返回手牌（不触发乐队技能的（3）效果）。」
        // -- C# `if (!Hit && discard.Remove(Card)) hand.Add(Card)`.
        TriggerKind::TurnStart => {
            if trigger::player_id() != player_id {
                return Ok(());
            }
            // TODO(规则书)（不触发乐队技能的（3）效果）: the return must not read as
            //   a hand gain for band (3) -- we do not resync the band's hand
            //   watermark, so the next draw still sees a 4→5 crossing (before 4,
            //   now 6) and adds the crystal.
            if ctx::prop(PROP_HIT) == 0 {
                ctx::set_dest(ctx::Dest::Hand);
                ctx::log(
                    player_id,
                    &Msg::new(key!("fate_together_returned")).player_id("who", player_id),
                );
            } else {
                ctx::set_dest(ctx::Dest::Graveyard);
            }
        }
        _ => {}
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn fate_together_kani(player_id: i32) -> card_sdk::Asked {
    fate_together(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
