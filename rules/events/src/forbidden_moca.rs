//! `event:Forbidden Moca` -- 事件卡「Forbidden Moca」（中立事件 A20）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `Forbidden Moca`）:
//! > 直到你的下个回合结束，场上所有移动掷骰/2，在你的下个回合结束时永久移除此事件！

use bandori_std::ctx;
use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::{CardText};
use card_sdk::{Action, CardDef, Filter, Msg, On, RollValue};

use crate::util::{expire_removed, keep};

const ID: &str = "Forbidden Moca";

pub const FORBIDDEN_MOCA: CardDef = CardDef::new(
    "event:Forbidden Moca",
    &[
        On::Play("", None, None, play),
        // 「场上所有移动掷骰/2」 -- every face while the event is up. `always`
        // deleted (a no-op prefetch); the `value` param is the face being
        // halved (effect), not applicability.
        On::Filter(Filter::DiceRollValue("", None, halve)),
        On::Action(Action::AtEnd("", None, None, expire)),
    ],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"Forbidden Moca"#)
        .text(CardText::para(r#"直到你的下个回合结束，场上所有移动掷骰/2，在你的下个回合结束时永久移除此事件！"#))
        // sheet cell A20
        .derived(false),
);

/// 规则书: 「直到你的下个回合结束…」 -- the event stays in play (「你的」 is the
/// player who drew it) and its expiry is scheduled on that player's next turn
/// end. 「永久移除此事件！」 is the `AtEnd` body below.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::at_next_turn_end(player_id);
    ctx::log(
        player_id,
        &Msg::new("log.event.moca_on").player_id("who", player_id),
    );
    Ok(())
}

/// 规则书: 「场上所有移动掷骰/2」 -- every move roll while the event is up is
/// halved (integer division, floor). `DiceRollValue` is the face filter raised
/// immediately before `DiceRollAfter` (`docs/CARDS.md`); the return is the new
/// figure and the host folds it via `set_move_roll`.
fn halve(_player_id: i32, v: RollValue) -> RollValue {
    let value = if v.value <= 0 { v.value } else { v.value / 2 };
    RollValue { value, ..v }
}

/// 规则书: 「在你的下个回合结束时永久移除此事件！」
fn expire(_player_id: i32) -> card_sdk::Asked {
    expire_removed(ID);
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn halve_kani(player_id: i32, v: card_sdk::RollValue) -> card_sdk::RollValue {
    halve(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn expire_kani(player_id: i32) -> card_sdk::Asked {
    expire(player_id)
}
