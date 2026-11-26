//! the card stays in play; the 流星堂 stop is live on the `PassTile` hook.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:仓库里的Random Star`）:
//! > 仓库里的Random Star：
//!
//! > （1）依次进行以下操作：
//! > 1. 将此卡放置在自身场上，并将弃卡区和手牌洗入卡组。
//! > 2. 将1张“拍卖撤下来了”放置在卡组底端。
//!
//! > （2）[经过]“流星堂”时：可使用2个[星星贴纸]，在“流星堂”[强制停下]并[结算]。
//!

use bandori_std::ctx;
use card_sdk::abi::ActionKind;
use card_sdk::{it, it_sub, items, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const RANDOM_STAR: CardDef = CardDef::new(
    "PPP:仓库里的Random Star",
    &[
        On::Play("", None, random_star),
        // 规则书（2）: 「[经过]“流星堂”时可使用2星星贴纸」 -- the owner's
        // still-walking (non-teleport) pass of 流星堂, with two stickers in hand.
        On::Action(Action::PassTile("card.placed && actor == owner && tile.id == tile_named('流星堂') && move.remaining > 0 && move.kind != Teleport && tok('星星贴纸') >= 2", None, pass_tile)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"仓库里的Random Star"#)
        .raw_name(r#"仓库里的Random Star"#)
        .tags(&[])
        .text(CardText::items(&[it_sub(r#"依次进行以下操作："#, &[r#"将此卡放置在自身场上，并将弃卡区和手牌洗入卡组。"#, r#"将1张“拍卖撤下来了”放置在卡组底端。"#]), it(r#"[经过]“流星堂”时：可使用2个[星星贴纸]，在“流星堂”[强制停下]并[结算]。"#)]))
        // sheet cell B12
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]));

fn random_star(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「将此卡放置自身场上」
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "PPP:仓库里的Random Star",
        &Msg::new(key!("random_star_note")),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("random_star_placed")).player_id("who", player_id),
    );
    ctx::sweep_to_deck(player_id);
    ctx::add_to_dest(
        player_id,
        "PPP:[衍生]拍卖撤下来了",
        ctx::Dest::DeckBottom,
        false,
    );
    ctx::log(
        player_id,
        &Msg::new(key!("random_star_swept")).player_id("who", player_id),
    );
    Ok(())
}

fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `card.placed && actor == owner && tile.id == tile_named('流星堂') &&
    // move.remaining > 0 && move.kind != Teleport && tok('星星贴纸') >= 2` is
    // the condition (the ask itself is the effect).
    let ryuseido = ctx::tile_named("流星堂");
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("random_star_stop_title")),
        &Msg::new(key!("random_star_stop_ask")),
    )? {
        return Ok(());
    }
    if !ctx::gate(player_id, bandori_std::kind::abnormal::Stop) {
        return Ok(());
    }
    // `set_stop_at` is the forced stop from this `PassTile` hook: the walk settles
    // at the stop tile (`ctx::plan::stop_at()` is the read-only check).
    ctx::plan::set_stop_at(ryuseido);
    ctx::plan::set_settles(true);
    ctx::add_held_by_name(player_id, "星星贴纸", -2, i32::MAX)?;
    ctx::log(
        player_id,
        &Msg::new(key!("random_star_stop"))
            .player_id("who", player_id)
            .tile("tile", ryuseido),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn random_star_kani(player_id: i32) -> card_sdk::Asked {
    random_star(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}
