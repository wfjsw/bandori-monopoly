//! `Mor:高贵的微蓝` -- C# `CardNobleBlue` (MatchHost.cs:5079-5107): settle your own
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:高贵的微蓝`）:
//! > 高贵的微蓝：当你位于一块地契价值大于等于2200的地块时，可以打出此卡，如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记，有标记时此地块不能被指定。
//!
//! expensive tile immediately and mark it untargetable.

use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const NOBLE_BLUE: CardDef =
    CardDef::new("Mor:高贵的微蓝", &[On::Play("", Some(cant_play), noble_blue)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"高贵的微蓝"#)
        .raw_name(r#"高贵的微蓝"#)
        .tags(&[])
        .text(CardText::para(r#"当你位于一块地契价值大于等于2200的地块时，可以打出此卡，如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记，有标记时此地块不能被指定。"#))
        // sheet cell G11
        .fields(&[("group", r#"Morfonica"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「当你位于一块地契价值大于等于2200的地块时，可以打出此卡」
    // C# `CardNobleBlue.WhyNot` refuses when `H._tiles[pos].price < 2200`
    // (land price only -- not `buy_price`, which adds houses).
    let pos = ctx::player_pos(player_id);
    if pos < 0 || ctx::tile_price(pos) < 2200 {
        return Some(Msg::new(key!("noble_blue_why_not")));
    }
    None
}

fn noble_blue(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    if pos < 0 {
        return Ok(());
    }
    // 规则书: 「如果此地块属于你」
    if ctx::tile_owner(pos) != player_id {
        // C# `c.Effective = false; H.Log(... 没有效果)`.
        // TODO(规则书)（如果此地块属于你）[judgement]: the not-yours branch is the
        // no-effect case. `PlayCtx.Effective = false` is the C#'s mutable side
        // channel and is not being ported (a routine should *return* whether it took
        // effect); until that lands, what "not effective" changes has to be ruled.
        // The spend looks settled -- haneoka 「放入弃牌堆且视为此卡未生效」 says yes
        // and noble_blue / starry_night's "the card is spent anyway" says yes too
        // (the engine's play-with-no-destination default agrees) -- but what else
        // observes "not effective", and what counts a use it would suppress?
        ctx::log(
            player_id,
            &Msg::new(key!("noble_blue_not_yours")).tile("tile", pos),
        );
        return Ok(());
    }
    // 规则书1: 「立刻进行一次触发结算」 -- C# `H.SettleAt(seat, pos, CardName)`.
    // The player does not move; the tile they are on resolves again.
    // TODO(规则书)（立刻进行一次触发结算）[judgement] OPEN-触发结算: 触发结算 is undefined --
    //   settle-at-tile, and flagging the forced settle `main` though nothing moved,
    //   are our readings.
    ctx::card_settle_at(player_id, pos, true);
    // 规则书2: 「在该地块上放置一个标记，有标记时此地块不能被指定」 -- the mark kind
    // is the gate: `target_tile` refuses any tile carrying `noTarget`.
    // `place_mark_new`: the old `add_mark` row-per-call semantics.
    // TODO(规则书)（有标记时此地块不能被指定）[judgement] OPEN-指定: whether 「指定」 is
    //   exactly `target_tile` (and not every tile pick -- `ask_tile` does not
    //   filter `noTarget`) is undecided.
    ctx::place_mark(
        pos,
        card_sdk::abi::mark::NO_TARGET,
        "",
        player_id,
        ctx::self_uid(),
        1,
        &Msg::new(key!("noble_blue_mark_note")),
        card_sdk::abi::Stack::Fresh,
    );
    ctx::log(
        player_id,
        &Msg::new(key!("noble_blue_placed"))
            .tile("tile", pos)
            .player_id("who", player_id),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn noble_blue_kani(player_id: i32) -> card_sdk::Asked {
    noble_blue(player_id)
}
