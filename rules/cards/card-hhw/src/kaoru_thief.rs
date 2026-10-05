//! `HHW:（薰）怪盗hello happy` -- C# `CardKaoruThief` (MatchHost.cs:3780-3871).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（薰）怪盗hello happy`）:
//! > （薰）怪盗hello happy：
//! >  将此卡放置于场上（充能3，衰减1）并向一名玩家场上放置一个怪盗标记，你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。
//!
//! Place this card in play and put a thief mark on one other player. The
//! crystal decay and the RollPlan dice (1d2 direction + 1d10 distance) are in;
//! the force-stop stays a TODO below.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "HHW:（薰）怪盗hello happy";

pub const KAORU_THIEF: CardDef = CardDef::new("HHW:（薰）怪盗hello happy", &[
    On::Play(play),
    On::CantPlay(cant_play),
    On::Hook(&[TriggerKind::TurnEnd], turn_end),
    On::RollPlan(roll_plan),
]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardKaoruThief.WhyNot`: refuses the play with no other player alive.
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_rival")));
    }
    None
}

fn play(player_id: i32) {
    let others = ctx::others(player_id);
    // 规则书: 「向一名玩家场上放置一个怪盗标记」 -- C# `H.PickTarget` over `H.Others`:
    // `H.AskSeat` then the `H.Target` gate (`SingleTarget`).
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("kaoru_thief_ask_title")),
        &Msg::new(key!("kaoru_thief_ask_text")),
        &others,
    );
    // C# `H.Target(c, r.index, t)` -> `res.index = t.yes ? t.index : -1`: out /
    // exile / `ImmuneAll` / `Untargetable` / the `target` [反击] window all fail
    // the designation, and a `redirect` hook may move the hit.
    let hit = ctx::target(who);
    // 规则书: 「将此卡放置于场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`, placed
    // whether or not the designation got through.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("kaoru_thief_note")));
    // 规则书: 「（充能3，衰减1）」 -- the placement's crystal charge (C#
    // `H.PlaceFromPlay(c, -1, -1, 3)`).
    ctx::set_crystals(player_id, 3);
    // 规则书: 「向一名玩家场上放置一个怪盗标记」 -- C# `if (r.index >= 0)` gates the
    // mark on the gate's answer (the player actually hit, after any redirect).
    if let Some(who) = hit {
        ctx::add_tok(who, key!("kaoru_thief_tok"), 1, i32::MAX);
        ctx::log(player_id, &Msg::new(key!("kaoru_thief_marked")).player_id("who", who));
    }
    // TODO(规则书): 「你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算」
    // -- needs the Fx.PassSeat hook body (C# `CardKaoruThief.PassSeat` ->
    // `Stop`, `m.Stopped = true`) -- the `passPlayer` kind exists but the
    // force-stop is an immediate-move routine (`H.AbnormalGate` + `MoveCtx`
    // mutation) that is still landing in another session.
}

/// 规则书: 「（充能3，衰减1）」 -- C# `DecayCard.TurnEnd` (`turn == DecayOn` =
/// the owner's turn) -> `AddCrystals(-1)`; empty -> `Empty()` /
/// `H.Unplace(this, "discard")`. `ctx::decay` is that body.
fn turn_end(player_id: i32) {
    if !ctx::is_placed(player_id) || trigger::player_id() != player_id {
        return;
    }
    if ctx::decay(player_id, ID) == 0 {
        ctx::log(player_id, &Msg::new(key!("kaoru_thief_decayed")).player_id("who", player_id));
    }
}

/// 规则书: 「此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）
/// 和1d10（距离）进行结算」 -- C# `CardKaoruThief.RollPlan`: when anyone but the
/// owner starts a main move standing on the card's tile (and the plan has no
/// fixed roll yet), the dice become 1d2 (direction) + 1d10 (distance).
fn roll_plan(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    let mover = ctx::turn_player();
    // C# `m.Main && m.Seat != Seat && seats[m.Seat].pos == Me.pos && m.FixedRoll < 0`.
    if mover == player_id || mover < 0 {
        return;
    }
    if ctx::player_pos(mover) != ctx::player_pos(player_id) {
        return;
    }
    if ctx::fixed_roll().is_some() {
        return;
    }
    // 规则书: 「投掷1d2（前后）」 -- C# `H.Roll(m.Seat, 1, 2, ...)` sets
    // `m.Reverse = num == 2`.
    let dir = ctx::roll(mover, 1, 2);
    // 规则书: 「（前后）」 -- C# `m.Reverse = num == 2` (`set_reverse` is
    // `Plan.Reverse`).
    ctx::plan::set_reverse(dir == 2);
    // 规则书: 「和1d10（距离）」 -- C# `m.Base = { (1, 10, ...) }`; the planned
    // roll is the 1d10 face (`set_fixed_roll` is `Plan.FixedRoll`). The C#
    // `m.Base.Clear(); m.Base.Add((1, 10, ...))` dice-shape rewrite has no
    // `set_base` counterpart, so the 1d10 is pre-rolled as the fixed roll.
    let dist = ctx::roll(mover, 1, 10);
    ctx::set_fixed_roll(dist);
    ctx::log(
        mover,
        &Msg::new(key!("kaoru_thief_rollplan"))
            .player_id("who", mover)
            .i("dir", dir as i64)
            .i("dist", dist as i64),
    );
}
