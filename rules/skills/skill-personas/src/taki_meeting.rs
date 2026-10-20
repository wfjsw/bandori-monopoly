//! `skill:椎名立希:决定练习日的会议`
//!
//! 规则书（skill sheet, 椎名立希）:
//! > （1）场上每有玩家获得一层[停留]时，你获得一个[火罐]（初始0，上限5）
//! > （2）运营阶段可消耗1个[火罐]为你场上的任何一张卡添加一个[奇迹水晶]，每回合限一次。
//!
//! （1） 「场上每有玩家」 is *anyone*, not just this player, so the `abnormal`
//! outcome hook fires for every recipient and the clause counts each layer.
//!
//! （2） 「为你场上的任何一张卡」 -- the target is chosen among this player's own
//! placed cards, and the crystal lands on that card (`Card.AddCrystals`), not
//! on the band or the player. 「每回合限一次」 is a latch, which is what the
//! keyed state is for.

use bandori_std::state_key; use card_sdk::abi::{counter, AbKind, ActionKind};
use bandori_std::ctx::{self, state, trigger};
use card_sdk::{Action, CardDef, key, Msg, On};

/// Latch for 「每回合限一次」.
const USED: &str = "skill.takiMeeting.used";

pub const TAKI_MEETING: CardDef = CardDef::new(
    "skill:椎名立希:决定练习日的会议",
    &[
        On::Play("", "", Some(can_use), use_skill),
        On::Action(Action::TurnStartBefore("", "", None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", "", None, declare_cap)),
        On::Action(Action::Abnormal("", "", None, on_abnormal)),
        On::Action(Action::TurnStartBefore(card_sdk::pre::MINE, "", None, reset)),
    ],
)
    .silent(&[1, 2, 4])
    // 「（初始0，上限5）」
    .props(&[(bandori_std::fire::FIRE_INITIAL, 0), (bandori_std::fire::FIRE_CAP, 5)]);

/// 「初始0，上限5」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

fn reset(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, USED, 0);
    Ok(())
}

/// （1）「场上每有玩家获得一层[停留]时，你获得一个[火罐]」 -- one pot per
/// layer the effect actually landed. The `abnormal` outcome hook fires for
/// every landing, self-applied included (「场上每有玩家」 is anyone).
fn on_abnormal(player_id: i32) -> card_sdk::Asked {
    if trigger::abnormal_kind() != Some(bandori_std::kind::abnormal::Stay) {
        return Ok(());
    }
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("taki_meeting_gain")))?;
    Ok(())
}

/// （2） 「运营阶段可消耗1个[火罐]」 plus 「每回合限一次」.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, USED) != 0 {
        return Some(Msg::new(key!("taki_meeting_used")));
    }
    if state::get(player_id, state_key::CHARGE) < 1 {
        return Some(Msg::new(key!("taki_meeting_no_fire")));
    }
    let _ = player_id;
    None
}

/// （2）「为你场上的任何一张卡添加一个[奇迹水晶]」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    // 「为你场上的任何一张卡添加一个[奇迹水晶]」 -- the pick is an *instance*,
    // so walk the field as `(uid, id)` and keep the uid rather than resolving
    // the name back afterwards (two copies would both resolve to the first).
    let field = ctx::field_instances(player_id);
    if field.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("taki_meeting_title")),
        &Msg::new(key!("taki_meeting_ask")),
        &field
            .iter()
            .map(|(_, c)| Msg::new(key!("taki_meeting_option")).card("card", c))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&(uid, ref card)) = field.get(pick) else {
        return Ok(());
    };
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("taki_meeting_spend")))? {
        return Ok(());
    }
    state::set(player_id, USED, 1);
    ctx::add_counter_at(uid, counter::CRYSTALS, 1, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("taki_meeting_added")).card("card", card),
    );
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_abnormal_kani(player_id: i32) -> card_sdk::Asked {
    on_abnormal(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn reset_kani(player_id: i32) -> card_sdk::Asked {
    reset(player_id)
}
