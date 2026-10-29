//! `Mujica:（祥子）斩断留恋，忘却一切` -- C# `CardSakikoCut` (MatchHost.cs:5744-5814):
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（祥子）斩断留恋，忘却一切`）:
//! > （祥子）斩断留恋，忘却一切：
//! >  抵押一张你拥有且未抵押的最贵地契并弃置一张手牌（若无手牌则弃掉下一张抽到的牌），立刻传送至任意可购买或已拥有的格子并触发结算，作为你的主要移动。若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡
//!
//! mortgage your best deed, discard a hand card, teleport with settle as main move.

use bandori_std::ctx::{self, CardPile};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const SAKIKO_CUT: CardDef = CardDef::new(
    "Mujica:（祥子）斩断留恋，忘却一切",
    &[On::Play("", Some(cant_play), sakiko_cut)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"斩断留恋，忘却一切"#)
        .raw_name(r#"（祥子）斩断留恋，忘却一切"#)
        .tags(&[])
        .text(CardText::para(r#"抵押一张你拥有且未抵押的最贵地契并弃置一张手牌（若无手牌则弃掉下一张抽到的牌），立刻传送至任意可购买或已拥有的格子并触发结算，作为你的主要移动。若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡"#))
        // sheet cell J9
        .fields(&[("group", r#"Ave Mujica"#), ("owner", r#"丰川祥子"#)]));

/// C# `H.Mortgageable(seat)`: owned ∧ `IsBuyable` ∧ no `NO_MORTGAGE` prop ∧
/// not mortgaged.
fn mortgageable(player_id: i32) -> alloc::vec::Vec<i32> {
    // 规则书: 「抵押一张你拥有且未抵押的最贵地契」 -- owned ∧ buyable, a deed
    // that can be mortgaged (the `NO_MORTGAGE` prop is what excludes RiNG, the
    // engine's own test) ∧ not already mortgaged.
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| {
            ctx::is_buyable(t)
                && ctx::tile_prop(t, card_sdk::abi::prop::NO_MORTGAGE) == 0
                && !ctx::mortgaged_of(t)
        })
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
    rules_status::move_blocked(player_id)
}

fn sakiko_cut(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡」
    // `ctx::play_from_hand()` names the play's origin (`false` when the card came
    // off the draw pile, e.g. the 状态2 auto-play). C# `c.FromDeck` gates a
    // three-way pick -- skip the discard / discard as usual / don't fire; the
    // pick is not built, so a deck play always discards (when it fires).
    // TODO(规则书)（若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡）: 书 gives a
    //   deck play the skip-discard and the explicit decline -- we always discard when it fires
    let skip_discard = false;
    // 规则书: 「抵押一张你拥有且未抵押的最贵地契」 -- C# `H.Mortgageable(i)`
    // ordered by `_tiles[num].price` descending, then `H.MortgageRoutine`.
    //
    // Replay note: `card_mortgage` is a host routine, so the body re-runs from
    // the top with the call's answer re-read from the log. The re-run has to
    // issue exactly one mortgage request again, or the answer stays in the log
    // and the next prompt (`ask_card` / `ask_tile`) reads it as its own pick.
    // With one deed the list is empty on the re-run, so it falls back to the
    // most expensive *owned* deed -- the one just mortgaged. The replayed call
    // is answered from the log (the engine would refuse that tile), so it is a
    // no-op that consumes the slot and keeps the prompts aligned.
    let mine = mortgageable(player_id);
    // 规则书: 「最贵地契」 -- C# `list.OrderByDescending((int num) =>
    // H._tiles[num].price).First()`.
    let best = mine.into_iter().reduce(|a, b| {
        if ctx::tile_price(b) > ctx::tile_price(a) {
            b
        } else {
            a
        }
    });
    // Re-derivable stand-in for the replayed pass (see the note above).
    let stand_in = ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::is_buyable(t) && ctx::tile_prop(t, card_sdk::abi::prop::NO_MORTGAGE) == 0)
        .reduce(|a, b| {
            if ctx::tile_price(b) > ctx::tile_price(a) {
                b
            } else {
                a
            }
        });
    let paid = match best.or(stand_in) {
        // 规则书: 「抵押」 -- C# `H.MortgageRoutine(i, t, ...)`. The engine refuses
        // (and logs its own reason) when the deed cannot be mortgaged; a replayed
        // call instead returns the logged answer.
        Some(t) => ctx::card_mortgage(player_id, t),
        None => false,
    };
    if !paid {
        // 规则书: 「抵押一张你拥有且未抵押的最贵地契」 -- the mortgage is a
        // mandatory cost, but only the hand-press gate (`cant_play`) checks it;
        // `ctx::play_card` (the draw-pile auto-play) runs this entry without the
        // gate. Re-check here: an unpayable play declines (「或选择不发动此卡」)
        // and the card is spent with no effect.
        return Ok(());
    }
    // 规则书: 「并弃置一张手牌（若无手牌则弃掉下一张抽到的牌）」
    //
    // The pick is taken now, but the discard itself is deferred past the
    // teleport's host call: a host routine re-runs this body from the top
    // against a fresh copy, so anything written to the copy before it is
    // dropped. Asking first (a prompt replays the same way) and discarding
    // after the last host call is what makes the discard stick.
    let mut discard_pick: Option<usize> = None;
    let mut discard_names: alloc::vec::Vec<card_sdk::ctx::NameId> = alloc::vec::Vec::new();
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
                discard_pick = Some(pick);
                discard_names = hand;
            }
        } else {
            // TODO(规则书)（若无手牌则弃掉下一张抽到的牌）: 书 discards the next card
            //   drawn -- it is only logged here. The route exists: `place_self_to_field`
            //   keeps this instance in play and a one-shot `On::Action(Action::Drew(..))`
            //   discards that drawn card and retires the instance (`dream_ahead` pattern);
            //   this branch would place instead of letting the card go to the discard.
            ctx::log(
                player_id,
                &Msg::new(key!("sakiko_cut_no_hand")).player_id("who", player_id),
            );
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
        if let Some(i) = discard_pick {
            if let Some(card) = discard_names.get(i) {
                ctx::discard_from_hand(player_id, card.as_str());
            }
        }
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
        &Msg::new(key!("sakiko_cut_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    // 规则书: 「弃置一张手牌」 -- deferred past `card_move` so it commits with
    // this run (see the note above).
    // TODO(规则书)（并弃置一张手牌（若无手牌则弃掉下一张抽到的牌），立刻传送至任意可购买或已拥有的格子并触发结算）:
    //   书 discards before the teleport -- we discard after `card_move`, so the card is
    //   still in hand through the destination settle, and the pick is re-applied by
    //   index to a hand that settle may have changed (`discard_from_hand`'s bool is
    //   dropped, so a vanished copy silently no-ops)
    if let Some(i) = discard_pick {
        if let Some(card) = discard_names.get(i) {
            ctx::discard_from_hand(player_id, card.as_str());
        }
    }
    // 规则书: 「并触发结算」 -- the settle half is `set_resolve(true)` above
    // (the teleport runs `settleBefore` -> `settle` -> `land` -> `settleAfter`
    // on the destination).
    // 规则书: 「作为你的主要移动」 -- `card_move` runs `MainMoveAs`
    // (MatchHost.cs:23102-23120), which sets `_turnCtx.MainMoved` on the turn
    // player; that is exactly `card_move`'s bookkeeping.
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn sakiko_cut_kani(player_id: i32) -> card_sdk::Asked {
    sakiko_cut(player_id)
}
