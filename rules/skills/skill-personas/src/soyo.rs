//! `skill:长崎素世:通透的颜色`
//!
//! 规则书（skill sheet, 长崎素世）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
//! > （2）移动掷骰后可消耗一个火罐，使自己本回合的[移动终点]对你视为对应颜色
//! > 的地产商格子，持续至你的下回合开始；若以此法单次免除了至少1500资金的
//! > [支付]，则你下次经过CiRCLE时不获得火罐。
//!
//! （2） is a re-colour of the landing tile *for this player*, which is the
//! `colorFor:<p>` tile prop. 「对应颜色」 is the colour of the 地产商 tile being
//! settled, which is what `is_live_house_for` already reads -- the clause names
//! the colour group of the agent, so the landing counts as that group.
//!
//! The penalty half -- 「若以此法单次免除了至少1500资金的[支付]，则你下次经过
//! CiRCLE时不获得火罐」 -- is a latch on a saving the re-colour actually caused.
//! 「以此法免除」 is attributed: the settle latches what the ordinary property
//! landing would have charged and compares it with what the dealer settle
//! actually took; a single saving of 1500 or more latches the penalty, which
//! `on_pass` consumes.

use bandori_std::ctx::{self, state};
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

/// The re-colour is armed for this turn's landing.
const ARMED: &str = "skill.soyoClear.armed";
/// 「下次经过CiRCLE时不获得火罐」 -- the penalty latch.
const PENALTY: &str = "skill.soyoClear.penalty";
/// The tile the `colorFor:<p>` re-colour sits on, stored as `tile + 1` so 0
/// means "none". Cleared at the player's next turn start.
const COLOURED: &str = "skill.soyoClear.coloured";
/// The payment the landing would have charged without the re-colour (the
/// attribution baseline); 0 when there is nothing to waive.
const WAIVED: &str = "skill.soyoClear.waived";
/// Money at the moment the re-coloured settle starts, to measure what was
/// actually paid.
const WAS: &str = "skill.soyoClear.was";

pub const SOYO: CardDef = CardDef::new(
    "skill:长崎素世:通透的颜色",
    &[
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        On::Action(Action::Pass(bandori_std::guards::OWN_START, None, on_pass)),
        // （2）「移动掷骰后可消耗一个火罐」 -- only with a pot to spend, and
        // only on a free roll.
        On::Action(Action::DiceRollAfter(
            "actor == owner && charge(owner) >= 1 && plan.fixed_roll == null",
            None,
            offer,
        )),
        // 「若以此法单次免除了至少1500资金的[支付]」 needs the money *before*
        // the settle moved it: the re-coloured settle's dealer leg commits
        // while the `Settle` body is parked, so reading it there would already
        // include the dealer's charge. Silent bookkeeping.
        On::Action(Action::SettleBefore(
            "actor == owner && slot('skill.soyoClear.armed') != 0",
            None,
            arm_waiver,
        ))
        .silent(),
        On::Action(Action::Settle(
            "actor == owner && slot('skill.soyoClear.armed') != 0",
            None,
            on_settle,
        )),
        // The re-coloured settle has finished: attribute the waiver. `coloured`
        // pins this to the landing the re-colour wrote -- the nested dealer
        // settle has its own `settleAfter` on the agent tile.
        On::Action(Action::SettleAfter(
            "actor == owner && slot('skill.soyoClear.coloured') == tile.id + 1 && slot('skill.soyoClear.waived') > 0",
            None,
            after_settle,
        )),
    ],
)
.rule()
// 「（初始1，上限1）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 1),
    (bandori_std::fire::FIRE_CAP, 1),
]);

/// 「初始1，上限1」 -- and the re-colour's expiry.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    // 规则书（2）: 「持续至你的下回合开始」 -- the `colorFor:<p>` write ends at
    // the player's next turn start. Silent bookkeeping (this entry is on the
    // `silent` list -- the expiry is not a printed effect to narrate).
    let remembered = state::get(player_id, COLOURED);
    if remembered > 0 && ctx::trigger::player_id() == player_id {
        state::set(player_id, COLOURED, 0);
        ctx::set_tile_prop(
            remembered - 1,
            &alloc::format!("{}{}", card_sdk::abi::prop::COLOR_FOR_PREFIX, player_id),
            0,
        );
        ctx::set_tile_prop(remembered - 1, &dealer_key(player_id), 0);
    }
    Ok(())
}

/// [`dealer_key`]: the per-player 「视为地产商格子」 class mark.
fn dealer_key(player_id: i32) -> alloc::string::String {
    alloc::format!("classFor:{player_id}:dealer")
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」, minus the penalty latch.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, PENALTY) != 0 {
        state::set(player_id, PENALTY, 0);
        ctx::log(player_id, &Msg::new(key!("soyo_denied")));
        return Ok(());
    }
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("soyo_gain")))?;
    Ok(())
}

