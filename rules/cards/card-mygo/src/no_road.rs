//! `MyGO:无路矢` -- C# `CardNoRoad` (MatchHost.cs:6416): pick another player's
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:无路矢`）:
//! > 无路矢：
//! >  指定场上自己以外的一位玩家所在格子，获得2层[除外]并在[除外]层数归0后[传送]至该格子，视为当回合的主要移动。[除外]期间本应获得的格子收入由此前指定的那名玩家获得。
//!
//! tile, take 2 [exile] layers, and teleport there when the exile runs out.

use card_sdk::{ctx, key, CardDef, Msg};

pub const NO_ROAD: CardDef = CardDef {
    id: "MyGO:无路矢",
    play: Some(no_road),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// 规则书: 「指定场上自己以外的一位玩家所在格子」 -- C# `CardNoRoad.WhyNot`
/// refuses the card with no other player alive ("没有别的玩家").
fn why_not(seat: i32) -> Option<Msg> {
    if ctx::others(seat).is_empty() {
        return Some(Msg::new(key!("no_road_why_not_none")));
    }
    None
}

fn no_road(seat: i32) {
    let others = ctx::others(seat);
    if others.is_empty() {
        return;
    }
    // 规则书: 「指定场上自己以外的一位玩家所在格子」
    let who = ctx::ask_seat(
        seat,
        &Msg::new(key!("no_road_title")),
        &Msg::new(key!("no_road_ask")),
        &others,
    );
    let tile = ctx::seat_pos(who);
    // 规则书: 「获得2层[除外]并在[除外]层数归0后[传送]至该格子」
    // `H.GiveExile(seat, 2, tile)` stores the return tile; the engine teleports
    // there when the last layer ticks away.
    ctx::give_exile(seat, 2, tile);
    // 规则书: 「视为当回合的主要移动」 -- C# `H.SetV(i, "exileMain", 1)`.
    ctx::set_slot(seat, "exileMain", 1);
    // 规则书: 「在[除外]层数归0后[传送]至该格子」 -- the log names both the
    // destination and the redirect below.
    ctx::log(
        seat,
        &Msg::new(key!("no_road_log"))
            .tile("tile", tile)
            .seat("who", who)
            .card("card", "MyGO:无路矢"),
    );
    // TODO(规则书): 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得」 -- needs
    // Fx.RedirectReceiver (C# `NoRoadFx.RedirectReceiver` reroutes rent to the
    // chosen seat while `Me.exile > 0`).
    // TODO(规则书): the C# `H.PickTarget` also runs the `H.Target` targeting gate
    // after the seat prompt (Untargetable check) -- needs H.Target targeting.
    // TODO(规则书): the engine must honour the `exileMain` slot so the expiry
    // teleport counts as that turn's main move (C# `H.MoveWhyNot` /
    // `_turnCtx.MainMoved`); today `game-core`'s exile tick teleports without
    // consuming the main move.
}
