//! `AG:（摩卡）0.5倍速` -- C# `CardMocaHalf` (MatchHost.cs:1161-1197):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（摩卡）0.5倍速`）:
//! > （摩卡）0.5倍速：
//! > (1)  将此卡放置在场上并获得3个奇迹水晶，你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆。
//! > (2) 此卡在场时，你的移动掷骰的最终结算/2（向上取整）且你的所有资金支付与消耗减半。
//!
//! placed card, 3 crystals decaying each own turn end; halve rolls and payments.
//!
//! Reviewed 10/24/2026

use bandori_std::ctx::{self, trigger};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Filter, Msg, On};

const ID: &str = "AG:（摩卡）0.5倍速";

/// 规则书(1): 「奇迹水晶为0时此卡放入弃牌堆」
///
/// Listens to this card's own [`ActionKind::CounterChanged`] (name-filtered to
/// `counter::CRYSTALS`) rather than being re-checked at the decay tick, so a
/// count emptied by *any* write leaves the field just the same.
///
/// Migrated to CEL: the former Rust residual guard (`trigger::card_is(ID) &&
/// trigger::name() == bandori_std::counter::CRYSTALS`) is folded into this
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

        // 规则书(1): 「你的回合结束时移除一个奇迹水晶」 -- the owner's turn end
        // only: the field hook walk visits every seat's field, so gate on actor.
        On::Action(Action::TurnEnd("actor == owner && card.placed", None, turn_end)),

        // 规则书(1): 「奇迹水晶为0时此卡放入弃牌堆」 -- a printed discard, so the
        // entry flashes like every sibling crystal-decay discard (the body logs).
        On::Action(Action::CounterChanged(
            CRYSTALS_CHANGED_PRE,
            None,
            on_crystals_changed,
        )),

        // > (2) 此卡在场时，你的移动掷骰的最终结算/2（向上取整）
        // TODO: Check 最终结算
        On::Action(Action::DiceRollAfter("actor == owner && move.main && card.placed", None, roll_half)),

        // 且你的所有资金支付与消耗减半。
        // Note: Mul happens after Add
        On::Filter(Filter::PaySplitMul("actor == owner && card.placed && value > 0", None, pay_mul)),
    ],
)
// 规则书(1) 「3个奇迹水晶」: the `⌛3` marker the client shows.
.props(&[(bandori_std::marker::TURNS_LEFT, 3)])
.meta(&card_sdk::meta::CardMeta::new()
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
    // 规则书(1): 「将此卡放置在场上」 -- the place-now + fate-later fold
    // (`place_card` creates the instance, `Dest::Field` stops the commit from
    // also spending the hand card); see `ctx::place_self_to_field`.
    ctx::place_self_to_field(player_id, ID, &Msg::new(key!("moca_half_note")));
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
    // 「你的回合结束时」 -- the tick.
    ctx::dec_turns_left();
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
/// main-move roll. An `Action::DiceRollAfter` body rewriting the face
/// (`trigger::set_move_roll`) -- `DiceRollAfter` is the event, `DiceRollValue`
/// the pure reshape stage.
///
/// TODO(规则书) composition with another roll reshape. 「最终结算」 reads as
/// *after* additive modifiers (`docs/rulebook/spec/cards/ag.md`
/// `C-AG-half-speed-03`): against `PP:不要背负期待` 「投掷结果减少2」 that is
/// 7 → 5 → ceil(5/2) = **3**. The `DiceRollAfter` stage gives that order for
/// `DiceRollValue` reshapes regardless of placement
/// (`g02_half_speed_plus_expectations_floor_on_a_rent` pins 3); **other**
/// `DiceRollAfter` bodies still fold in placement order, and the sheet does not
/// order equal-stage effects. Filed as OPEN-ROLLRESHAPE-01.
fn roll_half(player_id: i32) -> card_sdk::Asked {
    let roll = trigger::value().max(0);

    // TODO: Replace to .div_ceil as soon as https://github.com/rust-lang/rust/issues/88581
    let halved = roll / 2 + roll % 2;

    ctx::log(
        player_id,
        &Msg::new(key!("moca_half_roll"))
            .i("from", roll as i64)
            .i("to", halved as i64),
    );

    // 规则书(2): 「移动掷骰的最终结算/2（向上取整）」 -- rewrite the move's face.
    trigger::set_move_roll(halved);
    Ok(())
}

/// 规则书(2): 「且你的所有资金支付与消耗减半」. The `PaySplitMul` pass (after
/// `PaySplitAdd`, before `PaySplitChoose`) rewrites the amount via the Filter fold.
///
/// User ruling 2026-10-15 (OPEN-取整 family): a 减半 modifier transforms the
/// running amount exactly and must NOT CeilTo in its own body; the engine
/// rounds up to a multiple of 10 once after all add/mul modifiers. Integer
/// fold rounds the .5 up (`value.div_ceil(2)`) so a sole modifier still settles
/// at `round10(exact)` (25 → 12.5 → 20).
fn pay_mul(player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    // `value > 0`, so ceil-halving is `value / 2 + value % 2` (stable; avoids
    // the unstable `int_roundings` feature behind `div_ceil`, and unlike
    // `(value + 1) / 2` it cannot overflow at `i32::MAX`).
    // TODO: Replace to .div_ceil as soon as https://github.com/rust-lang/rust/issues/88581
    let half = value / 2 + value % 2;
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
pub fn roll_half_kani(player_id: i32) -> card_sdk::Asked {
    roll_half(player_id)
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
