//! `skill:椎名立希:决定练习日的会议`
//!
//! 规则书（skill sheet, 椎名立希）:
//! > （1）场上每有玩家获得一层[停留]时，你获得一个[火罐]（初始0，上限5）
//! > （2）运营阶段可消耗1个[火罐]为你场上的任何一张卡添加一个[奇迹水晶]，每回合限一次。
//!
//! The library narrows （2）: 「自己的运营阶段且不拥有[立希]时：可使用1个[火罐]，
//! 为自己场上的任意1张卡拥有[奇迹水晶]资源的卡添加1个[奇迹水晶]并获得1层“回合
//! 结束时减少1层的”[立希]状态」. The target must already **hold** a [奇迹水晶]
//! resource, and the [立希] status is the once-a-turn latch.
//!
//! （1） 「场上每有玩家」 is *anyone*, not just this player, so the `abnormal`
//! outcome hook fires for every recipient and the clause counts each layer.

use bandori_std::counter;
use bandori_std::ctx::{self, state, trigger};
use bandori_std::state_key;
use card_sdk::abi::{AbKind, ActionKind};
use card_sdk::{key, Action, CardDef, Msg, On};

/// Latch for the 「[立希]」 status. 「每回合限一次」 / 「不拥有[立希]」 are the
/// same latch: while the status is up the press is refused; it wears one layer
/// at the owner's next turn start (「回合结束时减少1层」, the decrement lands on
/// the following turn start here).
const USED: &str = "skill.takiMeeting.used";

pub const TAKI: CardDef = CardDef::new(
    "skill:椎名立希:决定练习日的会议",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        On::Action(Action::Abnormal("", None, on_abnormal)),
        On::Action(Action::TurnStartBefore(
            card_sdk::condition::MINE,
            None,
            reset,
        ))
        .silent(),
    ],
)
.rule()
// 「（初始0，上限5）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 0),
    (bandori_std::fire::FIRE_CAP, 5),
]);

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
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("taki_gain")))?;
    Ok(())
}

/// （2） 「运营阶段可消耗1个[火罐]」 plus 「每回合限一次」.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, USED) != 0 {
        return Some(Msg::new(key!("taki_holding_riki")));
    }
    if bandori_std::fire::get(player_id) < 1 {
        return Some(Msg::new(key!("taki_no_fire")));
    }
    let _ = player_id;
    None
}

/// （2）「为自己场上的任意1张卡拥有[奇迹水晶]资源的卡添加一个[奇迹水晶]」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    // 「拥有[奇迹水晶]资源的卡」 -- only instances already carrying the
    // counter qualify (the add is on top of one, not a first crystal).
    // The pick is an *instance*, so walk the field as `(uid, id)` and keep the
    // uid rather than resolving the name back afterwards (two copies would
    // both resolve to the first).
    let field: alloc::vec::Vec<_> = ctx::field_instances(player_id)
        .into_iter()
        .filter(|(uid, _)| ctx::counter_at(*uid, counter::CRYSTALS) > 0)
        .collect();
    if field.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_choice(
        player_id,
        &Msg::new(key!("taki_title")),
        &Msg::new(key!("taki_ask")),
        &field
            .iter()
            .map(|(_, c)| Msg::new(key!("taki_option")).card("card", c.as_str()))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&(uid, card)) = field.get(pick) else {
        return Ok(());
    };
    if !bandori_std::fire::spend_skill(player_id, 1, &Msg::new(key!("taki_spend")))? {
        return Ok(());
    }
    state::set(player_id, USED, 1);
    ctx::add_counter_at(uid, counter::CRYSTALS, 1, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("taki_added")).card("card", card.as_str()),
    );
    Ok(())
}

//

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
