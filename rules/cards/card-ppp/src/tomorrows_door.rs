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

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const TOMORROWS_DOOR: CardDef = CardDef::new("PPP:Tomorrow's Door", &[
    On::Play(play),
    On::Hook(&[TriggerKind::SettleAfter], settle_after),
]);

/// 规则书（1）: 「此卡指定的序列（从前往后）为…」 -- C# `CardTomorrowsDoor.Route`.
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

/// Where the route cursor is written down (C# `Mem["step"]`). `ROUTE.len()`
/// means the route is finished and the card sits in the owner's play area
/// (C# `Tile = -1`), which is when the tax (3) applies. While the cursor is
/// below `ROUTE.len()` the card is still travelling and the tax is dormant.
const SLOT_STEP: &str = "tomorrows_door_step";

fn play(player_id: i32) {
    // 规则书（2）: 「将此卡放置在“流星堂”上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // H.TileNamed(Route[0]))`. The card stays in play.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PPP:Tomorrow's Door", &Msg::new(key!("tomorrows_door_note")));
    // The route cursor starts at stop 0 (travelling; C# `Mem["step"] = 0`).
    ctx::set_slot(player_id, SLOT_STEP, 0);
    ctx::log(player_id, &Msg::new(key!("tomorrows_door_placed")).player_id("who", player_id));
    // TODO(ABI): 「将此卡放置在“流星堂”上」 -- the placement is bound to the 流星堂
    //   tile (C# `H.PlaceFromPlay(c, owner, tile)` + per-card `Mem["step"]`), not the
    //   player's field; the ABI's `place_card` only parks the card at a player. Keep the
    //   route table above as the source of truth for `step` (`ROUTE[0]` is 流星堂).
    // TODO(ABI)（2）: 「此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡（1）效果的
    //   序列中的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域」
    //   -- needs the Fx.PassTile hook (C# `CardTomorrowsDoor.PassTile`), the per-card
    //   `Mem["step"]` cursor, and tile re-placement (`card.Tile = H.TileNamed(...)` /
    //   `Tile = -1` for the owner's field). When it lands it must advance
    //   `SLOT_STEP`; at `ROUTE.len()` the card is in the play area and the tax
    //   below wakes up.
}

/// `Fx.SettleAfter` (C# `CardTomorrowsDoor.SettleAfter`) -- once the card sits
/// in the owner's play area, another player settling on the owner's land or on
/// 梦开始的地方 pays `houses(星之鼓动山丘) × 100` to the owner.
fn settle_after(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    // C# `Tile >= 0` -- still travelling along the route: no tax. The cursor is
    // the stand-in for `Tile`; it only reaches `ROUTE.len()` once (2) lands.
    if ctx::slot(player_id, SLOT_STEP) < ROUTE.len() as i32 {
        return;
    }
    // C# `m.Seat == Seat` -- the owner's own settle is not taxed.
    let payer = trigger::player_id();
    if payer == player_id || ctx::player_out(payer) {
        return;
    }
    // 规则书（3）: 「[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时
    //   额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。」
    let at = trigger::tile();
    if at < 0 {
        return;
    }
    let dream = ctx::tile_named("梦开始的地方");
    let hill = ctx::tile_named("星之鼓动山丘");
    if ctx::tile_owner(at) != player_id && !(dream >= 0 && at == dream) {
        return;
    }
    let houses = if hill >= 0 { ctx::houses_of(hill) } else { 0 };
    let due = houses * 100;
    if due <= 0 {
        return;
    }
    ctx::transfer(payer, player_id, due, &Msg::new(key!("tomorrows_door_tax")).player_id("who", player_id).player_id("target", payer).n("money", due as i64));
}
