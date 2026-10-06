//! `PP:[丸山彩]憧憬的前方` -- C# `CardAyaLonging` (MatchHost.cs:7995-8063):
//! stay in play, take a card back from the discard, and shave payments while
//! poorest.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[丸山彩]憧憬的前方`）:
//! > [丸山彩]憧憬的前方：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。
//! > [持续]：
//! >
//! > （1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]
//! > 或[支付]时将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。
//! >
//! > （2）[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）。
//!
//! The discard pick runs in `Play`; the [持续]（1） shave lives in the
//! `PayChoose` hook.

use alloc::vec::Vec;

use card_sdk::abi::{TriggerKind, HookKind, CardPile};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const AYA_LONGING: CardDef = CardDef::new("PP:[丸山彩]憧憬的前方", &[
    On::Play(None, aya_longing),
    On::Hook(&[HookKind::PayAdd], |_| true, pay_add)]);

/// C# `CardAyaLonging.X` -- 100, or 200 once the owner holds 10+ [P✽P粉丝].
fn shave_x(player_id: i32) -> i32 {
    let fans = ctx::tok(player_id, "P✽P粉丝(正)") + ctx::tok(player_id, "P✽P粉丝(反)");
    if fans < 10 {
        100
    } else {
        200
    }
}

/// C# `CardAyaLonging.Poorest` -- every alive player has at least as much money.
fn poorest(player_id: i32) -> bool {
    let me = ctx::money_of(player_id);
    (0..ctx::player_count()).all(|p| p == player_id || ctx::player_out(p) || ctx::money_of(p) >= me)
}

fn aya_longing(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:[丸山彩]憧憬的前方", &Msg::new(key!("aya_longing_note")));
    // 规则书[手]: 「并将弃卡区中的一张卡加入手卡」
    // C# `H._hidden[i].discard.Distinct()` + `H.AskCard` + `H._hidden[i].discard.Remove`
    // + `H.AddToHand`.
    let mut ids: Vec<alloc::string::String> = Vec::new();
    for c in ctx::cards_in(player_id, ctx::CardPile::Discard) {
        if !ids.contains(&c) {
            ids.push(c);
        }
    }
    if ids.is_empty() {
        return Ok(());
    }
    let refs: Vec<&str> = ids.iter().map(|c| c.as_str()).collect();
    let pick = ctx::ask_card(
        player_id,
        &Msg::new(key!("aya_longing_title")),
        &Msg::new(key!("aya_longing_ask")),
        &refs,
    )?;
    let id = ids.swap_remove(pick.min(ids.len() - 1));
    if ctx::take_card(player_id, ctx::CardPile::Discard, &id) {
        ctx::add_to_hand(player_id, &id);
        ctx::log(
            player_id,
            &Msg::new(key!("aya_longing_back")).player_id("who", player_id).card("card", &id),
        );
    }
    Ok(())
}

/// C# `CardAyaLonging.PayAdd` -- while the owner is
/// the poorest alive player, every [消耗]/[支付] drops by X (floor 0).
fn pay_add(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::PayAdd
        || trigger::player_id() != player_id
        || trigger::value() <= 0
        || !poorest(player_id)
    {
        return Ok(());
    }
    let x = shave_x(player_id);
    let amount = trigger::value();
    trigger::set_pay_amount((amount - x).max(0));
    ctx::log(player_id, &Msg::new(key!("aya_longing_shave")).i("n", x as i64));
    // 规则书[持续]（2）: 「[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）」 --
    // on top of the X shave above, floored at 0.
    if crate::resonance::try_resonance(player_id)? {
        trigger::set_pay_amount((trigger::value() - 1500).max(0));
    }
    Ok(())
}