//! `RAS:（和奏瑞依）寄于指尖的执念` -- C# `CardLayerKeep` (MatchHost.cs:10118-10187):
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（和奏瑞依）寄于指尖的执念`）:
//! > （和奏瑞依）寄于指尖的执念：
//! >  [反击] 当你使用火罐进行掷骰时，可打出此卡并保留（写下）未被选择的另一个骰点，在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点。可保留多个骰点。
//!
//! keep the unpicked fire-pot die and later spend fire to reuse it.

use alloc::vec::Vec;
use card_sdk::abi::{ActionKind, ChainKind};
use card_sdk::CardText;

use bandori_std::ctx;
use card_sdk::{key, Action, CardDef, Msg, On};

pub const LAYER_KEEP: CardDef = CardDef::new(
    "RAS:（和奏瑞依）寄于指尖的执念",
    &[
        // One Play entry for both contexts (the engine dispatches only the
        // first): the placement from hand (「可打出此卡」), or the later press
        // that spends a 火罐 on a kept die. The gate admits whenever either
        // branch is available.
        // TODO(规则书)（[反击] 当你使用火罐进行掷骰时）: the sheet times the card
        // only to the [反击] window; the hand placement branch is open in any
        // phase (and with `COUNTERACT_FROM_FIELD` a premplay stays usable).
        On::Play("", Some(cant_play), play),
        On::Action(Action::DiceRollAfter("actor == owner && card.placed", None, roll_after)),
        // 规则书 [反击]: 「当你使用火罐进行掷骰时，可打出此卡并保留（写下）未被
        // 选择的另一个骰点」 -- the window opens on a `Roll` chain link whose
        // source is `roll_source::CHARGE` (「使用火罐进行掷骰」).
        On::Counteract(
            &[ChainKind::DiceRollBefore],
            "actor == owner && roll_source == 1",
            None,
            counter_fire),
    ],
)
// 「可保留多个骰点」 -- the card stays on the field and must answer later fire
// rolls from there (`COUNTERACT_FROM_FIELD`). One reservation per roll so the
// placed copy cannot re-offer itself on the same window (the tae shape).
.props(&[
    (card_sdk::abi::prop::COUNTERACT_FROM_FIELD, 1),
    (card_sdk::abi::prop::COUNTERACT_GROUP, 1),
])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"寄于指尖的执念"#)
        .raw_name(r#"（和奏瑞依）寄于指尖的执念"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 当你使用火罐进行掷骰时，可打出此卡并保留（写下）未被选择的另一个骰点，在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点。可保留多个骰点。"#))
        // sheet cell H16
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", r#"和奏瑞依"#)]));

const ID: &str = "RAS:（和奏瑞依）寄于指尖的执念";

/// Where the fire-pot reroll is meant to write the unchosen die (C#
/// `H.V(Seat, "layerUnused")`); no writer exists yet -- see [`roll_after`].
const SLOT_UNUSED: &str = "layerUnused";
/// How many dice are kept (C# `_kept.Count`).
const SLOT_COUNT: &str = "layer_keep_count";
/// Kept-die slots (C# `_kept` list); 8 is the practical cap.
const SLOT_DICE: [&str; 8] = [
    "layer_keep_0",
    "layer_keep_1",
    "layer_keep_2",
    "layer_keep_3",
    "layer_keep_4",
    "layer_keep_5",
    "layer_keep_6",
    "layer_keep_7",
];

/// Combined gate: not yet placed (the placement branch), or placed with a
/// kept die and the 「一个火罐」 the reuse needs.
fn cant_play(player_id: i32) -> Option<Msg> {
    if !ctx::is_placed() {
        return None;
    }
    if ctx::slot(player_id, SLOT_COUNT) < 1 {
        return Some(Msg::new(key!("layer_keep_no_die")));
    }
    // 规则书: 「在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰」
    if bandori_std::fire::get(player_id) < 1 {
        return Some(Msg::new(key!("layer_keep_no_fire")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    if ctx::is_placed() {
        return use_die(player_id);
    }
    // 规则书: 「可打出此卡并保留（写下）未被选择的另一个骰点」 -- the card
    // itself just stays in play (C# `H.PlaceFromPlay(c)`).
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("layer_keep_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("layer_keep_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardLayerKeep.DiceRollAfter` -- after a roll, keep the unchosen die.
/// `actor == owner && card.placed` is the pre; it keys on the field copy only
/// (no roll-source test), so any later roll of the owner reaches the body.
///
/// TODO(规则书)（保留（写下）未被选择的另一个骰点）: nothing is banked today --
/// no code writes the `layerUnused` slot, and a card placed inside the reroll's
/// own counteraction window is not in the `DiceRollAfter` walk already in flight.

