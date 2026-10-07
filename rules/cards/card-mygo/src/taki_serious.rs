//! `MyGO:（立希）想认真去做` -- C# `CardTakiSerious` (MatchHost.cs:6749-6791):
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（立希）想认真去做`）:
//! > （立希）想认真去做：
//! >  使场上所有拥有[停留]的玩家立刻在所在格子前后2格内你选择的一个格子进行一次[触发结算]，本次结算导致的所有[支付]变为原价的四分之一
//!
//! every player holding [停留] settles immediately on a tile you pick within
//! 2 of their position, all payments at a quarter price.

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const TAKI_SERIOUS: CardDef = CardDef::new(
    "MyGO:（立希）想认真去做",
    &[On::Play(Some(cant_play), taki_serious)],
);

/// C# `CardTakiSerious.Stayers` -- present players holding [停留].
fn stayers() -> Vec<i32> {
    // C# `H.Present(p) && H.State.seats[p].stay > 0`. `Present` is "in the game
    // and not exiled"; `player_out` covers the out-of-game half (exile layers are
    // not readable, so an exiled stayer may slip in).
    (0..ctx::player_count())
        .filter(|&p| !ctx::player_out(p) && ctx::stay_of(p) > 0)
        .collect()
}

fn cant_play(_player: i32) -> Option<Msg> {
    // 规则书: 「使场上所有拥有[停留]的玩家...」 -- C# `CardTakiSerious.WhyNot`
    // refuses the play when nobody holds [停留].
    if stayers().is_empty() {
        return Some(Msg::new(key!("taki_serious_why_not_stay")));
    }
    None
}

fn taki_serious(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    for p in stayers() {
        if ctx::player_out(p) {
            continue;
        }
        let pos = ctx::player_pos(p);
        // 规则书: 「所在格子前后2格内」 -- C# `Enumerable.Range(-2, 5)` over the
        // ring, distinct. Ready for the settle below.
        let mut tiles: Vec<i32> = Vec::new();
        for d in -2..=2 {
            let t = ((pos + d) % n + n) % n;
            if !tiles.contains(&t) {
                tiles.push(t);
            }
        }
        if tiles.is_empty() {
            return Ok(());
        }
        let pick = ctx::ask_tile(
            player_id,
            &Msg::new(key!("taki_serious_title")),
            &Msg::new(key!("taki_serious_where")).player_id("who", p),
            &tiles,
        )?;
        // 「...你选择的一个格子进行一次[触发结算]，本次结算导致的所有[支付]变为
        // 原价的四分之一」 -- the stayer settles there at a quarter price.
        ctx::plan::set_pay_factor(250);
        ctx::card_settle_at(p, pick, true);
        ctx::log(
            player_id,
            &Msg::new(key!("taki_serious_settled"))
                .tile("tile", pick)
                .player_id("who", p),
        );
    }
    Ok(())
}
