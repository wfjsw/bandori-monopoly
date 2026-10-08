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
use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger, CardPile};
use card_sdk::{key, CardDef, Msg, On};

pub const TOMORI_NO_LONGER: CardDef = CardDef::new(
    "MyGO:（灯）不再迷茫",
    &[
        On::Play(None, tomori_no_longer, ""),
        On::Hook(&[HookKind::TurnEnd], None, sweep, card_sdk::pre::MINE),
        On::Hook(&[HookKind::FireSpent], None, cover, card_sdk::pre::MINE),
        // Sheet I3 (2) 「当此卡上的水晶由于此效果以外的原因减少时，此卡立刻
        // 获得等同于减少量的[奇迹水晶]」.
        On::Hook(&[HookKind::CrystalsChanged], None, refill, card_sdk::pre::MINE),
    ],
)
    .legacy(&[(1, legacy_mine), (2, legacy_mine), (3, legacy_mine)]);

/// Set while (2)'s substitution is running, so [`refill`] knows the decrease
/// is the skill-cost swap and must not be refunded.
const SLOT_COVERING: &str = "tomori_no_longer.covering";

/// （2）'s substitution.
fn cover(player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() < 1 {
        return Ok(());
    }
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
    ctx::set_slot(player_id, SLOT_COVERING, 1);
    ctx::add_crystals(-1, i32::MAX)?;
    ctx::set_slot(player_id, SLOT_COVERING, 0);
    ctx::gain_fire(player_id, spent, &Msg::new(key!("tomori_no_longer_refund")))?;
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_no_longer_covered")).i("n", spent as i64),
    );
    Ok(())
}

/// Sheet I3 (2): refund any crystal decrease that is not the skill-cost
/// substitution.
fn refill(player_id: i32) -> card_sdk::Asked {
    let delta = trigger::value();
    if delta >= 0 {
        return Ok(());
    }
    // The substitution (`cover`) sets this flag around its own `-1`.
    if ctx::slot(player_id, SLOT_COVERING) != 0 {
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
    ctx::give_stay(player_id, 2);
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

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （3）「此卡上的奇迹水晶耗尽后，[移除]此卡」.
fn sweep(_player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() > 0 {
        return Ok(());
    }
    if !ctx::is_placed() {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Graveyard);
    Ok(())
}
