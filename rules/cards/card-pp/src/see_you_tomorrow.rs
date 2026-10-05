//! `PP:[衍生]明天见` -- C# `CardSeeYouTomorrow`: flip the player's face-down
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]明天见`）:
//! > [衍生]明天见：
//! > [特]：
//! >
//! > （1）只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用。
//! >
//! > （2）[共鸣]无视此卡的[特]效果
//! > （1）。
//! > [手]：
//! > 将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正。
//!
//! `P✽P粉丝` face-up (up to the fan count). The [特] gate lives in `cant_play`.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const SEE_YOU_TOMORROW: CardDef = CardDef::new("PP:[衍生]明天见", &[
    On::Play(Some(cant_play), see_you_tomorrow)]);

/// The C# `H.FansUp` / `H.FansDown` token names (`P✽P粉丝` faces).
const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[特]（1）: 「只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用」
    // 规则书[特]（2）: 「[共鸣]无视此卡的[特]效果（1）」 -- C# `CardSeeYouTomorrow.WhyNot`
    // opens the gate while `H.HasResonance(seat)` (「PP:[衍生]共鸣」 is in hand).
    let up = ctx::tok(player_id, FANS_UP);
    let down = ctx::tok(player_id, FANS_DOWN);
    let has_resonance = ctx::hand_count(player_id, "PP:[衍生]共鸣") > 0;
    if up > down && !has_resonance {
        return Some(Msg::new(key!("see_you_tomorrow_why_not")));
    }
    None
}

fn see_you_tomorrow(player_id: i32) {
    let up = ctx::tok(player_id, FANS_UP);
    let down = ctx::tok(player_id, FANS_DOWN);
    // 规则书[特]（2）: 「[共鸣]无视此卡的[特]效果（1）」 -- when the gate was passed
    // only because a 共鸣 is in hand, paying that 共鸣 is the cost of the play.
    // Declining ends the effect here.
    if up > down && !crate::resonance::try_resonance(player_id) {
        return;
    }
    // 规则书[手]: 「将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正」
    let fans = up + down;
    let flip = fans.min(down);
    if flip > 0 {
        ctx::add_tok(player_id, FANS_DOWN, -flip, i32::MAX);
        ctx::add_tok(player_id, FANS_UP, flip, i32::MAX);
        ctx::log(player_id, &Msg::new(key!("see_you_tomorrow_up")).player_id("who", player_id).i("n", flip as i64));
    }
}