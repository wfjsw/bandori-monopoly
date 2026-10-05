//! `R:Fire bird` -- C# `CardFireBird` (MatchHost.cs:10446-10516): pay 1,600 and
//!
//! 规则书（docs/rulebook/cards.json, id `R:Fire bird`）:
//! > Fire bird：
//! >  支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶，自己场地上存在此卡时，每回合结束时失去400资金并移除1个奇迹水晶，自己的所有格子收费变成1.5倍，奇迹水晶耗尽时将此卡放入弃牌堆
//!
//! place this card with X self-chosen miracle crystals. While it is in play:
//! pay 400 and burn one crystal at every turn end, all your tiles charge 1.5x
//! rent, and the card hits the discard when the crystals run out.

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const FIRE_BIRD: CardDef = CardDef::new("R:Fire bird", &[
    On::Play(play),
    On::CantPlay(cant_play),
    On::Hook(&[HookKind::TurnEnd], turn_end),
    On::Hook(&[HookKind::PayMul], pay_mul),
]);

const ID: &str = "R:Fire bird";

/// C# `CardFireBird.WhyNot`: 「资金不够 1,600」.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「支付1600资金」 -- the pay is the card's cost (C#
    // `H.State.seats[seat].money >= 1600`).
    if ctx::money(player_id) < 1600 {
        return Some(Msg::new(key!("fire_bird_no_money")));
    }
    None
}

fn play(player_id: i32) {
    // 规则书: 「支付1600资金」 -- C# `PayCtx { amount = 1600, kind = "pay", must = false }`.
    let paid = ctx::pay(player_id, 1600, &Msg::new(key!("fire_bird_why")));
    if paid < 1600 {
        // TODO(规则书): the C# sets `c.Effective = false` when the payment does not
        // go through (the play is wasted); the ABI has no set_effective hook.
        return;
    }
    // 规则书: 「并为其添加X（自选）个奇迹水晶」 -- C# `H.AskNumber(i, ..., 1, 10, ...)`.
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("fire_bird_ask_title")),
        &Msg::new(key!("fire_bird_ask_text")),
        1,
        10,
    );
    // 规则书: 「将此卡放置在自己场地上」 -- C# `H.PlaceFromPlay(c, -1, -1, Math.Max(1, r.value))`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("fire_bird_note")));
    // 规则书: 「并为其添加X（自选）个奇迹水晶」 -- the crystals live on the placed card
    // (C# `Card.Crystals` / `H.PlaceFromPlay(..., crystals)`), charged here.
    let n = x.max(1);
    ctx::set_crystals(player_id, n);
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
}

/// C# `CardFireBird.PayMul`: rent paid to this card's owner is charged at
/// `CeilTo(amount * 1.5, 10)`. Runs through the Fx hook dispatch at `payMul`
/// (after `PayAdd`, before `PayChoose`), so this is a field effect, not a [反击].
fn pay_mul(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    if trigger::kind() != TriggerKind::PayMul || !trigger::pay_is_rent() {
        return;
    }
    // C# `p.to == Player` -- the rent must be landing on this card's owner.
    if trigger::target() != player_id {
        return;
    }
    let amount = trigger::value();
    if amount <= 0 {
        return;
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
}

/// `DecayCard.TurnEnd` -> `CardFireBird.Burn` (C# `H.LoseR(Seat, 400, "Fire bird")`
/// then `AddCrystals(-1)`; empty -> `H.Unplace(this, "discard")`). Runs through the
/// Fx hook dispatch, so this is a field effect, not a [反击].
fn turn_end(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    if trigger::kind() != TriggerKind::TurnEnd {
        return;
    }
    // 规则书: 「每回合结束时失去400资金」 -- C# `H.LoseR(Seat, 400, "Fire bird")`.
    ctx::pay(player_id, 400, &Msg::new(key!("fire_bird_burn")).player_id("who", player_id));
    // 规则书: 「并移除1个奇迹水晶」 / 「奇迹水晶耗尽时将此卡放入弃牌堆」 --
    // C# `AddCrystals(-1, "回合结束")`; empty -> `H.Unplace(this, "discard")`.
    if ctx::decay(player_id, ID) == 0 {
        ctx::log(player_id, &Msg::new(key!("fire_bird_decayed")).player_id("who", player_id));
    }
}
