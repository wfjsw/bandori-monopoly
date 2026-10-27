//! `RAS:EXIST` -- C# `CardExist` (MatchHost.cs:9321-9366).
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:EXIST`）:
//! > EXIST：
//! > 将此卡放置于自己场上，直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你，你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡
//!

use bandori_std::ctx::{self};
use card_sdk::abi::{ActionKind, FilterKind, Payee, Seat};
use card_sdk::{CardText};
use card_sdk::{key, Action, CardDef, Filter, Msg, On, RedirectValue};

pub const EXIST: CardDef = CardDef::new(
    "RAS:EXIST",
    &[
        On::Play("", None, exist),
        // The body-top `target == owner` early-out is the condition now (C#
        // `return target != Seat`): already aimed at us, nothing to retarget.
        On::Filter(Filter::Redirect("target != owner", None, redirect)),
        // 「造成影响」 latch: the redirect lands, then `Effect` fires at the
        // target window. Mark used there (Action) rather than inside the
        // Redirect Filter (`set_prop` is refuse_if_filter'd).
        On::Action(Action::Effect("target == owner && actor != owner", None, mark_used)),
        On::Action(Action::TurnStart("", Some(turn_start_guard), turn_start)),
    ],
)
// 规则书「直到自己的下一回合开始」: the `⌛1` marker the client shows.
.props(&[(card_sdk::abi::prop::TURNS_LEFT, 1)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"EXIST"#)
        .raw_name(r#"EXIST"#)
        .tags(&[])
        .text(CardText::para(r#"将此卡放置于自己场上，直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你，你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡"#))
        // sheet cell H3
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

const ID: &str = "RAS:EXIST";

/// C# `Mem["used"]` -- set by the redirect when this card retargeted a card.
/// Stored on the card instance (`FieldCard::props`), not in the player's
/// keyed-state map, so the write stays inside this rule's scope.
const PROP_USED: &str = "exist.used";

fn exist(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「将此卡放置于自己场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("exist_note")));
    // 规则书: 「直到自己的下一回合开始…你的下回合开始时将其翻入弃牌堆」 -- one
    // turn start away (the `⌛1` face marker is the static `TURNS_LEFT` prop).
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
/// Pure: the retarget rides `RedirectValue.payee`. The 「造成影响」 latch is
/// [`mark_used`] on `Effect`.
fn redirect(player_id: i32, v: RedirectValue) -> RedirectValue {
    // 规则书: 「场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你」
    // -- already aimed at us is the entry's condition (`target != owner`).
    RedirectValue {
        payee: Some(Payee::Seat(Seat(player_id))),
        ..v
    }
}

/// C# `CardExist.Used` -> `Mem["used"] = 1` -- this card retargeted a play.
/// Rides `Effect` (the target window's declaration, after the `redirect`
/// fold). The condition is "someone else's effect now targets us".
/// TODO(规则书)[judgement]: 「造成影响」 is taken to mean *the effect resolved*.
/// TODO(规则书): this marks used for any other-player Effect aimed at the
///   owner, not only ones that went through the Redirect fold. C# `Used()`
///   runs only when the redirect lands; a direct targeting of the owner also
///   matches here. Over-mark suppresses the extra draw (「没有造成影响」).
fn mark_used(_player_id: i32) -> card_sdk::Asked {
    ctx::set_prop(PROP_USED, 1);
    Ok(())
}

/// C# `CardExist.TurnStart` -> `End()` -- flip the card into the discard at the
/// start of this player's next turn; draw 1 when it never redirected anything.
/// Pure guard for [`turn_start`] -- the activation gate. `false`
/// means the card is not activated at all.
fn turn_start_guard(player_id: i32) -> bool {
    ctx::is_placed() && ctx::trigger::player_id() == player_id
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

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn exist_kani(player_id: i32) -> card_sdk::Asked {
    exist(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn redirect_kani(player_id: i32, v: card_sdk::RedirectValue) -> card_sdk::RedirectValue {
    redirect(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn mark_used_kani(player_id: i32) -> card_sdk::Asked {
    mark_used(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_start_kani(player_id: i32) -> card_sdk::Asked {
    turn_start(player_id)
}
