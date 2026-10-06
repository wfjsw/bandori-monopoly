//! `AG:Y.O.L.O` -- C# `CardYolo` (MatchHost.cs:956-987).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:Y.O.L.O`）:
//! > Y.O.L.O： 
//! >  你的掷骰结算前打出此卡，使结果增加1d4结果的数字
//!
//! Counteraction: before *your* (move) roll settles, add 1d4 to the result.

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const YOLO: CardDef = CardDef::new(
    "AG:Y.O.L.O",
    &[On::Counteract(
        &[ChainKind::Roll, ChainKind::MoveRoll],
        can_counteract,
        counteract,
    )],
);

fn can_counteract(player_id: i32) -> bool {
    // 规则书: 「**你的**掷骰结算前」 -- only the card user's own roll, not anyone else's.
    matches!(trigger::kind(), TriggerKind::Roll | TriggerKind::MoveRoll)
        && trigger::player_id() == player_id
        && trigger::move_roll().is_some()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `ctx::roll` honours a forced extreme (「以理论最大值或最小值结算」).
    let Some(before) = trigger::move_roll() else {
        return Ok(());
    };
    let n = ctx::roll(player_id, 1, 4);
    trigger::set_move_roll(before + n);
    ctx::log(
        player_id,
        &Msg::new(key!("yolo_boost"))
            .player_id("who", trigger::player_id())
            .i("n", n as i64)
            .i("total", (before + n) as i64),
    );
    Ok(())
}
