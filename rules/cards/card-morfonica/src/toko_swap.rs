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

fn value(t: i32) -> i32 {
    ctx::buy_price(t)
}

fn cant_play(player_id: i32) -> Option<Msg> {
    if pairs(player_id).is_empty() {
        return Some(Msg::new(key!("toko_swap_blocked")));
    }
    None
}

/// `pick` asks for one tile and answers its **id**, so the answer survives the
/// body's own writes: `set_owner` is a host request -- it applies to the live
/// world and the body re-runs, and by then the pair list no longer holds the
/// pair (the player has given `mine` away). An index into a fresh list would
/// slide; an id does not, and an empty list on a re-run is harmless because
/// the answer is already logged.
fn pick(player_id: i32, text: &Msg, tiles: &[i32]) -> Result<i32, card_sdk::Prompt> {
    let mut labels: Vec<Msg> = Vec::with_capacity(tiles.len());
    let mut prices: Vec<i32> = Vec::with_capacity(tiles.len());
    for &t in tiles {
        labels.push(Msg::new("ask.tileOption").tile("tile", t));
        prices.push(0);
    }
    ctx::ask_tiles(
        player_id,
        &Msg::new(key!("toko_swap_title")),
        text,
        tiles,
        &labels,
        &prices,
        -1,
    )
}

fn toko_swap(player_id: i32) -> card_sdk::Asked {
    let ps = pairs(player_id);
    // 规则书: 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换」
    // One prompt per side. The candidate lists only have to be right on the
    // pass the player answers them; the id answers carry the rest.
    let mut mines: Vec<i32> = Vec::with_capacity(ps.len());
    for &(mine, _) in ps.iter() {
        if !mines.contains(&mine) {
            mines.push(mine);
        }
    }
    // The ask runs even with an empty list: on a re-run after the first
    // `set_owner` the player no longer owns `mine`, so the list is empty
    // there while the logged answer still names the pair.
    let mine = pick(player_id, &Msg::new(key!("toko_swap_ask")), &mines)?;
    if mine < 0 {
        // A decline is silent; an empty offer list is the no-pair case.
        if mines.is_empty() {
            ctx::log(
                player_id,
                &Msg::new(key!("toko_swap_no_pair")).player_id("who", player_id),
            );
        }
        return Ok(());
    }
    let mut theirs_opts: Vec<i32> = Vec::with_capacity(ps.len());
    for &(m, t) in ps.iter() {
        if m == mine {
            theirs_opts.push(t);
        }
    }
    let theirs = pick(
        player_id,
        &Msg::new(key!("toko_swap_ask_theirs")).tile("mine", mine),
        &theirs_opts,
    )?;
    if theirs < 0 || mine == theirs {
        return Ok(());
    }
    // The counterparty, whichever half of the swap has already landed: while
    // `mine` is still the player's, `theirs`'s owner names it; once handed
    // over, `mine`'s owner does.
    let other = match ctx::tile_owner(mine) {
        owner if owner != player_id && owner >= 0 => owner,
        _ => ctx::tile_owner(theirs),
    };
    if other < 0 || other == player_id || ctx::player_out(other) {
        return Ok(());
    }
    // Both deed owners are the player or the counterparty -- true before the
    // swap, between the two writes (both `other`'s) and after it.
    for t in [mine, theirs] {
        let o = ctx::tile_owner(t);
        if o != player_id && o != other {
            return Ok(());
        }
        if !ctx::mortgaged_of(t) {
            return Ok(());
        }
    }
    if !(is_color(player_id, theirs, ctx::tile_group(mine))
        || is_color(other, mine, ctx::tile_group(theirs)))
    {
        return Ok(());
    }
    // A `noTarget` mark makes `target_tile` refuse the pick.
    if ctx::count_marks(theirs, &MarkFilter::any().kind(mark::NO_TARGET)) != 0 {
        return Ok(());
    }
    if !ctx::target_tile(theirs) {
        return Ok(());
    }
    let num = value(mine);
    let num2 = value(theirs);
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
    // The 差价 comes before the 免费赎回: the payment can pause for raise-funds, and the
    // book leaves the received deed mortgaged until after it.
    let reason = &Msg::new(key!("toko_swap_diff"));
    if num < num2 {
        ctx::transfer(player_id, other, num2 - num, reason)?;
    } else if num2 < num {
        ctx::transfer(other, player_id, num - num2, reason)?;
    }
    // 规则书: 「互换后将你得到的地契免费赎回」
    ctx::commit_redeem(player_id, theirs, 0)?;
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
