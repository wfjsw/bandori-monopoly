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

use card_sdk::abi::{CardPile, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const DREAM_AHEAD: CardDef = CardDef::new("PP:梦在前方，结彩当下", &[
    On::Hook(&[TriggerKind::DeckBeforeGame], deck_before_game),
    On::Hook(&[TriggerKind::Drew], drew),
    On::Hook(&[TriggerKind::SettleAfter], settle_after),
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
fn deck_before_game(player_id: i32) {
    // The hook fires on this card while it sits in a draw pile (once per
    // distinct id per player, up to 5 passes before the opening deal).
    if !ctx::take_card(player_id, CardPile::Deck, "PP:梦在前方，结彩当下") {
        return;
    }
    // 规则书[特]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:梦在前方，结彩当下", &Msg::new(key!("dream_ahead_place")));
    // 规则书[特]: 「初始手牌减1」 -- C# `H.IncV(seat, "startHandMinus")`.
    ctx::inc_slot(player_id, "startHandMinus", 1);
}

/// C# `CardDreamAhead.Drew` -- per card drawn, add a crystal (cap 5); at the
/// cap each further card bumps X instead.
/// 规则书[持续]（1）: 「[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每
/// 获得一个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）」
fn drew(player_id: i32) {
    if !ctx::is_placed(player_id) || trigger::player_id() != player_id {
        return;
    }
    for _ in 0..trigger::value().max(0) {
        if ctx::crystals(player_id) < 5 {
            ctx::add_crystals(player_id, 1, 5);
        } else {
            ctx::inc_slot(player_id, SLOT_X, 1);
            ctx::log(
                player_id,
                &Msg::new(key!("dream_ahead_overflow")).i("x", x(player_id) as i64),
            );
        }
    }
}

// TODO(规则书): [持续]（2）「[拥有者]不可盖房且手卡上限数量减1」 -- needs the
// Fx.CanBuild hook (C# `Card.CanBuild` returning `player_id != Player`) and
// Fx.HandLimitDelta (C# `Card.HandLimitDelta` returning -1 for the owner).

/// C# `CardDreamAhead.SettleAfter`: someone else settles on an owned deed --
/// they pay the owner `fans × X + min(X×30, 300)`.
fn settle_after(player_id: i32) {
    if trigger::kind() != TriggerKind::SettleAfter || !ctx::is_placed(player_id) {
        return;
    }
    let mover = trigger::player_id();
    if mover == player_id || ctx::player_out(mover) {
        return;
    }
    let x = x(player_id);
    if x <= 0 {
        return;
    }
    let t = trigger::tile();
    if t < 0 || ctx::tile_owner(t) != player_id {
        return;
    }
    // 规则书[持续]（3）: 「额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)」
    let amount = fans(player_id) * x + (x * 30).min(300);
    if amount <= 0 {
        return;
    }
    ctx::transfer(mover, player_id, amount, &Msg::new(key!("dream_ahead_tax")).i("n", amount as i64));
}

// TODO(规则书): [持续]（4）「[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格
// 以内有无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子
// 前后3格以内的无主的[可购买格子]」 -- the Fx.TurnEnd hook and the crystal spend
// (`ctx::add_crystals(player_id, -3, 0)`) are expressible, and the free tiles are
// `is_buyable(t) && tile_owner(t) < 0 && dist(pos, t) <= 3`, but the body still
// needs H.TryResonance (discard 「PP:[衍生]共鸣」) and the buy routine
// (`H.BuyRoutine(Seat, tile, free: false, ...)`).