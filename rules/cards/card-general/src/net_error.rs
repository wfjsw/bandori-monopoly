//! `通用:网络链接异常` -- C# `CardNetError` (MatchHost.cs:2258-2303): [反击]
//! another player's [手] effect or an event before it lands.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:网络链接异常`）:
//! > 网络链接异常：
//! > [手]：
//! > [反击]1张手卡的[手]效果或事件卡的效果生效前：根据卡和效果的类型依次进行以下操作：
//! > 1. 手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]；
//! > 2. 手卡的[手]效果且没有[指定]目标则抵消其所有的效果；
//! > 3. 事件卡的效果手牌则则抵消其所有的效果。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const NET_ERROR: CardDef = CardDef {
    id: "通用:网络链接异常",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「[反击]1张手卡的[手]效果或事件卡的效果生效前」
    // C# `CardNetError.CanReact`: kind=="event", or kind=="card" with a live play
    // by someone else.
    match trigger::kind() {
        TriggerKind::Event => true,
        TriggerKind::Card => {
            // C# `t.Play != null && !t.Play.Cancelled && t.Seat != seat`.
            trigger::seat() != seat
            // TODO(ABI): the trigger carries no PlayCtx, so `t.Play == null` and
            // `t.Play.Cancelled` cannot be checked (C# `CardNetError.CanReact`).
        }
        _ => false,
    }
}

fn react(seat: i32) {
    match trigger::kind() {
        TriggerKind::Event => {
            // 规则书（3）[反击]: 「事件卡的效果手牌则则抵消其所有的效果」
            // C# `trigger.Cancelled = true` then
            // 「网络链接异常：事件「…」的效果被抵消」.
            ctx::log(seat, &Msg::new(key!("net_error_event")).seat("who", seat));
            // TODO(ABI): 「抵消其所有的效果」 -- needs a trigger-cancel hook
            // (C# `trigger.Cancelled = true`); until then the event still resolves.
            // The C# log also names the event (`H.EventTitle(t.Card)`); the trigger
            // carries no card id.
        }
        TriggerKind::Card => {
            // 规则书（1）[反击]: 「手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]」
            // 规则书（2）[反击]: 「手卡的[手]效果且没有[指定]目标则抵消其所有的效果」
            // C# branches on `t.Play.Def.Targeting`: set `play.Tags["immune"+seat]=1`
            // (drop this seat from the designation) vs `play.Cancelled = true`.
            ctx::log(seat, &Msg::new(key!("net_error_card")).seat("who", seat));
            // TODO(ABI): 「取消其对目标之一的[指定]」 / 「抵消其所有的效果」 -- needs
            // the played card's `Targeting` flag plus play-cancel / play-tag hooks
            // (C# `play.Tags["immune"+c.Seat] = 1` / `play.Cancelled = true`). The
            // trigger also carries no card id for the log line.
        }
        _ => {}
    }
}