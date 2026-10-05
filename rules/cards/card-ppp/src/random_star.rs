//! `PPP:仓库里的Random Star` -- C# `CardRandomStar` (MatchHost.cs:8919-8970):
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:仓库里的Random Star`）:
//! > 仓库里的Random Star：
//! >
//! > （1）将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”放置在卡组底端
//! >
//! > （2）[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]
//! >
//! the card stays in play; the 流星堂 stop is live on the `PassTile` hook.

use card_sdk::abi::{HookKind, MoveKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const RANDOM_STAR: CardDef = CardDef::new("PPP:仓库里的Random Star", &[
    On::Play(None, random_star),
    On::Hook(&[HookKind::PassTile], pass_tile_guard, pass_tile)]);

fn random_star(player_id: i32) {
    // 规则书（1）: 「将此卡放置自身场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PPP:仓库里的Random Star", &Msg::new(key!("random_star_note")));
    ctx::log(player_id, &Msg::new(key!("random_star_placed")).player_id("who", player_id));
    // 规则书（1）: 「并将弃牌堆和手牌洗入卡组」 -- C# `H.ShuffleAllIntoDeck(seat, hand:
    // true, discard: true)`.
    ctx::sweep_to_deck(player_id);
    // 规则书（1）: 「然后将一张“拍卖撤下来了”放置在卡组底端」 -- C#
    // `H.AddToDeck(seat, "PPP:[衍生]拍卖撤下来了", "bottom")`.
    ctx::add_to_deck_at(player_id, "PPP:[衍生]拍卖撤下来了", ctx::DeckPos::Bottom);
    ctx::log(player_id, &Msg::new(key!("random_star_swept")).player_id("who", player_id));
}

/// 规则书（2）: 「[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]」
/// -- C# `CardRandomStar.PassTile` -> `Stop`.
/// Pure guard for [`pass_tile`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pass_tile_guard(player_id: i32) -> bool {
    ctx::is_placed(player_id)
}

fn pass_tile(player_id: i32) {
    // C# `m.Seat != Seat || t != Ryuseido || m.Remaining <= 0 || m.Teleport ||
    // H.Tok(Player, "星星贴纸") < 2`.
    if trigger::player_id() != player_id {
        return;
    }
    let ryuseido = ctx::tile_named("流星堂");
    if ryuseido < 0 || trigger::tile() != ryuseido {
        return;
    }
    // C# `m.Remaining <= 0` -- only a still-walking pass can be intercepted.
    if trigger::move_remaining() <= 0 {
        return;
    }
    // C# `m.Teleport` -- a teleport does not walk past the tile.
    if trigger::move_kind() == Some(MoveKind::Teleport) {
        return;
    }
    if ctx::tok(player_id, "星星贴纸") < 2 {
        return;
    }
    // 规则书（2）: 「可使用2星星贴纸」 -- C# `H.AskYes(..., "经过流星堂：要用 2 个
    // 星星贴纸在这里 [强制停下] 并结算吗？")`.
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("random_star_stop_title")),
        &Msg::new(key!("random_star_stop_ask")),
    ) {
        return;
    }
    // 规则书（2）: 「在“流星堂”强制停下并[结算]」 -- C# `m.Stopped = true; m.Resolve = true`
    // (MatchHost.cs:8955-8963, behind `H.AbnormalGate`). `set_stop_at` is the
    // forced stop from this `PassTile` hook: the walk settles at the stop tile
    // (`plan::stopped()` is the read-only check).
    ctx::plan::set_stop_at(ryuseido);
    ctx::plan::set_resolve(true);
    // 规则书（2）: 「使用2星星贴纸」 -- C# `H.AddTok(Seat, "星星贴纸", -2)`.
    ctx::add_tok(player_id, "星星贴纸", -2, i32::MAX);
    ctx::log(
        player_id,
        &Msg::new(key!("random_star_stop")).player_id("who", player_id).tile("tile", ryuseido),
    );
}