//! `event:超燃甩头` -- 事件卡「超燃甩头」（中立事件 A18）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `超燃甩头`）:
//! > 将此卡放置于场地中央，抽到的玩家的下回合结束前所有玩家的移动掷骰增加1d6，此后抽到的玩家的下回合结束前所有玩家的移动掷骰减少1d6，此后放入事件弃牌。

use card_sdk::abi::{FilterValue, ActionKind, FilterKind};
use ruleset_core::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep, roll};

const ID: &str = "超燃甩头";

/// Phase counter on the instance's crystals: 1 = 「增加1d6」, 2 = 「减少1d6」.
const PHASE_UP: i32 = 1;
const PHASE_DOWN: i32 = 2;

pub const HEAT_HEAD: CardDef = CardDef::new(
    "event:超燃甩头",
    &[
        On::Play("", "", None, play),
        // 「所有玩家的移动掷骰」 -- every plan / every face. `always` deleted
        // (a no-op prefetch). The phase split lives in the bodies: the crystal
        // counter is the event's own phase state (`card.cp` is not a reliable
        // read on a BOARD_OWNER event instance).
        On::FilterValue(&[FilterKind::DiceRollPlan], "", None, add_die),
        // v79: `sub_die` is a pure DiceRollValue Filter using the host-supplied
        // `recorded_roll` (a peeked 1d6 from the RNG stream clone).
        On::FilterValue(&[FilterKind::DiceRollValue], "", None, sub_die_filter),
        On::Action(&[ActionKind::AtEnd], "", "", None, phase_end),
    ],
)
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"超燃甩头"#)
            .text(r#"将此卡放置于场地中央，抽到的玩家的下回合结束前所有玩家的移动掷骰增加1d6，此后抽到的玩家的下回合结束前所有玩家的移动掷骰减少1d6，此后放入事件弃牌。"#)
            .cell(r#"A18"#)
            .derived(false));

/// 规则书: 「将此卡放置于场地中央，抽到的玩家的下回合结束前所有玩家的移动
/// 掷骰增加1d6」 -- keep the event, start phase 1, schedule the drawer's next
/// turn end.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::set_crystals(PHASE_UP);
    ctx::at_next_turn_end(player_id);
    ctx::log(
        player_id,
        &Msg::new("log.event.heat_up").player_id("who", player_id),
    );
    Ok(())
}

/// 规则书: 「所有玩家的移动掷骰增加1d6」 -- the dice pool gains a d6 while
/// phase 1 is up. Pure plan-dice reshape on `DiceRollPlan`.
fn add_die(_player_id: i32, mut v: FilterValue) -> FilterValue {
    if ctx::crystals() == PHASE_UP {
        v.extra_count = 1;
        v.extra_sides = 6;
    }
    v
}

/// 规则书: 「此后…所有玩家的移动掷骰减少1d6」 -- phase 2 subtracts a d6 from
/// the face.
/// TODO(规则书): 「减少1d6」 as a subtraction from the face rather than a
/// `add_extra_dice(-1, 6)` term -- the dice pool has no negative-die op. The
/// sheet says 减少1d6 (a d6 less), not 少投1d6; if the intent is dropping a die
/// from a pool that has one, this body is short.
// Body is a pure DiceRollValue FilterValue (v79); the old Action-at-
// DiceRollAfter shape is gone. See docs/CARDS.md for the one documented
// exception that still allows an Action at DiceRollAfter to write the face.
fn sub_die_filter(_player_id: i32, v: FilterValue) -> FilterValue {
    if ctx::crystals() != PHASE_DOWN {
        return v;
    }
    let face = v.value;
    if face <= 0 {
        return v;
    }
    let cut = v.recorded_roll;
    if cut == FilterValue::UNCHANGED {
        // Host did not pre-roll; leave the face unchanged.
        return v;
    }
    let cut = cut.min(face).max(0);
    FilterValue {
        value: face - cut,
        ..v
    }
}

/// 规则书: 「抽到的玩家的下回合结束前…增加1d6，此后抽到的玩家的下回合结束前…
/// 减少1d6，此后放入事件弃牌」 -- the first turn end flips to phase 2 and
/// schedules the second; the second discards the event.
fn phase_end(player_id: i32) -> card_sdk::Asked {
    let phase = ctx::crystals();
    if phase == PHASE_UP {
        ctx::set_crystals(PHASE_DOWN);
        // 「此后抽到的玩家的下回合结束前」 -- the turn after this one. This body
        // runs *at* a turn end, so the next occurrence of that player's turn end
        // is 「下回合结束时」: `at_turn_end` (not `at_next_turn_end`, which would
        // also skip the next one and land a turn late).
        ctx::at_turn_end(player_id);
        ctx::log(
            player_id,
            &Msg::new("log.event.heat_down").player_id("who", player_id),
        );
    } else {
        expire(ID);
        ctx::log(player_id, &Msg::new("log.event.heat_off"));
    }
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn add_die_kani(player_id: i32, v: FilterValue) -> FilterValue {
    add_die(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn sub_die_filter_kani(player_id: i32, v: FilterValue) -> FilterValue {
    sub_die_filter(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn phase_end_kani(player_id: i32) -> card_sdk::Asked {
    phase_end(player_id)
}
