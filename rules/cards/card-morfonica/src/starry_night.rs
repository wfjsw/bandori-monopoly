//! `Mor:蝴蝶飞舞的星月夜` -- C# `CardStarryNight` (MatchHost.cs:4666-4778): pay X
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:蝴蝶飞舞的星月夜`）:
//! > 蝴蝶飞舞的星月夜：
//!
//! > （1） 支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]，此卡没有[奇迹水晶]时加入弃牌堆; 此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次
//!
//! > （2）当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个[奇迹水晶]，此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]。
//!
//! > （3）此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。
//!
//! ×1000 to stock X crystals; each removal pays the owner 1,000.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, TriggerKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "Mor:蝴蝶飞舞的星月夜";

pub const STARRY_NIGHT: CardDef = CardDef::new(
    "Mor:蝴蝶飞舞的星月夜",
    &[
        // NEGATION-AUDIT V2 (settled): no activation cost. The 「支付X次1000
        // 的资金」 is effect content (rulebook L13-14), so there is no
        // `money >= 1000` play gate -- an unaffordable in-body payment takes
        // the Q1 shortfall path. C# `CardStarryNight.WhyNot` had one.
        On::Play("", None, starry_night),
        // 规则书（1）: 「此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次」 and
        // 规则书（2）: 「当其他角色的移动掷骰结果是偶数时」 -- both arrive here.
        On::Action(Action::DiceRollAfter("card.placed && card.counter('crystals') > 0", None, hook)),
        // 规则书（2）: 「此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]」
        On::Action(Action::PassTile("card.placed && card.counter('crystals') > 0", None, hook)),
        // 规则书（3）: 「此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金」 -- and
        // 规则书（1）: 「此卡没有[奇迹水晶]时加入弃牌堆」.
        On::Action(Action::CounterChanged("", Some(crystals_changed_guard), on_crystals_changed)),
    ],
).props(&[(card_sdk::abi::prop::EST_COST, 1000)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"蝴蝶飞舞的星月夜"#)
        .raw_name(r#"蝴蝶飞舞的星月夜"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]，此卡没有[奇迹水晶]时加入弃牌堆; 此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次"#),
            it(r#"当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个[奇迹水晶]，此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]。"#),
            it(r#"此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。"#),
        ]))
        // sheet cell G5
        .fields(&[("group", r#"Morfonica"#), ("owner", "")]));

fn starry_night(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「支付X次1000的的资金」 -- C# `H.AskNumber(..., 1, max)` with
    // `max = Math.Max(1, Math.Min(5, money / 1000))`. C# is not the spec:
    // TODO(规则书)（支付X次1000的的资金）: the book prints no bound for X -- the `5`
    //   is C#'s `Math.Min(5, ...)` (OPEN-X), and the `money / 1000` cash
    //   pre-cap is a leftover of the removed play gate (`ctx::pay` takes the
    //   shortfall path).
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("starry_night_ask_title")),
        &Msg::new(key!("starry_night_ask_text")),
        1,
        5,
    )?;
    // 规则书（1）: 「支付X次1000的的资金」 -- C# `PayCtx { amount = 1000 * x, kind = "pay", must = false }`.
    let paid = ctx::pay(
        player_id,
        1000 * x,
        &Msg::new(key!("starry_night_why")).i("n", x as i64),
    )?;
    if paid < 1000 * x {
        // C# `c.Effective = false` when the payment does not go through.
        // TODO(规则书)[judgement]: 「视为此卡未生效」 -- the clause names a state without
        // saying what observes it. `PlayCtx.Effective = false` is the C#'s mutable
        // side channel and is not being ported (a routine should *return* whether
        // it took effect); but before that lands, what "not effective" changes has
        // to be ruled: does the card get spent (haneoka 「放入弃牌堆且视为此卡未生效」
        // says yes) or not (noble_blue / starry_night's "the card is spent anyway"
        // implies no)? And what counts a use that this would suppress?
        return Ok(());
    }
    // 规则书（1）: 「将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]」
    // TODO(规则书)（将此卡放置在场地中央）: 「场地中央」 is not a board slot -- the
    //   card is a field card of the owner (OPEN-场地中央).
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        ID,
        &Msg::new(key!("starry_night_note")).i("n", x as i64),
    );
    ctx::add_crystals(x, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("starry_night_placed"))
            .player_id("who", player_id)
            .i("n", x as i64),
    );
    // 规则书（1）: 「此卡没有[奇迹水晶]时加入弃牌堆」 and 规则书（2）/（3） -- the
    // DiceRollAfter / PassTile hooks below handle the reroll, the even-roll attack and
    // the CiRCLE decay; [`on_crystals_changed`] pays out each removal and
    // discards at 0.
    Ok(())
}

/// C# `CardStarryNight.Remove` -- burn one crystal. The payout and the
/// discard ride the write: [`on_crystals_changed`] is 规则书（3） and（1）.
fn remove_crystal(_player_id: i32) -> card_sdk::Asked {
    ctx::remove_crystal()?;
    Ok(())
}

/// 规则书（3）: 「此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金」, and
/// 规则书（1）: 「此卡没有[奇迹水晶]时加入弃牌堆」 -- C# `CardStarryNight.Remove`
/// paid and discarded at the spend site, which only covered the two sites that
/// called it. Both clauses here catch a count changed by *any* write.
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all. Name-filtered to `counter::CRYSTALS`.
fn crystals_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        // TODO(ABI) trigger uid: `trigger::card_is` is name-only and the hook
        //   dispatch has no per-uid dedupe, so with two copies of this card on
        //   one field a single crystal removal runs both copies' payout.
        && trigger::card_is(ID)
        && trigger::name() == bandori_std::counter::CRYSTALS
        // （3） pays for removals; （1） leaves when the count lands on 0.
        && (trigger::value() < 0 || ctx::crystals() == 0)
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    // 规则书（3）: 「每个[奇迹水晶]移除时」 -- one payout per crystal removed.
    let removed = (-trigger::value()).max(0);
    if removed > 0 {
        ctx::gain(
            player_id,
            1000 * removed,
            &Msg::new(key!("starry_night_remove")).player_id("who", player_id),
        )?;
    }
    // 规则书（1）: 「此卡没有[奇迹水晶]时加入弃牌堆」.
    if ctx::crystals() == 0 && trigger::value() <= 0 {
        ctx::set_dest(ctx::Dest::Graveyard);
        ctx::log(
            player_id,
            &Msg::new(key!("starry_night_empty")).player_id("who", player_id),
        );
    }
    Ok(())
}

/// `Fx.DiceRollAfter` / `Fx.PassTile` (C# `CardStarryNight.DiceRollAfter` / `PassTile`).
/// Runs through the Fx hook dispatch, so these are field effects, not [反击].
fn hook(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        TriggerKind::DiceRollAfter => {
            let roller = trigger::player_id();
            let roll = trigger::move_roll().unwrap_or_else(trigger::value);
            if roller == player_id {
                // 规则书（1）: 「你每次移动掷骰时可以放弃第一次的结果重骰一次」 --
                // C# `!m.Main || m.FixedRoll >= 0`: only the main move's own free
                // roll. The guard belongs to the reroll alone; (2)'s attack below
                // carries no such qualifier.
                if !trigger::move_is_main() || ctx::fixed_roll().is_some() {
                    return Ok(());
                }
                reroll(player_id, roll)?;
            } else if roll % 2 == 0 {
                // 规则书（2）: 「当其他角色的移动掷骰结果是偶数时」.
                attack(player_id, roller, roll)?;
            }
        }
        TriggerKind::PassTile => {
            // 规则书（2）: 「此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]」 -- C#
            // `m.Seat == Seat && H.Tile(t)?.kind == "circle" && Crystals > 0`.
            if trigger::player_id() == player_id && ctx::is_start(trigger::tile()) {
                ctx::log(
                    player_id,
                    &Msg::new(key!("starry_night_circle")).player_id("who", player_id),
                );
                remove_crystal(player_id)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// 规则书（1）: 「此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次」 -- C#
/// `CardStarryNight.Reroll`: ask (the timeout falls back to no), then `H.DoMoveRoll`.
fn reroll(player_id: i32, roll: i32) -> card_sdk::Asked {
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("starry_night_reroll_title")),
        &Msg::new(key!("starry_night_reroll_ask")).i("roll", roll as i64),
    )? {
        return Ok(());
    }
    // 规则书: the C# rerolls with `H.DoMoveRoll`, which sums the move's whole
    // dice table (base + extra dice + flat bonuses) rather than a bare 1d20.
    // TODO(ABI) dice value stage: `do_move_roll` sums the raw tables and the
    //   new face never passes `diceRollValue`, so 「最终结算」 modifiers (AG:（摩卡）0.5倍速)
    //   are not re-applied -- needs an export that re-runs the value stage.
    let new_roll = ctx::do_move_roll(player_id)?;
    trigger::set_move_roll(new_roll);
    ctx::log(
        player_id,
        &Msg::new(key!("starry_night_rerolled"))
            .player_id("who", player_id)
            .i("from", roll as i64)
            .i("to", new_roll as i64),
    );
    Ok(())
}

/// 规则书（2）: 「当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个
/// [奇迹水晶]」 -- the omitted subject of 「可消耗」 is this card's controller
/// (the same 「你」 that owns the card and receives （3）'s payout), not the
/// roller. C# `CardStarryNight.Attack` billed the roller; that is not what the
/// sheet says. With （3） paying 1,000 per crystal the owner is out net 0 and
/// the crystal count drops -- a voluntary decay of this card.
fn attack(player_id: i32, roller: i32, roll: i32) -> card_sdk::Asked {
    // 规则书（2）: 「可消耗1000资金」 prints no cash-on-hand rider -- a short
    // owner is still offered the removal; `ctx::pay` raises funds (mortgage)
    // when it must, and `paid < 1000` below skips the crystal.
    if ctx::player_out(player_id) || !rules_status::can_pay(player_id) {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("starry_night_attack_title")),
        &Msg::new(key!("starry_night_attack_ask"))
            .card("card", ID)
            .player_id("who", roller)
            .i("roll", roll as i64),
    )? {
        return Ok(());
    }
    let paid = ctx::pay(
        player_id,
        1000,
        &Msg::new(key!("starry_night_attack_pay")).player_id("who", player_id),
    )?;
    if paid >= 1000 {
        remove_crystal(player_id)?;
        ctx::log(
            player_id,
            &Msg::new(key!("starry_night_attack_done"))
                .player_id("who", player_id)
                .player_id("owner", player_id)
                .player_id("roller", roller),
        );
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn starry_night_kani(player_id: i32) -> card_sdk::Asked {
    starry_night(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn hook_kani(player_id: i32) -> card_sdk::Asked {
    hook(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
