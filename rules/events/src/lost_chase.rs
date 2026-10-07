//! `event:迷子的追逐` -- 事件卡「迷子的追逐」（中立事件 A26，衍生）.
//!
//! 事件文本（data/events.json, id `迷子的追逐`）:
//! > 所有玩家各投5d20。如果抽到此卡的玩家的点数严格大于所有其他玩家的点数则将一张“[衍生]让我来结束一切”背面朝上放置于事件牌堆顶部，否则将一张“[衍生]一切不会结束”背面朝上放置于事件牌堆顶。

use alloc::vec::Vec;

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const LOST_CHASE: CardDef = CardDef::new("event:迷子的追逐", &[On::Play(None, play)]);

/// 规则书: 「所有玩家各投5d20」 -- one bare 5d20 per player (no [反击] window).
/// 「如果抽到此卡的玩家的点数严格大于所有其他玩家的点数则将一张
/// “[衍生]让我来结束一切”背面朝上放置于事件牌堆顶部，否则将一张
/// “[衍生]一切不会结束”背面朝上放置于事件牌堆顶。」
fn play(player_id: i32) -> card_sdk::Asked {
    let players = all_players();
    let mut faces: Vec<(i32, i32)> = Vec::new();
    for &p in &players {
        let face = roll(p, 5, 20);
        ctx::log(
            player_id,
            &Msg::new("log.event.dice").player_id("who", p).i("n", face as i64),
        );
        faces.push((p, face));
    }
    let mine = faces.iter().find(|&&(p, _)| p == player_id).map(|&(_, f)| f);
    let Some(mine) = mine else {
        // Drawer already out -- treat as the 否则 branch.
        ctx::event_deck_push("一切不会结束", true);
        ctx::log(player_id, &Msg::new("log.event.lost_never").card("event", "event:一切不会结束"));
        return Ok(());
    };
    // 「严格大于所有其他玩家的点数」 -- a tie is not a win.
    let won = faces.iter().all(|&(p, f)| p == player_id || mine > f);
    if won {
        // Coupling with `end_it_all.rs`: 「此卡在场上则[衍生]让我来结束一切不会被
        // [衍生]迷子的追逐放置在事件牌堆顶部」 -- while `让我来结束一切` is
        // active on the field, this win branch must not push it. The 否则 branch
        // is about the dice, so a blocked push places nothing at all.
        if ctx::event_is_active("让我来结束一切") {
            ctx::log(
                player_id,
                &Msg::new("log.event.lost_blocked").card("event", "event:让我来结束一切"),
            );
            return Ok(());
        }
        ctx::event_deck_push("让我来结束一切", true);
        ctx::log(
            player_id,
            &Msg::new("log.event.lost_end").card("event", "event:让我来结束一切"),
        );
    } else {
        ctx::event_deck_push("一切不会结束", true);
        ctx::log(
            player_id,
            &Msg::new("log.event.lost_never").card("event", "event:一切不会结束"),
        );
    }
    Ok(())
}