//! `MyGO:羽丘的不可思议女孩` -- C# `CardHaneoka` (MatchHost.cs:6495): roll 1d20
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:羽丘的不可思议女孩`）:
//! > 羽丘的不可思议女孩：
//! >  位于属于自己的格子上时，投掷1d20，若出目大于10则在当前格子免费加盖一层房屋，若出目大于15，则额外抽一张卡，大于20，则将此卡放置在自己场上，在后续任何时刻可将其置入弃牌堆并抵消一次任意付款。若严格小于10，此卡放入弃牌堆且视为此卡未生效
//!
//! on your own tile: 11+ builds a free house, 16+ draws, 20 keeps the card in
//! play so it can cancel a payment later.

use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const HANEOKA: CardDef = CardDef::new(
    "MyGO:羽丘的不可思议女孩",
    &[
        On::Play(Some(cant_play), haneoka),
        // C# `CardHaneoka.PayChoose` -- while placed, may cancel one payment.
        On::Hook(&[HookKind::PayChoose], |_| true, pay_choose),
    ],
);

const ID: &str = "MyGO:羽丘的不可思议女孩";

/// 规则书: 「位于属于自己的格子上时」 -- C# `CardHaneoka.WhyNot` refuses the
/// card off your own tile ("只有站在自己的格子上才能打出").
fn cant_play(player_id: i32) -> Option<Msg> {
    let pos = ctx::player_pos(player_id);
    if ctx::tile_owner(pos) != player_id {
        return Some(Msg::new(key!("haneoka_why_not_own")));
    }
    None
}

fn haneoka(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    // 规则书: 「位于属于自己的格子上时」
    if ctx::tile_owner(pos) != player_id {
        return Ok(());
    }
    // 规则书: 「投掷1d20」
    // `ctx::roll` honours a forced extreme (「以理论最大值或最小值结算」).
    let r = ctx::roll(player_id, 1, 20);
    if r < 10 {
        // 规则书: 「若严格小于10，此卡放入弃牌堆且视为此卡未生效」
        ctx::log(
            player_id,
            &Msg::new(key!("haneoka_no_effect"))
                .card("card", ID)
                .i("roll", r as i64),
        );
        // TODO(规则书)[judgement]: 「视为此卡未生效」 -- the clause names a state without
        // saying what observes it. `PlayCtx.Effective = false` is the C#'s mutable
        // side channel and is not being ported (a routine should *return* whether
        // it took effect); but before that lands, what "not effective" changes has
        // to be ruled: does the card get spent (haneoka 「放入弃牌堆且视为此卡未生效」
        // says yes) or not (noble_blue / starry_night's "the card is spent anyway"
        // implies no)? And what counts a use that this would suppress?
        return Ok(());
    }
    if r > 10 {
        // 规则书: 「若出目大于10则在当前格子免费加盖一层房屋」 -- C# `H.AddHouse(seat,
        // pos, CardName)`.
        // The C# gate is `H.WhyNotBuildOn(seat, pos) == null || (own && rent &&
        // under the rent-table cap && !mortgaged)`; the own + not-mortgaged arm
        // is what we can check (`WhyNotBuildOn` and the `rent.Length - 1` cap
        // have no query).
        if ctx::tile_owner(pos) == player_id && !ctx::mortgaged_of(pos) {
            let h = ctx::add_house(pos, 1);
            ctx::log(
                player_id,
                &Msg::new(key!("haneoka_house"))
                    .player_id("who", player_id)
                    .tile("tile", pos)
                    .i("n", h as i64)
                    .card("card", ID),
            );
        } else {
            ctx::log(
                player_id,
                &Msg::new(key!("haneoka_no_house"))
                    .tile("tile", pos)
                    .card("card", ID),
            );
        }
        // 规则书: 「免费加盖一层房屋」 -- `add_house` itself clamps at the tile's
        // rent-table max (`rent.Length - 1`), so a tile that cannot take another
        // house simply does not move.
    }
    if r > 15 {
        // 规则书: 「若出目大于15，则额外抽一张卡」
        ctx::draw(player_id, 1);
    }
    if r >= 20 {
        // 规则书: 「大于20，则将此卡放置在自己场上」 -- the PayChoose hook below
        // offers 「在后续任何时刻可将其置入弃牌堆并抵消一次任意付款」.
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(
            player_id,
            ID,
            &Msg::new(key!("haneoka_placed"))
                .card("card", ID)
                .player_id("who", player_id),
        );
    }
    Ok(())
}

/// C# `CardHaneoka.PayChoose` -- while placed, ask to bin the card and cancel
/// one payment of yours (`p.cancel = true`).
fn pay_choose(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::PayChoose
        || trigger::player_id() != player_id
        || !ctx::is_placed()
    {
        return Ok(());
    }
    let amount = trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    // 规则书: 「在后续任何时刻可将其置入弃牌堆并抵消一次任意付款」 -- C# `Ask`:
    // `H.AskYes(Seat, CardName, "要付 ... ：要把「羽丘的不可思议女孩」放入弃卡区，
    // 抵消这次付款吗？")`, then `p.cancel = true` and `H.Unplace(this, "discard", ...)`.
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("haneoka_cancel_title")).card("card", ID),
        &Msg::new(key!("haneoka_cancel_ask")).n("money", amount as i64),
    )? {
        return Ok(());
    }
    if !ctx::is_placed() {
        return Ok(());
    }
    trigger::set_pay_amount(0);
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("haneoka_cancelled"))
            .card("card", ID)
            .n("money", amount as i64),
    );
    Ok(())
}
