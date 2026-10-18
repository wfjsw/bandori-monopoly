//! `PP:梦在前方，结彩当下` -- C# `CardDreamAhead` (MatchHost.cs:7479-7609):
//!
//! 规则书（docs/rulebook/cards.json, id `PP:梦在前方，结彩当下`）:
//! > 梦在前方，结彩当下：
//! > [特]：
//! > 游戏开始前将此卡放置在[使用者]的[场地]且初始手牌减1。
//! > [持续]：
//!
//! > （1）[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每获得一个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）。
//!
//! > （2）[拥有者]不可盖房且手卡上限数量减1。
//!
//! > （3）[拥有者]以外的玩家在[拥有者]拥有的格子[结算]时额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)。
//!
//! > （4）[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子前后3格以内的无主的[可购买格子]。
//!
//! placed before the game starts; crystals grow on draws and settle tax grows
//! with X.
//! Not a hand play (C# `Normal => false`, no `Play` override): the card is
//! placed by `DeckBeforeGame`. The settle tax is live in the `SettleAfter`
//! hook; the crystal growth runs on the `Drew` hook.

use alloc::vec::Vec;

use card_sdk::abi::{prop, CardPile, ActionKind, TriggerKind};
use skill_groups::fans;
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const DREAM_AHEAD: CardDef = CardDef::new(
    "PP:梦在前方，结彩当下",
    &[
        // （4）「[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有
        // 无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意…」 --
        // a [持续] clause with a [反击] rider: `[反击]X：Y` = Y resolves before
        // X, so the buy is a counteraction at the owner's turn-end timing.
        //
        // TODO(规则书) LOY-DREAM-01: the turn-end family (`TurnEnd` /
        // `TurnEndBefore` / `TurnEndAfter`) is hook-only (`is_hook_only` in
        // `wasm_rules.rs`) -- `hand_counteractions` never opens there, so
        // `On::Counteract(&[ChainKind::TurnEndBefore], …)` is a dead entry.
        // Missing shape: a counteraction window on turn-end (Class-B blocker).
        // Wired as a field-card `On::Action` at `TurnEndBefore` (skill-personas:
        // a field card answers settlement points automatically, no declaration)
        // so the buy still settles before the turn end's own effects.
        On::Action(
            &[ActionKind::TurnEndBefore],
            "actor == owner && card.placed && card.counter('crystals') >= 3",
            // No residual guard: the condition covers placed + ≥3 crystals and
            // `buy_one` owns the "is there a tile" scan (one board walk per
            // turn-end instead of two).
            None,
            buy_one,
        ),
        On::Action(&[ActionKind::DeckBeforeGame], "", None, deck_before_game),
        On::Action(&[ActionKind::Drew], "actor == owner && card.placed", None, drew),
        // （3）「[结算]时额外[支付]」 is 行动阶段 15 -- an entry in the settle's
        // effect list (`SETTLE-STAGES.md` §4 M2), not the 「[触发结算]后」
        // window. A field card that replaces the body skips this entry.
        On::Action(
            &[ActionKind::SettleBody],
            "card.placed && actor != owner && slot('dream_ahead_x') > 0 && tile.id >= 0 && tile.owner == owner",
            None,
            settle_body,
        ),
    ],
)
// 规则书[持续]（2）: 「[拥有者]不可盖房且手卡上限数量减1」 -- two continuous
// properties (`docs/TILES.md`), stamped at placement and gone with the card:
// `noBuild` gates `why_not_build_on`, `handLimitDelta` is C# `Card.HandLimitDelta`.
.props(&[(prop::HAND_LIMIT_DELTA, -1), (prop::NO_BUILD, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"梦在前方，结彩当下"#)
        .raw_name(r#"梦在前方，结彩当下"#)
        .tags(&[r#"反击"#, r#"持续"#, r#"特"#])
        .text(r#"[特]：
游戏开始前将此卡放置在[使用者]的[场地]且初始手牌减1。
[持续]：
（1）[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每获得一个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）。
（2）[拥有者]不可盖房且手卡上限数量减1。
（3）[拥有者]以外的玩家在[拥有者]拥有的格子[结算]时额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)。
（4）[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子前后3格以内的无主的[可购买格子]。"#)
        .cell(r#"D10"#)
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]));

/// C# `Mem["x"]` -- the overflow counter (starts 0, +1 per crystal past the cap).
const SLOT_X: &str = "dream_ahead_x";

/// C# `CardDreamAhead.X`.
fn x(player_id: i32) -> i32 {
    ctx::slot(player_id, SLOT_X)
}

/// C# `H.Fans(seat)` -- both [P✽P粉丝] faces together.
fn fans(player_id: i32) -> i32 {
    fans::up(player_id) + fans::down(player_id)
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
    ctx::place_card(
        player_id,
        "PP:梦在前方，结彩当下",
        &Msg::new(key!("dream_ahead_place")),
    );
    // 规则书[特]: 「初始手牌减1」 -- the before-match-start point owns the
    // authoritative initial hand size; this lowers it by 1 (minimum 0).
    ctx::inc_start_hand(player_id, -1);
    // 规则书[持续]（2）: 「[拥有者]不可盖房」 -- the noBuild flag.
    Ok(())
}

/// C# `CardDreamAhead.Drew` -- per card drawn, add a crystal (cap 5); at the
/// cap each further card bumps X instead.
/// 规则书[持续]（1）: 「[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每
/// 获得一个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）」
/// `actor == owner && card.placed` is the pre.
/// `drew` fires **per single card** (an N-card draw raises it N times), so
/// 「每次抽牌时」 is one crystal per raise.
fn drew(player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() < 5 {
        ctx::add_crystals(1, 5)?;
    } else {
        ctx::inc_slot(player_id, SLOT_X, 1);
        ctx::log(
            player_id,
            &Msg::new(key!("dream_ahead_overflow")).i("x", x(player_id) as i64),
        );
    }
    Ok(())
}

/// 规则书[持续]（2）: 「[拥有者]不可盖房且手卡上限数量减1」 -- both halves are
/// continuous properties on the `CardDef` above, stamped at placement.

/// C# `CardDreamAhead.SettleAfter`: someone else settles on an owned deed --
/// they pay the owner `fans × X + min(X×30, 300)`.
/// 规则书[持续]（3）: 「…[结算]时额外[支付]」 -- 行动阶段 15
/// (`SETTLE-STAGES.md` §4 M2): an entry in the settle's effect list, so a body
/// replace (`trigger::cancelled()`) skips it.
fn settle_body(player_id: i32) -> card_sdk::Asked {
    // `card.placed && actor != owner && slot('dream_ahead_x') > 0 &&
    // tile.id >= 0 && tile.owner == owner` is the pre; `cancelled()` is
    // resolution-time and stays. `player_out` of the mover is a derived-list
    // residual.
    if trigger::cancelled() {
        return Ok(());
    }
    let mover = trigger::player_id();
    if ctx::player_out(mover) {
        return Ok(());
    }
    let x = x(player_id);
    let t = trigger::tile();
    // 规则书[持续]（3）: 「额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)」
    let amount = fans(player_id) * x + (x * 30).min(300);
    if amount <= 0 {
        return Ok(());
    }
    ctx::transfer(
        mover,
        player_id,
        amount,
        &Msg::new(key!("dream_ahead_tax")).i("n", amount as i64),
    )?;
    Ok(())
}

// [持续]（4）「[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有无主的
// [可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子前后3格以内
// 的无主的[可购买格子]」 -- the buy is `card_buy`, the resonance cost is
// the same `try_resonance` every other [共鸣] card uses.

/// Free buyable tiles within 3 of the owner (「所在格子前后3格以内的无主的
/// [可购买格子]」). The `TurnEndBefore` condition already covers placed + ≥3
/// crystals, so this is the only residual fact -- and `buy_one` owns the scan
/// (one board walk per turn-end, not two).
fn free_near(player_id: i32) -> Vec<i32> {
    let pos = ctx::player_pos(player_id);
    (0..ctx::tile_count())
        .filter(|&t| ctx::is_buyable(t) && ctx::tile_owner(t) < 0 && ctx::dist(pos, t) <= 3)
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
    // 规则书（4）: 「使用此卡上3个[奇迹水晶]，[拥有者]购买任意…无主的[可购买格子]」
    // -- printed order is spend-then-buy, so the crystals go **before** the
    // purchase attempt. Paying after `card_buy` let a failed buy keep the
    // crystals while the [共鸣] fuel was already gone; both costs now sit on
    // the same side of the purchase. The text offers no refund on a refused
    // buy.
    ctx::add_crystals(-3, 0)?;
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
    let Some(&t) = pool.get(pick) else {
        return Ok(());
    };
    if !ctx::card_buy(player_id, t) {
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new(key!("dream_ahead_bought")).tile("tile", t),
    );
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn buy_one_kani(player_id: i32) -> card_sdk::Asked {
    buy_one(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn deck_before_game_kani(player_id: i32) -> card_sdk::Asked {
    deck_before_game(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn drew_kani(player_id: i32) -> card_sdk::Asked {
    drew(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_body_kani(player_id: i32) -> card_sdk::Asked {
    settle_body(player_id)
}

