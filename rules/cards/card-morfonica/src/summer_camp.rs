//! `Mor:夏日合宿` -- C# `CardSummerCamp` (MatchHost.cs:4584-4643): stay untargetable
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:夏日合宿`）:
//! > 夏日合宿： 
//! >  打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌
//!
//! by others until your next turn, then draw if nothing was blocked.

use card_sdk::abi::{GateKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "Mor:夏日合宿";

/// C# `Mem["blocked"]` -- how many targetings this card negated.
const SLOT_BLOCKED: &str = "summer_camp_blocked";

pub const SUMMER_CAMP: CardDef = CardDef::new(
    "Mor:夏日合宿",
    &[
        On::Play("", None, summer_camp),
        On::Gate(&[GateKind::Untargetable], untargetable),
        On::Hook(&[HookKind::TurnStart], "actor == owner && card.placed", None, turn_start),
    ],
);

fn summer_camp(player_id: i32) -> card_sdk::Asked {
    // C# `AiPlay => false` -- bots never play this card; `CardDef` has no AiPlay
    // hook yet, so a bot prompt will still offer it.
    // 规则书: 「打出此卡，直到下个自己的回合开始前」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("summer_camp_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("summer_camp_placed")).player_id("who", player_id),
    );
    // 规则书: 「你只会被自己发动的效果指定」 -- the Untargetable hook below refuses
    //   any `by != player_id` and counts negations in `SLOT_BLOCKED`.
    // The Fx.TurnStart hook below ends the effect on the owner's next turn.
    Ok(())
}

/// `Fx.Untargetable` (C# `CardSummerCamp.Untargetable`): while placed, only the
/// owner's own effects may target them. Counts negations for the draw gate.
fn untargetable(player_id: i32) -> card_sdk::Asked {
    // The hook runs on every placed card across all players; only guard our own
    // seat (`t.player` = the target).
    if trigger::player_id() != player_id {
        return Ok(());
    }
    // 规则书: 「只会被自己发动的效果指定」 -- C# `if (seat != Seat || by == Seat)
    //   return false;` self-targeting passes (the engine already allows it).
    let by = trigger::by_card();
    if by.is_none_or(|b| b == player_id) {
        return Ok(());
    }
    // C# `Mem["blocked"] = Blocked + 1; return true;`.
    ctx::inc_slot(player_id, SLOT_BLOCKED, 1);
    trigger::set_cancelled();
    Ok(())
}

/// `Fx.TurnStart` (C# `CardSummerCamp.TurnStart` -> `End`): the effect ends at
/// the owner's next turn start; draw 1 when nothing was negated.
fn turn_start(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && card.placed` is the pre.
    // 规则书: 「当此卡效果结束」 -- C# `H.Unplace(this, "discard", "效果结束了")`.
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("summer_camp_end")).player_id("who", player_id),
    );
    // 规则书: 「你没有因为此卡效果无效化任何影响则抽一张牌」 -- C# `End`:
    // `H.DrawR(Seat, 1, ...)` when `Blocked == 0`.
    let blocked = ctx::slot(player_id, SLOT_BLOCKED);
    ctx::set_slot(player_id, SLOT_BLOCKED, 0);
    if blocked == 0 {
        ctx::draw(player_id, 1)?;
        ctx::log(
            player_id,
            &Msg::new(key!("summer_camp_draw")).player_id("who", player_id),
        );
    }
    Ok(())
}
