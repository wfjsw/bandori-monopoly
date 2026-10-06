//! `AG:绯红之魂` -- C# `CardCrimsonSoul` (MatchHost.cs:1583-1680):
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
//! place with N crystals; spend a crystal to cut a payment by 1,000.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "AG:绯红之魂";

pub const CRIMSON_SOUL: CardDef = CardDef::new(
    "AG:绯红之魂",
    &[
        On::Play(Some(cant_play), play),
        On::Hook(&[HookKind::PayChoose], pay_choose_guard, pay_choose),
        On::Hook(&[HookKind::PayAfter], pay_after_guard, pay_after),
        On::Hook(&[HookKind::SkillUsed], skill_used_guard, skill_used),
        On::Hook(
            &[HookKind::CrystalsChanged],
            crystals_changed_guard,
            on_crystals_changed,
        ),
    ],
);

/// C# `CardCrimsonSoul.WhyNot`: refuses under 500.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::money_of(player_id) < 500 {
        return Some(Msg::new(key!("crimson_soul_no_money")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」 -- C# `H.PlaceFromPlay(c, -1, -1, num)`.
    ctx::set_dest(ctx::Dest::Field);
    // 规则书[手]: 「选择[消耗]1到5次500资金」 -- C# `H.AskNumber(i, ..., 1, Math.Max(1, min(5, money/500)))`.
    let max = (ctx::money_of(player_id) / 500).min(5);
    let n = ctx::ask_number(
        player_id,
        &Msg::new(key!("crimson_soul_title")),
        &Msg::new(key!("crimson_soul_ask")),
        1,
        max.max(1),
    )?;
    // C# `n = Math.Max(1, Math.Min(max, r.value))`.
    let n = n.clamp(1, max);
    // 规则书[手]: 「[消耗]1到5次500资金」 -- C# `PayCtx { kind: "lose", must: false, amount = 500 * n }`.
    let paid = ctx::pay(player_id, 500 * n, &Msg::new(key!("crimson_soul_why")))?;
    // 规则书[手]: 「并在这张卡上放置对应数量的[奇迹水晶]」 -- C# `num = p.paid ? n : 0`,
    // `H.PlaceFromPlay(c, -1, -1, num)`. A run that moved no money earns no crystals.
    let num = if paid > 0 { n } else { 0 };
    ctx::place_card(player_id, ID, &Msg::new(key!("crimson_soul_note")));
    ctx::set_crystals(num);
    ctx::log(
        player_id,
        &Msg::new(key!("crimson_soul_placed")).player_id("who", player_id),
    );
    // C# `if (num <= 0) H.Unplace(pc, "discard", "没有奇迹水晶")`. That is
    // 规则书[持续]（3） seeing its own count land on zero, so it lives in
    // [`on_crystals_changed`] -- `set_crystals` above is a write like any other.
    Ok(())
}

/// 规则书[持续]（1）: 「[拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，此次
/// [消耗]或[支付]金额减少1000」 -- C# `CardCrimsonSoul.PayChoose` / `Use`:
/// the owner is about to pay -> may spend 1 crystal for −1,000 (0 floor).
/// Pure guard for [`pay_choose`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pay_choose_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

fn pay_choose(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    if amount <= 0 || ctx::crystals() <= 0 {
        return Ok(());
    }
    // C# `H.AskYes(Seat, CardName, "要付 …：移除 1 个 [奇迹水晶] 让这次 −1,000 吗？…", r, p.amount >= 800)`.
    let to = trigger::target();
    let ask = if to >= 0 && to != player_id {
        Msg::new(key!("crimson_soul_pay_ask_to"))
            .n("n", amount as i64)
            .player_id("who", to)
    } else {
        Msg::new(key!("crimson_soul_pay_ask")).n("n", amount as i64)
    };
    if !ctx::ask_yes(player_id, &Msg::new(key!("crimson_soul_title")), &ask)? {
        return Ok(());
    }
    // C# `AddCrystals(-1, "付钱时使用")`.
    ctx::add_crystals(-1, 0);
    // 规则书[持续]（1）: 「金额减少1000（最少为0）」 -- C# `p.amount = Math.Max(0, p.amount - 1000)`.
    trigger::set_pay_amount((amount - 1000).max(0));
    ctx::log(
        player_id,
        &Msg::new(key!("crimson_soul_used"))
            .player_id("who", player_id)
            .n("n", amount as i64),
    );
    // 规则书[持续]（1）: 「若为[支付]则被[支付]玩家[获得]500资金」 -- C# tags the
    // `PayCtx` with "crimson" and `PayAfter` pays the 500. The tag rides a
    // per-player slot so `pay_after` below can see it; stored as `to + 1` so an
    // unset slot (0) reads as "no tag" (player 0 is a real player).
    if to >= 0 && to != player_id {
        ctx::set_slot(player_id, "crimson_payee", to + 1);
    }
    // 规则书[持续]（3） follows the `add_crystals` above through
    // [`on_crystals_changed`]; no spend site re-checks the count.
    Ok(())
}

/// 规则书[持续]（1）: 「若为[支付]则被[支付]玩家[获得]500资金」 -- C#
/// `CardCrimsonSoul.PayAfter` (`p.tags["crimson"]` -> `H.GainR(p.to, 500, ...)`).
/// Pure guard for [`pay_after`] -- the activation gate. `false`
/// means the card is not activated at all.
// TODO(规则书): the 500 is owed as part of rule (1) the moment the crystal is
// spent, but it is paid here -- after rule (3) may have already discarded the
// card for running out -- so spending the **last** crystal on a [支付] loses
// the payee their 500. Rule (1) does not condition the payout on the card
// still being in play.
fn pay_after_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn pay_after(player_id: i32) -> card_sdk::Asked {
    let due = ctx::slot(player_id, "crimson_payee") - 1;
    if due < 0 {
        return Ok(());
    }
    ctx::set_slot(player_id, "crimson_payee", 0);
    if ctx::player_out(due) {
        return Ok(());
    }
    ctx::gain(
        due,
        500,
        &Msg::new(key!("crimson_soul_payee")).player_id("who", due),
    );
    Ok(())
}

/// 规则书[持续]（2）: 「[拥有者]使用自己原有的技能（2）时移除此卡的1个[奇迹水晶]」 --
/// C# `CardCrimsonSoul.SkillUsed`.
/// Pure guard for [`skill_used`] -- the activation gate. `false`
/// means the card is not activated at all.
fn skill_used_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id && ctx::crystals() > 0
}

fn skill_used(player_id: i32) -> card_sdk::Asked {
    ctx::add_crystals(-1, 0);
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
/// Listens to this card's own [`HookKind::CrystalsChanged`] rather than being
/// re-checked at each spend site, so a count emptied by *any* write -- a spend
/// here, another card's `add_card_crystals`, the [手] placing none -- leaves the
/// field just the same.
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all.
fn crystals_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && ctx::crystals() == 0
        // Only a write that did not raise the count speaks for the empty
        // state. A run that writes twice (say up and then back to zero) raises
        // once per write against the count it ended on, and the earlier write
        // must not speak for the later one.
        && trigger::value() <= 0
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    // 规则书[持续]（3）: 「将此卡放入[使用者]弃卡区」
    ctx::set_dest(ctx::Dest::Graveyard);
    // The [手] placing none 「没有奇迹水晶」; a spent-out card 「奇迹水晶用完了」.
    // `t.value` is the write's change -- 0 for the first, negative for the rest.
    let why = if trigger::value() < 0 {
        key!("crimson_soul_empty")
    } else {
        key!("crimson_soul_no_crystals")
    };
    ctx::log(player_id, &Msg::new(why).player_id("who", player_id));
    Ok(())
}
