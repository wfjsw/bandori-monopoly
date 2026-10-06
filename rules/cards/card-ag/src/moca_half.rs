//! `AG:（摩卡）0.5倍速` -- C# `CardMocaHalf` (MatchHost.cs:1161-1197):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（摩卡）0.5倍速`）:
//! > （摩卡）0.5倍速：
//! > (1)  将此卡放置在场上并获得3个奇迹水晶，你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆。
//! > (2) 此卡在场时，你的移动掷骰的最终结算/2（向上取整）且你的所有资金支付与消耗减半。
//!
//! placed card, 3 crystals decaying each own turn end; halve rolls and payments.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "AG:（摩卡）0.5倍速";

pub const MOCA_HALF: CardDef = CardDef::new(
    "AG:（摩卡）0.5倍速",
    &[
        On::Play(None, play),
        On::Hook(&[HookKind::TurnEnd], turn_end_guard, turn_end),
        On::Hook(&[HookKind::RollAfter], |_| true, roll_after),
        On::Hook(&[HookKind::PayMul], |_| true, pay_mul),
        On::Hook(
            &[HookKind::CrystalsChanged],
            crystals_changed_guard,
            on_crystals_changed,
        ),
    ],
);

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书(1): 「将此卡放置在场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("moca_half_note")));
    // 规则书(1): 「并获得3个奇迹水晶」 -- the placement's crystal charge (C#
    // `H.PlaceFromPlay(c, -1, -1, 3)`).
    ctx::set_crystals(3);
    ctx::log(
        player_id,
        &Msg::new(key!("moca_half_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书(1): 「你的回合结束时移除一个奇迹水晶」 -- C# `DecayCard.TurnEnd` ->
/// `AddCrystals(-1)`. The 「奇迹水晶为0时此卡放入弃牌堆」 half is
/// [`on_crystals_changed`], not this tick.
/// Pure guard for [`turn_end`] -- the activation gate. `false`
/// means the card is not activated at all.
fn turn_end_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn turn_end(_player_id: i32) -> card_sdk::Asked {
    ctx::decay();
    Ok(())
}

/// 规则书(1): 「奇迹水晶为0时此卡放入弃牌堆」 -- C# `Empty()` /
/// `H.Unplace(this, "discard")`.
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
        &Msg::new(key!("moca_half_decayed")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书(2): 「此卡在场时，你的移动掷骰的最终结算/2（向上取整）」 -- C#
/// `CardMocaHalf.RollAfter`: `m.Roll = ceil(max(0, roll)/2)` on the player's own
/// main-move roll.
fn roll_after(player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() != player_id || !trigger::move_is_main() || !ctx::is_placed() {
        return Ok(());
    }
    let roll = trigger::value().max(0);
    let halved = (roll + 1) / 2;
    trigger::set_move_roll(halved);
    ctx::log(
        player_id,
        &Msg::new(key!("moca_half_roll"))
            .player_id("who", player_id)
            .i("from", roll as i64)
            .i("to", halved as i64),
    );
    Ok(())
}

/// 规则书(2): 「且你的所有资金支付与消耗减半」 -- C# `CardMocaHalf.PayMul`
/// (`p.from == Player && p.amount > 0` -> `p.amount = CeilTo(amount / 2.0, 10)`).
/// The `PayMul` pass (after `PayAdd`, before `PayChoose`) rewrites the amount
/// via `ctx::trigger::set_pay_amount`.
fn pay_mul(player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() != player_id || !ctx::is_placed() {
        return Ok(());
    }
    let amount = trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    // `CeilTo(amount / 2.0, 10)` -- half, rounded up to a multiple of 10.
    let half = (amount as i64 + 1) / 2;
    let half = ((half + 9) / 10) * 10;
    let half = half.clamp(0, i32::MAX as i64) as i32;
    trigger::set_pay_amount(half);
    ctx::log(
        player_id,
        &Msg::new(key!("moca_half_pay"))
            .player_id("who", player_id)
            .n("from", amount as i64)
            .n("to", half as i64),
    );
    Ok(())
}
