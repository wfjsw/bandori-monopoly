//! `skill:Sumimi:人气偶像组合`
//!
//! 规则书（band sheet, Sumimi）:
//! > （1）若你在一回合内从其他玩家处获得过资金，你的回合结束时为此卡添加1个奇迹
//! > 水晶；否则，移除此卡上的所有奇迹水晶。（2）此卡上的每个奇迹水晶可使你在向其他
//! > 人收费时让金额额外提高100资金。（3）你可携带两个角色的专属卡牌，但仅有在你的
//! > 角色卡为对应角色时才可打出。
//!
//! 「为此卡添加」 is a band-card crystal (`add_card_crystals` on this rule's own
//! id). （1）'s 「从其他玩家处获得过资金」 is a `PayAfter` landing in this
//! player's favour, latched per turn.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "skill:Sumimi:人气偶像组合";
/// 「若你在一回合内从其他玩家处获得过资金」.
const GOT: &str = "skill.sumimi.got";

pub const SUMIMI: CardDef = CardDef::new(
    "skill:Sumimi:人气偶像组合",
    &[
        On::Hook(&[HookKind::PayAfter], None, after_pay, card_sdk::pre::MINE),
        On::Hook(&[HookKind::PayMul], None, bend, card_sdk::pre::MINE),
        On::Hook(&[HookKind::TurnEndBefore], None, at_turn_end, card_sdk::pre::MINE),
    ],
)
    .legacy(&[(0, legacy_mine), (1, legacy_mine), (2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）'s latch.
fn after_pay(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::target() != player_id {
        return Ok(());
    }
    if ctx::trigger::player_id() == player_id {
        return Ok(());
    }
    state::set(player_id, GOT, 1);
    Ok(())
}

/// （2）「此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金」.
fn bend(player_id: i32) -> card_sdk::Asked {
    let n = ctx::crystals();
    if n <= 0 {
        return Ok(());
    }
    // 「向其他人收费」 -- this player is the payer's counterpart.
    if ctx::trigger::target() != player_id {
        return Ok(());
    }
    let amount = ctx::trigger::value();
    ctx::trigger::set_pay_amount(amount + n * 100);
    Ok(())
}

/// （1）「你的回合结束时为此卡添加1个奇迹水晶；否则，移除此卡上的所有奇迹水晶」.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, GOT) != 0 {
        state::set(player_id, GOT, 0);
        ctx::add_crystals(1, i32::MAX)?;
        ctx::log(player_id, &Msg::new(key!("sumimi_crystal")));
        return Ok(());
    }
    let n = ctx::crystals();
    if n > 0 {
        ctx::add_crystals(-n, i32::MAX)?;
        ctx::log(player_id, &Msg::new(key!("sumimi_cleared")));
    }
    Ok(())
}

// （3）「你可携带两个角色的专属卡牌，但仅有在你的角色卡为对应角色时才可打出」 --
// a hand-composition rule (two characters' exclusive cards may be held) plus a
// play gate that reads the *character* the card belongs to.
// TODO(规则书)（3）: 「你可携带两个角色的专属卡牌」 -- carrying two characters'
//   exclusive cards is a deck/hand building rule the engine has no form for
//   (there is no per-card 「this belongs to character X」 field to filter the hand
//   on, and no hand-slot rule). The play half -- 「仅有在你的角色卡为对应角色时
//   才可打出」 -- would be `ctx::character_is` in each exclusive card's own gate.
