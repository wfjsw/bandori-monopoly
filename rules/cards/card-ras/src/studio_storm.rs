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

use card_sdk::{ctx, key, CardDef, Msg};

pub const STUDIO_STORM: CardDef = CardDef {
    id: "RAS:练习室里的风暴",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `H.IsLiveHouse` (color group 6) tiles that can hold houses.
const LIVEHOUSE_PROPS: [&str; 4] = ["DUB MUSIC EXPERIMENT", "武道馆", "Space", "Live House Galaxy"];

fn on_own_lh(seat: i32) -> bool {
    let pos = ctx::seat_pos(seat);
    pos >= 0
        && ctx::tile_owner(pos) == seat
        && LIVEHOUSE_PROPS.iter().any(|&n| ctx::tile_named(n) == pos)
}

/// C# `CardStudioStorm.WhyNot`: 「只有站在自己的 Live House 格子上才能打出」
fn why_not(seat: i32) -> Option<Msg> {
    if !on_own_lh(seat) {
        return Some(Msg::new(key!("studio_storm_not_on_lh")));
    }
    None
}

fn play(seat: i32) {
    // 规则书（1）: 「当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上」
    // -- C# `WhyNot` requires `owners[pos] == seat && H.IsLiveHouse(seat, pos)`;
    // the gate is now `why_not` above.
    let pos = ctx::seat_pos(seat);
    if !on_own_lh(seat) {
        return;
    }
    // 规则书（1）: 「将此卡放置在当前格子上」 -- C# `H.PlaceFromPlay(c, seat, pos)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "RAS:练习室里的风暴", &Msg::new(key!("studio_storm_note")));
    ctx::log(
        seat,
        &Msg::new(key!("studio_storm_placed")).seat("who", seat).tile("tile", pos),
    );
    // TODO(规则书): 「将此卡放置在当前格子上」 -- the placement is bound to `pos`
    // rather than the seat's field; needs field-card tile placement
    // (`H.PlaceFromPlay(c, owner, tile)`).
    // TODO(规则书）（1）: 「每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）」
    // -- needs the Fx.PassTile hook (C# `CardStudioStorm.PassTile`, tile kind
    // "circle") and the per-card crystal counter (`Card.Crystals`, `MaxCrystals = 3`).
    // TODO(规则书）（2）: 「[使用者]以外的玩家在距此卡所在格子X个格子处[结算]时将此卡放入弃牌堆且那个玩家进行一次此卡所在格子的[结算]，此次[结算]的地租为普通[结算]的(4-X)/4倍，此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动」
    // -- needs the Fx.SettleAfter hook (C# `CardStudioStorm.SettleAfter` /
    // `Storm`) and a settle with `PayFactor = (4 - X) / 4` (`H.Settle` with
    // `MoveCtx.PayFactor`). `ctx::dist(a, b)` is ready for the ring distance X;
    // the crystal counter is still missing.
}
