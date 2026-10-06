//! `PP:[衍生]共鸣` -- the [共鸣] fuel card.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]共鸣`）:
//! > [衍生]共鸣：
//! > [特]：
//! > 拥有此卡的玩家的卡可发动[共鸣]时将此卡放入弃卡区，发动那张卡的[共鸣]效果并为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]。
//!
//! Not a hand play (no `Play`): the [特] is what every other card's [共鸣] branch
//! calls, and it is [`try_resonance`] below. The clause is one sentence with
//! three parts and they all happen together:
//!
//! - 「将此卡放入弃卡区」 -- the cost is this card leaving the hand, so a player
//!   holding none cannot offer it at all.
//! - 「发动那张卡的[共鸣]效果」 -- the caller's own branch; this module only
//!   decides whether it runs.
//! - 「为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]」 -- paid on top
//!   of the branch, and only to a Pastel✽Palettes player.

use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg};

/// The [共鸣] fuel card. Empty `On`: it has no hand play and no hook of its own
/// -- it is fuel for other cards' [共鸣] branches, offered by [`try_resonance`].
pub const RESONANCE: CardDef = CardDef::new("PP:[衍生]共鸣", &[]);

/// Offer the [共鸣] cost. Returns whether the player paid it, which is the
/// caller's signal to run its own [共鸣] branch.
///
/// `false` when they decline or hold none -- the cost is this card, not some
/// abstract point. On success the card is discarded and, for a
/// Pastel✽Palettes player, 2 [奇迹水晶] go to the band card (「添加2[奇迹水晶]」).
pub fn try_resonance(player_id: i32) -> Result<bool, card_sdk::Prompt> {
    if ctx::hand_count(player_id, RESONANCE.id) <= 0 {
        return Ok(false);
    }
    let yes = ctx::ask_yes(
        player_id,
        &Msg::new(key!("resonance_title")),
        &Msg::new(key!("resonance_text")),
    )?;
    if !yes {
        return Ok(false);
    }
    if !ctx::take_from_hand(player_id, RESONANCE.id) {
        return Ok(false);
    }
    // 「将此卡放入弃卡区」
    ctx::to_discard(player_id, RESONANCE.id);
    // 「为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]」
    if ctx::in_band(player_id, "Pastel✽Palettes") {
        ctx::add_band_crystals(player_id, 2, i32::MAX);
    }
    Ok(true)
}
