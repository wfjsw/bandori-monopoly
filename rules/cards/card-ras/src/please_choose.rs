//! `RAS:PLEASE CHOOSE` -- C# `CardPleaseChoose` (MatchHost.cs:9565-9636):
//! [反击] the settler picks a forced play at you, or you teleport onto them.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:PLEASE CHOOSE`）:
//! > PLEASE CHOOSE：
//! >  [反击] 当有玩家在livehouse格子上结算时，使对方选择以下效果之一执行：
//! > （1）立即打出一张可将你指定为目标的牌并将你指定为目标（之一），
//! > （2）使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const PLEASE_CHOOSE: CardDef = CardDef {
    id: "RAS:PLEASE CHOOSE",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// C# `H.IsLiveHouse(t.Seat, t.Tile) && H._tiles[t.Tile].IsBuyable` -- a
/// buyable Live House (color group 6, `kind` property/ring). `Live House` the
/// agent is not buyable and so is not in this list.
const LIVEHOUSE_BUYABLE: [&str; 8] = [
    "RiNG 1",
    "RiNG 2",
    "DUB MUSIC EXPERIMENT",
    "RiNG 3",
    "武道馆",
    "Space",
    "Live House Galaxy",
    "RiNG 4",
];

/// 规则书[反击]: 「当有玩家在livehouse格子上结算时」
fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「当有玩家在livehouse格子上结算时」 -- any settle, not ours.
    if trigger::kind() != TriggerKind::Settle || trigger::seat() == seat {
        return false;
    }
    let t = trigger::tile();
    if t < 0 {
        return false;
    }
    // 规则书[反击]: 「在livehouse格子上结算」 -- C# `H.IsLiveHouse` + `IsBuyable`.
    // `IsBuyable` is now `ctx::is_buyable`; `H.IsLiveHouse` (color group /
    // ExtraColor) still uses the name list.
    // TODO(ABI): `H.IsLiveHouse` (color group / ExtraColor) is not queryable;
    // the name list is board.json's group-6 buyable tiles.
    LIVEHOUSE_BUYABLE.iter().any(|&n| ctx::tile_named(n) == t) && ctx::is_buyable(t)
}

fn react(seat: i32) {
    let other = trigger::seat();
    // 规则书[反击]: 「使对方选择以下效果之一执行」 -- C# `H.AskPick` of `other`.
    // TODO(规则书)[反击]（1）: 「立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」
    // -- C# filters `other`'s hand for `Normal && Targeting && WhyNot == null`,
    // `H.AskCard`, then `H.PlayCard` with `Tags["force"] = seat`. Needs hand
    // listing, the targeting flags (`H.Target` / `Card.Targeting`), and a
    // force-target play tag. Until then option (1) is offered but not executed.
    let opt1 = Msg::new(key!("please_choose_opt1")).seat("who", seat);
    // 规则书[反击]（2）: 「使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）」
    let opt2 = Msg::new(key!("please_choose_opt2")).seat("who", seat);
    let pick = ctx::ask_pick(
        other,
        &Msg::new(key!("please_choose_title")),
        &Msg::new(key!("please_choose_ask")).seat("who", seat),
        &[opt1, opt2],
    );
    if pick == 0 {
        // TODO(规则书)[反击]（1）: 「立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」
        // -- see above; the forced play has no vocabulary yet.
        ctx::log(
            other,
            &Msg::new(key!("please_choose_opt1_todo")).seat("who", other).seat("reactor", seat),
        );
        return;
    }
    // 规则书[反击]（2）: 「使你立即传送至对方所在格子」 -- C# `H.ForceTeleport(i,
    // to, resolve: false, ...)`; `ctx::teleport_to` is the resolve:false teleport.
    let to = ctx::seat_pos(other);
    if to >= 0 {
        ctx::teleport_to(seat, to);
        ctx::log(
            seat,
            &Msg::new(key!("please_choose_moved")).seat("who", seat).tile("tile", to),
        );
    }
    // TODO(规则书)[反击]（2）: 「（不触发结算但视为可触发乐队技能）」 -- the
    // no-settle half is `ctx::teleport_to`; the band-skill half needs the
    // `BandBase.PassTile` fan-out (C# `H.EachOf(i, f => f.PassTile(m, to))`).
}
