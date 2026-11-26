//! `skill:千早爱音:重新开始`
//!
//! 规则书（skill sheet, 千早爱音）:
//! > [资源]：
//! > （1）[火罐]：初始2，上限3。
//! > [自动]：
//! > （1）自己每次[经过][CiRCLE]时：获得1个[火罐]。
//! > [主动]：
//! > （1）自己回合结束时自己的[停留]减少层数时：可使用2个[火罐]，自己[抽卡]1张。
//! > （2）自己[传送]后：可使用2个[火罐]，自己[抽卡]1张。
//!
//! [自动]（1） is `Pass` onto CiRCLE.
//!
//! The [主动] offer has two arms. （2） 「自己[传送]后」 is the *outcome* of a
//! `Teleport` effect, which is what `abnormal` is: the settlement hook, fired
//! after the effect landed; the trigger carries the kind in `abnormal_kind()`.
//!
//! （1） is the owner's own [停留] layer decrease at the owner's own turn end.
//! Natural wear-off is a bare negative state write -- no event of any kind
//! (未 `Abnormal`) -- so the hook rides `TurnEnd`, which fires right after the
//! wear-off tick. A 1 -> 0 layer is already invisible there, so a
//! `TurnEndBefore` memo records that a layer was there to fall off.

use bandori_std::ctx::{self, state};
use bandori_std::state_key;
use card_sdk::abi::{AbKind, ActionKind};
use card_sdk::{key, Action, CardDef, Msg, On};

/// Pre-tick memo ([主动]（1）): 1 = the owner had a [停留] layer before this
/// turn's wear-off. Written at `TurnEndBefore`, cleared by the offer body.
const STAY_DEC: &str = "skill.anonStay.dec";

pub const ANON: CardDef = CardDef::new(
    "skill:千早爱音:重新开始",
    &[
        On::Action(Action::TurnStartBefore("actor == owner", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        On::Action(Action::Pass(bandori_std::guards::OWN_START, None, on_pass)),
        // [主动]（2）「自己[传送]后，可以消耗2个火罐」 -- the offer exists only
        // for the `Teleport` kind and with two pots to spend. Kind is a
        // residual (`AbKind` match); fire is CEL.
        On::Action(Action::Abnormal(
            "actor == owner && charge(owner) >= 2",
            Some(on_teleport),
            on_teleport_outcome,
        )),
        // [主动]（1）「自己回合结束时自己的[停留]减少层数时」 -- memo the
        // pending decrease before the tick, then offer at `TurnEnd`.
        On::Action(Action::TurnEndBefore("actor == owner", None, mark_stay_dec)).silent(),
        On::Action(Action::TurnEnd(
            "",
            None,
            on_stay_tick,
        )),
    ],
)
.rule()
// 「（初始2，上限3）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 2),
    (bandori_std::fire::FIRE_CAP, 3),
]);

/// 「初始2，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// [自动]（1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("anon_gain")))?;
    Ok(())
}

/// Residual guard for [`on_teleport_outcome`]: the abnormal kind is [传送].
/// `AbKind` is not a CEL atom (the wire value lives in `value`).
fn on_teleport(_player_id: i32) -> bool {
    matches!(
        ctx::trigger::abnormal_kind(),
        Some(bandori_std::kind::abnormal::Teleport)
    )
}

/// [`mark_stay_dec`] the pre-tick [停留] layer count as a 0/1 memo.
fn mark_stay_dec(player_id: i32) -> card_sdk::Asked {
    let had = state::get(player_id, state_key::STAY) > 0;
    state::set(player_id, STAY_DEC, had as i32);
    Ok(())
}

/// [主动]（1）「自己的[停留]减少层数时」 -- clear the memo, then the same offer.
fn on_stay_tick(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, STAY_DEC, 0);
    offer(player_id)
}

/// [主动]（2）「自己[传送]后，可以消耗2个火罐，抽一张卡」. Applicability is
/// `charge(owner) >= 2` + the kind guard; spend after accept is a cost, not an
/// applicability check.
fn on_teleport_outcome(player_id: i32) -> card_sdk::Asked {
    offer(player_id)
}

/// The shared [主动] offer: ask, spend two pots, draw one.
fn offer(player_id: i32) -> card_sdk::Asked {
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("anon_title")),
        &Msg::new(key!("anon_ask")),
    )? {
        return Ok(());
    }
    if !bandori_std::fire::spend_skill(player_id, 2, &Msg::new(key!("anon_spend")))? {
        return Ok(());
    }
    ctx::draw(player_id, 1)?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_teleport_outcome_kani(player_id: i32) -> card_sdk::Asked {
    on_teleport_outcome(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_stay_tick_kani(player_id: i32) -> card_sdk::Asked {
    on_stay_tick(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn mark_stay_dec_kani(player_id: i32) -> card_sdk::Asked {
    mark_stay_dec(player_id)
}
