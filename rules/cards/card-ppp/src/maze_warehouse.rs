//! `PPP:迷宫般的仓库` -- C# `CardMazeWarehouse` (MatchHost.cs:8814-8872): roll 1d10
//! and walk from 流星堂 past that many unowned buyable tiles, buying for free.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:迷宫般的仓库`）:
//! > 迷宫般的仓库：
//! > 位于“流星堂”前后5格内时，可打出此卡，投掷1d10，从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地，视为你的主要移动且本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋。如果[经过]“流星堂”则[强制停下]且[消耗]6000资金
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const MAZE_WAREHOUSE: CardDef = CardDef::new("PPP:迷宫般的仓库", &[
    On::Play(Some(cant_play), play)]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「位于“流星堂”前后5格内时，可打出此卡」 -- C# `CardMazeWarehouse.WhyNot`
    // refuses with 「只有在流星堂前后 5 格以内才能打出」 outside that ring distance,
    // then returns `H.MoveWhyNot(seat)` when in range.
    let ryuseido = ctx::tile_named("流星堂");
    if ryuseido >= 0 && ctx::dist(ctx::player_pos(player_id), ryuseido) > 5 {
        return Some(Msg::new(key!("maze_warehouse_range")).player_id("who", player_id));
    }
    ctx::cant_move(player_id)
}

/// C# 「无主可购买地」 -- `H._tiles[t].IsBuyable && H.State.owners[t] < 0`.
fn buyable_unowned(t: i32) -> bool {
    ctx::tile_owner(t) < 0 && ctx::is_buyable(t)
}

fn play(player_id: i32) {
    let ryuseido = ctx::tile_named("流星堂");
    let n = ctx::tile_count();
    if ryuseido < 0 || n <= 0 {
        return;
    }
    // 规则书: 「投掷1d10」
    let want = ctx::roll(player_id, 1, 10);
    // 规则书: 「从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地」
    // -- C# `CardMazeWarehouse.Play` walks forward from 流星堂, counting buyable
    // unowned tiles; wrapping the whole ring without reaching `want` lands back on
    // 流星堂 (`loop`).
    let mut steps = n;
    let mut seen = 0;
    let mut looped = false;
    for j in 1..=n {
        let t = (ryuseido + j) % n;
        if t == ryuseido {
            looped = true;
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
    let dest = (ryuseido + steps) % n;
    // 规则书: 「本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋」
    // -- C# `H._turnCtx.FreeBuy = true; H._turnCtx.RazeOnBuy = true`.
    ctx::log(
        player_id,
        &Msg::new(key!("maze_warehouse_free_buy")).player_id("who", player_id).i("n", want as i64),
    );
    // 规则书: 「本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋」
    // -- C# `H._turnCtx.FreeBuy = true; H._turnCtx.RazeOnBuy = true`.
    ctx::set_free_buy(true);
    ctx::set_raze_on_buy(true);
    // 规则书: 「视为你的主要移动」 / 「从“流星堂”开始移动」 -- C# `H.CardMove(c, new
    // MoveCtx { Steps = steps, Start = ryuseido, StartWhy = "迷宫般的仓库" })`
    // (MatchHost.cs:8854-8859): the walk runs now as the main move, starting from
    // 流星堂 (`set_start`) for `steps` tiles (`set_steps`) and settling on
    // arrival (`MoveCtx.Resolve` defaults to true).
    ctx::plan::set_steps(steps);
    ctx::plan::set_start(ryuseido, "迷宫般的仓库");
    // 规则书: 「如果[经过]“流星堂”则[强制停下]」 -- C# `moveCtx.StopAt = ryuseido`
    // when the walk wraps: `set_stop_at` forces the walk to stop on 流星堂 (the
    // walk settles at the stop tile; `plan::stopped()` is the read-only check).
    if looped {
        ctx::plan::set_stop_at(ryuseido);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("maze_warehouse_walk"))
            .player_id("who", player_id)
            .tile("tile", dest)
            .i("n", steps as i64),
    );
    ctx::card_move(player_id);
    if looped && !ctx::player_out(player_id) {
        // 规则书: 「[消耗]6000资金」 -- C# `H.LoseR(i, 6000, "迷宫般的仓库")`
        // (MatchHost.cs:8866-8869) after the forced stop.
        ctx::log(player_id, &Msg::new(key!("maze_warehouse_loop")).player_id("who", player_id));
        ctx::pay(player_id, 6000, &Msg::new(key!("maze_warehouse_lose")));
    }
}