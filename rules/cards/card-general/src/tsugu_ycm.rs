//! `通用:@Tsugu ycm` -- C# `CardTsuguYcm` (MatchHost.cs:1902-1974): roll 3d10
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
//! and draw / discount / teleport to Bandori车站 / buy or gain by the total.

use alloc::vec::Vec;
use bandori_std::ctx;
use bandori_std::ctx::{plan, trigger};
use card_sdk::abi::{roll_source, BuyKind, FilterKind, MoveKind, RollPurpose};
use card_sdk::{key, CardDef, Filter, Msg, On};
use card_sdk::{para, sec, steps, CardText};

pub const TSUGU_YCM: CardDef = CardDef::new(
    "通用:@Tsugu ycm",
    &[
        On::Play("", None, play),
        // （3）「本回合购买格子时[消耗]资金时降低1500（最低0）」 -- a `BuyAdd`
        // stage on the buy price, riding a `ctx::linger` instance for the turn
        // (`docs/PURCHASE.md`). The engine floors the stage at 0. The tile is
        // the hook's applicability (the residual guard).
        On::Filter(Filter::BuyAdd(card_sdk::pre::MINE, Some(buy_add_on_a_tile), buy_add_1500)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"@Tsugu ycm"#)
        .raw_name(r#"@Tsugu ycm"#)
        .tags(&[r#"手"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"投掷3d10并根据结果依次进行以下操作："#),
                steps(&[
                    r#"结果至少为22则抽1张卡；"#,
                    r#"结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；"#,
                    r#"结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；"#,
                    r#"结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并[结算]。"#,
                ]),
            ]),
        ]))
        // sheet cell A2
        .fields(&[("group", r#"通用"#), ("owner", "")]));

/// 「购买格子时」 -- the `BuyAdd` hook's applicability: there is a tile being
/// bought. (`pre::MINE` already states the actor.)
fn buy_add_on_a_tile(_player_id: i32) -> bool {
    trigger::tile() >= 0
}

/// （3）: 「本回合购买格子时[消耗]资金时降低1500（最低0）」 -- `BuyAdd` is the
/// fixed-± stage (`docs/PURCHASE.md`), so the hook subtracts and the engine
/// floors the stage at 0.
fn buy_add_1500(_player_id: i32, value: i32) -> i32 {
    value - 1500
}

/// C# `H.TileNamed("Bandori车站")` -- the rulebook spells it “bandori车站”.
const STATION: &str = "Bandori车站";

/// `ctx::slot` marker: the `ctx::turn_key` of a play that already took its
/// teleport. A replay re-runs the body with the move applied, so the main-move
/// gate must not be re-read then (it would say "already moved") -- LOY-TSUGU-01.
const MOVED: &str = "tsugu_ycm_moved";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「投掷3d10并根据结果依次进行以下操作」 -- C# `H.CardRoll(c, 3, 10, ...)`.
    let r = ctx::roll(player_id, 3, 10, RollPurpose::Effect, roll_source::CARD)?;
    let station = ctx::tile_named(STATION);

    // 规则书（1）[手]: 「结果至少为22则抽1张卡」
    if r >= 22 {
        ctx::draw(player_id, 1)?;
    }

    // 规则书（3）[手]: 「结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）」
    // -- set before the (2) buy below so 「本回合购买」 includes it.
    // TODO(规则书)（投掷3d10并根据结果依次进行以下操作：）: 「依次」 runs (2)'s
    //   optional buy before (3)'s −1500 -- we install the linger first, so the
    //   (2) buy is already discounted (LOY-C pile / OPEN-GEN-tsugu-3-order).
    if r >= 28 {
        // 规则书（3）: 「本回合购买格子时[消耗]资金时降低1500（最低0）」 -- a
        // turn-scoped `BuyAdd` instance (`docs/PURCHASE.md`), the hand-card home
        // that replaces the old `buy_discount` flag.
        ctx::linger(player_id, 0);
        ctx::log(
            player_id,
            &Msg::new(key!("tsugu_discount"))
                .player_id("who", player_id)
                .n("n", 1500),
        );
    }

    // 规则书（2）[手]: 「结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到
    // “bandori车站”并不[结算]」 -- C# `H.CardMove(c, MoveCtx{TeleportTo=station,
    // Resolve=false})`.
    if r >= 26 {
        // 规则书（2）: 「进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”
        // 并不[结算]」 -- one move: it is the turn's main move, it consumes it, and
        // 「不[结算]」 is the landing not resolving.
        // 规则书: the (1) draw and (3) discount do not need the main move, so
        // only this teleport is gated (the play is no longer refused wholesale).
        // TODO(规则书)（[手]：）: the book prints no [限]; the (4) move pick falls
        // back to the gain when the main move is already spent -- (2) here just
        // skips the teleport.
        if station >= 0 {
            // A replay (for the buy prompt below) re-runs this body with the
            // move already applied; asking `move_blocked` then would answer
            // "already moved" and drop it. The marker (or having arrived) says
            // this play already teleported, so the gate is only read on the
            // first run (LOY-TSUGU-01).
            let moved_here = ctx::player_pos(player_id) == station
                || ctx::slot(player_id, MOVED) == ctx::turn_key();
            if moved_here || rules_status::move_blocked(player_id).is_none() {
                ctx::set_slot(player_id, MOVED, ctx::turn_key());
                plan::set_kind(MoveKind::Teleport);
                plan::set_teleport_to(station);
                plan::set_resolve(false);
                ctx::card_move(player_id);
            }
        }
        // 规则书（2）[手]: 「且可选择购买任意无主的[可购买格子]」 -- C#
        // `H._tiles[t].IsBuyable && owners[t] < 0 && money >= BuyPriceFor(i, t)`
        // (`ctx::is_buyable` is `TileData.IsBuyable`, a deed tile). Affordability
        // is the hook-aware quote (`ctx::buy_quotes`, the figure `card_buy`
        // charges), so the −1500 above is already in it and a `BuyGate`-refused
        // tile is dropped by `eligible` (HOST-07).
        let mut unowned: Vec<i32> = Vec::new();
        for t in 0..ctx::tile_count() {
            if ctx::is_buyable(t) && ctx::tile_owner(t) < 0 {
                unowned.push(t);
            }
        }
        let money = ctx::money_of(player_id);
        let mut free: Vec<(i32, i32)> = unowned
            .iter()
            .copied()
            .zip(ctx::buy_quotes(player_id, BuyKind::Card as i32, &unowned))
            .filter_map(|(t, (price, eligible))| {
                (eligible && price >= 0 && price <= money).then_some((t, price))
            })
            .collect();
        if !free.is_empty() {
            // C# `H.AskTileOf(..., allowNone: true)` -- a yes/no stands in for
            // allowNone. C# also defaults the prompt to the most expensive free
            // tile (`free.OrderByDescending(price).First()`): the list carries
            // that order, which is what `ask_tile` falls back to.
            free.sort_by(|a, b| b.1.cmp(&a.1));
            let tiles: Vec<i32> = free.iter().map(|&(t, _)| t).collect();
            let title = Msg::new(key!("tsugu_buy_title"));
            let text = Msg::new(key!("tsugu_buy_ask"));
            if ctx::ask_yes(player_id, &title, &text)? {
                let tile = ctx::ask_tile(player_id, &title, &text, &tiles)?;
                // 规则书（2）: 「购买」 -- C# `H.BuyRoutine(i, rt.index)`.
                // HOST-06: `card_buy` answers whether the deed actually changed
                // hands. A refused buy (`BuyGate` / unpaid) must not log one.
                if ctx::card_buy(player_id, tile) {
                    let price = free
                        .iter()
                        .find(|&&(t, _)| t == tile)
                        .map(|&(_, price)| price)
                        .unwrap_or_else(|| ctx::buy_price(tile));
                    ctx::log(
                        player_id,
                        &Msg::new(key!("tsugu_buy"))
                            .player_id("who", player_id)
                            .tile("tile", tile)
                            .n("price", price as i64),
                    );
                }
            }
        }
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
        )?;
        if pick == 1 {
            // C# `rr.index == 1 && !H._turnCtx.MainMoved` -- the teleport is a
            // main-move option, so it is only taken while `H.MoveWhyNot` still
            // passes (`rules_status::move_blocked(player_id).is_none()`).
            // TODO(规则书)（选择[获得]1000资金或进入移动阶段）: the book's 「或」 has
            //   no fallback -- choosing the move silently pays 1000 instead
            //   when the main move is gone (latent until the whole-card gate goes).
            if station >= 0 && rules_status::move_blocked(player_id).is_none() {
                // 规则书（4）[手]: 「进入移动阶段并将本回合的[主要移动]改为[传送]到
                // “bandori车站”并[结算]」 -- one move: the turn's main move, and it
                // settles on arrival (the default).
                plan::set_kind(MoveKind::Teleport);
                plan::set_teleport_to(station);
                ctx::card_move(player_id);
            } else {
                // 规则书（4）[手]: 「[获得]1000资金」 -- no station tile, or the main
                // move is no longer available (C# falls back to the gain).
                // `ctx::n(1, …)` so 「后勤人员的努力」 (「将打出的那张卡[手]效果中的
                // 一个数字变为两倍」) can double it; slot 1 is this card's only
                // tagged number and exactly one of the two gains fires.
                ctx::gain(player_id, ctx::n(1, 1000), &Msg::new(key!("tsugu_why")))?;
            }
        } else {
            // 规则书（4）[手]: 「[获得]1000资金」 -- the same tagged number.
            ctx::gain(player_id, ctx::n(1, 1000), &Msg::new(key!("tsugu_why")))?;
        }
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn buy_add_1500_kani(player_id: i32, value: i32) -> i32 {
    buy_add_1500(player_id, value)
}
