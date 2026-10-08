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
//! until your next turn start, hop to the next player's tile on every rent collection.

use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const FATE_TOGETHER: CardDef = CardDef::new(
    "CRYCHIC:一起演奏音乐的命运共同体",
    &[
        On::Play("", None, fate_together),
        On::Hook(&[HookKind::PayAfter, HookKind::TurnStart], "", Some(counteract_guard), counteract),
    ],
);

const ID: &str = "CRYCHIC:一起演奏音乐的命运共同体";

/// C# `FateFx.Hit` -- did (1) produce a result?
const SLOT_HIT: &str = "fate_hit";

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
    ctx::set_slot(player_id, SLOT_HIT, 0);
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
            // 规则书（1）: 「你前方的下一个属于行动序列后一名玩家的格子」 -- the first tile
            // forward whose owner is `num`.
            let n = ctx::tile_count();
            let pos = ctx::player_pos(player_id);
            if n <= 0 || pos < 0 {
                return Ok(());
            }
            for i in 1..n {
                let tile = (pos + i).rem_euclid(n);
                if ctx::tile_owner(tile) == num {
                    // 规则书（1）: 「（不触发结算）」 -- C# `H.Walk(Seat, i, resolve: false)`
                    //   = `set_steps(i)` + `set_resolve(false)` + `card_move(player_id)`.
                    ctx::plan::set_steps(i);
                    ctx::plan::set_resolve(false);
                    ctx::card_move(player_id);
                    ctx::set_slot(player_id, SLOT_HIT, 1);
                    ctx::log(
                        player_id,
                        &Msg::new(key!("fate_together_moved"))
                            .player_id("who", player_id)
                            .tile("tile", tile),
                    );
                    return Ok(());
                }
            }
        }
        // 规则书（1）: 「直到你的下回合开始」 -- C# `FateFx.TurnStart` removes the effect at
        // this player's next turn start.
        // 规则书（2）: 「若此卡打出后（1）效果未产生作用，将此卡返回手牌」 -- C#
        // `if (!Hit && discard.Remove(Card)) hand.Add(Card)`.
        TriggerKind::TurnStart => {
            if trigger::player_id() != player_id {
                return Ok(());
            }
            if ctx::slot(player_id, SLOT_HIT) == 0 {
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
