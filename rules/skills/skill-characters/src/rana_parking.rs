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
//!
//! R4 (`SETTLE-STAGES.md` §9, user ruling 2026-10-07): （2）'s two branches are
//! two different gestures and live on two different stages. 「免除付款」 is a
//! **payment-stage cancel** (`payTotalCancel`, 规则书 支付阶段 5). 「将该次结算
//! 改为在space格子上添加一个"抹茶芭菲"」 is a **settle replacement** -- the
//! `settleBody` body-replace gesture (「将该次结算改为」, the same shape as Hey
//! Kids / Parking Space), not a pre-settle write. Both used to be done at
//! `settleBefore`.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state, trigger};
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「抹茶芭菲」 parked on space.
const ON_TILE: &str = "抹茶芭菲";
/// A player's held parfaits -- a counter.
const HELD: &str = "抹茶芭菲";

pub const RANA_PARKING: CardDef = CardDef::new(
    "skill:要乐奈:投币式停车场的猫",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::Pass], card_sdk::pre::MINE, None, on_pass),
        // （2）「免除付款」 -- a payment-stage cancel (R4). The 「并抽1张卡」
        // rides the settle's effect list below.
        On::Hook(&[HookKind::PayTotalCancel], "", Some(any), exempt_pay),
        // （2）「将该次结算改为在space格子上添加一个"抹茶芭菲"」 -- the
        // settle-body replace gesture (R4), and the 「抽1张卡」 half of the
        // other branch.
        On::Hook(&[HookKind::SettleBody], "", Some(any), replace_body),
        On::Hook(&[HookKind::PassPlayer], card_sdk::pre::MINE, None, on_overlap),
    ],
)
    .legacy(&[(2, legacy_mine), (5, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn any(_player_id: i32) -> bool {
    true
}

/// 「初始3，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 3, 3);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」, and （3）「拥有"抹茶芭菲"的玩家经过
/// 任意"RiNG"时可使用一个"抹茶芭菲"获得400资金」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    if ctx::is_circle(t) {
        ctx::gain_fire(player_id, 1, &Msg::new(key!("rana_parking_gain")))?;
        return Ok(());
    }
    if ctx::is_ring(t) && ctx::tok(player_id, HELD) >= 1 {
        if ctx::ask_yes(
            player_id,
            &Msg::new(key!("rana_parking_title")),
            &Msg::new(key!("rana_parking_ring")),
        )? {
            ctx::add_tok(player_id, HELD, -1, i32::MAX)?;
            ctx::gain(player_id, 400, &Msg::new(key!("rana_parking_ring_gain")))?;
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
    if !ctx::spend_fire(player_id, 3, &Msg::new(key!("rana_parking_spend")))? {
        return Ok(());
    }
    plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    plan::set_teleport_to(space);
    plan::set_resolve(true);
    // 「该次传送不可进行地契购买」 -- the plan's no-buy flag.
    plan::set_no_buy(true);
    // 「传送至space代替本回合的移动」 -- the press *is* the main move. Setting
    // the plan alone leaves the piece put; `card_move` executes it (and marks
    // `MainMoved`, so no further main move this turn).
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("rana_parking_moved")).tile("tile", space),
    );
    Ok(())
}

/// （2）「若space已属于其他玩家，免除付款」 -- R4: a **payment-stage cancel**
/// (规则书 支付阶段 5 「取消支付」), not a pre-settle write. Cancels the Space
/// payment the mover would have owed; the 「并抽1张卡」 half rides the settle
/// body entry below.
fn exempt_pay(player_id: i32) -> card_sdk::Asked {
    if trigger::cancelled() {
        return Ok(());
    }
    let space = ctx::tile_named("Space");
    if space < 0 || trigger::tile() != space {
        return Ok(());
    }
    let mover = trigger::player_id();
    if mover != player_id {
        return Ok(());
    }
    let owner = ctx::tile_owner(space);
    // 「若space已属于其他玩家」 -- the owner-branch is the settle replace, not
    // this.
    if owner < 0 || owner == mover {
        return Ok(());
    }
    // 「免除付款」 -- drop the payment command.
    trigger::set_cancelled();
    ctx::log(player_id, &Msg::new(key!("rana_parking_free")));
    Ok(())
}

/// （2）'s two settle-stage halves (`SETTLE-STAGES.md` §9 R4).
///
/// * 「若space已属于其他玩家，…并抽1张卡」 -- an entry in the settle's effect
///   list: the settle happens, the payment was already exempted above, and the
///   mover draws.
/// * 「若自己为space的拥有者，则可选择将该次结算改为在space格子上添加一个
///   "抹茶芭菲"」 -- the **body-replace** gesture (`trigger::set_cancelled()`,
///   the same shape as Parking Space / Hey Kids): the settle becomes "add a
///   parfait", so the tile's own effect list (including any payment) never
///   runs.
fn replace_body(player_id: i32) -> card_sdk::Asked {
    if trigger::cancelled() {
        return Ok(());
    }
    let space = ctx::tile_named("Space");
    if space < 0 || trigger::tile() != space {
        return Ok(());
    }
    let owner = ctx::tile_owner(space);
    let mover = trigger::player_id();
    if owner >= 0 && owner != mover {
        // 「免除付款并抽1张卡」 -- the draw half. (The payment cancel already
        // ran at `payTotalCancel`.)
        if mover == player_id {
            ctx::draw(player_id, 1)?;
            ctx::log(player_id, &Msg::new(key!("rana_parking_free")));
        }
        return Ok(());
    }
    if owner != player_id {
        return Ok(());
    }
    // 「若自己为space的拥有者，则可选择将该次结算改为在space格子上添加一个
    // "抹茶芭菲"」 -- the body-replace gesture (R4).
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("rana_parking_title")),
        &Msg::new(key!("rana_parking_parfait")),
    )? {
        return Ok(());
    }
    // 「将该次结算改为…」 -- replace the body: the tile's own effect list is
    // skipped and the parfait is what the settle does instead.
    trigger::set_cancelled();
    ctx::add_mark(space, player_id, ON_TILE, &Msg::new(key!("rana_parking_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("rana_parking_placed")).tile("tile", space),
    );
    Ok(())
}

/// （3）「你与其他玩家重合时，获得其800资金并使那个玩家获得一个"抹茶芭菲"」.
fn on_overlap(player_id: i32) -> card_sdk::Asked {
    let other = ctx::trigger::player_id();
    if other == player_id {
        return Ok(());
    }
    ctx::transfer(
        other,
        player_id,
        800,
        &Msg::new(key!("rana_parking_overlap")),
    )?;
    ctx::add_tok(other, HELD, 1, i32::MAX)?;
    ctx::log(
        player_id,
        &Msg::new(key!("rana_parking_gave")).player_id("who", other),
    );
    Ok(())
}
