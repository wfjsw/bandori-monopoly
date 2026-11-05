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
        // 规则书: 「若在此期间此卡没有造成影响，抽1张卡」 -- the latch is spent inside
        // [`redirect`], where the influence actually happens (an effect merely
        // aimed at the owner without passing this fold changes nothing).
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

/// The 「造成影响」 latch, one per **instance** (`exist.used.<uid>`). A player
/// state key (not an instance prop) because the filter body that writes it may
/// not call `set_prop`; the uid keeps two copies apart.
const USED: &str = "exist.used";

fn used_key() -> alloc::string::String {
    alloc::format!("{USED}.{}", ctx::self_uid())
}

fn exist(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, ID, &Msg::new(key!("exist_note")));
    // 规则书: 「直到自己的下一回合开始…你的下回合开始时将其翻入弃牌堆」 -- one
    // turn start away (the `⌛1` face marker is the static `TURNS_LEFT` prop).
    ctx::state::set(player_id, &used_key(), 0);
    ctx::log(
        player_id,
        &Msg::new(key!("exist_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// The retarget rides `RedirectValue.payee`; the fold reaching here is itself
/// the 「造成影响」, so the latch is spent.
fn redirect(player_id: i32, v: RedirectValue) -> RedirectValue {
    // 规则书: 「场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你」
    // -- already aimed at us is the entry's condition (`target != owner`).
    // `state::add` is the one write a filter body may make (not
    // `refuse_if_filter`-guarded): the latch is per instance.
    // TODO(规则书): 「若在此期间此卡没有造成影响」 -- the fold is counted before any
    // counter settles (C-Q8 / WINDOW-03) and a probe pass may run it twice; the
    // book asks whether the card had a final influence.
    ctx::state::add(player_id, &used_key(), 1);
    RedirectValue {
        payee: Some(Payee::Seat(Seat(player_id))),
        ..v
    }
}

/// Pure guard for [`turn_start`] -- the activation gate. `false` means the card
/// is not activated at all.
fn turn_start_guard(player_id: i32) -> bool {
    ctx::is_placed() && ctx::trigger::player_id() == player_id
}

fn turn_start(player_id: i32) -> card_sdk::Asked {
    // `send_to_dest` files it **now**: the flip precedes the draw in the text, and a
    // deferred dest would leave the card on the field while the draw reshuffles the
    // (empty) deck without it.
    let used = ctx::state::get(player_id, &used_key());
    ctx::send_to_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
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
pub fn turn_start_kani(player_id: i32) -> card_sdk::Asked {
    turn_start(player_id)
}
