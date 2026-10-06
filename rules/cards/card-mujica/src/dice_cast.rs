//! `Mujica:骰子已经掷下` -- C# `CardDiceCast` (MatchHost.cs:5581-5653): [反击]
//! that places itself and locks other players out of hand [反击]s this turn.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:骰子已经掷下`）:
//! > 骰子已经掷下：
//! >  将此卡放置于自身场上，本回合内所有其他玩家无法从手牌中使用[反击]，回合结束后放入弃牌堆（此卡可以被反击）
//!

use card_sdk::abi::{ChainKind, HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

/// C# `H._noReactTurn` stand-in: the turn key of the player that locked hand
/// [反击]s this turn (C# clears it at that player's turn end).
const NO_REACT: &str = "diceCastActive";

const ID: &str = "Mujica:骰子已经掷下";

pub const DICE_CAST: CardDef = CardDef::new(
    "Mujica:骰子已经掷下",
    &[
        On::Play(Some(cant_play), play),
        On::CounterAct(&[ChainKind::Card], can_react, react),
        // C# `CardDiceCast.TurnEndAfter` -- off the field at the card's own turn end
        // (ABI v23 `TurnEndAfter`, matching the C# `Fx.TurnEndAfter` dispatch).
        On::Hook(&[HookKind::TurnEndAfter], |_| true, turn_end),
    ],
);

fn can_react(player_id: i32) -> bool {
    // 规则书: 「（此卡可以被反击）」 -- playable as a [反击] too (C#
    // `CardDiceCast.CanReact`: any other player's card play during your own turn,
    // unless the lock is already yours).
    if trigger::kind() != TriggerKind::Card || trigger::player_id() == player_id {
        return false;
    }
    // C# `H.State.turn == seat` -- only during your own turn.
    if ctx::turn_player() != player_id {
        return false;
    }
    // C# `H._noReactTurn != player_id` -- not already active; the `NO_REACT` turn-key
    // latch is the stand-in (set in `cast`).
    ctx::slot(player_id, NO_REACT) != ctx::turn_key()
}

/// C# `CardDiceCast.WhyNot` -- 「已经在场上了」 while the lock is yours.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::slot(player_id, NO_REACT) == ctx::turn_key() {
        return Some(Msg::new(key!("x_in_play")));
    }
    None // playable
}

fn play(player_id: i32) -> card_sdk::Asked {
    cast(player_id);
    Ok(())
}

fn react(player_id: i32) -> card_sdk::Asked {
    cast(player_id);
    Ok(())
}

fn cast(player_id: i32) {
    // 规则书: 「将此卡放置于自身场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("dice_cast_note")));
    // 规则书: 「本回合内所有其他玩家无法从手牌中使用[反击]」 -- C#
    // `H._noReactTurn = c.Seat` makes `H.CanReactNow` refuse every other player.
    // Latched to the turn key so it expires at the next turn (the C# clears it
    // in `TurnEndAfter`); the engine must honour it as the no-react gate.
    ctx::set_slot(player_id, NO_REACT, ctx::turn_key());
    ctx::log(
        player_id,
        &Msg::new(key!("dice_cast_log")).player_id("who", player_id),
    );
}

/// C# `CardDiceCast.TurnEndAfter` (MatchHost.cs:5629-5645) -- clear the
/// no-react latch and drop the card into the discard pile at the card's own
/// turn end while it is in play. Runs through the Fx hook dispatch at
/// `turnEndAfter` (ABI v23 `TriggerKind::TurnEndAfter`), so this is a field
/// effect, not a [反击].
fn turn_end(player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() != player_id || !ctx::is_placed() {
        return Ok(());
    }
    // C# `if (H._noReactTurn == Player) H._noReactTurn = -1`.
    if ctx::slot(player_id, NO_REACT) == ctx::turn_key() {
        ctx::set_slot(player_id, NO_REACT, 0);
    }
    // 规则书: 「回合结束后放入弃牌堆」 -- C# `H.Unplace(this, "discard", "回合结束")`.
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("dice_cast_ended")).player_id("who", player_id),
    );
    // TODO(规则书)[judgement]: 「（此卡可以被反击）」 -- the engine must still let other
    //   the clause under-specifies -- see the note above it
    // [反击]s answer this card's own play window (C# reaction chain plays
    // declared reactions in reverse before `Cast` sets the lock).
    Ok(())
}
