//! `PP:[衍生]明天见` -- C# `CardSeeYouTomorrow`: flip the player's face-down
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
use skill_groups::fans;

pub const SEE_YOU_TOMORROW: CardDef = CardDef::new(
    "PP:[衍生]明天见",
    &[On::Play("", "", Some(cant_play), see_you_tomorrow)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"明天见"#)
        .raw_name(r#"[衍生]明天见"#)
        .derived(true)
        .tags(&[r#"手"#, r#"特"#, r#"衍生"#])
        .text(r#"[特]：
（1）只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用。
（2）[共鸣]无视此卡的[特]效果（1）。
[手]：
将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正。"#)
        .cell(r#"D6"#)
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[特]（1）: 「只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用」
    // 规则书[特]（2）: 「[共鸣]无视此卡的[特]效果（1）」 -- C# `CardSeeYouTomorrow.WhyNot`
    // opens the gate while `H.HasResonance(seat)` (「PP:[衍生]共鸣」 is in hand).
    let up = fans::up(player_id);
    let down = fans::down(player_id);
    let has_resonance = ctx::hand_count(player_id, "PP:[衍生]共鸣") > 0;
    if up > down && !has_resonance {
        return Some(Msg::new(key!("see_you_tomorrow_why_not")));
    }
    None
}

fn see_you_tomorrow(player_id: i32) -> card_sdk::Asked {
    let up = fans::up(player_id);
    let down = fans::down(player_id);
    // 规则书[特]（2）: 「[共鸣]无视此卡的[特]效果（1）」 -- when the gate was passed
    // only because a 共鸣 is in hand, paying that 共鸣 is the cost of the play.
    // Declining means the play was never allowed (「才可使用」): the card goes
    // back to the hand instead of the discard pile (`Dest::Hand`).
    // TODO(规则书): the engine has no pre-commit abort, so the play was already
    // announced (log, `card` window, played/two-cards counters) before this
    // refusal; only the card's fate is undone here.
    if up > down && !crate::resonance::try_resonance(player_id)? {
        ctx::set_dest(ctx::Dest::Hand);
        return Ok(());
    }
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

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn see_you_tomorrow_kani(player_id: i32) -> card_sdk::Asked {
    see_you_tomorrow(player_id)
}
