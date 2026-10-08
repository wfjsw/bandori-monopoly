//! `MyGO:无路矢` -- C# `CardNoRoad` (MatchHost.cs:6416): pick another player's
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:无路矢`）:
//! > 无路矢：
//! >  指定场上自己以外的一位玩家所在格子，获得2层[除外]并在[除外]层数归0后[传送]至该格子，视为当回合的主要移动。[除外]期间本应获得的格子收入由此前指定的那名玩家获得。
//!
//! tile, take 2 [exile] layers, and teleport there when the exile runs out.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const NO_ROAD: CardDef = CardDef::new(
    "MyGO:无路矢",
    &[
        On::Hook(&[card_sdk::abi::HookKind::PayAdd], Some(|player_id| ctx::is_placed()), redirect, ""),
        On::Play(Some(cant_play), no_road, ""),
    ],
);

/// 规则书: 「指定场上自己以外的一位玩家所在格子」 -- C# `CardNoRoad.WhyNot`
/// refuses the card with no other player alive ("没有别的玩家").
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("no_road_why_not_none")));
    }
    None
}

fn no_road(player_id: i32) -> card_sdk::Asked {
    let others = ctx::others(player_id);
    if others.is_empty() {
        return Ok(());
    }
    // 规则书: 「指定场上自己以外的一位玩家所在格子」 -- C# `H.PickTarget` asks among
    // `H.Others` (already out/exile-free) and then runs the `H.Target` gate; the
    // seat that lands (`t.index`, possibly redirected) is the one used below.
    let picked = ctx::ask_player(
        player_id,
        &Msg::new(key!("no_road_title")),
        &Msg::new(key!("no_road_ask")),
        &others,
    )?;
    // C# `H.Target(c, r.index, t)` after the prompt (ImmuneAll / Untargetable /
    // redirect / the `target` [反击] window); on failure PlayCtx is marked
    // non-effective and the card does nothing.
    let who = match ctx::target(picked) {
        Some(hit) => hit,
        None => {
            // TODO(规则书)[judgement]: 「视为此卡未生效」 -- the clause names a state without
            // saying what observes it. `PlayCtx.Effective = false` is the C#'s mutable
            // side channel and is not being ported (a routine should *return* whether
            // it took effect); but before that lands, what "not effective" changes has
            // to be ruled: does the card get spent (haneoka 「放入弃牌堆且视为此卡未生效」
            // says yes) or not (noble_blue / starry_night's "the card is spent anyway"
            // implies no)? And what counts a use that this would suppress?
            return Ok(());
        }
    };
    let tile = ctx::player_pos(who);
    // 规则书: 「获得2层[除外]并在[除外]层数归0后[传送]至该格子」
    // `H.GiveExile(seat, 2, tile)` stores the return Ok(tile); the engine teleports
    // there when the last layer ticks away.
    ctx::give_exile(player_id, 2, tile);
    // 规则书: 「视为当回合的主要移动」 -- C# `H.SetV(i, "exileMain", 1)`. The
    // engine's exile tick reads this slot and consumes the turn's main move.
    ctx::set_slot(player_id, card_sdk::abi::state_key::EXILE_MAIN, 1);
    // 规则书: 「在[除外]层数归0后[传送]至该格子」 -- the log names both the
    // destination and the redirect below.
    ctx::log(
        player_id,
        &Msg::new(key!("no_road_log"))
            .tile("tile", tile)
            .player_id("who", who)
            .card("card", "MyGO:无路矢"),
    );
    // 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得」 -- the redirect
    // hook below reroutes this player's rent to `who` while the exile lasts. The
    // redirect must outlive the play body, and a field hook only runs on a
    // *placed* card -- so the card stands in on the field for the exile (the
    // same field-stand-in + slot pattern Anon Tokyo uses). The hook's own
    // `exile > 0` guard stops it once the exile ticks away.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "MyGO:无路矢", &Msg::new(key!("no_road_log")));
    ctx::set_slot(player_id, "noRoad.who", who);
    Ok(())
}

/// 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得」 -- C#
/// `NoRoadFx.RedirectReceiver`. The exile layer count is the **live** one
/// (`state_key::EXILE`); `turn_snap` is the turn-start snapshot and is stale
/// for a layer granted mid-turn.
fn redirect(player_id: i32) -> card_sdk::Asked {
    if ctx::state::get(player_id, card_sdk::abi::state_key::EXILE) <= 0 {
        return Ok(());
    }
    let who = ctx::slot(player_id, "noRoad.who");
    if who < 0 || who == player_id {
        return Ok(());
    }
    // This player's own tile collected a rent -- hand it to `who`.
    if !ctx::trigger::pay_is_rent() || ctx::trigger::target() != player_id {
        return Ok(());
    }
    ctx::trigger::set_pay_target(who);
    ctx::log(
        player_id,
        &Msg::new(key!("no_road_redirect")).player_id("who", who),
    );
    Ok(())
}

// `exileMain` is honoured by the engine now (2026-10-06): `game-core`'s exile
// tick reads the slot at 「在[除外]层数归0后」 and runs the return teleport as
// that turn's main move -- `TurnCtx::main_moved` is set, so 「一回合只能触发
// 一次［主要移动］」 keeps the player from rolling as well (C#
// `H.MoveWhyNot` / `_turnCtx.MainMoved`).
