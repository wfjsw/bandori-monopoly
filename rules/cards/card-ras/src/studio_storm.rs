//! `RAS:练习室里的风暴` -- C# `CardStudioStorm` (MatchHost.cs:9637-9709):
//! placed on your Live House; CiRCLE passes charge it, a nearby settle spends it.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:练习室里的风暴`）:
//! > 练习室里的风暴：
//! >
//! > （1）当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上，每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）
//! >
//! > （2）[使用者]以外的玩家在距此卡所在格子X个格子处[结算]时将此卡放入弃牌堆且那个玩家进行一次此卡所在格子的[结算]，此次[结算]的地租为普通[结算]的(4-X)/4倍，此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动
//!

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const STUDIO_STORM: CardDef = CardDef::new("RAS:练习室里的风暴", &[
    On::Play(play),
    On::CantPlay(cant_play),
    On::Hook(&[HookKind::PassTile], pass_tile),
    On::Hook(&[HookKind::SettleAfter], settle_after),
]);

const ID: &str = "RAS:练习室里的风暴";

/// C# `H.IsLiveHouse` (color group 6) tiles that can hold houses.
const LIVEHOUSE_PROPS: [&str; 4] = ["DUB MUSIC EXPERIMENT", "武道馆", "Space", "Live House Galaxy"];

/// The tile the card sits on (C# `Mem` on a tile-bound field card).
const SLOT_TILE: &str = "studio_storm_tile";

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

fn play(player_id: i32) {
    // 规则书（1）: 「当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上」
    // -- C# `WhyNot` requires `owners[pos] == player_id && H.IsLiveHouse(player_id, pos)`;
    // the gate is now `cant_play` above.
    let pos = ctx::player_pos(player_id);
    if !on_own_lh(player_id) {
        return;
    }
    // 规则书（1）: 「将此卡放置在当前格子上」 -- C# `H.PlaceFromPlay(c, seat, pos)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("studio_storm_note")));
    ctx::set_slot(player_id, SLOT_TILE, pos);
    ctx::log(
        player_id,
        &Msg::new(key!("studio_storm_placed")).player_id("who", player_id).tile("tile", pos),
    );
    // 规则书（1）: 「每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）」
    // -- crystals start at 0 (the default) and cap at 3 via `add_crystals(.., 3)`.
    // TODO(规则书): 「将此卡放置在当前格子上」 -- the placement is bound to `pos`
    // rather than the player's field; needs field-card tile placement
    // (`H.PlaceFromPlay(c, owner, tile)`). The tile is remembered in `SLOT_TILE`
    // so the hooks below can key on it.
}

/// C# `CardStudioStorm.PassTile` -- the user passing a CiRCLE tile adds a crystal.
fn pass_tile(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    // C# `m.Seat != User || H.Tile(t)?.kind != "circle"`.
    if trigger::player_id() != player_id {
        return;
    }
    let t = trigger::tile();
    // 规则书（1）: 「每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）」
    if t < 0 || !ctx::is_circle(t) {
        return;
    }
    ctx::add_crystals(player_id, 1, 3);
    ctx::log(
        player_id,
        &Msg::new(key!("studio_storm_crystal")).player_id("who", player_id).tile("tile", t),
    );
}

/// C# `CardStudioStorm.SettleAfter` -- a non-user settle within `Crystals` ring
/// distance of the card's tile triggers the storm.
fn settle_after(player_id: i32) {
    if !ctx::is_placed(player_id) || trigger::player_id() == player_id {
        return;
    }
    let tile = ctx::slot(player_id, SLOT_TILE);
    if tile < 0 {
        return;
    }
    let a = trigger::tile();
    let a = if a >= 0 { a } else { ctx::player_pos(trigger::player_id()) };
    // 规则书（2）: 「此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动」
    let x = ctx::dist(a, tile);
    let crystals = ctx::crystals(player_id);
    if x < 1 || x > crystals {
        return;
    }
    // 规则书（2）: 「将此卡放入弃牌堆」 -- C# `Storm`: unplace to discard.
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, ID);
    ctx::log(
        player_id,
        &Msg::new(key!("studio_storm_fired")).player_id("who", trigger::player_id()).tile("tile", tile).i("x", x as i64),
    );
    // TODO(规则书）（2）: 「且那个玩家进行一次此卡所在格子的[结算]，此次[结算]的地租为
    // 普通[结算]的(4-X)/4倍」 -- the rent factor is `ctx::plan::set_rent_factor`
    // (milli-units, (4000 - 1000*X) / 4), but the settle routine (`H.SettleAt` /
    // `H.Settle`) that would run that settle is still held. The SettleAfter hook,
    // the ring distance `X`, the crystal gate, and the discard half above are done.
}
