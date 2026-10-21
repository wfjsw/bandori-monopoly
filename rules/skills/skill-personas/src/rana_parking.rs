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
//! **payment-stage cancel** (`payCombinedCancel`, 规则书 支付阶段 5). 「将该次结算
//! 改为在space格子上添加一个"抹茶芭菲"」 is a **settle replacement** -- the
//! `settleBody` body-replace gesture (「将该次结算改为」, the same shape as Hey
//! Kids / Parking Space), not a pre-settle write. Both used to be done at
//! `settleBefore`.

use bandori_std::state_key; use card_sdk::abi::{ActionKind, FilterKind};
use bandori_std::ctx::{self, plan, state, trigger};
use card_sdk::{Action, CardDef, Filter, key, Msg, On};

/// The 「抹茶芭菲」 counter on this skill instance. Tile units (parked on
/// space) and held units (a player's tokens) are the same counter bound to
/// different locations.
const PARFAIT: &str = "抹茶芭菲";

pub const RANA_PARKING: CardDef = CardDef::new(
    "skill:要乐奈:投币式停车场的猫",
    &[
        On::Play("", None, Some(can_use), use_skill),
        On::Action(Action::TurnStartBefore("", None, None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, None, declare_cap)),
        On::Action(Action::Pass(card_sdk::pre::MINE, None, None, on_pass)),
        // （2）「免除付款」 -- a payment-stage cancel (R4). The 「并抽1张卡」
        // rides the settle's effect list below.
        //
        // ABI v74 split (FilterFlag + Action): the owner-branch auto-cancel
        // (「若space已属于其他玩家」) is non-interactive and stays on
        // `FilterFlag(PayCombinedCancel)` (false = drop the command). The
        // non-owner parfait-transfer branch prompts (「可」), so its
        // `ask_yes` + `move_units` + `set_cancelled` ride `ActionKind::Effect`
        // (the payment declaration -- `t.value` the engine still folds).
        // PayAfter is too late to cancel. Effect fires per `money()` call,
        // so on a split-pay the interactive offer is per leg rather than once
        // on the command total (single-pair behaviour is identical).
        On::Filter(Filter::PayCombinedCancel("", Some(any), exempt_flag)),
        On::Action(Action::Effect("", None, Some(any), exempt_offer)),
        // （2）「将该次结算改为在space格子上添加一个"抹茶芭菲"」 -- the
        // settle-body replace gesture (R4), and the 「抽1张卡」 half of the
        // other branch.
        On::Action(Action::SettleBody("", None, Some(any), replace_body)),
        // （3）「你与其他玩家重合时」 -- 行动阶段 13 [重叠] (`passPlayer`). The
        // pre puts the owner in the actor seat (the mover); `target` is the
        // other player already standing on the landing tile (one raise per
        // such other). The body must read `target`, not `player_id`.
        On::Action(Action::PassPlayer(card_sdk::pre::MINE, None, None, on_overlap)),
    ],
)
    .labels(&[(3, "（1）"), (4, "（2）"), (5, "（2）"), (6, "（2）"), (7, "（3）")])
    .silent(&[1, 2])
    // 「（初始3，上限3）」
    .props(&[(bandori_std::fire::FIRE_INITIAL, 3), (bandori_std::fire::FIRE_CAP, 3)]);

fn any(_player_id: i32) -> bool {
    true
}

/// 「初始3，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」, and （3）「拥有"抹茶芭菲"的玩家经过
/// 任意"RiNG"时可使用一个"抹茶芭菲"获得400资金」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    if ctx::is_start(t) {
        bandori_std::fire::gain(player_id, 1, &Msg::new(key!("rana_parking_gain")))?;
        return Ok(());
    }
    if ctx::is_deed(t) && ctx::tok(player_id, PARFAIT) >= 1 {
        if ctx::ask_yes(
            player_id,
            &Msg::new(key!("rana_parking_title")),
            &Msg::new(key!("rana_parking_ring")),
        )? {
            ctx::add_tok(player_id, PARFAIT, -1, i32::MAX)?;
            ctx::gain(player_id, 400, &Msg::new(key!("rana_parking_ring_gain")))?;
        }
    }
    Ok(())
}

