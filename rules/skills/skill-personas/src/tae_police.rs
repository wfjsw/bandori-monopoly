//! `skill:花园多惠:花园警察，出警！`
//!
//! 规则书（skill sheet, 花园多惠）:
//! > （1）每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是"星空齿科"则在
//! > 结果对应的格子放置一个[多惠兔子]；当你经过有[多惠兔子]的格子时[移除]该格子上
//! > 的所有[多惠兔子]并获得等量的[火罐]（初始2，上限4）
//! > （2）在其他玩家使用角色/团技能的[主]效果或使用手牌的[手]效果时你可以使用4个
//! > [火罐]将其抵消，然后你获得500资金且被抵消的玩家获得2000资金
//!
//! （1）'s 「[多惠兔子]」 is a tile mark, which is what `place_mark_new` /
//! `bump_mark` carry; 「[移除]该格子上的所有[多惠兔子]并获得等量的[火罐]」 is a
//! count-and-clear on the pass.
//!
//! （2） 「将其抵消」 is a [反击]: the effect is named and then voided, which is
//! `trigger::set_cancelled`. 「使用角色/团技能的[主]效果或使用手牌的[手]效果」
//! is the `card` chain for a hand play and `skillUsed` for a skill press.

use card_sdk::abi::{prop, state_key, ChainKind, ActionKind, MarkFilter};
use bandori_std::ctx::{self, state, trigger};
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「[多惠兔子]」.
const RABBIT: &str = "多惠兔子";

pub const TAE_POLICE: CardDef = CardDef::new(
    "skill:花园多惠:花园警察，出警！",
    &[
        On::Action(&[ActionKind::TurnStartBefore, ActionKind::DeckAtGameStart], "", "", None, declare_cap),
        On::Action(&[bandori_std::kind::PassRewardAffected], card_sdk::pre::MINE, "", None, on_circle),
        // 「当你经过有[多惠兔子]的格子时」 -- a tile with rabbits is the hook's
        // applicability (residual guard; `pre::MINE` already states the actor).
        On::Action(&[ActionKind::Pass], card_sdk::pre::MINE, "", Some(on_pass_has_rabbits), on_pass),
        // （2） 「在其他玩家使用角色/团技能的[主]效果或使用手牌的[手]效果时你可以使用4个
        // [火罐]将其抵消」 -- a [反击] from the bound skill (always in play), so it
        // must be offered from the field (`COUNTERACT_FROM_FIELD`, the same shape
        // as 冰川纱夜（2）'s move-length counteraction).
        On::Counteract(&[ChainKind::Card, bandori_std::kind::SkillUsed.as_trigger()], "", "", Some(can_negate), negate),
    ],
)
    .props(&[
        (prop::COUNTERACT_FROM_FIELD, 1),
        // One cancel per activation: the skill stays in play, so without a
        // group reservation the ring would re-offer it after the declaration
        // (the seat keeps the floor until it passes).
        (prop::COUNTERACT_GROUP, 1),
    ])
    .silent(&[0]);

/// 「初始2，上限4」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是"星空齿科"
/// 则在结果对应的格子放置一个[多惠兔子]」.
fn on_circle(player_id: i32) -> card_sdk::Asked {
    let n = ctx::roll(player_id, 3, 20);
    let total = ctx::tile_count();
    if total <= 0 {
        return Ok(());
    }
    let t = (n - 1).rem_euclid(total);
    if t == ctx::tile_named("星空齿科") {
        return Ok(());
    }
    // One rabbit per placement, each its own row (tests pin row identity).
    ctx::place_mark(
        t,
        RABBIT,
        "",
        player_id,
        ctx::self_uid(),
        1,
        &Msg::new(key!("tae_police_note")), card_sdk::abi::Stack::Fresh);
    ctx::log(
        player_id,
        &Msg::new(key!("tae_police_placed"))
            .tile("tile", t)
            .i("n", n as i64),
    );
    Ok(())
}

/// 「经过有[多惠兔子]的格子」 -- the applicability of the `Pass` hook.
fn on_pass_has_rabbits(_player_id: i32) -> bool {
    ctx::count_marks(ctx::trigger::tile(), &MarkFilter::any().kind(RABBIT)) > 0
}

/// （1）「当你经过有[多惠兔子]的格子时[移除]该格子上的所有[多惠兔子]并获得
/// 等量的[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    let n = ctx::count_marks(t, &MarkFilter::any().kind(RABBIT));
    ctx::remove_marks(t, &MarkFilter::any().kind(RABBIT));
    rules_status::fire::gain(player_id, n, &Msg::new(key!("tae_police_gain")))?;
    Ok(())
}

/// （2） 「你可以使用4个[火罐]将其抵消」 -- only against someone else's activation.
/// The categories do the selecting (`card` = a hand play, `skillUsed` = a skill
/// press); the guard states the rest of the clause.
fn can_negate(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() != player_id
        && state::get(player_id, state_key::CHARGE) >= 4
    // TODO(规则书)[judgement]: the clause names 「[主]效果」 and 「[手]效果」, not
    //   every activation. `skillUsed` is a skill press (the 主 half), but
    //   `card` admits any hand play -- a [反击] or [特] card included. There is
    //   no tag query in the ABI to read the card's tags back off the trigger.
}

/// （2）「将其抵消，然后你获得500资金且被抵消的玩家获得2000资金」.
///
/// The [反击] window's play-vs-skip *is* the 「你可以使用」 choice; declaring
/// the counteraction spends the 4 [火罐] and voids the effect. No extra confirm.
fn negate(player_id: i32) -> card_sdk::Asked {
    if !rules_status::fire::spend(player_id, 4, &Msg::new(key!("tae_police_spend")))? {
        return Ok(());
    }
    trigger::set_cancelled();
    let src = trigger::player_id();
    ctx::gain(player_id, 500, &Msg::new(key!("tae_police_gain_self")))?;
    if src >= 0 && !ctx::player_out(src) {
        ctx::gain(src, 2000, &Msg::new(key!("tae_police_gain_other")))?;
    }
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_circle_kani(player_id: i32) -> card_sdk::Asked {
    on_circle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn negate_kani(player_id: i32) -> card_sdk::Asked {
    negate(player_id)
}
