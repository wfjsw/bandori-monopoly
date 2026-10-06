//! `skill:弦卷心:弦卷集团`
//!
//! 规则书（skill sheet, 弦卷心）:
//! > （1）开局时获得1000资金
//! > （2）[经过]CiRCLE时额外获得1500资金
//!
//! （2） is a field event -- [`On::Hook`] at `Pass`, this player's own landing on
//! a CiRCLE tile. 「额外」 means on top of the CiRCLE reward the tile already
//! pays, so it is a plain gain and not a replacement.
//!
//! （1） 「开局时」 has no occurrence to hang on: the character is picked during
//! setup and the first thing that fires after is a turn. It is declared at the
//! first `TurnStartBefore` and latched in the keyed state so it happens once.
//! That is close to 开局 rather than equal to it -- anything that paid out
//! between setup and the first turn start would come first.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// Latch so 「开局时」 pays once. Free-form state: this is a consumer naming its
/// own key, which is what the keyed map is for.
const DONE: &str = "skill.kasumiGroup.start";

pub const KASUMI_GROUP: CardDef = CardDef::new(
    "skill:弦卷心:弦卷集团",
    &[
        On::Hook(&[HookKind::TurnStartBefore], |_| true, at_start),
        On::Hook(&[HookKind::Pass], |_| true, on_pass),
    ],
);

/// （1）「开局时获得1000资金」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, DONE) != 0 {
        return Ok(());
    }
    state::set(player_id, DONE, 1);
    ctx::gain(player_id, 1000, &Msg::new(key!("kasumi_group_start")));
    Ok(())
}

/// （2）「[经过]CiRCLE时额外获得1500资金」 -- this player's own pass, onto a
/// CiRCLE tile.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::player_id() != player_id {
        return Ok(());
    }
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain(player_id, 1500, &Msg::new(key!("kasumi_group_pass")));
    Ok(())
}
