//! `skill:八幡海铃:熟练的支援贝斯手`
//!
//! 规则书（skill sheet, 八幡海铃）:
//! > （1）状态1：其他玩家可以在他们的回合开始时向你支付400资金指定一个属于你的格子，
//! > 使其立即对前后一格内的一名玩家进行一次半价[结算]，并获得相当于你获取数额的资金；
//! > 直到这两名玩家的下回合结束，若你仍在状态1，他们不会被该效果再次收款。每两个回合
//! > 没有玩家发动该技能，你在回合开始时获得1火罐（初始0，上限4），若达到上限，可在
//! > 你的回合开始时选择进入状态2。
//! > （2） 状态2： 进入状态2时你可消耗X个火罐，指定其他人的X个地契，每张地契向对应
//! > 玩家支付100+n*100资金（n为对应格子上的房屋数），直到状态2结束为止，你与对方均分
//! > 那些地契收取的资金；每通过此法消耗4个火罐，你抽1张卡。
//!
//! 状态1's offer is another player's, at *their* turn start -- so the hook runs
//! on every turn start and asks the turn's player. 「前后一格内的一名玩家」 is a
//! player standing on the named tile or one of its two neighbours; the
//! 「半价[结算]」 is `card_settle_at` with the plan's pay factor at 50.
//!
//! 「每两个回合没有玩家发动该技能」 is a shared rest counter: any press resets
//! it, and two quiet turns pay out one pot.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use bandori_std::state_key; use card_sdk::abi::{ActionKind, FilterKind};
use bandori_std::ctx::{self, plan, state};
use card_sdk::{Action, CardDef, Filter, key, Msg, On};

/// 「每两个回合没有玩家发动该技能」 -- a band-wide rest counter.
const REST: &str = "skill.kaedeSupport.rest";
/// 「直到这两名玩家的下回合结束…他们不会被该效果再次收款」 -- per-payer latch,
/// keyed on the turn it expires.
const COOLDOWN: &str = "skill.kaedeSupport.cooldown.";
/// Tiles named in 状态2, as `"<tile>:<owner>"` markers this skill splits income
/// on. The counterpart owner rides here (the designation mark cannot hold it);
/// the mark is the displayed [指定] record (see `enter_two`).
const SPLIT: &str = "skill.kaedeSupport.split.";

pub const KAEDE_SUPPORT: CardDef = CardDef::new(
    "skill:八幡海铃:熟练的支援贝斯手",
    &[
        On::Action(Action::TurnStartBefore("", None, None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, None, declare_cap)),
        // 状态1's offer: another player's turn start, not in 状态2, and the
        // per-payer 「不会被该效果再次收款」 latch has expired. The latch key is
        // dynamic (`skill.kaedeSupport.cooldown.<seat>`), so it is the residual.
        On::Action(Action::TurnStartBefore("actor != owner && slot('skillState') != 2", None, Some(offer_support_open), offer_support)),
        On::Action(Action::TurnStartBefore("actor == owner && slot('skillState') != 2", None, None, at_turn_start)),
        On::Action(Action::TurnEnd(card_sdk::pre::MINE, None, None, at_turn_end)),
        On::Play("", None, Some(can_enter_two), enter_two),
        // 「你与对方均分那些地契收取的资金」 -- two halves of one clause (ABI v74
        // Filter/Action split). The PaySplitMul Filter reshapes the owner's take; the
        // Effect Action pays the other half out. Effect (not PayAfter) so the
        // gain's `trigger::value()` is still the declared figure -- PayAfter's
        // value is the settled amount and cannot re-derive `amount/2` for odd.
        // TODO(规则书): the gain half reads the declared amount at Effect, so a
        //   PaySplitAdd modifier that runs after this declaration would not be split
        //   (the reshape half sees the PaySplitMul-stage figure). Same figure when no
        //   other add/mul modifier touches the rent.
        On::Filter(Filter::PaySplitMul("slot('skillState') == 2 && pay_is_rent && value > 0 && tile.id >= 0", Some(split_rent_applies), split_rent)),
        On::Action(Action::Effect("slot('skillState') == 2 && pay_is_rent && value > 0 && tile.id >= 0", None, Some(split_rent_applies), split_rent_gain)),
    ],
)
    .silent(&[0, 1, 4])
    // 「（初始0，上限4）」
    .props(&[(bandori_std::fire::FIRE_INITIAL, 0), (bandori_std::fire::FIRE_CAP, 4)]);

/// Residual guard for [`split_rent`]: the tile is one of 状态2's named deeds.
/// The SPLIT marker is keyed on the tile (a dynamic state key), so it is not
/// CEL (docs/GUARDS.md §4.2).
fn split_rent_applies(player_id: i32) -> bool {
    let t = ctx::trigger::tile();
    state::get(player_id, &format!("{}{}", SPLIT, t)) >= 0
}

