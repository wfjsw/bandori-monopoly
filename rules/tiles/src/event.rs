//! `tile:event` -- CiRCLE咖啡厅 and 流星堂's [结算].
//!
//! 规则书（data/rules.txt, 「基础[结算]规则」, lines 97–99）:
//! > · CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡。
//! > 　· 抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。
//! > 　· 事件结算后进入事件弃卡区。如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。
//!
//! Both board tiles share one rule -- the book gives them one sentence.

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

pub const EVENT: CardDef = CardDef::new("tile:event", &[On::Settle("", None, settle)]);

/// 规则书: 「[结算]是：抽取一张手卡，然后抽取一个事件卡。」
fn settle(player_id: i32) -> card_sdk::Asked {
    let at = ctx::self_tile().unwrap_or(-1);
    // 规则书: 「抽取一张手卡」 -- `H.DrawR`; the reshuffle-when-empty half is
    // the engine's (line 74 of the glossary: 「当抽卡区抽光时将弃卡区洗卡并放回抽卡区」).
    ctx::log(
        player_id,
        &Msg::new("log.land_event")
            .player_id("who", player_id)
            .tile("tile", at),
    );
    ctx::draw(player_id, 1)?;
    // 规则书: 「然后抽取一个事件卡」 with 「不进入手卡并向所有玩家公开，效果立刻
    // 生效」 and 「事件结算后进入事件弃卡区。如果没有事件可抽取则将事件弃卡区洗切
    // 并当作新的事件卡堆来抽取」 -- all three are what `H.DrawEvent` does
    // (`ctx::draw_event`); none of them is this body's to reimplement.
    ctx::draw_event(player_id)?;
    Ok(())
}