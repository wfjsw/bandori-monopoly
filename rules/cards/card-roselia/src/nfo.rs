//! `R:NFO` -- C# `CardNfo` (MatchHost.cs:10517-10628): roll 1d6 (wrapping above 6)
//!
//! 规则书（docs/rulebook/cards.json, id `R:NFO`）:
//! > NFO：
//! >  投掷1d6并在回合结束后获得一层[停留]。若投掷结果为1，获得1500资金；若结果为2，立刻移动到你前方最近一名玩家的前方一格并使其格子上的玩家分摊支付你1000资金，视为本回合的主要移动但不[触发结算]；若结果为3，将此卡置于场上，你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆；若结果为4，选择你弃牌堆中的一张满足打出条件的卡打出；若结果为5，为自己的角色卡添加6个奇迹水晶，你每次获得资金时，可消耗一个奇迹水晶使本次的额度提高300；若结果为6，依次获得结果1-5的全部效果。（若结果严格大于6，则从1开始重新计数）
//!
//! and resolve the matching effect; 6 runs 1 through 5 in order. Whatever the
//! roll, a [停留] layer lands at turn end.

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const NFO: CardDef = CardDef {
    id: "R:NFO",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

/// The nearest other seat strictly ahead of `seat` on the ring (C# `H.NearestAhead`).
fn nearest_ahead(seat: i32) -> i32 {
    let n = ctx::tile_count();
    let pos = ctx::seat_pos(seat);
    if n <= 0 || pos < 0 {
        return -1;
    }
    let mut best = -1;
    let mut best_d = i32::MAX;
    for o in ctx::others(seat) {
        let p = ctx::seat_pos(o);
        if p < 0 || p == pos {
            continue;
        }
        let d = (p - pos).rem_euclid(n);
        if d > 0 && d < best_d {
            best_d = d;
            best = o;
        }
    }
    best
}

fn play(seat: i32) {
    // 规则书: 「投掷1d6」 -- C# `(H.Roll(i, 1, 6, "NFO") - 1) % 6 + 1` wraps above 6.
    let roll = ctx::roll(seat, 1, 6);
    // 规则书: 「（若结果严格大于6，则从1开始重新计数）」
    let k = (roll - 1).rem_euclid(6) + 1;
    // 规则书: 「并在回合结束后获得一层[停留]」 -- C# `H._turnCtx.AfterEnd.Add(() =>
    // H.GiveStay(i, 1, i, "NFO"))`.
    // TODO(规则书): 「并在回合结束后获得一层[停留]」 -- needs the turn-end AfterEnd
    // hook (C# `TurnCtx.AfterEnd`) so the layer lands after the turn, not now.
    if k == 6 {
        // 规则书: 「若结果为6，依次获得结果1-5的全部效果。」
        for x in 1..=5 {
            effect(seat, x);
        }
    } else {
        // 规则书: 「若投掷结果为1，获得1500资金；若结果为2，…；若结果为3，…；若结果为4，…；若结果为5，…」
        effect(seat, k);
    }
}

fn effect(seat: i32, k: i32) {
    if ctx::seat_out(seat) {
        return;
    }
    match k {
        // 规则书: 「若投掷结果为1，获得1500资金」
        1 => {
            ctx::gain(seat, 1500, &Msg::new(key!("nfo_why")));
        }
        // 规则书: 「若结果为2，立刻移动到你前方最近一名玩家的前方一格并使其格子上的玩家分摊支付你1000资金，视为本回合的主要移动但不[触发结算]」
        2 => {
            let num = nearest_ahead(seat);
            if num < 0 {
                ctx::log(seat, &Msg::new(key!("nfo_no_ahead")));
                return;
            }
            let pos = ctx::seat_pos(seat);
            let pos2 = ctx::seat_pos(num);
            // 规则书: 「并使其格子上的玩家分摊支付你1000资金」 -- `H.SeatsOn(pos2, i)`.
            let payers: Vec<i32> = ctx::seats_on(pos2, seat);
            // 规则书: 「立刻移动到你前方最近一名玩家的前方一格」 -- `H.Forward(pos, pos2) + 1`
            // steps lands one tile past that player.
            let to = ctx::tile_steps_ahead(seat, ctx::tile_forward(pos, pos2) + 1);
            if to >= 0 {
                // 规则书: 「不[触发结算]」 -- C# `H.CardMove(c, new MoveCtx { Steps = ...,
                // Resolve = false })`.
                ctx::teleport_to(seat, to);
                ctx::log(
                    seat,
                    &Msg::new(key!("nfo_move")).seat("who", seat).tile("tile", to),
                );
            }
            // 规则书: 「并使其格子上的玩家分摊支付你1000资金」 -- `H.SplitPay(payers, i,
            // 1000, ...)`, only if the move did not knock the seat out (C# `if (!H.Out(i))`).
            if !ctx::seat_out(seat) {
                split_pay(&payers, seat, 1000, &Msg::new(key!("nfo_pay")));
            }
            // TODO(规则书): 「视为本回合的主要移动」 -- needs the H.CardMove / main-move
            // routine (C# `H.CardMove(c, new MoveCtx { Steps = ..., Resolve = false })`)
            // so this consumes the turn's main move; `teleport_to` only moves the seat
            // without settling and without touching the main-move budget.
        }
        // 规则书: 「若结果为3，将此卡置于场上，你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆」
        3 => {
            ctx::set_dest(ctx::Dest::Field);
            ctx::place_card(seat, "R:NFO", &Msg::new(key!("nfo_note")));
            ctx::log(seat, &Msg::new(key!("nfo_placed")).seat("who", seat));
            // TODO(规则书): 「你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆」 -- needs
            // the Fx.PayAt hook (C# `CardNfo.PayAt`): `p.amount = max(0, p.amount -
            // 1000)` then `H.Unplace(this, "discard", "用掉了")`.
        }
        // 规则书: 「若结果为4，选择你弃牌堆中的一张满足打出条件的卡打出」
        4 => {
            // TODO(ABI): 「选择你弃牌堆中的一张满足打出条件的卡打出」 -- needs discard
            // enumeration (C# `hidden.discard`) and `H.CanReplay` (the card's `WhyNot`),
            // then `H.PlayCard(...)` of the pick; the vocabulary can only `play_card` a
            // known id and cannot see the discard. The C# logs 「弃卡区没有能打出的卡」
            // when the filtered list is empty.
        }
        // 规则书: 「若结果为5，为自己的角色卡添加6个奇迹水晶，你每次获得资金时，可消耗一个奇迹水晶使本次的额度提高300」
        5 => {
            ctx::add_tok(seat, key!("nfo_crystals"), 6, i32::MAX);
            ctx::log(seat, &Msg::new(key!("nfo_crystals_added")).seat("who", seat));
            // TODO(规则书): 「为自己的角色卡添加6个奇迹水晶」 -- the crystals live on an
            // `H.ExtraOf<NfoCrystalsFx>(i)` attachment (C# `CardNfo.Effect` case 5), not
            // a plain seat token; needs H.ExtraOf skill attachments.
            // TODO(规则书): 「你每次获得资金时，可消耗一个奇迹水晶使本次的额度提高300」
            // -- needs the Fx.PayChoose hook (C# `NfoCrystalsFx.PayChoose`): on every
            // gain ask to spend 1 crystal for +300 on that payment.
        }
        _ => {}
    }
}

/// `H.SplitPay` -- every payer covers `ceil(ceil(total / n) / 10) * 10`.
fn split_pay(payers: &[i32], to: i32, total: i32, why: &Msg) {
    let list: Vec<i32> = payers
        .iter()
        .copied()
        .filter(|&p| p != to && !ctx::seat_out(p))
        .collect();
    if list.is_empty() || total <= 0 {
        return;
    }
    let per = (total + list.len() as i32 - 1) / list.len() as i32;
    let share = (per + 9) / 10 * 10;
    for p in list {
        ctx::transfer(p, to, share, why);
    }
}