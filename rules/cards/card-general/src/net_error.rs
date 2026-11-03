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
        counteract).declares_negation().counter_reason(key!("net_error_when"))],
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
    match trigger::kind() {
        TriggerKind::Event => true,
        TriggerKind::Card => {
            !trigger::cancelled() && trigger::player_id() != player_id && {
                // Clause 1 has nothing left to cancel once every designation
                // is already dropped; another copy would be spent for nothing.
                // Under a join the joiner acts for the opener (「改为你」), so
                // the designation that matters is the opener's.
                match ctx::join::opener() {
                    Some(opener) => !ctx::designation_cancelled(opener),
                    None => {
                        let designations = ctx::designations(trigger::player_id());
                        designations.is_empty()
                            || designations.iter().any(|&s| !ctx::designation_cancelled(s))
                    }
                }
            }
            // TODO(规则书)[judgement]: TODO(规则书)[judgement](ABI): 「1张手卡的[手]效果」 does not
            // restrict the play to another player's -- we refuse the play's own seat its own
            // [反击] (OPEN-自我反击).
        }
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
            trigger::set_cancelled();
            ctx::log(
                player_id,
                &Msg::new(key!("net_error_event")).player_id("who", player_id),
            );
            // TODO(规则书)[judgement](ABI):
        }
        TriggerKind::Card => {
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
                if let Some(seat) = next {
                    // 真奈 join (「改为你」): under a join the self-targeted
                    // cancel is redirected to the opener; a shielding counter
                    // marks the opener's report (`join::shield`). The draws
                    // both belong to 真奈's body now.
                    let to = match ctx::join::opener() {
                        Some(opener) => {
                            ctx::join::shield();
                            opener
                        }
                        None => seat,
                    };
                    ctx::cancel_designation(to);
                    ctx::log(
                        player_id,
                        &Msg::new(key!("net_error_target")).player_id("who", to),
                    );
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
            // -- this link *is* one designation (the joiner is presented as the
            // target, 「如同自身的对应目标被指定一般」); under a join the
            // cancel goes to the opener and shields it (「改为你」).
            let to = match ctx::join::opener() {
                Some(opener) => {
                    ctx::join::shield();
                    opener
                }
                None => player_id,
            };
            ctx::cancel_designation(to);
            ctx::log(
                player_id,
                &Msg::new(key!("net_error_target")).player_id("who", to),
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
