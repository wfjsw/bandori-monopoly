//!
//! 规则书（docs/rulebook/cards.json, id `R:学生会的检查`）:
//! > 学生会的检查：
//! >  移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出，向抽牌堆中加入一张“觉悟”，本回合无法加盖房屋，但可支付那格一层房屋的建造价格一半将此卡放于那个格子上，使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算。因此卡强制停下的玩家的结算地租价格为原价格一半。若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金。
//!
//! after the move, before settle: stuff a 「觉悟」 into the deck, lock the turn's
//! builds, and optionally pay half a house's build cost to leave this card on a
//! nearby deed -- the next passer-by stops there and settles at half rent.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, ChainKind, MoveKind, SettleOutcome, TriggerKind};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Msg, On};

pub const COUNCIL_CHECK: CardDef = CardDef::new(
    "R:学生会的检查",
    &[
        On::Counteract(
            &[ChainKind::SettleBefore],
            // 规则书[反击]: 「移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出」
            "actor == owner && turn_player == owner && owned_within(owner, 3) > 0",
            None,
            counteract),
        On::Action(Action::PassTile("card.placed && actor != owner && move.remaining > 0 && move.kind != Teleport", None, pass_tile)),
        // 规则书[反击]: 「结算获得的资金」 -- the forced settle's rent is tracked so
        // the refund threshold below can compare against it.
        On::Action(Action::PayAfter("card.placed && pay_is_rent && target == owner", None, pay_after)).silent(),
        // 规则书[反击]: 「…或未结算」 -- `TileResolved` also fires for a settle
        // that was cancelled (or whose payer went out), and the payload carries
        // the outcome, so the refund no longer hangs on a rent that never ran.
        On::Action(Action::TileResolved("card.placed", None, settle_after)),
        // 规则书[反击]: 「若此卡进入弃牌堆时」 -- the refund happens on the leave,
        // so a copy removed by another effect while its stop is pending
        // (未结算) still hands the placement cost back. `zoneLeft` runs on the
        // still-live row, so the instance props are readable here; the body
        // keeps it to the Graveyard (`Discarded` alone would also fire on a
        // leave the engine retargets). `trigger_card == card.id` (not `MINE`):
        // the raise `actor` is whoever's effect discarded the card, which need
        // not be the owner.
        // Not silent: the refund posts a visible gain line.
        On::Action(Action::ZoneLeft(
            "card.placed && trigger_card == card.id",
            None,
            on_zone_left,
        )),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"学生会的检查"#)
        .raw_name(r#"学生会的检查"#)
        .tags(&[])
        .text(CardText::para(r#"移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出，向抽牌堆中加入一张“觉悟”，本回合无法加盖房屋，但可支付那格一层房屋的建造价格一半将此卡放于那个格子上，使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算。因此卡强制停下的玩家的结算地租价格为原价格一半。若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金。"#))
        // sheet cell E3
        .fields(&[("group", r#"Roselia"#), ("owner", "")]));

fn near(player_id: i32) -> Vec<i32> {
    let pos = ctx::player_pos(player_id);
    let owned = ctx::owned_tiles(player_id);
    // Explicit loop + exact capacity instead of `filter(..).collect()`: the
    // `collect` specialization reuses the source allocation in place and its
    // dealloc layout trips the CBMC allocator checks under Kani. Iterate by
    // reference: consuming a cap-0 `Vec` into `IntoIter` makes its drop
    // deallocate the dangling pointer with a bogus size under Kani 0.68
    // (allocator-check false positive), while a direct `Vec` drop is fine.
    // Concrete cap: `owned.len()` crosses a call, and CBMC then cannot relate
    // the allocation size to the drop layout (Kani 0.68 allocator-check
    // false positive); `owned_tiles` is capped at 8.
    let mut out = Vec::with_capacity(8);
    for &t in &owned {
        // TODO(规则书)（前后三格内）: 书中没说算不算脚下这一格 -- `ctx::dist` 把所在
        //   格算 0，即落在自己的地契上也可以放（与「所在格子前后2格内」的既有读法一致）.
        if ctx::dist(pos, t) <= 3 {
            out.push(t);
        }
    }
    out
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    ctx::add_to_dest(player_id, "R:[衍生] 觉悟", ctx::Dest::DeckRandom, false);
    ctx::log(
        player_id,
        &Msg::new(key!("council_check_added"))
            .player_id("who", player_id)
            .card("card", "R:[衍生] 觉悟"),
    );
    // 规则书[反击]: 「本回合无法加盖房屋」 -- a turn-scoped instance carrying `prop::NO_BUILD`
    // (`docs/PURCHASE.md`), the hand-card home that replaces the old per-player
    // `noBuild` scratch key. 「本回合」 is the point: it expires at the turn end rather
    // than lingering.
    ctx::set_prop(card_sdk::abi::prop::NO_BUILD, 1);
    ctx::linger(player_id, 0);
    let near_tiles = near(player_id);
    let mut spots: Vec<i32> = Vec::with_capacity(8);
    // Same non-specialized, by-reference loop reason as `near`.
    for &t in &near_tiles {
        if ctx::build_cost(t) > 0 {
            spots.push(t);
        }
    }
    if spots.is_empty() {
        return Ok(());
    }
    let title = Msg::new(key!("council_check_ask_title"));
    let text = Msg::new(key!("council_check_ask_text")).card("card", "R:学生会的检查");
    if !ctx::ask_yes(player_id, &title, &text)? {
        return Ok(());
    }
    let tile = ctx::ask_tile(player_id, &title, &text, &spots)?;
    let amount = ctx::build_cost(tile) / 2;
    let receipt = ctx::PayCommand::new(
        player_id,
        -1,
        amount,
        &Msg::new(key!("council_check_reason")).tile("tile", tile),
    )
    .send()?;
    if !receipt.settled() {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::place_card_on(
        player_id,
        tile,
        "R:学生会的检查",
        &Msg::new(key!("council_check_note")).tile("tile", tile),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("council_check_placed"))
            .player_id("who", player_id)
            .tile("tile", tile),
    );
    // The refund stores the money actually spent, not the pre-scale figure.
    // `set_prop` writes the **instance** (the placement above just bound it as
    // the running one), so two placed copies no longer share one per-player
    // record and need no `trigger::tile()` disambiguation. Only the value that
    // now takes one is written -- the others read their default `0`.
    ctx::set_prop(PROP_PAID, receipt.amount());
    // 规则书[反击]: 「使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算」 /
    // 「因此卡强制停下的玩家的结算地租价格为原价格一半」 / 「若此卡进入弃牌堆时…」 --
    // see `pass_tile` / `pay_after` / `settle_after`.
    Ok(())
}

/// Per-**instance** memory (instance props, not player slots): the placement
/// cost, the rent collected so far, the stop latch and the halved rent the
/// stop should have paid. Each copy carries its own; a write happens when the
/// value takes one, and the instance dies with its row.
const PROP_PAID: &str = "council.paid";
const PROP_GOT: &str = "council.got";
const PROP_STOP: &str = "council.stop";
const PROP_EXPECT: &str = "council.expect";

/// 规则书[反击]: 「本卡原应拿到的资金」 -- the halved rent the forced settle should
/// have paid.
fn halved_rent(tile: i32) -> i32 {
    if tile < 0 {
        return 0;
    }
    // TODO(规则书)（本卡原应拿到的资金）:
    let r = ctx::rent_of(tile) as i64;
    (((r + 1) / 2 + 9) / 10 * 10).clamp(0, i32::MAX as i64) as i32
}

fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `card.placed && actor != owner && move.remaining > 0 &&
    // move.kind != Teleport` is the condition; `self_tile` is a derived lookup.
    let tile = ctx::self_tile().unwrap_or(-1);
    if tile < 0 || trigger::tile() != tile {
        return Ok(());
    }
    if !ctx::gate(trigger::player_id(), bandori_std::kind::abnormal::Stop) {
        return Ok(());
    }
    ctx::plan::set_stop_at(tile);
    ctx::plan::set_settles(true);
    ctx::plan::set_rent_factor(500);
    ctx::set_prop(PROP_STOP, 1);
    ctx::set_prop(PROP_EXPECT, halved_rent(tile));
    ctx::log(
        player_id,
        &Msg::new(key!("council_check_stop"))
            .player_id("who", trigger::player_id())
            .tile("tile", tile),
    );
    Ok(())
}

fn pay_after(_player_id: i32) -> card_sdk::Asked {
    // `card.placed && pay_is_rent && target == owner` is the condition.
    let tile = ctx::self_tile().unwrap_or(-1);
    if tile < 0 || trigger::tile() != tile {
        return Ok(());
    }
    if ctx::prop(PROP_STOP) == 0 {
        return Ok(());
    }
    let got = ctx::prop(PROP_GOT).saturating_add(trigger::value());
    ctx::set_prop(PROP_GOT, got);
    Ok(())
}

/// Once the forced settle is over, unplace to discard and refund the placement
/// cost -- unconditionally when the settle never happened, and when it did but
/// the halved rent fell short of what the tile should have paid.
fn settle_after(player_id: i32) -> card_sdk::Asked {
    // `card.placed` is the condition.
    if ctx::prop(PROP_STOP) == 0 {
        return Ok(());
    }
    // The props are the instance's own; `trigger::tile() == tile` still pins
    // the settle to the tile this copy stands on.
    let tile = ctx::self_tile().unwrap_or(-1);
    if tile < 0 || trigger::tile() != tile {
        return Ok(());
    }
    let paid = ctx::prop(PROP_PAID);
    let got = ctx::prop(PROP_GOT);
    let expect = ctx::prop(PROP_EXPECT);
    ctx::set_prop(PROP_STOP, 0);
    // The copy is leaving anyway; zeroing the cost here is what keeps the
    // `zoneLeft` backstop from refunding a second time.
    ctx::set_prop(PROP_PAID, 0);
    ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
    // 规则书[反击]: 「若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金」
    // -- 「或未结算」 makes the refund independent of `got`; only a settle that
    // ran is compared against the halved rent.
    let settled = SettleOutcome::from_i32(trigger::settle_outcome()) == SettleOutcome::Settled;
    if (!settled || got < expect) && paid > 0 {
        ctx::gain(
            player_id,
            paid,
            &Msg::new(key!("council_check_refund"))
                .player_id("who", player_id)
                .n("got", got as i64)
                .n("expect", expect as i64)
                .n("paid", paid as i64),
        )?;
    } else {
        ctx::log(
            player_id,
            &Msg::new(key!("council_check_done"))
                .player_id("who", player_id)
                .tile("tile", tile),
        );
    }
    // Settled: the leave hook has nothing left to refund.
    Ok(())
}

/// 规则书[反击]: 「若此卡进入弃牌堆时…或未结算则获得发动此卡消耗的资金」 -- a copy
/// that leaves the field for the discard pile before its forced settle
/// collected anything hands the placement cost back. A completed settle already
/// cleared `PROP_PAID`; any other destination (a bounce to the hand) keeps it.
fn on_zone_left(player_id: i32) -> card_sdk::Asked {
    // `card.placed && trigger_card == card.id` is the condition. Every copy of
    // the card answers a `Discarded` raise (the raise names the card, not the
    // row); the zone raise names the row, so only the leaving copy runs. A
    // settle the card itself just ran cleared the cost already.
    if trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    let to_discard = trigger::zone().is_some_and(|z| z.to == card_sdk::abi::Zone::Graveyard);
    let paid = ctx::prop(PROP_PAID);
    if !to_discard || paid <= 0 {
        return Ok(());
    }
    let got = ctx::prop(PROP_GOT);
    let expect = ctx::prop(PROP_EXPECT);
    ctx::set_prop(PROP_PAID, 0);
    ctx::gain(
        player_id,
        paid,
        &Msg::new(key!("council_check_refund"))
            .player_id("who", player_id)
            .n("got", got as i64)
            .n("expect", expect as i64)
            .n("paid", paid as i64),
    )?;
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
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_after_kani(player_id: i32) -> card_sdk::Asked {
    pay_after(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_after_kani(player_id: i32) -> card_sdk::Asked {
    settle_after(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_zone_left_kani(player_id: i32) -> card_sdk::Asked {
    on_zone_left(player_id)
}
