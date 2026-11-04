//!
//! 规则书（docs/rulebook/cards.json, id `RAS:游击演出`）:
//! > 游击演出：\[特]：
//! > 此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家支付了至少1000资金且你不拥有任何可盖房的live house格子且场上已不存在可购买的此类格子，则可选择将此卡置于场上，指定你拥有的一个最贵的地契，使其对你视为live house格子。
//! > [手]：
//! > [反击]当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后：传送至任意无主可购买的格子并[结算]，且必须购买。
//!
//! an unowned deed and must buy it; a [特] that revives it as a fake Live House.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, ChainKind, TriggerKind};
use card_sdk::{key, Action, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const GUERRILLA: CardDef = CardDef::new(
    "RAS:游击演出",
    &[
        On::Counteract(&[ChainKind::Paid], "", Some(can_counteract), counteract).counter_reason(key!("guerrilla_when")),
        // The counteract body also arms (the card lands in the discard after that
        // counteraction resolves).
        On::Action(Action::Discarded(card_sdk::condition::MINE, None, on_discarded)).silent(),
        // 规则书[特]: 「此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家
        // 支付了至少1000资金且你不拥有任何可盖房的live house格子且场上已不存在
        // 可购买的此类格子，则可选择…」 -- the offer exists only when every
        // clause of that 「如果」 holds. Derived scans, residual. The arm-latch
        // clear is the same entry's bookkeeping (the engine dispatches one
        // `AtEnd` body per entry), so the gate lives in the body.
        On::Schedule("at_end", "", None, at_end),
        // 规则书[特]: 「使其对你视为live house格子」 lasts while this card is on
        // the field -- the re-colour goes when the card leaves.
        On::Action(Action::ZoneLeft("card.placed", None, on_leave)),
    ],
)
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"游击演出"#)
        .raw_name(r#"游击演出"#)
        .tags(&[r#"手"#, r#"反击"#, r#"特"#])
        .text(CardText::of(&[
            sec(r#"[特]："#, &[
                para(r#"此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家支付了至少1000资金且你不拥有任何可盖房的live house格子且场上已不存在可购买的此类格子，则可选择将此卡置于场上，指定你拥有的一个最贵的地契，使其对你视为live house格子。"#),
            ]),
            sec(r#"[手]："#, &[
                para(r#"[反击]当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后：传送至任意无主可购买的格子并[结算]，且必须购买。"#),
            ]),
        ]))
        // sheet cell H9
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

const ID: &str = "RAS:游击演出";

const SLOT_ARMED: &str = "guerrilla_armed";
/// The deed this instance re-coloured, `tile + 1` (0 = none).
const SLOT_TILE: &str = "guerrilla_tile";

fn free_tiles() -> Vec<i32> {
    (0..ctx::tile_count())
        .filter(|&t| ctx::tile_owner(t) < 0 && ctx::is_buyable(t))
        .collect()
}

/// This half answers 「场上已不存在可购买的此类格子」 over the unowned tiles; the owned half is
/// [`owned_buildable_lh`].
const LIVEHOUSE_PROPS: [&str; 4] = [
    "DUB MUSIC EXPERIMENT",
    "武道馆",
    "Space",
    "Live House Galaxy",
];

fn lh_buildable(t: i32) -> bool {
    LIVEHOUSE_PROPS.iter().any(|&n| ctx::tile_named(n) == t) && ctx::is_buyable(t)
}

/// 规则书[特]: 「你不拥有任何可盖房的live house格子」 -- a Live House *for this
/// player* (the per-player colour sees a `colorFor:<p>` designation, this
/// card's own included) that the build gate would allow right now
/// (`can_build_on`: not mortgaged, upgrade available, no `NO_BUILD`).
fn owned_buildable_lh(player_id: i32, t: i32) -> bool {
    ctx::is_live_house_for(player_id, t) && ctx::can_build_on(player_id, t)
}

/// 规则书[手][反击]: 「当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后」
fn can_counteract(player_id: i32) -> bool {
    if trigger::kind() != TriggerKind::Paid || trigger::player_id() != player_id {
        return false;
    }
    if trigger::target() == player_id {
        return false;
    }
    if !trigger::pay_is_rent() {
        return false;
    }
    // 规则书[手][反击]: 「且当次[结算]时被其他玩家的格子收取资金后」 --
    // a tile's rent (`pay_rent`), not a card's 「收费」 (`charge_rent` stamps
    // `by_card`, docs/CARDS.md).
    if trigger::by_card().is_some() {
        return false;
    }
    // TODO(规则书)（当你经过属于其他玩家的livehouse格子后）: TODO(规则书)（传送至任意无主可购买的格子并[结算]，且必须购买）: an
    // unprinted availability precondition -- with no free tile the window stays shut
    // and the card never enters the discard, so its [特] revive is lost too.
    !free_tiles().is_empty()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[手][反击]: 「传送至任意无主可购买的格子并[结算]，且必须购买」
    let free = free_tiles();
    if free.is_empty() {
        return Ok(());
    }
    // 规则书[手][反击]: 「传送至任意无主可购买的格子」 -- the whole free pool is offered (the book names
    // no affordability gate); a tile the player cannot cover is the 「必须购买」 shortfall
    // below.
    let to = ctx::ask_tile(
        player_id,
        &Msg::new(key!("guerrilla_title")),
        &Msg::new(key!("guerrilla_ask")),
        &free,
    )?;
    ctx::teleport_to(player_id, to);
    // The settle still opens its own optional buy like any other landing; 「必须购买」
    // below is the binding one and runs after it.
    ctx::settle_tile_at(player_id, to, true);
    // Accepting the settle's own optional offer already bought the deed --
    // `card_buy` would answer "already owned" false, so the owner check keeps that
    // from logging 买不下.
    if ctx::tile_owner(to) != player_id
        && (ctx::money_of(player_id) < ctx::buy_price(to) || !ctx::card_buy(player_id, to))
    {
        ctx::log(
            player_id,
            &Msg::new(key!("guerrilla_broke"))
                .player_id("who", player_id)
                .tile("tile", to),
        );
    }
    ctx::log(
        player_id,
        &Msg::new(key!("guerrilla_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    // `before_turn_end` targets the current turn's end, so no owner-turn test is
    // needed.
    if ctx::slot(player_id, SLOT_ARMED) == 0 {
        ctx::set_slot(player_id, SLOT_ARMED, 1);
        ctx::before_turn_end(player_id, "at_end");
    }
    Ok(())
}

/// 规则书[特]「此卡进入弃卡区的回合结束时」 -- entering the discard arms the
/// check for the end of that same turn, whichever seat's turn it is.
fn on_discarded(player_id: i32) -> card_sdk::Asked {
    if ctx::slot(player_id, SLOT_ARMED) == 0 {
        ctx::set_slot(player_id, SLOT_ARMED, 1);
        ctx::before_turn_end(player_id, "at_end");
    }
    Ok(())
}

/// The turn-end body: clear the arm-latch (always), then offer if the 「如果」
/// holds. One `AtEnd` entry per card is what the engine dispatches, so the
/// bookkeeping and the offer share it.
fn at_end(player_id: i32) -> card_sdk::Asked {
    ctx::set_slot(player_id, SLOT_ARMED, 0);
    if revive_applies(player_id) {
        check(player_id)?;
    }
    Ok(())
}

/// Residual guard for [`check`]: every 「如果」 clause of the revive offer.
///
/// 规则书[特]:
/// > 此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家支付了至少1000资金
/// > 且你不拥有任何可盖房的live house格子且场上已不存在可购买的此类格子，则可选择
/// > 将此卡置于场上…
///
/// 「你不拥有任何可盖房的live house格子」 / 「场上已不存在可购买的此类格子」 -- derived tile scans. None of
/// these are CEL (`paid_in_settle` / `lh_buildable` / discard contents).
fn revive_applies(player_id: i32) -> bool {
    if ctx::paid_in_settle_by(player_id) < 1000 {
        return false;
    }
    if ctx::owned_tiles(player_id)
        .into_iter()
        .any(|t| owned_buildable_lh(player_id, t))
    {
        return false;
    }
    if (0..ctx::tile_count()).any(|t| lh_buildable(t) && ctx::tile_owner(t) < 0) {
        return false;
    }
    ctx::discard_count(player_id, ID) > 0
}

/// Applicability is [`revive_applies`] (called by [`at_end`]).
fn check(player_id: i32) -> card_sdk::Asked {
    let mine = ctx::owned_tiles(player_id);
    // 「则可选择将此卡置于场上，指定你拥有的一个最贵的地契，使其对你视为live house格子」
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("guerrilla_title")),
        &Msg::new(key!("guerrilla_revive")),
    )? {
        return Ok(());
    }
    if !ctx::take_card(player_id, ID, ctx::CardPile::Graveyard) {
        return Ok(());
    }
    let best = mine
        .into_iter()
        .max_by_key(|&t| ctx::tile_price(t))
        .unwrap_or(-1);
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::place_card(player_id, ID, &Msg::new(key!("guerrilla_note")));
    // 规则书[特]: 「指定你拥有的一个最贵的地契」 -- the [指定] is a designation
    // mark on that deed (owned by this field instance, so it dies with the card);
    // the functional effect is the `colorFor:<seat>` tile prop below.
    if best >= 0 {
        ctx::designate_tile(
            best,
            player_id,
            &Msg::new(key!("guerrilla_revived"))
                .player_id("who", player_id)
                .tile("tile", best),
        );
    }
    ctx::log(
        player_id,
        &Msg::new(key!("guerrilla_revived"))
            .player_id("who", player_id)
            .tile("tile", best),
    );
    // 规则书[特]: 「使其对你视为live house格子」 -- the `colorFor:<player_id>`
    // tile prop on the deed's board instance: group 6 for this player only. The
    // tile is remembered so the leave can clear it.
    if best >= 0 {
        ctx::set_tile_prop(
            best,
            &alloc::format!("{}{}", card_sdk::abi::prop::COLOR_FOR_PREFIX, player_id),
            6,
        );
        ctx::set_prop(SLOT_TILE, best + 1);
    }
    Ok(())
}

/// 规则书[特]: the re-colour lasts while the card is on the field; the card is
/// filed to the discard (so its [特] can arm again).
fn on_leave(player_id: i32) -> card_sdk::Asked {
    // Instance kind: another copy sees this raise too.
    if trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    let t = ctx::prop(SLOT_TILE) - 1;
    if t >= 0 {
        ctx::set_tile_prop(
            t,
            &alloc::format!("{}{}", card_sdk::abi::prop::COLOR_FOR_PREFIX, player_id),
            0,
        );
        ctx::set_prop(SLOT_TILE, 0);
    }
    bandori_std::zones::set_leave_destination(ctx::Dest::Graveyard);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_discarded_kani(player_id: i32) -> card_sdk::Asked {
    on_discarded(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_end_kani(player_id: i32) -> card_sdk::Asked {
    at_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_leave_kani(player_id: i32) -> card_sdk::Asked {
    on_leave(player_id)
}
