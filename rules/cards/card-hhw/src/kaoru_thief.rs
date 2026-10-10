//! `HHW:（薰）怪盗hello happy` -- C# `CardKaoruThief` (MatchHost.cs:3780-3871).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（薰）怪盗hello happy`）:
//! > （薰）怪盗hello happy： 
//! >  将此卡放置于场上（充能3，衰减1）并向一名玩家场上放置一个怪盗标记，你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。
//!
//! Place this card in play and put a thief mark on one other player. The
//! crystal decay, the RollPlan dice (1d2 direction + 1d10 distance) and the
//! optional force-stop are in; the `H.AbnormalGate` wrapper around the stop
//! stays a narrow TODO.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "HHW:（薰）怪盗hello happy";

pub const KAORU_THIEF: CardDef = CardDef::new(
    "HHW:（薰）怪盗hello happy",
    &[
        On::Play("", Some(cant_play), play),
        On::Hook(
            &[HookKind::TurnEnd],
            "actor == owner && card.placed",
            None,
            turn_end,
        ),
        On::Hook(
            &[HookKind::PassPlayer],
            "actor == owner && card.placed && move.remaining > 0 && slot('kaoru_thief_turn') == turn_key",
            None,
            pass_player,
        ),
        // The body-top applicability checks are the condition / residual guard
        // now (C# `m.Main && m.Seat != Seat && seats[m.Seat].pos == Me.pos &&
        // m.FixedRoll < 0`): a placed card whose tile the mover shares, and no
        // fixed roll yet. `fixed_roll` is derived plan state outside the CEL
        // schema (docs/GUARDS.md §4.2c), so it stays a residual guard.
        On::RollPlan(
            "card.placed && turn_player >= 0 && turn_player != owner && pos(turn_player) == owner.pos",
            Some(roll_plan_unfixed),
            roll_plan,
        ),
        On::Hook(
            &[HookKind::CounterChanged],
            "actor == owner && card.placed && counter_is('crystals') && trigger_card == card.id && card.counter('crystals') == 0 && value <= 0",
            Some(crystals_changed_guard),
            on_crystals_changed,
        ),
    ],
);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardKaoruThief.WhyNot`: refuses the play with no other player alive.
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_rival")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    let others = ctx::others(player_id);
    // 规则书: 「向一名玩家场上放置一个怪盗标记」 -- C# `H.PickTarget` over `H.Others`:
    // `H.AskSeat` then the `H.Target` gate (`SingleTarget`).
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("kaoru_thief_ask_title")),
        &Msg::new(key!("kaoru_thief_ask_text")),
        &others,
    )?;
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
    ctx::set_crystals(3);
    // 规则书: 「向一名玩家场上放置一个怪盗标记」 -- C# `if (r.index >= 0)` gates the
    // mark on the gate's answer (the player actually hit, after any redirect).
    if let Some(who) = hit {
        ctx::add_tok(who, key!("kaoru_thief_tok"), 1, i32::MAX)?;
        // C# `card.Mem["marked"] = r.index; card.Mem["turn"] = H.TurnKey` -- the
        // PassPlayer force-stop only works on the turn the mark was placed.
        ctx::set_slot(player_id, "kaoru_thief_marked", who);
        ctx::set_slot(player_id, "kaoru_thief_turn", ctx::turn_key());
        ctx::log(
            player_id,
            &Msg::new(key!("kaoru_thief_marked")).player_id("who", who),
        );
    }
    Ok(())
}

/// 规则书: 「你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算」
/// -- C# `CardKaoruThief.PassSeat` -> `Stop` (MatchHost.cs:3834-3860).
fn pass_player(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && card.placed && move.remaining > 0 &&
    // slot('kaoru_thief_turn') == turn_key` is the pre. `other != Marked`
    // keeps its `slot` sentinel (`kaoru_thief_marked` uses -1 = unmarked and
    // `slot()`'s missing = 0 does not match).
    let marked = ctx::slot(player_id, "kaoru_thief_marked");
    if marked < 0 || trigger::target() != marked {
        return Ok(());
    }
    // 规则书: 「可以选择在经过该玩家时使自己强制停下并触发结算」 -- C#
    // `H.AskYes(..., aiYes: false)` then `m.Stopped = true; m.Resolve = true`.
    let who = marked;
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("kaoru_thief_stop_title")),
        &Msg::new(key!("kaoru_thief_stop_ask")).player_id("who", who),
    )? {
        return Ok(());
    }
    // C# `m.Stopped = true; m.Resolve = true` -- stop at the marked player's
    // tile and settle (`plan::set_stop_at` + `set_resolve`).
    // 规则书: 「[强制停下]」 -- behind `H.AbnormalGate`.
    if !ctx::gate(trigger::player_id(), card_sdk::abi::AbKind::Stop) {
        return Ok(());
    }
    let tile = trigger::tile();
    if tile >= 0 {
        ctx::plan::set_stop_at(tile);
        ctx::plan::set_resolve(true);
    }
    Ok(())
}

