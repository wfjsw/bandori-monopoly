//! `PPP:（沙绫）总有一天要给这片天空命名` -- C# `CardSaayaSky` (MatchHost.cs:9199-9240):
//! park on 山吹面包房; after the user passes it, +1 fire and the card hops to 3d20.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（沙绫）总有一天要给这片天空命名`）:
//! > （沙绫）总有一天要给这片天空命名：
//! >
//! > （1）将此卡放置在山吹面包房
//! >
//! > （2）[使用者][经过]此卡后在回合结束后获得1个[火罐]，然后投掷3d20将此卡放置在投掷结果的格子上
//!

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "PPP:（沙绫）总有一天要给这片天空命名";

pub const SAAYA_SKY: CardDef = CardDef::new("PPP:（沙绫）总有一天要给这片天空命名", &[
    On::Play(None, play),
    On::Hook(&[HookKind::PassTile], pass_tile_guard, pass_tile),
    On::Hook(&[HookKind::TurnEndAfter], |_| true, turn_end_after)]);

/// Where the card's tile is written down (C# `Tile` on the placed card).
/// C# `CardSaayaSky._passed` -- the user walked past the card this turn.
const SLOT_PASSED: &str = "saaya_sky_passed";

fn play(player_id: i32) {
    // 规则书（1）: 「将此卡放置在山吹面包房」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // H.TileNamed("山吹面包房"))`.
    let tile = ctx::tile_named("山吹面包房");
    ctx::set_dest(ctx::Dest::Field);
    // 规则书（1）: 「将此卡放置在山吹面包房」 -- bound to that tile; the hop in
    // (2) moves it with `set_card_tile`.
    ctx::place_card_on(player_id, tile, ID, &Msg::new(key!("saaya_sky_note")));
    ctx::log(player_id, &Msg::new(key!("saaya_sky_placed")).player_id("who", player_id));
}

/// 规则书（2）: 「[使用者][经过]此卡后」 -- C# `CardSaayaSky.PassTile` sets `_passed`.
/// Pure guard for [`pass_tile`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pass_tile_guard(player_id: i32) -> bool {
    ctx::is_placed(player_id)
}

fn pass_tile(player_id: i32) {
    let tile = ctx::placed_tile(player_id, ID).unwrap_or(-1);
    if tile < 0 || trigger::tile() != tile {
        return;
    }
    // C# `m.Seat == User` -- only the placer's own pass counts.
    if trigger::player_id() != player_id {
        return;
    }
    ctx::set_slot(player_id, SLOT_PASSED, 1);
}

/// 规则书（2）: 「在回合结束后获得1个[火罐]，然后投掷3d20将此卡放置在投掷结果的格子上」
/// -- C# `CardSaayaSky.TurnEndAfter`.
fn turn_end_after(player_id: i32) {
    if !ctx::is_placed(player_id) || ctx::slot(player_id, SLOT_PASSED) == 0 {
        return;
    }
    ctx::set_slot(player_id, SLOT_PASSED, 0);
    // 规则书（2）: 「获得1个[火罐]」 -- C# `H.GainFire(User, 1, CardName)`.
    ctx::gain_fire(player_id, 1, &Msg::new(key!("saaya_sky_fire")));
    // 规则书（2）: 「投掷3d20」 -- C# `H.Roll(User, 3, 20, CardName)`.
    let num = ctx::roll(player_id, 3, 20);
    // 规则书（2）: 「将此卡放置在投掷结果的格子上」 -- C# `Tile = (num - 1) %
    // H._tiles.Length` (the roll face minus one is the *tile index*, not a hop).
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    let to = (num - 1).rem_euclid(n);
    ctx::set_card_tile(player_id, ID, to);
    ctx::log(
        player_id,
        &Msg::new(key!("saaya_sky_hop")).player_id("who", player_id).tile("tile", to),
    );
}
