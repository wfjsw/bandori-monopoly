//! `R:向着顶点` -- C# `CardToThePeak` (MatchHost.cs:10360-10422): walk to the next
//!
//! 规则书（docs/rulebook/cards.json, id `R:向着顶点`）:
//! > 向着顶点：
//! >  移动到下一个可被购买的livehouse格子，视为你的主要移动。若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋。
//!
//! free Livehouse, or build one of yours at 1.5x cost.

use alloc::vec::Vec;

use bandori_std::ctx;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const TO_THE_PEAK: CardDef = CardDef::new("R:向着顶点", &[On::Play("", Some(cant_play), play)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"向着顶点"#)
        .raw_name(r#"向着顶点"#)
        .tags(&[])
        .text(CardText::para(r#"移动到下一个可被购买的livehouse格子，视为你的主要移动。若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋。"#))
        // sheet cell E4
        .fields(&[("group", r#"Roselia"#), ("owner", "")]));

/// The buyable Livehouse deeds (C# `H.LiveHouses`: `IsBuyable && IsColor(6)`),
/// where `IsColor` sees both the `anyColor` tile prop and this player's
/// `colorFor:<p>`.
fn livehouses(player_id: i32) -> Vec<i32> {
    (0..ctx::tile_count())
        .filter(|&t| ctx::is_buyable(t) && ctx::is_color(player_id, t, 6))
        .collect()
}

/// C# `H.WhyNotBuildOn(i, t)` -- the engine's own build gate, so every refusal
/// the build step honours applies here too (`ctx::can_build_on` is that same
/// question: own/buyable, the 「无法加盖房屋」 locks, mortgage, the rent-table
/// level cap, full and the payer gate).
fn why_not_build_on(player_id: i32, t: i32) -> Option<&'static str> {
    // TODO(规则书)（为属于你的一个livehouse格子加盖一层房屋）: the sheet names no
    //   RiNG exception, but 「所有RiNG不可升级」 keeps RiNGs unupgradeable
    //   (OPEN-RING-BUILD, `docs/rulebook/spec/OPEN-QUESTIONS.md`) -- the C#
    //   reading is taken, so `can_build_on` refuses a RiNG via its no-upgrade
    //   check.
    if ctx::can_build_on(player_id, t) {
        None
    } else {
        Some("blocked")
    }
}

/// C# `CardToThePeak.WhyNot`.
fn cant_play(player_id: i32) -> Option<Msg> {
    let free: Vec<i32> = livehouses(player_id)
        .into_iter()
        .filter(|&t| ctx::tile_owner(t) < 0)
        .collect();
    if !free.is_empty() {
        // C# `H.Moving(seat)` -- the walk branch needs a movable player.
        if rules_status::stun::get(player_id) > 0 {
            return Some(Msg::new(key!("to_the_peak_stunned")));
        }
        if rules_status::stay::get(player_id) > 0
            && bandori_std::ctx::state::get(player_id, bandori_std::state_key::UNSTOPPABLE) <= 0
        {
            // `H.Moving` lets a `[停留]` player through when 「不可阻挡」 is up.
            return Some(Msg::new(key!("to_the_peak_stuck")));
        }
        // 规则书: 「视为你的主要移动」 -- `H.MoveWhyNot`: the main move must still be
        // available (own turn, not already moved, not skipped).
        return rules_status::move_blocked(player_id);
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
        // steps that settles at the destination. `Forward` 0 (the only free tile
        // is underfoot) is taken as a full lap, as in the `min_by_key` above.
        let num = ctx::tile_forward(pos, to);
        let num = if num == 0 { ctx::tile_count() } else { num };
        ctx::plan::set_steps(num);
        ctx::plan::set_resolve(true);
        // 规则书: 「视为你的主要移动」 -- `card_move` runs the walk as the main move.
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
    // TODO(规则书)（加盖一层房屋）: a 加盖 is a build and goes through the build
    //   hooks -- the raw `ctx::add_house` below raises no `buildBefore` /
    //   `buildAfter`, so 卡池BUG's belt, 奇迹's crystal and 修复公告's halving
    //   never see it. `ctx::card_build` would run the build body but pays the
    //   table cost only (`set_build_cost_pct` clamps at 100, so no 1.5x).
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

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
