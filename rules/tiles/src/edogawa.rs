//! `tile:edogawa` -- 江户川乐器店's [结算].
//!
//! 规则书（data/rules.txt, 「基础[结算]规则」, line 95）:
//! > · CiRCLE和江户川乐器店的[结算]是：抽取一张手卡。
//!
//! One draw, nothing else. CiRCLE's extra 「[经过]」 reward is a different
//! clause (line 96) and lives on `tile:circle`.
//!
//! TODO(规则书): the book names 江户川乐器店 and CiRCLE in one sentence but
//! gives them different boards' worth of extra text (the [经过] reward). If
//! the two shops ever diverge, this body is the one to split further.

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

pub const EDOGAWA: CardDef = CardDef::new("tile:edogawa", &[On::Settle(settle)]);

/// 规则书: 「江户川乐器店的[结算]是：抽取一张手卡。」
fn settle(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「[结算]是：抽取一张手卡」 -- `ctx::draw` is the engine's `H.DrawR`;
    // the reshuffle-when-empty half is the engine's (rulebook: 「当抽卡区抽光时
    // 将弃卡区洗卡并放回抽卡区」), not this body's.
    let at = ctx::self_tile().unwrap_or(-1);
    ctx::log(
        player_id,
        &Msg::new("log.land_draw")
            .player_id("who", player_id)
            .tile("tile", at),
    );
    ctx::draw(player_id, 1)?;
    Ok(())
}