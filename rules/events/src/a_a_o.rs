//! `event:A！A！O！` -- 事件卡「A！A！O！」（中立事件 A15）.
//!
//! 事件文本（data/events.json, id `A！A！O！`）:
//! > 将此卡放置于场地中央，抽到的玩家的下回合结束时放入事件弃牌。此卡在场时全场玩家不能使用卡牌的[手]效果。

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep};

const ID: &str = "A！A！O！";

/// The player who drew the event, on the instance's props.
const DRAWER: &str = "drawer";

pub const A_A_O: CardDef = CardDef::new(
    "event:A！A！O！",
    &[
        On::Play(None, play, ""),
        On::AtEnd(expire_at_end),
    ],
);

/// 规则书: 「将此卡放置于场地中央，抽到的玩家的下回合结束时放入事件弃牌」
/// -- keep the event and schedule the drawer's next turn end (`AtEnd`).
/// 规则书: 「此卡在场时全场玩家不能使用卡牌的[手]效果」 -- a global hand-effect
/// veto over every player while the event is up.
/// TODO(规则书)/TODO(engine): that veto has no ctx surface. The C#
/// `CannotPlay` / `Fx.CantPlayHand` gate has no engine field yet
/// (`crates/game-rules/src/wasm_rules.rs`: 「`_noCounteractTurn` and
/// Fx.CantPlayHand have no engine field yet」; `docs/EVENTS.md` 「Left short」).
/// Nothing is written to stand in for it -- no hook both sees every hand [手]
/// effect and can refuse it -- so the clause is left short rather than faked.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::set_prop(DRAWER, player_id);
    ctx::at_next_turn_end(player_id);
    ctx::log(
        player_id,
        &Msg::new("log.event.a_a_o_on").player_id("who", player_id),
    );
    Ok(())
}

/// 规则书: 「抽到的玩家的下回合结束时放入事件弃牌」.
fn expire_at_end(_player_id: i32) -> card_sdk::Asked {
    expire(ID);
    Ok(())
}