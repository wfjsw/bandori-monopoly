//! `通用:网络链接异常` -- C# `CardNetError` (MatchHost.cs:2258-2303): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `通用:网络链接异常`）:
//! > 网络链接异常：
//! > [手]：
//! > [反击]1张手卡的[手]效果或事件卡的效果生效前：根据卡和效果的类型依次进行以下操作：
//! > 1. 手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]；
//! > 2. 手卡的[手]效果且没有[指定]目标则抵消其所有的效果；
//! > 3. 事件卡的效果手牌则则抵消其所有的效果。
//!
//! another player's [手] effect or an event before it lands.

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const NET_ERROR: CardDef = CardDef::new(
    "通用:网络链接异常",
    &[On::Counteract(
        &[ChainKind::Effect, ChainKind::Card, ChainKind::Event],
        can_counteract,
        counteract,
    )],
);

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「[反击]1张手卡的[手]效果或事件卡的效果生效前」
    // C# `CardNetError.CanCounteract`: kind=="event", or kind=="card" with a live play
    // by someone else.
    match trigger::kind() {
        TriggerKind::Event => true,
        TriggerKind::Card => {
            // C# `t.Play != null && !t.Play.Cancelled && t.Seat != seat`.
            trigger::player_id() != player_id
            // TODO(规则书)[judgement](ABI): the trigger carries no PlayCtx, so `t.Play == null` and
            //   the clause under-specifies -- see the note above it
            // `t.Play.Cancelled` cannot be checked (C# `CardNetError.CanCounteract`).
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

fn counteract(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        TriggerKind::Event => {
            // 规则书（3）[反击]: 「事件卡的效果手牌则则抵消其所有的效果」
            // C# `trigger.Cancelled = true` then
            // 「网络链接异常：事件「…」的效果被抵消」.
            trigger::set_cancelled();
            ctx::log(
                player_id,
                &Msg::new(key!("net_error_event")).player_id("who", player_id),
            );
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
            // The two branches are decided by whether the play *names* anyone:
            // `H.Db.Card(id).Targeting` is the effect list's own recipients, which
            // is `ctx::effect::count()` / `effect::target(i)`.
            let designations = ctx::designations(trigger::player_id());
            if !designations.is_empty() {
                // （1）「取消其对目标之一的[指定]」 -- per-pair cancel (C#
                // `play.Tags["immune"+seat]=1`): cancel the counteractor's own
                // designation when it has one, else the first named seat. The
                // rest of the play's designations still land.
                let seat = designations
                    .iter()
                    .copied()
                    .find(|&s| s == player_id)
                    .unwrap_or(designations[0]);
                ctx::cancel_designation(seat);
                ctx::log(
                    player_id,
                    &Msg::new(key!("net_error_target")).player_id("who", seat),
                );
            } else {
                // （2）「没有[指定]目标则抵消其所有的效果」
                trigger::set_cancelled();
                ctx::log(
                    player_id,
                    &Msg::new(key!("net_error_card")).player_id("who", player_id),
                );
            }
        }
        TriggerKind::Target => {
            // 规则书（1）[反击]: 「取消其对目标之一的[指定]」 -- the expressible half:
            // cancel the designation of this player at its `target` window
            // (`ctx::target` then answers `None`). The C# instead pre-tags the
            // play at the `card` window so `H.Target` never reaches its window
            // for this player -- that per-play `immune<p>` tag stays TODO'd above.
            trigger::set_cancelled();
            ctx::log(
                player_id,
                &Msg::new(key!("net_error_target")).player_id("who", player_id),
            );
        }
        _ => {}
    }
    Ok(())
}
