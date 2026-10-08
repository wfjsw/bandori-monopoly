//! `event:泪水的含义` -- 事件卡「泪水的含义」（中立事件 A22）.
//!
//! 事件文本（data/events.json, id `泪水的含义`）:
//! > 所有玩家依次和行动序列里的下一位玩家进行比较：当前玩家和下一位玩家拥有同色地契则下一位玩家[支付]当前玩家700资金，否则当前玩家[支付]下一位玩家300资金。

use alloc::vec::Vec;

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::all_players;

pub const TEARS: CardDef = CardDef::new("event:泪水的含义", &[On::Play("", None, play)]);

/// 「同色」 -- the two players share a colour group on their deeds.
/// TODO(规则书): 「同色地契」 is one colour in common, or the same single deed
/// colour pair? Read here as "at least one colour both own a deed in".
fn share_color(a: i32, b: i32) -> bool {
    let mut colors_a: Vec<i32> = Vec::new();
    for t in ctx::owned_tiles(a) {
        let g = ctx::tile_group(t);
        if g >= 0 {
            colors_a.push(g);
        }
    }
    for t in ctx::owned_tiles(b) {
        let g = ctx::tile_group(t);
        if g >= 0 && colors_a.contains(&g) {
            return true;
        }
    }
    false
}

/// 规则书: 「所有玩家依次和行动序列里的下一位玩家进行比较」 -- each player and
/// the next in turn order, wrapping around the table once.
fn play(player_id: i32) -> card_sdk::Asked {
    let players = all_players();
    if players.len() < 2 {
        return Ok(());
    }
    for (k, &p) in players.iter().enumerate() {
        let q = players[(k + 1) % players.len()];
        if q == p {
            continue;
        }
        // 规则书: 「当前玩家和下一位玩家拥有同色地契则下一位玩家[支付]当前
        // 玩家700资金，否则当前玩家[支付]下一位玩家300资金」
        if share_color(p, q) {
            ctx::transfer(q, p, 700, &Msg::new("log.event.tears_share"))?;
        } else {
            ctx::transfer(p, q, 300, &Msg::new("log.event.tears_pay"))?;
        }
    }
    let _ = player_id;
    Ok(())
}