//!
//! 规则书（docs/rulebook/cards.json, id `Mor:纯真振翅`）:
//! > 纯真振翅：传送到移动方向20格后（不触发结算），立刻进行移动掷骰
//!
//! ahead (no settle), then take the main move roll right away.

use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const PURE_WINGS: CardDef = CardDef::new("Mor:纯真振翅", &[On::Play("", None, pure_wings)])
    .meta(
        &card_sdk::meta::CardMeta::new()
            .name(r#"纯真振翅"#)
            .raw_name(r#"纯真振翅"#)
            .tags(&[])
            .text(CardText::para(
                r#"传送到移动方向20格后（不触发结算），立刻进行移动掷骰"#,
            ))
            // sheet cell G13
            .fields(&[("group", r#"Morfonica"#), ("owner", "")]),
    );

fn pure_wings(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    // 规则书: 「移动方向」 is the moving player's own direction. The plan frame is
    // seeded from the command, so `dir()` is this move's only direction source.
    let dir = ctx::plan::dir();
    let to = ctx::tile_steps_ahead(player_id, 20 * dir);
    if to < 0 {
        return Ok(());
    }
    // 规则书: 「传送到移动方向20格后（不触发结算）」 -- the jump settles nothing.
    ctx::plan::set_settles(false);
    ctx::teleport_to(player_id, to);
    if ctx::player_out(player_id) {
        return Ok(());
    }
    // 规则书: 「立刻进行移动掷骰」 -- clear the destination and the step count so
    // `run_planned_move` rolls this move's dice and walks that far, then settles
    // on the landing. The frame's own fields are the only delta; the pay factor
    // starts at the frame default.
    ctx::plan::clear_steps();
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Walk);
    ctx::plan::set_teleport_to(-1);
    ctx::plan::set_settles(true);
    ctx::run_planned_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("pure_wings_roll")).player_id("who", player_id),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn pure_wings_kani(player_id: i32) -> card_sdk::Asked {
    pure_wings(player_id)
}
