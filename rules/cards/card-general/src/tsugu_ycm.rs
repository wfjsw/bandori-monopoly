//! `通用:@Tsugu ycm` -- C# `CardTsuguYcm` (MatchHost.cs:1902-1974): roll 3d10
//! and draw / discount / teleport to Bandori车站 / buy or gain by the total.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:@Tsugu ycm`）:
//! > @Tsugu ycm：
//! > [手]：
//! > 投掷3d10并根据结果依次进行以下操作：
//! > 1. 结果至少为22则抽1张卡；
//! > 2. 结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；
//! > 3. 结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；
//! > 4. 结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并[结算]。
//!

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, On, Msg};

pub const TSUGU_YCM: CardDef = CardDef::new("通用:@Tsugu ycm", &[
    On::Play(play),
    On::CantPlay(cant_play),
]);

/// C# `H.TileNamed("Bandori车站")` -- the rulebook spells it “bandori车站”.
const STATION: &str = "Bandori车站";

/// C# `CardTsuguYcm.WhyNot` defers to `H.MoveWhyNot`'s main-move check: the
/// card is only playable while this turn's main move is still available.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「将本回合的[主要移动]改为[传送]」 -- the card replaces the main
    // move, so the C# `H.MoveWhyNot` gate applies (own turn, main move still
    // available, turn's move not skipped).
    ctx::cant_move(player_id)
}

fn play(player_id: i32) {
    // 规则书[手]: 「投掷3d10并根据结果依次进行以下操作」 -- C# `H.CardRoll(c, 3, 10, ...)`.
    let r = ctx::roll(player_id, 3, 10);
    let station = ctx::tile_named(STATION);

    // 规则书（1）[手]: 「结果至少为22则抽1张卡」
    if r >= 22 {
        ctx::draw(player_id, 1);
    }

    // 规则书（2）[手]: 「结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到
    // “bandori车站”并不[结算]」 -- C# `H.CardMove(c, MoveCtx{TeleportTo=station,
    // Resolve=false})`.
    if r >= 26 {
        if station >= 0 {
            ctx::teleport_to(player_id, station);
        }
        // TODO(规则书)（2）: 「进进入移动阶段并将本回合的[主要移动]改为[传送]…并不[结算]」
        // -- needs the H.CardMove / main-move routine (C# `H.CardMove(c, new MoveCtx
        // { TeleportTo = station, Resolve = false })`) so the teleport consumes the
        // turn's main move and skips settle. Until then the player is only moved and
        // still gets its normal main move afterwards.
        // 规则书（2）[手]: 「且可选择购买任意无主的[可购买格子]」 -- C#
        // `H._tiles[t].IsBuyable && owners[t] < 0 && money >= BuyPriceFor(i, t)`
        // (`ctx::is_buyable` is `TileData.IsBuyable`, a deed tile).
        let mut free: Vec<i32> = Vec::new();
        for t in 0..ctx::tile_count() {
            if ctx::is_buyable(t) && ctx::tile_owner(t) < 0 && ctx::money(player_id) >= ctx::buy_price(t) {
                free.push(t);
            }
        }
        if !free.is_empty() {
            // C# `H.AskTileOf(..., allowNone: true)` -- a yes/no stands in for allowNone.
            let title = Msg::new(key!("tsugu_buy_title"));
            let text = Msg::new(key!("tsugu_buy_ask"));
            if ctx::ask_yes(player_id, &title, &text) {
                let tile = ctx::ask_tile(player_id, &title, &text, &free);
                // TODO(规则书)（2）: 「购买」 -- needs the H.BuyRoutine purchase routine
                // (C# `H.BuyRoutine(i, rt.index)`); until then the tile is only named.
                // C# also defaults the prompt to the most expensive free tile
                // (`free.OrderByDescending(price).First()`).
                ctx::log(
                    player_id,
                    &Msg::new(key!("tsugu_buy"))
                        .player_id("who", player_id)
                        .tile("tile", tile)
                        .n("price", ctx::buy_price(tile) as i64),
                );
            }
        }
    }

    // 规则书（3）[手]: 「结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）」
    if r >= 28 {
        // TODO(规则书)（3）: the turn's buy discount (C# `H._turnCtx.BuyDiscount =
        // c.N(0, 1500)` logged as 「本回合买地少花 1,500」) -- needs a turn-scoped
        // buy-discount hook; until then purchases cost full price.
    }

    // 规则书（4）[手]: 「结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]
    // 改为[传送]到“bandori车站”并[结算]」
    if r < 26 {
        let pick = ctx::ask_pick(
            player_id,
            &Msg::new(key!("tsugu_pick_title")),
            &Msg::new(key!("tsugu_pick")).i("roll", r as i64),
            &[
                Msg::new(key!("tsugu_pick_gain")).n("n", 1000),
                Msg::new(key!("tsugu_pick_move")),
            ],
        );
        if pick == 1 {
            // C# `rr.index == 1 && !H._turnCtx.MainMoved` -- the teleport is a
            // main-move option, so it is only taken while `H.MoveWhyNot` still
            // passes (`ctx::cant_move(player_id).is_none()`).
            if station >= 0 && ctx::cant_move(player_id).is_none() {
                // 规则书（4）[手]: 「进入移动阶段并将本回合的[主要移动]改为[传送]到
                // “bandori车站”并[结算]」 -- C# `H.CardMove(c, MoveCtx{TeleportTo=station})`.
                ctx::teleport_to(player_id, station);
                // TODO(规则书)（4）: 「进入移动阶段并将本回合的[主要移动]改为…并[结算]」
                // -- needs the H.CardMove / main-move setter (C# `H.CardMove(c,
                // new MoveCtx { TeleportTo = station })`) so the teleport consumes
                // the turn's main move and settles on arrival.
            } else {
                // 规则书（4）[手]: 「[获得]1000资金」 -- no station tile, or the main
                // move is no longer available (C# falls back to the gain).
                ctx::gain(player_id, 1000, &Msg::new(key!("tsugu_why")));
            }
        } else {
            // 规则书（4）[手]: 「[获得]1000资金」
            ctx::gain(player_id, 1000, &Msg::new(key!("tsugu_why")));
        }
    }
}