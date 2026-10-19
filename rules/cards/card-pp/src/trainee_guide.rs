//! `PP:练习生解密指南` -- C# `CardTraineeGuide` (MatchHost.cs:7870-7994): stay
//!
//! 规则书（docs/rulebook/cards.json, id `PP:练习生解密指南`）:
//! > 练习生解密指南：
//! > [手]：
//! > 为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]并将此卡放置在[使用者]的[场地]，在此卡上放置“粉丝数量”÷3个[奇迹水晶]，然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/双色标记，如果[共鸣]则[消耗]500资金并添加任意2个标记。
//! > [持续]：
//!
//! > （1）手卡上限数量减1。
//!
//! > （2）回合结束时添加1个[奇迹水晶]。
//!
//! > （3）此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作随后进入弃卡区：
//! > 1. 每个单色效果为获得1层状态“下次盖房的价格减少1000（可溢出），盖房后减少1层”；
//! > 2. 每个双色效果为将自己的所有反面[P✽P粉丝]变正。
//!
//! in play with crystals and mono/dual marks; cash the marks in at 5 crystals.
//! The crystal counter, the turn-end tick, the resonance marks and the
//! (3) cash-in are live. The marks are this instance's `mono` / `dual`
//! counters, so they die with it.

use bandori_std::state_key; use card_sdk::abi::{prop, ActionKind};
use skill_groups::fans;
use bandori_std::ctx;
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "PP:练习生解密指南";

