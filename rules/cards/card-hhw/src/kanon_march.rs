//! `HHW:（花音）Wacha Mocha 啪嗒进行曲` -- C# `CardKanonMarch` (MatchHost.cs:4234-4291).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（花音）Wacha Mocha 啪嗒进行曲`）:
//! > （花音）Wacha Mocha 啪嗒进行曲：[场]花音每次倒走时此卡获得一个水母标记；主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。
//!

use alloc::vec::Vec;

use card_sdk::abi::{ActionKind, MoveKind};
use card_sdk::CardText;
use card_sdk::{ctx, key, Action, CardDef, Msg, On};

const ID: &str = "HHW:（花音）Wacha Mocha 啪嗒进行曲";
/// 「水母标记」.
const JELLY: &str = "水母标记";

pub const KANON_MARCH: CardDef = CardDef::new(
    "HHW:（花音）Wacha Mocha 啪嗒进行曲",
    &[
        // One Play entry for both contexts (the engine dispatches only the
        // first): [场] placement from a hand play, or the ≥9-marker jump on a
        // press of the placed copy. The body picks the branch by its origin.
        // 规则书: 「主要阶段中」 -- the engine's play/skill gates only open in
        // the owner's OPS stage, so the entry needs no stage check of its own.
        On::Play("", Some(cant_play), play),
        // 「花音每次倒走时」 -- once per completed backward walk that is not a
        // teleport (C# `m.Reverse && !m.Teleport`). `move.dir < 0` is the
        // direction clause; `moveAfter` fires once per move, not once per tile.
        On::Action(Action::MoveAfter("actor == owner && move.kind != Teleport && move.dir < 0", None, on_pass)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Wacha Mocha 啪嗒进行曲"#)
        .raw_name(r#"（花音）Wacha Mocha 啪嗒进行曲"#)
        .tags(&[])
        .text(CardText::para(r#"[场]花音每次倒走时此卡获得一个水母标记；主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。"#))
        // sheet cell F13
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", r#"松原花音"#)]));

/// Combined gate: a hand play is the [场] placement and is always allowed; a
/// press on the placed copy is the jump -- it needs 「至少9个水母标记」 and an
/// open main move. A press below 9 markers with no copy in hand is refused with
/// `kanon_march_need`; the hand-copy case is re-checked in `jump`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 「[场]」 -- a hand copy can always be played (it is placed by `play`). The
    // field copy's id is bound even for a hand play (uid by id), so `is_placed`
    // alone cannot tell the two apart: a held copy also has to admit the play.
    // TODO(规则书)(ABI)（视为本次主要移动）: the gate cannot tell a hand play
    // from a press -- `ctx::play_from_hand()` is only set once the hand play is
    // committed, so a held copy also admits a below-9 / move-spent press (which
    // `jump` then re-checks for markers only).
    if ctx::hand_count(player_id, ID) > 0 || !ctx::is_placed() {
        return None;
    }
    if ctx::tok(player_id, JELLY) >= 9 {
        // 「视为本次主要移动」 -- the jump is refused while that move is shut.
        return rules_status::move_blocked(player_id);
    }
    Some(Msg::new(key!("kanon_march_need")))
}

fn play(player_id: i32) -> card_sdk::Asked {
    // The play's origin decides the branch: a hand (or forced) play places the
    // [场] card, a press on the placed copy (`use_skill`) is the jump.
    if ctx::play_from_hand() || !ctx::is_placed() {
        // 规则书: 「[场]」 -- a field card; C# `H.PlaceFromPlay(c)`.
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(
            player_id,
            "HHW:（花音）Wacha Mocha 啪嗒进行曲",
            &Msg::new(key!("kanon_march_note")),
        );
        ctx::log(
            player_id,
            &Msg::new(key!("kanon_march_placed")).player_id("who", player_id),
        );
        ctx::add_tok(player_id, JELLY, 0, 9)?;
        return Ok(());
    }
    // TODO(规则书)（视为本次主要移动）: the jump needs a press of the placed copy,
    // but the press surface lists `skill:*` field instances only -- no client or
    // bot can trigger it in real play until placed cards are pressable.
    jump(player_id)
}

/// 「花音每次倒走时此卡获得一个水母标记」 -- C# `CardKanonMarch.Arrive`
/// (`m.Reverse && !m.Teleport`), fired once per completed backward walk.
/// Applicability is the entry's condition (`move.dir < 0`).
fn on_pass(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「花音每次倒走时此卡获得一个水母标记」
    ctx::add_tok(player_id, JELLY, 1, 9)?;
    ctx::log(
        player_id,
        &Msg::new(key!("kanon_march_mark")).i("n", ctx::tok(player_id, JELLY) as i64),
    );
    Ok(())
}

/// 「主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆
/// 或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆」.
fn jump(player_id: i32) -> card_sdk::Asked {
    // 「至少9个水母标记」 is the play gate's (`cant_play`) clause; a press on
    // the placed copy can pass that gate below 9 markers while a second copy is
    // held (the gate then admits the hand play), so re-check here. The re-run
    // after `card_move` still reads the pre-clear count (the clear is written
    // after the pause), so this check is replay-safe; a `move_blocked` check
    // would not be (the move has already stamped `main_moved`), so the main
    // move stays the gate's job.
    if ctx::tok(player_id, JELLY) < 9 {
        return Ok(());
    }
    let a = ctx::tile_named("水族馆");
    let b = ctx::tile_named("弦卷豪宅");
    // 规则书: 「传送到#4水族馆或者 #30弦卷豪宅」 -- the printed indices are the
    // fallback when a custom board renames the two tiles.
    let a = if a >= 0 { a } else { 3 };
    let b = if b >= 0 { b } else { 29 };
    let mut pool: Vec<i32> = Vec::new();
    if a >= 0 {
        pool.push(a);
    }
    if b >= 0 {
        pool.push(b);
    }
    if pool.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_tile(
        player_id,
        &Msg::new(key!("kanon_march_title")),
        &Msg::new(key!("kanon_march_where")),
        &pool,
    )?;
    // 规则书: 「清除所有标记」 -- every 水母标记 on this card; the discard is
    // the 「然后此卡置入弃牌堆」 at the tail of the body.
    ctx::set_tok(player_id, JELLY, 0, 0, -1);
    // 规则书: 「传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)」
    // -- the jump runs now, as the turn's main move (`card_move`).
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(pick);
    ctx::plan::set_resolve(true);
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("kanon_march_jump")).tile("tile", pick),
    );
    // 规则书: 「然后此卡置入弃牌堆」
    ctx::set_dest(ctx::Dest::Graveyard);
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

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}
