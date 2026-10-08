//! `event:EX任务挑战` -- 事件卡「EX任务挑战」（中立事件 A10）.
//!
//! 事件文本（data/events.json, id `EX任务挑战`）:
//! > 抽出此卡的玩家投掷1d4并记录结果和26相加为X，然后所有玩家各自投掷3d20，结果至少为X的玩家可选择更换角色皮肤（技能相同）

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const EX_QUEST: CardDef = CardDef::new("event:EX任务挑战", &[On::Play(None, play, "")]);

/// 规则书: 「抽出此卡的玩家投掷1d4并记录结果和26相加为X」 -- X = 26 + 1d4
/// (27..=30), rolled by the drawer as a bare `ctx::roll` (no [反击] window).
fn play(player_id: i32) -> card_sdk::Asked {
    let x = 26 + roll(player_id, 1, 4);
    ctx::log(
        player_id,
        &Msg::new("log.event.ex_quest_x")
            .player_id("who", player_id)
            .i("n", x as i64),
    );

    // 规则书: 「然后所有玩家各自投掷3d20，结果至少为X的玩家可选择更换角色
    // 皮肤（技能相同）」 -- each player rolls 3d20; a face >= X is eligible.
    for &p in &all_players() {
        let face = roll(p, 3, 20);
        ctx::log(
            player_id,
            &Msg::new("log.event.dice").player_id("who", p).i("n", face as i64),
        );
        if face < x {
            continue;
        }
        // 「可选择更换角色皮肤（技能相同）」 -- a cosmetic swap only.
        // TODO(规则书)/TODO(engine): 「更换角色皮肤」 has no mechanical effect
        // (「技能相同」) and no ctx surface (no skin / cosmetic API), so the body
        // only logs who became eligible. A skin-change ask is the missing surface.
        ctx::log(
            player_id,
            &Msg::new("log.event.ex_quest_skin").player_id("who", p).i("n", face as i64),
        );
    }
    Ok(())
}