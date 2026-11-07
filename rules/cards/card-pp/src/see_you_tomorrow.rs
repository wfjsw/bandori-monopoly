//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]明天见`）:
//! > [衍生]明天见：
//! > [特]：
//!
//! > （1）只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用。
//!
//! > （2）[共鸣]无视此卡的[特]效果
//! > （1）。
//! > [手]：
//! > 将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正。
//!
//! `P✽P粉丝` face-up (up to the fan count). The [特] gate lives in `cant_play`.

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{it, items, para, sec, Activation, CardText, Declared};
use skill_groups::fans;

pub const SEE_YOU_TOMORROW: CardDef = CardDef::new(
    "PP:[衍生]明天见",
    &[On::Play("", Some(cant_play), unused_body).declare_arg(see_you_declare, see_you_tomorrow)],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"明天见"#)
        .raw_name(r#"[衍生]明天见"#)
        .derived(true)
        .tags(&[r#"手"#, r#"特"#, r#"衍生"#])
        .text(CardText::of(&[
            sec(
                r#"[特]："#,
                &[items(&[
                    it(r#"只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用。"#),
                    it(r#"[共鸣]无视此卡的[特]效果（1）。"#),
                ])],
            ),
            sec(
                r#"[手]："#,
                &[para(r#"将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正。"#)],
            ),
        ]))
        // sheet cell D6
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]),
);

/// Placeholder body -- `.declare_arg` replaces it with [`see_you_tomorrow`].
const fn unused_body(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

fn cant_play(player_id: i32) -> Option<Msg> {
    let up = fans::up(player_id);
    let down = fans::down(player_id);
    let has_resonance = ctx::hand_count(player_id, "PP:[衍生]共鸣") > 0;
    if up > down && !has_resonance {
        return Some(Msg::new(key!("see_you_tomorrow_blocked")));
    }
    None
}

/// 规则书[特]（2）: 「[共鸣]无视此卡的[特]效果（1）」 -- when the (1) gate fails,
/// paying a 共鸣 is the cost of the play. Collected **before** the point of no
/// return (`docs/ACTIVATION.md` §2, LOY-SEEYOU-01): a refusal declines the
/// activation, so the card and the 共鸣 stay in hand, nothing is logged and no
/// played counter moves. The gate is `cant_play`; when it already holds without
/// 共鸣 the collector does not spend one. The collector only *asks* -- its
/// writes are dropped -- and answers `1` when the body must pay.
fn see_you_declare(player_id: i32) -> Declared<i32> {
    let up = fans::up(player_id);
    let down = fans::down(player_id);
    if up <= down {
        return Ok(Activation::Arg(0));
    }
    if !crate::resonance::resonance_available(player_id)
        || !crate::resonance::ask_resonance(player_id)?
    {
        return Ok(Activation::Decline);
    }
    Ok(Activation::Arg(1))
}

fn see_you_tomorrow(player_id: i32, arg: i32) -> card_sdk::Asked {
    // 规则书[特]（2）: the 共鸣 is paid here, not in the collector (a declare
    // pass's writes are dropped). `arg` 1 = the gate failed and 共鸣 was
    // accepted; recompute the fans (the payment may have had riders).
    if arg == 1 {
        // The fuel vanishing between the ask and the body is the one case the
        // committed activation cannot undo; the play simply resolves.
        let _ = crate::resonance::pay_resonance(player_id);
    }
    let up = fans::up(player_id);
    let down = fans::down(player_id);
    // TODO(规则书)（2）: 「[共鸣]无视此卡的[特]效果（1）」 says ignore, not that the
    //   共鸣 is a mandatory cost -- we refuse the play instead of merely letting it
    //   through, and the offer is short-circuited behind the (1) gate, so a player
    //   who could already play can never pay it (and its +2 crystals). BACKLOG
    //   LOY-SEEYOU-02.
    // 规则书[手]: 「将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正」 -- X is the
    // total fan count (not the face-down count), so band clause （2） sees
    // `X > down` whenever the player also holds face-ups.
    let x = up + down;
    if x > 0 {
        let flip = fans::flip_up(player_id, x)?;
        if flip > 0 {
            ctx::log(
                player_id,
                &Msg::new(key!("see_you_tomorrow_up"))
                    .player_id("who", player_id)
                    .i("n", flip as i64),
            );
        }
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn see_you_tomorrow_kani(player_id: i32, arg: i32) -> card_sdk::Asked {
    see_you_tomorrow(player_id, arg)
}

#[cfg(kani)]
#[inline(always)]
pub fn see_you_declare_kani(player_id: i32) -> card_sdk::Declared<i32> {
    see_you_declare(player_id)
}
