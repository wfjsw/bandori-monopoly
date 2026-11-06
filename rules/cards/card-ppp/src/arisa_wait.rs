//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（有咲）等等等一下`）:
//! > （有咲）等等等一下：
//! > [手]：
//! >  将此卡放置在[使用者]的[场地]。
//! > [持续]：
//!
//! > （1）任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。
//!
//! > （2）事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。
//!

use bandori_std::ctx;
use card_sdk::abi::ActionKind;
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "PPP:（有咲）等等等一下";

pub const ARISA_WAIT: CardDef = CardDef::new(
    "PPP:（有咲）等等等一下",
    &[
        On::Play("", None, arisa_wait),
        // （2）「事件触发时…」 -- placement and count are the applicability.
        On::Action(Action::Event(
            "card.placed && card.counter('crystals') >= 3",
            None,
            defer,
        )),
        // （1）「任何非因为此卡导致的事件结算时…」
        On::Action(Action::EventAfter("card.placed", Some(foreign), event_after)),
        // release callback（「此后额外抽取…」）
        On::Action(Action::EventResolved(
            "card.placed",
            Some(resolved_mine),
            resolved,
        )),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"等等等一下"#)
        .raw_name(r#"（有咲）等等等一下"#)
        .tags(&[r#"手"#, r#"持续"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"将此卡放置在[使用者]的[场地]。"#),
            ]),
            sec(r#"[持续]："#, &[
                items(&[
                    it(r#"任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。"#),
                    it(r#"事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。"#),
                ]),
            ]),
        ]))
        // sheet cell B16
        .fields(&[("group", r#"Poppin' Party"#), ("owner", r#"市谷有咲"#)]));

fn arisa_wait(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]。」
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, ID, &Msg::new(key!("arisa_wait_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("arisa_wait_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书[持续]（1）: 「任何非因为此卡导致的事件结算时」 -- the event must not
/// be this copy's doing. A board draw carries no cause (`None`), so it counts.
fn foreign(_: i32) -> bool {
    ctx::trigger::by_card_instance() != Some(ctx::self_uid())
}

/// 规则书[持续]（2）: 「事件触发时如果此卡拥有至少3个[奇迹水晶]则移除…」.
/// Only the copy that wins the claim spends; a later copy on the same event
/// gets `false` and leaves its crystals alone.
fn defer(player_id: i32) -> card_sdk::Asked {
    if !ctx::event_defer()? {
        return Ok(());
    }
    let left = ctx::crystals() - 3;
    ctx::set_crystals(left);
    ctx::log(
        player_id,
        &Msg::new(key!("arisa_wait_deferred"))
            .player_id("who", player_id)
            .i("n", left as i64),
    );
    Ok(())
}

/// Only the copy whose deferral this is hears the release callback.
fn resolved_mine(_: i32) -> bool {
    ctx::trigger::by_card_instance() == Some(ctx::self_uid())
}

/// 「此后额外抽取一个视为其他玩家在“流星堂”抽取的事件」 -- the next live
/// seat after the owner draws, attributed to the station tile (Q1).
fn resolved(player_id: i32) -> card_sdk::Asked {
    let other = ctx::neighbor(player_id, 1);
    if other >= 0 {
        ctx::draw_event_as(other, ctx::tile_named("流星堂"))?;
    }
    Ok(())
}

fn event_after(player_id: i32) -> card_sdk::Asked {
    ctx::add_crystals(1, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("arisa_wait_crystal"))
            .player_id("who", player_id)
            .i("n", ctx::crystals() as i64),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn arisa_wait_kani(player_id: i32) -> card_sdk::Asked {
    arisa_wait(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn event_after_kani(player_id: i32) -> card_sdk::Asked {
    event_after(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn defer_kani(player_id: i32) -> card_sdk::Asked {
    defer(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn resolved_kani(player_id: i32) -> card_sdk::Asked {
    resolved(player_id)
}
