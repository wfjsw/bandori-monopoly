//! `event:弦卷集团地产开发` -- 事件卡「弦卷集团地产开发」（中立事件 A3）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `弦卷集团地产开发`）:
//! > 随机指定一个可购买格子（如果有）然后所有玩家进行竞价，出价最大的玩家支付选择数量的资金并获得该格子的地契

use alloc::vec::Vec;

use bandori_std::ctx;
use card_sdk::CardText;
use card_sdk::{CardDef, Msg, On};

use crate::util::roll;

pub const TSURUMAKI_ESTATE: CardDef =
    CardDef::new("event:弦卷集团地产开发", &[On::Play("", None, play)]).rule()
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"弦卷集团地产开发"#)
            .text(CardText::para(r#"随机指定一个可购买格子（如果有）然后所有玩家进行竞价，出价最大的玩家支付选择数量的资金并获得该格子的地契"#))
            // sheet cell A3
            .derived(false));

/// 规则书: 「随机指定一个可购买格子（如果有）」 -- one unowned buyable tile,
/// chosen uniformly by a bare 1dN. 「如果有」 is the empty-pool no-op below.
fn play(player_id: i32) -> card_sdk::Asked {
    let mut open: Vec<i32> = Vec::new();
    for t in 0..ctx::tile_count() {
        if ctx::is_buyable(t) && ctx::tile_owner(t) < 0 {
            open.push(t);
        }
    }
    if open.is_empty() {
        ctx::log(player_id, &Msg::new("log.event.estate_none"));
        return Ok(());
    }
    // 1..=len from the match RNG, mapped down to a 0-based index.
    let face = roll(player_id, 1, open.len() as i32)?;
    let tile = open[(face - 1).clamp(0, open.len() as i32 - 1) as usize];
    ctx::log(
        player_id,
        &Msg::new("log.event.estate_tile").tile("tile", tile),
    );

    // 规则书: 「然后所有玩家进行竞价，出价最大的玩家支付选择数量的资金并获得
    // 该格子的地契」 -- the engine's ordinary auction (ABI-09, `ctx::auction`):
    // every present payer bids in turn, each raise at least +100 over the
    // standing bid, the last (highest) bidder pays exactly that bid and takes
    // the deed. Mincing raises mean a tie cannot occur. The auction's own log
    // lines carry the tile, each bid and the winning amount.
    let winner = ctx::auction(player_id, tile)?;
    if winner < 0 {
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new("log.event.estate_win")
            .player_id("who", winner)
            .tile("tile", tile),
    );
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
