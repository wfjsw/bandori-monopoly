//! `PP:[衍生]重叠的声音` -- C# `CardOverlappingVoices` (MatchHost.cs:7290-7331):
//! remove itself and 「初次演出事故」, teleport to Bandori车站 without settling.
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
//! The teleport is approximated with `teleport_to` (see the TODO on the main
//! move); the turn-end body is scheduled with `at_turn_end`.

use card_sdk::ctx;
use card_sdk::{key, CardDef, On, Msg};

pub const OVERLAPPING_VOICES: CardDef = CardDef::new("PP:[衍生]重叠的声音", &[
    On::Play(overlapping_voices),
    On::CantPlay(cant_play),
    On::AtEnd(at_end),
]);

/// C# `CardOverlappingVoices.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[手]2: 「进入移动阶段并将本回合的[主要移动]改为[传送]」 -- the teleport
    // is the turn's main move, so the C# `H.MoveWhyNot` gate applies (own turn,
    // main move still available, turn's move not skipped).
    ctx::cant_move(player_id)
}

fn overlapping_voices(player_id: i32) {
    // 规则书[手]1: 「[移除]此卡」 -- C# `c.Dest = "removed"`.
    ctx::set_dest(ctx::Dest::Banished);
    // TODO(规则书)[手]1: 「和[使用者][场地]上的“初次演出事故”（如果有）」 -- needs
    // unplace-another-field-card (C# `H.Unplace(card, "removed", "重叠的声音")` on
    // the placed `CardFirstLiveAccident`); `unplace_card` only removes this card.
    // 规则书[手]2: 「[传送]到“bandori车站”且不[结算]」 -- C#
    // `H.TileNamed("Bandori车站")` + `H.ForceTeleport(..., resolve: false)`.
    let to = ctx::tile_named("Bandori车站");
    if to >= 0 {
        ctx::teleport_to(player_id, to);
    }
    // TODO(规则书)[手]2: 「进入移动阶段并将本回合的[主要移动]改为」 -- HELD:
    // `ctx::card_move` (run the move now as the main move, C# `H.CardMove(c, new
    // MoveCtx { TeleportTo = ..., Resolve = false })`) is not landed yet. Until
    // then the player still gets its normal main move after the teleport.
    // 规则书[手]3: 「回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”
    // 加入手卡」 -- C# `H._turnCtx.AfterEnd.Add(() => After(i))`.
    ctx::at_turn_end(player_id);
}

/// C# `CardOverlappingVoices.After` -- the scheduled turn-end body.
fn at_end(player_id: i32) {
    // 规则书[手]3: 「获得1层[停留]」
    ctx::give_stay(player_id, 1);
    // 规则书[手]3: 「和1个正面的[P✽P粉丝]」 -- C# `H.GainFans(i, 1, up: true)`.
    ctx::add_tok(player_id, "P✽P粉丝(正)", 1, i32::MAX);
    // 规则书[手]3: 「将1张“明天见”加入手卡」
    ctx::add_to_hand(player_id, "PP:[衍生]明天见");
    ctx::log(player_id, &Msg::new(key!("overlapping_voices_after")).player_id("who", player_id));
}
// TODO(规则书): [特]「此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上
// 的“初次演出事故”（如果有）」 -- the `Discarded` hook kind is landed (C#
// `Card.OnDiscarded(int)`); the body still needs unplace-another-field-card
// (C# `H.Unplace(card, "removed", ...)` on the placed 「初次演出事故」),
// which `unplace_card` cannot express (it only removes *this* card).