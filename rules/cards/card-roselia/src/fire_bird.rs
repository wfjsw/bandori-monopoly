//! place this card with X self-chosen miracle crystals. While it is in play:
//!
//! 规则书（docs/rulebook/cards.json, id `R:Fire bird`）:
//! > Fire bird：
//! > 支付1600资金，将此卡放置在自己的场地上，并为此卡添加X个[奇迹水晶]；X由自己选择。
//! > 自己的场地上存在此卡时：
//! > - 每回合结束时：失去400资金，并移除此卡的1个[奇迹水晶]。
//! > - 自己的所有格子收费变为原本的1.5倍。
//! > 此卡的[奇迹水晶]耗尽时，将此卡放入弃卡区。
//!
//! pay 400 and burn one crystal at every turn end, all your tiles charge 1.5x
//! rent, and the card hits the discard when the crystals run out.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, FilterKind, TriggerKind};
use card_sdk::CardText;
use card_sdk::{key, para, sec, Action, CardDef, Filter, Msg, On};

pub const FIRE_BIRD: CardDef = CardDef::new(
    "R:Fire bird",
    &[
        // NEGATION-AUDIT V2 (settled): no activation cost. The 「支付1600资金」 is effect
        // content (rulebook L13-14), so there is no `money >= 1600` play gate -- an
        // unaffordable in-body payment takes the Q1 shortfall path (mortgage, then
        // bankruptcy).
        On::Play("", None, play),
        // TODO(规则书)（每回合结束时）: 规则书: 「自己场地上存在此卡时」 -- `card.placed` is this instance in
        // play.
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
        .text(CardText::of(&[sec(r#""#, &[para(r#"支付1600资金，将此卡放置在自己的场地上，并为此卡添加X个[奇迹水晶]；X由自己选择。"#), para(r#"自己的场地上存在此卡时："#), para(r#"- 每回合结束时：失去400资金，并移除此卡的1个[奇迹水晶]。"#), para(r#"- 自己的所有格子收费变为原本的1.5倍。"#), para(r#"此卡的[奇迹水晶]耗尽时，将此卡放入弃卡区。"#)])]))
        // sheet cell E6
        .fields(&[("group", r#"Roselia"#), ("owner", "")]));

const ID: &str = "R:Fire bird";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「支付1600资金」.
    let receipt =
        ctx::PayCommand::new(player_id, -1, 1600, &Msg::new(key!("fire_bird_reason"))).send()?;
    if !receipt.settled() {
        // 规则书: 「视为此卡未生效」 -- a shortfall on the 1600. Card spent anyway
        // (C-Q5(a)); the waste bit carries the clause (CH-07).
        bandori_std::zones::set_ineffective();
        return Ok(());
    }
    // TODO(规则书)（X（自选）
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("fire_bird_ask_title")),
        &Msg::new(key!("fire_bird_ask_text")),
        1,
        10,
    )?;
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, ID, &Msg::new(key!("fire_bird_note")));
    let n = x.max(1);
    ctx::set_crystals(n);
    // 规则书: 「每回合结束时失去400资金并移除1个奇迹水晶」 -- the chosen X is
    // also the remaining-turn count.
    ctx::set_turns_left(n);
    ctx::log(
        player_id,
        &Msg::new(key!("fire_bird_placed"))
            .player_id("who", player_id)
            .i("n", n as i64),
    );
    Ok(())
}

/// 规则书: 「自己的所有格子收费变为原本的1.5倍」. Rent on this card's owner's
/// tiles is charged at ×1.5. Runs through the hook dispatch at `paySplitMul`
/// (after `PaySplitAdd`, before `PaySplitChoose`), so this is a field effect,
/// not a [反击].
///
/// `原本` is the mul stage's input, not a second base home: the ruling makes
/// the add row a separate, earlier stage that composes its fixed values before
/// any scale (MONEY-01 / C-Q1, ruled 2026-10-31; G02 asserts `(R+100)/2`), so a
/// live `+100` lands first and the ×1.5 covers `charge+100`. A `paySplitMul`
/// body only sees that stage input -- no base value is exported -- and
/// `ctx::rent_of` would miss a dice-rent roll (it quotes the unit, not
/// `unit × 1d20`), so the module stays one factor of the product.
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
    // （自己的所有格子收费变为原本的1.5倍）: one factor of the mul stage. The
    //   mul fold reads the stage input, so the fixed-value row is already in
    //   it -- `原本` here is base charge + live adds (MONEY-01 / C-Q1, ruled
    //   2026-10-31). Holds `(25 → 38 → 40)` for a lone modifier.
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

/// Runs through the hook dispatch, so this is a field effect, not a [反击].
fn turn_end(player_id: i32) -> card_sdk::Asked {
    // TODO(规则书): 「每回合结束时」 -- the tick, before the burn. 「每回合结束时」 does not say whose.
    ctx::dec_turns_left();
    ctx::pay(
        player_id,
        400,
        &Msg::new(key!("fire_bird_burn")).player_id("who", player_id),
    )?;
    // The 「奇迹水晶耗尽时将此卡放入弃牌堆」 half is [`on_crystals_changed`].
    ctx::remove_crystal()?;
    Ok(())
}

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
        && bandori_std::crystals_changed_of(ID)
        && ctx::crystals() == 0
        // Only a write that did not raise the count speaks for the empty
        // state; see AG:绯红之魂 (3).
        && trigger::value() <= 0
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
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
