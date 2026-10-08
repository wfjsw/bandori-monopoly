//! `event:让我来结束一切` -- 事件卡「让我来结束一切」（中立事件 A27，衍生，常驻）.
//!
//! 事件文本（data/events.json, id `让我来结束一切`）:
//! > 触发事件的玩家从所有非衍生事件中选择3个移除，并将此卡永久放置于场上，此卡在场上则[衍生]让我来结束一切不会被[衍生]迷子的追逐放置在事件牌堆顶部

use alloc::vec::Vec;

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::keep;

/// Every non-derived event id, in `data/events.json` order.
/// TODO(规则书)/TODO(engine): the non-derived list lives in `data/events.json`
/// (`derived: false`) and is not reachable from `ctx`, so the ids are
/// duplicated here. Keep the table in step with the data file.
const NON_DERIVED: &[&str] = &[
    "对邦",
    "弦卷集团地产开发",
    "很噜的感觉",
    "前往哈比内尔王国旅游",
    "这只手我不会放开",
    "PICO灵魂交换",
    "协助CiRCLE重建",
    "前场队还是后场队？",
    "EX任务挑战",
    "发送熊饼表情",
    "飞鸟山之战",
    "幻觉来了",
    "卡池BUG",
    "A！A！O！",
    "麻里奈小姐的礼物箱",
    "元祖！邦多利酱",
    "超燃甩头",
    "意外的对邦",
    "Forbidden Moca",
    "上学时间",
    "泪水的含义",
    "Kizuna Music",
    "火种燃尽之后会怎么样呢？",
];

pub const END_IT_ALL: CardDef = CardDef::new("event:让我来结束一切", &[On::Play(None, play, "")]);

/// 规则书: 「触发事件的玩家从所有非衍生事件中选择3个移除」 -- the drawer picks
/// three distinct non-derived event ids and each is banished for good.
/// 「并将此卡永久放置于场上」 -- the event stays up forever (no expiry clause),
/// so it is `keep()` and nothing ever calls `expire`.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::log(
        player_id,
        &Msg::new("log.event.end_it_on").player_id("who", player_id),
    );

    let mut pool: Vec<&str> = NON_DERIVED.to_vec();
    for _ in 0..3 {
        if pool.is_empty() {
            break;
        }
        let options: Vec<Msg> = pool
            .iter()
            .map(|id| Msg::new("ask.cardOption").card("event", id))
            .collect();
        // 「选择3个移除」 -- without replacement; a picked id leaves the pool.
        let pick = ctx::ask_pick(
            player_id,
            &Msg::new("log.event.end_it_title"),
            &Msg::new("log.event.end_it_ask"),
            &options,
        )?;
        let id = pool.remove(pick.min(pool.len() - 1));
        // 「移除」 -- out of the game entirely (`docs/EVENTS.md`).
        ctx::event_banish(id);
        ctx::log(
            player_id,
            &Msg::new("log.event.end_it_banish")
                .player_id("who", player_id)
                .card("event", id),
        );
    }

    // 「此卡在场上则[衍生]让我来结束一切不会被[衍生]迷子的追逐放置在事件牌堆
    // 顶部」 -- the guard lives on `lost_chase.rs`'s win branch: it checks
    // `ctx::event_is_active("让我来结束一切")` before pushing this card. While
    // this event sits on the field (it never expires on its own), `迷子的追逐`
    // cannot stack a second copy on the event deck. Coupled files: this one
    // keeps, `lost_chase.rs` guards.
    Ok(())
}