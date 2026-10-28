//! `PPP:迷宫般的仓库` -- C# `CardMazeWarehouse` (MatchHost.cs:8814-8872): roll 1d10
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:迷宫般的仓库`）:
//! > 迷宫般的仓库：
//! > 位于“流星堂”前后5格内时，可打出此卡，投掷1d10，从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地，视为你的主要移动且本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋。如果[经过]“流星堂”则[强制停下]且[消耗]6000资金
//!
//! and walk from 流星堂 past that many unowned buyable tiles, buying for free.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{roll_source, FilterKind, RollPurpose};
use card_sdk::CardText;
use card_sdk::{key, CardDef, DealValue, Filter, Msg, On};

pub const MAZE_WAREHOUSE: CardDef = CardDef::new(
    "PPP:迷宫般的仓库",
    &[
        On::Play("", Some(cant_play), play),
        // 规则书: 「本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋」
        // -- a `BuySet` 0 (free) and a `BuyAssign` raze, riding a `ctx::linger`
        // instance for the turn (`docs/PURCHASE.md`).
        On::Filter(Filter::BuySet(card_sdk::pre::MINE, None, free_buy)),
        On::Filter(Filter::BuyAssign(card_sdk::pre::MINE, None, raze)),
    ],
).props(&[(card_sdk::abi::prop::EST_COST, 6000)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"迷宫般的仓库"#)
        .raw_name(r#"迷宫般的仓库"#)
        .tags(&[])
        .text(CardText::para(r#"位于“流星堂”前后5格内时，可打出此卡，投掷1d10，从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地，视为你的主要移动且本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋。如果[经过]“流星堂”则[强制停下]且[消耗]6000资金"#))
        // sheet cell B10
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]));

/// 规则书: 「本回合购买格子不[消耗]资金」 -- `BuySet` is the free / fixed stage
/// (`docs/PURCHASE.md`), so the hook returns 0 as the running price.
fn free_buy(_player_id: i32, value: i32) -> i32 {
    if trigger::tile() < 0 {
        return value;
    }
    0
}

/// 规则书: 「如果购买则拆除那个格子上的所有房屋」 -- `BuyAssign` rewrites the deal's
/// post-commit house count (`docs/PURCHASE.md`).
fn raze(_player_id: i32, mut v: DealValue) -> DealValue {
    if trigger::tile() < 0 {
        return v;
    }
    v.houses = Some(0);
    v
}

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「位于“流星堂”前后5格内时，可打出此卡」 -- C# `CardMazeWarehouse.WhyNot`
    // refuses with 「只有在流星堂前后 5 格以内才能打出」 outside that ring distance,
    // then returns `H.MoveWhyNot(seat)` when in range.
    let ryuseido = ctx::tile_named("流星堂");
    if ryuseido < 0 || ctx::dist(ctx::player_pos(player_id), ryuseido) > 5 {
        return Some(Msg::new(key!("maze_warehouse_range")).player_id("who", player_id));
    }
    rules_status::move_blocked(player_id)
}

/// C# 「无主可购买地」 -- `H._tiles[t].IsBuyable && H.State.owners[t] < 0`.
fn buyable_unowned(t: i32) -> bool {
    ctx::tile_owner(t) < 0 && ctx::is_buyable(t)
}

fn play(player_id: i32) -> card_sdk::Asked {
    let ryuseido = ctx::tile_named("流星堂");
    let n = ctx::tile_count();
    if ryuseido < 0 || n <= 0 {
        return Ok(());
    }
    // 规则书: 「投掷1d10」
    let want = ctx::roll(player_id, 1, 10, RollPurpose::Effect, roll_source::CARD)?;
    // 规则书: 「从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地」
    // -- C# `CardMazeWarehouse.Play` walks forward from 流星堂, counting buyable
    // unowned tiles; wrapping the whole ring without reaching `want` lands back on
    // 流星堂.
    let mut steps = n;
    let mut seen = 0;
    for j in 1..=n {
        let t = (ryuseido + j) % n;
        if t == ryuseido {
            steps = j;
            break;
        }
        if buyable_unowned(t) {
            seen += 1;
            if seen >= want {
                steps = j;
                break;
            }
        }
    }
    // 规则书: 「本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋」
    // -- a turn-scoped `BuySet` 0 + `BuyAssign` raze instance (`docs/PURCHASE.md`),
    // the hand-card home that replaces the old `free_buy` / `raze_on_buy` flags.
    // `seen` is the buyable count the walk will actually cross.
    ctx::log(
        player_id,
        &Msg::new(key!("maze_warehouse_free_buy"))
            .player_id("who", player_id)
            .i("n", seen as i64),
    );
    ctx::linger(player_id, 0);
    // 规则书: 「视为你的主要移动」 / 「从“流星堂”开始移动」 -- C# `H.CardMove(c, new
    // MoveCtx { Steps = steps, Start = ryuseido, StartWhy = "迷宫般的仓库" })`
    // (MatchHost.cs:8854-8859): the walk runs now as the main move, starting from
    // 流星堂 (`set_start`) for `steps` tiles (`set_steps`) and settling on
    // arrival (`MoveCtx.Resolve` defaults to true).
    // The shared turn plan is never reset between moves, so clear what this walk
    // must not inherit from an earlier one.
    ctx::plan::set_stop_at(-1);
    ctx::plan::set_extra_steps(0);
    ctx::plan::set_reverse(false);
    ctx::plan::set_resolve(true);
    ctx::plan::set_steps(steps);
    ctx::plan::set_start(ryuseido, "迷宫般的仓库");
    // 规则书: 「如果[经过]“流星堂”则[强制停下]」 -- C# `moveCtx.StopAt = ryuseido`
    // whenever the walk crosses back: `set_stop_at` forces the walk to stop on 流星堂
    // (the walk settles at the stop tile; `ctx::plan::stop_at()` reports it).
    ctx::plan::set_stop_at(ryuseido);
    ctx::card_move(player_id);
    let arrived = ctx::player_pos(player_id);
    // The log names where the piece actually ended, not the planned landing (an
    // effect may stop the walk early).
    ctx::log(
        player_id,
        &Msg::new(key!("maze_warehouse_walk"))
            .player_id("who", player_id)
            .tile("tile", arrived)
            .i("n", steps as i64),
    );
    // 规则书: 「如果[经过]“流星堂”则[强制停下]且[消耗]6000资金」 -- the charge keys
    // on the stop the walk actually made: a walk cut short elsewhere pays
    // nothing, and a settle that moves the piece off 流星堂 (an event) still
    // pays. `seen < want` is the wrap that reaches 流星堂; `stop_at` still names
    // the tile unless another effect force-stopped the walk elsewhere.
    if seen < want && ctx::plan::stop_at() == ryuseido && !ctx::player_out(player_id) {
        // 规则书: 「[消耗]6000资金」 -- C# `H.LoseR(i, 6000, "迷宫般的仓库")`
        // (MatchHost.cs:8866-8869) after the forced stop.
        ctx::log(
            player_id,
            &Msg::new(key!("maze_warehouse_loop")).player_id("who", player_id),
        );
        ctx::pay(player_id, 6000, &Msg::new(key!("maze_warehouse_lose")))?;
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn free_buy_kani(player_id: i32, value: i32) -> i32 {
    free_buy(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn raze_kani(player_id: i32, v: card_sdk::DealValue) -> card_sdk::DealValue {
    raze(player_id, v)
}