pub const TRAINEE_GUIDE: CardDef = CardDef::new(
    "PP:练习生解密指南",
    &[
        On::Action(&[card_sdk::abi::ActionKind::BuildBefore], card_sdk::pre::MINE, "", None, before_build),
        On::Action(&[card_sdk::abi::ActionKind::BuildAfter], card_sdk::pre::MINE, "", None, after_build),
        On::Play("", "", None, trainee_guide),
        // 规则书[持续]（2）: 「回合结束时添加1个[奇迹水晶]」 -- the tick only.
        On::Action(&[ActionKind::TurnEnd], "", "", Some(turn_end_guard), turn_end),
        // 规则书[持续]（3）: 「此卡拥有至少5个[奇迹水晶]时…随后进入弃卡区」 -- a
        // state clause on the crystal count: any write that lands on >= 5
        // leaves the field, including the [手] placement `set_crystals` and the
        // turn-end +1 (a write like any other, crimson_soul.rs:94-96).
        On::Action(
            &[ActionKind::CounterChanged],
            "actor == owner && card.placed && counter_is('crystals') && card.counter('crystals') >= 5", "",
            Some(cash_in_guard),
            cash_in),
    ],
)
// 规则书[持续]（1）: 「手卡上限数量减1」 -- the `handLimitDelta` property
// (C# `Card.HandLimitDelta`), continuous while this instance sits on the field.
.props(&[(prop::HAND_LIMIT_DELTA, -1)])
    .silent(&[1]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"练习生解密指南"#)
        .raw_name(r#"练习生解密指南"#)
        .tags(&[r#"手"#, r#"持续"#])
        .text(r#"[手]：
为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]并将此卡放置在[使用者]的[场地]，在此卡上放置“粉丝数量”÷3个[奇迹水晶]，然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/双色标记，如果[共鸣]则[消耗]500资金并添加任意2个标记。
[持续]：
（1）手卡上限数量减1。
（2）回合结束时添加1个[奇迹水晶]。
（3）此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作随后进入弃卡区：
1. 每个单色效果为获得1层状态“下次盖房的价格减少1000（可溢出），盖房后减少1层”；
2. 每个双色效果为将自己的所有反面[P✽P粉丝]变正。"#)
        .cell(r#"D15"#)
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]));

fn trainee_guide(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    ctx::add_group_crystals(player_id, 3, i32::MAX);
    // 规则书[手]: 「并将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "PP:练习生解密指南",
        &Msg::new(key!("trainee_guide_note")),
    );
    // 规则书[手]: 「在此卡上放置“粉丝数量”÷3个[奇迹水晶]」 -- C#
    // `H.PlaceFromPlay(c, -1, -1, H.Fans(i) / 3)`.
    let fans = fans::up(player_id) + fans::down(player_id);
    ctx::set_crystals(fans / 3);
    // 「然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/双色标记」
    // -- `cards_in(Deck)` is the reveal; a card's colour is the band prefix of
    // its id (`PP:`, `R:`, ...), which is what `H.Db.Card(id)?.group` reads.
    // One distinct colour -> 1 mono mark; two or more -> 1 dual mark.
    let deck = ctx::cards_in(player_id, ctx::CardPile::Deck);
    let mut seen: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    for c in deck.iter() {
        let band = c.split(':').next().unwrap_or("");
        if !band.is_empty() && !seen.contains(&band) {
            seen.push(band);
        }
    }
    ctx::log(
        player_id,
        &Msg::new(key!("trainee_guide_revealed")).i("n", seen.len() as i64),
    );
    if seen.len() == 1 {
        ctx::add_counter("mono", 1, i32::MAX)?;
    } else if seen.len() >= 2 {
        ctx::add_counter("dual", 1, i32::MAX)?;
    }
    // 规则书[手]: 「如果[共鸣]则[消耗]500资金并添加任意2个标记」 -- the [共鸣]
    // cost, then the 500 and the two extra marks. 「任意2个」 = the player
    // picks 2 (2 mono, 2 dual, or 1 of each).
    if crate::resonance::try_resonance(player_id)? {
        ctx::pay(
            player_id,
            500,
            &Msg::new(key!("trainee_guide_resonance_cost")),
        )?;
        // 「添加任意2个标记」 -- one pick per mark, each mono or dual.
        for _ in 0..2 {
            let pick = ctx::ask_pick(
                player_id,
                &Msg::new(key!("trainee_guide_mark_title")),
                &Msg::new(key!("trainee_guide_mark_ask")),
                &[
                    Msg::new(key!("trainee_guide_mark_mono")),
                    Msg::new(key!("trainee_guide_mark_dual")),
                ],
            )?;
            if pick == 0 {
                ctx::add_counter("mono", 1, i32::MAX)?;
            } else {
                ctx::add_counter("dual", 1, i32::MAX)?;
            }
        }
        ctx::log(player_id, &Msg::new(key!("trainee_guide_marks_added")));
    }
    // 规则书[持续]（1）: 「手卡上限数量减1」 -- the continuous delta declared on
    // the `CardDef` above (`props(&[(prop::HAND_LIMIT_DELTA, -1)])`), stamped on the instance at
    // placement and gone with it. No state write: that would double-count.
    Ok(())
}

/// C# `CardTraineeGuide.TurnEnd` -> `Tick`: +1 crystal only.
/// Pure guard for [`turn_end`] -- the activation gate. `false`
/// means the card is not activated at all.
fn turn_end_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn turn_end(player_id: i32) -> card_sdk::Asked {
    // 规则书[持续]（2）: 「回合结束时添加1个[奇迹水晶]」 -- C# `AddCrystals(1, "回合结束")`.
    // (3)'s cash-in is the `CounterChanged` handler below; this +1 trips it.
    ctx::add_crystals(1, 0)?;
    Ok(())
}

/// Residual guard for [`cash_in`] -- the shared "this instance's crystals
/// changed" half lives in `bandori_std`; the >= 5 state is the condition's.
fn cash_in_guard(_player_id: i32) -> bool {
    ctx::is_placed() && bandori_std::crystals_changed_of(ID) && ctx::crystals() >= 5
}

/// 规则书[持续]（3）: 「此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作
/// 随后进入弃卡区」 -- the marks are this instance's `mono` / `dual` counters
/// (they die with the instance). Fires on any crystal write that lands on >= 5
/// (placement `set_crystals`, the turn-end +1, another card's grant).
fn cash_in(player_id: i32) -> card_sdk::Asked {
    let mono = ctx::counter("mono");
    let dual = ctx::counter("dual");
    // 2. 「每个双色效果为将自己的所有反面[P✽P粉丝]变正」 -- X = all face-downs.
    // Canonical marker name (U+273d); the previous body flipped a sparkle-spelled
    // alias that is not the real marker.
    if dual > 0 {
        let down = skill_groups::fans::down(player_id);
        if down > 0 {
            let flipped = skill_groups::fans::flip_up(player_id, down)?;
            if flipped > 0 {
                ctx::log(
                    player_id,
                    &Msg::new(key!("trainee_guide_flipped")).i("n", flipped as i64),
                );
            }
        }
    }
    // 1. 「每个单色效果为获得1层状态"下次盖房的价格减少1000（可溢出），盖房后减少1层"」
    if mono > 0 {
        ctx::state::add(player_id, "trainee_guide.buildOff", mono);
        ctx::log(
            player_id,
            &Msg::new(key!("trainee_guide_build_off")).i("n", mono as i64),
        );
    }
    // 「随后进入弃卡区」 -- `send_to_dest` leaves **now**, so a second crystal
    // write in the same raise batch cannot re-enter this body (a deferred
    // `set_dest` would still see `is_placed()` and need a latch).
    ctx::send_to_dest(ctx::Dest::Graveyard);
    Ok(())
}

/// 「下次盖房的价格减少1000（可溢出），盖房后减少1层」 -- a stack of build
/// discounts, one per mono mark.
fn before_build(player_id: i32) -> card_sdk::Asked {
    let n = ctx::state::get(player_id, "trainee_guide.buildOff");
    if n <= 0 {
        return Ok(());
    }
    ctx::set_build_discount(1000 * n, 1);
    Ok(())
}

fn after_build(player_id: i32) -> card_sdk::Asked {
    if ctx::state::get(player_id, "trainee_guide.buildOff") > 0 {
        ctx::state::add(player_id, "trainee_guide.buildOff", -1);
    }
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn before_build_kani(player_id: i32) -> card_sdk::Asked {
    before_build(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn after_build_kani(player_id: i32) -> card_sdk::Asked {
    after_build(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn trainee_guide_kani(player_id: i32) -> card_sdk::Asked {
    trainee_guide(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_end_kani(player_id: i32) -> card_sdk::Asked {
    turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn cash_in_kani(player_id: i32) -> card_sdk::Asked {
    cash_in(player_id)
}
