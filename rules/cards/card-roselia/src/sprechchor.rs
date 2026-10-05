//! `R:Sprechchor` -- C# `CardSprechchor`: roll 1d20, gain 1,000 + roll x 120.
//!
//! 规则书（docs/rulebook/cards.json, id `R:Sprechchor`）:
//! > Sprechchor：
//! >  在Livehouse地块开始回合时，可打出此卡并投掷1d20，获得1000+X*120的资金，X为本次掷骰出目
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const SPRECHCHOR: CardDef = CardDef::new("R:Sprechchor", &[
    On::Play(Some(cant_play), sprechchor)]);

/// 规则书: 「在Livehouse地块开始回合时」 -- the **turn-start** square, not the
/// one a mid-turn walk has since reached (C# `H._turnSnap[i].pos`).
fn cant_play(player_id: i32) -> Option<Msg> {
    let t = ctx::turn_start_pos(player_id);
    if t >= 0 && ctx::is_live_house_for(player_id, t) {
        return None;
    }
    Some(Msg::new(key!("sprechchor_why_not")))
}

fn sprechchor(player_id: i32) {
    // `ctx::roll` honours a forced extreme (「以理论最大值或最小值结算」).
    let n = ctx::roll(player_id, 1, 20);
    ctx::gain(player_id, 1000 + n * 120, &Msg::new(key!("sprechchor_why")));
}
