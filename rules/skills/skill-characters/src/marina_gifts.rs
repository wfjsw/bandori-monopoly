//! `skill:月岛麻里奈:礼物还有好多好多哟`
//!
//! 规则书（skill sheet, 月岛麻里奈）:
//! > 其他玩家经过CiRCLE时，可向你支付一次500资金，你投掷一次1d10，
//! > 如果投掷结果至少为6，那名玩家获得1200资金。
//!
//! A field event, so one [`On::Hook`] at `Pass`. The wording splits three ways
//! and the order matters:
//!
//! - 「其他玩家经过CiRCLE时」 -- another player's pass, onto a CiRCLE tile.
//! - 「可向你支付一次500资金」 -- the passer *may*, so it is their decision and
//!   not this player's. Nothing happens if they decline or cannot pay.
//! - 「你投掷一次1d10，如果投掷结果至少为6」 -- the roll is this player's, and
//!   the payout is a straight threshold on it.
//!
//! 「一次」 is per occurrence: a player passes a given CiRCLE at most once per
//! move, so there is nothing to latch.

use card_sdk::abi::HookKind;
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const MARINA_GIFTS: CardDef = CardDef::new(
    "skill:月岛麻里奈:礼物还有好多好多哟",
    // 「其他玩家经过CiRCLE时」 -- another player's pass onto CiRCLE.
    &[On::Hook(
        &[HookKind::Pass],
        "actor != owner && actor >= 0 && is_circle(tile.id)",
        None,
        on_pass,
    )],
);

/// The cost of the offer, named in the clause.
const COST: i32 = 500;
/// 「获得1200资金」 when the roll lands.
const PAYOUT: i32 = 1200;

fn on_pass(player_id: i32) -> card_sdk::Asked {
    // 「其他玩家经过CiRCLE时」 is the condition
    // (`actor != owner && actor >= 0 && is_circle(tile.id)`).
    let passer = ctx::trigger::player_id();
    // 「可向你支付一次500资金」 -- the passer's call.
    let offered = ctx::ask_yes(
        passer,
        &Msg::new(key!("marina_gifts_offer_title")),
        &Msg::new(key!("marina_gifts_offer_text"))
            .player_id("who", player_id)
            .n("amount", COST as i64),
    )?;
    if !offered {
        return Ok(());
    }
    // Paying is all-or-nothing; a passer who cannot pay simply does not play.
    if ctx::transfer(
        passer,
        player_id,
        COST,
        &Msg::new(key!("marina_gifts_paid")),
    )? == 0
    {
        return Ok(());
    }
    // 「你投掷一次1d10，如果投掷结果至少为6，那名玩家获得1200资金」
    let face = ctx::roll(player_id, 1, 10);
    if face >= 6 {
        ctx::gain(
            passer,
            PAYOUT,
            &Msg::new(key!("marina_gifts_win")).i("n", face as i64),
        )?;
    }
    Ok(())
}
