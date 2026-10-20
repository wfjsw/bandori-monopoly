//! `HHW:（薰）怪盗hello happy` -- C# `CardKaoruThief` (MatchHost.cs:3780-3871).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（薰）怪盗hello happy`）:
//! > （薰）怪盗hello happy： 
//! >  将此卡放置于场上（充能3，衰减1）并向一名玩家场上放置一个怪盗标记，你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。
//!
//! Place this card in play and put a thief mark on one other player. The
//! crystal decay, the DiceRollPlan dice (1d2 direction + 1d10 distance) and the
//! optional force-stop are in; the `H.AbnormalGate` wrapper around the stop
//! stays a narrow TODO.

use card_sdk::abi::{ActionKind, Dice, FilterKind};
use bandori_std::ctx::{self, trigger};
use card_sdk::{key, CardDef, FilterBody, Msg, On, PlanValue};

const ID: &str = "HHW:（薰）怪盗hello happy";

pub const KAORU_THIEF: CardDef = CardDef::new(
    "HHW:（薰）怪盗hello happy",
    &[
        On::Play("", "", Some(cant_play), play),
        On::Action(
            &[ActionKind::TurnEnd],
            "actor == owner && card.placed", "",
            None,
            turn_end,
        ),
        // 「你本回合的移动阶段可以选择在经过该玩家时使自己强制停下」 -- the
        // offer is about the *marked* player. The mark-match is a dynamic-key
        // residual (the -1 unmarked sentinel is not CEL's `slot` missing=0).
        On::Action(
            &[ActionKind::PassPlayer],
            "actor == owner && card.placed && move.remaining > 0 && slot('kaoru_thief_turn') == turn_key", "",
            Some(marked_here),
            pass_player,
        ),
        // The body-top applicability checks are the condition now (C#
        // `m.Main && m.Seat != Seat && seats[m.Seat].pos == Me.pos &&
        // m.FixedRoll < 0`): a placed card whose tile the mover shares, and no
        // fixed roll yet.
        On::FilterValue(&[FilterKind::DiceRollPlan],
            "card.placed && turn_player >= 0 && turn_player != owner && pos(turn_player) == owner.pos && plan.fixed_roll == null",
            None,
            FilterBody::Plan(roll_plan),
        ),
        On::Action(
            &[ActionKind::CounterChanged],
            "actor == owner && card.placed && counter_is('crystals') && trigger_card == card.id && card.counter('crystals') == 0 && value <= 0", "",
            Some(crystals_changed_guard),
            on_crystals_changed,
        ),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"怪盗hello happy"#)
        .raw_name(r#"（薰）怪盗hello happy"#)
        .tags(&[])
        .text(r#"将此卡放置于场上（充能3，衰减1）并向一名玩家场上放置一个怪盗标记，你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。"#)
        .cell(r#"F5"#)
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", r#"濑田薰"#)]));

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
    // The mark is a held counter on that player (「…玩家场上」), named for the
    // book's 「怪盗标记」; `skill:濑田薰:梦幻的王子殿下` reads and removes the
    // same counter.
    if let Some(who) = hit {
        ctx::add_tok(who, "怪盗标记", 1, i32::MAX)?;
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
/// Residual guard for [`pass_player`]: the passer is the marked player.
/// `kaoru_thief_marked` uses -1 = unmarked (`slot()`'s missing = 0 does not
/// match), so this stays out of CEL.
fn marked_here(player_id: i32) -> bool {
    let marked = ctx::slot(player_id, "kaoru_thief_marked");
    marked >= 0 && trigger::target() == marked
}

fn pass_player(player_id: i32) -> card_sdk::Asked {
    let marked = ctx::slot(player_id, "kaoru_thief_marked");
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
    if !ctx::gate(trigger::player_id(), bandori_std::kind::abnormal::Stop) {
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
/// 和1d10（距离）进行结算」 -- C# `CardKaoruThief.DiceRollPlan`: when anyone but the
/// owner starts a main move standing on the card's tile (and the plan has no
/// fixed roll yet), the dice become 1d2 (direction) + 1d10 (distance).
/// Applicability is the entry's condition; `mover` is
/// the plan's `turn_player` (C# `m.Seat`).
fn roll_plan(_player_id: i32, v: PlanValue) -> PlanValue {
    // 规则书: 「投掷1d2（前后）」 -- the host pre-rolls the direction face into
    // `recorded_roll` (ABI v78) so this Filter stays pure. Odd face = reverse
    // (task formula; C# was `m.Reverse = num == 2` -- face 2 only. Documented
    // inversion: the sheet says 「前后」 as a 1d2, and the v78 convention is
    // odd=reverse).
    // TODO(规则书): the host does not fill `recorded_roll` yet, so the
    //   direction stays `None` and the `kaoru_thief_rollplan` dir log is
    //   skipped (no face). Once the host pre-rolls, `reverse` and the log
    //   pick up the face. The 1d10 base is independent of that.
    let reverse = v.recorded_roll.map(|dir| dir % 2 == 1);
    if let Some(dir) = v.recorded_roll {
        let mover = ctx::turn_player();
        ctx::log(
            mover,
            &Msg::new(key!("kaoru_thief_rollplan"))
                .player_id("who", mover)
                .i("dir", dir as i64),
        );
    }
    // 规则书: 「和1d10（距离）」 -- the face is *exactly* 1d10 (clear extra dice).
    PlanValue {
        reverse,
        clear_dice: true,
        base: Some(Dice { count: 1, sides: 10 }),
        ..v
    }
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn pass_player_kani(player_id: i32) -> card_sdk::Asked {
    pass_player(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_end_kani(player_id: i32) -> card_sdk::Asked {
    turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn roll_plan_kani(player_id: i32, v: PlanValue) -> PlanValue {
    roll_plan(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
