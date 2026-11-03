//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（NNM）稍微努力了一下`）:
//! > （NNM）稍微努力了一下：
//! > 弃置手中x枚角色标记，发动以下效果中的一个：
//!
//! > （1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张
//!
//! > （2）获得x次经过CiRCLE时的资金奖励
//!
//! > （3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的
//! > （2）技能，此次技能不受数量或轮数限制
//!
//! > （4）当奇迹水晶耗尽时，将此卡置入弃牌堆
//!
//! x character tokens for one of three effects.

use bandori_std::ctx::{self, trigger, CardPile};
use card_sdk::abi::REWARD_MONEY;
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

/// 「[获得]2000资金」 (专有名词 11) -- the default CiRCLE money reward. A
/// `passRewardAffected` listener may replace it (Morfonica's cycle).
const CIRCLE_MONEY: i32 = 2000;

const ID: &str = "Mor:（NNM）稍微努力了一下";
/// The counter names 「角色标记:*」.
const TOKEN_PREFIX: &str = "角色标记:";

pub const NANAMI_EFFORT: CardDef = CardDef::new(
    "Mor:（NNM）稍微努力了一下",
    &[
        // The Play is the hand placement; (3)'s 「你可以移除一个奇迹水晶」 is a
        // sub-choice of the body that just placed the card -- not an entry of
        // its own, and not a press surface.
        On::Play("", Some(cant_play), nanami_effort),
        On::Schedule("discard_down_to_five", "", None, discard_down_to_five),
        // （4）「当奇迹水晶耗尽时，将此卡置入弃牌堆」 -- the same CounterChanged
        // shape as R:Fire bird (identity check in the residual, not the condition).
        On::Action(Action::CounterChanged("actor == owner && card.placed && counter_is('crystals') && card.counter('crystals') == 0 && value <= 0", Some(crystals_changed_guard), on_crystals_changed)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"稍微努力了一下"#)
        .raw_name(r#"（NNM）稍微努力了一下"#)
        .tags(&[])
        .text(CardText::of(&[
            sec("", &[
                para(r#"弃置手中x枚角色标记，发动以下效果中的一个："#),
                items(&[
                    it(r#"抽x张卡（可超过上限），回合结束后将手牌弃置到五张"#),
                    it(r#"获得x次经过CiRCLE时的资金奖励"#),
                    it("将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的\n（2）技能，此次技能不受数量或轮数限制"),
                    it(r#"当奇迹水晶耗尽时，将此卡置入弃牌堆"#),
                ]),
            ]),
        ]))
        // sheet cell G7
        .fields(&[("group", r#"Morfonica"#), ("owner", r#"广町七深"#)]));

/// The x-标记 body needs at least one 「角色标记:*」; everything (3) later needs
/// is checked inline, where it is asked.
fn cant_play(player_id: i32) -> Option<Msg> {
    let names = ctx::held_names(player_id, TOKEN_PREFIX);
    if names
        .iter()
        .any(|n| ctx::held_count(player_id, n.as_str()) > 0)
    {
        return None;
    }
    Some(Msg::new(key!("nanami_effort_no_tok")))
}

fn nanami_effort(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「弃置手中x枚角色标记，发动以下效果中的一个」 -- the tokens are the
    // `角色标记:*` counters; x is chosen from what the player actually holds, then
    // they are spent and one effect is picked.
    let names = ctx::held_names(player_id, TOKEN_PREFIX);
    let total: i32 = names
        .iter()
        .map(|n| ctx::held_count_id(player_id, *n))
        .sum();
    if total <= 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("nanami_effort_no_tok")).player_id("who", player_id),
        );
        return Ok(());
    }
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("nanami_effort_how_many")),
        &Msg::new(key!("nanami_effort_how_many_text")),
        1,
        total,
    )?;
    // 「弃置…x枚」 -- spend them lowest-index first until x is gone.
    let mut left = x;
    for name in &names {
        if left <= 0 {
            break;
        }
        let take = ctx::held_count_id(player_id, *name).min(left);
        if take > 0 {
            ctx::add_held_id(player_id, *name, -take, i32::MAX)?;
            left -= take;
        }
    }
    // 「发动以下效果中的一个」
    let k = ctx::ask_choice(
        player_id,
        &Msg::new(key!("nanami_effort_pick_title")),
        &Msg::new(key!("nanami_effort_pick_text")),
        &[
            Msg::new(key!("nanami_effort_effect_1")),
            Msg::new(key!("nanami_effort_effect_2")),
            Msg::new(key!("nanami_effort_effect_3")),
        ],
    )?;
    match k {
        // (1) 「抽x张卡（可超过上限）」 -- drawn now in `effect_draw`; only
        // 「回合结束后将手牌弃置到五张」 is scheduled.
        0 => effect_draw(player_id, x)?,
        // 规则书（2）: 「获得x次经过CiRCLE时的资金奖励」 -- Sheet 2026-10-06 新卡组卡 G7.
        // x times the CiRCLE pass *money* reward, not a flat x*2000. Each
        // time runs the money half of the reward step (`passRewardAffected`
        // between pick and payout), so a listener that rewrites the amount
        // (Morfonica's 1000/1500/2000 cycle) scales this too.
        1 => {
            for _ in 0..x {
                gain_circle_money(player_id)?;
            }
        }
        // (3) 「将此卡放置在场上并放置x个奇迹水晶」 -- placed with x crystals
        // on it; (4) is its exhaustion.
        2 => place_with_offer(player_id, x)?,
        _ => {}
    }
    Ok(())
}

/// 规则书 (3): 「将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶
/// 视为发动你的（2）技能」. The placement is unconditional; the charge is the
/// offer's yes.
fn place_with_offer(player_id: i32, x: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("nanami_effort_note")));
    ctx::add_crystals(x, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("nanami_effort_placed")).i("n", x as i64),
    );
    // 规则书（3）: 「你可以移除一个奇迹水晶视为发动你的（2）技能」 -- asked,
    // not assumed, and asked before the crystal goes so a misoffer cannot
    // burn it. `persona_skill` names the character's own skill card
    // (`bind_skills` placed it) -- a band attachment is also
    // `skill:<group>:<name>` and must not be picked. (2) is that
    // skill's `On::Action(CardUsed)` entry, addressed by its declared id.
    let Some(id) = ctx::persona_skill(player_id) else {
        return Ok(());
    };
    // (2) pays a pot inside its body, so a pot must exist before the
    // question is asked; without one the answer would be a dead end.
    if bandori_std::fire::total(player_id) < 1 {
        return Ok(());
    }
    if ctx::ask_yes(
        player_id,
        &Msg::new(key!("nanami_effort_ask_title")),
        &Msg::new(key!("nanami_effort_ask_text")),
    )? {
        ctx::add_crystals(-1, 0)?;
        ctx::log(
            player_id,
            &Msg::new(key!("nanami_effort_skill")).card("card", id.as_str()),
        );
        // 「视为发动你的（2）技能，此次技能不受数量或轮数限制」 -- `invoke_entry`
        // runs the body only, so the skill's own count/CD gate is never
        // consulted and the waiver is structural.
        ctx::invoke_entry(player_id, id.as_str(), "2")?;
    }
    Ok(())
}

/// One 「经过CiRCLE时的资金奖励」 -- the money half of the CiRCLE reward step.
/// `passRewardAffected` fires between the pick and the payout (value = 0 = money),
/// so a listener may replace the amount or cancel it outright; if it stands,
/// the default 专有名词 11 figure is paid as a `pass`-typed gain, like the
/// CiRCLE reward it copies.
fn gain_circle_money(player_id: i32) -> card_sdk::Asked {
    if ctx::raise(
        player_id,
        bandori_std::kind::PassRewardAffected,
        REWARD_MONEY,
    )? {
        ctx::gain_typed(
            player_id,
            CIRCLE_MONEY,
            "pass",
            &Msg::new(key!("nanami_effort_money")).i("n", CIRCLE_MONEY as i64),
        )?;
    }
    Ok(())
}

/// 规则书（4）: 「当奇迹水晶耗尽时，将此卡置入弃牌堆」 -- the exhaustion half of
/// effect (3). Residual guard: this card's own crystals counter hit 0.
fn crystals_changed_guard(_player_id: i32) -> bool {
    trigger::card_is(ID) && trigger::name() == bandori_std::counter::CRYSTALS
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("nanami_effort_exhausted")).player_id("who", player_id),
    );
    Ok(())
}

fn effect_draw(player_id: i32, x: i32) -> card_sdk::Asked {
    // 规则书（1）: 「抽x张卡（可超过上限）」 -- 「可超过上限」 is the hand limit
    // itself, so lift it for the draw and put it back (the limit is keyed state,
    // which is what `NoLimitFx` was an attachment for).
    // TODO(ABI): a guest state *increase* does not cross onto the live world for a
    //   host routine (docs/ENGINE.md 「Only decreases cross」), so the draw still
    //   trims to the old cap -- 「可超过上限」 needs a draw that skips the over-hand
    //   trim (or increases the host can see).
    let cap = ctx::state::get(player_id, card_sdk::abi::state_key::HAND_LIMIT);
    ctx::state::set(
        player_id,
        card_sdk::abi::state_key::HAND_LIMIT,
        card_sdk::abi::state_key::NO_HAND_LIMIT,
    );
    ctx::draw(player_id, x)?;
    ctx::state::set(player_id, card_sdk::abi::state_key::HAND_LIMIT, cap);
    ctx::before_turn_end(player_id, "discard_down_to_five");
    Ok(())
}

/// Scheduled by the play's effect (1) via `ctx::before_turn_end(player_id,
/// "discard_down_to_five")`.
fn discard_down_to_five(player_id: i32) -> card_sdk::Asked {
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    let mut extra = hand.len() as i32 - 5;
    for card in &hand {
        if extra <= 0 {
            break;
        }
        if ctx::discard_from_hand(player_id, card.as_str()) {
            extra -= 1;
        }
    }
    if extra < 0 {
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new(key!("nanami_effort_discard_down")).player_id("who", player_id),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    nanami_effort(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn nanami_effort_kani(player_id: i32) -> card_sdk::Asked {
    nanami_effort(player_id)
}

/// (3)'s sub-body alone: the token spend, the `x` pick and the effect pick
/// are outside, so the offer can be proven without them.
#[cfg(kani)]
#[inline(always)]
pub fn place_with_offer_kani(player_id: i32, x: i32) -> card_sdk::Asked {
    place_with_offer(player_id, x)
}

#[cfg(kani)]
#[inline(always)]
pub fn discard_down_to_five_kani(player_id: i32) -> card_sdk::Asked {
    discard_down_to_five(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
