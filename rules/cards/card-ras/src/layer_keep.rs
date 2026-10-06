//! `RAS:（和奏瑞依）寄于指尖的执念` -- C# `CardLayerKeep` (MatchHost.cs:10118-10187):
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（和奏瑞依）寄于指尖的执念`）:
//! > （和奏瑞依）寄于指尖的执念：
//! > 当你使用火罐进行掷骰时，保留（写下）未被选择的另一个骰点，在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点。可保留多个骰点。
//!
//! keep the unpicked fire-pot die and later spend fire to reuse it.

use alloc::vec::Vec;
use card_sdk::abi::HookKind;

use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const LAYER_KEEP: CardDef = CardDef::new(
    "RAS:（和奏瑞依）寄于指尖的执念",
    &[
        On::Play(None, play),
        On::Play(Some(can_use), use_die),
        On::Hook(&[HookKind::RollAfter], roll_after_guard, roll_after),
    ],
);

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

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「当你使用火罐进行掷骰时，保留（写下）未被选择的另一个骰点」
    // -- the card itself just stays in play (C# `H.PlaceFromPlay(c)`)?.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("layer_keep_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("layer_keep_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardLayerKeep.RollAfter` -- after a fire-pot roll, keep the unchosen die.
/// Pure guard for [`roll_after`] -- the activation gate. `false`
/// means the card is not activated at all.
fn roll_after_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
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

/// 「在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰」 -- a press.
fn can_use(player_id: i32) -> Option<Msg> {
    if !ctx::is_placed() {
        return Some(Msg::new(key!("layer_keep_not_placed")));
    }
    if ctx::slot(player_id, SLOT_COUNT) < 1 {
        return Some(Msg::new(key!("layer_keep_no_die")));
    }
    if ctx::fire(player_id) < 1 {
        return Some(Msg::new(key!("layer_keep_no_fire")));
    }
    None
}

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
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("layer_keep_spend"))) {
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
