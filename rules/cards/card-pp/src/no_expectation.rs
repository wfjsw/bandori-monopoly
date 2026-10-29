//! `PP:不要背负期待` -- C# `CardNoExpectation`: stay in play, 「共鸣」 joins the
//!
//! 规则书（docs/rulebook/cards.json, id `PP:不要背负期待`）:
//! > 不要背负期待：
//! > [特]：
//! > 此卡不受任何其他效果影响。
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]并将1张“共鸣”加入卡组，然后抽1张牌。
//! > [持续]：
//!
//! > （1）以下效果对[拥有者]持续生效：非回合开始时进行投掷的投掷结果减少2（如果是移动投掷则最终结果最小为0，如果为0则此次移动不[结算]）；手卡上限数量减1。
//!
//! > （2）以下效果此卡放入[场地]或每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次：[支付]资金时金额提高100；[收取]资金时金额减少100（最低0）。
//!
//! deck, draw 1. The roll shave and the pay bends live in the Fx hooks;
//! the reshuffle stack growth runs on `Reshuffled`.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{prop, ActionKind, FilterKind};
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, Action, CardDef, Filter, Msg, On, PayAddValue, RollValue};

pub const NO_EXPECTATION: CardDef = CardDef::new(
    "PP:不要背负期待",
    &[
        On::Play("", None, no_expectation),
        // ABI v74 Filter/Action split of the old shared `hook` body; v79
        // folded the shave + the 「为0则不[结算]」 clamp into one pure
        // FilterValue at DiceRollValue (option C -- the text is a result
        // modifier, not a 「掷骰后」 listener). No DiceRollAfter entry.
        // PaySplitAdd is the pure payment bend (filters may not write, so the
        // 「生效2次」 charges are spent by the `PayAfter` action below).
        //
        // TODO(ABI): DiceRollValue is raised only inside a move dispatch --
        // effect rolls (`ctx::roll`) and card-driven move rerolls never reach
        // this filter; both need a roll-value stage.
        // 规则书: 「非回合开始时进行投掷的投掷结果减少2」
        On::Filter(Filter::DiceRollValue("card.placed", None, roll_shave)),
        // 规则书: 「[支付]资金时金额提高100；[收取]资金时金额减少100（最低0）」
        On::Filter(Filter::PaySplitAdd("card.placed", None, pay_add)),
        // The bend applied to this money move -- spend the charges.
        On::Action(Action::PayAfter(
            "card.placed && ((actor == owner && target >= 0 && target != owner) \
             || (target == owner && actor >= 0 && actor != owner))",
            None,
            consume,
        ))
        .silent(),
        // 规则书: 「此卡放入[场地]或每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次」
        On::Action(Action::Reshuffled("actor == owner && card.placed", None, reshuffled)).silent(),
    ],
)
// 规则书[特]: 「此卡不受任何其他效果影响」 -- static [特] property, stamped at
// placement (`FieldCard.immune`). 规则书[持续]（1）: 「手卡上限数量减1」 -- the
// `handLimitDelta` property (C# `Card.HandLimitDelta`), continuous while this
// instance sits on the field.
.props(&[(prop::HAND_LIMIT_DELTA, -1), (prop::IMMUNE, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"不要背负期待"#)
        .raw_name(r#"不要背负期待"#)
        .tags(&[r#"手"#, r#"持续"#, r#"特"#])
        .text(CardText::of(&[
            sec(r#"[特]："#, &[
                para(r#"此卡不受任何其他效果影响。"#),
            ]),
            sec(r#"[手]："#, &[
                para(r#"将此卡放置在[使用者]的[场地]并将1张“共鸣”加入卡组，然后抽1张牌。"#),
            ]),
            sec(r#"[持续]："#, &[
                items(&[
                    it(r#"以下效果对[拥有者]持续生效：非回合开始时进行投掷的投掷结果减少2（如果是移动投掷则最终结果最小为0，如果为0则此次移动不[结算]）；手卡上限数量减1。"#),
                    it(r#"以下效果此卡放入[场地]或每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次：[支付]资金时金额提高100；[收取]资金时金额减少100（最低0）。"#),
                ]),
            ]),
        ]))
        // sheet cell D7
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]));

/// C# `Mem["stacks"]` -- how many times the pay bend is stacked (2 on place,
/// +2 per reshuffle). On the placed instance, so copies do not share a count.
const PROP_STACKS: &str = "no_expectation.stacks";

fn stacks() -> i32 {
    ctx::prop(PROP_STACKS)
}

fn no_expectation(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "PP:不要背负期待",
        &Msg::new(key!("no_expectation_note")),
    );
    // 规则书[持续]（2）: 「此卡放入[场地]……对[拥有者]生效2次」 -- C#
    // `H.PlaceFromPlay(c).Mem["stacks"] = 2`.
    // TODO(规则书)（此卡放入[场地]或每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次）:
    // 窗口的另一端（何时作废）未印（OPEN-window2）-- 现只在一次实际加减时耗尽
    ctx::set_prop(PROP_STACKS, 2);
    // 规则书[手]: 「并将1张“共鸣”加入卡组」
    ctx::add_to_dest(player_id, "PP:[衍生]共鸣", ctx::Dest::DeckRandom);
    ctx::log(
        player_id,
        &Msg::new(key!("no_expectation_added"))
            .player_id("who", player_id)
            .card("card", "PP:[衍生]共鸣"),
    );
    // 规则书[手]: 「然后抽1张牌」
    ctx::draw(player_id, 1)?;
    // 规则书[特]: 「此卡不受任何其他效果影响」 -- C# `Card.Immune`. A flag on the
    // card: effects that would touch it read `card_immune` and skip.
    ctx::set_self_immune(true);
    Ok(())
}

/// 规则书[持续]（1）: 「非回合开始时进行投掷的投掷结果减少2」 -- C#
/// `CardNoExpectation` (1): 「投掷结果减少2（…如果为0则此次移动不[结算]）」.
/// Pure RollValue at DiceRollValue (v79): shaves the face by 2 and, when the
/// result is 0, clears `resolve` so the walk does not settle. `card.placed`
/// is the pre; only the owner's rolls are shaved.
fn roll_shave(player_id: i32, v: RollValue) -> RollValue {
    // TODO(规则书)（非回合开始时进行投掷的投掷结果减少2）: 与减半卡（moca_half）的
    // 先后未定 -- 现在按摆放顺序折叠，未按「最终结果」复核（OPEN-ROLLRESHAPE-01）
    if trigger::player_id() != player_id {
        return v;
    }
    let roll = v.value;
    let cut = (roll - 2).max(0);
    if roll > 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("no_expectation_roll")).i("n", cut as i64),
        );
    }
    let mut out = RollValue { value: cut, ..v };
    if cut == 0 {
        // 「如果为0则此次移动不[结算]」 -- also when the face was already 0
        // (the clause keys on 「为0」, not on the shave having done work).
        out.resolve = Some(false);
        ctx::log(
            player_id,
            &Msg::new(key!("no_expectation_no_resolve")).player_id("who", player_id),
        );
    }
    out
}

