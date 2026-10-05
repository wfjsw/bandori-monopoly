//! `PPP:STAR BEAT!` -- C# `CardStarBeat` (MatchHost.cs:8693-8734): bank a build
//! layer, then either take 2 star stickers and teleport, or roll a pile of d10s.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:STAR BEAT!`）:
//! > STAR BEAT!：
//! > [手]：[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层”，然后进入移动阶段并选择以下操作之一：
//! > 1. 获得2个星星贴纸，本回合的主要移动改为[传送]到(45×“资金数包含5的玩家数量+1”) mod 60格并结算；
//! > 2. 本回合的主要移动改为移动(2×“资金数包含5的玩家数量+1”)d10格并结算。
//!

use card_sdk::{ctx, key, CardDef, Msg};

// TODO(规则书): C# `CardStarBeat.WhyNot` is just `H.MoveWhyNot(seat)` (main move
//   already used / movement blocked) -- needs `H.MoveWhyNot`; `why_not` stays None.
pub const STAR_BEAT: CardDef = CardDef {
    id: "PPP:STAR BEAT!",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

/// C# `H.MoneyHasDigit(seat, '5')` -- the decimal form of |money| contains 5.
fn money_has_digit5(seat: i32) -> bool {
    let mut x = ctx::money(seat).unsigned_abs();
    if x == 0 {
        return false;
    }
    while x > 0 {
        if x % 10 == 5 {
            return true;
        }
        x /= 10;
    }
    false
}

fn play(seat: i32) {
    // TODO(ABI): 「[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的
    //   格子上进行一次盖房，随后减少1层”」 -- needs the H.ExtraOf attachment
    //   (C# `H.ExtraOf<StarBeatFx>(i).Layers++`) plus the after-settle build offer
    //   (C# `StarBeatFx.SettleAfter` -> `H.OfferBuildAmong` over owned tiles within
    //   absolute distance 5) and `H.OfferBuildAmong` itself.
    // 规则书: 「(45×“资金数包含5的玩家数量+1”) mod 60格」 / 「(2×“资金数包含5的玩家数量+1”)d10」
    let n = (0..ctx::seat_count())
        .filter(|&p| !ctx::seat_out(p) && money_has_digit5(p))
        .count() as i32;
    let tiles = ctx::tile_count();
    if tiles <= 0 {
        return;
    }
    // C# `45 * (num + 1) % H._tiles.Length`.
    let to = (45 * (n + 1)).rem_euclid(tiles);
    let dice = 2 * (n + 1);
    // 规则书: 「然后进入移动阶段并选择以下操作之一」 -- C# `H.AskPick`.
    let pick = ctx::ask_pick(
        seat,
        &Msg::new(key!("star_beat_title")),
        &Msg::new(key!("star_beat_ask")).i("n", n as i64),
        &[
            Msg::new(key!("star_beat_opt_teleport")).tile("tile", to),
            Msg::new(key!("star_beat_opt_move")).i("n", dice as i64),
        ],
    );
    if pick == 0 {
        // 规则书1: 「获得2个星星贴纸」 -- C# `H.AddTok(i, "星星贴纸", 2)`.
        ctx::add_tok(seat, "星星贴纸", 2, i32::MAX);
        ctx::log(
            seat,
            &Msg::new(key!("star_beat_stickers")).seat("who", seat).i("n", 2),
        );
        // 规则书1: 「本回合的主要移动改为[传送]到(45×…) mod 60格并结算」 -- C#
        // `H.CardMove(c, new MoveCtx { TeleportTo = tile })` (teleport *with* settle).
        if to >= 0 {
            ctx::teleport_to(seat, to);
            ctx::log(
                seat,
                &Msg::new(key!("star_beat_moved")).seat("who", seat).tile("tile", to),
            );
        }
        // TODO(ABI): 「并结算」 / 「本回合的主要移动改为[传送]」 -- `ctx::teleport_to`
        //   is `H.ForceTeleport(..., resolve: false)` (no settle, does not consume the
        //   main move). Needs `H.CardMove` / `H.ForceTeleport(..., resolve: true)` so
        //   the landing settles and the turn's main move is this teleport.
    } else {
        // 规则书2: 「本回合的主要移动改为移动(2×…)d10格并结算」 -- C#
        // `H.CardMove(c, new MoveCtx { Base = { (dice, 10, ...) } })`. The dice are
        // rolled for the log; the walk itself still needs `H.CardMove` (TODO below).
        let _sum = ctx::roll(seat, dice, 10);
        // TODO(ABI): 「移动(2×“资金数包含5的玩家数量+1”)d10格并结算」 -- needs
        //   `H.CardMove` with a multi-die base (`MoveCtx.Base.Add((dice, 10, ...))`)
        //   so the walk passes tiles and settles on arrival. The bare `ctx::roll`
        //   above only burns the dice; it does not move the seat.
    }
}