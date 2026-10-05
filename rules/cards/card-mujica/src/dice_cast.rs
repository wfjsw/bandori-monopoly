//! `Mujica:骰子已经掷下` -- C# `CardDiceCast` (MatchHost.cs:5581-5653): [反击]
//! that places itself and locks other players out of hand [反击]s this turn.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:骰子已经掷下`）:
//! > 骰子已经掷下：
//! >  将此卡放置于自身场上，本回合内所有其他玩家无法从手牌中使用[反击]，回合结束后放入弃牌堆（此卡可以被反击）
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

/// C# `H._noReactTurn` stand-in: the turn key of the seat that locked hand
/// [反击]s this turn (C# clears it at that seat's turn end).
const NO_REACT: &str = "diceCastActive";

pub const DICE_CAST: CardDef = CardDef {
    id: "Mujica:骰子已经掷下",
    play: Some(play),
    can_react: Some(can_react),
    react: Some(react),
    why_not: Some(why_not),
};

fn can_react(seat: i32) -> bool {
    // 规则书: 「（此卡可以被反击）」 -- playable as a [反击] too (C#
    // `CardDiceCast.CanReact`: any other seat's card play during your own turn,
    // unless the lock is already yours).
    if trigger::kind() != TriggerKind::Card || trigger::seat() == seat {
        return false;
    }
    // C# `H.State.turn == seat` -- only during your own turn.
    if ctx::turn_seat() != seat {
        return false;
    }
    // C# `H._noReactTurn != seat` -- not already active; the `NO_REACT` turn-key
    // latch is the stand-in (set in `cast`).
    ctx::slot(seat, NO_REACT) != ctx::turn_key()
}

/// C# `CardDiceCast.WhyNot` -- 「已经在场上了」 while the lock is yours.
fn why_not(seat: i32) -> Option<Msg> {
    if ctx::slot(seat, NO_REACT) == ctx::turn_key() {
        return Some(Msg::new(key!("x_in_play")));
    }
    None // playable
}

fn play(seat: i32) {
    cast(seat);
}

fn react(seat: i32) {
    cast(seat);
}

fn cast(seat: i32) {
    // 规则书: 「将此卡放置于自身场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Mujica:骰子已经掷下", &Msg::new(key!("dice_cast_note")));
    // 规则书: 「本回合内所有其他玩家无法从手牌中使用[反击]」 -- C#
    // `H._noReactTurn = c.Seat` makes `H.CanReactNow` refuse every other seat.
    // Latched to the turn key so it expires at the next turn (the C# clears it
    // in `TurnEndAfter`); the engine must honour it as the no-react gate.
    ctx::set_slot(seat, NO_REACT, ctx::turn_key());
    ctx::log(seat, &Msg::new(key!("dice_cast_log")).seat("who", seat));
    // TODO(规则书): 「回合结束后放入弃牌堆」 -- needs the Fx.TurnEndAfter hook
    // (C# `CardDiceCast.TurnEndAfter` -> `H.Unplace(this, "discard", "回合结束")`)
    // plus clearing the `NO_REACT` latch.
    // TODO(规则书): 「（此卡可以被反击）」 -- the engine must still let other
    // [反击]s answer this card's own play window (C# reaction chain plays
    // declared reactions in reverse before `Cast` sets the lock).
}