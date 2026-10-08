//! `R:Fire bird` -- C# `CardFireBird` (MatchHost.cs:10446-10516): pay 1,600 and
//!
//! 规则书（docs/rulebook/cards.json, id `R:Fire bird`）:
//! > Fire bird：
//! >  支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶，自己场地上存在此卡时，每回合结束时失去400资金并移除1个奇迹水晶，自己的所有格子收费变成1.5倍，奇迹水晶耗尽时将此卡放入弃牌堆
//!
//! place this card with X self-chosen miracle crystals. While it is in play:
//! pay 400 and burn one crystal at every turn end, all your tiles charge 1.5x
//! rent, and the card hits the discard when the crystals run out.

use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const FIRE_BIRD: CardDef = CardDef::new(
    "R:Fire bird",
    &[
        // TODO(规则书) NEGATION-AUDIT V2: no activation cost. The 「支付1600资金」
        // is effect content (rulebook L13-14), so there is no `money >= 1600`
        // play gate -- an unaffordable in-body payment takes the Q1 shortfall
        // path (mortgage, then bankruptcy). C# `CardFireBird.WhyNot` had one.
        On::Play("", None, play),
        On::Hook(&[HookKind::TurnEnd], "", Some(turn_end_guard), turn_end),
        On::Hook(&[HookKind::PayMul], "", Some(pay_mul_guard), pay_mul),
        On::Hook(&[HookKind::CrystalsChanged], "", Some(crystals_changed_guard), on_crystals_changed),
    ],
).props(&[(card_sdk::abi::prop::EST_COST, 1600)]);

const ID: &str = "R:Fire bird";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「支付1600资金」 -- C# `PayCtx { amount = 1600, kind = "pay", must = false }`.
    let paid = ctx::pay(player_id, 1600, &Msg::new(key!("fire_bird_why")))?;
    if paid < 1600 {
        // TODO(规则书)[judgement]: 「视为此卡未生效」 -- the clause names a state without
        // saying what observes it. `PlayCtx.Effective = false` is the C#'s mutable
        // side channel and is not being ported (a routine should *return* whether
        // it took effect); but before that lands, what "not effective" changes has
        // to be ruled: does the card get spent (haneoka 「放入弃牌堆且视为此卡未生效」
        // says yes) or not (noble_blue / starry_night's "the card is spent anyway"
        // implies no)? And what counts a use that this would suppress?
        return Ok(());
    }
    // 规则书: 「并为其添加X（自选）个奇迹水晶」 -- C# `H.AskNumber(i, ..., 1, 10, ...)`.
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("fire_bird_ask_title")),
        &Msg::new(key!("fire_bird_ask_text")),
        1,
        10,
    )?;
    // 规则书: 「将此卡放置在自己场地上」 -- C# `H.PlaceFromPlay(c, -1, -1, Math.Max(1, r.value))`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("fire_bird_note")));
    // 规则书: 「并为其添加X（自选）个奇迹水晶」 -- the crystals live on the placed card
    // (C# `Card.Crystals` / `H.PlaceFromPlay(..., crystals)`), charged here.
    let n = x.max(1);
    ctx::set_crystals(n);
    ctx::log(
        player_id,
        &Msg::new(key!("fire_bird_placed"))
            .player_id("who", player_id)
            .i("n", n as i64),
    );
    // 规则书: 「每回合结束时失去400资金并移除1个奇迹水晶」 / 「奇迹水晶耗尽时将此卡放入
    // 弃牌堆」 -- `DecayCard.TurnEnd` -> `CardFireBird.Burn`, see `turn_end`.
    // 规则书: 「自己的所有格子收费变成1.5倍」 -- C# `CardFireBird.PayMul`, see
    // `pay_mul`.
    Ok(())
}

/// C# `CardFireBird.PayMul`: rent paid to this card's owner is charged at
/// `CeilTo(amount * 1.5, 10)`. Runs through the Fx hook dispatch at `payMul`
/// (after `PayAdd`, before `PayChoose`), so this is a field effect, not a [反击].
/// Pure guard for [`pay_mul`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pay_mul_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn pay_mul(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::PayMul || !trigger::pay_is_rent() {
        return Ok(());
    }
    // C# `p.to == Player` -- the rent must be landing on this card's owner.
    if trigger::target() != player_id {
        return Ok(());
    }
    let amount = trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    // `CeilTo(amount * 1.5, 10)` -- 1.5x, rounded up to a multiple of 10.
    let boosted = (amount as i64 * 3 + 1) / 2;
    let boosted = ((boosted + 9) / 10) * 10;
    let boosted = boosted.clamp(0, i32::MAX as i64) as i32;
    trigger::set_pay_amount(boosted);
    ctx::log(
        player_id,
        &Msg::new(key!("fire_bird_pay_mul"))
            .player_id("who", player_id)
            .n("from", amount as i64)
            .n("to", boosted as i64),
    );
    Ok(())
}

/// `DecayCard.TurnEnd` -> `CardFireBird.Burn` (C# `H.LoseR(Seat, 400, "Fire bird")`
/// then `AddCrystals(-1)`; empty -> `H.Unplace(this, "discard")`). Runs through the
/// Fx hook dispatch, so this is a field effect, not a [反击].
/// Pure guard for [`turn_end`] -- the activation gate. `false`
/// means the card is not activated at all.
fn turn_end_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn turn_end(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「每回合结束时失去400资金」 -- C# `H.LoseR(Seat, 400, "Fire bird")`.
    ctx::pay(
        player_id,
        400,
        &Msg::new(key!("fire_bird_burn")).player_id("who", player_id),
    )?;
    // 规则书: 「并移除1个奇迹水晶」 -- C# `AddCrystals(-1, "回合结束")`. The
    // 「奇迹水晶耗尽时将此卡放入弃牌堆」 half is [`on_crystals_changed`].
    ctx::decay()?;
    Ok(())
}

/// 规则书: 「奇迹水晶耗尽时将此卡放入弃牌堆」 -- C# `H.Unplace(this, "discard")`.
///
/// Listens to this card's own [`HookKind::CrystalsChanged`] rather than being
/// re-checked at the decay tick, so a count emptied by *any* write leaves the
/// field just the same.
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all.
fn crystals_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && ctx::crystals() == 0
        // Only a write that did not raise the count speaks for the empty
        // state; see AG:绯红之魂 (3).
        && trigger::value() <= 0
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("fire_bird_decayed")).player_id("who", player_id),
    );
    Ok(())
}
