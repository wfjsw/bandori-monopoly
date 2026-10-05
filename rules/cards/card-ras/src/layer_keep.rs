//! `RAS:（和奏瑞依）寄于指尖的执念` -- C# `CardLayerKeep` (MatchHost.cs:10118-10187):
//! keep the unpicked fire-pot die and later spend fire to reuse it.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（和奏瑞依）寄于指尖的执念`）:
//! > （和奏瑞依）寄于指尖的执念：
//! > 当你使用火罐进行掷骰时，保留（写下）未被选择的另一个骰点，在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点。可保留多个骰点。
//!

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const LAYER_KEEP: CardDef = CardDef::new("RAS:（和奏瑞依）寄于指尖的执念", &[
    On::Play(play),
    On::Hook(&[HookKind::RollAfter], roll_after),
]);

const ID: &str = "RAS:（和奏瑞依）寄于指尖的执念";

/// Where the fire-pot system writes the unchosen die (C# `H.V(Seat, "layerUnused")`).
const SLOT_UNUSED: &str = "layerUnused";
/// How many dice are kept (C# `_kept.Count`).
const SLOT_COUNT: &str = "layer_keep_count";
/// Kept-die slots (C# `_kept` list); 8 is the practical cap.
const SLOT_DICE: [&str; 8] = [
    "layer_keep_0", "layer_keep_1", "layer_keep_2", "layer_keep_3",
    "layer_keep_4", "layer_keep_5", "layer_keep_6", "layer_keep_7",
];

fn play(player_id: i32) {
    // 规则书: 「当你使用火罐进行掷骰时，保留（写下）未被选择的另一个骰点」
    // -- the card itself just stays in play (C# `H.PlaceFromPlay(c)`).
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("layer_keep_note")));
    ctx::log(player_id, &Msg::new(key!("layer_keep_placed")).player_id("who", player_id));
}

/// C# `CardLayerKeep.RollAfter` -- after a fire-pot roll, keep the unchosen die.
fn roll_after(player_id: i32) {
    if !ctx::is_placed(player_id) || trigger::player_id() != player_id {
        return;
    }
    // 规则书: 「当你使用火罐进行掷骰时，保留（写下）未被选择的另一个骰点」
    // -- C# reads `H.V(Seat, "layerUnused")`, keeps `value - 1`, clears the slot.
    let unused = ctx::slot(player_id, SLOT_UNUSED);
    if unused <= 0 {
        return;
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
        &Msg::new(key!("layer_keep_kept")).player_id("who", player_id).i("n", die as i64),
    );
    // TODO(规则书): 「在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点。可保留多个骰点。」
    // -- needs the `H.Actions` action menu (C# `CardLayerKeep.Actions` offers
    // 「用保留的骰点」whenever `_kept` is non-empty). `ctx::spend_fire` and
    // `ctx::set_fixed_roll` are ready; the kept dice live in `layer_keep_N` slots.
    // The action entry point itself is not in the vocabulary yet.
}
