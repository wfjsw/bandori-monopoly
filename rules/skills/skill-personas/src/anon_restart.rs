//! `skill:千早爱音:重新开始`
//!
//! 规则书（skill sheet, 千早爱音）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限3）
//! > （2）在受到[停留][传送]效果影响并结算该效果后，可以消耗2个火罐，抽一张卡
//!
//! （1） is `Pass` onto CiRCLE.
//!
//! （2） 「受到[停留][传送]效果影响并结算该效果后」 is the *outcome* of an
//! abnormal effect, which is what `abnormal` is: the settlement hook, fired
//! after the effect landed. The two kinds the clause names are `Stay` and
//! `Teleport`; the trigger carries which one in `abnormal_kind()`.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{AbKind, ActionKind};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const ANON_RESTART: CardDef = CardDef::new(
    "skill:千早爱音:重新开始",
    &[
        On::Action(Action::TurnStartBefore("", None, None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, None, declare_cap)),
        On::Action(Action::Pass(
            bandori_std::guards::OWN_START,
            None,
            None,
            on_pass,
        )),
        // （2）「在受到[停留][传送]效果影响并结算该效果后，可以消耗2个火罐」 --
        // the offer exists only for those two abnormal kinds and with two
        // pots to spend. Kind is a residual (`AbKind` match); fire is CEL.
        On::Action(Action::Abnormal(
            "actor == owner && charge(owner) >= 2",
            None,
            Some(stay_or_teleport),
            on_abnormal,
        )),
    ],
)
.silent(&[0, 1])
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

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("anon_restart_gain")))?;
    Ok(())
}

/// Residual guard for [`on_abnormal`]: the abnormal kind is [停留] or [传送].
/// `AbKind` is not a CEL atom (the wire value lives in `value`).
fn stay_or_teleport(_player_id: i32) -> bool {
    matches!(
        trigger::abnormal_kind(),
        Some(bandori_std::kind::abnormal::Stay | bandori_std::kind::abnormal::Teleport)
    )
}

/// （2）「在受到[停留][传送]效果影响并结算该效果后，可以消耗2个火罐，抽一张卡」.
/// Applicability is `charge(owner) >= 2` + `stay_or_teleport`; spend after
/// accept is a cost, not an applicability check.
fn on_abnormal(player_id: i32) -> card_sdk::Asked {
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("anon_restart_title")),
        &Msg::new(key!("anon_restart_ask")),
    )? {
        return Ok(());
    }
    if !bandori_std::fire::spend(player_id, 2, &Msg::new(key!("anon_restart_spend")))? {
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
pub fn on_abnormal_kani(player_id: i32) -> card_sdk::Asked {
    on_abnormal(player_id)
}
