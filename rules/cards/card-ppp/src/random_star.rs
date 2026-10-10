//! `PPP:仓库里的Random Star` -- C# `CardRandomStar` (MatchHost.cs:8919-8970):
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:仓库里的Random Star`）:
//! > 仓库里的Random Star：
//!
//! > （1）将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”放置在卡组底端
//!
//! > （2）[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]
//!
//! the card stays in play; the 流星堂 stop is live on the `PassTile` hook.

use card_sdk::abi::HookKind;
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const RANDOM_STAR: CardDef = CardDef::new(
    "PPP:仓库里的Random Star",
    &[
        On::Play("", None, random_star),
        // 规则书（2）: 「[经过]“流星堂”时可使用2星星贴纸」 -- the owner's
        // still-walking (non-teleport) pass of 流星堂, with two stickers in hand.
        On::Hook(
            &[HookKind::PassTile],
            "card.placed && actor == owner && tile.id == tile_named('流星堂') && move.remaining > 0 && move.kind != Teleport && tok('星星贴纸') >= 2",
            None,
            pass_tile,
        ),
    ],
);

fn random_star(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「将此卡放置自身场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "PPP:仓库里的Random Star",
        &Msg::new(key!("random_star_note")),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("random_star_placed")).player_id("who", player_id),
    );
    // 规则书（1）: 「并将弃牌堆和手牌洗入卡组」 -- C# `H.ShuffleAllIntoDeck(seat, hand:
    // true, discard: true)`.
    ctx::sweep_to_deck(player_id);
    // 规则书（1）: 「然后将一张“拍卖撤下来了”放置在卡组底端」 -- C#
    // `H.AddToDeck(seat, "PPP:[衍生]拍卖撤下来了", "bottom")`.
    ctx::add_to_deck_at(player_id, "PPP:[衍生]拍卖撤下来了", ctx::DeckPos::Bottom);
    ctx::log(
        player_id,
        &Msg::new(key!("random_star_swept")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书（2）: 「[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]」
/// -- C# `CardRandomStar.PassTile` -> `Stop`.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `card.placed && actor == owner && tile.id == tile_named('流星堂') &&
    // move.remaining > 0 && move.kind != Teleport && tok('星星贴纸') >= 2` is
    // the pre (the ask itself is the effect).
    let ryuseido = ctx::tile_named("流星堂");
    // 规则书（2）: 「可使用2星星贴纸」 -- C# `H.AskYes(..., "经过流星堂：要用 2 个
    // 星星贴纸在这里 [强制停下] 并结算吗？")`.
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("random_star_stop_title")),
        &Msg::new(key!("random_star_stop_ask")),
    )? {
        return Ok(());
    }
    // 规则书（2）: 「在“流星堂”强制停下并[结算]」 -- C# `m.Stopped = true; m.Resolve = true`
    // (MatchHost.cs:8955-8963, behind `H.AbnormalGate`). `set_stop_at` is the
    // forced stop from this `PassTile` hook: the walk settles at the stop tile
    // (`plan::stopped()` is the read-only check).
    ctx::plan::set_stop_at(ryuseido);
    ctx::plan::set_resolve(true);
    // 规则书（2）: 「使用2星星贴纸」 -- C# `H.AddTok(Seat, "星星贴纸", -2)`.
    ctx::add_tok(player_id, "星星贴纸", -2, i32::MAX)?;
    ctx::log(
        player_id,
        &Msg::new(key!("random_star_stop"))
            .player_id("who", player_id)
            .tile("tile", ryuseido),
    );
    Ok(())
}
