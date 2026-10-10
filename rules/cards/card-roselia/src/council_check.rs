//! `R:学生会的检查` -- C# `CardCouncilCheck` (MatchHost.cs:10198-10342): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:学生会的检查`）:
//! > 学生会的检查：
//! >  移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出，向抽牌堆中加入一张“觉悟”，本回合无法加盖房屋，但可支付那格一层房屋的建造价格一半将此卡放于那个格子上，使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算。因此卡强制停下的玩家的结算地租价格为原价格一半。若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金。
//!
//! after the move, before settle: stuff a 「压」 into the deck, lock the turn's
//! builds, and optionally pay half a house's build cost to leave this card on a
//! nearby deed -- the next passer-by stops there and settles at half rent.

use alloc::vec::Vec;

use card_sdk::abi::{ChainKind, HookKind, MoveKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const COUNCIL_CHECK: CardDef = CardDef::new(
    "R:学生会的检查",
    &[
        On::Counteract(
            &[ChainKind::SettleBefore],
            // 规则书[反击]: 「移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出」
            "actor == owner && turn_player == owner && owned_within(owner, 3) > 0",
            None,
            counteract,
        ),
        On::Hook(
            &[HookKind::PassTile],
            "card.placed && actor != owner && move.remaining > 0 && move.kind != Teleport",
            None,
            pass_tile,
        ),
        On::Hook(
            &[HookKind::PayAfter],
            "card.placed && pay_is_rent && target == owner",
            None,
            pay_after,
        ),
        On::Hook(&[HookKind::SettleAfter], "card.placed", None, settle_after),
    ],
);

/// C# `CardCouncilCheck.Near` -- own deeds within 3 tiles either way (`H.Dist`).
fn near(player_id: i32) -> Vec<i32> {
    let pos = ctx::player_pos(player_id);
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::dist(pos, t) <= 3)
        .collect()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击] (sheet 2026-10-06 新卡组卡 E3): 「向抽牌堆中加入一张“觉悟”」
    // (was 「压」) -- C# `H.AddToDeck(i, "R:[衍生] 觉悟")`.
    ctx::add_to_deck(player_id, "R:[衍生] 觉悟", true);
    ctx::log(
        player_id,
        &Msg::new(key!("council_check_added"))
            .player_id("who", player_id)
            .card("card", "R:[衍生] 觉悟"),
    );
    // 规则书[反击]: 「本回合无法加盖房屋」 -- C# `H._turnCtx.NoBuild = true; H.State.built = true`.
    // 规则书[反击]: 「本回合无法加盖房屋」 -- a turn-scoped instance carrying
    // `prop::NO_BUILD` (`docs/PURCHASE.md`), the hand-card home that replaces
    // the old per-player `noBuild` scratch key. 「本回合」 is the point: it
    // expires at the turn end rather than lingering.
    ctx::set_prop(card_sdk::abi::prop::NO_BUILD, 1);
    ctx::linger(player_id, 0);
    // 规则书[反击]: 「但可支付那格一层房屋的建造价格一半将此卡放于那个格子上」 -- only
    // near deeds with a house build cost (C# `H._tiles[t].house > 0`).
    let spots: Vec<i32> = near(player_id)
        .into_iter()
        .filter(|&t| ctx::build_cost(t) > 0)
        .collect();
    if spots.is_empty() {
        return Ok(());
    }
    let title = Msg::new(key!("council_check_ask_title"));
    let text = Msg::new(key!("council_check_ask_text")).card("card", "R:学生会的检查");
    // C# `H.AskTileOf(..., allowNone: true)` -- a yes/no stands in for allowNone.
    if !ctx::ask_yes(player_id, &title, &text)? {
        return Ok(());
    }
    let tile = ctx::ask_tile(player_id, &title, &text, &spots)?;
    // 规则书[反击]: 「支付那格一层房屋的建造价格一半」 -- C# `H._tiles[r.index].house / 2`.
    let amount = ctx::build_cost(tile) / 2;
    let paid = ctx::pay(
        player_id,
        amount,
        &Msg::new(key!("council_check_why")).tile("tile", tile),
    )?;
    if paid < amount {
        return Ok(());
    }
    // 规则书[反击]: 「将此卡放于那个格子上」 -- C# `H.PlaceFromPlay(c, i, r.index)`.
    ctx::set_dest(ctx::Dest::Field);
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
    // The C# also stores the paid amount in the card's `Mem["paid"]` -- a slot
    // stands in for that per-card map.
    ctx::set_slot(player_id, SLOT_PAID, amount);
    ctx::set_slot(player_id, SLOT_GOT, 0);
    ctx::set_slot(player_id, SLOT_STOP, 0);
    // 规则书[反击]: 「使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算」 /
    // 「因此卡强制停下的玩家的结算地租价格为原价格一半」 / 「若此卡进入弃牌堆时…」 --
    // see `pass_tile` / `pay_after` / `settle_after`.
    Ok(())
}

