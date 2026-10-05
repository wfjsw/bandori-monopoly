//! `Mor:（toko）` -- C# `CardTokoSwap` (MatchHost.cs:4928-4995): swap one of your
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（toko）`）:
//! > （toko）：
//! > 选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回
//!
//! mortgaged deeds with another player's same-colour mortgaged deed.

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const TOKO_SWAP: CardDef = CardDef::new("Mor:（toko）", &[
    On::Play(toko_swap),
    On::CantPlay(cant_play),
]);

/// One swappable pair: `(mine, theirs)` -- both mortgaged, same colour group,
/// `theirs` owned by another still-in player (C# `CardTokoSwap.Pairs`).
fn pairs(player_id: i32) -> Vec<(i32, i32)> {
    let mut list: Vec<(i32, i32)> = Vec::new();
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
                && ctx::tile_group(theirs) == group
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

fn toko_swap(player_id: i32) {
    let ps = pairs(player_id);
    if ps.is_empty() {
        ctx::log(player_id, &Msg::new(key!("toko_swap_no_pair")).player_id("who", player_id));
        return;
    }
    // 规则书: 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换」
    // C# `H.AskPick` over the pairs (`MatchHost.cs:4966`).
    let options: Vec<Msg> = ps
        .iter()
        .map(|&(mine, theirs)| {
            Msg::new(key!("toko_swap_pair"))
                .tile("mine", mine)
                .tile("theirs", theirs)
                .player_id("other", ctx::tile_owner(theirs))
        })
        .collect();
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("toko_swap_title")),
        &Msg::new(key!("toko_swap_ask")),
        &options,
    );
    let (mine, theirs) = ps[(pick.max(0) as usize).min(ps.len() - 1)];
    let other = ctx::tile_owner(theirs);
    if other < 0 || other == player_id || ctx::player_out(other) {
        return;
    }
    // 规则书: the C# confirms the picked tile with `H.TargetTile` before the
    //   swap (`MatchHost.cs:4973`); false when the tile cannot be targeted.
    if !ctx::target_tile(theirs) {
        return;
    }
    let num = value(mine);
    let num2 = value(theirs);
    // 规则书: 「交换」 -- C# `H.State.owners[m] = other; H.State.owners[t] = i`.
    ctx::set_owner(mine, other);
    ctx::set_owner(theirs, player_id);
    // 规则书: 「互换后将你得到的地契免费赎回」 -- C# `H.State.mortgaged[pick.theirs] = false`.
    ctx::set_mortgaged(theirs, false);
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
    let why = &Msg::new(key!("toko_swap_diff"));
    if num < num2 {
        ctx::transfer(player_id, other, num2 - num, why);
    } else if num2 < num {
        ctx::transfer(other, player_id, num - num2, why);
    }
}