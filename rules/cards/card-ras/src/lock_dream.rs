//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（LOCK）追逐梦想的步伐`）:
//! > （LOCK）追逐梦想的步伐：
//!
//! > （1）抽取游戏开始的2手牌前将此卡从卡组展示给所有玩家并放置在自身场上，游戏开始时[传送]到“东京外”获得3层[除外]
//!
//! > （2）如果此卡拥有者的主要移动[经过]了“Bandori车站”则在触发结算前将行动终点改为“旭汤澡堂”，然后此卡[移除]
//!
//! pre-game field card that exiles to 东京外 and reroutes a Bandori车站 pass.

use bandori_std::ctx;
use card_sdk::abi::{ActionKind, CardPile};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const LOCK_DREAM: CardDef = CardDef::new(
    "RAS:（LOCK）追逐梦想的步伐",
    &[
        // Card-in-deck is not in the CEL schema (and a `card_in(pile)` probe
        // would cost a pile walk per candidate for a hook that fires ~5 times
        // before the opening deal) -- the residual guard stays.
        On::Action(Action::DeckBeforeGame("", Some(deck_before_game_guard), deck_before_game)),
        On::Action(Action::DeckAtGameStart("card.placed", None, deck_at_game_start)),
        On::Action(Action::PassTile("actor == owner && move.is_main_move && tile.id == tile_named('Bandori车站')", None, pass_tile)).silent(),
        On::Action(Action::SettleBefore("actor == owner && slot('lock_dream_tag') != 0", None, settle_before)),
        // One-shot latch drop: a main move that passed but never settled
        // (`resolve: false`) must not reroute the owner's next settle.
        On::Action(Action::MoveResolved("actor == owner && move.is_main_move", None, clear_tag)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"追逐梦想的步伐"#)
        .raw_name(r#"（LOCK）追逐梦想的步伐"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"抽取游戏开始的2手牌前将此卡从卡组展示给所有玩家并放置在自身场上，游戏开始时[传送]到“东京外”获得3层[除外]"#),
            it(r#"如果此卡拥有者的主要移动[经过]了“Bandori车站”则在触发结算前将行动终点改为“旭汤澡堂”，然后此卡[移除]"#),
        ]))
        // sheet cell H13
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", r#"朝日六花"#)]));

const SLOT_TAG: &str = "lock_dream_tag";

// 规则书（1）: 「抽取游戏开始的2手牌前将此卡从卡组展示给所有玩家并放置在自身场上」
/// Residual guard for the hook: the card is still in the owner's deck.
/// The hook fires on this card while it sits in a draw pile (once per
/// distinct id per player, up to 5 passes before the opening deal).
fn deck_before_game_guard(player_id: i32) -> bool {
    ctx::cards_in(player_id, CardPile::Deck)
        .iter()
        .any(|c| c == "RAS:（LOCK）追逐梦想的步伐")
}

fn deck_before_game(player_id: i32) -> card_sdk::Asked {
    if !ctx::take_card(player_id, "RAS:（LOCK）追逐梦想的步伐", CardPile::Deck) {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "RAS:（LOCK）追逐梦想的步伐",
        &Msg::new(key!("lock_dream_note")),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("lock_dream_revealed")).player_id("who", player_id),
    );
    Ok(())
}

// 规则书（1）: 「游戏开始时[传送]到“东京外”获得3层[除外]」
/// Activation gate is the condition `card.placed` (`ctx::is_placed()`).
fn deck_at_game_start(player_id: i32) -> card_sdk::Asked {
    let to = ctx::tile_named("东京外");
    if to < 0 {
        return Ok(());
    }
    // `teleport_as` is the move-raising teleport (a bare `teleport_to` write raises
    // no `passTile` at the destination); `as_main` stays false -- this is not the
    // turn's 主要移动.
    ctx::teleport_as(player_id, to, false, false);
    rules_status::exile::send(player_id, 3, to);
    ctx::log(
        player_id,
        &Msg::new(key!("lock_dream_exiled"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    Ok(())
}

fn pass_tile(player_id: i32) -> card_sdk::Asked {
    ctx::set_slot(player_id, SLOT_TAG, 1);
    Ok(())
}

/// Drop the pass tag at the end of the owner's main move, so only that move's
/// own settle (which runs before `moveResolved`) can read it.
fn clear_tag(_player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    if mover >= 0 {
        ctx::set_slot(mover, SLOT_TAG, 0);
    }
    Ok(())
}

/// The condition `actor == owner && slot('lock_dream_tag') != 0` selects exactly
/// that move (the slot lives only for the move that armed it).
fn settle_before(player_id: i32) -> card_sdk::Asked {
    ctx::set_slot(player_id, SLOT_TAG, 0);
    let to = ctx::tile_named("旭汤澡堂");
    if to >= 0 {
        ctx::teleport_to(player_id, to);
        ctx::log(
            player_id,
            &Msg::new(key!("lock_dream_moved"))
                .player_id("who", player_id)
                .tile("tile", to),
        );
    }
    ctx::send_to_dest(ctx::Dest::Banished);
    ctx::log(
        player_id,
        &Msg::new(key!("lock_dream_removed")).player_id("who", player_id),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn deck_before_game_kani(player_id: i32) -> card_sdk::Asked {
    deck_before_game(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn deck_at_game_start_kani(player_id: i32) -> card_sdk::Asked {
    deck_at_game_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_before_kani(player_id: i32) -> card_sdk::Asked {
    settle_before(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_tag_kani(player_id: i32) -> card_sdk::Asked {
    clear_tag(player_id)
}
