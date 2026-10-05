//! `MyGO:[千早爱音]Anon Tokyo` -- C# `CardAnonTokyo` (MatchHost.cs:6567-6640):
//! pick an adjacent buyable tile; if you own both tiles, pay half the dearer
//! deed to link them with a miracle crystal, else step one tile over and settle.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:[千早爱音]Anon Tokyo`）:
//! > [千早爱音]Anon Tokyo：
//! > [限]：
//! > 自己所在的格子是[可购买格子]。
//! > [手]：
//! > [指定][使用者]所在当格的任一相邻的[可购买格子]，根据使用者所在格子和被[指定]格子的状态依次进行以下操作：
//! > 1. [使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的资金的一半并在上述的两个格子间放置1个[奇迹水晶]，被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半；
//! > 2. [使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]格子且[结算]。
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const ANON_TOKYO: CardDef = CardDef {
    id: "MyGO:[千早爱音]Anon Tokyo",
    play: Some(anon_tokyo),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    let n = ctx::tile_count();
    let here = ctx::seat_pos(seat);
    // 规则书[限]: 「自己所在的格子是[可购买格子]」 -- C# `CardAnonTokyo.WhyNot`
    // refuses off a buyable tile (`H._tiles[pos].IsBuyable`).
    if here < 0 || n <= 0 || !ctx::is_buyable(here) {
        return Some(Msg::new(key!("anon_tokyo_why_not_here")));
    }
    // C# also refuses when `Adjacent(seat)` is empty.
    if adjacent(here, n).is_empty() {
        return Some(Msg::new(key!("anon_tokyo_why_not_adj")));
    }
    None
}

/// C# `CardAnonTokyo.Adjacent` -- the buyable tiles at `(pos ± 1) % n`.
fn adjacent(here: i32, n: i32) -> Vec<i32> {
    let mut adj: Vec<i32> = Vec::new();
    for d in [1, -1] {
        let t = ((here + d) % n + n) % n;
        if ctx::is_buyable(t) && !adj.contains(&t) {
            adj.push(t);
        }
    }
    adj
}

fn anon_tokyo(seat: i32) {
    let n = ctx::tile_count();
    let here = ctx::seat_pos(seat);
    // 规则书[限]: 「自己所在的格子是[可购买格子]」 -- C# `H._tiles[pos].IsBuyable`
    // (kind "property" | "ring").
    if here < 0 || n <= 0 || !ctx::is_buyable(here) {
        return;
    }
    // 规则书[手]: 「[指定][使用者]所在当格的任一相邻的[可购买格子]」 -- C#
    // `CardAnonTokyo.Adjacent` over `(pos ± 1) % n` filtered by `IsBuyable`.
    let adj = adjacent(here, n);
    if adj.is_empty() {
        return;
    }
    let t = ctx::ask_tile(
        seat,
        &Msg::new(key!("anon_tokyo_title")),
        &Msg::new(key!("anon_tokyo_ask")),
        &adj,
    );
    // TODO(规则书): the C# `H.TargetTile(c, t, tt)` targeting gate after the tile
    // prompt -- needs the H.Target targeting pipeline.
    if ctx::tile_owner(here) == seat && ctx::tile_owner(t) == seat {
        // 规则书[手]1: 「[使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的
        // 资金的一半」 -- C# `PayCtx { amount = max(price(here), price(t)) / 2,
        // kind = "lose", must = false }` (money is destroyed; a short purse
        // pays what it has). `tile_price` is the C# `H._tiles[t].price` land
        // price (houses are extra; cf. `buy_price`).
        let amount = ctx::tile_price(here).max(ctx::tile_price(t)) / 2;
        if amount > 0 {
            ctx::pay(seat, amount, &Msg::new(key!("anon_tokyo_pay")));
        }
        // 规则书[手]1: 「并在上述的两个格子间放置1个[奇迹水晶]」 -- C#
        // `AnonLinkFx.Link(here, t)` puts an `Anon Tokyo` mark on both tiles.
        ctx::add_mark(here, seat, key!("anon_tokyo_mark"), &Msg::new(key!("anon_tokyo_mark_note")).tile("tile", t));
        ctx::add_mark(t, seat, key!("anon_tokyo_mark"), &Msg::new(key!("anon_tokyo_mark_note")).tile("tile", here));
        ctx::log(
            seat,
            &Msg::new(key!("anon_tokyo_linked")).tile("a", here).tile("b", t).card("card", "MyGO:[千早爱音]Anon Tokyo"),
        );
        // TODO(规则书)1: 「被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的
        // 一半」 -- needs the Fx.PayAdd hook (C# `AnonLinkFx.PayAdd` adds
        // `ceil_to(RentOf(linked) / 2, 10)` when rent is charged on either end).
    } else {
        // 规则书[手]2: 「[使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]
        // 改为向前或后移动1格到被[指定]格子且[结算]」 -- C# `H.CardMove(c, new
        // MoveCtx { Steps = 1, Reverse = Forward(here, t) != 1 })`.
        ctx::log(seat, &Msg::new(key!("anon_tokyo_step")).tile("tile", t));
        // TODO(规则书)2: 「进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]
        // 格子且[结算]」 -- needs the H.CardMove / main-move routine (one step
        // toward the target, settling on arrival) and the `H.MoveWhyNot` gate
        // (C# logs 「这回合不能移动了」 and sets `c.Effective = false` when the
        // turn's main move is already spent). `teleport_to` jumps without
        // settling and does not consume the main move.
    }
}