/// （2） 「可花费3个火罐传送至space代替本回合的移动」.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::CHARGE) < 3 {
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
    if !bandori_std::fire::spend(player_id, 3, &Msg::new(key!("rana_parking_spend")))? {
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
/// (规则书 支付阶段 5 「取消支付」). Non-interactive owner-branch half (ABI v74
/// FilterFlag): the settling player is the Space owner and the tile belongs to
/// someone else, so the payment is dropped outright. `false` = cancel (the host
/// calls `set_cancelled()`). The interactive parfait-transfer half is
/// [`exempt_offer`] on `ActionKind::Effect`.
fn exempt_flag(player_id: i32, _value: i32) -> bool {
    // true = keep
    if trigger::cancelled() {
        return true;
    }
    let space = ctx::tile_named("Space");
    if space < 0 || trigger::tile() != space {
        return true;
    }
    let mover = trigger::player_id();
    if mover != player_id {
        // The non-owner branch is interactive (「可」) -- left to [`exempt_offer`].
        return true;
    }
    let owner = ctx::tile_owner(space);
    // 「若space已属于其他玩家」 -- the owner-branch is the settle replace, not
    // this.
    if owner < 0 || owner == mover {
        return true;
    }
    // 「免除付款」 -- drop the payment command.
    ctx::log(player_id, &Msg::new(key!("rana_parking_free")));
    false
}

/// （2）'s non-owner branch: 「其他人在space触发结算时可将自己拥有的一个"抹茶芭菲"
/// 转移到该格上以免除当次付款」 -- the settling non-owner **may** (「可」) pay a
/// held parfait instead of money. ABI v74 Pattern E: `ask_yes` + `move_units` +
/// `set_cancelled` cannot live in a FilterFlag (which cannot prompt), so the
/// offer rides `ActionKind::Effect` (the payment declaration).
fn exempt_offer(player_id: i32) -> card_sdk::Asked {
    if trigger::cancelled() {
        return Ok(());
    }
    let space = ctx::tile_named("Space");
    if space < 0 || trigger::tile() != space {
        return Ok(());
    }
    let mover = trigger::player_id();
    if mover == player_id {
        // The owner-branch auto-cancel is [`exempt_flag`].
        return Ok(());
    }
    if ctx::tok(mover, PARFAIT) < 1 {
        return Ok(());
    }
    let yes = ctx::ask_yes(
        mover,
        &Msg::new(key!("rana_parking_title")),
        &Msg::new(key!("rana_parking_transfer")),
    )?;
    if !yes {
        return Ok(());
    }
    // One held unit moves onto space; the payment never runs.
    ctx::move_units(PARFAIT, -1, mover, space, -1, 1);
    trigger::set_cancelled();
    ctx::log(
        player_id,
        &Msg::new(key!("rana_parking_transfer_done")).player_id("who", mover),
    );
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
        // ran at `payCombinedCancel`.)
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
    // One unit of this instance's 「抹茶芭菲」 counter bound to space. Stacks
    // onto the tile row -- 「每个将使其收费增加500资金」 reads the summed count.
    ctx::place_mark(
        space,
        PARFAIT,
        "",
        player_id,
        ctx::self_uid(),
        1,
        &Msg::new(key!("rana_parking_note")), card_sdk::abi::Stack::Merge);
    ctx::log(
        player_id,
        &Msg::new(key!("rana_parking_placed")).tile("tile", space),
    );
    Ok(())
}

/// （3）「你与其他玩家重合时，获得其800资金并使那个玩家获得一个"抹茶芭菲"」.
/// `passPlayer`'s `player_id`/`actor` is the mover (the owner, on this `MINE`
/// pre); `target` is the other player already on the landing tile.
fn on_overlap(player_id: i32) -> card_sdk::Asked {
    let other = ctx::trigger::target();
    if other < 0 || other == player_id {
        return Ok(());
    }
    ctx::transfer(
        other,
        player_id,
        800,
        &Msg::new(key!("rana_parking_overlap")),
    )?;
    ctx::add_tok(other, PARFAIT, 1, i32::MAX)?;
    ctx::log(
        player_id,
        &Msg::new(key!("rana_parking_gave")).player_id("who", other),
    );
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn exempt_flag_kani(player_id: i32, value: i32) -> bool {
    exempt_flag(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn exempt_offer_kani(player_id: i32) -> card_sdk::Asked {
    exempt_offer(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn replace_body_kani(player_id: i32) -> card_sdk::Asked {
    replace_body(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_overlap_kani(player_id: i32) -> card_sdk::Asked {
    on_overlap(player_id)
}
