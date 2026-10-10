//! `event:Forbidden Moca` -- 事件卡「Forbidden Moca」（中立事件 A20）.
//!
//! 事件文本（data/events.json, id `Forbidden Moca`）:
//! > 直到你的下个回合结束，场上所有移动掷骰/2，在你的下个回合结束时永久移除此事件！

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire_removed, keep};

const ID: &str = "Forbidden Moca";

pub const FORBIDDEN_MOCA: CardDef = CardDef::new(
    "event:Forbidden Moca",
    &[
        On::Play("", None, play),
        // 「场上所有移动掷骰/2」 -- every face while the event is up. `always`
        // deleted (a no-op prefetch); the `move_roll` read in the body is the
        // face being halved (effect), not applicability.
        On::Hook(&[HookKind::RollAfter], "", None, halve),
        On::AtEnd("", None, expire),
    ],
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
/// halved (integer division, floor). Runs at `rollAfter`, the point at which
/// the face is known and a rewrite reaches the engine (`docs/CARDS.md`).
fn halve(_player_id: i32) -> card_sdk::Asked {
    let face = trigger::move_roll().unwrap_or_else(|| trigger::value());
    if face <= 0 {
        return Ok(());
    }
    trigger::set_move_roll(face / 2);
    Ok(())
}

/// 规则书: 「在你的下个回合结束时永久移除此事件！」
fn expire(_player_id: i32) -> card_sdk::Asked {
    expire_removed(ID);
    Ok(())
}