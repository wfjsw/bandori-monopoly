//! `R:Fire bird` -- C# `CardFireBird` (MatchHost.cs:10446-10516): pay 1,600 and
//!
//! 规则书（docs/rulebook/cards.json, id `R:Fire bird`）:
//! > Fire bird：
//! >  支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶，自己场地上存在此卡时，每回合结束时失去400资金并移除1个奇迹水晶，自己的所有格子收费变成1.5倍，奇迹水晶耗尽时将此卡放入弃牌堆
//!
//! place this card with X self-chosen miracle crystals. While it is in play:
//! pay 400 and burn one crystal at every turn end, all your tiles charge 1.5x
//! rent, and the card hits the discard when the crystals run out.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, FilterKind, TriggerKind};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Filter, Msg, On};

pub const FIRE_BIRD: CardDef = CardDef::new(
    "R:Fire bird",
    &[
        // NEGATION-AUDIT V2 (settled): no activation cost. The 「支付1600资金」
        // is effect content (rulebook L13-14), so there is no `money >= 1600`
        // play gate -- an unaffordable in-body payment takes the Q1 shortfall
        // path (mortgage, then bankruptcy). C# `CardFireBird.WhyNot` had one.
        On::Play("", None, play),
        // 规则书: 「自己场地上存在此卡时」 -- `card.placed` is this instance in play.
        // TODO(规则书)（每回合结束时）: 书中没把「每回合」限定成拥有者的回合 --
        //   这里每个回合结束都烧（C# `DecayCard` 只在拥有者的回合结束；同 tritone / J11）.
        On::Action(Action::TurnEnd("card.placed", None, turn_end)),
        // 规则书: 「自己场地上存在此卡时」 -- `card.placed`.
        // 规则书: 「自己的所有格子收费变成1.5倍」 -- anchored on the *tile's*
        // owner, not on the payee: a `paySplitAdd` payee redirect runs first, so
        // `target` may name someone else for the owner's own tile (and vice
        // versa). `tile` rides the pay payload through every split stage.
        On::Filter(Filter::PaySplitMul("card.placed && pay_is_rent && tile.id >= 0 && tile.owner == owner", None, pay_mul)),
        On::Action(Action::CounterChanged("", Some(crystals_changed_guard), on_crystals_changed)),
    ],
).props(&[(card_sdk::abi::prop::EST_COST, 1600), (card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Fire bird"#)
        .raw_name(r#"Fire bird"#)
        .tags(&[])
        .text(CardText::para(r#"支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶，自己场地上存在此卡时，每回合结束时失去400资金并移除1个奇迹水晶，自己的所有格子收费变成1.5倍，奇迹水晶耗尽时将此卡放入弃牌堆"#))
        // sheet cell E6
        .fields(&[("group", r#"Roselia"#), ("owner", "")]));

const ID: &str = "R:Fire bird";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「支付1600资金」 -- C# `PayCtx { amount = 1600, kind = "pay", must = false }`.
    let paid = ctx::pay(player_id, 1600, &Msg::new(key!("fire_bird_why")))?;
    if paid < 1600 {
        // TODO(规则书)（支付1600资金）: 书中只说付出 1600 并放置此卡，没有定义「付不满」
        //   时的状态（C# 叫「视为此卡未生效」，CH-07 / C-Q5）-- 这里把付不满当无效返回；
        //   另外 `pay` 返回的是实际搬走的钱，支付修正（如 0.5 倍）会让付足 1600 也被读成
        //   不足（缺一个「支付成立 / 被打折」的信号）.
        return Ok(());
    }
    // 规则书: 「并为其添加X（自选）个奇迹水晶」 -- C# `H.AskNumber(i, ..., 1, 10, ...)`.
    // TODO(规则书)（X（自选））: 书中对 X 没有上下限 -- 1..=10 取自 C#，X = 0 是否合法也未定
    //   （OPEN-FIRE-X）.
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("fire_bird_ask_title")),
        &Msg::new(key!("fire_bird_ask_text")),
        1,
        10,
    )?;
    // 规则书: 「将此卡放置在自己场地上」 -- C# `H.PlaceFromPlay(c, -1, -1, Math.Max(1, r.value))`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("fire_bird_note")));
    // 规则书: 「并为其添加X（自选）个奇迹水晶」 -- the crystals live on the placed card
    // (C# `Card.Crystals` / `H.PlaceFromPlay(..., crystals)`), charged here.
    let n = x.max(1);
    ctx::set_crystals(n);
    ctx::log(
        player_id,
        &Msg::new(key!("fire_bird_placed"))
            .player_id("who", player_id)
            .i("n", n as i64),
    );
    // 规则书: 「每回合结束时失去400资金并移除1个奇迹水晶」 / 「奇迹水晶耗尽时将此卡放入
    // 弃牌堆」 -- `DecayCard.TurnEnd` -> `CardFireBird.Burn`, see `turn_end`.
    // 规则书: 「自己的所有格子收费变成1.5倍」 -- C# `CardFireBird.PaySplitMul`, see
    // `pay_mul`.
    Ok(())
}

/// 规则书: 「自己的所有格子收费变成1.5倍」. Rent on this card's owner's tiles is
/// charged at ×1.5. Runs through the Fx hook dispatch at `paySplitMul` (after
/// `PaySplitAdd`, before `PaySplitChoose`), so this is a field effect, not a [反击].
///
/// User ruling 2026-10-15 (OPEN-取整 family): a ×1.5 modifier transforms the
/// running amount exactly and must NOT CeilTo in its own body; the engine
/// rounds up to a multiple of 10 once after all add/mul modifiers. Integer
/// fold rounds the .5 up (`(amount * 3 + 1) / 2`) so a sole modifier still
/// settles at `round10(exact)` (25 → 37.5 → 40).
fn pay_mul(player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    // TODO(规则书)（自己的所有格子收费变成1.5倍）: 「变成1.5倍」与其他倍率（Ave Mujica
    //   1.5 倍、HHW 双倍、摩卡 0.5 倍）是叠乘还是取代未定（OPEN-R1 / MONEY-01）-- 这里叠乘.
    let boosted = ((value as i64 * 3 + 1) / 2).clamp(0, i32::MAX as i64) as i32;
    ctx::log(
        player_id,
        &Msg::new(key!("fire_bird_pay_mul"))
            .player_id("who", player_id)
            .n("from", value as i64)
            .n("to", boosted as i64),
    );
    boosted
}

/// `DecayCard.TurnEnd` -> `CardFireBird.Burn` (C# `H.LoseR(Seat, 400, "Fire bird")`
/// then `AddCrystals(-1)`; empty -> `H.Unplace(this, "discard")`). Runs through the
/// Fx hook dispatch, so this is a field effect, not a [反击].
fn turn_end(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「每回合结束时失去400资金」 -- C# `H.LoseR(Seat, 400, "Fire bird")`.
    ctx::pay(
        player_id,
        400,
        &Msg::new(key!("fire_bird_burn")).player_id("who", player_id),
    )?;
    // 规则书: 「并移除1个奇迹水晶」 -- C# `AddCrystals(-1, "回合结束")`. The
    // 「奇迹水晶耗尽时将此卡放入弃牌堆」 half is [`on_crystals_changed`].
    ctx::remove_crystal()?;
    Ok(())
}

/// 规则书: 「奇迹水晶耗尽时将此卡放入弃牌堆」 -- C# `H.Unplace(this, "discard")`.
///
/// Listens to this card's own [`ActionKind::CounterChanged`] (name-filtered to
/// `counter::CRYSTALS`) rather than being re-checked at the decay tick, so a
/// count emptied by *any* write leaves the field just the same.
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all.
fn crystals_changed_guard(player_id: i32) -> bool {
    // 规则书: 「自己场地上存在此卡时」 -- re-checked on the live instance.
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && trigger::name() == card_sdk::abi::counter::CRYSTALS
        && ctx::crystals() == 0
        // Only a write that did not raise the count speaks for the empty
        // state; see AG:绯红之魂 (3).
        && trigger::value() <= 0
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("fire_bird_decayed")).player_id("who", player_id),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_end_kani(player_id: i32) -> card_sdk::Asked {
    turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_mul_kani(player_id: i32, value: i32) -> i32 {
    pay_mul(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
