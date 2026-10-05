//! `PPP:迷宫般的仓库` -- C# `CardMazeWarehouse` (MatchHost.cs:8814-8872): roll 1d10
//! and walk from 流星堂 past that many unowned buyable tiles, buying for free.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:迷宫般的仓库`）:
//! > 迷宫般的仓库：
//! > 位于“流星堂”前后5格内时，可打出此卡，投掷1d10，从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地，视为你的主要移动且本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋。如果[经过]“流星堂”则[强制停下]且[消耗]6000资金
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const MAZE_WAREHOUSE: CardDef = CardDef {
    id: "PPP:迷宫般的仓库",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「位于“流星堂”前后5格内时，可打出此卡」 -- C# `CardMazeWarehouse.WhyNot`
    // refuses with 「只有在流星堂前后 5 格以内才能打出」 outside that ring distance.
    // TODO(规则书): the C# also returns `H.MoveWhyNot(seat)` when in range (main move
    //   already used / movement blocked) -- needs `H.MoveWhyNot` (this gate currently
    //   only covers the range check).
    let ryuseido = ctx::tile_named("流星堂");
    if ryuseido >= 0 && ctx::dist(ctx::seat_pos(seat), ryuseido) > 5 {
        return Some(Msg::new(key!("maze_warehouse_range")).seat("who", seat));
    }
    None // playable
}

/// C# 「无主可购买地」 -- `H._tiles[t].IsBuyable && H.State.owners[t] < 0`.
fn buyable_unowned(t: i32) -> bool {
    ctx::tile_owner(t) < 0 && ctx::is_buyable(t)
}

fn play(seat: i32) {
    let ryuseido = ctx::tile_named("流星堂");
    let n = ctx::tile_count();
    if ryuseido < 0 || n <= 0 {
        return;
    }
    // 规则书: 「投掷1d10」
    let want = ctx::roll(seat, 1, 10);
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
        seat,
        &Msg::new(key!("maze_warehouse_free_buy")).seat("who", seat).i("n", want as i64),
    );
    // TODO(ABI): 「视为你的主要移动」 / the walk itself -- needs `H.CardMove` with
    //   `MoveCtx { Steps, Start = ryuseido, StopAt = ryuseido when looped }` so the
    //   seat actually passes tiles and settles (or force-stops) on `dest`.
    // TODO(ABI): 「本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋」
    //   -- needs the turn flags `FreeBuy` / `RazeOnBuy` (C# `H._turnCtx`).
    ctx::log(
        seat,
        &Msg::new(key!("maze_warehouse_walk"))
            .seat("who", seat)
            .tile("tile", dest)
            .i("n", steps as i64),
    );
    if looped {
        // 规则书: 「如果[经过]“流星堂”则[强制停下]且[消耗]6000资金」 -- C#
        // `H.LoseR(i, 6000, ...)` after the forced stop. The walk itself is TODO
        // above, so the penalty is only logged here (charging without moving would
        // be the cost without the travel).
        ctx::log(seat, &Msg::new(key!("maze_warehouse_loop")).seat("who", seat));
        // TODO(ABI): 「[强制停下]且[消耗]6000资金」 -- needs the AbnormalGate forced-stop
        //   gate (C# `Abnormal{Kind = "stop"}` / `m.Stopped`) and then
        //   `ctx::pay(seat, 6000, ...)` against a `maze_warehouse_lose` reason.
    }
}