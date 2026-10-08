//! `PP:[衍生]重叠的声音` -- C# `CardOverlappingVoices` (MatchHost.cs:7290-7331):
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]重叠的声音`）:
//! > [衍生]重叠的声音：
//! > [特]：
//! > 此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上的“初次演出事故”（如果有）。
//! > [手]：
//! > 依次进行以下操作：
//! > 1. [移除]此卡和[使用者][场地]上的“初次演出事故”（如果有）；
//! > 2. 进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”且不[结算]；
//! > 3. 回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”加入手卡。
//!
//! remove itself and 「初次演出事故」, teleport to Bandori车站 without settling.
//! The teleport is the turn's main move (`ctx::plan::set_teleport_to` +
//! `ctx::card_move`); the turn-end body is scheduled with `at_turn_end`.

use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const OVERLAPPING_VOICES: CardDef = CardDef::new(
    "PP:[衍生]重叠的声音",
    &[
        On::Play(Some(cant_play), overlapping_voices, ""),
        On::AtEnd(at_end),
    ],
);

/// C# `CardOverlappingVoices.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[手]2: 「进入移动阶段并将本回合的[主要移动]改为[传送]」 -- the teleport
    // is the turn's main move, so the C# `H.MoveWhyNot` gate applies (own turn,
    // main move still available, turn's move not skipped).
    ctx::cant_move(player_id)
}

fn overlapping_voices(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]1: 「[移除]此卡」 -- C# `c.Dest = "removed"`.
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书[手]1: 「和[使用者][场地]上的“初次演出事故”（如果有）」 -- the other
    // card is named, so it is a named unplace rather than this card's own.
    if let Some(uid) = ctx::find_card(player_id, "PP:初次演出事故") {
        ctx::unplace_at(uid);
    }
    // 规则书[手]3: 「回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”
    // 加入手卡」 -- C# `H._turnCtx.AfterEnd.Add(() => After(i))` (before the move).
    ctx::at_turn_end(player_id);
    // 规则书[手]2: 「进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”
    // 且不[结算]」 -- C# `H.CardMove(c, new MoveCtx { TeleportTo =
    // H.TileNamed("Bandori车站"), Resolve = false })`: a teleport to the tile
    // that consumes the turn's main move and does not settle.
    let to = ctx::tile_named("Bandori车站");
    if to >= 0 {
        ctx::plan::set_teleport_to(to);
        ctx::plan::set_resolve(false);
        ctx::card_move(player_id);
    }
    Ok(())
}

/// C# `CardOverlappingVoices.After` -- the scheduled turn-end body.
fn at_end(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]3: 「获得1层[停留]」
    ctx::give_stay(player_id, 1);
    // 规则书[手]3: 「和1个正面的[P✽P粉丝]」 -- C# `H.GainFans(i, 1, up: true)`.
    ctx::add_tok(player_id, "P✽P粉丝(正)", 1, i32::MAX)?;
    // 规则书[手]3: 「将1张“明天见”加入手卡」
    ctx::add_to_hand(player_id, "PP:[衍生]明天见");
    ctx::log(
        player_id,
        &Msg::new(key!("overlapping_voices_after")).player_id("who", player_id),
    );
    Ok(())
}
// 规则书[特]: 「此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上
// 的“初次演出事故”（如果有）」 -- a named unplace on the discard.
