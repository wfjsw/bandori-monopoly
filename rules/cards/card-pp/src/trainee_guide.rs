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
//! The crystal counter and the turn-end tick are live; the mark bookkeeping
//! and the cash-in still need hooks the ABI lacks.

use card_sdk::abi::{prop, state_key, HookKind};
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const TRAINEE_GUIDE: CardDef = CardDef::new(
    "PP:练习生解密指南",
    &[
        On::Hook(&[card_sdk::abi::HookKind::BuildBefore], None, before_build, card_sdk::pre::MINE),
        On::Hook(&[card_sdk::abi::HookKind::BuildAfter], None, after_build, card_sdk::pre::MINE),
        On::Play(None, trainee_guide, ""),
        On::Hook(&[HookKind::TurnEnd], Some(turn_end_guard), turn_end, ""),
    ],
)
// 规则书[持续]（1）: 「手卡上限数量减1」 -- the `handLimitDelta` property
// (C# `Card.HandLimitDelta`), continuous while this instance sits on the field.
.props(&[(prop::HAND_LIMIT_DELTA, -1)])
    .legacy(&[(0, legacy_mine), (1, legacy_mine)]);

fn trainee_guide(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    ctx::add_band_crystals(player_id, 3, i32::MAX);
    // 规则书[手]: 「并将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "PP:练习生解密指南",
        &Msg::new(key!("trainee_guide_note")),
    );
    // 规则书[手]: 「在此卡上放置“粉丝数量”÷3个[奇迹水晶]」 -- C#
    // `H.PlaceFromPlay(c, -1, -1, H.Fans(i) / 3)`.
    let fans = ctx::tok(player_id, "P✽P粉丝(正)") + ctx::tok(player_id, "P✽P粉丝(反)");
    ctx::set_crystals(fans / 3);
    // 「然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/双色标记」
    // -- `cards_in(Deck)` is the reveal; a card's colour is the band prefix of
    // its id (`PP:`, `R:`, ...), which is what `H.Db.Card(id)?.band` reads.
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
        ctx::inc_slot(player_id, "trainee_guide.mono", 1);
    } else if seen.len() >= 2 {
        ctx::inc_slot(player_id, "trainee_guide.dual", 1);
    }
    // 规则书[手]: 「如果[共鸣]则[消耗]500资金并添加任意2个标记」 -- the [共鸣]
    // cost, then the 500 and the two extra marks. The marks themselves still
    // need per-card storage (see the [持续]（3） TODO in `turn_end`).
    if crate::resonance::try_resonance(player_id)? {
        ctx::pay(
            player_id,
            500,
            &Msg::new(key!("trainee_guide_resonance_cost")),
        )?;
    }
    // 规则书[持续]（1）: 「手卡上限数量减1」 -- the continuous delta declared on
    // the `CardDef` above (`props(&[(prop::HAND_LIMIT_DELTA, -1)])`), stamped on the instance at
    // placement and gone with it. No state write: that would double-count.
    Ok(())
}

/// C# `CardTraineeGuide.TurnEnd` -> `Tick`: +1 crystal, then the 5-crystal cash-in.
/// Pure guard for [`turn_end`] -- the activation gate. `false`
/// means the card is not activated at all.
fn turn_end_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn turn_end(player_id: i32) -> card_sdk::Asked {
    // 规则书[持续]（2）: 「回合结束时添加1个[奇迹水晶]」 -- C# `AddCrystals(1, "回合结束")`.
    ctx::add_crystals(1, 0)?;
    // [持续]（3）「此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作
    // 随后进入弃卡区」 -- the marks are the `trainee_guide.mono` / `.dual` slots
    // the reveal above wrote.
    if ctx::crystals() < 5 {
        return Ok(());
    }
    let mono = ctx::slot(player_id, "trainee_guide.mono");
    let dual = ctx::slot(player_id, "trainee_guide.dual");
    // 2. 「每个双色效果为将自己的所有反面[P✨P粉丝]变正」
    if dual > 0 {
        let down = ctx::tok(player_id, "P✨P粉丝(反)");
        if down > 0 {
            ctx::add_tok(player_id, "P✨P粉丝(反)", -down, i32::MAX)?;
            ctx::add_tok(player_id, "P✨P粉丝(正)", down, i32::MAX)?;
            ctx::log(
                player_id,
                &Msg::new(key!("trainee_guide_flipped")).i("n", down as i64),
            );
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
    // 「随后进入弃卡区」
    ctx::set_dest(ctx::Dest::Graveyard);
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

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}
