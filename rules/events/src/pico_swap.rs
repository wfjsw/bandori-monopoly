//! `event:PICO灵魂交换` -- 事件卡「PICO灵魂交换」（中立事件 A7）.
//!
//! 事件文本（data/events.json, id `PICO灵魂交换`）:
//! > 所有未[除外]玩家[传送]到行动顺序的下一位未[除外]玩家的位置（不触发场地效果，例：第一位行动的玩家[传送]到第二位行动的玩家的位置并以此类推，最后行动的玩家[传送]到第一位行动的玩家的位置）

use alloc::vec::Vec;

use card_sdk::abi::state_key;
use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::all_players;

pub const PICO_SWAP: CardDef = CardDef::new("event:PICO灵魂交换", &[On::Play(None, play, "")]);

/// 规则书: 「所有未[除外]玩家[传送]到行动顺序的下一位未[除外]玩家的位置」 --
/// every player whose [除外] layer count is 0, rotated onto the next such
/// player's seat in turn order (「例：第一位…第二位…最后…第一位」). Positions
/// are snapshotted first so the chain does not cascade.
/// 「不触发场地效果」 -- `teleport_to` does not settle.
///
/// The snapshot is latched (`latch:`, the 黑色生日 pattern): `teleport_to` is
/// a host request the body replays past, and a live re-read would see an
/// already-swapped seat -- the last player would jump to the first player's
/// *new* square rather than their original one.
fn play(_player_id: i32) -> card_sdk::Asked {
    let seats: Vec<i32> = all_players()
        .into_iter()
        // 「未[除外]」 -- `state_key::EXILE` layers still up mean off the board.
        .filter(|&p| ctx::state::get(p, state_key::EXILE) <= 0)
        .collect();
    if seats.len() < 2 {
        return Ok(());
    }
    // Each seat's destination, stored as `tile + 1` so 0 can mean "not
    // latched yet" (a latched CiRCLE is tile 0).
    const DEST: &str = "latch:pico.dest";
    for (k, &p) in seats.iter().enumerate() {
        if ctx::slot(p, DEST) == 0 {
            let to = ctx::player_pos(seats[(k + 1) % seats.len()]);
            ctx::set_slot(p, DEST, to + 1);
        }
    }
    for &p in &seats {
        let to = ctx::slot(p, DEST) - 1;
        ctx::teleport_to(p, to);
        ctx::log(
            p,
            &Msg::new("log.event.pico_swap_to")
                .player_id("who", p)
                .tile("tile", to),
        );
    }
    for &p in &seats {
        ctx::set_slot(p, DEST, 0); // clear for a later play
    }
    Ok(())
}