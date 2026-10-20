//! `MyGO:难以复刻的奇迹` -- C# `CardMiracle` (MatchHost.cs:7010-7086): place
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:难以复刻的奇迹`）:
//! > 难以复刻的奇迹：
//!
//! > （1）[手] 将此卡置于场上，将你乐队技能上的奇迹水晶全部转移至此卡上，当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费。
//! > （2）[持续] 当你[经过]场上的所有玩家各一次，获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金，然后此卡进入弃牌堆。
//! > （3）若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。
//!
//! this card, move every band-skill crystal onto it, and spend those crystals
//! instead of money when building; after passing every other player once,
//! collect the "last three digits" purse of the crystal-closest rival.

use card_sdk::abi::{counter, ActionKind};
use bandori_std::ctx::{self, trigger};
use card_sdk::{Action, CardDef, key, Msg, On};

pub const MIRACLE: CardDef = CardDef::new(
    "MyGO:难以复刻的奇迹",
    &[
        On::Play("", "", None, miracle),
        On::Action(Action::BuildBefore("actor == owner && card.counter('crystals') >= 1 && card.placed && tile.id >= 0", "", None, before_build)),
        On::Action(Action::BuildAfter("actor == owner && card.counter('crystals') >= 1 && card.placed", "", None, after_build)),
        // （2）「当你[经过]场上的所有玩家各一次」 -- 行动阶段 12 [经过]
        // (`SETTLE-STAGES.md` §4 M4): each step onto a tile a player stands on
        // counts, not only the end-tile [重叠]. Bookkeeping (the running mask)
        // is silent; the reward is the visible entry.
        On::Action(Action::PassTile("actor == owner && card.placed && players_on(tile.id, owner) > 0", "", Some(record_pass_applies), record_pass)),
        On::Action(Action::PassTile("actor == owner && card.placed", "", Some(all_others_passed), pass_player)),
    ],
)
.silent(&[3]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"难以复刻的奇迹"#)
        .raw_name(r#"难以复刻的奇迹"#)
        .tags(&[r#"手"#, r#"持续"#])
        .text(r#"（1）[手] 将此卡置于场上，将你乐队技能上的奇迹水晶全部转移至此卡上，当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费。（2）[持续] 当你[经过]场上的所有玩家各一次，获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金，然后此卡进入弃牌堆。（3）若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。"#)
        .cell(r#"I15"#)
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", "")]));

const ID: &str = "MyGO:难以复刻的奇迹";

fn miracle(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）[手]: 「将此卡置于场上」 -- C# `H.PlaceFromPlay(c, -1, -1, num)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("miracle_note")));
    // 规则书（1）[手]: 「将你乐队技能上的奇迹水晶全部转移至此卡上」 -- C# drains
    // `H.BandCrystals(seat)` into `PlaceFromPlay(..., crystals: num)`.
    let n = ctx::group_crystals(player_id);
    if n > 0 {
        ctx::add_group_crystals(player_id, -n, 0);
    }
    ctx::set_crystals(n);
    ctx::log(
        player_id,
        &Msg::new(key!("miracle_crystals_moved"))
            .player_id("who", player_id)
            .i("n", n as i64),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("miracle_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// （1）「当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费」 -- C#
/// `CardMiracle.BuildCost` returns 0 while a crystal remains.
fn before_build(player_id: i32) -> card_sdk::Asked {
    let t = trigger::tile();
    ctx::set_build_discount(ctx::build_cost(t), 1);
    Ok(())
}

/// （1）'s 「移除此卡上的一个奇迹水晶」 -- C# `CardMiracle.Built` spends one.
fn after_build(player_id: i32) -> card_sdk::Asked {
    ctx::add_crystals(-1, i32::MAX)?;
    ctx::log(player_id, &Msg::new(key!("miracle_built")));
    Ok(())
}

/// Residual guard for [`record_pass`]: this step actually adds a new
/// [经过] bit. Derived scan over the running mask (docs/GUARDS.md §4.2).
fn record_pass_applies(player_id: i32) -> bool {
    let key = "miracle_passed";
    let mask = ctx::slot(player_id, key);
    for other in ctx::players_on(trigger::tile(), player_id) {
        if other < 0 {
            continue;
        }
        if mask & (1i32 << other) == 0 {
            return true;
        }
    }
    false
}

/// C# `CardMiracle.PassSeat` bookkeeping: remember each player [经过].
/// Silent -- the player never sees the running mask.
fn record_pass(player_id: i32) -> card_sdk::Asked {
    let key = "miracle_passed";
    let mut mask = ctx::slot(player_id, key);
    for other in ctx::players_on(trigger::tile(), player_id) {
        if other < 0 {
            continue;
        }
        mask |= 1i32 << other;
    }
    ctx::set_slot(player_id, key, mask);
    Ok(())
}

/// Residual guard for the reward: every other player has been passed once.
/// Derived scan over the running mask (docs/GUARDS.md §4.2).
fn all_others_passed(player_id: i32) -> bool {
    let mask = ctx::slot(player_id, "miracle_passed");
    let others = ctx::others(player_id);
    let all = others.iter().fold(0i32, |m, &o| m | (1i32 << o));
    all != 0 && mask & all == all
}

/// C# `CardMiracle.PassSeat` -- when every other player has been passed once,
/// collect the closest rival's purse. Applicability is [`all_others_passed`].
fn pass_player(player_id: i32) -> card_sdk::Asked {
    let others = ctx::others(player_id);
    // 规则书（2）: 「获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金，然后此卡
    // 进入弃牌堆。」 -- C# `Reward()`: `mine = PlacedOf(Player).Sum(Crystals) +
    // BandCrystals(Player)`. Band-card crystals live on the band skill's own
    // field instance now, so `field_crystals` already counts them -- the C# sum
    // is one term, not two.
    let mine = field_crystals(player_id);
    let mut best = -1;
    let mut best_d = i32::MAX;
    for o in others {
        let d = (field_crystals(o) - mine).abs();
        if d < best_d {
            best_d = d;
            best = o;
        }
    }
    if best < 0 {
        return Ok(());
    }
    let purse = ctx::money_of(best).abs() % 1000;
    ctx::log(
        player_id,
        &Msg::new(key!("miracle_reward"))
            .player_id("who", best)
            .n("money", purse as i64),
    );
    if purse > 0 {
        ctx::gain(
            player_id,
            purse,
            &Msg::new(key!("miracle_reward"))
                .player_id("who", best)
                .n("money", purse as i64),
        )?;
    }
    let left = ctx::crystals() > 0;
    // C# `H.Unplace(this, "discard", left ? "上面还有奇迹水晶：视为没有生效" : "完成了")`.
    ctx::set_dest(ctx::Dest::Graveyard);
    if left {
        // TODO(规则书)[judgement]（3）: 「若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。」 -- needs
        //   the clause under-specifies -- see the note above it
        // PlayCtx.Effective / `H.CardWasted` (C# `H.CardWasted(Seat, Id)` when the
        // card is discarded with crystals left).
        ctx::log(
            player_id,
            &Msg::new(key!("miracle_wasted")).player_id("who", player_id),
        );
    }
    Ok(())
}

/// 「场上奇迹水晶数」 -- `H.PlacedOf(p).Sum(x => x.Crystals)`: the crystals on
/// every card that player has placed, not the player's own running counter.
fn field_crystals(p: i32) -> i32 {
    ctx::field_instances(p)
        .into_iter()
        .map(|(uid, _)| ctx::counter_at(uid, counter::CRYSTALS))
        .sum()
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn miracle_kani(player_id: i32) -> card_sdk::Asked {
    miracle(player_id)
}

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
pub fn record_pass_kani(player_id: i32) -> card_sdk::Asked {
    record_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pass_player_kani(player_id: i32) -> card_sdk::Asked {
    pass_player(player_id)
}
