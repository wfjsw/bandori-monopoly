//! `AG:绯红之魂`
//!
//! 规则书（docs/rulebook/cards.json, id `AG:绯红之魂`）:
//! > 绯红之魂：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]，然后选择[消耗]1到5次500资金并在这张卡上放置对应数量的[奇迹水晶]。
//! > [持续]：
//!
//! > （1）
//! > [反击][拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，此次[消耗]或[支付]金额减少1000（最少为0，若为[支付]则被[支付]玩家[获得]500资金）。
//!
//! > （2）[拥有者]使用自己原有的技能
//! > （2）时移除此卡的1个[奇迹水晶]。
//!
//! > （3）此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区。
//!
//! place with N crystals; spend a crystal to cut a [消耗] or [支付] by 1,000
//! (the payee gains 500 on a [支付]).
//!
//! Reviewed 10/23/2026

use bandori_std::ctx::{self, trigger};
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "AG:绯红之魂";

pub const CRIMSON_SOUL: CardDef = CardDef::new(
    "AG:绯红之魂",
    &[
        // > [手]：
        // > 将此卡放置在[使用者]的[场地]，然后选择[消耗]1到5次500资金并在这张卡上放置对应数量的[奇迹水晶]。
        On::Play("", None, play),

        // 规则书[持续]（1）: 「[拥有者]因导致的[消耗]或[支付]时可选择移除此卡的
        // 1个[奇迹水晶]」 -- the offer is to *remove a crystal*, so it exists
        // only with a crystal on the card.
        // Fully expressible (actor / placed / card.counter) -- G4.
        //
        // Rides `PaySplitChooseBefore` (v78), restoring the rulebook PaySplitChoose position:
        // the interactive -1000 rewrite sits immediately before the pure
        // `PaySplitChoose` filter stage.

        // > （1）
        // > [反击][拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，此次[消耗]或[支付]金额减少1000
        // > （最少为0，若为[支付]则被[支付]玩家[获得]500资金）。
        // TODO(规则书)（[拥有者]因导致的[消耗]或[支付]）: the sheet's 「因导致的」 lacks
        //   its antecedent -- we take the maximal "any owner 消耗/支付" reading
        //   (actor == owner).
        // TODO(规则书)（此次[消耗]或[支付]金额减少1000）: the sheet adds no amount
        //   condition and floors at 0, so a running 0 is still a payment to cut
        //   -- `value > 0` refuses the offer there, where a [支付] still owes
        //   the payee 500.
        On::Action(Action::PaySplitChooseBefore("actor == owner && card.placed && card.counter('crystals') > 0", None, pay_choose).counter_reason(key!("crimson_soul_when"))).label("（1）"),

        // > （2）[拥有者]使用自己原有的技能（2）时移除此卡的1个[奇迹水晶]。
        On::Action(Action::CardUsed("", Some(skill_used_guard), skill_used)).label("（2）"),

        // > （3）此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区。
        On::Action(Action::CounterChanged("", Some(crystals_changed_guard), on_crystals_changed)).label("（3）"),
    ],
)
.props(&[(card_sdk::abi::prop::EST_COST, 500)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"绯红之魂"#)
        .raw_name(r#"绯红之魂"#)
        .tags(&[r#"手"#, r#"反击"#, r#"持续"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"将此卡放置在[使用者]的[场地]，然后选择[消耗]1到5次500资金并在这张卡上放置对应数量的[奇迹水晶]。"#),
            ]),
            sec(r#"[持续]："#, &[
                items(&[
                    it(r#"[反击][拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，此次[消耗]或[支付]金额减少1000（最少为0，若为[支付]则被[支付]玩家[获得]500资金）。"#),
                    it(r#"[拥有者]使用自己原有的技能（2）时移除此卡的1个[奇迹水晶]。"#),
                    it(r#"此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区。"#),
                ]),
            ]),
        ]))
        // sheet cell C12
        .fields(&[("group", r#"Afterglow"#), ("owner", "")]));

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]，然后选择[消耗]1到5次500资金」
    // -- C# `H.PlaceFromPlay(c, -1, -1, num)`: the card is placed first.
    ctx::place_self_to_field(player_id, ID, &Msg::new(key!("crimson_soul_note")));
    // 规则书[手]: 「选择[消耗]1到5次500资金」 -- the printed range is **1到5**,
    // unconditionally. C# narrowed it to `Math.Max(1, min(5, money/500))`, a
    // leftover of the old money play gate; NEGATION-AUDIT V2 removed that gate,
    // so a shortfall is reachable at any n in 1..=5 and the choice must not be
    // pre-capped by cash on hand.
    let n = ctx::ask_number(
        player_id,
        &Msg::new(key!("crimson_soul_title")),
        &Msg::new(key!("crimson_soul_ask")),
        1,
        5,
    )?;
    let n = n.clamp(1, 5);
    // 规则书[手]: 「[消耗]1到5次500资金」 -- k separate 500 [消耗]s, so every
    // per-payment hook and [反击] window runs once per 500.
    for _ in 0..n {
        if ctx::player_out(player_id) {
            break;
        }
        ctx::pay(player_id, 500, &Msg::new(key!("crimson_soul_why")))?;
    }
    // 规则书[手]: 「并在这张卡上放置对应数量的[奇迹水晶]」.
    ctx::set_crystals(n);
    ctx::log(
        player_id,
        &Msg::new(key!("crimson_soul_placed")).player_id("who", player_id),
    );

    Ok(())
}

/// 规则书[持续]（1）: 「[拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，此次
/// [消耗]或[支付]金额减少1000」
fn pay_choose(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    // C# `H.AskYes(Seat, CardName, "要付 …：移除 1 个 [奇迹水晶] 让这次 −1,000 吗？…", r, p.amount >= 800)`.
    let to = trigger::target();
    let ask = if to >= 0 && !ctx::player_out(to) {
        Msg::new(key!("crimson_soul_pay_ask_to"))
            .n("n", amount as i64)
            .player_id("who", to)
    } else {
        Msg::new(key!("crimson_soul_pay_ask")).n("n", amount as i64)
    };

    // TODO: We get to move this to the guard.
    if !ctx::ask_yes(player_id, &Msg::new(key!("crimson_soul_title")), &ask)? {
        return Ok(());
    }

    // 规则书[持续]（1）: 「若为[支付]则被[支付]玩家[获得]500资金」 -- the 500 is
    // owed the moment the crystal is spent, not conditioned on the card still
    // being in play: rule (3) resolves only in `commit_after`, after this whole
    // body, so the card is still placed for the gain below.
    if to >= 0 && !ctx::player_out(to) {
        ctx::gain(
            to,
            500,
            &Msg::new(key!("crimson_soul_payee")).player_id("who", to),
        )?;
    }
    ctx::add_crystals(-1, 0)?;
    // 规则书[持续]（1）: 「金额减少1000（最少为0）」 .
    // TODO: Feel like we should not be doing this... Check PayAdd and its family
    trigger::set_pay_amount((amount - 1000).max(0));
    ctx::log(
        player_id,
        &Msg::new(key!("crimson_soul_used"))
            .player_id("who", player_id)
            .n("n", amount as i64),
    );
    // 规则书[持续]（3） follows the `add_crystals` above through
    // [`on_crystals_changed`]; no spend site re-checks the count.
    Ok(())
}

/// 规则书[持续]（2）: 「[拥有者]使用自己原有的技能（2）时移除此卡的1个[奇迹水晶]」 --
/// Pure guard for [`skill_used`] -- the activation gate. `false`
/// means the card is not activated at all.
fn skill_used_guard(player_id: i32) -> bool {
    // TODO(规则书)（[拥有者]使用自己原有的技能）: a skill press is `skill:*`, which
    //   excludes a placed-card declaration (`cardUsed` carries the card id
    //   there); 「自己原有的技能（2）」 is the *persona* skill (2) -- a band skill
    //   (`skill:<group>:*`) still strips a crystal too.
    // TODO: Can this be ported to CEL?
    ctx::is_placed()
        && trigger::player_id() == player_id
        && ctx::crystals() > 0
        && trigger::cards()
            .first()
            .is_some_and(|c| c.as_str().starts_with("skill:"))
}

fn skill_used(player_id: i32) -> card_sdk::Asked {
    ctx::add_crystals(-1, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("crimson_soul_skill")).player_id("who", player_id),
    );
    // 规则书[持续]（3） follows the spend through [`on_crystals_changed`].
    Ok(())
}

/// 规则书[持续]（3）: 「此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区」 -- C# `Check` ->
/// `H.Unplace(this, "discard", "奇迹水晶用完了")`.
///
/// Listens to this card's own [`ActionKind::CounterChanged`] (name-filtered to
/// `counter::CRYSTALS`) rather than being re-checked at each spend site, so a
/// count emptied by *any* write -- a spend here, another card's
/// `add_card_crystals` -- leaves the field just the same.
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all.
fn crystals_changed_guard(player_id: i32) -> bool {
    // TODO: Can this be ported to CEL?

    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && trigger::name() == bandori_std::counter::CRYSTALS
        // Only a write that did not raise the count speaks for the empty
        // state. A run that writes twice (say up and then back to zero) raises
        // once per write against the count it ended on, and the earlier write
        // must not speak for the later one.
        && trigger::value() <= 0
        && ctx::crystals() == 0
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    // 规则书[持续]（3）: 「将此卡放入[使用者]弃卡区」
    ctx::set_dest(ctx::Dest::Graveyard);
    // `value == 0`: a write that landed on the already-zero count
    // 「没有奇迹水晶」; `value < 0`: the last crystal spent 「奇迹水晶用完了」.
    let why = if trigger::value() < 0 {
        key!("crimson_soul_empty")
    } else {
        key!("crimson_soul_no_crystals")
    };
    ctx::log(player_id, &Msg::new(why).player_id("who", player_id));
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_choose_kani(player_id: i32) -> card_sdk::Asked {
    pay_choose(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn skill_used_kani(player_id: i32) -> card_sdk::Asked {
    skill_used(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
