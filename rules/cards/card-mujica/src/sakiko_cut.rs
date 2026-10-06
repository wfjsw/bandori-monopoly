//! `Mujica:（祥子）斩断留恋，忘却一切` -- C# `CardSakikoCut` (MatchHost.cs:5744-5814):
//! mortgage your best deed, discard a hand card, teleport with settle as main move.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（祥子）斩断留恋，忘却一切`）:
//! > （祥子）斩断留恋，忘却一切：
//! >  抵押一张你拥有且未抵押的最贵地契并弃置一张手牌（若无手牌则弃掉下一张抽到的牌），立刻传送至任意可购买或已拥有的格子并触发结算，作为你的主要移动。若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡
//!
//!

use card_sdk::ctx::{self, CardPile};
use card_sdk::{key, CardDef, On, Msg};


pub const SAKIKO_CUT: CardDef = CardDef::new("Mujica:（祥子）斩断留恋，忘却一切", &[
    On::Play(Some(cant_play), sakiko_cut)]);

/// C# `H.Mortgageable(seat)`: owned ∧ `IsBuyable` ∧ `kind != "ring"` ∧ not
/// mortgaged.
fn mortgageable(player_id: i32) -> alloc::vec::Vec<i32> {
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::is_buyable(t) && !ctx::is_ring(t) && !ctx::mortgaged_of(t))
        .collect()
}

/// C# `CardSakikoCut.WhyNot`: needs a mortgageable deed, then `H.MoveWhyNot`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「抵押一张你拥有且未抵押的最贵地契」 -- C# refuses with
    // 「没有没抵押的地契」 when `H.Mortgageable(seat)` is empty.
    if mortgageable(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_mortgageable")));
    }
    // 规则书: 「作为你的主要移动」 -- the teleport is the main move, so the C#
    // `H.MoveWhyNot` gate applies.
    ctx::cant_move(player_id)
}

fn sakiko_cut(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡」
    // C# `c.FromDeck` gates the three-way pick. No FromDeck flag in the
    // vocabulary, so the base case (always discard) is taken.
    // TODO(规则书)[judgement]: 「若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡」 -- needs
    //   the clause under-specifies -- see the note above it
    // the `FromDeck` play-source flag (C# `PlayCtx.FromDeck`) so the 3-way prompt
    // (skip the discard / discard as usual / don't fire) can appear; without it
    // the discard below always runs.
    let skip_discard = false;
    // 规则书: 「抵押一张你拥有且未抵押的最贵地契」 -- C# `H.Mortgageable(i)`
    // ordered by `_tiles[num].price` descending, then `H.MortgageRoutine`.
    let mine = mortgageable(player_id);
    if mine.is_empty() || ctx::cant_move(player_id).is_some() {
        // C# `c.Effective = false` when the list is empty or `H.MoveWhyNot(i)`
        // refuses.
        return Ok(());
    }
    // 规则书: 「最贵地契」 -- C# `list.OrderByDescending((int num) =>
    // H._tiles[num].price).First()`.
    let best = mine
        .into_iter()
        .reduce(|a, b| if ctx::tile_price(b) > ctx::tile_price(a) { b } else { a });
    if let Some(t) = best {
        // 规则书: 「抵押」 -- C# `H.MortgageRoutine(i, t, ...)`. The engine refuses
        // (and logs its own reason) when the deed cannot be mortgaged.
        ctx::card_mortgage(player_id, t);
    }
    // 规则书: 「并弃置一张手牌（若无手牌则弃掉下一张抽到的牌）」
    if !skip_discard {
        // 规则书: 「若无手牌则弃掉下一张抽到的牌」 -- C# branches on
        // `H._hidden[i].hand.Count == 0`; `ctx::hand_size(player_id)` is that count.
        if ctx::hand_size(player_id) > 0 {
            // 规则书: 「弃置一张手牌」 -- C# `H.AskCard` over `H._hidden[i].hand`
            // then `H.DiscardFromHand`.
            let hand = ctx::cards_in(player_id, CardPile::Hand);
            let pool: alloc::vec::Vec<&str> = hand.iter().map(|s| s.as_str()).collect();
            if !pool.is_empty() {
                let pick = ctx::ask_card(
                    player_id,
                    &Msg::new(key!("sakiko_cut_title")),
                    &Msg::new(key!("sakiko_cut_discard")),
                    &pool,
                )?;
                ctx::discard_from_hand(player_id, pool[pick]);
            }
        } else {
            // TODO(规则书): 「若无手牌则弃掉下一张抽到的牌」 -- C#
            // `H.ExtraOf<DiscardNextFx>` attaches a one-shot `Drew` hook that
            // discards the first card of the next draw batch (MatchHost.cs
            // `DiscardNextFx.Drew`). The `Drew` hook kind is in the ABI (v23),
            // but the temporary `H.ExtraOf` attachment that would outlive this
            // play is still held, so no hook can be declared here.
            ctx::log(player_id, &Msg::new(key!("sakiko_cut_no_hand")).player_id("who", player_id));
        }
    }
    // 规则书: 「立刻传送至任意可购买或已拥有的格子并触发结算」 -- the destination
    // pool is buyable or self-owned tiles (C# `IsBuyable && (owners < 0 ||
    // owners == i)`).
    let mut pool = alloc::vec::Vec::new();
    for t in 0..ctx::tile_count() {
        if ctx::is_buyable(t) && (ctx::tile_owner(t) < 0 || ctx::tile_owner(t) == player_id) {
            pool.push(t);
        }
    }
    if pool.is_empty() {
        return Ok(());
    }
    let to = ctx::ask_tile(
        player_id,
        &Msg::new(key!("sakiko_cut_title")),
        &Msg::new(key!("sakiko_cut_ask")),
        &pool,
    )?;
    // 规则书: 「立刻传送至任意可购买或已拥有的格子并触发结算」 -- C# `H.CardMove`
    // with `TeleportTo = to` (`MoveCtx { TeleportTo = to }`, `Resolve` defaults
    // to true, MatchHost.cs:5807-5810) = `set_teleport_to(to)` +
    // `set_resolve(true)` + `card_move(player_id)`. `set_teleport_to` also
    // flips the kind to `MoveKind::Teleport`.
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_resolve(true);
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("sakiko_cut_moved")).player_id("who", player_id).tile("tile", to),
    );
    // 规则书: 「并触发结算」 -- the settle half is `set_resolve(true)` above
    // (the teleport runs `settleBefore` -> `settle` -> `land` -> `settleAfter`
    // on the destination).
    // 规则书: 「作为你的主要移动」 -- `card_move` runs `MainMoveAs`
    // (MatchHost.cs:23102-23120), which sets `_turnCtx.MainMoved` on the turn
    // player; that is exactly `card_move`'s bookkeeping.
    Ok(())
}
