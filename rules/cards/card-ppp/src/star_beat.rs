//! `PPP:STAR BEAT!` -- C# `CardStarBeat` (MatchHost.cs:8693-8734): bank a build
//! layer, then either take 2 star stickers and teleport, or roll a pile of d10s.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:STAR BEAT!`）:
//! > STAR BEAT!：
//! > [手]：[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层”，然后进入移动阶段并选择以下操作之一：
//! > 1. 获得2个星星贴纸，本回合的主要移动改为[传送]到(45×“资金数包含5的玩家数量+1”) mod 60格并结算；
//! > 2. 本回合的主要移动改为移动(2×“资金数包含5的玩家数量+1”)d10格并结算。
//!

use card_sdk::abi::MoveKind;
use card_sdk::{ctx, key, CardDef, On, Msg};

pub const STAR_BEAT: CardDef = CardDef::new("PPP:STAR BEAT!", &[
    On::Play(Some(cant_play), play)]);

/// C# `CardStarBeat.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    ctx::cant_move(player_id)
}

/// C# `H.MoneyHasDigit(seat, '5')` -- the decimal form of |money| contains 5.
fn money_has_digit5(player_id: i32) -> bool {
    let mut x = ctx::money(player_id).unsigned_abs();
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

fn play(player_id: i32) {
    // TODO(ABI): 「[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的
    //   格子上进行一次盖房，随后减少1层”」 -- needs the H.ExtraOf attachment
    //   (C# `H.ExtraOf<StarBeatFx>(i).Layers++`) plus the after-settle build offer
    //   (C# `StarBeatFx.SettleAfter` -> `H.OfferBuildAmong` over owned tiles within
    //   absolute distance 5) and `H.OfferBuildAmong` itself.
    // 规则书: 「(45×“资金数包含5的玩家数量+1”) mod 60格」 / 「(2×“资金数包含5的玩家数量+1”)d10」
    let n = (0..ctx::player_count())
        .filter(|&p| !ctx::player_out(p) && money_has_digit5(p))
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
        player_id,
        &Msg::new(key!("star_beat_title")),
        &Msg::new(key!("star_beat_ask")).i("n", n as i64),
        &[
            Msg::new(key!("star_beat_opt_teleport")).tile("tile", to),
            Msg::new(key!("star_beat_opt_move")).i("n", dice as i64)],
    );
    if pick == 0 {
        // 规则书1: 「获得2个星星贴纸」 -- C# `H.AddTok(i, "星星贴纸", 2)`.
        ctx::add_tok(player_id, "星星贴纸", 2, i32::MAX);
        ctx::log(
            player_id,
            &Msg::new(key!("star_beat_stickers")).player_id("who", player_id).i("n", 2),
        );
        // 规则书1: 「本回合的主要移动改为[传送]到(45×…) mod 60格并结算」 -- C#
        // `H.CardMove(c, new MoveCtx { TeleportTo = tile })` (MatchHost.cs:8717;
        // teleport *with* settle -- `MoveCtx.Resolve` defaults to true). The plan
        // names the destination and settles, and `card_move` runs it as the main
        // move.
        if to >= 0 {
            ctx::plan::set_kind(MoveKind::Teleport);
            ctx::plan::set_teleport_to(to);
            ctx::plan::set_resolve(true);
            ctx::log(
                player_id,
                &Msg::new(key!("star_beat_moved")).player_id("who", player_id).tile("tile", to),
            );
            ctx::card_move(player_id);
        }
    } else {
        // 规则书2: 「本回合的主要移动改为移动(2×…)d10格并结算」 -- C#
        // `H.CardMove(c, new MoveCtx { Base.Clear(); Base.Add((dice, 10, ...)) })`
        // (MatchHost.cs:8723-8730): the base dice table becomes `dice`d10
        // (`set_base_dice` replaces the default 1d20) and the walk runs and
        // settles (`MoveCtx.Resolve` defaults to true). `card_move` rolls the
        // dice and walks; no separate `ctx::roll`.
        ctx::plan::set_base_dice(dice, 10, "（STAR BEAT!）");
        ctx::plan::set_resolve(true);
        ctx::card_move(player_id);
    }
}