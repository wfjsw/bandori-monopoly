//! `RAS:PLEASE CHOOSE` -- C# `CardPleaseChoose` (MatchHost.cs:9565-9636):
//! [反击] the settler picks a forced play at you, or you teleport onto them.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:PLEASE CHOOSE`）:
//! > PLEASE CHOOSE：
//! >  [反击] 当有玩家在livehouse格子上结算时，使对方选择以下效果之一执行：
//! > （1）立即打出一张可将你指定为目标的牌并将你指定为目标（之一），
//! > （2）使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）
//!

use card_sdk::abi::{TriggerKind, ChainKind, CardPile};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const PLEASE_CHOOSE: CardDef = CardDef::new("RAS:PLEASE CHOOSE", &[
    On::CounterAct(&[ChainKind::Settle], can_react, react),
]);

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
fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「当有玩家在livehouse格子上结算时」 -- any settle, not ours.
    if trigger::kind() != TriggerKind::Settle || trigger::player_id() == player_id {
        return false;
    }
    let t = trigger::tile();
    if t < 0 {
        return false;
    }
    // 规则书[反击]: 「在livehouse格子上结算」 -- C# `H.IsLiveHouse` + `IsBuyable`.
    // `is_live_house_for` is the whole check: group 6 (or this player's
    // `Fx.ExtraColor`) and buyable.
    ctx::is_live_house_for(player_id, t)
}

fn react(player_id: i32) {
    let other = trigger::player_id();
    // 规则书[反击]: 「使对方选择以下效果之一执行」 -- C# `H.AskPick` of `other`.
    // TODO(规则书)[judgement][反击]（1）: 「立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」
    //   the clause under-specifies -- see the note above it
    // -- C# filters `other`'s hand for `Normal && Targeting && WhyNot == null`,
    // `H.AskCard`, then `H.PlayCard` with `Tags["force"] = player_id`. Hand listing
    // (`ctx::cards_in`) and the replayable gate (`ctx::card_replayable`) are
    // ready; the targeting flags (`H.Target` / `Card.Targeting`) and the
    // force-target play tag are still missing, so option (1) is offered but
    // not executed.
    let opt1 = Msg::new(key!("please_choose_opt1")).player_id("who", player_id);
    // 规则书[反击]（2）: 「使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）」
    let opt2 = Msg::new(key!("please_choose_opt2")).player_id("who", player_id);
    let pick = ctx::ask_pick(
        other,
        &Msg::new(key!("please_choose_title")),
        &Msg::new(key!("please_choose_ask")).player_id("who", player_id),
        &[opt1, opt2],
    );
    if pick == 0 {
        // TODO(规则书)[judgement][反击]（1）: 「立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」
        //   the clause under-specifies -- see the note above it
        // -- see above; the forced play needs `CardDef.targeting()` / `Normal`
        // (hand filter) and a force-target play tag. `ctx::cards_in(other,
        // CardPile::Hand)` lists the hand now.
        ctx::log(
            other,
            &Msg::new(key!("please_choose_opt1_todo")).player_id("who", other).player_id("reactor", player_id),
        );
        return;
    }
    // 规则书[反击]（2）: 「使你立即传送至对方所在格子」 -- C# `H.ForceTeleport(i,
    // to, resolve: false, ...)`; `ctx::teleport_to` is the resolve:false teleport.
    let to = ctx::player_pos(other);
    if to >= 0 {
        ctx::teleport_to(player_id, to);
        ctx::log(
            player_id,
            &Msg::new(key!("please_choose_moved")).player_id("who", player_id).tile("tile", to),
        );
    }
    // TODO(规则书)[judgement][反击]（2）: 「（不触发结算但视为可触发乐队技能）」 -- the
    //   the clause under-specifies -- see the note above it
    // no-settle half is `ctx::teleport_to`; the band-skill half needs the
    // `BandBase.PassTile` fan-out (C# `H.EachOf(i, f => f.PassTile(m, to))`).
}
