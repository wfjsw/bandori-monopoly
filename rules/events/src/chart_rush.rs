//! `event:冲榜` -- 事件卡「冲榜」（中立事件 A24，衍生）.
//!
//! 事件文本（data/events.json, id `冲榜`）:
//! > 所有人失去所有[火罐]并且投掷Xd20，X为失去的指示物数量加一，点数最高的人获得2000资金或抽一张卡，点数第二的人获得1500资金，点数第三的人获得1000资金，点数第四的人获得500资金（如果两个人点数相同则同时获得较高的 那一档，例：获得奖励名次为1，2，2，4）

use alloc::vec::Vec;

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const CHART_RUSH: CardDef = CardDef::new("event:冲榜", &[On::Play(None, play, "")]);

/// 规则书: 「所有人失去所有[火罐]并且投掷Xd20，X为失去的指示物数量加一」 --
/// every player loses every fire pot and rolls (lost + 1) d20. One-shot: the
/// engine already files derived events into `event_removed`, so no `keep()`.
fn play(player_id: i32) -> card_sdk::Asked {
    let players = all_players();
    let mut faces: Vec<(i32, i32)> = Vec::new();
    for &p in &players {
        // 「所有人失去所有[火罐]」
        let lost = ctx::fire(p);
        if lost > 0 {
            ctx::spend_fire(p, lost, &Msg::new("log.event.chart_lose"))?;
        }
        // 「并且投掷Xd20，X为失去的指示物数量加一」
        let face = roll(p, lost + 1, 20);
        ctx::log(
            player_id,
            &Msg::new("log.event.dice").player_id("who", p).i("n", face as i64),
        );
        faces.push((p, face));
    }

    // 规则书: 「（如果两个人点数相同则同时获得较高的 那一档，例：获得奖励名次
    // 为1，2，2，4）」 -- competition ranking: a player's rank is 1 + how many
    // faces are strictly higher. Ties share the higher tier and the next rank
    // skips (1,2,2,4 -- nobody sits on 3).
    for &(p, f) in &faces {
        let rank = 1 + faces.iter().filter(|&&(_, g)| g > f).count() as i32;
        match rank {
            // 「点数最高的人获得2000资金或抽一张卡」
            1 => {
                let money = ctx::ask_yes(
                    p,
                    &Msg::new("log.event.chart_prize_title").i("n", rank as i64),
                    &Msg::new("log.event.chart_prize_ask"),
                )?;
                if money {
                    ctx::gain(p, 2000, &Msg::new("log.event.chart_prize"))?;
                } else {
                    ctx::draw(p, 1)?;
                    ctx::log(
                        player_id,
                        &Msg::new("log.event.chart_prize_card").player_id("who", p),
                    );
                }
            }
            // 「点数第二的人获得1500资金」
            2 => {
                ctx::gain(p, 1500, &Msg::new("log.event.chart_prize"))?;
            }
            // 「点数第三的人获得1000资金」
            3 => {
                ctx::gain(p, 1000, &Msg::new("log.event.chart_prize"))?;
            }
            // 「点数第四的人获得500资金」 -- beyond 4th gets nothing.
            4 => {
                ctx::gain(p, 500, &Msg::new("log.event.chart_prize"))?;
            }
            _ => {}
        }
    }
    Ok(())
}