/// （2）「移动掷骰后可消耗一个火罐」 -- the moment the face exists.
fn offer(player_id: i32) -> card_sdk::Asked {
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("soyo_title")),
        &Msg::new(key!("soyo_ask")),
    )? {
        return Ok(());
    }
    if bandori_std::fire::spend_skill(player_id, 1, &Msg::new(key!("soyo_spend")))? {
        state::set(player_id, ARMED, 1);
    }
    Ok(())
}

/// The waiver baseline: the money at the settle window's start, before any
/// payment this settle makes (read by [`after_settle`]).
fn arm_waiver(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, WAS, ctx::money_of(player_id));
    Ok(())
}

/// （2）「使自己本回合的[移动终点]对你视为对应颜色的地产商格子，持续至你的下回合
/// 开始」 -- applied at the settle, when the landing tile is known. The
/// `colorFor:<p>` write is cleared at the player's next turn start
/// ([`declare_cap`]).
///
/// 「视为…地产商格子」 is more than a colour: the endpoint resolves as that
/// colour's dealer landing. The landing carries the per-player `dealer`
/// class, the normal property body stands down
/// (`rules/tiles/src/property.rs`), and the dealer body runs here against the
/// agent tile of the landing's colour. The payment the landing would have
/// charged is latched first so [`after_settle`] can attribute the saving.
fn on_settle(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, ARMED, 0);
    // A stale baseline (the payer went out before `settleAfter`) must not leak
    // into a later settle.
    state::set(player_id, WAIVED, 0);
    let t = ctx::trigger::tile();
    if t < 0 {
        return Ok(());
    }
    // 「对应颜色」 -- the landing's own colour group is what the dealer landing
    // is of. A colourless tile has none and the re-colour is a no-op.
    let g = ctx::tile_group(t);
    if g < 0 {
        return Ok(());
    }
    // 「对你视为对应颜色的地产商格子」 -- the `colorFor:<player_id>` tile prop
    // on the landing tile's board instance.
    ctx::set_tile_prop(
        t,
        &alloc::format!("{}{}", card_sdk::abi::prop::COLOR_FOR_PREFIX, player_id),
        g,
    );
    // Remember it as `tile + 1` so `declare_cap` can expire it at the player's
    // next turn start (0 = none).
    state::set(player_id, COLOURED, t + 1);
    ctx::log(player_id, &Msg::new(key!("soyo_coloured")).tile("tile", t));
    // The endpoint already is that colour's dealer tile: its own body is the
    // dealer body, nothing to reroute.
    if bandori_std::is_agent(t) {
        return Ok(());
    }
    let Some(agent) = agent_of(g) else {
        return Ok(());
    };
    ctx::set_tile_prop(t, &dealer_key(player_id), 1);
    // Attribution baseline: the ordinary property settling of this landing.
    // `WAS` was captured at `settleBefore`, before any money moved.
    state::set(player_id, WAIVED, would_pay(player_id, t));
    ctx::settle_tile_at(player_id, agent, false);
    Ok(())
}

/// The agent tile of `group`, or `None`.
fn agent_of(group: i32) -> Option<i32> {
    (0..ctx::tile_count()).find(|&t| bandori_std::is_agent(t) && ctx::tile_group(t) == group)
}

/// What the ordinary property settle of `t` would charge `player_id` -- the
/// baseline for 「以此法免除」. Unowned (a purchase) and owned (a build) are
/// payments too; a mortgaged other-player deed is the force-buy price
/// (规则书: 「总价的两倍」).
fn would_pay(player_id: i32, t: i32) -> i32 {
    let owner = ctx::tile_owner(t);
    if owner < 0 {
        return ctx::buy_price(t);
    }
    if owner == player_id {
        return if ctx::can_build_on(player_id, t) {
            ctx::build_cost(t)
        } else {
            0
        };
    }
    if ctx::mortgaged_of(t) {
        return ctx::buy_price(t) * 2;
    }
    ctx::rent_of(t)
}

/// 「若以此法单次免除了至少1500资金的[支付]」 -- the baseline minus what the
/// dealer settle actually took; a net saving of 1500 or more latches the
/// penalty. The re-coloured settle is the only one whose `settleAfter` sees a
/// live baseline: the nested dealer settle's own `settleAfter` runs while the
/// parked `Settle` body's writes are rolled back (`waived` 0 there), and its
/// tile is the agent's, not the `coloured` landing.
fn after_settle(player_id: i32) -> card_sdk::Asked {
    let normal = state::get(player_id, WAIVED);
    let was = state::get(player_id, WAS);
    state::set(player_id, WAIVED, 0);
    let saved = normal - (was - ctx::money_of(player_id));
    if saved >= 1500 {
        state::set(player_id, PENALTY, 1);
        ctx::log(
            player_id,
            &Msg::new(key!("soyo_waived")).i("n", saved as i64),
        );
    }
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
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
pub fn offer_kani(player_id: i32) -> card_sdk::Asked {
    offer(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_settle_kani(player_id: i32) -> card_sdk::Asked {
    on_settle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn after_settle_kani(player_id: i32) -> card_sdk::Asked {
    after_settle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn arm_waiver_kani(player_id: i32) -> card_sdk::Asked {
    arm_waiver(player_id)
}
