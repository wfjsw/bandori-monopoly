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

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const ANON_TOKYO: CardDef = CardDef::new("MyGO:[千早爱音]Anon Tokyo", &[
    On::Play(Some(cant_play), anon_tokyo)]);

fn cant_play(player_id: i32) -> Option<Msg> {
    let n = ctx::tile_count();
    let here = ctx::player_pos(player_id);
    // 规则书[限]: 「自己所在的格子是[可购买格子]」 -- C# `CardAnonTokyo.WhyNot`
    // refuses off a buyable tile (`H._tiles[pos].IsBuyable`).
    if here < 0 || n <= 0 || !ctx::is_buyable(here) {
        return Some(Msg::new(key!("anon_tokyo_why_not_here")));
    }
    // C# also refuses when `Adjacent(player_id)` is empty.
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

fn anon_tokyo(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    let here = ctx::player_pos(player_id);
    // 规则书[限]: 「自己所在的格子是[可购买格子]」 -- C# `H._tiles[pos].IsBuyable`
    // (kind "property" | "ring").
    if here < 0 || n <= 0 || !ctx::is_buyable(here) {
        return Ok(());
    }
    // 规则书[手]: 「[指定][使用者]所在当格的任一相邻的[可购买格子]」 -- C#
    // `CardAnonTokyo.Adjacent` over `(pos ± 1) % n` filtered by `IsBuyable`.
    let adj = adjacent(here, n);
    if adj.is_empty() {
        return Ok(());
    }
    let t = ctx::ask_tile(
        player_id,
        &Msg::new(key!("anon_tokyo_title")),
        &Msg::new(key!("anon_tokyo_ask")),
        &adj,
    )?;
    // 规则书[手]: 「[指定]…可购买格子」 -- C# `H.TargetTile(c, t, tt)` after the
    // tile prompt: another player's tile also targets its owner (ImmuneAll +
    // the targeted/target [反击] window). Fail = the card does nothing more.
    if !ctx::target_tile(t) {
        return Ok(());
    }
    if ctx::tile_owner(here) == player_id && ctx::tile_owner(t) == player_id {
        // 规则书[手]1: 「[使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的
        // 资金的一半」 -- C# `PayCtx { amount = max(price(here), price(t)) / 2,
        // kind = "lose", must = false }` (money is destroyed; a short purse
        // pays what it has). `tile_price` is the C# `H._tiles[t].price` land
        // price (houses are extra; cf. `buy_price`).
        let amount = ctx::tile_price(here).max(ctx::tile_price(t)) / 2;
        if amount > 0 {
            ctx::pay(player_id, amount, &Msg::new(key!("anon_tokyo_pay")))?;
        }
        // 规则书[手]1: 「并在上述的两个格子间放置1个[奇迹水晶]」 -- C#
        // `AnonLinkFx.Link(here, t)` puts an `Anon Tokyo` mark on both tiles.
        ctx::add_mark(here, player_id, key!("anon_tokyo_mark"), &Msg::new(key!("anon_tokyo_mark_note")).tile("tile", t));
        ctx::add_mark(t, player_id, key!("anon_tokyo_mark"), &Msg::new(key!("anon_tokyo_mark_note")).tile("tile", here));
        ctx::log(
            player_id,
            &Msg::new(key!("anon_tokyo_linked")).tile("a", here).tile("b", t).card("card", "MyGO:[千早爱音]Anon Tokyo"),
        );
        // TODO(规则书)1: 「被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的
        // 一半」 -- the `Fx.PayAdd` hook kind is in (`On::Hook(&[HookKind::PayAdd], …)`),
        // but the C# puts the body on `AnonLinkFx`, an `H.ExtraOf` attachment on
        // the player that outlives this hand card (which goes to the graveyard
        // after Play, so its own hooks never run). Still held on `H.ExtraOf`
        // attachments: once one exists, the body is `p.IsRent && p.to == Seat &&
        // p.amount > 0`, then per link add `ceil_to(RentOf(linked) / 2, 10)` when
        // `p.tile` is either end and the other end is owned by Player.
    } else if ctx::cant_move(player_id).is_none() {
        // 规则书[手]2: 「[使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]
        // 改为向前或后移动1格到被[指定]格子且[结算]」 -- C# gates this branch on
        // `H.MoveWhyNot(i) == null` (else it logs 「这回合不能移动了」 and sets
        // `c.Effective = false`), then `H.CardMove(c, new MoveCtx { Steps = 1,
        // Reverse = Forward(here, t) != 1 })`. `card_move` is that main move;
        // `MoveCtx.Resolve` defaults to true, so the landing settles.
        ctx::plan::set_steps(1);
        // Forward(here, t) != 1 means the chosen tile is the step behind.
        ctx::plan::set_reverse(ctx::tile_forward(here, t) != 1);
        ctx::log(player_id, &Msg::new(key!("anon_tokyo_step")).tile("tile", t));
        ctx::card_move(player_id);
    }
    Ok(())
}