/// 规则书[持续]（2）: 「[支付]资金时金额提高100；[收取]资金时金额减少100（最低0）」
/// -- C# `CardNoExpectation.PaySplitAdd`, once per stack. Pure Filter (ABI v74):
/// returns the bent figure.
fn pay_add(player_id: i32, v: PayAddValue) -> PayAddValue {
    let n = stacks();
    if n <= 0 {
        return PayAddValue {
            value: v.value,
            ..v
        };
    }
    let from = trigger::player_id();
    let to = trigger::target();
    if v.value <= 0 {
        return PayAddValue {
            value: v.value,
            ..v
        };
    }
    let bump = 100 * n;
    let out = if from == player_id && to >= 0 && to != player_id {
        // 规则书: 「[支付]资金时金额提高100」 -- between players only (bank excluded)
        v.value + bump
    } else if to == player_id && from >= 0 && from != player_id {
        // 规则书: 「[收取]资金时金额减少100（最低0）」 -- between players only
        (v.value - bump).max(0)
    } else {
        v.value
    };
    PayAddValue { value: out, ..v }
}

/// 规则书[持续]（2）: 「对[拥有者]生效2次」 -- this money move carried the bend,
/// so the charges are spent (a Filter body may not write, hence `PayAfter`).
fn consume(_player_id: i32) -> card_sdk::Asked {
    ctx::set_prop(PROP_STACKS, 0);
    Ok(())
}

/// C# `CardNoExpectation.Reshuffled` -- each time the owner's discard pile is
/// shuffled back into the draw pile, the pay/gain bend gains 2 more stacks.
/// 规则书[持续]（2）: 「每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次」
/// `actor == owner && card.placed` is the pre.
fn reshuffled(_player_id: i32) -> card_sdk::Asked {
    // TODO(规则书)（每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次）: 窗口何时作废
    // 未印（OPEN-window2）-- 现只在一次实际加减时耗尽
    ctx::set_prop(PROP_STACKS, stacks() + 2);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
// TODO(kani): no proof drives these yet -- clauses （1）/（2） (roll shave, pay
// bend) are unproved, unlike sibling cards whose exports have iff proofs.
#[cfg(kani)]
#[inline(always)]
pub fn no_expectation_kani(player_id: i32) -> card_sdk::Asked {
    no_expectation(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn roll_shave_kani(player_id: i32, v: card_sdk::RollValue) -> card_sdk::RollValue {
    roll_shave(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_add_kani(player_id: i32, v: card_sdk::PayAddValue) -> card_sdk::PayAddValue {
    pay_add(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn reshuffled_kani(player_id: i32) -> card_sdk::Asked {
    reshuffled(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn consume_kani(player_id: i32) -> card_sdk::Asked {
    consume(player_id)
}
