//! `MyGO:（立希）想认真去做` -- C# `CardTakiSerious` (MatchHost.cs:6749-6791):
//! every player holding [停留] settles immediately on a tile you pick within
//! 2 of their position, all payments at a quarter price.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（立希）想认真去做`）:
//! > （立希）想认真去做：
//! >  使场上所有拥有[停留]的玩家立刻在所在格子前后2格内你选择的一个格子进行一次[触发结算]，本次结算导致的所有[支付]变为原价的四分之一
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const TAKI_SERIOUS: CardDef = CardDef {
    id: "MyGO:（立希）想认真去做",
    play: Some(taki_serious),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardTakiSerious.Stayers` -- present seats holding [停留].
fn stayers() -> Vec<i32> {
    // C# `H.Present(p) && H.State.seats[p].stay > 0`. `Present` is "in the game
    // and not exiled"; `seat_out` covers the out-of-game half (exile layers are
    // not readable, so an exiled stayer may slip in).
    (0..ctx::seat_count())
        .filter(|&p| !ctx::seat_out(p) && ctx::stay_of(p) > 0)
        .collect()
}

fn why_not(_seat: i32) -> Option<Msg> {
    // 规则书: 「使场上所有拥有[停留]的玩家...」 -- C# `CardTakiSerious.WhyNot`
    // refuses the play when nobody holds [停留].
    if stayers().is_empty() {
        return Some(Msg::new(key!("taki_serious_why_not_stay")));
    }
    None
}

fn taki_serious(_seat: i32) {
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    for p in stayers() {
        if ctx::seat_out(p) {
            continue;
        }
        let pos = ctx::seat_pos(p);
        // 规则书: 「所在格子前后2格内」 -- C# `Enumerable.Range(-2, 5)` over the
        // ring, distinct. Ready for the settle below.
        let mut tiles: Vec<i32> = Vec::new();
        for d in -2..=2 {
            let t = ((pos + d) % n + n) % n;
            if !tiles.contains(&t) {
                tiles.push(t);
            }
        }
        let _ = &tiles;
        // TODO(规则书): 「...你选择的一个格子进行一次[触发结算]，本次结算导致的所有[支付]变为
        // 原价的四分之一」 -- needs `H.SettleAt(p, t, CardName, 0.25)` (settle on
        // the chosen tile with a pay factor of 1/4). The ±2 window above is
        // computed; the C# `H.AskTileOf` over `tiles` and the settle itself wait
        // on that hook (a prompt with nothing to settle would dead-end).
    }
}
