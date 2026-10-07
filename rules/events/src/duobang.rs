//! `event:对邦` -- 事件卡「对邦」（中立事件 A2）.
//!
//! 事件文本（data/events.json, id `对邦`）:
//! > 所有人立刻进行一次1d20骰子拼点，所有点数最高者从所有点数最低者获得胜出点数x50的钱（例：最高点数18，最低点数16，则需支付 (18-16)x50=100）

use alloc::vec::Vec;

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const DUI_BANG: CardDef = CardDef::new("event:对邦", &[On::Play(None, play)]);

/// 规则书: 「所有人立刻进行一次1d20骰子拼点」 -- one 1d20 per player, no
/// [反击] window (the text says 立刻, not 掷骰结算前).
fn play(player_id: i32) -> card_sdk::Asked {
    let players = all_players();
    if players.len() < 2 {
        return Ok(());
    }
    let mut faces: Vec<(i32, i32)> = Vec::new();
    for &p in &players {
        let face = roll(p, 1, 20);
        faces.push((p, face));
        ctx::log(
            player_id,
            &Msg::new("log.event.dice").player_id("who", p).i("n", face as i64),
        );
    }
    let high = faces.iter().map(|(_, f)| *f).max().unwrap_or(0);
    let low = faces.iter().map(|(_, f)| *f).min().unwrap_or(0);
    // 规则书: 「所有点数最高者从所有点数最低者获得胜出点数x50的钱（例：最高
    // 点数18，最低点数16，则需支付 (18-16)x50=100）」 -- each lowest pays each
    // highest the gap x 50. A tie (high == low) pays nothing.
    let gap = high - low;
    if gap <= 0 {
        return Ok(());
    }
    let each = gap * 50;
    for &(p, f) in &faces {
        if f != low {
            continue;
        }
        for &(q, g) in &faces {
            if g != high || q == p {
                continue;
            }
            ctx::transfer(p, q, each, &Msg::new("log.event.duobang_pay"))?;
        }
    }
    Ok(())
}