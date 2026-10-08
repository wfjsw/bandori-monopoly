//! `R:NFO` -- C# `CardNfo` (MatchHost.cs:10517-10628): roll 1d6 (wrapping above 6)
//!
//! 规则书（docs/rulebook/cards.json, id `R:NFO`）:
//! > NFO：
//! >  投掷1d6并在回合结束后获得一层[停留]。若投掷结果为1，获得1500资金；若结果为2，立刻移动到你前方最近一名玩家的前方一格并使其格子上的玩家分摊支付你1000资金，视为本回合的主要移动但不[触发结算]；若结果为3，将此卡置于场上，你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆；若结果为4，选择你弃牌堆中的一张满足打出条件的卡打出；若结果为5，为自己的角色卡添加6个奇迹水晶，你每次获得资金时，可消耗一个奇迹水晶使本次的额度提高300；若结果为6，依次获得结果1-5的全部效果。（若结果严格大于6，则从1开始重新计数）
//!
//! and resolve the matching effect; 6 runs 1 through 5 in order. Whatever the
//! roll, a [停留] layer lands at turn end.

use alloc::string::String;
use alloc::vec::Vec;

use card_sdk::abi::{CardPile, HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const NFO: CardDef = CardDef::new(
    "R:NFO",
    &[
        On::Hook(&[card_sdk::abi::HookKind::PayChoose], Some(gain_guard), gain_bump, ""),
        On::Play(None, play, ""),
        On::Hook(&[HookKind::PayAt], None, counteract, ""),
        On::AtEnd(at_end),
    ],
);

const ID: &str = "R:NFO";

/// The nearest other player strictly ahead of `player_id` on the ring (C# `H.NearestAhead`).
fn nearest_ahead(player_id: i32) -> i32 {
    let n = ctx::tile_count();
    let pos = ctx::player_pos(player_id);
    if n <= 0 || pos < 0 {
        return -1;
    }
    let mut best = -1;
    let mut best_d = i32::MAX;
    for o in ctx::others(player_id) {
        let p = ctx::player_pos(o);
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

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「投掷1d6」 -- C# `(H.Roll(i, 1, 6, "NFO") - 1) % 6 + 1` wraps above 6.
    let roll = ctx::roll(player_id, 1, 6);
    // 规则书: 「（若结果严格大于6，则从1开始重新计数）」
    let k = (roll - 1).rem_euclid(6) + 1;
    // 规则书: 「并在回合结束后获得一层[停留]」 -- C# `H._turnCtx.AfterEnd.Add(() =>
    // H.GiveStay(i, 1, i, "NFO"))`; the scheduled body is `at_end`.
    ctx::at_turn_end(player_id);
    if k == 6 {
        // 规则书: 「若结果为6，依次获得结果1-5的全部效果。」
        ctx::effect(player_id, &Msg::new(key!("nfo_effect_all")));
        for x in 1..=5 {
            apply(player_id, x)?;
        }
    } else {
        // 规则书: 「若投掷结果为1，获得1500资金；若结果为2，…；若结果为3，…；若结果为4，…；若结果为5，…」
        ctx::effect(player_id, &Msg::new(key!("nfo_effect")).i("n", k as i64));
        apply(player_id, k)?;
    }
    Ok(())
}

/// C# `H._turnCtx.AfterEnd` -- the [停留] layer lands after the turn's status
/// wear-off (so it survives into the next turn).
fn at_end(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「并在回合结束后获得一层[停留]」 -- C# `H.GiveStay(i, 1, i, "NFO")`.
    ctx::give_stay(player_id, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("nfo_stay")).player_id("who", player_id),
    );
    Ok(())
}

fn apply(player_id: i32, k: i32) -> card_sdk::Asked {
    if ctx::player_out(player_id) {
        return Ok(());
    }
    match k {
        // 规则书: 「若投掷结果为1，获得1500资金」
        1 => {
            ctx::gain(player_id, 1500, &Msg::new(key!("nfo_why")))?;
        }
        // 规则书: 「若结果为2，立刻移动到你前方最近一名玩家的前方一格并使其格子上的玩家分摊支付你1000资金，视为本回合的主要移动但不[触发结算]」
        2 => {
            let num = nearest_ahead(player_id);
            if num < 0 {
                ctx::log(player_id, &Msg::new(key!("nfo_no_ahead")));
                return Ok(());
            }
            let pos = ctx::player_pos(player_id);
            let pos2 = ctx::player_pos(num);
            // 规则书: 「并使其格子上的玩家分摊支付你1000资金」 -- `H.SeatsOn(pos2, i)`.
            let payers: Vec<i32> = ctx::players_on(pos2, player_id);
            // 规则书: 「立刻移动到你前方最近一名玩家的前方一格」 -- C#
            // `H.CardMove(c, new MoveCtx { Steps = H.Forward(pos, pos2) + 1,
            // Resolve = false })`: a forward walk of that many steps lands one
            // tile past that player.
            let steps = ctx::tile_forward(pos, pos2) + 1;
            ctx::plan::set_steps(steps);
            // 规则书: 「不[触发结算]」 -- C# `Resolve = false`.
            ctx::plan::set_resolve(false);
            // 规则书: 「视为本回合的主要移动」 -- C# `H.CardMove` (`MainMoveAs`)
            // consumes the turn's main move and walks the plan immediately.
            ctx::card_move(player_id);
            let to = ctx::tile_steps_ahead(player_id, steps);
            if to >= 0 {
                ctx::log(
                    player_id,
                    &Msg::new(key!("nfo_move"))
                        .player_id("who", player_id)
                        .tile("tile", to),
                );
            }
            // 规则书: 「并使其格子上的玩家分摊支付你1000资金」 -- C# `H.SplitPay(payers, i,
            // 1000, ...)`, only if the move did not knock the player_id out (C# `if (!H.Out(i))`).
            if !ctx::player_out(player_id) {
                split_pay(&payers, player_id, 1000, &Msg::new(key!("nfo_pay")))?;
            }
        }
        // 规则书: 「若结果为3，将此卡置于场上，你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆」
        3 => {
            ctx::set_dest(ctx::Dest::Field);
            ctx::place_card(player_id, ID, &Msg::new(key!("nfo_note")));
            ctx::log(
                player_id,
                &Msg::new(key!("nfo_placed")).player_id("who", player_id),
            );
            // 「你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆」 -- see `counteract`.
        }
        // 规则书: 「若结果为4，选择你弃牌堆中的一张满足打出条件的卡打出」
        4 => {
            replay_from_discard(player_id)?;
        }
        // 规则书: 「若结果为5，为自己的角色卡添加6个奇迹水晶，你每次获得资金时，可消耗一个奇迹水晶使本次的额度提高300」
        5 => {
            ctx::add_tok(player_id, key!("nfo_crystals"), 6, i32::MAX)?;
            ctx::log(
                player_id,
                &Msg::new(key!("nfo_crystals_added")).player_id("who", player_id),
            );
            // 「为自己的角色卡添加6个奇迹水晶」 -- held as a player counter
            // (`nfo_crystals`), the stand-in for the C# `H.ExtraOf<NfoCrystalsFx>`
            // attachment; the gain half is `gain_bump` below.
        }
        _ => {}
    }
    Ok(())
}

/// 规则书: 「选择你弃牌堆中的一张满足打出条件的卡打出」 -- C#
/// `h.discard.Distinct()` + `H.CanReplay` + `H.AskCard` + `h.discard.Remove` +
/// `H.PlayCard`, which then applies the card's own `Dest`.
fn replay_from_discard(player_id: i32) -> card_sdk::Asked {
    let mut ok: Vec<String> = Vec::new();
    for id in ctx::cards_in(player_id, ctx::CardPile::Discard) {
        // C# `where id != Id && H.CanReplay(i, id)` (`Distinct()` drops duplicates).
        if id == ID || ok.contains(&id) {
            continue;
        }
        if ctx::card_replayable(player_id, &id) {
            ok.push(id);
        }
    }
    if ok.is_empty() {
        // C# 「弃卡区没有能打出的卡（NFO）」.
        ctx::log(player_id, &Msg::new(key!("nfo_no_replay")));
        return Ok(());
    }
    let refs: Vec<&str> = ok.iter().map(|c| c.as_str()).collect();
    let pick = ctx::ask_card(
        player_id,
        &Msg::new(key!("nfo_title")),
        &Msg::new(key!("nfo_replay_ask")),
        &refs,
    )?;
    let id = ok.swap_remove(pick.min(ok.len() - 1));
    // C# `h.discard.Remove(text)` -- the card leaves the discard before it resolves.
    if !ctx::take_card(player_id, ctx::CardPile::Discard, &id) {
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new("log.play")
            .player_id("who", player_id)
            .card("card", &id),
    );
    // C# `H.PlayCard(...)` runs the effect and then applies that card's `Dest`.
    let dest = ctx::play_card(&id, player_id)?;
    apply_dest(player_id, &id, dest);
    Ok(())
}

/// Land a card just replayed out of the discard on its own `Dest` (C#
/// `PlayCard`'s `switch (c.Dest)`). `Field` / `Banished` need nothing here:
/// the inner card already placed itself, or is simply out of the game.
fn apply_dest(player_id: i32, id: &str, dest: ctx::Dest) {
    match dest {
        ctx::Dest::Graveyard => ctx::to_discard(player_id, id),
        ctx::Dest::Hand => ctx::add_to_hand(player_id, id),
        ctx::Dest::Field | ctx::Dest::Banished => {}
        ctx::Dest::DeckTop => ctx::add_to_deck_at(player_id, id, ctx::DeckPos::Top),
        ctx::Dest::DeckBottom => ctx::add_to_deck_at(player_id, id, ctx::DeckPos::Bottom),
        ctx::Dest::DeckRandom => ctx::add_to_deck_at(player_id, id, ctx::DeckPos::Random),
    }
}

/// `H.SplitPay` -- every payer covers `ceil(ceil(total / n) / 10) * 10`.
/// `PIPELINE-AUDIT` Q2: the command-wide pre-split stage shapes `total` before
/// it divides (`ctx::split_pay`).
fn split_pay(payers: &[i32], to: i32, total: i32, why: &Msg) -> card_sdk::Asked {
    ctx::split_pay(payers, to, total, why)?;
    Ok(())
}

/// C# `CardNfo.PayAt` (effect 3, while placed): the owner's next payment is cut
/// by 1000, then the card is used up and goes to the discard pile. Runs through
/// the Fx hook dispatch at `payAt` (after `PayChoose`, before the `pay` [反击]
/// window), so this is a field effect, not a [反击].
fn counteract(player_id: i32) -> card_sdk::Asked {
        if trigger::player_id() != player_id
        || !ctx::is_placed() {
        return Ok(());
        }
    let amount = trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    trigger::set_pay_amount((amount - 1000).max(0));
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("nfo_used"))
            .player_id("who", player_id)
            .n("money", (amount.min(1000)) as i64),
    );
    Ok(())
}

fn gain_guard(player_id: i32) -> bool {
    ctx::trigger::target() == player_id && ctx::tok(player_id, "nfo_crystals") > 0
}

/// 「你每次获得资金时，可消耗一个奇迹水晶使本次的额度提高300」 -- C#
/// `NfoCrystalsFx.PayChoose`, keyed on `p.to == Player` (the gainer).
fn gain_bump(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::target() != player_id {
        return Ok(());
    }
    if ctx::tok(player_id, "nfo_crystals") < 1 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("nfo_title")),
        &Msg::new(key!("nfo_bump")),
    )? {
        return Ok(());
    }
    ctx::add_tok(player_id, "nfo_crystals", -1, i32::MAX)?;
    ctx::trigger::set_pay_amount(ctx::trigger::value() + 300);
    ctx::log(player_id, &Msg::new(key!("nfo_bumped")));
    Ok(())
}
