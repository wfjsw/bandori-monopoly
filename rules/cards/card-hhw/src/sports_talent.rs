//! `HHW:运动的天赋` -- C# `CardSportsTalent` (MatchHost.cs:4025-4091).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:运动的天赋`）:
//! > 运动的天赋：
//! >  将此卡放置于自己场上并放置3个奇迹水晶，每回合结束时失去一个，为0时置入弃牌堆。此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100。每次移动掷骰时，重骰移动掷骰直至结果为10以上为止。
//!

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "HHW:运动的天赋";

pub const SPORTS_TALENT: CardDef = CardDef::new("HHW:运动的天赋", &[
    On::Play(None, play),
    On::Hook(&[HookKind::TurnEnd, HookKind::RollAfter], react_guard, react),
    On::RollPlan(roll_plan),
    On::Hook(&[HookKind::CrystalsChanged], crystals_changed_guard, on_crystals_changed)]);

/// C# `CardSportsTalent.Next` / `Mem["next"]` -- the next dice-roll reward
/// (starts at 700, drops by 100, floors at 100). The player slot stands in for
/// the per-field-card `Mem` map.
const SLOT_NEXT: &str = "sports_talent_next";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「将此卡放置于自己场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("sports_talent_note")));
    // 规则书: 「并放置3个奇迹水晶」 -- the placement's crystal charge (C#
    // `H.PlaceFromPlay(c, -1, -1, 3)`).
    ctx::set_crystals(3);
    ctx::log(player_id, &Msg::new(key!("sports_talent_placed")).player_id("who", player_id));
    // (the turn-end decay and the dice-roll reward run in `react`, which the Fx
    // hook dispatch runs at `turnEnd` / `rollAfter`.)
    Ok(())
}

/// C# `CardSportsTalent : DecayCard` + `RollAfter`.
/// Pure guard for [`react`] -- the activation gate. `false`
/// means the card is not activated at all.
fn react_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn react(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        // 规则书: 「每回合结束时失去一个」 -- C# `DecayCard.TurnEnd`
        // (`turn == DecayOn` = the owner's turn) -> `AddCrystals(-1)`. The
        // 「为0时置入弃牌堆」 half is [`on_crystals_changed`].
        TriggerKind::TurnEnd => {
            if trigger::player_id() != player_id {
                return Ok(());
            }
            ctx::decay();
        }
        // 规则书: 「此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100」
        // -- C# `CardSportsTalent.RollAfter` (`m.Seat == Seat && m.Main &&
        // m.FixedRoll < 0`) -> `Reward` (`GainR(Player, Next)` with `Next` walking
        // 700 -> 600 -> ... -> 100).
        TriggerKind::RollAfter => {
            if trigger::player_id() != player_id || !trigger::move_is_main() {
                return Ok(());
            }
            // C# `m.FixedRoll >= 0` skips the reward on a plan's fixed roll.
            if trigger::move_roll().is_none() {
                return Ok(());
            }
            reward(player_id);
            // 规则书: 「每次移动掷骰时，重骰移动掷骰直至结果为10以上为止」
            reroll_to_ten(player_id);
        }
        _ => {}
    }
    Ok(())
}

/// C# `CardSportsTalent.Reward` -- pay the current `Next` and step it down.
fn reward(player_id: i32) {
    let mut next = ctx::slot(player_id, SLOT_NEXT);
    if next <= 0 {
        next = 700;
    }
    ctx::set_slot(player_id, SLOT_NEXT, (next - 100).max(100));
    ctx::gain(player_id, next, &Msg::new(key!("sports_talent_reward")).n("money", next as i64));
}

/// 规则书: 「每次移动掷骰时，重骰移动掷骰直至结果为10以上为止」 -- C#
/// `MoveCtx.MinRoll` clamps the final face up to 10 after the reactions
/// (`if (!m.Signed) m.Roll = max(m.MinRoll, m.Roll)`); `set_min_roll` is that
/// plan field, shaped here before the dice (`On::RollPlan`). Same gate as the
/// C# `RollAfter` (`m.Seat == Seat && m.Main && m.FixedRoll < 0`).
fn roll_plan(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    // `On::RollPlan` runs on the move being planned; `turn_player` is its mover.
    if ctx::turn_player() != player_id {
        return Ok(());
    }
    if ctx::fixed_roll().is_some() {
        return Ok(());
    }
    ctx::plan::set_min_roll(10);
    Ok(())
}

/// 规则书: 「每次移动掷骰时，重骰移动掷骰直至结果为10以上为止」 -- reroll with
/// `H.DoMoveRoll` until the face is at least 10 (at most 10 attempts, bailing
/// out when the player is out). Called from the `RollAfter` branch of [`react`].
fn reroll_to_ten(player_id: i32) {
    let mut x = trigger::move_roll().unwrap_or(trigger::value());
    for _ in 0..10 {
        if x >= 10 || ctx::player_out(player_id) {
            break;
        }
        x = ctx::do_move_roll(player_id).max(0);
        trigger::set_move_roll(x);
    }
    ctx::log(player_id, &Msg::new(key!("sports_talent_reroll")).player_id("who", player_id).i("n", x as i64));
}

/// 规则书: 「为0时置入弃牌堆」 -- C# `Empty()` / `H.Unplace(this, "discard")`.
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
    ctx::unplace_self();
    ctx::to_discard(player_id, ID);
    ctx::log(player_id, &Msg::new(key!("sports_talent_decayed")).player_id("who", player_id));
    Ok(())
}