/// 「你与对方均分那些地契收取的资金」 -- half to the deed's owner, half here.
/// `slot('skillState') == 2 && pay_is_rent && value > 0 && tile.id >= 0` is
/// the pre and [`split_rent_applies`] the residual.
///
/// Filter half: reshape the owner's take to `amount - amount/2` (exact old
/// `set_pay_amount(amount - half)`). `value < 2` early-returns unchanged
/// (`half <= 0` in the old body).
fn split_rent(_player_id: i32, value: i32) -> i32 {
    if value < 2 {
        return value;
    }
    value - value / 2
}

/// Action half of the split: `ctx::gain(player_id, half, ...)`. Reads the
/// **declared** amount at `TriggerKind::Effect` (the figure the old body saw
/// at PaySplitMul when no other add/mul modifier touched the rent) and gains
/// `amount / 2`.
fn split_rent_gain(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    let half = amount / 2;
    if half <= 0 {
        // `value > 0` admits 1, whose half is 0 -- nothing to split.
        return Ok(());
    }
    ctx::gain(
        player_id,
        half,
        &Msg::new(key!("kaede_support_split")).i("n", half as i64),
    )?;
    Ok(())
}

/// Residual guard for [`offer_support`]: the per-payer cooldown latch has
/// expired. The key is `skill.kaedeSupport.cooldown.<seat>` (dynamic), so it
/// stays out of CEL.
fn offer_support_open(player_id: i32) -> bool {
    let other = ctx::trigger::player_id();
    let key = format!("{}{}", COOLDOWN, other);
    state::get(player_id, &key) <= ctx::turn_key()
}

/// 「初始0，上限4」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// 状态1's offer, on the *other* player's turn start.
/// `actor != owner && slot('skillState') != 2` is the pre and
/// [`offer_support_open`] the residual (the cooldown latch).
fn offer_support(player_id: i32) -> card_sdk::Asked {
    let other = ctx::trigger::player_id();
    if !ctx::ask_yes(
        other,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_offer"))
            .player_id("who", player_id)
            .i("n", 400),
    )? {
        return Ok(());
    }
    if ctx::transfer(other, player_id, 400, &Msg::new(key!("kaede_support_paid")))? == 0 {
        return Ok(());
    }
    // 「指定一个属于你的格子」
    let mine_tiles = ctx::owned_tiles(player_id);
    if mine_tiles.is_empty() {
        return Ok(());
    }
    let tile = ctx::ask_tile(
        other,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_which")),
        &mine_tiles,
    )?;
    // 「使其立即对前后一格内的一名玩家进行一次半价[结算]」
    let mut near: Vec<i32> = Vec::new();
    for t in [tile, tile - 1, tile + 1] {
        if t < 0 || t >= ctx::tile_count() {
            continue;
        }
        for p in ctx::players_on(t, player_id) {
            if !near.contains(&p) {
                near.push(p);
            }
        }
    }
    if near.is_empty() {
        return Ok(());
    }
    let who = ctx::ask_player(
        other,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_who")),
        &near,
    )?;
    let before = ctx::money_of(player_id);
    plan::set_pay_factor(50);
    ctx::card_settle_at(who, tile, true);
    // 「并获得相当于你获取数额的资金」
    let got = (ctx::money_of(player_id) - before).max(0);
    if got > 0 {
        ctx::gain(player_id, got, &Msg::new(key!("kaede_support_gain")))?;
    }
    // 「直到这两名玩家的下回合结束…不会被该效果再次收款」
    let until = ctx::turn_key() + 2;
    state::set(player_id, &format!("{}{}", COOLDOWN, other), until);
    state::set(player_id, &format!("{}{}", COOLDOWN, who), until);
    // A press resets the rest counter.
    state::set(player_id, REST, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("kaede_support_done"))
            .tile("tile", tile)
            .player_id("who", who),
    );
    Ok(())
}

/// 「每两个回合没有玩家发动该技能，你在回合开始时获得1火罐…若达到上限，可在你的
/// 回合开始时选择进入状态2」.
/// `actor == owner && slot('skillState') != 2` is the pre.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    let n = state::get(player_id, REST) + 1;
    state::set(player_id, REST, n);
    if n >= 2 {
        state::set(player_id, REST, 0);
        bandori_std::fire::gain(player_id, 1, &Msg::new(key!("kaede_support_gain_charge")))?;
    }
    if state::get(player_id, state_key::CHARGE) < state::max(player_id, state_key::CHARGE) {
        return Ok(());
    }
    if ctx::ask_yes(
        player_id,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_enter")),
    )? {
        state::set(player_id, state_key::SKILL_STATE, 2);
        ctx::log(player_id, &Msg::new(key!("kaede_support_two")));
    }
    Ok(())
}

