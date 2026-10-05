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

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const NET_ERROR: CardDef = CardDef::new("通用:网络链接异常", &[
    On::CounterAct(&[ChainKind::Effect, ChainKind::Card, ChainKind::Event],
        can_react,
        react,
    ),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「[反击]1张手卡的[手]效果或事件卡的效果生效前」
    // C# `CardNetError.CanReact`: kind=="event", or kind=="card" with a live play
    // by someone else.
    match trigger::kind() {
        TriggerKind::Event => true,
        TriggerKind::Card => {
            // C# `t.Play != null && !t.Play.Cancelled && t.Seat != seat`.
            trigger::player_id() != player_id
            // TODO(规则书)[judgement](ABI): the trigger carries no PlayCtx, so `t.Play == null` and
            //   the clause under-specifies -- see the note above it
            // `t.Play.Cancelled` cannot be checked (C# `CardNetError.CanReact`).
        }
        // 规则书（1）的可表达半边: a player-designation of this player (C# `H.Target`,
        // not `H.TargetTile` -- the `immune<p>` tag only guards `H.Target`).
        TriggerKind::Effect => {
            ctx::effect::has(TriggerKind::Target)
                && trigger::tile() < 0
                && trigger::target() == player_id
                && trigger::by_card().is_some_and(|by| by != player_id)
        }
        _ => false,
    }
}

fn react(player_id: i32) {
    match trigger::kind() {
        TriggerKind::Event => {
            // 规则书（3）[反击]: 「事件卡的效果手牌则则抵消其所有的效果」
            // C# `trigger.Cancelled = true` then
            // 「网络链接异常：事件「…」的效果被抵消」.
            trigger::set_cancelled();
            ctx::log(player_id, &Msg::new(key!("net_error_event")).player_id("who", player_id));
            // TODO(规则书)[judgement](ABI): the C# log also names the event (`H.EventTitle(t.Card)`);
            //   the clause under-specifies -- see the note above it
            // reading the trigger's card id back needs a host->guest string.
        }
        TriggerKind::Card => {
            // 规则书（1）[反击]: 「手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]」
            // -- the per-play half, PARTIAL: C# sets `play.Tags["immune"+seat]=1`
            // so every later `H.Target(me)` on this play fails silently. Those
            // tags are not ported (see below); the per-designation cancel that
            // IS expressible runs at the `target` window instead.
            // 规则书（2）[反击]: 「手卡的[手]效果且没有[指定]目标则抵消其所有的效果」
            // C# branches on `t.Play.Def.Targeting`: the `immune<p>` tag above vs
            // `play.Cancelled = true`.
            ctx::log(player_id, &Msg::new(key!("net_error_card")).player_id("who", player_id));
            // TODO(ABI): the two branches need the played card's `Targeting` flag
            // on the trigger (and the `play.Tags["immune"+seat]` op for (1)).
            // Rule (2)'s 「抵消其所有的效果」 is `trigger::set_cancelled()` on this
            // `card` trigger -- ready, but it must not fire while the branch is
            // undecidable or it would over-cancel targeting plays.
        }
        TriggerKind::Target => {
            // 规则书（1）[反击]: 「取消其对目标之一的[指定]」 -- the expressible half:
            // cancel the designation of this player at its `target` window
            // (`ctx::target` then answers `None`). The C# instead pre-tags the
            // play at the `card` window so `H.Target` never reaches its window
            // for this player -- that per-play `immune<p>` tag stays TODO'd above.
            trigger::set_cancelled();
            ctx::log(player_id, &Msg::new(key!("net_error_target")).player_id("who", player_id));
        }
        _ => {}
    }
}