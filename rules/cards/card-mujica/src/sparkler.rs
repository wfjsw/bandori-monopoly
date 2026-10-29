//! `Mujica:燃尽前的线香花火` -- C# `CardSparkler` (MatchHost.cs:5542-5580): place
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:燃尽前的线香花火`）:
//! > 燃尽前的线香花火：
//! >  将此卡放置于场上并放置2个奇迹水晶（上限2），你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合，最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]。
//!
//! with 2 miracle crystals; each turn end burns one for an extra turn -- except the
//! last, which only discards the card and gives 2 [眩晕].

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ActionKind;
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "Mujica:燃尽前的线香花火";

pub const SPARKLER: CardDef = CardDef::new(
    "Mujica:燃尽前的线香花火",
    &[
        On::Play("", None, sparkler),
        // 规则书: 「你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合」
        // C# `CardSparkler.TurnEndAfter` -> `Burn` -- a field hook on the card's own
        // turn end while it is in play (ABI v23 `TurnEndAfter`: after `TurnEnd`,
        // matching the C# `Fx.TurnEndAfter` dispatch).
        On::Action(Action::TurnEndAfter("actor == owner && card.placed", None, turn_end)),
        // 规则书: 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]」
        On::Action(Action::CounterChanged("actor == owner && card.placed && counter_is('crystals') && card.counter('crystals') == 0 && value < 0", Some(crystals_changed_guard), on_crystals_changed)),
    ],
)
// 规则书「放置2个奇迹水晶」: the `⌛2` marker the client shows.
.props(&[(bandori_std::marker::TURNS_LEFT, 2)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"燃尽前的线香花火"#)
        .raw_name(r#"燃尽前的线香花火"#)
        .tags(&[])
        .text(CardText::para(r#"将此卡放置于场上并放置2个奇迹水晶（上限2），你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合，最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]。"#))
        // sheet cell J5
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]));

fn sparkler(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「将此卡放置于场上并放置2个奇迹水晶（上限2）」
    // C# `H.PlaceFromPlay(c, -1, -1, 2)` loads the card with 2 crystals
    // (`MaxCrystals = 2`).
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("sparkler_note")));
    // TODO(ABI): there is no per-instance counter max -- `add_counter_at` clamps
    //   this call only, so an external add (`crystal_swap`'s `add`, `max = 0`) can
    //   push the count past 上限2, and the burn then hands out one more extra turn
    //   than the two crystals allow.
    ctx::add_crystals(2, 2)?;
    ctx::log(
        player_id,
        &Msg::new(key!("sparkler_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardSparkler.TurnEndAfter` -> `Burn` (MatchHost.cs:5560-5578).
/// Runs through the Fx hook dispatch at `turnEndAfter` (ABI v23
/// `TriggerKind::TurnEndAfter`), so this is a field effect, not a [反击].
/// The registration's `actor == owner` scopes this to the card owner's own turn
/// end (the body takes no turn argument).
fn turn_end(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合」
    // C# `AddCrystals(-1, "回合结束")` then `H.GiveExtraTurn(Seat, CardName)`.
    // The burn reads the count *before* the write so the last removal knows it
    // is last: 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]」
    // is the terminal effect of that removal (discard + 2 [眩晕]), not an extra
    // turn on top. C-Q26a ruling (this batch): the last removal does **not**
    // grant 「获得一个额外回合」 -- the 2 [眩晕] it hands out would otherwise
    // auto-skip that extra turn (`play.rs` `stunned` -> `end_turn`) and its
    // 「每回合结束时移除一层」 tick would eat one of the two, netting 1.
    // 「你的回合结束后」 -- the tick.
    ctx::dec_turns_left();
    let before = ctx::crystals();
    ctx::add_crystals(-1, 0)?;
    // TODO(规则书)（并使你获得一个额外回合）: 书 grants an extra turn on every turn-end
    //   removal -- we withhold it on the last one (C-Q26a ruling: the 2 [眩晕] would
    //   eat that extra turn and its end-of-turn tick one of the two)
    if before > 1 {
        ctx::give_extra_turn(player_id);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("sparkler_burned")).player_id("who", player_id),
    );
    // 规则书: 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]」
    // rides the write above through [`on_crystals_changed`].
    Ok(())
}

/// 规则书: 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]」 --
/// C# `if (Crystals <= 0) { H.Unplace(this, "discard", "奇迹水晶用完了");
/// H.GiveStun(Player, 2, ...); }`.
///
/// 「移除后」 is the trigger, not the bare empty count: a card that somehow
/// sits at 0 without a removal taking it there does not stun its owner.
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all.
/// Residual guard -- `card_is` is the real check; the `trigger::name()` test
/// duplicates the condition's `counter_is('crystals')` (belt and braces).
fn crystals_changed_guard(_player_id: i32) -> bool {
    trigger::card_is(ID) && trigger::name() == bandori_std::counter::CRYSTALS
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    // 规则书: 「并立刻使你获得2层[眩晕]」
    rules_status::stun::add(player_id, 2);
    ctx::log(
        player_id,
        &Msg::new(key!("sparkler_out")).player_id("who", player_id),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn sparkler_kani(player_id: i32) -> card_sdk::Asked {
    sparkler(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_end_kani(player_id: i32) -> card_sdk::Asked {
    turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
