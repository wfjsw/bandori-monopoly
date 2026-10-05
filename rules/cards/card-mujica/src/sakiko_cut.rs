//! `Mujica:（祥子）斩断留恋，忘却一切` -- C# `CardSakikoCut` (MatchHost.cs:5744-5814):
//! mortgage your best deed, discard a hand card, teleport with settle as main move.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（祥子）斩断留恋，忘却一切`）:
//! > （祥子）斩断留恋，忘却一切：
//! >  抵押一张你拥有且未抵押的最贵地契并弃置一张手牌（若无手牌则弃掉下一张抽到的牌），立刻传送至任意可购买或已拥有的格子并触发结算，作为你的主要移动。若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡
//!
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const SAKIKO_CUT: CardDef = CardDef {
    id: "Mujica:（祥子）斩断留恋，忘却一切",
    play: Some(sakiko_cut),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `H.Mortgageable(seat)`: owned ∧ `IsBuyable` ∧ `kind != "ring"` ∧ not
/// mortgaged.
fn mortgageable(seat: i32) -> alloc::vec::Vec<i32> {
    // TODO(ABI): the C# also excludes RiNG deeds (`_tiles[t].kind != "ring"`);
    // there is no tile-kind query, so a RiNG-only owner still counts as
    // mortgageable here (permissive: never refuses what the C# allows).
    ctx::owned_tiles(seat)
        .into_iter()
        .filter(|&t| ctx::is_buyable(t) && !ctx::mortgaged_of(t))
        .collect()
}

/// C# `CardSakikoCut.WhyNot`: needs a mortgageable deed, then `H.MoveWhyNot`.
fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「抵押一张你拥有且未抵押的最贵地契」 -- C# refuses with
    // 「没有没抵押的地契」 when `H.Mortgageable(seat)` is empty.
    if mortgageable(seat).is_empty() {
        return Some(Msg::new(key!("x_no_mortgageable")));
    }
    // 规则书: 「作为你的主要移动」 -- the teleport is the main move, so the C#
    // `H.MoveWhyNot` gate applies: own turn first (`只能在自己的回合`).
    if ctx::turn_seat() != seat {
        return Some(Msg::new(key!("x_not_your_turn")));
    }
    // TODO(规则书): the rest of C# `H.MoveWhyNot` -- refuses after this turn's
    // main move (`_turnCtx.MainMoved` -> 「这回合已经移动过了」) and when the
    // turn's move is skipped (`State.skipMove` -> 「本回合不能移动」); needs
    // `H.MoveWhyNot` in the ABI.
    None // playable
}

fn sakiko_cut(seat: i32) {
    // 规则书: 「若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡」
    // C# `c.FromDeck` gates the three-way pick. No FromDeck flag in the
    // vocabulary, so the skip-discard branch is always taken (the safer read).
    let skip_discard = true;
    // TODO(规则书): 「若直接从抽牌堆打出」 -- needs the `FromDeck` play-source
    // flag (C# `PlayCtx.FromDeck`) so the 3-way prompt can appear.
    // 规则书: 「抵押一张你拥有且未抵押的最贵地契」 -- C# `H.Mortgageable(i)`
    // ordered by `_tiles[num].price` descending, then `H.MortgageRoutine`.
    let mine = mortgageable(seat);
    if mine.is_empty() || ctx::turn_seat() != seat {
        // C# `c.Effective = false` when the list is empty or `H.MoveWhyNot(i)`
        // refuses (the rest of `H.MoveWhyNot` is still TODO'd in `why_not`).
        return;
    }
    // 规则书: 「最贵地契」 -- C# `list.OrderByDescending((int num) =>
    // H._tiles[num].price).First()`.
    let best = mine
        .into_iter()
        .reduce(|a, b| if ctx::tile_price(b) > ctx::tile_price(a) { b } else { a });
    if let Some(t) = best {
        // TODO(规则书): 「抵押」 -- C# `H.MortgageRoutine(i, t, ...)`. The query
        // half (`H.Mortgageable` / `_tiles[t].price` / `mortgaged`) is real now;
        // only the mortgage routine is missing.
        let _ = t;
    }
    // 规则书: 「并弃置一张手牌（若无手牌则弃掉下一张抽到的牌）」
    if !skip_discard {
        // 规则书: 「弃置一张手牌」 -- C# `H.AskCard` over `H._hidden[i].hand`
        // then `H.DiscardFromHand`.
        // TODO(规则书): 「弃置一张手牌」 -- `ctx::discard_from_hand` is available,
        // but there is still no hand-list query (C# `H.HandOf` /
        // `_hidden[i].hand`) to build the `ask_card` pool from.
        // TODO(规则书): 「若无手牌则弃掉下一张抽到的牌」 -- C# `H.ExtraOf<DiscardNextFx>`
        // attaches a one-shot `Drew` hook; needs the Fx.Drew / ExtraOf machinery.
    }
    // 规则书: 「立刻传送至任意可购买或已拥有的格子并触发结算」 -- the destination
    // pool is buyable or self-owned tiles (C# `IsBuyable && (owners < 0 ||
    // owners == i)`).
    let mut pool = alloc::vec::Vec::new();
    for t in 0..ctx::tile_count() {
        if ctx::is_buyable(t) && (ctx::tile_owner(t) < 0 || ctx::tile_owner(t) == seat) {
            pool.push(t);
        }
    }
    if pool.is_empty() {
        return;
    }
    let to = ctx::ask_tile(
        seat,
        &Msg::new(key!("sakiko_cut_title")),
        &Msg::new(key!("sakiko_cut_ask")),
        &pool,
    );
    // TODO(ABI): 「并触发结算，作为你的主要移动」 -- C# `H.CardMove` with
    // `TeleportTo = to` (settle on arrival, counts as the main move). The
    // vocabulary has `teleport_to` (no settle) and no main-move bookkeeping.
    // The teleport below jumps without settling; the settle is TODO'd.
    ctx::teleport_to(seat, to);
    ctx::log(
        seat,
        &Msg::new(key!("sakiko_cut_moved")).seat("who", seat).tile("tile", to),
    );
    // TODO(规则书): 「作为你的主要移动」 -- needs `H.MoveWhyNot` / `_turnCtx.MainMoved`.
}