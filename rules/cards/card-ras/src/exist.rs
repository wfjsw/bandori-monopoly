//! `RAS:EXIST` -- C# `CardExist` (MatchHost.cs:9321-9366).
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:EXIST`）:
//! > EXIST：
//! > 将此卡放置于自己场上，直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你，你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡
//!

use card_sdk::abi::{GateKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const EXIST: CardDef = CardDef::new(
    "RAS:EXIST",
    &[
        On::Play(None, exist, ""),
        On::Gate(&[GateKind::Redirect], redirect),
        On::Hook(&[HookKind::TurnStart], Some(turn_start_guard), turn_start, ""),
    ],
);

const ID: &str = "RAS:EXIST";

/// C# `Mem["used"]` -- set by the redirect when this card retargeted a card.
/// Stored on the card instance (`FieldCard::props`), not in the player's
/// keyed-state map, so the write stays inside this rule's scope.
const PROP_USED: &str = "exist.used";

fn exist(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「将此卡放置于自己场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("exist_note")));
    ctx::set_prop(PROP_USED, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("exist_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardExist.Redirects` -> `Used()` -- while the card is placed, every
/// single-target play is retargeted at this seat (「包括其他玩家指向自身的卡」
/// covers another player's self-targeted plays too). The engine only raises
/// `redirect` at single-target `ctx::target` calls and applies the answer only
/// when it differs from both the intended target and the actor (C# `H.Target`'s
/// `card.Seat != p && card.Seat != c.Seat`).
fn redirect(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你」
    // -- C# `return target != Seat`: already aimed at us, nothing to retarget.
    if trigger::target() == player_id {
        return Ok(());
    }
    trigger::set_target(player_id);
    // C# calls `Used()` only when the redirect actually lands (`card.Seat != p
    // && card.Seat != c.Seat`) -- never when the play is our own.
    if trigger::player_id() != player_id {
        // TODO(规则书)[judgement]: 「造成影响」 is taken to mean *the effect resolved* -- a
        //   the clause under-specifies -- see the note above it
        // judgement call, not something the text settles. This is written at
        // **declaration** (the redirect re-names the recipient before the chain
        // opens), so a later counter that negates the effect still leaves the
        // flag set and the draw unearned. Moving it to settlement (a `target`
        // hook) would mean "nothing landed on me", which is the other reading.
        ctx::set_prop(PROP_USED, 1);
    }
    Ok(())
}

/// C# `CardExist.TurnStart` -> `End()` -- flip the card into the discard at the
/// start of this player's next turn; draw 1 when it never redirected anything.
/// Pure guard for [`turn_start`] -- the activation gate. `false`
/// means the card is not activated at all.
fn turn_start_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

fn turn_start(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「你的下回合开始时将其翻入弃牌堆」 -- C# `H.Unplace(this, "discard")`.
    let used = ctx::prop(PROP_USED);
    ctx::set_dest(ctx::Dest::Graveyard);
    // 规则书: 「若在此期间此卡没有造成影响，抽1张卡」 -- C# draws 1 only when
    // `Mem["used"]` was never set; the `redirect` hook above sets it (C#
    // `CardExist.Used` -> `Mem["used"] = 1`) when this card retargeted a play.
    if used == 0 {
        ctx::draw(player_id, 1)?;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("exist_ended")).player_id("who", player_id),
    );
    Ok(())
}
