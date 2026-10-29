//! `PPP:Tomorrow's Door` -- C# `CardTomorrowsDoor` (MatchHost.cs:8582-8657): a
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Tomorrow's Door`）:
//! > Tomorrow's Door：
//!
//! > （1）此卡指定的序列（从前往后）为“流星堂”，“花咲川女子学院”，“Space”，“Live House Galaxy”，“武道馆”，“RiNG 1”，“RiNG 2”，“RiNG 3”，“RiNG 4”，“大阪中之岛公园”
//!
//! > （2）将此卡放置在“流星堂”上，此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡
//! > （1）效果的序列中的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域
//!
//! > （3）此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。
//!
//! route marker that walks 流星堂 -> 大阪中之岛公园 as its user passes it, then
//! taxes settles from the owner's field.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, FilterKind, TriggerKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Filter, Msg, On, PayAddValue};

pub const TOMORROWS_DOOR: CardDef = CardDef::new(
    "PPP:Tomorrow's Door",
    &[
        On::Play("", None, play),
        On::Action(Action::PassTile("card.placed && actor == owner", Some(pass_tile_guard), pass_tile)),
        // 规则书: 「[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外
        // 支付[拥有者]星之鼓动山丘上房子数量×100的资金」 -- the surcharge joins the
        // rent payment at `paySplitAdd` (before `paySplitMul`), so a 「支付减半」
        // scaler sees the shaped total.
        // TODO(规则书)（在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付）: the book
        //   gates on the settle's tile -- we key on the payment (`pay_is_rent` + the
        //   charged tile), so a rent-shaped 「收费」 outside a settle (练习室里的风暴)
        //   also collects it; needs the settle tile on the pay payload.
        On::Filter(Filter::PaySplitAdd("card.placed && pay_is_rent && actor != owner && tile.id >= 0 && tile.owner == owner", Some(pay_add_guard), pay_add)),
        // （3）「…[结算]时额外支付」 is 行动阶段 15 (`SETTLE-STAGES.md` §4 M2)
        // -- an entry in the settle's effect list, for the shapes no rent
        // payment carries (梦开始的地方, a mortgaged owner tile). A field card
        // that replaces the body skips this entry.
        On::Action(Action::SettleBody("card.placed && actor != owner && tile.id >= 0", Some(settle_body_guard), settle_body)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Tomorrow's Door"#)
        .raw_name(r#"Tomorrow's Door"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"此卡指定的序列（从前往后）为“流星堂”，“花咲川女子学院”，“Space”，“Live House Galaxy”，“武道馆”，“RiNG 1”，“RiNG 2”，“RiNG 3”，“RiNG 4”，“大阪中之岛公园”"#),
            it(r#"将此卡放置在“流星堂”上，此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡（1）效果的序列中的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域"#),
            it(r#"此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。"#),
        ]))
        // sheet cell B6
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]));

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

