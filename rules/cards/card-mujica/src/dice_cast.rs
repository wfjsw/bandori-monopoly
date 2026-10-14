//! `Mujica:骰子已经掷下` -- C# `CardDiceCast` (MatchHost.cs:5581-5653): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:骰子已经掷下`）:
//! > 骰子已经掷下：
//! >  将此卡放置于自身场上，本回合内所有其他玩家无法从手牌中使用[反击]，回合结束后放入弃牌堆（此卡可以被反击）
//!
//! that places itself and locks other players out of hand [反击]s this turn.

use card_sdk::abi::state_key;
use card_sdk::abi::{ChainKind, HookKind};
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

/// Once-per-turn placement latch. `cast` is reached from both `play` and the
/// played card's own `counteract` follow-up (`a_played_cards_own_counteract_runs_once`)
/// on a hand play; `is_placed` is uid-based and reads false on both (the hand
/// play has `current_uid = -1`), so a turn-key slot is the once-guard.
const PLACED: &str = "diceCastPlaced";

const ID: &str = "Mujica:骰子已经掷下";

pub const DICE_CAST: CardDef = CardDef::new(
    "Mujica:骰子已经掷下",
    &[
        On::Play("", Some(cant_play), play),
        // 「（此卡可以被反击）」 -- the play window stays open for counters
        // before `cast` sets the lock (the lock is armed in the body).
        On::Counteract(
            &[ChainKind::Card],
            "actor != owner && turn_player == owner && slot('noCounteractTurn') != turn_key",
            None,
            counteract,
        ),
        // C# `CardDiceCast.TurnEndAfter` -- off the field at the card's own turn end
        // (ABI v23 `TurnEndAfter`, matching the C# `Fx.TurnEndAfter` dispatch).
        On::Hook(&[HookKind::TurnEndAfter], "actor == owner && card.placed", None, turn_end),
    ],
)
.props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"骰子已经掷下"#)
        .raw_name(r#"骰子已经掷下"#)
        .tags(&[r#"反击"#])
        .text(r#"将此卡放置于自身场上，本回合内所有其他玩家无法从手牌中使用[反击]，回合结束后放入弃牌堆（此卡可以被反击）"#)
        .cell(r#"J6"#)
        .fields(&[("band", r#"Ave Mujica"#), ("owner", "")]));

/// C# `CardDiceCast.WhyNot` -- 「已经在场上了」 while the lock is yours.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::slot(player_id, PLACED) == ctx::turn_key() {
        return Some(Msg::new(key!("x_in_play")));
    }
    None // playable
}

fn play(player_id: i32) -> card_sdk::Asked {
    cast(player_id);
    Ok(())
}

/// Using this card *as* a [反击] against someone else. The played card's own
/// follow-up also reaches this entry (unguarded, see
/// `a_played_cards_own_counteract_runs_once`); that is the same activation as
/// `play` and must not re-cast -- and must not arm the no-counteract lock
/// before the play's own [反击] window (「此卡可以被反击」).
fn counteract(player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() == player_id {
        return Ok(());
    }
    cast(player_id);
    Ok(())
}

fn cast(player_id: i32) {
    // 规则书: 「将此卡放置于自身场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    // Place at most once per turn (see `PLACED`). `is_placed` is uid-based and
    // reads false on a hand play, so it cannot be the once-guard here.
    if ctx::slot(player_id, PLACED) == ctx::turn_key() {
        return;
    }
    ctx::set_slot(player_id, PLACED, ctx::turn_key());
    ctx::place_card(player_id, ID, &Msg::new(key!("dice_cast_note")));
    // 规则书: 「本回合内所有其他玩家无法从手牌中使用[反击]」 -- the engine's
    // `can_counteract_now` honours `state_key::NO_COUNTERACT_TURN` as the
    // no-hand-counteract gate (every seat but the latch owner). Latched to the
    // turn key so it expires at the next turn.
    ctx::set_slot(player_id, state_key::NO_COUNTERACT_TURN, ctx::turn_key());
    ctx::log(
        player_id,
        &Msg::new(key!("dice_cast_log")).player_id("who", player_id),
    );
}

/// C# `CardDiceCast.TurnEndAfter` (MatchHost.cs:5629-5645) -- clear the
/// no-counteract latch and drop the card into the discard pile at the card's own
/// turn end while it is in play. Runs through the Fx hook dispatch at
/// `turnEndAfter` (ABI v23 `TriggerKind::TurnEndAfter`), so this is a field
/// effect, not a [反击].
fn turn_end(player_id: i32) -> card_sdk::Asked {
    // C# `if (H._noCounteractTurn == Player) H._noCounteractTurn = -1`.
    if ctx::slot(player_id, state_key::NO_COUNTERACT_TURN) == ctx::turn_key() {
        ctx::set_slot(player_id, state_key::NO_COUNTERACT_TURN, 0);
    }
    if ctx::slot(player_id, PLACED) == ctx::turn_key() {
        ctx::set_slot(player_id, PLACED, 0);
    }
    // 规则书: 「回合结束后放入弃牌堆」 -- C# `H.Unplace(this, "discard", "回合结束")`.
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("dice_cast_ended")).player_id("who", player_id),
    );
    // TODO(规则书)[judgement]: 「（此卡可以被反击）」 -- the engine must still let other
    //   the clause under-specifies -- see the note above it
    // [反击]s answer this card's own play window (C# counteraction chain plays
    // declared counteractions in reverse before `Cast` sets the lock).
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_end_kani(player_id: i32) -> card_sdk::Asked {
    turn_end(player_id)
}
