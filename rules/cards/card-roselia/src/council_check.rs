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

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const COUNCIL_CHECK: CardDef = CardDef {
    id: "R:学生会的检查",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// C# `CardCouncilCheck.Near` -- own deeds within 3 tiles either way (`H.Dist`).
fn near(seat: i32) -> Vec<i32> {
    let pos = ctx::seat_pos(seat);
    ctx::owned_tiles(seat)
        .into_iter()
        .filter(|&t| ctx::dist(pos, t) <= 3)
        .collect()
}

/// 规则书[反击]: 「移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出」
fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「[触发结算]前可打出」 -- C# `t.Kind == "settleBefore" && t.Seat == seat`.
    if trigger::kind() != TriggerKind::SettleBefore || trigger::seat() != seat {
        return false;
    }
    // C# `CardCouncilCheck.CanReact` also wants `H.State.turn == seat`.
    if ctx::turn_seat() != seat {
        return false;
    }
    // 规则书[反击]: 「移动结束后前后三格内若存在你拥有地契的格子」
    !near(seat).is_empty()
}

fn react(seat: i32) {
    // 规则书[反击]: 「向抽牌堆中加入一张“压”」 -- C# `H.AddToDeck(i, "R:[衍生] 压")`.
    ctx::add_to_deck(seat, "R:[衍生] 压", true);
    ctx::log(
        seat,
        &Msg::new(key!("council_check_added"))
            .seat("who", seat)
            .card("card", "R:[衍生] 压"),
    );
    // 规则书[反击]: 「本回合无法加盖房屋」 -- C# `H._turnCtx.NoBuild = true; H.State.built = true`.
    ctx::set_slot(seat, "noBuild", 1);
    // TODO(规则书)[反击]: 「本回合无法加盖房屋」 -- the engine must honour the `noBuild`
    // slot when offering builds (C# `H.WhyNotBuildOn` -> `_turnCtx.NoBuild`).
    // 规则书[反击]: 「但可支付那格一层房屋的建造价格一半将此卡放于那个格子上」 -- only
    // near deeds with a house build cost (C# `H._tiles[t].house > 0`).
    let spots: Vec<i32> = near(seat)
        .into_iter()
        .filter(|&t| ctx::build_cost(t) > 0)
        .collect();
    if spots.is_empty() {
        return;
    }
    let title = Msg::new(key!("council_check_ask_title"));
    let text = Msg::new(key!("council_check_ask_text"));
    // C# `H.AskTileOf(..., allowNone: true)` -- a yes/no stands in for allowNone.
    if !ctx::ask_yes(seat, &title, &text) {
        return;
    }
    let tile = ctx::ask_tile(seat, &title, &text, &spots);
    // 规则书[反击]: 「支付那格一层房屋的建造价格一半」 -- C# `H._tiles[r.index].house / 2`.
    let amount = ctx::build_cost(tile) / 2;
    let paid = ctx::pay(seat, amount, &Msg::new(key!("council_check_why")).tile("tile", tile));
    if paid < amount {
        return;
    }
    // 规则书[反击]: 「将此卡放于那个格子上」 -- C# `H.PlaceFromPlay(c, i, r.index)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        seat,
        "R:学生会的检查",
        &Msg::new(key!("council_check_note")).tile("tile", tile),
    );
    ctx::log(seat, &Msg::new(key!("council_check_placed")).seat("who", seat).tile("tile", tile));
    // TODO(规则书)[反击]: 「将此卡放于那个格子上」 -- the placement is bound to `tile`
    // (`H.PlaceFromPlay(c, owner, tile)`), not the seat's field; needs field-card
    // tile placement. The C# also stores the paid amount in the card's `Mem["paid"]`.
    // TODO(规则书)[反击]: 「使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算」
    // -- needs the Fx.PassTile hook (C# `CardCouncilCheck.PassTile` -> `Stop`): force
    // stop + resolve when another seat passes this tile mid-move (`m.Stopped = true;
    // m.Resolve = true`), behind the H.AbnormalGate stop guard.
    // TODO(规则书)[反击]: 「因此卡强制停下的玩家的结算地租价格为原价格一半」 -- needs the
    // same move's `m.RentFactor *= 0.5` on the forced settle (C# `CardCouncilCheck.Stop`).
    // TODO(规则书)[反击]: 「若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金」
    // -- needs Fx.SettleAfter / Fx.PayAfter (C# `CardCouncilCheck.Done`): unplace to
    // discard and refund the placement cost when the halved rent fell short.
}