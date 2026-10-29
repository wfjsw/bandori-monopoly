//! `Mor:（toko）` -- C# `CardTokoSwap` (MatchHost.cs:4928-4995): swap one of your
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（toko）`）:
//! > （toko）：
//! > 选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回
//!
//! mortgaged deeds with another player's same-colour mortgaged deed.

use alloc::vec::Vec;

use bandori_std::ctx::is_color;
use card_sdk::abi::{mark, MarkFilter};
use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const TOKO_SWAP: CardDef =
    CardDef::new("Mor:（toko）", &[On::Play("", Some(cant_play), toko_swap)]).meta(&card_sdk::meta::CardMeta::new()
        .name("")
        .raw_name(r#"（toko）"#)
        .tags(&[])
        .text(CardText::para(r#"选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回"#))
        // sheet cell G8
        .fields(&[("group", r#"Morfonica"#), ("owner", r#"桐谷透子"#)]));

/// One swappable pair: `(mine, theirs)` -- both mortgaged, same colour,
/// `theirs` owned by another still-in player (C# `CardTokoSwap.Pairs`).
fn pairs(player_id: i32) -> Vec<(i32, i32)> {
    // Capacity up front: a `Vec` that grows through `__rust_realloc` makes
    // Kani's `same_allocation` abort every path (2026-10-28). 8 covers the
    // per-board pair count; a larger production list just reallocs as before.
    let mut list: Vec<(i32, i32)> = Vec::with_capacity(8);
    // 规则书: 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换」
    for mine in ctx::owned_tiles(player_id) {
        if !ctx::mortgaged_of(mine) {
            continue;
        }
        let group = ctx::tile_group(mine);
        let n = ctx::tile_count();
        for theirs in 0..n {
            let owner = ctx::tile_owner(theirs);
            if owner >= 0
                && owner != player_id
                && !ctx::player_out(owner)
                && ctx::mortgaged_of(theirs)
                // 「颜色相同」 is the colour *view*, not the raw group: `is_color`
                // honours `anyColor` / `colorFor`, and either tile counting as
                // the other's colour is 同色.
                && (is_color(player_id, theirs, group)
                    || is_color(owner, mine, ctx::tile_group(theirs)))
                // A `noTarget` mark makes `target_tile` refuse the pick, which
                // aborts the whole effect -- such a pair must not be offered.
                && ctx::count_marks(theirs, &MarkFilter::any().kind(mark::NO_TARGET)) == 0
            {
                list.push((mine, theirs));
            }
        }
    }
    list
}

/// C# `CardTokoSwap.Value` -- land price plus standing houses (`buy_price`).
fn value(t: i32) -> i32 {
    ctx::buy_price(t)
}

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardTokoSwap.WhyNot` refuses with no swappable pair
    // (「没有能交换的同色抵押地契（自己的和别人的都要是抵押中）」).
    if pairs(player_id).is_empty() {
        return Some(Msg::new(key!("toko_swap_why_not")));
    }
    None
}

fn toko_swap(player_id: i32) -> card_sdk::Asked {
    let ps = pairs(player_id);
    if ps.is_empty() {
        ctx::log(
            player_id,
            &Msg::new(key!("toko_swap_no_pair")).player_id("who", player_id),
        );
        return Ok(());
    }
    // 规则书: 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换」
    // C# `H.AskPick` over the pairs (`MatchHost.cs:4966`).
    // Exact capacity + a plain push loop (no `iter().map().collect()` extend
    // path -- both are Kani unwinding/pointer-check cliffs).
    let mut options: Vec<Msg> = Vec::with_capacity(ps.len());
    for &(mine, theirs) in ps.iter() {
        options.push(
            Msg::new(key!("toko_swap_pair"))
                .tile("mine", mine)
                .tile("theirs", theirs)
                .player_id("other", ctx::tile_owner(theirs)),
        );
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("toko_swap_title")),
        &Msg::new(key!("toko_swap_ask")),
        &options,
    )?;
    let (mine, theirs) = ps[(pick.max(0) as usize).min(ps.len() - 1)];
    let other = ctx::tile_owner(theirs);
    if other < 0 || other == player_id || ctx::player_out(other) {
        return Ok(());
    }
    // 规则书: 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换」 --
    //   C# still confirms the picked tile with `H.TargetTile` (`MatchHost.cs:4973`):
    //   a negation / immunity answer can still refuse the tile.
    if !ctx::target_tile(theirs) {
        return Ok(());
    }
    let num = value(mine);
    let num2 = value(theirs);
    // 规则书: 「交换」 -- C# `H.State.owners[m] = other; H.State.owners[t] = i`.
    ctx::set_owner(mine, other);
    ctx::set_owner(theirs, player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("toko_swap_swapped"))
            .player_id("who", player_id)
            .player_id("other", other)
            .tile("mine", mine)
            .tile("theirs", theirs),
    );
    // 规则书: 「地契+房屋价格更低的玩家向更高的玩家支付差价」
    // C# `H.PayR` of `Value(theirs) - Value(mine)` from the lower to the higher.
    // The 差价 comes before the 免费赎回: the payment can pause for raise-funds,
    // and the book leaves the received deed mortgaged until after it.
    let why = &Msg::new(key!("toko_swap_diff"));
    if num < num2 {
        ctx::transfer(player_id, other, num2 - num, why)?;
    } else if num2 < num {
        ctx::transfer(other, player_id, num - num2, why)?;
    }
    // 规则书: 「互换后将你得到的地契免费赎回」 -- written raw rather than through
    // `apply_redeem`: a host routine runs against a world that does not yet
    // carry this body's `set_owner` writes (they are replayed after the
    // pause), so `apply_redeem`'s owner guard still saw the previous owner and
    // returned 0. The flag clear is replayed; copy the rest of what
    // `apply_redeem` does -- the 「本回合进行过赎回操作」 record (turn-end) and
    // the `log.redeem` line.
    ctx::set_mortgaged(theirs, false);
    ctx::state::set(player_id, "redeemed", 1);
    ctx::state::set_expires(player_id, "redeemed", ctx::state::TURN_END);
    ctx::log(
        player_id,
        &Msg::new("log.redeem")
            .player_id("who", player_id)
            .n("cost", 0)
            .tile("tile", theirs),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn toko_swap_kani(player_id: i32) -> card_sdk::Asked {
    toko_swap(player_id)
}
