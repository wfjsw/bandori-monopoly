//! `PPP:（里美）我的心就像巧克力螺` -- C# `CardRimiChoco` (MatchHost.cs:9241-9285):
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（里美）我的心就像巧克力螺`）:
//! > （里美）我的心就像巧克力螺：
//! > [手]：
//! > 立刻进入移动阶段，本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子并[结算]，期间[不可阻挡]。
//!
//! jump 1-4 tiles (either way) as the main move, unstoppable.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const RIMI_CHOCO: CardDef = CardDef::new(
    "PPP:（里美）我的心就像巧克力螺",
    &[On::Play("", Some(cant_play), play)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"我的心就像巧克力螺"#)
        .raw_name(r#"（里美）我的心就像巧克力螺"#)
        .tags(&[r#"手"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"立刻进入移动阶段，本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子并[结算]，期间[不可阻挡]。"#),
            ]),
        ]))
        // sheet cell B19
        .fields(&[("group", r#"Poppin' Party"#), ("owner", r#"牛込里美"#)]));

/// 规则书: 「本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子」
/// -- the card replaces the main move, so only on the owner's turn and while
/// the main move is still available (C# `CardRimiChoco.WhyNot` =
/// `H.MoveWhyNot(seat)`).
fn cant_play(player_id: i32) -> Option<Msg> {
    // 「期间[不可阻挡]」 -- an existing [停留] only latches the move gate, which
    // this card's own grant reopens before the walk (glossary 51: an
    // unstoppable holder ignores a stay), so that latch is not a refusal reason
    // (`to_the_peak`'s `H.Moving` admits the same case). Not-your-turn and
    // already-moved stay refusals.
    if ctx::cant_move_code(player_id) == 3
        && rules_status::stay::get(player_id) > 0
        && ctx::state::get(player_id, bandori_std::state_key::UNSTOPPABLE) <= 0
    {
        return None;
    }
    rules_status::move_blocked(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    let pos = ctx::player_pos(player_id);
    if n <= 0 || pos < 0 {
        return Ok(());
    }
    // 规则书: 「移动到当前格子绝对距离1到4格或以内的任何格子」 -- C#
    // `CardRimiChoco.Play` offers the eight tiles at offsets ±1..±4.
    let mut tiles: Vec<i32> = Vec::new();
    for i in -4i32..=4 {
        if i != 0 {
            tiles.push((pos + i).rem_euclid(n));
        }
    }
    if tiles.is_empty() {
        return Ok(());
    }
    // Eight options -- the bound the Kani `ask_tile_walk` model covers.
    let to = ctx::ask_tile_walk(
        player_id,
        &Msg::new(key!("rimi_choco_title")),
        &Msg::new(key!("rimi_choco_ask")),
        &tiles,
    )?;
    // 规则书: 「期间[不可阻挡]」 -- the walk is armed [不可阻挡]. The stay gate
    // reads the state; `abnormal_gate` does not, so a [停留] met during the
    // walk still lands (BACKLOG STA-02).
    let held = ctx::state::get(player_id, "unstoppable");
    // `add` stacks a layer rather than clobbering the count (`tsukushi_garden`,
    // `rinko_1cm` grant the same status this way).
    ctx::state::add(player_id, "unstoppable", 1);
    bandori_std::status::declare_companion(player_id, "unstoppable");
    // TODO(规则书)（期间[不可阻挡]）: the book scopes [不可阻挡] to this move -- we
    // leave it to the turn's end, so an END-phase [晕眩]/[除外] grant is refused
    // although the walk is already over. A longer grant already running (to the
    // next turn start, or permanent) is not shortened: its expiry is kept, and
    // the layer added here wears off on that same clock.
    if held <= 0 {
        ctx::state::set_expires(player_id, "unstoppable", ctx::state::TURN_END);
    }
    // 规则书: 「并[结算]」 / 「本回合的[主要移动]改为移动到当前格子绝对距离1到4格
    // 或以内的任何格子」 -- C# `H.CardMove` with
    // `Steps` (forward, or `Reverse` with `n - steps` when the target is behind).
    // `set_resolve(true)` states the landing settles: the shared `turn.plan`
    // outlives a move and can still carry a `false` from an earlier effect.
    let steps = ctx::tile_forward(pos, to);
    let reverse = steps > 4;
    let len = if reverse { n - steps } else { steps };
    ctx::plan::set_steps(len);
    ctx::plan::set_reverse(reverse);
    ctx::plan::set_resolve(true);
    ctx::log(
        player_id,
        &Msg::new(key!("rimi_choco_moved"))
            .player_id("who", player_id)
            .tile("tile", to)
            .i("n", len as i64),
    );
    // 规则书: 「立刻进入移动阶段」 -- the card runs the walk now. `ctx::card_move`
    // goes through `abnormal_gate`, which never consults [不可阻挡] (its
    // `movement` local is dead, STA-02), so the status only opens the stay gate
    // and suppresses new [晕眩]/[除外] layers.
    ctx::card_move(player_id);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
