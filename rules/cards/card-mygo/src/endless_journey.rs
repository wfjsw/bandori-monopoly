//! `MyGO:哪怕这旅程没有终点` -- C# `CardEndlessJourney` (MatchHost.cs:6971-7009):
//! plant this card on the current tile with 4 miracle crystals; settles pay
//! out 60 per tile walked, and a long main move burns a crystal.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:哪怕这旅程没有终点`）:
//! > 哪怕这旅程没有终点：
//! >  [手] 将此卡放置于当前格子上并为其放置4个奇迹水晶，每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶。当此卡奇迹水晶数量为0时，将此卡置入弃牌堆。
//! > （2）[持续] 触发结算时，获得X*60资金，X为你此次主要移动[经过]的格数
//!

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const ENDLESS_JOURNEY: CardDef = CardDef::new("MyGO:哪怕这旅程没有终点", &[
    On::Play(None, endless_journey),
    On::Hook(&[HookKind::TurnEnd], |_| true, turn_end),
    On::Hook(&[HookKind::SettleAfter], |_| true, settle_after),
    On::Hook(&[HookKind::CrystalsChanged], crystals_changed_guard, on_crystals_changed)]);

const ID: &str = "MyGO:哪怕这旅程没有终点";

fn endless_journey(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置于当前格子上」 -- bound to where the player is.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card_on(player_id, ctx::player_pos(player_id), ID, &Msg::new(key!("endless_journey_note")));
    // 规则书[手]: 「并为其放置4个奇迹水晶」 -- the placement's crystal charge.
    ctx::set_crystals(4);
    ctx::log(player_id, &Msg::new(key!("endless_journey_placed")).player_id("who", player_id));
    Ok(())
}

/// C# `CardEndlessJourney.TurnEnd` -- when the owner's main move walked more
/// than 6 steps, burn one miracle crystal; at 0 the card is discarded.
fn turn_end(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::TurnEnd
        || trigger::player_id() != player_id
        || !ctx::is_placed()
    {
        return Ok(());
    }
    // 规则书[手]: 「每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶。当此卡奇迹
    // 水晶数量为0时，将此卡置入弃牌堆。」 -- C# `H._turnCtx.LastMain > 6` ->
    // `AddCrystals(-1)` and `H.Unplace(this, "discard", ...)` at 0.
    if ctx::turn_main_steps() <= 6 {
        return Ok(());
    }
    ctx::decay();
    Ok(())
}

/// 规则书[手]: 「当此卡奇迹水晶数量为0时，将此卡置入弃牌堆」 -- C#
/// `H.Unplace(this, "discard", ...)`.
///
/// Listens to this card's own [`HookKind::CrystalsChanged`] rather than being
/// re-checked at the decay tick, so a count emptied by *any* write leaves the
/// field just the same.
/// Pure guard for [`on_crystals_changed`] -- the activation gate. `false`
/// means the card is not activated at all.
fn crystals_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && ctx::crystals() == 0
        // Only a write that did not raise the count speaks for the empty
        // state; see AG:绯红之魂 (3).
        && trigger::value() <= 0
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(player_id, &Msg::new(key!("endless_journey_decayed")).player_id("who", player_id));
    Ok(())
}

/// C# `CardEndlessJourney.SettleAfter` -- when the owner settles off their own
/// main move, pay out 60 per tile that walk passed.
fn settle_after(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::SettleAfter || !ctx::is_placed() {
        return Ok(());
    }
    // C# `m.Seat != Seat || !m.Main || m.Path.Count == 0` -- only the owner's
    // own main move pays out (`t.Move` is the move that just settled).
    if trigger::player_id() != player_id || !trigger::move_is_main() {
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
        &Msg::new(key!("endless_journey_gain")).card("card", ID).i("n", x as i64),
    );
    Ok(())
}
