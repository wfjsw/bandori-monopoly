//!
//! 规则书（docs/rulebook/cards.json, id `RAS:EXIST`）:
//! > EXIST：
//! > 将此卡放置于自己场上，直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你，你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡
//!

use bandori_std::ctx::{self};
use card_sdk::abi::{ActionKind, FilterKind, Payee, Seat};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Filter, Msg, On, RedirectValue};

pub const EXIST: CardDef = CardDef::new(
    "RAS:EXIST",
    &[
        On::Play("", None, exist),
        On::Filter(Filter::Redirect("target != owner", None, redirect)),
        // 「造成影响」 latch: the redirect lands, then `Effect` fires at the
        // target window. Mark used there (Action) rather than inside the
        // Redirect Filter (`set_prop` is refuse_if_filter'd).
        // 规则书: 「若在此期间此卡没有造成影响，抽1张卡」
        On::Action(Action::Effect("target == owner && actor != owner", None, mark_used)),
        // 规则书: 「你的下回合开始时将其翻入弃牌堆」
        On::Action(Action::TurnStart("", Some(turn_start_guard), turn_start)),
    ],
)
// 规则书「直到自己的下一回合开始」: the `⌛1` marker the client shows.
.props(&[(bandori_std::marker::TURNS_LEFT, 1)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"EXIST"#)
        .raw_name(r#"EXIST"#)
        .tags(&[])
        .text(CardText::para(r#"将此卡放置于自己场上，直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你，你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡"#))
        // sheet cell H3
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

const ID: &str = "RAS:EXIST";

/// Stored on the card instance (`FieldCard::props`), not in the player's
/// keyed-state map, so the write stays inside this rule's scope.
const PROP_USED: &str = "exist.used";

fn exist(player_id: i32) -> card_sdk::Asked {
    ctx::set_fate(ctx::Fate::Field);
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

/// TODO(规则书)（若在此期间此卡没有造成影响）: TODO(规则书): Rides `Effect` (the target window's
/// declaration, after the `redirect` fold). The condition is "someone else's
/// effect now targets us". 「造成影响」 is latched on the effect **declaration** here,
/// before any counter settles (C-Q8 / WINDOW-03); the book asks whether the card
/// had an influence. this marks used for any other-player Effect aimed at the
/// owner, not only ones that went through the Redirect fold. Over-mark suppresses
/// the extra draw (「没有造成影响」).
fn mark_used(_player_id: i32) -> card_sdk::Asked {
    // 规则书: 「若在此期间此卡没有造成影响，抽1张卡」
    ctx::set_prop(PROP_USED, 1);
    Ok(())
}

/// Pure guard for [`turn_start`] -- the activation gate. `false` means the card
/// is not activated at all.
fn turn_start_guard(player_id: i32) -> bool {
    ctx::is_placed() && ctx::trigger::player_id() == player_id
}

fn turn_start(player_id: i32) -> card_sdk::Asked {
    // `send_to_fate` files it **now**: the flip precedes the draw in the text, and a
    // deferred fate would leave the card on the field while the draw reshuffles the
    // (empty) deck without it.
    let used = ctx::prop(PROP_USED);
    ctx::send_to_fate(ctx::Fate::Graveyard);
    // The flag is written by [`mark_used`] on the `effect` declaration, not by the
    // (pure) `redirect` hook.
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
