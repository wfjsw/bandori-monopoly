//! `PPP:Tomorrow's Door` -- C# `CardTomorrowsDoor` (MatchHost.cs:8582-8657): a
//! route marker that walks 流星堂 -> 大阪中之岛公园 as its user passes it, then
//! taxes settles from the owner's field.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Tomorrow's Door`）:
//! > Tomorrow's Door：
//! >
//! > （1）此卡指定的序列（从前往后）为“流星堂”，“花咲川女子学院”，“Space”，“Live House Galaxy”，“武道馆”，“RiNG 1”，“RiNG 2”，“RiNG 3”，“RiNG 4”，“大阪中之岛公园”
//! >
//! > （2）将此卡放置在“流星堂”上，此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡
//! > （1）效果的序列中的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域
//! >
//! > （3）此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。
//!
//! `play` only places the card; the travelling and the tax are hooks the ABI does
//! not carry yet (TODO below).

use card_sdk::{ctx, key, CardDef, Msg};

pub const TOMORROWS_DOOR: CardDef = CardDef {
    id: "PPP:Tomorrow's Door",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

/// 规则书（1）: 「此卡指定的序列（从前往后）为…」 -- C# `CardTomorrowsDoor.Route`.
/// Source of truth for the `Mem["step"]` cursor once the PassTile hook exists.
#[allow(dead_code)]
const ROUTE: [&str; 10] = [
    "流星堂",
    "花咲川女子学院",
    "Space",
    "Live House Galaxy",
    "武道馆",
    "RiNG 1",
    "RiNG 2",
    "RiNG 3",
    "RiNG 4",
    "大阪中之岛公园",
];

fn play(seat: i32) {
    // 规则书（2）: 「将此卡放置在“流星堂”上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // H.TileNamed(Route[0]))`. The card stays in play.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PPP:Tomorrow's Door", &Msg::new(key!("tomorrows_door_note")));
    ctx::log(seat, &Msg::new(key!("tomorrows_door_placed")).seat("who", seat));
    // TODO(ABI): 「将此卡放置在“流星堂”上」 -- the placement is bound to the 流星堂
    //   tile (C# `H.PlaceFromPlay(c, owner, tile)` + per-card `Mem["step"]`), not the
    //   seat's field; the ABI's `place_card` only parks the card at a seat. Keep the
    //   route table above as the source of truth for `step` (`ROUTE[0]` is 流星堂).
    // TODO(ABI)（2）: 「此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡（1）效果的
    //   序列中的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域」
    //   -- needs the Fx.PassTile hook (C# `CardTomorrowsDoor.PassTile`), the per-card
    //   `Mem["step"]` cursor, and tile re-placement (`card.Tile = H.TileNamed(...)` /
    //   `Tile = -1` for the owner's field).
    // TODO(ABI)（3）: 「此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦
    //   开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。」
    //   -- needs the Fx.SettleAfter hook (C# `CardTomorrowsDoor.SettleAfter`) plus
    //   this card's owner/field state. The house count itself is now a query
    //   (`ctx::houses_of(H.TileNamed("星之鼓动山丘"))`); payment would be
    //   `ctx::transfer(other, owner, houses * 100, &...)`.
}