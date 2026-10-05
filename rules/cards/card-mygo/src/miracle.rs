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

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const MIRACLE: CardDef = CardDef::new("MyGO:难以复刻的奇迹", &[
    On::Play(miracle),
    On::Hook(&[TriggerKind::PassPlayer], pass_player),
]);

const ID: &str = "MyGO:难以复刻的奇迹";

fn miracle(player_id: i32) {
    // 规则书（1）[手]: 「将此卡置于场上」 -- C# `H.PlaceFromPlay(c, -1, -1, num)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("miracle_note")));
    // 规则书（1）[手]: 「将你乐队技能上的奇迹水晶全部转移至此卡上」 -- C# drains
    // `H.BandCrystals(seat)` into `PlaceFromPlay(..., crystals: num)`.
    let n = ctx::band_crystals(player_id);
    if n > 0 {
        ctx::add_band_crystals(player_id, -n, 0);
    }
    ctx::set_crystals(player_id, n);
    ctx::log(
        player_id,
        &Msg::new(key!("miracle_crystals_moved")).player_id("who", player_id).i("n", n as i64),
    );
    ctx::log(player_id, &Msg::new(key!("miracle_placed")).player_id("who", player_id));
    // TODO(规则书)（1）[手]: 「当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费」
    // -- needs the Fx.BuildCost / Fx.Built hooks (C# `CardMiracle.BuildCost`
    // returns 0 while a crystal remains, `Built` spends one); the build cost
    // query is part of the build routine, not a declared hook kind.
}

/// C# `CardMiracle.PassSeat` -- remember each player [经过], and when every
/// other player has been passed once, collect the closest rival's purse.
fn pass_player(player_id: i32) {
    if trigger::kind() != TriggerKind::PassPlayer
        || trigger::player_id() != player_id
        || !ctx::is_placed(player_id)
    {
        return;
    }
    let other = trigger::target();
    if other < 0 {
        return;
    }
    // C# `HashSet<int> _passed` over `H.Others(Seat)`; a slot bitset stands in.
    let key = "miracle_passed";
    let mask = ctx::slot(player_id, key);
    let bit = 1i32 << other;
    if mask & bit != 0 {
        return;
    }
    ctx::set_slot(player_id, key, mask | bit);
    let others = ctx::others(player_id);
    let all = others.iter().fold(0i32, |m, &o| m | (1i32 << o));
    if all == 0 || ctx::slot(player_id, key) & all != all {
        return;
    }
    // 规则书（2）: 「获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金，然后此卡
    // 进入弃牌堆。」 -- C# `Reward()`: `mine = PlacedOf(Player).Sum(Crystals) +
    // BandCrystals(Player)`, pick the rival whose total is closest to `mine`, and
    // gain `abs(money(rival)) % 1000`.
    let mine = ctx::crystals(player_id) + ctx::band_crystals(player_id);
    let mut best = -1;
    let mut best_d = i32::MAX;
    for o in others {
        // Best effort: only the rival's band crystals are readable (see the
        // TODO below on per-card crystal sums).
        let d = (ctx::band_crystals(o) - mine).abs();
        if d < best_d {
            best_d = d;
            best = o;
        }
    }
    if best < 0 {
        return;
    }
    let purse = ctx::money(best).abs() % 1000;
    ctx::log(
        player_id,
        &Msg::new(key!("miracle_reward")).player_id("who", best).n("money", purse as i64),
    );
    if purse > 0 {
        ctx::gain(player_id, purse, &Msg::new(key!("miracle_reward")).player_id("who", best).n("money", purse as i64));
    }
    let left = ctx::crystals(player_id) > 0;
    // C# `H.Unplace(this, "discard", left ? "上面还有奇迹水晶：视为没有生效" : "完成了")`.
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, ID);
    if left {
        // TODO(规则书)（3）: 「若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。」 -- needs
        // PlayCtx.Effective / `H.CardWasted` (C# `H.CardWasted(Seat, Id)` when the
        // card is discarded with crystals left).
        ctx::log(player_id, &Msg::new(key!("miracle_wasted")).player_id("who", player_id));
    }
}

// TODO(规则书)（2）: 「获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金」 -- the
// `PassPlayer` timing maps to `TriggerKind::PassPlayer`, but two halves stay
// unmapped: the engine must still raise `passPlayer` (it only raises `pass` today),
// and the closest-rival pick needs per-card crystal sums over *every* placed
// card (`H.PlacedOf(p).Sum(x => x.Crystals)`), which `ctx::crystals` cannot see
// (it reads only the running card's own counter).