/// 规则书 [反击]: 「可打出此卡并保留（写下）未被选择的另一个骰点」 -- a
/// counteract declaration runs this body, never `play`, so the placement is here
/// (C# `H.PlaceFromPlay(c)`; the even_lost shape). A field copy answering a
/// later fire already sits there and must not be placed twice.
///
/// TODO(规则书)（可打出此卡并保留（写下）未被选择的另一个骰点）: the body
/// places the card but records nothing; the unchosen die is never handed over.
fn counter_fire(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(player_id, ID, &Msg::new(key!("layer_keep_note")));
        ctx::log(
            player_id,
            &Msg::new(key!("layer_keep_placed")).player_id("who", player_id),
        );
    }
    ctx::log(player_id, &Msg::new(key!("layer_keep_counter")));
    Ok(())
}

fn roll_after(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「当你使用火罐进行掷骰时，可打出此卡并保留（写下）未被选择的另一个骰点」
    // -- C# reads `H.V(Seat, "layerUnused")`, keeps `value - 1`, clears the slot.
    // TODO(规则书)（保留（写下）未被选择的另一个骰点）: the fire reroll writes
    // no `layerUnused`, so this read is always 0 and nothing is banked.
    let unused = ctx::slot(player_id, SLOT_UNUSED);
    if unused <= 0 {
        return Ok(());
    }
    ctx::set_slot(player_id, SLOT_UNUSED, 0);
    let die = unused - 1;
    // C# `_kept.Add(num - 1)` -- the kept-dice list; slots stand in for the
    // per-field-card `List<int>` memory.
    let n = ctx::slot(player_id, SLOT_COUNT);
    if n >= 0 && (n as usize) < SLOT_DICE.len() {
        ctx::set_slot(player_id, SLOT_DICE[n as usize], die);
    }
    ctx::set_slot(player_id, SLOT_COUNT, n + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("layer_keep_kept"))
            .player_id("who", player_id)
            .i("n", die as i64),
    );
    Ok(())
}

/// 「在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰」 -- the
/// placed-press branch of the merged Play entry.
/// 「随后删去该骰点。可保留多个骰点。」
fn use_die(player_id: i32) -> card_sdk::Asked {
    let n = ctx::slot(player_id, SLOT_COUNT);
    if n < 1 {
        return Ok(());
    }
    let mut opts: Vec<Msg> = Vec::new();
    let mut vals: Vec<i32> = Vec::new();
    for i in 0..n {
        if (i as usize) >= SLOT_DICE.len() {
            break;
        }
        let v = ctx::slot(player_id, SLOT_DICE[i as usize]);
        vals.push(v);
        opts.push(Msg::new(key!("layer_keep_option")).i("n", v as i64));
    }
    if vals.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("layer_keep_title")),
        &Msg::new(key!("layer_keep_which")),
        &opts,
    )?;
    let Some(&die) = vals.get(pick) else {
        return Ok(());
    };
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("layer_keep_spend")))? {
        return Ok(());
    }
    // 规则书: 「随后删去该骰点」 -- drop the picked slot (one die), not every
    // equal face, and shift the rest down.
    let mut kept: Vec<i32> = Vec::new();
    for i in 0..n {
        if (i as usize) >= SLOT_DICE.len() {
            break;
        }
        if i as usize != pick {
            kept.push(ctx::slot(player_id, SLOT_DICE[i as usize]));
        }
    }
    for i in 0..SLOT_DICE.len() {
        ctx::set_slot(player_id, SLOT_DICE[i], *kept.get(i).unwrap_or(&0));
    }
    ctx::set_slot(player_id, SLOT_COUNT, kept.len() as i32);
    // 规则书: 「消耗一个火罐以用于替代当回合的移动掷骰」
    // TODO(规则书)（用于替代当回合的移动掷骰）: `fixed_roll` is never one-shot
    // cleared -- it also feeds any later move roll of the same turn, while the
    // kept die it replaces has already been deleted.
    ctx::set_fixed_roll(die);
    ctx::log(
        player_id,
        &Msg::new(key!("layer_keep_used")).i("n", die as i64),
    );
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
pub fn roll_after_kani(player_id: i32) -> card_sdk::Asked {
    roll_after(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counter_fire_kani(player_id: i32) -> card_sdk::Asked {
    counter_fire(player_id)
}
