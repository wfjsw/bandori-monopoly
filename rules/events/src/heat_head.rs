//! `event:超燃甩头` -- 事件卡「超燃甩头」（中立事件 A18）.
//!
//! 事件文本（data/events.json, id `超燃甩头`）:
//! > 将此卡放置于场地中央，抽到的玩家的下回合结束前所有玩家的移动掷骰增加1d6，此后抽到的玩家的下回合结束前所有玩家的移动掷骰减少1d6，此后放入事件弃牌。

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, plan, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep, roll};

const ID: &str = "超燃甩头";

/// Phase counter on the instance's crystals: 1 = 「增加1d6」, 2 = 「减少1d6」.
const PHASE_UP: i32 = 1;
const PHASE_DOWN: i32 = 2;

pub const HEAT_HEAD: CardDef = CardDef::new(
    "event:超燃甩头",
    &[
        On::Play("", None, play),
        On::Hook(&[HookKind::RollPlan], "", Some(always), add_die),
        On::Hook(&[HookKind::RollAfter], "", Some(always), sub_die),
        On::AtEnd(phase_end),
    ],
);

fn always(_player_id: i32) -> bool {
    true
}

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
/// phase 1 is up.
fn add_die(_player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() == PHASE_UP {
        plan::add_extra_dice(1, 6, ID);
    }
    Ok(())
}

/// 规则书: 「此后…所有玩家的移动掷骰减少1d6」 -- phase 2 subtracts a d6 from
/// the face.
/// TODO(规则书): 「减少1d6」 as a subtraction from the face rather than a
/// `add_extra_dice(-1, 6)` term -- the dice pool has no negative-die op. The
/// sheet says 减少1d6 (a d6 less), not 少投1d6; if the intent is dropping a die
/// from a pool that has one, this body is short.
fn sub_die(player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() != PHASE_DOWN {
        return Ok(());
    }
    let face = trigger::move_roll().unwrap_or_else(|| trigger::value());
    if face <= 0 {
        return Ok(());
    }
    let cut = roll(player_id, 1, 6).min(face);
    trigger::set_move_roll(face - cut);
    Ok(())
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