//! `RAS:（和奏瑞依）寄于指尖的执念` -- C# `CardLayerKeep` (MatchHost.cs:10118-10187):
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（和奏瑞依）寄于指尖的执念`）:
//! > （和奏瑞依）寄于指尖的执念：
//! >  [反击] 当你使用火罐进行掷骰时，可打出此卡并保留（写下）未被选择的另一个骰点，在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点。可保留多个骰点。
//!
//! keep the unpicked fire-pot die and later spend fire to reuse it.

use alloc::vec::Vec;
use card_sdk::abi::{roll_source, ChainKind, HookKind};

use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const LAYER_KEEP: CardDef = CardDef::new(
    "RAS:（和奏瑞依）寄于指尖的执念",
    &[
        // One Play entry for both contexts (the engine dispatches only the
        // first): the placement from hand (「可打出此卡」), or the later press
        // that spends a 火罐 on a kept die. The gate admits whenever either
        // branch is available.
        On::Play("", Some(cant_play), play),
        On::Hook(&[HookKind::RollAfter], "actor == owner && card.placed", None, roll_after),
        // 规则书 [反击]: 「当你使用火罐进行掷骰时，可打出此卡并保留（写下）未被
        // 选择的另一个骰点」 -- the window opens on a `Roll` chain link whose
        // source is `roll_source::FIRE` (「使用火罐进行掷骰」).
        On::Counteract(
            &[ChainKind::Roll],
            "actor == owner && roll_source == 1",
            None,
            counter_fire,
        ),
    ],
)
.legacy(&[(2, legacy_can_counter_fire)]);

/// G4 audit oracle (docs/GUARDS.md §5.1).
fn legacy_can_counter_fire(player_id: i32) -> bool {
    trigger::player_id() == player_id && trigger::roll_source() == roll_source::FIRE
}

const ID: &str = "RAS:（和奏瑞依）寄于指尖的执念";

/// Where the fire-pot system writes the unchosen die (C# `H.V(Seat, "layerUnused")`).
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
    if ctx::fire(player_id) < 1 {
        return Some(Msg::new(key!("layer_keep_no_fire")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    if ctx::is_placed() {
        return use_die(player_id);
    }
    // 规则书: 「可打出此卡并保留（写下）未被选择的另一个骰点」 -- the card
    // itself just stays in play (C# `H.PlaceFromPlay(c)`); the unchosen die is
    // written by the `RollAfter` hook below.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("layer_keep_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("layer_keep_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardLayerKeep.RollAfter` -- after a fire-pot roll, keep the unchosen die.
/// `actor == owner && card.placed` is the pre.
/// 规则书 [反击]: 「当你使用火罐进行掷骰时」 -- only a `Roll` chain link whose
/// source is `roll_source::FIRE` (the fire-pot reroll).
// (the live guard is the `pre` on the Counteract entry; `legacy_can_counter_fire`
// above is the G4 audit oracle)

/// 规则书 [反击]: 「可打出此卡并保留（写下）未被选择的另一个骰点」 -- the card
/// is played from hand; its `play` body places it on the field. The unchosen
/// die is written by the fire-pot reroll path to the `layerUnused` slot and
/// picked up by the `RollAfter` hook.
fn counter_fire(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("layer_keep_counter")),
    );
    Ok(())
}

fn roll_after(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「当你使用火罐进行掷骰时，保留（写下）未被选择的另一个骰点」
    // -- C# reads `H.V(Seat, "layerUnused")`, keeps `value - 1`, clears the slot.
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
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("layer_keep_spend")))? {
        return Ok(());
    }
    // 「删去该骰点」 -- compact the slot list.
    let mut kept: Vec<i32> = Vec::new();
    for i in 0..n {
        if (i as usize) >= SLOT_DICE.len() {
            break;
        }
        let v = ctx::slot(player_id, SLOT_DICE[i as usize]);
        if v != die {
            kept.push(v);
        }
    }
    for i in 0..SLOT_DICE.len() {
        ctx::set_slot(player_id, SLOT_DICE[i], *kept.get(i).unwrap_or(&0));
    }
    ctx::set_slot(player_id, SLOT_COUNT, kept.len() as i32);
    ctx::set_fixed_roll(die);
    ctx::log(
        player_id,
        &Msg::new(key!("layer_keep_used")).i("n", die as i64),
    );
    Ok(())
}
