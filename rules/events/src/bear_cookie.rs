//! `event:发送熊饼表情` -- 事件卡「发送熊饼表情」（中立事件 A11）.
//!
//! 事件文本（data/events.json, id `发送熊饼表情`）:
//! > 所有技能中含有火罐的玩家可选择补充X个火罐（不能超过上限），并支付X次1000资金。随后，将一张“[衍生]冲榜”背面朝上放置于事件牌堆顶部。

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::all_players;

pub const BEAR_COOKIE: CardDef = CardDef::new("event:发送熊饼表情", &[On::Play(None, play)]);

/// 「技能中含有火罐」 -- the player's character or band skill text mentions 「火罐」.
fn skill_mentions_fire_pot(p: i32) -> bool {
    if let Some(skill) = ctx::character_skill(p) {
        if ctx::card_text_mentions(&skill, "火罐") {
            return true;
        }
    }
    if let Some(skill) = ctx::band_skill(p) {
        if ctx::card_text_mentions(&skill, "火罐") {
            return true;
        }
    }
    false
}

/// 规则书: 「所有技能中含有火罐的玩家可选择补充X个火罐（不能超过上限），并
/// 支付X次1000资金」 -- each such player picks X (0..=room under their own cap
/// and what they can pay for at 1000 a pot), gains that many pots, pays X*1000.
fn play(player_id: i32) -> card_sdk::Asked {
    for &p in &all_players() {
        if !skill_mentions_fire_pot(p) {
            continue;
        }
        // 「不能超过上限」 -- `gain_fire` already caps at the player's own max.
        let room = (ctx::fire_max(p) - ctx::fire(p)).max(0);
        let afford = (ctx::money_of(p) / 1000).max(0);
        let cap = room.min(afford);
        if cap <= 0 {
            continue;
        }
        // 「可选择补充X个」 -- X is the player's own choice, including 0 (pass).
        let x = ctx::ask_number(
            p,
            &Msg::new("log.event.bear_cookie_title"),
            &Msg::new("log.event.bear_cookie_ask").i("n", cap as i64),
            0,
            cap,
        )?;
        if x <= 0 {
            continue;
        }
        // 「补充X个火罐」
        ctx::gain_fire(p, x, &Msg::new("log.event.bear_cookie_fill"));
        // 「并支付X次1000资金」 -- X lots of 1000, i.e. X*1000.
        ctx::pay(p, x * 1000, &Msg::new("log.event.bear_cookie_pay"))?;
        ctx::log(
            player_id,
            &Msg::new("log.event.bear_cookie_done")
                .player_id("who", p)
                .i("n", x as i64),
        );
    }
    // 规则书: 「随后，将一张“[衍生]冲榜”背面朝上放置于事件牌堆顶部。」
    ctx::event_deck_push("冲榜", true);
    ctx::log(player_id, &Msg::new("log.event.bear_cookie_push").card("event", "event:冲榜"));
    Ok(())
}