fn at_turn_end(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

/// 状态2（2）: 「进入状态2时你可消耗X个火罐，指定其他人的X个地契」.
fn can_enter_two(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::SKILL_STATE) != 2 {
        return Some(Msg::new(key!("kaede_support_not_two")));
    }
    if state::get(player_id, state_key::CHARGE) < 1 {
        return Some(Msg::new(key!("kaede_support_no_fire")));
    }
    None
}

fn enter_two(player_id: i32) -> card_sdk::Asked {
    // 「你可消耗X个火罐，指定其他人的X个地契」 -- X is however many pots the
    // player spends, one deed each.
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("kaede_support_title")),
        &Msg::new(key!("kaede_support_how_many")),
        1,
        state::get(player_id, state_key::CHARGE),
    )?;
    if x <= 0 {
        return Ok(());
    }
    if !bandori_std::fire::spend(player_id, x, &Msg::new(key!("kaede_support_spend")))? {
        return Ok(());
    }
    for _ in 0..x {
        let theirs = other_deeds(player_id);
        if theirs.is_empty() {
            break;
        }
        let t = ctx::ask_tile(
            player_id,
            &Msg::new(key!("kaede_support_title")),
            &Msg::new(key!("kaede_support_deed")),
            &theirs,
        )?;
        let owner = ctx::tile_owner(t);
        if owner < 0 {
            continue;
        }
        // 「每张地契向对应玩家支付100+n*100资金（n为对应格子上的房屋数）」
        let n = ctx::houses_of(t);
        ctx::transfer(
            player_id,
            owner,
            100 + n * 100,
            &Msg::new(key!("kaede_support_buy_in")),
        )?;
        // 「直到状态2结束为止，你与对方均分那些地契收取的资金」
        state::set(player_id, &format!("{}{}", SPLIT, t), owner);
        // 规则书[状态2]: 「指定其他人的X个地契…直到状态2结束为止」 -- the lasting
        // [指定] while this skill is in 状态2 is a designation mark per deed
        // (displayed record). The SPLIT key stays the functional storage: it also
        // carries the counterpart `owner`, a value the mark does not hold.
        //
        // TODO(规则书): the designation (and the SPLIT key) outlives 状态2.
        // The only exit is the band skill's fire-pot drain
        // (`skill_groups::ave_mujica` `at_turn_end`, SKILL_STATE 2 -> 1), which
        // does not clear the SPLIT keys -- and that rule cannot see them. The
        // split itself is gated on `slot('skillState') == 2`, so behaviour is
        // unchanged; the marks (and keys) linger as stale labels until this
        // skill instance is destroyed. Clearing them needs a hook on the state
        // exit (or an undesignate pass here on a turn-end check), which the
        // sheet's 「直到状态2结束为止」 does not yet pin down.
        ctx::designate_tile(
            t,
            player_id,
            &Msg::new(key!("kaede_support_deed")),
        );
    }
    // 「每通过此法消耗4个火罐，你抽1卡」
    let draws = x / 4;
    if draws > 0 {
        ctx::draw(player_id, draws)?;
        ctx::log(
            player_id,
            &Msg::new(key!("kaede_support_drew")).i("n", draws as i64),
        );
    }
    Ok(())
}

fn other_deeds(player_id: i32) -> Vec<i32> {
    let mut out: Vec<i32> = Vec::new();
    for p in ctx::others(player_id) {
        out.extend(ctx::owned_tiles(p));
    }
    out
}

// 「你与对方均分那些地契收取的资金」 -- the split is a bend on the rent a named
// deed collects, which is the `paySplitMul` window with `t.Pay.IsRent`. 状态2's exit
// is the band skill's fire-pot drain (`skill_groups::ave_mujica`), so the split
// runs only while `skillState == 2`.
// TODO(规则书)[judgement]: 「均分」 -- the clause does not say whether it divides
//   the rent figure before or after any other bend, nor what happens on an odd
//   amount. Read here as a clean split of the settled rent with the remainder to
//   the deed's owner.
//
// TODO(规则书)[judgement]: 状态2's exit -- the sheet gives none (「直到状态2结束
//   为止」 with no clause that ends it). Contrast 三角初华's fire-pot cap and
//   若叶睦's hand-size rule. Without an exit, the split is permanent once named.
//   (The band skill's fire-pot drain is the engine's stand-in exit; it does not
//   clear the SPLIT keys or the designation marks -- see `enter_two`.)

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn offer_support_kani(player_id: i32) -> card_sdk::Asked {
    offer_support(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn enter_two_kani(player_id: i32) -> card_sdk::Asked {
    enter_two(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn split_rent_kani(player_id: i32, value: i32) -> i32 {
    split_rent(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn split_rent_gain_kani(player_id: i32) -> card_sdk::Asked {
    split_rent_gain(player_id)
}
