//! `Sumimi:Sumimi不会解散哦` -- C# `CardNoBreakup` (MatchHost.cs:11038-11118):
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sumimi不会解散哦`）:
//! > Sumimi不会解散哦：
//! >  当你本回合未进行过抵押/赎回操作且资金不含有相同数字时可打出，进行一次可使你的资金变动为拥有相同数字的传送，视为你的主要移动；若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果
//!
//! settle-teleport that must leave money with a repeated digit, else fully revert.

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const NO_BREAKUP: CardDef = CardDef::new(
    "Sumimi:Sumimi不会解散哦",
    &[On::Play(Some(cant_play), no_breakup)],
);

/// C# `RepeatedDigits` -- does `|money|`'s decimal form repeat a digit?
fn repeated_digits(money: i32) -> bool {
    let mut x = money.unsigned_abs();
    let mut seen: u16 = 0;
    if x == 0 {
        return false;
    }
    while x > 0 {
        let d = (x % 10) as u16;
        if seen & (1 << d) != 0 {
            return true;
        }
        seen |= 1 << d;
        x /= 10;
    }
    false
}

/// C# `CardNoBreakup.Predict` -- money after settling on `t` (rent if other-owned).
/// C# `H.RentOf` pays nothing to an out-of-game or mortgaged owner.
fn predict(player_id: i32, t: i32) -> i32 {
    let money = ctx::money_of(player_id);
    let owner = ctx::tile_owner(t);
    if owner >= 0 && owner != player_id && !ctx::player_out(owner) && !ctx::mortgaged_of(t) {
        money - ctx::rent_of(t)
    } else {
        money
    }
}

/// C# `CardNoBreakup.WhyNot` -- playability gates.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「资金不含有相同数字时可打出」 -- C# refuses with 「资金里有相同的数字」
    // when `RepeatedDigits(H.State.seats[player_id].money)`.
    if repeated_digits(ctx::money_of(player_id)) {
        return Some(Msg::new(key!("no_breakup_repeated")));
    }
    // 规则书: 「当你本回合未进行过抵押/赎回操作」 -- both actions record
    // themselves and clear at the turn end, so the gate is just those records.
    if ctx::state::get(player_id, "mortgaged") != 0 || ctx::state::get(player_id, "redeemed") != 0 {
        return Some(Msg::new(key!("no_breakup_mortgaged")));
    }
    // 规则书: 「视为你的主要移动」 -- the teleport is the turn's main move, so the
    // C# `H.MoveWhyNot` gate applies.
    ctx::cant_move(player_id)
}

fn no_breakup(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    let pos = ctx::player_pos(player_id);
    if n <= 0 {
        return Ok(());
    }
    // 规则书: 「进行一次可使你的资金变动为拥有相同数字的传送」 -- C# prefers the
    // tiles whose predicted post-settle money has a repeated digit, else all tiles
    // but the current one.
    let mut all: Vec<i32> = Vec::new();
    let mut good: Vec<i32> = Vec::new();
    for t in 0..n {
        if t == pos {
            continue;
        }
        all.push(t);
        if repeated_digits(predict(player_id, t)) {
            good.push(t);
        }
    }
    if all.is_empty() {
        return Ok(());
    }
    let picks: &[i32] = if good.is_empty() { &all } else { &good };
    let to = ctx::ask_tile(
        player_id,
        &Msg::new(key!("no_breakup_title")),
        &Msg::new(key!("no_breakup_ask")),
        picks,
    )?;
    // 规则书: 「进行一次…传送，视为你的主要移动」 -- C# `H.CardMove(c, new MoveCtx
    // { TeleportTo = teleportTo })` (`Resolve` defaults to true) =
    // `set_teleport_to(to)` + `set_resolve(true)` + `card_move(player_id)`.
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_resolve(true);
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("no_breakup_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    // 规则书: 「视为你的主要移动」 -- `card_move` runs `MainMoveAs`
    // (MatchHost.cs:23102-23120), which sets `_turnCtx.MainMoved` on the turn
    // player; that is exactly `card_move`'s bookkeeping.
    // 规则书: 「若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果」
    // -- C# snapshots pos / stay / stun / exile / every player's money / owners /
    // houses / mortgaged before the move and restores them all when
    // `!RepeatedDigits(money) && !H.Out(i)` after the settle.
    // TODO(规则书)[judgement]: the revert half -- a world-snapshot / restore (C#
    //   the clause under-specifies -- see the note above it
    // `CardNoBreakup.Play`); the teleport now settles and consumes the main
    // move, but nothing rolls the world back when the money still lacks a
    // repeated digit.
    Ok(())
}
