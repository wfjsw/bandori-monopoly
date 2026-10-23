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
//! What counts as 「以此法免除」 is not fully determined by the text: it says
//! the payment was avoided by *this* effect, and the engine does not currently
//! attribute a payment's reduction to one skill. See the TODO.

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

pub const SOYO_CLEAR: CardDef = CardDef::new(
    "skill:长崎素世:通透的颜色",
    &[
        On::Action(Action::TurnStartBefore("", None, None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, None, declare_cap)),
        On::Action(Action::Pass(
            bandori_std::guards::OWN_START,
            None,
            None,
            on_pass,
        )),
        // （2）「移动掷骰后可消耗一个火罐」 -- only with a pot to spend, and
        // only on a free roll.
        On::Action(Action::DiceRollAfter(
            "actor == owner && charge(owner) >= 1 && plan.fixed_roll == null",
            None,
            None,
            offer,
        )),
        On::Action(Action::Settle(
            "actor == owner && slot('skill.soyoClear.armed') != 0",
            None,
            None,
            on_settle,
        )),
    ],
)
.silent(&[0, 1])
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
    }
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」, minus the penalty latch.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, PENALTY) != 0 {
        state::set(player_id, PENALTY, 0);
        ctx::log(player_id, &Msg::new(key!("soyo_clear_denied")));
        return Ok(());
    }
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("soyo_clear_gain")))?;
    Ok(())
}

/// （2） 「移动掷骰后可消耗一个火罐」 -- the moment the face exists.
fn offer(player_id: i32) -> card_sdk::Asked {
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("soyo_clear_title")),
        &Msg::new(key!("soyo_clear_ask")),
    )? {
        return Ok(());
    }
    if bandori_std::fire::spend(player_id, 1, &Msg::new(key!("soyo_clear_spend")))? {
        state::set(player_id, ARMED, 1);
    }
    Ok(())
}

/// （2）「使自己本回合的[移动终点]对你视为对应颜色的地产商格子，持续至你的下回合
/// 开始」 -- applied at the settle, when the landing tile is known. The
/// `colorFor:<p>` write is cleared at the player's next turn start
/// ([`declare_cap`]).
fn on_settle(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, ARMED, 0);
    let t = ctx::trigger::tile();
    if t < 0 {
        return Ok(());
    }
    // 「对应颜色」 -- the colour of the 地产商 tile the settle is about. When
    // the landing *is* an agent tile that is its own group; otherwise the
    // clause has no colour to name and the re-colour is a no-op.
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
    ctx::log(
        player_id,
        &Msg::new(key!("soyo_clear_coloured")).tile("tile", t),
    );
    // TODO(规则书)[judgement]: 「若以此法单次免除了至少1500资金的[支付]，则你下次经过CiRCLE时
    //   不获得火罐」 -- the clause under-specifies -- the engine does not attribute
    //   a payment's reduction to one skill, so 「以此法免除」 has no reading that
    //   is both faithful and checkable. The penalty latch is wired in `on_pass`.
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
