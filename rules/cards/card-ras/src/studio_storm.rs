//! `RAS:练习室里的风暴` -- C# `CardStudioStorm` (MatchHost.cs:9637-9709):
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:练习室里的风暴`）:
//! > 练习室里的风暴：
//!
//! > （1）当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上，每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）
//!
//! > （2）[使用者]以外的玩家在距此卡所在格子X个格子处[结算]时将此卡放入弃牌堆且对那个玩家进行一次相当于此卡所在格子普通[结算]的(4-X)/4倍价格的收费，此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动
//!
//! placed on your Live House; CiRCLE passes charge it, a nearby settle spends it.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const STUDIO_STORM: CardDef = CardDef::new(
    "RAS:练习室里的风暴",
    &[
        On::Play(Some(cant_play), play),
        On::Hook(&[HookKind::PassTile], pass_tile_guard, pass_tile),
        On::Hook(&[HookKind::SettleAfter], |_| true, settle_after),
    ],
);

const ID: &str = "RAS:练习室里的风暴";

/// C# `H.IsLiveHouse` (color group 6) tiles that can hold houses.
const LIVEHOUSE_PROPS: [&str; 4] = [
    "DUB MUSIC EXPERIMENT",
    "武道馆",
    "Space",
    "Live House Galaxy",
];

/// The tile the card sits on (C# `Mem` on a tile-bound field card).

fn on_own_lh(player_id: i32) -> bool {
    let pos = ctx::player_pos(player_id);
    pos >= 0
        && ctx::tile_owner(pos) == player_id
        && LIVEHOUSE_PROPS.iter().any(|&n| ctx::tile_named(n) == pos)
}

/// C# `CardStudioStorm.WhyNot`: 「只有站在自己的 Live House 格子上才能打出」
fn cant_play(player_id: i32) -> Option<Msg> {
    if !on_own_lh(player_id) {
        return Some(Msg::new(key!("studio_storm_not_on_lh")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上」
    // -- C# `WhyNot` requires `owners[pos] == player_id && H.IsLiveHouse(player_id, pos)`;
    // the gate is now `cant_play` above.
    let pos = ctx::player_pos(player_id);
    if !on_own_lh(player_id) {
        return Ok(());
    }
    // 规则书（1）: 「将此卡放置在当前格子上」 -- C# `H.PlaceFromPlay(c, seat, pos)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card_on(player_id, pos, ID, &Msg::new(key!("studio_storm_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("studio_storm_placed"))
            .player_id("who", player_id)
            .tile("tile", pos),
    );
    // 规则书（1）: 「每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）」
    // -- crystals start at 0 (the default) and cap at 3 via `add_crystals(.., 3)`.
    Ok(())
}

/// C# `CardStudioStorm.PassTile` -- the user passing a CiRCLE tile adds a crystal.
/// Pure guard for [`pass_tile`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pass_tile_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // C# `m.Seat != User || H.Tile(t)?.kind != "circle"`.
    if trigger::player_id() != player_id {
        return Ok(());
    }
    let t = trigger::tile();
    // 规则书（1）: 「每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）」
    if t < 0 || !ctx::is_circle(t) {
        return Ok(());
    }
    ctx::add_crystals(1, 3);
    ctx::log(
        player_id,
        &Msg::new(key!("studio_storm_crystal"))
            .player_id("who", player_id)
            .tile("tile", t),
    );
    Ok(())
}

/// C# `CardStudioStorm.SettleAfter` -- a non-user settle within `Crystals` ring
/// distance of the card's tile triggers the storm.
fn settle_after(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() || trigger::player_id() == player_id {
        return Ok(());
    }
    let Some(tile) = ctx::self_tile() else {
        return Ok(());
    };
    if tile < 0 {
        return Ok(());
    }
    let a = trigger::tile();
    let a = if a >= 0 {
        a
    } else {
        ctx::player_pos(trigger::player_id())
    };
    // 规则书（2）: 「此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动」
    let x = ctx::dist(a, tile);
    let crystals = ctx::crystals();
    if x < 1 || x > crystals {
        return Ok(());
    }
    // 规则书（2）: 「将此卡放入弃牌堆」 -- C# `Storm`: unplace to discard.
    // The [结算] below settles, so the card must be gone before it runs.
    ctx::send_to_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("studio_storm_fired"))
            .player_id("who", trigger::player_id())
            .tile("tile", tile)
            .i("x", x as i64),
    );
    // 「且那个玩家进行一次此卡所在格子的[结算]，此次[结算]的地租为普通[结算]的
    // (4-X)/4倍」 -- the mover settles here at that rent factor (milli-units:
    // 1000 * (4-X)/4 = 250 * (4-X)).
    ctx::plan::set_rent_factor(250 * (4 - x));
    ctx::card_settle_at(trigger::player_id(), tile, true);
    Ok(())
}
