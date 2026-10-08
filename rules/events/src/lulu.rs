//! `event:很噜的感觉` -- 事件卡「很噜的感觉」（中立事件 A4）.
//!
//! 事件文本（data/events.json, id `很噜的感觉`）:
//! > 所有玩家可选择[传送]至快餐店（不触发场地效果），如果选择[传送]需支付500资金并喊出“噜噜噜！”（只要玩家嗓子允许，必须喊出“噜噜噜！”）

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::all_players;

pub const LULU: CardDef = CardDef::new("event:很噜的感觉", &[On::Play("", None, play)]);

/// 规则书: 「所有玩家可选择[传送]至快餐店（不触发场地效果）」 -- each player in
/// turn order is asked; `teleport_to` jumps with no settle (「不触发场地效果」).
fn play(_player_id: i32) -> card_sdk::Asked {
    let shop = ctx::tile_named("快餐店");
    for p in all_players() {
        // 规则书: 「可选择[传送]」 -- the teleport is optional.
        let yes = ctx::ask_yes(
            p,
            &Msg::new("log.event.lulu_ask"),
            &Msg::new("log.event.lulu_ask_text"),
        )?;
        if !yes {
            continue;
        }
        // 规则书: 「如果选择[传送]需支付500资金」 -- the jump costs 500.
        // TODO(规则书): a player who chose [传送] but holds under 500 -- does the
        // payment prompt / force debt, or does the choice fizzle? Reading as
        // "cannot pay -> no teleport" (no debt taken).
        if ctx::money_of(p) < 500 {
            continue;
        }
        ctx::pay(p, 500, &Msg::new("log.event.lulu_pay"))?;
        // 规则书: 「并喊出“噜噜噜！”（只要玩家嗓子允许，必须喊出“噜噜噜！”）」
        // -- out-of-game flavour; log the shout, no mechanic.
        // TODO(规则书): the shout is a real-world obligation the engine cannot
        // check; logged only.
        ctx::log(p, &Msg::new("log.event.lulu_shout").player_id("who", p));
        // 「不触发场地效果」 -- `teleport_to` already does not settle.
        ctx::teleport_to(p, shop);
    }
    Ok(())
}