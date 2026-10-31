//!
//! 规则书（docs/rulebook/cards.json, id `HHW:运动的天赋`）:
//! > 运动的天赋：
//! >  将此卡放置于自己场上并放置3个奇迹水晶，每回合结束时失去一个，为0时置入弃牌堆。此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100。每次移动掷骰时，重骰移动掷骰直至结果至少为10为止。
//!

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Filter, Msg, On, PlanValue};

const ID: &str = "HHW:运动的天赋";

pub const SPORTS_TALENT: CardDef = CardDef::new(
    "HHW:运动的天赋",
    &[
        On::Play("", None, play),
        // 规则书: 「每回合结束时失去一个」 -- every turn end, not only the
        // owner's (the cell does not say 你的; same for every peer decay card).
        On::Action(Action::TurnEnd("card.placed", None, at_turn_end)),
        // TODO(规则书)（每次掷骰获得一次资金）: The fixed-roll check is residual. 每次掷骰都该给钱 -- 我们只认
        // 掷骰者的主要移动掷骰；效果掷骰根本不发 diceRollAfter，非主要移动也 不给.
        On::Action(Action::DiceRollAfter("actor == owner && card.placed && move.is_main_move", Some(roll_after_applies), on_roll_after)),
        // TODO(规则书)（每次移动掷骰时）: `plan.fixed_roll` carries the "no fixed face" half now;
        // `actor` is the mover, same target as the `DiceRollAfter` entry.
        // 规则书没写「你的」，每次移动掷骰都该 重骰 -- 我们只认掷骰者是此卡主人的主要移动掷骰.
        On::Filter(Filter::DiceRollPlan("card.placed && actor == owner && move.is_main_move && plan.fixed_roll == null", None, roll_plan)),
        On::Action(Action::CounterChanged("", Some(crystals_changed_guard), on_crystals_changed)),
    ],
)
// 规则书「3个奇迹水晶」: the `⌛3` marker the client shows.
.props(&[(bandori_std::marker::TURNS_LEFT, 3)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"运动的天赋"#)
        .raw_name(r#"运动的天赋"#)
        .tags(&[])
        .text(CardText::para(r#"将此卡放置于自己场上并放置3个奇迹水晶，每回合结束时失去一个，为0时置入弃牌堆。此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100。每次移动掷骰时，重骰移动掷骰直至结果至少为10为止。"#))
        // sheet cell F9
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

/// The player slot stands in for the per-field-card `Mem` map, so it outlives the
/// instance: [`play`] resets it to 700 and two copies on one field share one
/// ladder.
const SLOT_NEXT: &str = "sports_talent_next";

fn play(player_id: i32) -> card_sdk::Asked {
    ctx::set_fate(ctx::Fate::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("sports_talent_note")));
    ctx::set_crystals(3);
    ctx::log(
        player_id,
        &Msg::new(key!("sports_talent_placed")).player_id("who", player_id),
    );
    // 规则书: 「起始为700」 -- the ladder starts over for a fresh instance (the
    // slot outlives the card, so without this a replayed copy resumes the old
    // step).
    ctx::set_slot(player_id, SLOT_NEXT, 700);
    // (the turn-end decay and the dice-roll reward run in
    // [`at_turn_end`] / [`on_roll_after`], which the hook dispatch runs at
    // `turnEnd` / `rollAfter`.)
    Ok(())
}

/// `DecayOn` pinned it to the owner's turn; the cell does not say 你的 and every
/// peer decay card ticks at every turn end, so `card.placed` alone is the condition.
/// `R:Fire bird` reads the same phrase for every seat's turn end too. The
/// 「为0时置入弃牌堆」 half is [`on_crystals_changed`].
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    // 「每回合结束时失去一个」 -- the tick.
    ctx::dec_turns_left();
    ctx::remove_crystal()?;
    Ok(())
}

fn roll_after_applies(_player_id: i32) -> bool {
    trigger::move_roll().is_some() && ctx::fixed_roll().is_none()
}

/// `actor == owner && card.placed && move.is_main_move` is the condition;
/// [`roll_after_applies`] is the residual.
fn on_roll_after(player_id: i32) -> card_sdk::Asked {
    reward(player_id)?;
    // 规则书: 「每次移动掷骰时，重骰移动掷骰直至结果至少为10为止」
    reroll_to_ten(player_id)?;
    Ok(())
}

fn reward(player_id: i32) -> card_sdk::Asked {
    let mut next = ctx::slot(player_id, SLOT_NEXT);
    if next <= 0 {
        next = 700;
    }
    ctx::set_slot(player_id, SLOT_NEXT, (next - 100).max(100));
    ctx::gain(
        player_id,
        next,
        &Msg::new(key!("sports_talent_reward")).n("money", next as i64),
    )?;
    Ok(())
}

fn roll_plan(_player_id: i32, v: PlanValue) -> PlanValue {
    PlanValue {
        min_roll: Some(10),
        ..v
    }
}

/// Called from [`on_roll_after`].
fn reroll_to_ten(player_id: i32) -> card_sdk::Asked {
    let mut x = trigger::move_roll().unwrap_or(trigger::value());
    loop {
        if x >= 10 || ctx::player_out(player_id) {
            break;
        }
        x = ctx::do_move_roll(player_id)?.max(0);
        trigger::set_move_roll(x);
        // 规则书: 「每次掷骰获得一次资金」 -- a re-roll is another 掷骰 and pays
        // again (the ladder steps down each time).
        reward(player_id)?;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("sports_talent_reroll"))
            .player_id("who", player_id)
            .i("n", x as i64),
    );
    Ok(())
}

///
/// Listens to this card's own [`ActionKind::CounterChanged`] (name-filtered to
/// `counter::CRYSTALS`) rather than being re-checked at the decay tick, so a
/// count emptied by *any* write leaves the field just the same.
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all.
fn crystals_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && trigger::name() == bandori_std::counter::CRYSTALS
        && ctx::crystals() == 0
        // Only a write that did not raise the count speaks for the empty
        // state; see AG:绯红之魂 (3).
        && trigger::value() <= 0
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_fate(ctx::Fate::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("sports_talent_decayed")).player_id("who", player_id),
    );
    Ok(())
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
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_roll_after_kani(player_id: i32) -> card_sdk::Asked {
    on_roll_after(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn roll_plan_kani(player_id: i32, v: card_sdk::PlanValue) -> card_sdk::PlanValue {
    roll_plan(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
