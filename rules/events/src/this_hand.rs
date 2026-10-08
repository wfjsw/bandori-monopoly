//! `event:这只手我不会放开` -- 事件卡「这只手我不会放开」（中立事件 A6）.
//!
//! 事件文本（data/events.json, id `这只手我不会放开`）:
//! > 所有玩家移动X，X=1d20-1d20+1d20，如果X是负数则向自身移动方向的反方向移动（每个玩家独立投掷三个1d20进行计算，不触发场地效果）

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const THIS_HAND: CardDef = CardDef::new("event:这只手我不会放开", &[On::Play(None, play, "")]);

/// 规则书: 「所有玩家移动X，X=1d20-1d20+1d20，如果X是负数则向自身移动方向的反
/// 方向移动（每个玩家独立投掷三个1d20进行计算，不触发场地效果）」 -- three bare
/// 1d20 per player, `X = a - b + c`; a negative X walks backwards. `card_move`
/// shaped by `plan::set_steps` / `set_reverse` / `set_resolve(false)` is the
/// forced no-settle walk (`fate_together.rs` pattern).
fn play(_player_id: i32) -> card_sdk::Asked {
    for p in all_players() {
        let a = roll(p, 1, 20);
        let b = roll(p, 1, 20);
        let c = roll(p, 1, 20);
        let x = a - b + c;
        ctx::log(
            p,
            &Msg::new("log.event.this_hand_roll")
                .player_id("who", p)
                .i("a", a as i64)
                .i("b", b as i64)
                .i("c", c as i64)
                .i("x", x as i64),
        );
        // 「如果X是负数则向自身移动方向的反方向移动」 -- `set_reverse` for a
        // negative X, walking |X| steps the other way around the ring.
        if x < 0 {
            ctx::plan::set_steps(-x);
            ctx::plan::set_reverse(true);
        } else {
            ctx::plan::set_steps(x);
        }
        // 「不触发场地效果」 -- the landing does not settle.
        ctx::plan::set_resolve(false);
        ctx::card_move(p);
    }
    Ok(())
}