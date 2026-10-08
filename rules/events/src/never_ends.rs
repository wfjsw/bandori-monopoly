//! `event:一切不会结束` -- 事件卡「一切不会结束」（中立事件 A28，衍生）.
//!
//! 事件文本（data/events.json, id `一切不会结束`）:
//! > 抽出此卡的玩家投掷1d4，所有玩家[传送]到对应数字的RiNG且获得1层在回合开始时移除的[除外]。

use alloc::format;

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const NEVER_ENDS: CardDef = CardDef::new("event:一切不会结束", &[On::Play("", None, play)]);

/// 规则书: 「抽出此卡的玩家投掷1d4，所有玩家[传送]到对应数字的RiNG」 -- the
/// drawer rolls 1d4 and everyone teleports to RiNG 1..4 (board names in
/// `data/board.json`) with no settle. 「且获得1层在回合开始时移除的[除外]」 --
/// one exile layer that the engine's turn-start tick wears off.
fn play(player_id: i32) -> card_sdk::Asked {
    let n = roll(player_id, 1, 4);
    ctx::log(
        player_id,
        &Msg::new("log.event.dice").player_id("who", player_id).i("n", n as i64),
    );
    // 「对应数字的RiNG」 -- `data/board.json` names them "RiNG 1".."RiNG 4".
    let tile = ctx::tile_named(&format!("RiNG {n}"));
    if tile < 0 {
        // Board without the four RiNGs -- nothing to teleport to.
        return Ok(());
    }
    for &p in &all_players() {
        // 「[传送]」 -- a position write, not a move; no landing settle.
        ctx::teleport_to(p, tile);
        // 「获得1层在回合开始时移除的[除外]」 -- `give_exile(p, 1, -1)` is the
        // plain layer the engine ticks off at the player's next turn start
        // (`misaki_other`'s 「在你的下回合开始前移除的[除外]」 is the same shape).
        ctx::give_exile(p, 1, -1);
        ctx::log(
            player_id,
            &Msg::new("log.event.never_tp")
                .player_id("who", p)
                .tile("tile", tile),
        );
    }
    Ok(())
}