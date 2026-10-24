//! `MyGO:（灯）不再迷茫` -- C# `CardTomoriNoLonger` (MatchHost.cs:6308-6347):
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（灯）不再迷茫`）:
//! > （灯）不再迷茫：  
//! >  
//! > （1）获得两层仅能被自然流失效果移除的[停留]，将此卡放置于场上并将所有手卡置入弃牌堆，在此卡上放置X+1个[奇迹水晶]，X为你弃置的手牌数
//! > （2）你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗，当此卡上的水晶由于此效果以外的原因减少时，此卡立刻获得等同于减少量的[奇迹水晶]
//! > （3）此卡上的奇迹水晶耗尽后，[移除]此卡
//!
//! Sheet 2026-10-06 新卡组卡 I3 adds the refill clause (2): a decrease that is
//! not the skill-cost substitution is refunded immediately.

use alloc::vec::Vec;

use alloc::string::String;
use bandori_std::ctx::{self, trigger, CardPile};
use bandori_std::kind::Actions;
use card_sdk::abi::ActionKind;
use card_sdk::{CardText, it};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const TOMORI_NO_LONGER: CardDef = CardDef::new(
    "MyGO:（灯）不再迷茫",
    &[
        On::Play("", None, tomori_no_longer),
        // （3）「此卡上的奇迹水晶耗尽后，[移除]此卡」 -- swept whenever the
        // count empties, whichever write emptied it (the skill-cost swap, a
        // move, or a decay). `CounterChanged` is name-filtered to
        // `counter::CRYSTALS`.
        On::Action(Action::CounterChanged("", Some(sweep_guard), sweep)),
        // （2）「可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗」 -- a
        // fire-cost substitution. The offer exists only for a positive spend
        // with a crystal to trade; accepting cancels the fire spend and burns
        // one crystal instead.
        //
        // ABI v74: ChargePaying is **not** a FilterFlag -- its 「可」 opt-in
        // prompts, and Filter bodies cannot. It stays an `On::Action` on
        // `ChargePaying` (the window `spend_charge` raises before the pots
        // move); `set_cancelled()` makes `raise` return false, i.e. cost
        // substituted and the skill body proceeds.
        On::Action(Actions::ChargePaying("actor == owner && card.counter('crystals') >= 1", None, cover)),
        // Sheet I3 (2) 「当此卡上的水晶由于此效果以外的原因减少时，此卡立刻
        // 获得等同于减少量的[奇迹水晶]」. `ActionKind::CounterChanged`
        // name-filtered to `counter::CRYSTALS` (the name is not yet in the
        // condition vocabulary).
        On::Action(Action::CounterChanged(card_sdk::pre::MINE, Some(refill_guard), refill)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"不再迷茫"#)
        .raw_name(r#"（灯）不再迷茫"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"获得两层仅能被自然流失效果移除的[停留]，将此卡放置于场上并将所有手卡置入弃牌堆，在此卡上放置X+1个[奇迹水晶]，X为你弃置的手牌数"#),
            it(r#"你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗，当此卡上的水晶由于此效果以外的原因减少时，此卡立刻获得等同于减少量的[奇迹水晶]"#),
            it(r#"此卡上的奇迹水晶耗尽后，[移除]此卡"#),
        ]))
        // sheet cell I3
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", r#"高松灯"#)]));

/// Set while (2)'s substitution is running, so [`refill`] knows the decrease
/// is the skill-cost swap and must not be refunded.
const SLOT_COVERING: &str = "tomori_no_longer.covering";

/// （2）'s substitution. Applicability is the entry's condition (a fire-pot
/// spend with a crystal to trade). Cancelling the `ChargePaying` window is the
/// substitute gesture: `raise` returns false (`cost substituted`), the fire
/// pots never move, and one crystal is removed instead.
fn cover(player_id: i32) -> card_sdk::Asked {
    // `ChargePaying`'s `value` is the cost.
    let spent = ctx::trigger::value();
    if spent <= 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tomori_no_longer_title")),
        &Msg::new(key!("tomori_no_longer_cover")).i("n", spent as i64),
    )? {
        return Ok(());
    }
    ctx::set_slot(
        player_id,
        SLOT_COVERING,
        ctx::slot(player_id, SLOT_COVERING) + 1,
    );
    ctx::add_crystals(-1, i32::MAX)?;
    // `CounterChanged` is raised after the run commits (`wasm_rules` counter_log),
    // so do **not** clear the flag here -- [`refill`] consumes it when the
    // deferred hook fires.
    ctx::trigger::set_cancelled();
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_no_longer_covered")).i("n", spent as i64),
    );
    Ok(())
}

/// Sheet I3 (2): refund any crystal decrease that is not the skill-cost
/// substitution.
/// Residual guard -- the name is also `counter_is('crystals')` in the pre
/// (kept here as well).
fn refill_guard(_player_id: i32) -> bool {
    trigger::name() == card_sdk::abi::counter::CRYSTALS
}

fn refill(player_id: i32) -> card_sdk::Asked {
    let delta = trigger::value();
    if delta >= 0 {
        return Ok(());
    }
    // The substitution (`cover`) parks a count here for each of its own `-1`s;
    // `CounterChanged` is deferred past the cover body, so consume one now.
    let covering = ctx::slot(player_id, SLOT_COVERING);
    if covering > 0 {
        ctx::set_slot(player_id, SLOT_COVERING, covering - 1);
        return Ok(());
    }
    let back = -delta;
    ctx::add_crystals(back, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_no_longer_refilled")).i("n", back as i64),
    );
    Ok(())
}

const ID: &str = "MyGO:（灯）不再迷茫";

fn tomori_no_longer(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「获得两层仅能被自然流失效果移除的[停留]」 -- C#
    // `H.GiveStay(i, 2, i, CardName)`.
    rules_status::stay::add(player_id, 2);
    // 规则书（1）: 「仅能被自然流失效果移除的[停留]」 -- C# `H.IncV(i,
    // "lockedStay", 2)` marks the two layers as locked (cleared only by natural
    // decay).
    ctx::inc_slot(player_id, "lockedStay", 2);
    // 规则书（1）: 「将此卡放置于场上」 -- C# `H.PlaceFromPlay(c, -1, -1, list.Count + 1)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("tomori_no_longer_note")));
    // 规则书（1）: 「并将所有手卡置入弃牌堆」 -- C# walks `H._hidden[i].hand`
    // (`cards_in(player_id, CardPile::Hand)`) and `H.DiscardFromHand`s each copy.
    let hand: Vec<String> = ctx::cards_in(player_id, CardPile::Hand);
    let mut discarded = 0;
    for id in &hand {
        if ctx::discard_from_hand(player_id, id) {
            discarded += 1;
        }
    }
    // 规则书（1）: 「在此卡上放置X+1个[奇迹水晶]，X为你弃置的手牌数」 -- C#
    // `PlaceFromPlay(..., crystals: list.Count + 1)`.
    ctx::set_crystals(discarded + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_no_longer_discarded"))
            .player_id("who", player_id)
            .i("n", discarded as i64)
            .i("crystals", (discarded + 1) as i64),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_no_longer_placed")).player_id("who", player_id),
    );
    // （2）「你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗」
    // -- `fireSpent` is raised after the spend commits, so the substitution is a
    // crystal taken here and the fire handed back.
    // （3）「此卡上的奇迹水晶耗尽后，[移除]此卡」 -- swept at the turn end rather
    // than inside the spend, so it also catches a spend from another card.
    Ok(())
}

/// Guard for [`sweep`] -- the count emptied (the write was a decrease and the
/// card now holds 0). Name-filtered to `counter::CRYSTALS`.
fn sweep_guard(player_id: i32) -> bool {
    trigger::name() == card_sdk::abi::counter::CRYSTALS
        && ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && trigger::value() <= 0
        && ctx::crystals() == 0
}

/// （3）「此卡上的奇迹水晶耗尽后，[移除]此卡」 -- `[移除]` is
/// [`Dest::Banished`] (gone from the field, not into the discard).
fn sweep(_player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Banished);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn tomori_no_longer_kani(player_id: i32) -> card_sdk::Asked {
    tomori_no_longer(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn sweep_kani(player_id: i32) -> card_sdk::Asked {
    sweep(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn cover_kani(player_id: i32) -> card_sdk::Asked {
    cover(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn refill_kani(player_id: i32) -> card_sdk::Asked {
    refill(player_id)
}
