//! `event:元祖！邦多利酱` -- 事件卡「元祖！邦多利酱」（中立事件 A17）.
//!
//! 事件文本（data/events.json, id `元祖！邦多利酱`）:
//! > 将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃牌。所有玩家的回合免费时间变为5秒，且回合恢复时间变为0秒。

use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep};

const ID: &str = "元祖！邦多利酱";

/// The player who drew the event, on the instance's props.
const DRAWER: &str = "drawer";
/// How many of the drawer's own turn starts have passed.
const TURNS: &str = "turns";

pub const BANGDREAM_CHAN: CardDef = CardDef::new(
    "event:元祖！邦多利酱",
    &[
        On::Play(None, play, ""),
        On::Hook(&[HookKind::TurnStartBefore], Some(always), on_turn_start, ""),
    ],
);

fn always(_player_id: i32) -> bool {
    true
}

/// 规则书: 「将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃牌」
/// -- keep the event and remember who drew it; the turn-start hook below counts
/// their turns and files it away on the third.
/// 规则书: 「所有玩家的回合免费时间变为5秒，且回合恢复时间变为0秒」
/// TODO(规则书)/TODO(engine): free time / recovery time are host-clock values
/// (`docs/ENGINE.md`, the `Match` host's turn clock), not rule state. No ctx
/// surface writes them, so the clock half of this event is left short -- only
/// the expiry below is real.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::set_prop(DRAWER, player_id);
    ctx::set_prop(TURNS, 0);
    ctx::log(
        player_id,
        &Msg::new("log.event.bangdream_on").player_id("who", player_id),
    );
    Ok(())
}

/// 规则书: 「抽到的玩家的第3回合开始时放入事件弃牌」 -- the drawer is always
/// mid-turn when the card is drawn, so their current turn is already the 1st;
/// the 3rd turn start is the **2nd** one after the draw.
fn on_turn_start(_player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() != ctx::prop(DRAWER) {
        return Ok(());
    }
    let n = ctx::prop(TURNS) + 1;
    ctx::set_prop(TURNS, n);
    // The draw happens mid-turn-1: turn starts 1 and 2 are the drawer's 2nd
    // and 3rd turns. 「第3回合开始时」 = `n >= 2` (the same moment 意外的对邦's
    // 「下2回合开始时」 names).
    if n >= 2 {
        expire(ID);
        ctx::log(
            trigger::player_id(),
            &Msg::new("log.event.bangdream_off").card("event", ID),
        );
    }
    Ok(())
}