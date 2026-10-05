//! `RAS:（和奏瑞依）寄于指尖的执念` -- C# `CardLayerKeep` (MatchHost.cs:10118-10187):
//! keep the unpicked fire-pot die and later spend fire to reuse it.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:（和奏瑞依）寄于指尖的执念`）:
//! > （和奏瑞依）寄于指尖的执念：
//! > 当你使用火罐进行掷骰时，保留（写下）未被选择的另一个骰点，在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点。可保留多个骰点。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const LAYER_KEEP: CardDef = CardDef {
    id: "RAS:（和奏瑞依）寄于指尖的执念",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // 规则书: 「当你使用火罐进行掷骰时，保留（写下）未被选择的另一个骰点」
    // -- the card itself just stays in play (C# `H.PlaceFromPlay(c)`).
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        seat,
        "RAS:（和奏瑞依）寄于指尖的执念",
        &Msg::new(key!("layer_keep_note")),
    );
    ctx::log(seat, &Msg::new(key!("layer_keep_placed")).seat("who", seat));
    // TODO(规则书): 「当你使用火罐进行掷骰时，保留（写下）未被选择的另一个骰点」
    // -- needs the Fx.RollAfter hook (C# `CardLayerKeep.RollAfter` reads
    // `H.V(Seat, "layerUnused")`, keeps `value - 1`, and clears the slot). The
    // slot itself is readable today as `ctx::slot(seat, "layerUnused")`; the
    // hook that runs after the roll is missing.
    // TODO(规则书): 「在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点。可保留多个骰点。」
    // -- needs the Fx.Actions hook (C# `CardLayerKeep.Actions` offers
    // "用保留的骰点" whenever `_kept` is non-empty), `H.SpendFire` (only
    // `ctx::fire` / `ctx::gain_fire` exist), the move dice plan's `FixedRoll`
    // (C# `H._turnCtx.Plan.FixedRoll`), and per-card `_kept` memory (`Mem` on a
    // placed card -- not in the vocabulary).
}