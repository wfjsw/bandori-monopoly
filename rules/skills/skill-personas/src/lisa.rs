//! `skill:今井莉莎:慈爱女神`
//!
//! 规则书（skill sheet, 今井莉莎）:
//! > （1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）
//! > （2）移动阶段前，你可以消耗一个火罐，将此次移动变为传送到距离你正向距离最近的
//! > 角色所在的格子，触发结算。本次传送触发结算前，你可以额外消耗一个火罐或使与你
//! > 在同一个格子上的另一个角色获得一个可超过上限的临时火罐，若你这样做，此次传送
//! > 不触发任何结算。
//! > （3）你提供的临时火罐将会在两回合后失去，使用你提供的临时火罐需要支付你600
//! > 资金（被冲榜类效果被动消耗无需支付）。
//!
//! （1） is `Pass` onto CiRCLE.
//!
//! （2） is a teleport to the nearest character *ahead*, and then an optional
//! second cost that cancels the settle -- 「若你这样做，此次传送不触发任何结算」.
//! Both halves are one press; the second is offered while the move is still
//! being planned.
//!
//! （3） is the temporary pot: a counter on another player that expires after
//! two of this seat's turns and whose *use* pays this seat 600. The kernel
//! carries the provenance with the counter (`fire::temp`): the grant stamps
//! this seat and an expiry, [`declare_cap`] drops the pot at the stamp, and
//! `fire::spend` charges 600 per temp pot to the stamp's provider -- except
//! on a passive spend (「被冲榜类效果被动消耗无需支付」), which is
//! `fire::spend_passive`.

use bandori_std::ctx::{self, plan, state};
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

pub const LISA: CardDef = CardDef::new(
    "skill:今井莉莎:慈爱女神",
    &[
        On::Play("", Some(can_use), use_skill),
        // The expiry narrates `log.fire_temp_expired`, so the turn-start entry
        // is a visible body; the game-start one has nothing to expire.
        On::Action(Action::TurnStartBefore("", None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        On::Action(Action::Pass(bandori_std::guards::OWN_START, None, on_pass)),
    ],
)
.rule()
// 「（初始2，上限2）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 2),
    (bandori_std::fire::FIRE_CAP, 2),
]);

/// 「初始2，上限2」, and （3）'s 「两回合后失去」 -- the provider's turn start
/// is where the temp pots it handed out expire.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    bandori_std::fire::temp::expire_owned_by(player_id);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得1个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("lisa_gain")))?;
    Ok(())
}

/// （2） 「你可以消耗一个火罐」.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if bandori_std::fire::get(player_id) < 1 {
        return Some(Msg::new(key!("lisa_no_fire")));
    }
    None
}

/// （2）「将此次移动变为传送到距离你正向距离最近的角色所在的格子，触发结算」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    // 「距离你正向距离最近的角色所在的格子」 -- the nearest character ahead,
    // plus any seat a `R:必然的联系（莉莎）` copy grants the waiver on
    // (「无视【距离最近】」). The pick only appears when that makes the set
    // non-singleton, so the plain path is unchanged.
    let candidates = bandori_std::nearest::skill_candidates(player_id, ctx::DistanceKind::Forward);
    let to = match candidates.len() {
        0 => return Ok(()),
        1 => ctx::player_pos(candidates[0]),
        _ => ctx::player_pos(ctx::ask_player(
            player_id,
            &Msg::new(key!("lisa_pick_title")),
            &Msg::new(key!("lisa_pick_ask")),
            &candidates,
        )?),
    };
    if to < 0 {
        return Ok(());
    }
    if !bandori_std::fire::spend_skill(player_id, 1, &Msg::new(key!("lisa_spend")))? {
        return Ok(());
    }
    plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    plan::set_teleport_to(to);
    plan::set_settles(true);
    ctx::log(player_id, &Msg::new(key!("lisa_moved")).tile("tile", to));
    // 「本次传送触发结算前，你可以额外消耗一个火罐或使与你在同一个格子上的另一个
    // 角色获得一个可超过上限的临时火罐，若你这样做，此次传送不触发任何结算」 --
    // offered here, before the move runs.
    if bandori_std::fire::get(player_id) < 1 {
        return Ok(());
    }
    let here: alloc::vec::Vec<i32> = (0..ctx::player_count())
        .filter(|&p| p != player_id && !ctx::player_out(p) && ctx::player_pos(p) == to)
        .collect();
    if here.is_empty() {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("lisa_cancel_title")),
        &Msg::new(key!("lisa_cancel_ask")),
    )? {
        return Ok(());
    }
    if !bandori_std::fire::spend_skill(player_id, 1, &Msg::new(key!("lisa_spend")))? {
        return Ok(());
    }
    // 「若你这样做，此次传送不触发任何结算」
    plan::set_settles(false);
    // 「使与你在同一个格子上的另一个角色获得一个可超过上限的临时火罐」
    // The temp pot is the kernel's second declared counter (`fire::temp`),
    // not a free key (MARKERS.md §7); `grant_from` stamps this seat as its
    // provider so （3）'s 600-on-use and two-turn loss apply.
    let who = here[0];
    let got = bandori_std::fire::temp::grant_from(player_id, who, 1);
    // 效果适用 narrates the outcome: a grant that landed on the floor must not
    // read as one that took.
    if got > 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("lisa_temp")).player_id("who", who),
        );
    } else {
        ctx::log(
            player_id,
            &Msg::new(key!("lisa_temp_none")).player_id("who", who),
        );
    }
    Ok(())
}

//

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
