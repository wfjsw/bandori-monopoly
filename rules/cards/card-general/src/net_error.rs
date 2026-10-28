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

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::{key, CardDef, Msg, On};
use card_sdk::{para, sec, steps, CardText};

pub const NET_ERROR: CardDef = CardDef::new(
    "通用:网络链接异常",
    &[On::Counteract(
        &[ChainKind::Effect, ChainKind::Card, ChainKind::Event],
        "",
        Some(can_counteract),
        counteract).counter_reason(key!("net_error_when"))],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"网络链接异常"#)
        .raw_name(r#"网络链接异常"#)
        .tags(&[r#"手"#, r#"反击"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"[反击]1张手卡的[手]效果或事件卡的效果生效前：根据卡和效果的类型依次进行以下操作："#),
                steps(&[
                    r#"手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]；"#,
                    r#"手卡的[手]效果且没有[指定]目标则抵消其所有的效果；"#,
                    r#"事件卡的效果手牌则则抵消其所有的效果。"#,
                ]),
            ]),
        ]))
        // sheet cell A10
        .fields(&[("group", r#"通用"#), ("owner", "")]),
);

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「[反击]1张手卡的[手]效果或事件卡的效果生效前」
    // C# `CardNetError.CanCounteract`: kind=="event", or kind=="card" with a live play
    // by someone else.
    match trigger::kind() {
        TriggerKind::Event => true,
        TriggerKind::Card => {
            // C# `t.Play != null && !t.Play.Cancelled && t.Seat != seat`.
            !trigger::cancelled() && trigger::player_id() != player_id && {
                // Clause 1 has nothing left to cancel once every designation
                // is already dropped; another copy would be spent for nothing.
                let designations = ctx::designations(trigger::player_id());
                designations.is_empty()
                    || designations.iter().any(|&s| !ctx::designation_cancelled(s))
            }
            // TODO(规则书)[judgement]: 「1张手卡的[手]效果」 does not restrict the
            //   play to another player's -- we refuse the play's own seat its own
            //   [反击] (OPEN-自我反击).
            // TODO(规则书)[judgement](ABI): the trigger carries no PlayCtx, so
            //   `t.Play == null` cannot be checked -- the clause under-specifies.
        }
        // 规则书（1）[反击]: 「手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]」
        // -- the expressible half: a player-designation of this player (C# `H.Target`,
        // not `H.TargetTile` -- the `immune<p>` tag only guards `H.Target`).
        TriggerKind::Effect => {
            // 规则书（1）[反击]: 「手卡的[手]效果且有[指定]目标」 -- a hand card's
            // effect only: a placed card's press or a skill body is not 「手卡」.
            ctx::play_from_hand()
                && ctx::effect::has(TriggerKind::Target)
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
            // -- C# sets `play.Tags["immune"+seat]=1`, which is our
            // `ctx::cancel_designation(seat)`: the play's later `H.Target(me)`
            // fails silently while the rest of its designations still land.
            // 规则书（2）[反击]: 「手卡的[手]效果且没有[指定]目标则抵消其所有的效果」
            // C# branches on `t.Play.Def.Targeting`: the `immune<p>` tag above vs
            // `play.Cancelled = true`.
            // The two branches are decided by whether the play *names* anyone
            // (`ctx::designations`, the static targeting list).
            let designations = ctx::designations(trigger::player_id());
            if designations.is_empty() {
                // （2）「没有[指定]目标则抵消其所有的效果」
                trigger::set_cancelled();
                ctx::log(
                    player_id,
                    &Msg::new(key!("net_error_card")).player_id("who", player_id),
                );
            } else {
                // （1）「取消其对目标之一的[指定]」 -- per-pair cancel: cancel the
                // counteractor's own designation when it has one, else the first
                // named seat still standing (an earlier copy may have dropped one).
                // The rest of the play's designations still land.
                let live = |s: i32| !ctx::designation_cancelled(s);
                let self_seat = designations
                    .iter()
                    .copied()
                    .find(|&s| s == player_id && live(s));
                let next = self_seat.or_else(|| designations.iter().copied().find(|&s| live(s)));
                // No live designation left: clause 1 has nothing to do (C# re-sets
                // an already-set `immune<p>` tag) -- the play is NOT clause 2 and
                // keeps its non-designation effects.
                if let Some(seat) = next {
                    // 真奈 join (「改为你」): when played under a join, the
                    // self-targeted cancel is redirected to 真奈's user.
                    if let (Some(_), Some(to)) = (self_seat, ctx::join_redirect_target()) {
                        ctx::cancel_designation(to);
                        ctx::set_join_used();
                        // 真奈's clause: 「若以此种方式使你免于受到该影响，打出那张
                        // [反击]卡的玩家可抽一张卡」 -- 「可」, so the joiner chooses.
                        // TODO(规则书): this draw is 真奈's effect, not this card's
                        //   -- once the nested join window reports joiners, move it
                        //   to 真奈's body.
                        if ctx::ask_yes(
                            player_id,
                            &Msg::new(key!("net_error_draw_title")),
                            &Msg::new(key!("net_error_draw_ask")),
                        )? {
                            ctx::draw(player_id, 1)?;
                        }
                        ctx::log(
                            player_id,
                            &Msg::new(key!("net_error_target")).player_id("who", to),
                        );
                    } else {
                        ctx::cancel_designation(seat);
                        ctx::log(
                            player_id,
                            &Msg::new(key!("net_error_target")).player_id("who", seat),
                        );
                    }
                }
            }
        }
        // `effect` is the tail of 「取消其对目标之一的[指定]」 for a play whose
        // designation the `card` window above did not stop: this link *is* that
        // one designation, so cancelling its declaration spares it
        // (`target_player` reads the cancelled list only before raising it).
        // The settlement `target` hook cannot retract a landed hit, so there is
        // no `Target` arm and `ChainKind::Target` is not declared.
        TriggerKind::Effect => {
            // 规则书（1）[反击]: 「手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]」
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

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
