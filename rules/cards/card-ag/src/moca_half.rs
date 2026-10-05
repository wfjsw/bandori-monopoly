//! `AG:（摩卡）0.5倍速` -- C# `CardMocaHalf` (MatchHost.cs:1161-1197):
//! placed card, 3 crystals decaying each own turn end; halve rolls and payments.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（摩卡）0.5倍速`）:
//! > （摩卡）0.5倍速：
//! > (1)  将此卡放置在场上并获得3个奇迹水晶，你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆。
//! > (2) 此卡在场时，你的移动掷骰的最终结算/2（向上取整）且你的所有资金支付与消耗减半。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "AG:（摩卡）0.5倍速";

pub const MOCA_HALF: CardDef = CardDef::new("AG:（摩卡）0.5倍速", &[
    On::Play(play),
    On::Hook(&[TriggerKind::TurnEnd], turn_end),
    On::Hook(&[TriggerKind::RollAfter], roll_after),
    On::Hook(&[TriggerKind::PayMul], pay_mul),
]);

fn play(player_id: i32) {
    // 规则书(1): 「将此卡放置在场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("moca_half_note")));
    // 规则书(1): 「并获得3个奇迹水晶」 -- the placement's crystal charge (C#
    // `H.PlaceFromPlay(c, -1, -1, 3)`).
    ctx::set_crystals(player_id, 3);
    ctx::log(player_id, &Msg::new(key!("moca_half_placed")).player_id("who", player_id));
}

/// 规则书(1): 「你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆」
/// -- C# `DecayCard.TurnEnd` -> `AddCrystals(-1)`; empty -> `Empty()` /
/// `H.Unplace(this, "discard")`. `ctx::decay` is that body.
fn turn_end(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    if ctx::decay(player_id, ID) == 0 {
        ctx::log(player_id, &Msg::new(key!("moca_half_decayed")).player_id("who", player_id));
    }
}

/// 规则书(2): 「此卡在场时，你的移动掷骰的最终结算/2（向上取整）」 -- C#
/// `CardMocaHalf.RollAfter`: `m.Roll = ceil(max(0, roll)/2)` on the player's own
/// main-move roll.
fn roll_after(player_id: i32) {
    if trigger::player_id() != player_id || !trigger::move_is_main() || !ctx::is_placed(player_id) {
        return;
    }
    let roll = trigger::value().max(0);
    let halved = (roll + 1) / 2;
    trigger::set_move_roll(halved);
    ctx::log(
        player_id,
        &Msg::new(key!("moca_half_roll")).player_id("who", player_id).i("from", roll as i64).i("to", halved as i64),
    );
}

/// 规则书(2): 「且你的所有资金支付与消耗减半」 -- C# `CardMocaHalf.PayMul`
/// (`p.from == Player && p.amount > 0` -> `p.amount = CeilTo(amount / 2.0, 10)`).
/// The `PayMul` pass (after `PayAdd`, before `PayChoose`) rewrites the amount
/// via `ctx::trigger::set_pay_amount`.
fn pay_mul(player_id: i32) {
    if trigger::player_id() != player_id || !ctx::is_placed(player_id) {
        return;
    }
    let amount = trigger::value();
    if amount <= 0 {
        return;
    }
    // `CeilTo(amount / 2.0, 10)` -- half, rounded up to a multiple of 10.
    let half = (amount as i64 + 1) / 2;
    let half = ((half + 9) / 10) * 10;
    let half = half.clamp(0, i32::MAX as i64) as i32;
    trigger::set_pay_amount(half);
    ctx::log(
        player_id,
        &Msg::new(key!("moca_half_pay")).player_id("who", player_id).n("from", amount as i64).n("to", half as i64),
    );
}
