//! `PP:梦在前方，结彩当下` -- C# `CardDreamAhead` (MatchHost.cs:7479-7609):
//! placed before the game starts; crystals grow on draws and settle tax grows
//! with X.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:梦在前方，结彩当下`）:
//! > 梦在前方，结彩当下：
//! > [特]：
//! > 游戏开始前将此卡放置在[使用者]的[场地]且初始手牌减1。
//! > [持续]：
//! >
//! > （1）[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每获得一个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）。
//! >
//! > （2）[拥有者]不可盖房且手卡上限数量减1。
//! >
//! > （3）[拥有者]以外的玩家在[拥有者]拥有的格子[结算]时额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)。
//! >
//! > （4）[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子前后3格以内的无主的[可购买格子]。
//!
//! Not a hand play (C# `Normal => false`, no `Play` override): the card is
//! placed by `DeckBeforeGame`. The settle tax is live in the `SettleAfter`
//! hook; the crystal growth runs on the `Drew` hook.

use alloc::vec::Vec;

use card_sdk::abi::state_key;
use card_sdk::abi::{TriggerKind, HookKind, CardPile};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const DREAM_AHEAD: CardDef = CardDef::new("PP:梦在前方，结彩当下", &[
    On::Play(Some(can_buy), buy_one),
    On::Hook(&[HookKind::DeckBeforeGame], |_| true, deck_before_game),
    On::Hook(&[HookKind::Drew], drew_guard, drew),
    On::Hook(&[HookKind::SettleAfter], |_| true, settle_after),
]);

/// C# `Mem["x"]` -- the overflow counter (starts 0, +1 per crystal past the cap).
const SLOT_X: &str = "dream_ahead_x";

/// C# `CardDreamAhead.X`.
fn x(player_id: i32) -> i32 {
    ctx::slot(player_id, SLOT_X)
}

/// C# `H.Fans(seat)` -- both [P✽P粉丝] faces together.
fn fans(player_id: i32) -> i32 {
    ctx::tok(player_id, "P✽P粉丝(正)") + ctx::tok(player_id, "P✽P粉丝(反)")
}

/// C# `CardDreamAhead.DeckBeforeGame` -- pull the card out of the draw pile and
/// place it on the owner's field before the opening deal.
/// 规则书[特]: 「游戏开始前将此卡放置在[使用者]的[场地]且初始手牌减1」
fn deck_before_game(player_id: i32) -> card_sdk::Asked {
    // The hook fires on this card while it sits in a draw pile (once per
    // distinct id per player, up to 5 passes before the opening deal).
    if !ctx::take_card(player_id, CardPile::Deck, "PP:梦在前方，结彩当下") {
        return Ok(());
    }
    // 规则书[特]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:梦在前方，结彩当下", &Msg::new(key!("dream_ahead_place")));
    // 规则书[特]: 「初始手牌减1」 -- C# `H.IncV(seat, "startHandMinus")`.
    ctx::inc_slot(player_id, "startHandMinus", 1);
    Ok(())
}

/// C# `CardDreamAhead.Drew` -- per card drawn, add a crystal (cap 5); at the
/// cap each further card bumps X instead.
/// 规则书[持续]（1）: 「[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每
/// 获得一个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）」
/// Pure guard for [`drew`] -- the activation gate. `false`
/// means the card is not activated at all.
fn drew_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

fn drew(player_id: i32) -> card_sdk::Asked {
    for _ in 0..trigger::value().max(0) {
        if ctx::crystals() < 5 {
            ctx::add_crystals(1, 5);
        } else {
            ctx::inc_slot(player_id, SLOT_X, 1);
            ctx::log(
                player_id,
                &Msg::new(key!("dream_ahead_overflow")).i("x", x(player_id) as i64),
            );
        }
    }
    Ok(())
}

/// 规则书[持续]（2）: 「[拥有者]不可盖房且手卡上限数量减1」 -- both halves are
/// keyed flags the engine already reads: `noBuild` gates `why_not_build_on`,
/// `handLimit` is the limit itself.
/// Pure guard for [`no_build`] -- the activation gate. `false`
/// means the card is not activated at all.
fn no_build_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn no_build(player_id: i32) {
    ctx::state::add(player_id, state_key::NO_BUILD, 1);
    ctx::state::add(player_id, state_key::HAND_LIMIT, -1);
}

/// C# `CardDreamAhead.SettleAfter`: someone else settles on an owned deed --
/// they pay the owner `fans × X + min(X×30, 300)`.
fn settle_after(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::SettleAfter || !ctx::is_placed() {
        return Ok(());
    }
    let mover = trigger::player_id();
    if mover == player_id || ctx::player_out(mover) {
        return Ok(());
    }
    let x = x(player_id);
    if x <= 0 {
        return Ok(());
    }
    let t = trigger::tile();
    if t < 0 || ctx::tile_owner(t) != player_id {
        return Ok(());
    }
    // 规则书[持续]（3）: 「额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)」
    let amount = fans(player_id) * x + (x * 30).min(300);
    if amount <= 0 {
        return Ok(());
    }
    ctx::transfer(mover, player_id, amount, &Msg::new(key!("dream_ahead_tax")).i("n", amount as i64))?;
    Ok(())
}

// [持续]（4）「[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有无主的
// [可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子前后3格以内
// 的无主的[可购买格子]」 -- a press; the buy is `card_buy`, the resonance cost is
// the same `try_resonance` every other [共鸣] card uses.

/// （4）「使用此卡上3个[奇迹水晶]，[拥有者]购买…无主的[可购买格子]」.
fn can_buy(player_id: i32) -> Option<Msg> {
    if !ctx::is_placed() {
        return Some(Msg::new(key!("dream_ahead_not_placed")));
    }
    if ctx::crystals() < 3 {
        return Some(Msg::new(key!("dream_ahead_no_crystal")));
    }
    if free_near(player_id).is_empty() {
        return Some(Msg::new(key!("dream_ahead_no_tile")));
    }
    None
}

fn free_near(player_id: i32) -> Vec<i32> {
    let pos = ctx::player_pos(player_id);
    (0..ctx::tile_count())
        .filter(|&t| {
            ctx::is_buyable(t) && ctx::tile_owner(t) < 0 && ctx::dist(pos, t) <= 3
        })
        .collect()
}

fn buy_one(player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() < 3 {
        return Ok(());
    }
    let pool = free_near(player_id);
    if pool.is_empty() {
        return Ok(());
    }
    if !crate::resonance::try_resonance(player_id)? {
        return Ok(());
    }
    let opts: Vec<Msg> = pool
        .iter()
        .map(|&t| Msg::new(key!("dream_ahead_tile")).tile("tile", t))
        .collect();
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("dream_ahead_title")),
        &Msg::new(key!("dream_ahead_which")),
        &opts,
    )?;
    let Some(&t) = pool.get(pick) else { return Ok(()) };
    if !ctx::card_buy(player_id, t) {
        return Ok(());
    }
    ctx::add_crystals(-3, 0);
    ctx::log(player_id, &Msg::new(key!("dream_ahead_bought")).tile("tile", t));
    Ok(())
}