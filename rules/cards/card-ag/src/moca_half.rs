//! `AG:（摩卡）0.5倍速` -- C# `CardMocaHalf` (MatchHost.cs:1161-1197):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（摩卡）0.5倍速`）:
//! > （摩卡）0.5倍速：
//! > (1)  将此卡放置在场上并获得3个奇迹水晶，你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆。
//! > (2) 此卡在场时，你的移动掷骰的最终结算/2（向上取整）且你的所有资金支付与消耗减半。
//!
//! placed card, 3 crystals decaying each own turn end; halve rolls and payments.

use bandori_std::ctx;
use card_sdk::{CardText, it};
use card_sdk::{key, Action, CardDef, Filter, Msg, On, RollValue};

const ID: &str = "AG:（摩卡）0.5倍速";


/// 规则书(1): 「奇迹水晶为0时此卡放入弃牌堆」
///
/// Listens to this card's own [`ActionKind::CounterChanged`] (name-filtered to
/// `counter::CRYSTALS`) rather than being re-checked at the decay tick, so a
/// count emptied by *any* write leaves the field just the same.
///
/// Migrated to CEL: the former Rust residual guard (`trigger::card_is(ID) &&
/// trigger::name() == card_sdk::abi::counter::CRYSTALS`) is folded into this
/// pre as `trigger_card == card.id && counter_is('crystals')`. The cond
/// vocabulary has no `card_is` function; `trigger_card` / `card.id` are the
/// same `id_of` hashes and this entry only runs as this card, so the
/// comparison is the exact `trigger::card_is(ID)` spelling.
const CRYSTALS_CHANGED_PRE: &str =
    "actor == owner && card.placed && trigger_card == card.id && counter_is('crystals') && card.counter('crystals') == 0 && value <= 0";

pub const MOCA_HALF: CardDef = CardDef::new(
    "AG:（摩卡）0.5倍速",
    &[
        // > (1)  将此卡放置在场上并获得3个奇迹水晶
        On::Play("", None, play),

        // 你的回合结束时移除一个奇迹水晶
        On::Action(Action::TurnEnd("card.placed", None, turn_end)),

        // 奇迹水晶为0时此卡放入弃牌堆。
        // TODO: Check, this effect should be silent
        On::Action(Action::CounterChanged(
            CRYSTALS_CHANGED_PRE,
            None,
            on_crystals_changed,
        )),

        // > (2) 此卡在场时，你的移动掷骰的最终结算/2（向上取整）
        // TODO: Check 最终结算
        On::Filter(Filter::DiceRollValue("actor == owner && move.main && card.placed", None, roll_half)),

        // 且你的所有资金支付与消耗减半。
        // Note: Mul happens after Add
        On::Filter(Filter::PaySplitMul("actor == owner && card.placed && value > 0", None, pay_mul)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"0.5倍速"#)
        .raw_name(r#"（摩卡）0.5倍速"#)
        .tags(&[r#"手"#, r#"场地"#])
        .text(CardText::items(&[
            it(r#"将此卡放置在场上并获得3个奇迹水晶，你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆。"#),
            it(r#"此卡在场时，你的移动掷骰的最终结算/2（向上取整）且你的所有资金支付与消耗减半。"#),
        ]))
        // sheet cell C7
        .fields(&[("group", r#"Afterglow"#), ("owner", r#"青叶摩卡"#)]));

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书(1): 「将此卡放置在场上」
    // TODO: Why we need 2 function to place a card on the field?
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("moca_half_note")));
    // 规则书(1): 「并获得3个奇迹水晶」
    ctx::set_crystals(3);
    ctx::log(
        player_id,
        &Msg::new(key!("moca_half_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书(1): 「你的回合结束时移除一个奇迹水晶」 
/// The 「奇迹水晶为0时此卡放入弃牌堆」 half is
/// [`on_crystals_changed`], not this tick.
fn turn_end(_player_id: i32) -> card_sdk::Asked {
    ctx::remove_crystal()?;
    Ok(())
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
/// `CardMocaHalf.DiceRollAfter`: `m.Roll = ceil(max(0, roll)/2)` on the player's own
/// main-move roll. ABI v74: a pure `DiceRollValue` filter (placement order among
/// roll reshapes is preserved).
///
/// TODO(规则书) composition with another roll reshape. 「最终结算」 reads as
/// *after* additive modifiers (`docs/rulebook/spec/cards/ag.md`
/// `C-AG-half-speed-03`), which would put this half **last**: against
/// `PP:不要背负期待` 「投掷结果减少2」 that is 7 → 5 → ceil(5/2) = **3**. The
/// engine instead folds roll reshapes in placement order, so with 0.5倍速
/// placed first it is 7 → 4 → 2. `g02_half_speed_plus_expectations_floor_on_a_rent`
/// pins the 2 (start 27, land 29); its comment's "roll 7 -> 5 -> 2 either way
/// round" is loose -- 5 → 2 is a *floor* half, and the card says 向上取整. Both
/// readings are defensible and they disagree by one step, so the composition
/// order needs a ruling. Filed as OPEN-ROLLRESHAPE-01.
fn roll_half(player_id: i32, v: RollValue) -> RollValue {
    let roll = v.value.max(0);
    let halved = (roll + 1) / 2;
    ctx::log(
        player_id,
        &Msg::new(key!("moca_half_roll"))
            .i("from", roll as i64)
            .i("to", halved as i64),
    );
    RollValue { value: halved, ..v }
}

/// 规则书(2): 「且你的所有资金支付与消耗减半」. The `PaySplitMul` pass (after
/// `PaySplitAdd`, before `PaySplitChoose`) rewrites the amount via the Filter fold.
///
/// User ruling 2026-10-15 (OPEN-取整 family): a 减半 modifier transforms the
/// running amount exactly and must NOT CeilTo in its own body; the engine
/// rounds up to a multiple of 10 once after all add/mul modifiers. Integer
/// fold rounds the .5 up (`(amount + 1) / 2`) so a sole modifier still settles
/// at `round10(exact)` (25 → 12.5 → 20).
fn pay_mul(player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    let half = ((value as i64 + 1) / 2).clamp(0, i32::MAX as i64) as i32;
    ctx::log(
        player_id,
        &Msg::new(key!("moca_half_pay"))
            .player_id("who", player_id)
            .n("from", value as i64)
            .n("to", half as i64),
    );
    half
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_end_kani(player_id: i32) -> card_sdk::Asked {
    turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn roll_half_kani(player_id: i32, v: card_sdk::RollValue) -> card_sdk::RollValue {
    roll_half(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_mul_kani(player_id: i32, value: i32) -> i32 {
    pay_mul(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
