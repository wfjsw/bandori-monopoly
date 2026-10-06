//! `R:向着顶点` -- C# `CardToThePeak` (MatchHost.cs:10360-10422): walk to the next
//!
//! 规则书（docs/rulebook/cards.json, id `R:向着顶点`）:
//! > 向着顶点：
//! >  移动到下一个可被购买的livehouse格子。若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋。
//!
//! free Livehouse, or build one of yours at 1.5x cost.

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const TO_THE_PEAK: CardDef = CardDef::new("R:向着顶点", &[On::Play(Some(cant_play), play)]);

/// The buyable Livehouse deeds (C# `H.LiveHouses`: `IsBuyable && IsColor(6)`),
/// where `IsColor` sees both the global `_tileColors` re-colour and this
/// player's `Fx.ExtraColor`.
fn livehouses(player_id: i32) -> Vec<i32> {
    (0..ctx::tile_count())
        .filter(|&t| ctx::is_buyable(t) && ctx::is_color(player_id, t, 6))
        .collect()
}

/// C# `WhyNotBuildOn`'s 「RiNG 不能加盖房屋」: a RiNG deed is the buyable
/// group-6 tile with no rent table, so `rent_of <= 0` is the test (the
/// vocabulary has no `TileData.kind` query).
fn is_ring(t: i32) -> bool {
    ctx::rent_of(t) <= 0
}

/// C# `H.WhyNotBuildOn(i, t)` -- the checks the vocabulary can see.
fn why_not_build_on(player_id: i32, t: i32) -> Option<&'static str> {
    if ctx::tile_owner(t) != player_id {
        return Some("only_own");
    }
    if is_ring(t) {
        return Some("ring");
    }
    if ctx::mortgaged_of(t) {
        return Some("mortgaged");
    }
    // C# `houses[t] >= rent.Length - 1` (「这块地已经盖满了」). The vocabulary has
    // no rent-table length; every buildable tile in `data/board.json` holds 3.
    if ctx::houses_of(t) >= MAX_HOUSES {
        return Some("full");
    }
    if !ctx::can_pay(player_id) {
        return Some("cannot_pay");
    }
    None
    // Not checked (no hook): `_turnCtx.NoBuild` (学生会的检查's build lock),
    // the 「卡池BUG」 event, and `Fx.CanBuild`.
}

/// C# `tileData.rent.Length - 1` for the board's buildable tiles (all 4-step
/// rent tables); a rent-length query would generalise this.
const MAX_HOUSES: i32 = 3;

/// C# `CardToThePeak.WhyNot`.
fn cant_play(player_id: i32) -> Option<Msg> {
    let free: Vec<i32> = livehouses(player_id)
        .into_iter()
        .filter(|&t| ctx::tile_owner(t) < 0)
        .collect();
    if !free.is_empty() {
        // C# `H.Moving(seat)` -- the walk branch needs a movable player.
        if ctx::stun_of(player_id) > 0 {
            return Some(Msg::new(key!("to_the_peak_stunned")));
        }
        if ctx::stay_of(player_id) > 0
            && card_sdk::ctx::state::get(player_id, card_sdk::abi::state_key::UNSTOPPABLE) <= 0
        {
            // `H.Moving` lets a `[停留]` player through when 「不可阻挡」 is up.
            return Some(Msg::new(key!("to_the_peak_stuck")));
        }
        return None;
    }
    // C# the build branch: `H.LiveHouses(i, t => owners[t] == i &&
    // H.WhyNotBuildOn(i, t) == null)` (no money check here -- that is in Play).
    if livehouses(player_id)
        .into_iter()
        .any(|t| ctx::tile_owner(t) == player_id && why_not_build_on(player_id, t).is_none())
    {
        return None;
    }
    Some(Msg::new(key!("to_the_peak_nothing")))
}

fn play(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    // 规则书: 「移动到下一个可被购买的livehouse格子」 -- unowned Livehouses, nearest ahead.
    let free: Vec<i32> = livehouses(player_id)
        .into_iter()
        .filter(|&t| ctx::tile_owner(t) < 0)
        .collect();
    if !free.is_empty() {
        let to = free
            .into_iter()
            .min_by_key(|&t| {
                let d = ctx::tile_forward(pos, t);
                if d == 0 {
                    ctx::tile_count()
                } else {
                    d
                }
            })
            .unwrap_or(-1);
        // 规则书: 「移动到下一个可被购买的livehouse格子」 -- C# `H.Walk(i, Forward(pos, to),
        // resolve: true, null, "向着顶点")` = a forward walk of `Forward(pos, to)`
        // steps that settles at the destination.
        let num = ctx::tile_forward(pos, to);
        let num = if num == 0 { ctx::tile_count() } else { num };
        ctx::plan::set_steps(num);
        ctx::plan::set_resolve(true);
        ctx::card_move(player_id);
        ctx::log(
            player_id,
            &Msg::new(key!("to_the_peak_walk"))
                .player_id("who", player_id)
                .tile("tile", to),
        );
        return Ok(());
    }
    // 规则书: 「若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋」
    let mut mine: Vec<i32> = Vec::new();
    for t in livehouses(player_id) {
        if why_not_build_on(player_id, t).is_some() {
            continue;
        }
        if ctx::money_of(player_id) >= ctx::build_cost(t) * 3 / 2 {
            mine.push(t);
        }
    }
    if mine.is_empty() {
        ctx::log(player_id, &Msg::new(key!("to_the_peak_no_money")));
        return Ok(());
    }
    let title = Msg::new(key!("to_the_peak_ask_title"));
    let text = Msg::new(key!("to_the_peak_ask_text"));
    // C# `H.AskTileOf(..., allowNone: true)` -- a yes/no stands in for allowNone.
    if !ctx::ask_yes(player_id, &title, &text)? {
        return Ok(());
    }
    let tile = ctx::ask_tile(player_id, &title, &text, &mine)?;
    // 规则书: 「花费1.5倍价格」 -- C# `CeilTo(H.BuildCostFor(i, r.index) * 1.5, 10)`.
    let amount = ceil_to(ctx::build_cost(tile) as i64 * 3 / 2, 10);
    let paid = ctx::pay(
        player_id,
        amount,
        &Msg::new(key!("to_the_peak_why")).tile("tile", tile),
    )?;
    if paid < amount {
        return Ok(());
    }
    // 规则书: 「加盖一层房屋」 -- C# `H.BuildRoutine(i, r.index, free: true, "向着顶点")`:
    // the pay above covers the 1.5x cost and the raise is free (`H.AddHouse`).
    if why_not_build_on(player_id, tile).is_none() {
        ctx::add_house(tile, 1);
        ctx::log(
            player_id,
            &Msg::new(key!("to_the_peak_build"))
                .player_id("who", player_id)
                .tile("tile", tile),
        );
    }
    // The raise above goes out as `houseAdded` once the run commits, so a
    // persistent Fx listening for 「加盖」 sees this build too.
    Ok(())
}

/// C# `CeilTo(x, unit)` -- round up to a multiple of `unit`.
fn ceil_to(x: i64, unit: i64) -> i32 {
    if unit <= 0 {
        return x as i32;
    }
    (((x + unit - 1) / unit) * unit) as i32
}
