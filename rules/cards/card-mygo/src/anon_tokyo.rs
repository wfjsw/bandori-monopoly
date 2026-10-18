//! `MyGO:[千早爱音]Anon Tokyo` -- C# `CardAnonTokyo` (MatchHost.cs:6567-6640):
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:[千早爱音]Anon Tokyo`）:
//! > [千早爱音]Anon Tokyo：
//! > [限]：
//! > 自己所在的格子是[可购买格子]。
//! > [手]：
//! > [指定][使用者]所在当格的任一相邻的[可购买格子]，根据使用者所在格子和被[指定]格子的状态依次进行以下操作：
//! > 1. [使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的资金的一半并在上述的两个格子间放置1个[奇迹水晶]（上限1），被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半；
//! > 2. [使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]格子且[结算]。
//!
//! pick an adjacent buyable tile; if you own both tiles, pay half the dearer
//! deed to link them with a miracle crystal (cap 1), else step one tile over
//! and settle. A linked tile charges half of its partner's rent on top.

use alloc::vec::Vec;

use card_sdk::abi::{FilterKind, MarkFilter, TriggerKind};
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const ANON_TOKYO: CardDef = CardDef::new(
    "MyGO:[千早爱音]Anon Tokyo",
    &[
        On::Play("", "", Some(cant_play), anon_tokyo),
        On::Filter(&[FilterKind::PaySplitAdd], "card.placed && pay_is_rent && target == owner && value > 0", None, link_rent),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Anon Tokyo"#)
        .raw_name(r#"[千早爱音]Anon Tokyo"#)
        .tags(&[r#"手"#, r#"限"#])
        .text(r#"[限]：
自己所在的格子是[可购买格子]。
[手]：
[指定][使用者]所在当格的任一相邻的[可购买格子]，根据使用者所在格子和被[指定]格子的状态依次进行以下操作：
1. [使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的资金的一半并在上述的两个格子间放置1个[奇迹水晶]（上限1），被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半；
2. [使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]格子且[结算]。"#)
        .cell(r#"I8"#)
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", r#"千早爱音"#)]));

/// The field stand-in the `PaySplitAdd` rider hangs on (see [`link_rent`]).
const ID: &str = "MyGO:[千早爱音]Anon Tokyo";

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
        // 规则书[手]1: 「被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的
        // 一半」 -- rides the `PaySplitAdd` hook on a field stand-in (the C# puts the body
        // on `AnonLinkFx`, an `H.ExtraOf` attachment; the hook surface only dispatches
        // to placed cards, so this placement stands in for it -- same pattern as
        // `HHW:（育美）`'s `HagumiMarkFx`). Place the stand-in **before** the
        // crystal so the two rows bind to its instance and die with it.
        if !ctx::is_placed() {
            ctx::set_dest(ctx::Dest::Field);
            ctx::place_card(player_id, ID, &Msg::new(key!("anon_tokyo_linked")));
        }
        // 规则书[手]1: 「并在上述的两个格子间放置1个[奇迹水晶]（上限1）」 -- the
        // cap is on **the pair** (「放置1个…（上限1）」). `place_mark_pair` is
        // place-or-refuse atomic: if either end already carries a matching mark
        // it places **neither** row (no half-link). TODO(规则书) Class C:
        // whether a tile may sit in two links at once; the singular
        // 「1个…（上限1）」 leans no, and `place_mark_pair`'s either-end refusal
        // is that reading.
        let placed = ctx::place_mark_pair(
            here,
            t,
            key!("anon_tokyo_mark"),
            "",
            player_id,
            ctx::self_uid(),
            1,
            &Msg::new(key!("anon_tokyo_mark_note")).tile("tile", t),
            card_sdk::abi::Stack::Fresh,
        );
        if placed {
            ctx::log(
                player_id,
                &Msg::new(key!("anon_tokyo_linked"))
                    .tile("a", here)
                    .tile("b", t)
                    .card("card", "MyGO:[千早爱音]Anon Tokyo"),
            );
        }
    } else if rules_status::move_blocked(player_id).is_none() {
        // 规则书[手]2: 「[使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]
        // 改为向前或后移动1格到被[指定]格子且[结算]」 -- C# gates this branch on
        // `H.MoveWhyNot(i) == null` (else it logs 「这回合不能移动了」 and sets
        // `c.Effective = false`), then `H.CardMove(c, new MoveCtx { Steps = 1,
        // Reverse = Forward(here, t) != 1 })`. `card_move` is that main move;
        // `MoveCtx.Resolve` defaults to true, so the landing settles.
        ctx::plan::set_steps(1);
        // Forward(here, t) != 1 means the chosen tile is the step behind.
        ctx::plan::set_reverse(ctx::tile_forward(here, t) != 1);
        ctx::log(
            player_id,
            &Msg::new(key!("anon_tokyo_step")).tile("tile", t),
        );
        ctx::card_move(player_id);
    }
    Ok(())
}

/// Pure guard for [`link_rent`] -- the activation gate. `false` means the card
/// is not activated at all.
fn link_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

/// 规则书[手]1: 「被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半」
/// -- C# `AnonLinkFx.PaySplitAdd`: `p.IsRent && p.to == Seat && p.amount > 0`, then per
/// link add `RentOf(linked) / 2` when `p.tile` is either end and the other end is
/// owned by Player.
fn link_rent(player_id: i32, value: i32) -> i32 {
    let tile = trigger::tile();
    if tile < 0 {
        return value;
    }
    // The settled tile is one end of a link; the partner is the other end
    // (`TileMark.partner`, placed by `place_mark_pair`).
    let other = ctx::mark_partner(
        tile,
        &MarkFilter::any().kind(key!("anon_tokyo_mark")).owner(player_id),
    );
    if other < 0 {
        return value;
    }
    // C# `the other end is owned by Player`.
    if ctx::tile_owner(other) != player_id {
        return value;
    }
    // 「被连接的其他格子收费的一半」 -- half of the linked tile's rent.
    let bonus = ctx::rent_of(other) / 2;
    if bonus <= 0 {
        return value;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("anon_tokyo_link_rent"))
            .tile("tile", other)
            .i("n", bonus as i64),
    );
    value + bonus
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn anon_tokyo_kani(player_id: i32) -> card_sdk::Asked {
    anon_tokyo(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn link_rent_kani(player_id: i32, value: i32) -> i32 {
    link_rent(player_id, value)
}
