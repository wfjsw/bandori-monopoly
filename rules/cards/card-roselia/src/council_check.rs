//! `R:学生会的检查` -- C# `CardCouncilCheck` (MatchHost.cs:10198-10342): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:学生会的检查`）:
//! > 学生会的检查：
//! >  移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出，向抽牌堆中加入一张“压”，本回合无法加盖房屋，但可支付那格一层房屋的建造价格一半将此卡放于那个格子上，使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算。因此卡强制停下的玩家的结算地租价格为原价格一半。若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金。
//!
//! after the move, before settle: stuff a 「压」 into the deck, lock the turn's
//! builds, and optionally pay half a house's build cost to leave this card on a
//! nearby deed -- the next passer-by stops there and settles at half rent.

use alloc::vec::Vec;

use card_sdk::abi::{TriggerKind, ChainKind, HookKind, MoveKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const COUNCIL_CHECK: CardDef = CardDef::new("R:学生会的检查", &[
    On::React(&[ChainKind::SettleBefore], can_react, react),
    On::Hook(&[HookKind::PassTile], pass_tile),
    On::Hook(&[HookKind::PayAfter], pay_after),
    On::Hook(&[HookKind::SettleAfter], settle_after),
]);

/// C# `CardCouncilCheck.Near` -- own deeds within 3 tiles either way (`H.Dist`).
fn near(player_id: i32) -> Vec<i32> {
    let pos = ctx::player_pos(player_id);
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::dist(pos, t) <= 3)
        .collect()
}

/// 规则书[反击]: 「移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出」
fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「[触发结算]前可打出」 -- C# `t.Kind == "settleBefore" && t.Seat == seat`.
    if trigger::kind() != TriggerKind::SettleBefore || trigger::player_id() != player_id {
        return false;
    }
    // C# `CardCouncilCheck.CanReact` also wants `H.State.turn == seat`.
    if ctx::turn_player() != player_id {
        return false;
    }
    // 规则书[反击]: 「移动结束后前后三格内若存在你拥有地契的格子」
    !near(player_id).is_empty()
}

fn react(player_id: i32) {
    // 规则书[反击]: 「向抽牌堆中加入一张“压”」 -- C# `H.AddToDeck(i, "R:[衍生] 压")`.
    ctx::add_to_deck(player_id, "R:[衍生] 压", true);
    ctx::log(
        player_id,
        &Msg::new(key!("council_check_added"))
            .player_id("who", player_id)
            .card("card", "R:[衍生] 压"),
    );
    // 规则书[反击]: 「本回合无法加盖房屋」 -- C# `H._turnCtx.NoBuild = true; H.State.built = true`.
    ctx::set_slot(player_id, "noBuild", 1);
    // TODO(规则书)[反击]: 「本回合无法加盖房屋」 -- the engine must honour the `noBuild`
    // slot when offering builds (C# `H.WhyNotBuildOn` -> `_turnCtx.NoBuild`). The
    // plan-op `set_no_build` is per-walk, not turn-wide, so it does not cover this.
    // 规则书[反击]: 「但可支付那格一层房屋的建造价格一半将此卡放于那个格子上」 -- only
    // near deeds with a house build cost (C# `H._tiles[t].house > 0`).
    let spots: Vec<i32> = near(player_id)
        .into_iter()
        .filter(|&t| ctx::build_cost(t) > 0)
        .collect();
    if spots.is_empty() {
        return;
    }
    let title = Msg::new(key!("council_check_ask_title"));
    let text = Msg::new(key!("council_check_ask_text"));
    // C# `H.AskTileOf(..., allowNone: true)` -- a yes/no stands in for allowNone.
    if !ctx::ask_yes(player_id, &title, &text) {
        return;
    }
    let tile = ctx::ask_tile(player_id, &title, &text, &spots);
    // 规则书[反击]: 「支付那格一层房屋的建造价格一半」 -- C# `H._tiles[r.index].house / 2`.
    let amount = ctx::build_cost(tile) / 2;
    let paid = ctx::pay(player_id, amount, &Msg::new(key!("council_check_why")).tile("tile", tile));
    if paid < amount {
        return;
    }
    // 规则书[反击]: 「将此卡放于那个格子上」 -- C# `H.PlaceFromPlay(c, i, r.index)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "R:学生会的检查",
        &Msg::new(key!("council_check_note")).tile("tile", tile),
    );
    ctx::log(player_id, &Msg::new(key!("council_check_placed")).player_id("who", player_id).tile("tile", tile));
    // TODO(规则书)[反击]: 「将此卡放于那个格子上」 -- the placement is bound to `tile`
    // (`H.PlaceFromPlay(c, owner, tile)`), not the player's field; needs field-card
    // tile placement. The C# also stores the paid amount in the card's `Mem["paid"]`.
    // The tile and paid amount are kept in slots so the hooks below can read them.
    ctx::set_slot(player_id, SLOT_TILE, tile);
    ctx::set_slot(player_id, SLOT_PAID, amount);
    ctx::set_slot(player_id, SLOT_GOT, 0);
    ctx::set_slot(player_id, SLOT_STOP, 0);
    // 规则书[反击]: 「使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算」 /
    // 「因此卡强制停下的玩家的结算地租价格为原价格一半」 / 「若此卡进入弃牌堆时…」 --
    // see `pass_tile` / `pay_after` / `settle_after`.
}

