//! `skill:要乐奈:投币式停车场的猫`
//!
//! 规则书（skill sheet, 要乐奈）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始3，上限3）
//! > （2）可花费3个火罐传送至space代替本回合的移动，该次传送不可进行地契购买。在
//! > space上结算时，若space已属于其他玩家，免除付款并抽1张卡；若自己为space的拥有者，
//! > 则可选择将该次结算改为在space格子上添加一个"抹茶芭菲"，每个将使其收费增加500资金，
//! > 其他人在space触发结算时可将自己拥有的一个"抹茶芭菲"转移到该格上以免除当次付款
//! > （3）你与其他玩家重合时，获得其800资金并使那个玩家获得一个"抹茶芭菲"；拥有
//! > "抹茶芭菲"的玩家经过任意"RiNG"时可使用一个"抹茶芭菲"获得400资金
//!
//! The 「抹茶芭菲」 is the tile-mark / player-counter split again: the board
//! holds them on space, a player holds them as a counter, and 「每个将使其收费
//! 增加500资金」 is the tile's rent reading the mark count.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「抹茶芭菲」 parked on space.
const ON_TILE: &str = "抹茶芭菲";
/// A player's held parfaits -- a counter.
const HELD: &str = "抹茶芭菲";

pub const RANA_PARKING: CardDef = CardDef::new("skill:要乐奈:投币式停车场的猫", &[
    On::Play(Some(can_use), use_skill),
    On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
    On::Hook(&[HookKind::Pass], mine, on_pass),
    On::Hook(&[HookKind::SettleBefore], any, before_settle),
    On::Hook(&[HookKind::PassPlayer], mine, on_overlap),
]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn any(_player_id: i32) -> bool {
    true
}

/// 「初始3，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    state::set_bounds(player_id, state_key::FIRE, 0, 3);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」, and （3）「拥有"抹茶芭菲"的玩家经过
/// 任意"RiNG"时可使用一个"抹茶芭菲"获得400资金」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    if ctx::is_circle(t) {
        ctx::gain_fire(player_id, 1, &Msg::new(key!("rana_parking_gain")));
        return Ok(());
    }
    if ctx::is_ring(t) && ctx::tok(player_id, HELD) >= 1 {
        if ctx::ask_yes(
            player_id,
            &Msg::new(key!("rana_parking_title")),
            &Msg::new(key!("rana_parking_ring")),
        )? {
            ctx::add_tok(player_id, HELD, -1, i32::MAX);
            ctx::gain(player_id, 400, &Msg::new(key!("rana_parking_ring_gain")));
        }
    }
    Ok(())
}

/// （2） 「可花费3个火罐传送至space代替本回合的移动」.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 3 {
        return Some(Msg::new(key!("rana_parking_no_fire")));
    }
    None
}

/// （2）「传送至space代替本回合的移动，该次传送不可进行地契购买」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let space = ctx::tile_named("Space");
    if space < 0 {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 3, &Msg::new(key!("rana_parking_spend"))) {
        return Ok(());
    }
    plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    plan::set_teleport_to(space);
    plan::set_resolve(true);
    // 「该次传送不可进行地契购买」 -- the plan's no-buy flag.
    plan::set_no_buy(true);
    ctx::log(player_id, &Msg::new(key!("rana_parking_moved")).tile("tile", space));
    Ok(())
}

/// （2）「在space上结算时，若space已属于其他玩家，免除付款并抽1张卡；若自己为
/// space的拥有者，则可选择将该次结算改为…添加一个"抹茶芭菲"」.
fn before_settle(player_id: i32) -> card_sdk::Asked {
    let space = ctx::tile_named("Space");
    let t = ctx::trigger::tile();
    if t != space || t < 0 {
        return Ok(());
    }
    let owner = ctx::tile_owner(t);
    let mover = ctx::trigger::player_id();
    if owner >= 0 && owner != mover {
        // 「若space已属于其他玩家，免除付款并抽1张卡」 -- for the mover.
        if mover == player_id {
            ctx::trigger::set_pay_amount(0);
            ctx::draw(player_id, 1);
            ctx::log(player_id, &Msg::new(key!("rana_parking_free")));
        }
        return Ok(());
    }
    if owner != player_id {
        return Ok(());
    }
    // 「若自己为space的拥有者，则可选择将该次结算改为在space格子上添加一个
    // "抹茶芭菲"」
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("rana_parking_title")),
        &Msg::new(key!("rana_parking_parfait")),
    )? {
        return Ok(());
    }
    ctx::trigger::set_pay_amount(0);
    ctx::add_mark(t, player_id, ON_TILE, &Msg::new(key!("rana_parking_note")));
    ctx::log(player_id, &Msg::new(key!("rana_parking_placed")).tile("tile", t));
    Ok(())
}

/// （3）「你与其他玩家重合时，获得其800资金并使那个玩家获得一个"抹茶芭菲"」.
fn on_overlap(player_id: i32) -> card_sdk::Asked {
    let other = ctx::trigger::player_id();
    if other == player_id {
        return Ok(());
    }
    ctx::transfer(other, player_id, 800, &Msg::new(key!("rana_parking_overlap")))?;
    ctx::add_tok(other, HELD, 1, i32::MAX);
    ctx::log(player_id, &Msg::new(key!("rana_parking_gave")).player_id("who", other));
    Ok(())
}