/// Where the route cursor is written down (C# `Mem["step"]`) -- an instance
/// prop, so two copies each carry their own. `ROUTE.len()` means the route is
/// finished and the card sits in the owner's play area (C# `Tile = -1`), which
/// is when the tax (3) applies. While the cursor is below `ROUTE.len()` the
/// card is still travelling and the tax is dormant.
const PROP_STEP: &str = "tomorrows_door_step";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「将此卡放置在“流星堂”上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // H.TileNamed(Route[0]))`. The card stays in play.
    ctx::set_dest(ctx::Dest::Field);
    // 规则书（2）: 「将此卡放置在“流星堂”上」 -- bound to `ROUTE[0]`; the route
    // cursor is the per-card `Mem["step"]` (an instance prop, so copies do not
    // share it).
    ctx::place_card_on(
        player_id,
        ctx::tile_named(ROUTE[0]),
        "PPP:Tomorrow's Door",
        &Msg::new(key!("tomorrows_door_note")),
    );
    // After `place_card_on`, so the run is bound to the freshly placed copy.
    ctx::set_prop(PROP_STEP, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("tomorrows_door_placed")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书（2）: 「此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡（1）效果的序列中
/// 的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域」 -- the
/// hop runs on the user's own pass over wherever the card currently sits.
/// Pure guard for [`pass_tile`] -- the activation gate. `false`
/// means the card is not activated at all. Also the route cursor and the
/// "am I on the tile being passed" checks -- applicability, not effect (a
/// hook body that ran and returned still flashes).
fn pass_tile_guard(_player_id: i32) -> bool {
    // `card.placed && actor == owner` is the pre. The route cursor and
    // "am I on the tile being passed" are residuals (`self_tile`).
    let step = ctx::prop(PROP_STEP);
    if step < 0 || step >= ROUTE.len() as i32 {
        return false;
    }
    let here = ctx::self_tile().unwrap_or(-1);
    here >= 0 && trigger::tile() == here
}

fn pass_tile(player_id: i32) -> card_sdk::Asked {
    let step = ctx::prop(PROP_STEP);
    let next = step + 1;
    ctx::set_prop(PROP_STEP, next);
    // 「如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域」 --
    // the last stop is 大阪中之岛公园; past it the card goes back to the owner
    // (`tile = -1`), which is when the tax's `settle_body` entry wakes up.
    if next >= ROUTE.len() as i32 {
        ctx::set_self_tile(-1);
    } else {
        ctx::set_self_tile(ctx::tile_named(ROUTE[next as usize]));
    }
    ctx::log(
        player_id,
        &Msg::new(key!("tomorrows_door_hop"))
            .player_id("who", player_id)
            .i("n", next as i64),
    );
    Ok(())
}

/// (3)'s surcharge, as a flat add on the rent payment (`paySplitAdd` runs before
/// `paySplitMul`, so a 「支付减半」 scaler sees rent + surcharge as one figure).
/// The guard carries the whole applicability: the route must be finished, the
/// payment a rent on the owner's own tile by someone else, and there must be a
/// surcharge to add. `TriggerKind::PaySplitAdd` is the category's job -- not
/// re-checked here (docs/GUARDS.md).
fn pay_add_guard(_player_id: i32) -> bool {
    // 规则书: 「[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付
    // [拥有者]星之鼓动山丘上房子数量×100的资金」
    // `card.placed && pay_is_rent && actor != owner && tile.id >= 0 &&
    // tile.owner == owner` is the pre. Still travelling / player_out /
    // tax_due are residuals.
    if !in_play_area() && ctx::prop(PROP_STEP) < ROUTE.len() as i32 {
        return false;
    }
    if ctx::player_out(trigger::player_id()) {
        return false;
    }
    tax_due() > 0
}

/// Is the card in the owner's play area (the route finished)?
/// `place_raw` / a direct placement sets no route cursor, so the card's own
/// tile is the authority: `None` / `-1` = the play area.
fn in_play_area() -> bool {
    ctx::self_tile().unwrap_or(-1) < 0
}

/// 规则书（3）'s figure: 「星之鼓动山丘上房子数量×100」.
fn tax_due() -> i32 {
    let hill = ctx::tile_named("星之鼓动山丘");
    let houses = if hill >= 0 { ctx::houses_of(hill) } else { 0 };
    houses * 100
}

fn pay_add(player_id: i32, mut v: PayAddValue) -> PayAddValue {
    // 规则书: 「额外支付[拥有者]星之鼓动山丘上房子数量×100的资金」 -- add the figure
    // into the payment; the shell of the clause is in the registration above.
    let due = tax_due();
    let payer = trigger::player_id();
    ctx::log(
        player_id,
        &Msg::new(key!("tomorrows_door_tax"))
            .player_id("who", player_id)
            .player_id("target", payer)
            .n("money", due as i64),
    );
    PayAddValue {
        value: v.value + due,
        ..v
    }
}

/// `Fx.SettleBody` -- the non-rent side of (3): a settle on 梦开始的地方 (or on
/// a mortgaged owner tile, where no rent payment exists to carry the add) still
/// owes `houses(星之鼓动山丘) × 100`. 行动阶段 15 (`SETTLE-STAGES.md` §4 M2).
/// The guard carries the applicability (route finished, someone else settling
/// on 梦开始的地方 or a mortgaged owner tile, a surcharge due): the body is
/// the transfer.
fn settle_body_guard(player_id: i32) -> bool {
    // `card.placed && actor != owner && tile.id >= 0` is the pre;
    // `cancelled()` is resolution-time and stays.
    // TODO(规则书)（[结算]时额外支付）: 书 pays on any settle over such a tile -- we
    //   drop the whole surcharge when a body replacer (Parking Space) ran first,
    //   although the settle still resolves. Needs a settle-scoped 「body replaced」
    //   fact readable after the body stage.
    if trigger::cancelled() {
        return false;
    }
    if !in_play_area() && ctx::prop(PROP_STEP) < ROUTE.len() as i32 {
        return false;
    }
    if ctx::player_out(trigger::player_id()) {
        return false;
    }
    // 规则书（3）: 「[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时
    //   额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。」
    let at = trigger::tile();
    let dream = ctx::tile_named("梦开始的地方");
    let is_dream = dream >= 0 && at == dream;
    let is_owner_tile = ctx::tile_owner(at) == player_id;
    if !is_dream && !is_owner_tile {
        return false;
    }
    // Rent on an unmortgaged owner tile already carried the surcharge via
    // `pay_add`; don't charge it twice.
    // TODO(规则书)（[拥有者]拥有的格子或梦开始的地方[结算]时）: whether the rent
    //   actually ran is only guessed from the mortgage bit, so a body-replaced
    //   settle on a mortgaged tile charges or not by card-placement order.
    if is_owner_tile && !is_dream && !ctx::mortgaged_of(at) {
        return false;
    }
    tax_due() > 0
}

fn settle_body(player_id: i32) -> card_sdk::Asked {
    let due = tax_due();
    let payer = trigger::player_id();
    ctx::transfer(
        payer,
        player_id,
        due,
        &Msg::new(key!("tomorrows_door_tax"))
            .player_id("who", player_id)
            .player_id("target", payer)
            .n("money", due as i64),
    )?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_add_kani(player_id: i32, v: card_sdk::PayAddValue) -> card_sdk::PayAddValue {
    pay_add(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_body_kani(player_id: i32) -> card_sdk::Asked {
    settle_body(player_id)
}
