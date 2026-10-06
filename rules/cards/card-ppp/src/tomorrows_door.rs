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

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const TOMORROWS_DOOR: CardDef = CardDef::new("PPP:Tomorrow's Door", &[
    On::Play(None, play),
    On::Hook(&[HookKind::PassTile], pass_tile_guard, pass_tile),
    On::Hook(&[HookKind::SettleAfter], settle_after_guard, settle_after)]);

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
    "大阪中之岛公园"];

/// Where the route cursor is written down (C# `Mem["step"]`). `ROUTE.len()`
/// means the route is finished and the card sits in the owner's play area
/// (C# `Tile = -1`), which is when the tax (3) applies. While the cursor is
/// below `ROUTE.len()` the card is still travelling and the tax is dormant.
const SLOT_STEP: &str = "tomorrows_door_step";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「将此卡放置在“流星堂”上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // H.TileNamed(Route[0]))`. The card stays in play.
    ctx::set_dest(ctx::Dest::Field);
    // 规则书（2）: 「将此卡放置在“流星堂”上」 -- bound to `ROUTE[0]`; the route
    // cursor is the stand-in for the per-card `Mem["step"]`.
    ctx::place_card_on(player_id, ctx::tile_named(ROUTE[0]), "PPP:Tomorrow's Door", &Msg::new(key!("tomorrows_door_note")));
    ctx::set_slot(player_id, SLOT_STEP, 0);
    ctx::log(player_id, &Msg::new(key!("tomorrows_door_placed")).player_id("who", player_id));
    Ok(())
}

/// 规则书（2）: 「此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡（1）效果的序列中
/// 的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域」 -- the
/// hop runs on the user's own pass over wherever the card currently sits.
/// Pure guard for [`pass_tile`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pass_tile_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

fn pass_tile(player_id: i32) -> card_sdk::Asked {
    let step = ctx::slot(player_id, SLOT_STEP);
    if step < 0 || step >= ROUTE.len() as i32 {
        return Ok(());
    }
    let here = ctx::self_tile().unwrap_or(-1);
    if here < 0 || trigger::tile() != here {
        return Ok(());
    }
    let next = step + 1;
    ctx::set_slot(player_id, SLOT_STEP, next);
    // 「如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域」 --
    // the last stop is 大阪中之岛公园; past it the card goes back to the owner
    // (`tile = -1`), which is when the tax in `settle_after` wakes up.
    if next >= ROUTE.len() as i32 {
        ctx::set_self_tile(-1);
    } else {
        ctx::set_self_tile(ctx::tile_named(ROUTE[next as usize]));
    }
    ctx::log(
        player_id,
        &Msg::new(key!("tomorrows_door_hop")).player_id("who", player_id).i("n", next as i64),
    );
    Ok(())
}

/// `Fx.SettleAfter` (C# `CardTomorrowsDoor.SettleAfter`) -- once the card sits
/// in the owner's play area, another player settling on the owner's land or on
/// 梦开始的地方 pays `houses(星之鼓动山丘) × 100` to the owner.
/// Pure guard for [`settle_after`] -- the activation gate. `false`
/// means the card is not activated at all.
fn settle_after_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn settle_after(player_id: i32) -> card_sdk::Asked {
    // C# `Tile >= 0` -- still travelling along the route: no tax. The cursor is
    // the stand-in for `Tile`; it only reaches `ROUTE.len()` once (2) lands.
    if ctx::slot(player_id, SLOT_STEP) < ROUTE.len() as i32 {
        return Ok(());
    }
    // C# `m.Seat == Seat` -- the owner's own settle is not taxed.
    let payer = trigger::player_id();
    if payer == player_id || ctx::player_out(payer) {
        return Ok(());
    }
    // 规则书（3）: 「[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时
    //   额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。」
    let at = trigger::tile();
    if at < 0 {
        return Ok(());
    }
    let dream = ctx::tile_named("梦开始的地方");
    let hill = ctx::tile_named("星之鼓动山丘");
    if ctx::tile_owner(at) != player_id && !(dream >= 0 && at == dream) {
        return Ok(());
    }
    let houses = if hill >= 0 { ctx::houses_of(hill) } else { 0 };
    let due = houses * 100;
    if due <= 0 {
        return Ok(());
    }
    ctx::transfer(payer, player_id, due, &Msg::new(key!("tomorrows_door_tax")).player_id("who", player_id).player_id("target", payer).n("money", due as i64))?;
    Ok(())
}
