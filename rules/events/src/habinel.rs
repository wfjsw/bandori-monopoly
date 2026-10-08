//! `event:前往哈比内尔王国旅游` -- 事件卡「前往哈比内尔王国旅游」（中立事件 A5）.
//!
//! 事件文本（data/events.json, id `前往哈比内尔王国旅游`）:
//! > 每个玩家获得X层[除外]，每名玩家回合开始时移除1层[除外]，结束后传送到微笑号（不触发场地效果）X=1d2（每个玩家独立投掷1d2）

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const HABINEL: CardDef = CardDef::new("event:前往哈比内尔王国旅游", &[On::Play(None, play, "")]);

/// 规则书: 「每个玩家获得X层[除外]…X=1d2（每个玩家独立投掷1d2）」 -- each player
/// rolls their own bare 1d2 and gains that many [除外] layers.
/// 规则书: 「每名玩家回合开始时移除1层[除外]，结束后传送到微笑号（不触发场地效果）」
/// -- the engine's normal exile tick already is that clause: at the owner's
/// turn start `EXILE` loses a layer, and on the last layer the return teleport
/// goes to `exile_to` with no settle (`play.rs` exile tick). No extra body is
/// needed -- `give_exile(p, x, smile)` stores both numbers.
fn play(_player_id: i32) -> card_sdk::Asked {
    let smile = ctx::tile_named("微笑号");
    for p in all_players() {
        let x = roll(p, 1, 2);
        // 规则书: 「结束后传送到微笑号」 -- the return tile is `exile_to`.
        ctx::give_exile(p, x, smile);
        ctx::log(
            p,
            &Msg::new("log.event.habinel_exile")
                .player_id("who", p)
                .i("n", x as i64),
        );
    }
    Ok(())
}