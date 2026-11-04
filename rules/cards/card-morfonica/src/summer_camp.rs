//!
//! 规则书（docs/rulebook/cards.json, id `Mor:夏日合宿`）:
//! > 夏日合宿：
//! >  打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌
//!
//! by others until your next turn, then draw if nothing was blocked.

use bandori_std::ctx::{self, trigger};
use bandori_std::zones::ZoneChangeCause;
use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Filter, GateValue, Msg, On};

const ID: &str = "Mor:夏日合宿";

const SLOT_BLOCKED: &str = "summer_camp_blocked";

pub const SUMMER_CAMP: CardDef = CardDef::new(
    "Mor:夏日合宿",
    &[
        On::Play("", None, summer_camp),
        // `by` is the causer's seat and is always bound at this gate (a card's or a
        // skill's controller), so `by >= 0` is a defensive no-op; `by != owner` decides.
        // 规则书: 「你只会被自己发动的效果指定」
        On::Filter(Filter::Untargetable("actor == owner && by >= 0 && by != owner", None, untargetable)),
        On::Action(Action::TurnStart("actor == owner && card.placed", None, turn_start)),
        // 规则书: 「当此卡效果结束」 -- the end, whether the effect ran out at the
        // owner's turn start or the card left the field earlier.
        On::Action(Action::ZoneLeft("card.placed", None, on_leave)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"夏日合宿"#)
        .raw_name(r#"夏日合宿"#)
        .tags(&[])
        .text(CardText::para(r#"打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌"#))
        // sheet cell G3
        .fields(&[("group", r#"Morfonica"#), ("owner", "")]));

fn summer_camp(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, ID, &Msg::new(key!("summer_camp_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("summer_camp_placed")).player_id("who", player_id),
    );
    // 规则书: 「你只会被自己发动的效果指定」 -- the Untargetable hook below refuses any `by !=
    // player_id` and counts each refusal in `SLOT_BLOCKED`.
    Ok(())
}

/// Counts negations for the draw gate.
fn untargetable(player_id: i32, mut g: GateValue) -> GateValue {
    // 规则书: 「你只会被自己发动的效果指定」 -- one negation per refusal; the draw
    //   when the effect ends is gated on none having happened.
    ctx::state::add(player_id, SLOT_BLOCKED, 1);
    g.allowed = Some(false); // deny
    g
}

fn turn_start(_player_id: i32) -> card_sdk::Asked {
    // `actor == owner && card.placed` is the condition. The effect runs out: the
    // leave (expiry) passes through the `zoneLeft` body, which is also the path
    // an earlier leave takes.
    ctx::send_to_dest(ctx::Dest::Graveyard, ZoneChangeCause::Expire);
    Ok(())
}

/// 规则书: 「当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌」 -- the
/// end, drawn once however the effect ended.
fn on_leave(player_id: i32) -> card_sdk::Asked {
    // Instance kind: another copy sees this raise too.
    if trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    bandori_std::zones::set_leave_destination(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("summer_camp_end")).player_id("who", player_id),
    );
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

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn summer_camp_kani(player_id: i32) -> card_sdk::Asked {
    summer_camp(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn untargetable_kani(player_id: i32, v: card_sdk::GateValue) -> card_sdk::GateValue {
    untargetable(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_start_kani(player_id: i32) -> card_sdk::Asked {
    turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_leave_kani(player_id: i32) -> card_sdk::Asked {
    on_leave(player_id)
}