const SLOT_TILE: &str = "council_tile";
const SLOT_PAID: &str = "council_paid";
const SLOT_GOT: &str = "council_got";
const SLOT_STOP: &str = "council_stop";

/// C# `CardCouncilCheck.PassTile` -> `Stop`: when another player passes the card's
/// tile mid-move (remaining steps > 0, not a teleport) and is not the card's
/// owner, force a stop + settle at half rent. The `H.AbnormalGate` stop guard is
/// still held; the move-shaping itself is written below.
fn pass_tile(player_id: i32) {
    if trigger::kind() != TriggerKind::PassTile || !ctx::is_placed(player_id) {
        return;
    }
    let tile = ctx::slot(player_id, SLOT_TILE);
    if tile < 0 || trigger::tile() != tile {
        return;
    }
    // C# `m.Seat == Seat` -- the card's own owner is not stopped by it.
    if trigger::player_id() == player_id {
        return;
    }
    // C# `m.Remaining <= 0` -- only a still-walking pass is intercepted.
    if trigger::move_remaining() <= 0 {
        return;
    }
    // C# `m.Teleport` -- a teleport does not walk past the tile.
    if trigger::move_kind() == Some(MoveKind::Teleport) {
        return;
    }
    // TODO(规则书)[反击]: the `H.AbnormalGate` stop guard (C# `CardCouncilCheck.Stop`:
    // `H.WithCard(Seat, H.AbnormalGate(a))`) is still held -- a blocker with a
    // 「不可阻挡」-style bypass is not in the vocabulary.
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
}

/// C# `CardCouncilCheck.PayAfter`: track the rent the forced settle actually
/// collected (`_got += p.finalGain`), keyed on the council stop flag.
fn pay_after(player_id: i32) {
    if trigger::kind() != TriggerKind::PayAfter || !ctx::is_placed(player_id) {
        return;
    }
    if !trigger::pay_is_rent() || trigger::target() != player_id {
        return;
    }
    let tile = ctx::slot(player_id, SLOT_TILE);
    if tile < 0 || trigger::tile() != tile {
        return;
    }
    if ctx::slot(player_id, SLOT_STOP) == 0 {
        return;
    }
    let got = ctx::slot(player_id, SLOT_GOT).saturating_add(trigger::value());
    ctx::set_slot(player_id, SLOT_GOT, got);
}

/// C# `CardCouncilCheck.SettleAfter` -> `Done`: once the forced stop has settled,
/// unplace to discard and refund the placement cost when the halved rent fell
/// short of what the tile should have paid.
fn settle_after(player_id: i32) {
    if trigger::kind() != TriggerKind::SettleAfter || !ctx::is_placed(player_id) {
        return;
    }
    if ctx::slot(player_id, SLOT_STOP) == 0 {
        return;
    }
    ctx::set_slot(player_id, SLOT_STOP, 0);
    let tile = ctx::slot(player_id, SLOT_TILE);
    let paid = ctx::slot(player_id, SLOT_PAID);
    let got = ctx::slot(player_id, SLOT_GOT);
    // 规则书[反击]: 「此卡置入弃牌堆」 -- C# `H.Unplace(this, "discard", ...)`.
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, "R:学生会的检查");
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
        );
    } else {
        ctx::log(
            player_id,
            &Msg::new(key!("council_check_done")).player_id("who", player_id).tile("tile", tile),
        );
    }
}
