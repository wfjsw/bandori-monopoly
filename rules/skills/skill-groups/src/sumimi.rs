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

use bandori_std::state_key; use card_sdk::abi::{ActionKind, FilterKind};
use bandori_std::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "skill:Sumimi:人气偶像组合";
/// 「若你在一回合内从其他玩家处获得过资金」.
const GOT: &str = "skill.sumimi.got";

pub const SUMIMI: CardDef = CardDef::new(
    "skill:Sumimi:人气偶像组合",
    &[
        // 「从其他玩家处获得过资金」 -- a PayAfter *landing in this player's
        // favour* from a real other seat. `MINE` (`actor == owner`) only fires
        // when this player is the *payer*, which is the opposite of an inbound
        // charge; the raise's `actor` is `from`. Fire on every placed pay and
        // let the body keep the from/to filter.
        On::Action(&[ActionKind::PayAfter], "card.placed", "", None, after_pay),
        // 「向其他人收费」 -- this player is the payee of a charge from another
        // seat. Again `MINE` is the payer side; `target == owner` is the charge.
        On::Filter(&[FilterKind::PaySplitMul], "target == owner && card.placed", None, bend),
        // Both halves of （1） live in the body: add when GOT, else clear.
        // The old `slot('skill.sumimi.got') == 0` guard only ever admitted the
        // clear branch (FINDINGS C-SUMIMI-band-01).
        On::Action(&[ActionKind::TurnEndBefore], "actor == owner", "", None, at_turn_end),
    ],
)
    .silent(&[0]);

/// （1）'s latch -- 「若你在一回合内从其他玩家处获得过资金」.
fn after_pay(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::target() != player_id {
        return Ok(());
    }
    let from = ctx::trigger::player_id();
    // 「从其他玩家处」 -- not a bank print (`from < 0`) and not a self-pay.
    if from < 0 || from == player_id {
        return Ok(());
    }
    state::set(player_id, GOT, 1);
    Ok(())
}

/// （2）「此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金」.
fn bend(player_id: i32, value: i32) -> i32 {
    let n = ctx::crystals();
    if n <= 0 {
        return value;
    }
    // 「向其他人收费」 -- this player is the payee of another seat's payment.
    if ctx::trigger::target() != player_id {
        return value;
    }
    let from = ctx::trigger::player_id();
    if from < 0 || from == player_id {
        return value;
    }
    value + n * 100
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
// a deck-composition rule (two characters' exclusive cards may be carried) plus
// the generic in-match play gate. The carry half is **data** (the roster band row
// `sharesExclusives`, `deck::shares_exclusives`) -- deck building has no rules
// handle (docs/GUARDS.md §ExclusiveUse). The play half is the engine's generic
// own/base exclusive rule (「角色卡为对应角色」, with a variant counted as its
// base); this card needs no `ExclusiveUse` entry of its own.
// TODO(规则书)（3）: 「你可携带两个角色的专属卡牌」 -- carrying two characters'
//   exclusive cards is a deck/hand building rule. The deck half is data
//   (`sharesExclusives`); a hand-composition filter (holding a second
//   character's exclusive without it being in the starting deck) still has no
//   form.

/// Roster band row for this band skill (ABI v63).
pub const BAND: bandori_std::roster::GroupDef =
bandori_std::roster::GroupDef::new(r#"Sumimi"#)
    .short_name(r#"Sumimi"#)
    .color(r#"#FFFF66"#)
    .skill(r#"人气偶像组合"#)
    .text(r#"（1）若你在一回合内从其他玩家处获得过资金，你的回合结束时为此卡添加1个奇迹水晶；否则，移除此卡上的所有奇迹水晶。（2）此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金。（3）你可携带两个角色的专属卡牌，但仅有在你的角色卡为对应角色时才可打出。"#)
    .logo("")
    .shares_exclusives(true);

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn after_pay_kani(player_id: i32) -> card_sdk::Asked {
    after_pay(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn bend_kani(player_id: i32, value: i32) -> i32 {
    bend(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}
