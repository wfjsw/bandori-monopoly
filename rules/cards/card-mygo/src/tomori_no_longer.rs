//! `MyGO:（灯）不再迷茫` -- C# `CardTomoriNoLonger` (MatchHost.cs:6308-6347):
//! take two locked [停留] layers, place this card, bin the whole hand and load
//! it with X+1 miracle crystals (X = cards discarded).
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（灯）不再迷茫`）:
//! > （灯）不再迷茫：
//! >
//! > （1）获得两层仅能被自然流失效果移除的[停留]，将此卡放置于场上并将所有手卡置入弃牌堆，在此卡上放置X+1个[奇迹水晶]，X为你弃置的手牌数
//! > （2）你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗
//! > （3）此卡上的奇迹水晶耗尽后，[移除]此卡
//!

use alloc::vec::Vec;

use card_sdk::ctx::{self, CardPile};
use card_sdk::{key, CardDef, Msg, On};
use alloc::string::String;

pub const TOMORI_NO_LONGER: CardDef = CardDef::new("MyGO:（灯）不再迷茫", &[
    On::Play(tomori_no_longer),
]);

const ID: &str = "MyGO:（灯）不再迷茫";

fn tomori_no_longer(player_id: i32) {
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
    ctx::set_crystals(player_id, discarded + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_no_longer_discarded"))
            .player_id("who", player_id)
            .i("n", discarded as i64)
            .i("crystals", (discarded + 1) as i64),
    );
    ctx::log(player_id, &Msg::new(key!("tomori_no_longer_placed")).player_id("who", player_id));
    // TODO(规则书)（2）: 「你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐
    // 消耗」 -- needs the skill-use attachment hook (C# `CardTomoriNoLonger.Use`
    // via `H.ExtraOf`) to spend one crystal instead of the skill's fire cost.
    // TODO(规则书)（3）: 「此卡上的奇迹水晶耗尽后，[移除]此卡」 -- the C# exhaustion
    // path lives inside `Use` (`H.Unplace(this, "removed", ...)` when the spend
    // takes the last crystal); held together with (2).
}