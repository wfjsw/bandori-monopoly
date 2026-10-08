//! `HHW:热气球演出` -- C# `CardBalloonShow` (MatchHost.cs:4134-4162).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:热气球演出`）:
//! > 热气球演出：
//! >  投掷4次3d20mod60并记录其结果，选择其中之一，传送至结果对应序号的格子，视为你的主要移动
//!

use alloc::vec::Vec;
use card_sdk::abi::{roll_source, MoveKind};
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const BALLOON_SHOW: CardDef =
    CardDef::new("HHW:热气球演出", &[On::Play(Some(cant_play), play, "")]);

/// C# `CardBalloonShow.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「视为你的主要移动」 -- the teleport is the turn's main move, so the
    // C# `H.MoveWhyNot` gate applies (off-turn / already-moved / skip-move refuse).
    ctx::cant_move(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    // 规则书: 「投掷4次3d20并记录其结果」 -- C# `H.Roll(seat, 3, 20, ...)` four
    // times. `roll_ask` so the 「掷骰结算前」 [反击] window (Y.O.L.O 「你的任意
    // 掷骰结算前」) rings on each; source is CARD (a hand card's own roll).
    let mut tiles: Vec<i32> = Vec::new();
    for _ in 0..4 {
        let r = ctx::roll_ask(player_id, 3, 20, roll_source::CARD);
        let t = (r - 1).rem_euclid(n);
        if !tiles.contains(&t) {
            tiles.push(t);
        }
    }
    if tiles.is_empty() {
        return Ok(());
    }
    // 规则书: 「选择其中之一」 -- C# `H.AskTileOf` over the distinct rolled tiles.
    let title = Msg::new(key!("balloon_show_ask_title"));
    let text = Msg::new(key!("balloon_show_ask_text"));
    let to = ctx::ask_tile(player_id, &title, &text, &tiles)?;
    // 规则书: 「传送至结果对应序号的格子」 -- C# `H.CardMove(c, new MoveCtx
    // { TeleportTo = to })` (`Resolve` defaults to true, so it settles).
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_resolve(true);
    // 规则书: 「视为你的主要移动」 -- C# `H.CardMove` (`MainMoveAs`) consumes the
    // turn's main move and runs the teleport immediately.
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("balloon_show_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    Ok(())
}
