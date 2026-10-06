//! `RAS:（LOCK）追逐梦想的步伐` -- C# `CardLockDream` (MatchHost.cs:10003-10065):
//! pre-game field card that exiles to 东京外 and reroutes a Bandori车站 pass.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（LOCK）追逐梦想的步伐`）:
//! > （LOCK）追逐梦想的步伐：
//! >
//! > （1）抽取游戏开始的2手牌前将此卡从卡组展示给所有玩家并放置在自身场上，游戏开始时[传送]到“东京外”获得3层[除外]
//! >
//! > （2）如果此卡拥有者的主要移动[经过]了“Bandori车站”则在触发结算前将行动终点改为“旭汤澡堂”，然后此卡[移除]
//!

use card_sdk::abi::{HookKind, CardPile};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

/// C# `Normal => false` with no `Play`/`React`: the card is shown out of the
/// deck before the opening hands and lives on the field from there.
pub const LOCK_DREAM: CardDef = CardDef::new("RAS:（LOCK）追逐梦想的步伐", &[
    On::Hook(&[HookKind::DeckBeforeGame], |_| true, deck_before_game),
    On::Hook(&[HookKind::DeckAtGameStart], deck_at_game_start_guard, deck_at_game_start),
    On::Hook(&[HookKind::PassTile], |_| true, pass_tile),
    On::Hook(&[HookKind::SettleBefore], |_| true, settle_before),
]);

/// C# `m.Tags["lockStation"]` -- the owner's main move passed Bandori车站.
const SLOT_TAG: &str = "lock_dream_tag";

// 规则书（1）: 「抽取游戏开始的2手牌前将此卡从卡组展示给所有玩家并放置在自身场上」
/// C# `CardLockDream.DeckBeforeGame` -- pull the card out of the draw pile and
/// place it on the owner's field, revealed to everyone.
fn deck_before_game(player_id: i32) -> card_sdk::Asked {
    // The hook fires on this card while it sits in a draw pile (once per
    // distinct id per player, up to 5 passes before the opening deal).
    // C# pulls the id out of the deck and `H.PlaceCard(seat, seat, Id)`.
    if !ctx::take_card(player_id, CardPile::Deck, "RAS:（LOCK）追逐梦想的步伐") {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "RAS:（LOCK）追逐梦想的步伐", &Msg::new(key!("lock_dream_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("lock_dream_revealed")).player_id("who", player_id),
    );
    Ok(())
}

// 规则书（1）: 「游戏开始时[传送]到“东京外”获得3层[除外]」
/// C# `CardLockDream.AtGameStart` -> `H.Teleport` to `H.TileNamed("东京外")`
/// with `resolve: false` and `H.GiveExile(Seat, 3, to)`.
/// Pure guard for [`deck_at_game_start`] -- the activation gate. `false`
/// means the card is not activated at all.
fn deck_at_game_start_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn deck_at_game_start(player_id: i32) -> card_sdk::Asked {
    let to = ctx::tile_named("东京外");
    if to < 0 {
        return Ok(());
    }
    // 规则书（1）: 「[传送]到“东京外”」 -- C# `H.Teleport(..., resolve: false)`.
    ctx::teleport_to(player_id, to);
    // 规则书（1）: 「获得3层[除外]」 -- C# `H.GiveExile(Seat, 3, to)`.
    ctx::give_exile(player_id, 3, to);
    ctx::log(
        player_id,
        &Msg::new(key!("lock_dream_exiled")).player_id("who", player_id).tile("tile", to),
    );
    Ok(())
}

/// C# `CardLockDream.PassTile` -- tag when the owner's main move passes
/// Bandori车站.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「如果此卡拥有者的主要移动[经过]了“Bandori车站”」
    // -- C# `m.Seat == Seat && m.Main && H.Name(t) == "Bandori车站"`.
    if trigger::player_id() != player_id || !trigger::move_is_main() {
        return Ok(());
    }
    let t = trigger::tile();
    if t < 0 || ctx::tile_named("Bandori车站") != t {
        return Ok(());
    }
    // C# `m.Tags["lockStation"] = 1` -- a slot stands in for the move tag.
    ctx::set_slot(player_id, SLOT_TAG, 1);
    Ok(())
}

/// C# `CardLockDream.SettleBefore` -- if the tag is set, move the player to
/// 旭汤澡堂 before the settle and remove the card.
fn settle_before(player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() != player_id || ctx::slot(player_id, SLOT_TAG) == 0 {
        return Ok(());
    }
    ctx::set_slot(player_id, SLOT_TAG, 0);
    let to = ctx::tile_named("旭汤澡堂");
    if to >= 0 {
        // 规则书（2）: 「则在触发结算前将行动终点改为“旭汤澡堂”」
        // -- C# `Me.pos = num` (the settle then runs at the new tile).
        ctx::teleport_to(player_id, to);
        ctx::log(
            player_id,
            &Msg::new(key!("lock_dream_moved")).player_id("who", player_id).tile("tile", to),
        );
    }
    // 规则书（2）: 「然后此卡[移除]」 -- C# `H.Unplace(this, "removed")`.
    ctx::send_to_dest(ctx::Dest::Banished);
    ctx::log(player_id, &Msg::new(key!("lock_dream_removed")).player_id("who", player_id));
    Ok(())
}
