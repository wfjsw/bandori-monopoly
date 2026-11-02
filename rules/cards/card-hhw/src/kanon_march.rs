//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（花音）Wacha Mocha 啪嗒进行曲`）:
//! > （花音）Wacha Mocha 啪嗒进行曲：[场]花音每次倒走时此卡获得一个水母标记；主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。
//!

use alloc::vec::Vec;

use card_sdk::abi::MoveKind;
use card_sdk::CardText;
use card_sdk::{ctx, key, Action, Activation, CardDef, Declared, Msg, On};

const ID: &str = "HHW:（花音）Wacha Mocha 啪嗒进行曲";
/// 「水母标记」.
const JELLY: &str = "水母标记";

pub const KANON_MARCH: CardDef = CardDef::new(
    "HHW:（花音）Wacha Mocha 啪嗒进行曲",
    &[
        // 「[场]」 is the hand play and only the placement. The jump is a placed
        // offer anchored at the pre-dice plan pass -- `DiceRollPlan` is the
        // declaring side of the filter kind, so the question runs before
        // anything is announced and a decline is silent.
        On::Play("", Some(cant_place), place),
        On::Action(Action::DiceRollPlan(
            "card.placed && actor == owner && move.is_main_move",
            Some(can_jump),
            unused_body, // replaced by `declare_arg`
        ))
        .declare_arg(pick_jump, jump),
        // `move.dir < 0` is the direction clause; `moveAfter` fires once per move, not
        // once per tile.
        On::Action(Action::MoveAfter("actor == owner && move.kind != Teleport && move.dir < 0", None, on_pass)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Wacha Mocha 啪嗒进行曲"#)
        .raw_name(r#"（花音）Wacha Mocha 啪嗒进行曲"#)
        .tags(&[])
        .text(CardText::para(r#"[场]花音每次倒走时此卡获得一个水母标记；主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。"#))
        // sheet cell F13
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", r#"松原花音"#)]));

/// Constructor placeholder: `.declare_arg` replaces the body (docs/ACTIVATION.md §2).
const fn unused_body(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

/// 「[场]」 -- a placement is unconditional; the jump's conditions are the
/// offer's guard ([`can_jump`]) and its declare.
fn cant_place(_player_id: i32) -> Option<Msg> {
    None
}

/// 「若此卡上有至少9个水母标记」; another card's claim on this move stands this
/// one down. The movement gate is the anchor's own: `main_move` stops before the
/// plan pass once the main move is spent, and a teleport is legal under
/// [停留]/[晕眩] (rules.txt 47 「无法使用非[传送]的移动效果」).
fn can_jump(player_id: i32) -> bool {
    ctx::held_count(player_id, JELLY) >= 9 && !ctx::plan::is_claimed()
}

/// 「可以清除所有标记并传送到#4水族馆或者 #30弦卷豪宅」 -- asked at the
/// admission layer, so a decline leaves no body, no flash and no log. The pick
/// is the declaration's arg; the jump below acts on it.
fn pick_jump(player_id: i32) -> Declared<i32> {
    let a = ctx::tile_named("水族馆");
    let b = ctx::tile_named("弦卷豪宅");
    // 规则书: the printed indices are the fallback when a custom board renames
    // the two tiles.
    let a = if a >= 0 { a } else { 3 };
    let b = if b >= 0 { b } else { 29 };
    let pool: Vec<i32> = [a, b].into_iter().filter(|t| *t >= 0).collect();
    if pool.is_empty() {
        return Ok(Activation::Decline);
    }
    // `ask_tiles` carries a 「不选」 slot (it answers `-1`), so the one prompt is
    // both the 「可以」 and the destination pick -- `ask_tile` alone always
    // answers a tile and could not express the 不发动.
    let labels: Vec<Msg> = pool
        .iter()
        .map(|&t| Msg::new("ask.tileOption").tile("tile", t))
        .collect();
    let prices = vec![0i32; pool.len()];
    let pick = ctx::ask_tiles(
        player_id,
        &Msg::new(key!("kanon_march_title")),
        &Msg::new(key!("kanon_march_where")),
        &pool,
        &labels,
        &prices,
        -1,
    )?;
    if pick < 0 {
        Ok(Activation::Decline)
    } else {
        Ok(Activation::Arg(pick))
    }
}

/// 「[场]花音每次倒走时此卡获得一个水母标记」 -- the placement. `place` keeps
/// the 9-cap.
fn place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("kanon_march_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("kanon_march_placed")).player_id("who", player_id),
    );
    ctx::add_held_by_name(player_id, JELLY, 0, 9)?;
    Ok(())
}

/// Applicability is the entry's condition (`move.dir < 0`).
fn on_pass(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「花音每次倒走时此卡获得一个水母标记」
    ctx::add_held_by_name(player_id, JELLY, 1, 9)?;
    ctx::log(
        player_id,
        &Msg::new(key!("kanon_march_mark")).i("n", ctx::held_count(player_id, JELLY) as i64),
    );
    Ok(())
}

/// 「…视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆」 -- the accepted
/// half. `pick` is the tile the declare already chose.
fn jump(player_id: i32, pick: i32) -> card_sdk::Asked {
    // The claim comes first: a declare's writes are dropped, and only an
    // accepted offer may claim the move for the other offers' guards.
    ctx::plan::claim();
    // 规则书: 「清除所有标记」 -- every 水母标记 on this card.
    ctx::set_held(player_id, JELLY, 0, 0, -1);
    // 规则书: 「视为本次主要移动」 -- this *is* that move: the plan becomes a
    // teleport and main_move's teleport branch runs it, no die, landing settles.
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(pick);
    ctx::plan::set_settles(true);
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
    place(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn place_kani(player_id: i32) -> card_sdk::Asked {
    place(player_id)
}

/// The declare's collector -- it may prompt, so it answers a `Declared`.
#[cfg(kani)]
#[inline(always)]
pub fn pick_jump_kani(player_id: i32) -> card_sdk::Declared<i32> {
    pick_jump(player_id)
}

/// The declare's body arm; the caller supplies the answered tile.
#[cfg(kani)]
#[inline(always)]
pub fn jump_kani(player_id: i32, pick: i32) -> card_sdk::Asked {
    jump(player_id, pick)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}
