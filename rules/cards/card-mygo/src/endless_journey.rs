//! `MyGO:哪怕这旅程没有终点` -- C# `CardEndlessJourney` (MatchHost.cs:6971-7009):
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:哪怕这旅程没有终点`）:
//! > 哪怕这旅程没有终点：
//! >  [手] 将此卡放置于当前格子上并为其放置4个奇迹水晶，每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶。当此卡奇迹水晶数量为0时，将此卡置入弃牌堆。
//! > （2）[持续] 触发结算时，获得X*60资金，X为你此次主要移动[经过]的格数
//!
//! plant this card on the current tile with 4 miracle crystals; settles pay
//! out 60 per tile walked, and a long main move burns a crystal.

use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const ENDLESS_JOURNEY: CardDef = CardDef::new(
    "MyGO:哪怕这旅程没有终点",
    &[
        On::Play("", None, endless_journey),
        On::Hook(&[HookKind::TurnEnd], "actor == owner && card.placed", None, turn_end),
        // （2）「触发结算时」 is 行动阶段 15 -- an entry in the settle's effect
        // list (`SETTLE-STAGES.md` §4 M2), not the 「[触发结算]后」 window. A
        // field card that replaces the body skips this entry.
        On::Hook(&[HookKind::SettleBody], "actor == owner && move.main && card.placed", None, settle_body),
        On::Hook(&[HookKind::CounterChanged], "actor == owner && card.placed && counter_is('crystals') && card.counter('crystals') == 0 && value <= 0", Some(crystals_changed_guard), on_crystals_changed),
    ],
);

const ID: &str = "MyGO:哪怕这旅程没有终点";

fn endless_journey(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置于当前格子上」 -- bound to where the player is.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card_on(
        player_id,
        ctx::player_pos(player_id),
        ID,
        &Msg::new(key!("endless_journey_note")),
    );
    // 规则书[手]: 「并为其放置4个奇迹水晶」 -- the placement's crystal charge.
    ctx::set_crystals(4);
    ctx::log(
        player_id,
        &Msg::new(key!("endless_journey_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardEndlessJourney.TurnEnd` -- when the owner's main move walked more
/// than 6 steps, burn one miracle crystal; at 0 the card is discarded.
fn turn_end(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶。当此卡奇迹
    // 水晶数量为0时，将此卡置入弃牌堆。」 -- C# `H._turnCtx.LastMain > 6` ->
    // `AddCrystals(-1)` and `H.Unplace(this, "discard", ...)` at 0.
    if ctx::turn_main_steps() <= 6 {
        return Ok(());
    }
    ctx::decay()?;
    Ok(())
}

/// 规则书[手]: 「当此卡奇迹水晶数量为0时，将此卡置入弃牌堆」 -- C#
/// `H.Unplace(this, "discard", ...)`.
///
/// Listens to this card's own [`HookKind::CounterChanged`] (name-filtered to
/// `counter::CRYSTALS`) rather than being re-checked at the decay tick, so a
/// count emptied by *any* write leaves the field just the same.
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all.
/// Residual guard -- `card_is` + the counter name stay here (the name is not
/// yet in the condition vocabulary).
fn crystals_changed_guard(_player_id: i32) -> bool {
    trigger::card_is(ID) && trigger::name() == card_sdk::abi::counter::CRYSTALS
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("endless_journey_decayed")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardEndlessJourney.SettleAfter` -- when the owner settles off their own
/// main move, pay out 60 per tile that walk passed.
/// 规则书（2）: 「[持续] 触发结算时，获得X*60资金」 -- 行动阶段 15
/// (`SETTLE-STAGES.md` §4 M2): an entry in the settle's effect list, so a body
/// replace (`trigger::cancelled()`) skips it.
fn settle_body(player_id: i32) -> card_sdk::Asked {
    if trigger::cancelled() {
        return Ok(());
    }
    // 规则书（2）: 「[持续] 触发结算时，获得X*60资金，X为你此次主要移动[经过]的格数」
    // -- C# `m.Path.Count` is the tiles this main move walked (the trigger
    // payload's `move_total`).
    let x = trigger::move_total();
    if x <= 0 {
        return Ok(());
    }
    ctx::gain(
        player_id,
        x * 60,
        &Msg::new(key!("endless_journey_gain"))
            .card("card", ID)
            .i("n", x as i64),
    )?;
    Ok(())
}
