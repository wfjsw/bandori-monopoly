//! `event:前场队还是后场队？` -- 事件卡「前场队还是后场队？」（中立事件 A9）.
//!
//! 事件文本（data/events.json, id `前场队还是后场队？`）:
//! > 所有玩家各投掷1d20，然后将站在格子序号1到30的玩家的投掷结果相加定位X且站在格子序号31到60的玩家的投掷结果相加定位Y；如果X=Y则所有玩家获得500资金，如果X>Y则站在格子序号1到30的玩家分摊获得1000资金，如果Y>X则站在格子序号31到60的玩家分摊获得1000资金

use alloc::vec::Vec;

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const FRONT_OR_BACK: CardDef = CardDef::new("event:前场队还是后场队？", &[On::Play("", None, play)]);

/// 规则书: 「所有玩家各投掷1d20，然后将站在格子序号1到30的玩家的投掷结果相加
/// 定位X且站在格子序号31到60的玩家的投掷结果相加定位Y」 -- one bare 1d20 each.
/// Board tiles are 0-based in the engine (sheet 格子序号 1..=30 -> 0..=29,
/// 31..=60 -> 30..=59).
fn play(_player_id: i32) -> card_sdk::Asked {
    let players = all_players();
    let mut x = 0;
    let mut y = 0;
    let mut front: Vec<i32> = Vec::new();
    let mut back: Vec<i32> = Vec::new();
    for &p in &players {
        let face = roll(p, 1, 20);
        ctx::log(
            p,
            &Msg::new("log.event.dice").player_id("who", p).i("n", face as i64),
        );
        let t = ctx::player_pos(p);
        if (0..30).contains(&t) {
            x += face;
            front.push(p);
        } else if (30..60).contains(&t) {
            y += face;
            back.push(p);
        }
    }
    // 规则书: 「如果X=Y则所有玩家获得500资金」
    if x == y {
        for &p in &players {
            ctx::gain(p, 500, &Msg::new("log.event.front_or_back_tie"))?;
        }
        return Ok(());
    }
    // 规则书: 「如果X>Y则站在格子序号1到30的玩家分摊获得1000资金」 /
    // 「如果Y>X则站在格子序号31到60的玩家分摊获得1000资金」
    // TODO(规则书): 「分摊」 -- even integer split of the 1000; the remainder
    // (`1000 % n`) is dropped, the text does not say who gets it.
    let winners = if x > y { front } else { back };
    if winners.is_empty() {
        return Ok(());
    }
    // `PIPELINE-AUDIT` Q2: the command-wide pre-split stage shapes the 1000
    // before it divides (the 「分摊前」 figure). TODO(规则书): 「分摊」 -- even
    // integer split of the remainder (`1000 % n`) is dropped, the text does not
    // say who gets it.
    let why = Msg::new("log.event.front_or_back_win");
    let Some(shaped) = ctx::pay_total(-1, winners[0], 1000, &why)? else {
        return Ok(());
    };
    let each = shaped / winners.len() as i32;
    for &p in &winners {
        ctx::pay_leg(-1, p, each, &why.clone().i("n", each as i64))?;
    }
    Ok(())
}