//! `skill:佐藤益木:与燃烧的红色一起驰骋`
//!
//! 规则书（skill sheet, 佐藤益木）:
//! > [资源]：
//! > （1）[火罐]：初始0，上限3。
//! > [自动]：
//! > （1）自己每次领取完CiRCLE奖励后首次[经过]“白雪学园”或“银河拉面馆”之一时：
//! > 获得1个[火罐]。
//! > [主动]：
//! > （1）自己的移动阶段前：可使用X个[火罐]，为本回合用于确定[主要移动]步数的
//! > [投掷]额外添加X个d20骰子。
//!
//! [自动]（1） is `Pass` onto one of the two named tiles, gated to the **first**
//! such pass after a CiRCLE reward -- a latch set by the reward and cleared by
//! that first pass.
//!
//! [主动]（1） 「移动阶段前」 is the operations phase, the same window every
//! other 「移动阶段前可使用」 skill opens in; the engine already refuses an
//! action outside it. The extra dice are `add_extra_dice`, which rides along on
//! the plan rather than replacing its base -- 「额外添加」, not 「变更为」 -- and
//! the die is a d20.

use bandori_std::ctx::{self, plan, state};
use bandori_std::kind::Actions;
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

/// The 「领取完CiRCLE奖励后首次」 latch: 1 = a reward was taken and the first
/// qualifying pass has not happened yet; cleared by that pass.
const FIRST_PASS: &str = "skill.maskingBurn.after";

pub const MASKING: CardDef = CardDef::new(
    "skill:佐藤益木:与燃烧的红色一起驰骋",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        // 「领取完CiRCLE奖励后」 -- the reward arms the latch.
        On::Action(Actions::PassRewardAffected("actor == owner", None, arm))
            .silent(),
        // [自动]（1）「每次领取完CiRCLE奖励后首次[经过]“白雪学园”或“银河拉面馆”
        // 之一时」 -- the two named tiles are the hook's applicability
        // (`tile_named` is in the condition vocabulary); the latch makes it the
        // first pass only.
        On::Action(Action::Pass(
            "actor == owner && state(owner, 'skill.maskingBurn.after') > 0 && (tile.id == tile_named('白雪学园') || tile.id == tile_named('银河拉面馆'))",
            None,
            on_pass,
        )),
    ],
).rule()
    // 「（初始0，上限3）」
    .props(&[(bandori_std::fire::FIRE_INITIAL, 0), (bandori_std::fire::FIRE_CAP, 3)]);

/// 「初始0，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// 「领取完CiRCLE奖励后」 -- set the first-pass latch.
fn arm(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, FIRST_PASS, 1);
    Ok(())
}

/// [自动]（1）「…首次[经过]“白雪学园”或“银河拉面馆”之一时：获得1个[火罐]」.
/// The two named tiles and the latch are the condition; the pass spends it.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, FIRST_PASS, 0);
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("masking_gain")))?;
    Ok(())
}

/// [主动]（1）「移动阶段前可使用X个[火罐]」 -- X is what the player holds.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if bandori_std::fire::get(player_id) < 1 {
        return Some(Msg::new(key!("masking_no_fire")));
    }
    None
}

/// [主动]（1）「为本回合用于确定[主要移动]步数的[投掷]额外添加X个d20骰子」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let have = bandori_std::fire::get(player_id);
    if have < 1 {
        return Ok(());
    }
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("masking_title")),
        &Msg::new(key!("masking_ask")),
        1,
        have,
    )?;
    if x < 1 || !bandori_std::fire::spend_skill(player_id, x, &Msg::new(key!("masking_spend")))? {
        return Ok(());
    }
    plan::add_extra_dice(x, 20, "与燃烧的红色一起驰骋");
    ctx::log(player_id, &Msg::new(key!("masking_dice")).i("n", x as i64));
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn arm_kani(player_id: i32) -> card_sdk::Asked {
    arm(player_id)
}
