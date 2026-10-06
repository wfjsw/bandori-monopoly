//! `MyGO:难以复刻的奇迹` -- C# `CardMiracle` (MatchHost.cs:7010-7086): place
//! this card, move every band-skill crystal onto it, and spend those crystals
//! instead of money when building; after passing every other player once,
//! collect the "last three digits" purse of the crystal-closest rival.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:难以复刻的奇迹`）:
//! > 难以复刻的奇迹：
//! >
//! > （1）[手] 将此卡置于场上，将你乐队技能上的奇迹水晶全部转移至此卡上，当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费。
//! > （2）[持续] 当你[经过]场上的所有玩家各一次，获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金，然后此卡进入弃牌堆。
//! > （3）若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。
//!

use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const MIRACLE: CardDef = CardDef::new(
    "MyGO:难以复刻的奇迹",
    &[
        On::Play(None, miracle),
        On::Hook(&[HookKind::BuildBefore], mine, before_build),
        On::Hook(&[HookKind::BuildAfter], mine, after_build),
        On::Hook(&[HookKind::PassPlayer], |_| true, pass_player),
    ],
);

fn mine(player_id: i32) -> bool {
    trigger::player_id() == player_id && ctx::is_placed()
}

const ID: &str = "MyGO:难以复刻的奇迹";

fn miracle(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）[手]: 「将此卡置于场上」 -- C# `H.PlaceFromPlay(c, -1, -1, num)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("miracle_note")));
    // 规则书（1）[手]: 「将你乐队技能上的奇迹水晶全部转移至此卡上」 -- C# drains
    // `H.BandCrystals(seat)` into `PlaceFromPlay(..., crystals: num)`.
    let n = ctx::band_crystals(player_id);
    if n > 0 {
        ctx::add_band_crystals(player_id, -n, 0);
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
    if ctx::crystals() < 1 {
        return Ok(());
    }
    let t = trigger::tile();
    if t < 0 {
        return Ok(());
    }
    ctx::set_build_discount(ctx::build_cost(t), 1);
    Ok(())
}

/// （1）'s 「移除此卡上的一个奇迹水晶」 -- C# `CardMiracle.Built` spends one.
fn after_build(player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() < 1 {
        return Ok(());
    }
    ctx::add_crystals(-1, i32::MAX);
    ctx::log(player_id, &Msg::new(key!("miracle_built")));
    Ok(())
}

/// C# `CardMiracle.PassSeat` -- remember each player [经过], and when every
/// other player has been passed once, collect the closest rival's purse.
fn pass_player(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::PassPlayer
        || trigger::player_id() != player_id
        || !ctx::is_placed()
    {
        return Ok(());
    }
    let other = trigger::target();
    if other < 0 {
        return Ok(());
    }
    // C# `HashSet<int> _passed` over `H.Others(Seat)`; a slot bitset stands in.
    let key = "miracle_passed";
    let mask = ctx::slot(player_id, key);
    let bit = 1i32 << other;
    if mask & bit != 0 {
        return Ok(());
    }
    ctx::set_slot(player_id, key, mask | bit);
    let others = ctx::others(player_id);
    let all = others.iter().fold(0i32, |m, &o| m | (1i32 << o));
    if all == 0 || ctx::slot(player_id, key) & all != all {
        return Ok(());
    }
    // 规则书（2）: 「获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金，然后此卡
    // 进入弃牌堆。」 -- C# `Reward()`: `mine = PlacedOf(Player).Sum(Crystals) +
    // BandCrystals(Player)`, pick the rival whose total is closest to `mine`, and
    // gain `abs(money(rival)) % 1000`.
    let mine = field_crystals(player_id) + ctx::band_crystals(player_id);
    let mut best = -1;
    let mut best_d = i32::MAX;
    for o in others {
        let d = (field_crystals(o) + ctx::band_crystals(o) - mine).abs();
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
        );
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
        .map(|(uid, _)| ctx::crystals_at(uid))
        .sum()
}
