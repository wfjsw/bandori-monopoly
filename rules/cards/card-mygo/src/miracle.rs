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

use card_sdk::{ctx, key, CardDef, Msg};

pub const MIRACLE: CardDef = CardDef {
    id: "MyGO:难以复刻的奇迹",
    play: Some(miracle),
    can_react: None,
    react: None,
    why_not: None,
};

fn miracle(seat: i32) {
    // 规则书（1）[手]: 「将此卡置于场上」 -- C# `H.PlaceFromPlay(c, -1, -1, num)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "MyGO:难以复刻的奇迹", &Msg::new(key!("miracle_note")));
    ctx::log(seat, &Msg::new(key!("miracle_placed")).seat("who", seat));
    // TODO(规则书)（1）[手]: 「将你乐队技能上的奇迹水晶全部转移至此卡上」 -- needs a
    // per-card crystal counter as the destination (C# `H.AddBandCrystals(seat,
    // -num, ...)` into `PlaceFromPlay(..., crystals: num)`). Draining
    // `band_crystals` without that sink would lose the crystals, so the
    // transfer is not started here.
    // TODO(规则书)（1）[手]: 「当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费」
    // -- needs the Fx.BuildCost / Fx.Built hooks (C# `CardMiracle.BuildCost`
    // returns 0 while a crystal remains, `Built` spends one).
    // TODO(规则书)（2）: 「[持续] 当你[经过]场上的所有玩家各一次，获得相当于场上奇迹水晶数
    // 与你最相近的玩家资金后三位的资金，然后此卡进入弃牌堆。」 -- needs the
    // Fx.PassSeat hook (C# `CardMiracle.PassSeat` over `H.Others(Seat)`) plus
    // per-card crystal sums to pick the closest rival. The purse itself is
    // `abs(ctx::money(rival)) % 1000` once the rival is chosen.
    // TODO(规则书)（3）: 「若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。」 -- needs
    // the card crystal counter and a wasted/effective flag (C# `H.CardWasted`).
}