/// 规则书: 「（充能3，衰减1）」 -- C# `DecayCard.TurnEnd` (`turn == DecayOn` =
/// the owner's turn) -> `AddCrystals(-1)`. `actor == owner && card.placed` is
/// the pre.
fn turn_end(_player_id: i32) -> card_sdk::Asked {
    ctx::decay()?;
    Ok(())
}

/// 「充能/衰减」 runs out -- C# `Empty()` / `H.Unplace(this, "discard")`.
///
/// TODO(规则书): 「（充能3，衰减1）」 names the charge and the decay but never
/// says what happens at 0, while every other crystal card spells out 「为0时
/// 置入弃牌堆」. This keeps the C# behaviour (it decays out) -- the book may
/// intend something else, e.g. leaving a spent card on the field.
/// Residual guard -- the name is also `counter_is('crystals')` in the pre
/// (kept here as well). Card identity is already in the CEL pre (`trigger_card ==
/// card.id`).
fn crystals_changed_guard(_player_id: i32) -> bool {
    trigger::name() == card_sdk::abi::counter::CRYSTALS
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("kaoru_thief_decayed")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书: 「此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）
/// 和1d10（距离）进行结算」 -- C# `CardKaoruThief.RollPlan`: when anyone but the
/// owner starts a main move standing on the card's tile (and the plan has no
/// fixed roll yet), the dice become 1d2 (direction) + 1d10 (distance).
/// Applicability is the entry's condition + [`roll_plan_unfixed`]; `mover` is
/// the plan's `turn_player` (C# `m.Seat`).
fn roll_plan(player_id: i32) -> card_sdk::Asked {
    let mover = ctx::turn_player();
    // 规则书: 「投掷1d2（前后）」 -- C# `H.Roll(m.Seat, 1, 2, ...)` sets
    // `m.Reverse = num == 2`.
    let dir = ctx::roll(mover, 1, 2);
    // 规则书: 「（前后）」 -- C# `m.Reverse = num == 2` (`set_reverse` is
    // `Plan.Reverse`).
    ctx::plan::set_reverse(dir == 2);
    // 规则书: 「和1d10（距离）」 -- C# `m.Base.Clear(); m.Base.Add((1, 10, ...))`
    // plus `m.Dice.Clear()`: the face is *exactly* 1d10, so another card's
    // extra dice come off too.
    ctx::plan::clear_dice();
    ctx::plan::set_base_dice(1, 10, "（怪盗hello happy）");
    ctx::log(
        mover,
        &Msg::new(key!("kaoru_thief_rollplan"))
            .player_id("who", mover)
            .i("dir", dir as i64),
    );
    Ok(())
}

/// C# `m.FixedRoll < 0` -- the plan still has no fixed face, so the dice table
/// is still the one to rewrite. Derived plan state, outside the CEL schema
/// (docs/GUARDS.md §4.2c): the residual guard of [`roll_plan`].
fn roll_plan_unfixed(_player_id: i32) -> bool {
    ctx::fixed_roll().is_none()
}