const SLOT_PAID: &str = "council_paid";
const SLOT_GOT: &str = "council_got";
const SLOT_STOP: &str = "council_stop";

/// C# `CardCouncilCheck.PassTile` -> `Stop`: when another player passes the card's
/// tile mid-move (remaining steps > 0, not a teleport) and is not the card's
/// owner, force a stop + settle at half rent. The `H.AbnormalGate` stop guard is
/// still held; the move-shaping itself is written below.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `card.placed && actor != owner && move.remaining > 0 &&
    // move.kind != Teleport` is the pre; `self_tile` is a derived lookup.
    let tile = ctx::self_tile().unwrap_or(-1);
    if tile < 0 || trigger::tile() != tile {
        return Ok(());
    }
    // 规则书[反击]: the stop runs behind `H.AbnormalGate` (C#
    // `H.WithCard(Seat, H.AbnormalGate(a))`); a guarded or unstoppable mover
    // simply is not stopped.
    if !ctx::gate(trigger::player_id(), card_sdk::abi::AbKind::Stop) {
        return Ok(());
    }
    // C# `m.Stopped = true; m.Resolve = true; m.RentFactor *= 0.5`.
    ctx::plan::set_stop_at(tile);
    ctx::plan::set_resolve(true);
    ctx::plan::set_rent_factor(500);
    ctx::set_slot(player_id, SLOT_STOP, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("council_check_stop"))
            .player_id("who", trigger::player_id())
            .tile("tile", tile),
    );
    Ok(())
}

/// C# `CardCouncilCheck.PayAfter`: track the rent the forced settle actually
/// collected (`_got += p.finalGain`), keyed on the council stop flag.
fn pay_after(player_id: i32) -> card_sdk::Asked {
    // `card.placed && pay_is_rent && target == owner` is the pre.
    let tile = ctx::self_tile().unwrap_or(-1);
    if tile < 0 || trigger::tile() != tile {
        return Ok(());
    }
    if ctx::slot(player_id, SLOT_STOP) == 0 {
        return Ok(());
    }
    let got = ctx::slot(player_id, SLOT_GOT).saturating_add(trigger::value());
    ctx::set_slot(player_id, SLOT_GOT, got);
    Ok(())
}

/// C# `CardCouncilCheck.SettleAfter` -> `Done`: once the forced stop has settled,
/// unplace to discard and refund the placement cost when the halved rent fell
/// short of what the tile should have paid.
fn settle_after(player_id: i32) -> card_sdk::Asked {
    // `card.placed` is the pre.
    if ctx::slot(player_id, SLOT_STOP) == 0 {
        return Ok(());
    }
    ctx::set_slot(player_id, SLOT_STOP, 0);
    let tile = ctx::self_tile().unwrap_or(-1);
    let paid = ctx::slot(player_id, SLOT_PAID);
    let got = ctx::slot(player_id, SLOT_GOT);
    // 规则书[反击]: 「此卡置入弃牌堆」 -- C# `H.Unplace(this, "discard", ...)`.
    ctx::set_dest(ctx::Dest::Graveyard);
    // C# `CeilTo((double)H.RentOf(Tile) / 2.0, 10)` -- the expected halved rent.
    let expect = if tile >= 0 {
        let r = ctx::rent_of(tile) as i64;
        (((r + 1) / 2 + 9) / 10 * 10).clamp(0, i32::MAX as i64) as i32
    } else {
        0
    };
    // 规则书[反击]: 「若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金」
    if got < expect && paid > 0 {
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
    Ok(())
}
