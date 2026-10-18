//! `skill:北泽育美:全垒打！`
//!
//! 规则书（skill sheet, 北泽育美, `角色技能`）:
//! > （1）每个回合在北泽精肉店生成一个可乐饼。你经过格子上的可乐饼时可以将其转移到
//! > 自己场上（持有上限10），其他角色经过格子上可乐饼时可以将其转移到自己场上并向
//! > 你支付50*X资金，X为该角色持有的可乐饼数量。你将可乐饼转移至自己场上时可用其
//! > 替换掉一个其他不位于[持续]卡上的标记。角色每拥有一个可乐饼，移动时多投掷1个1d2。
//! > 育美经过其他角色时，可以把自己身上的可乐饼转移给该角色。育美可在主要阶段消耗4个
//! > 可乐饼兑换1000资金，其他角色拥有4个可乐饼时自动移除所有可乐饼停留一回合。
//!
//! Sheet 2026-10-17 restores the second paragraph (it was off the sheet on
//! 2026-10-06 and is back). 「可乐饼」 is a tile mark that a passing player takes
//! as a counter: the board holds them, a player collects them. 「持有上限10」 is
//! the collector's cap; 「每拥有一个可乐饼，移动时多投掷1个1d2」 is one extra die
//! per croquette, which rides on the plan's dice table.
//!
//! Second-paragraph timing: 「育美经过其他角色时」 is the passer's step onto a
//! tile another player stands on (`PassTile`, `SETTLE-STAGES.md` §4 M4 -- the
//! same reading sakiko_life takes for 「经过其他玩家」), not the end-tile
//! [重叠]. 「主要阶段」 is the skill button (`On::Play`). 「其他角色拥有4个
//! 可乐饼时」 is a standing condition on the held count; held-token writes have
//! no post-commit `*Changed` hook (only the pre-move `markerGain` window), so
//! the sweep runs after each of this skill's own croquette-raising writes --
//! the only sites that can raise the count (marker ownership).

use card_sdk::abi::{FilterValue, ActionKind, FilterKind, MarkFilter};
use ruleset_core::ctx;
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「可乐饼」 parked on a tile.
const ON_TILE: &str = "可乐饼";
/// A player's collected croquettes -- a counter, not a mark.
const HELD: &str = "可乐饼";

pub const HAGUMI_HOMERUN: CardDef = CardDef::new(
    "skill:北泽育美:全垒打！",
    &[
        On::Action(&[ActionKind::TurnEnd], "", None, at_turn_end),
        // 「你经过格子上的可乐饼时」 / 「其他角色经过格子上可乐饼时」 -- a tile
        // with a croquette is the hook's applicability (`count_marks` is not in
        // the condition vocabulary, so it is the residual guard).
        On::Action(&[ActionKind::Pass], "", Some(on_pass_has_croquette), on_pass),
        // 「育美经过其他角色时」 -- the passer's step onto a tile another player
        // stands on (行动阶段 12 [经过]). The other player is read off the tile
        // (`passTile` has no `target`).
        On::Action(
            &[ActionKind::PassTile],
            "actor == owner",
            Some(passing_a_player),
            on_pass_player,
        ),
        // 「育美可在主要阶段消耗4个可乐饼兑换1000资金」 -- the skill button.
        On::Play("", Some(can_cash), cash),
        On::FilterValue(&[FilterKind::DiceRollPlan], "", None, on_plan),
    ],
);

/// Residual guard for 「经过格子上的可乐饼时」 -- `count_marks` stays here
/// (not yet in the condition vocabulary).
fn on_pass_has_croquette(_player_id: i32) -> bool {
    ctx::count_marks(ctx::trigger::tile(), &MarkFilter::any().kind(ON_TILE)) > 0
}

/// Residual guard for 「育美经过其他角色时」 -- some other player stands on the
/// tile being stepped on.
fn passing_a_player(player_id: i32) -> bool {
    let t = ctx::trigger::tile();
    t >= 0 && !ctx::players_on(t, player_id).is_empty()
}

/// 「每个回合在北泽精肉店生成一个可乐饼」 + the standing overflow sweep
/// (「其他角色拥有4个可乐饼时自动移除所有可乐饼停留一回合」).
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_named("北泽精肉店");
    if t >= 0 {
        // One fresh row per turn -- croquettes accumulate as separate units.
        ctx::place_mark(
            t,
            ON_TILE,
            "",
            -1,
            ctx::self_uid(),
            1,
            &Msg::new(key!("hagumi_homerun_note")), card_sdk::abi::Stack::Fresh);
    }
    sweep_overflow(player_id)
}

/// 「你经过格子上的可乐饼时可以将其转移到自己场上（持有上限10）」, and
/// 「其他角色经过格子上可乐饼时…并向你支付50*X资金」. The croquette-on-tile
/// check is the residual guard (`on_pass_has_croquette`).
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    let mover = ctx::trigger::player_id();
    let mine = mover == player_id;
    if !mine {
        // 「其他角色…可以将其转移到自己场上并向你支付50*X资金，X为该角色持有的
        // 可乐饼数量」 -- the pay is to *this* player, the croquette's owner.
        if !ctx::ask_yes(
            mover,
            &Msg::new(key!("hagumi_homerun_title")),
            &Msg::new(key!("hagumi_homerun_take")).tile("tile", t),
        )? {
            return Ok(());
        }
        let x = ctx::tok(mover, HELD);
        ctx::transfer(
            mover,
            player_id,
            50 * x,
            &Msg::new(key!("hagumi_homerun_fee")),
        )?;
    } else {
        // 「你经过…可以将其转移到自己场上（持有上限10）」
        if ctx::tok(player_id, HELD) >= 10 {
            return Ok(());
        }
        if !ctx::ask_yes(
            player_id,
            &Msg::new(key!("hagumi_homerun_title")),
            &Msg::new(key!("hagumi_homerun_take")).tile("tile", t),
        )? {
            return Ok(());
        }
    }
    // The croquette moves from the tile to the collector.
    ctx::bump_mark(t, &MarkFilter::any().kind(ON_TILE), -1);
    let who = mover;
    ctx::add_tok(who, HELD, 1, 10)?;
    // 「你将可乐饼转移至自己场上时可用其替换掉一个其他不位于[持续]卡上的标记」 --
    // offered when the collector is this player.
    if mine {
        ctx::log(
            player_id,
            &Msg::new(key!("hagumi_homerun_got")).tile("tile", t),
        );
    }
    // 「其他角色拥有4个可乐饼时自动移除所有可乐饼停留一回合」 -- the take just
    // raised another player's held count.
    sweep_overflow(player_id)
}

/// 「育美经过其他角色时，可以把自己身上的可乐饼转移给该角色」.
///
/// The text names no number: the exact reading is any number she chooses,
/// including zero (decline the yes/no). The transfer is from her held pool to
/// the other player's, honouring 「持有上限10」 on the recipient.
fn on_pass_player(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    let Some(&other) = ctx::players_on(t, player_id).first() else {
        return Ok(());
    };
    let have = ctx::tok(player_id, HELD);
    if have <= 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("hagumi_homerun_title")),
        &Msg::new(key!("hagumi_homerun_give"))
            .player_id("who", other)
            .tile("tile", t),
    )? {
        return Ok(());
    }
    let room = 10 - ctx::tok(other, HELD);
    let max = have.min(room).max(0);
    if max <= 0 {
        return Ok(());
    }
    let n = ctx::ask_number(
        player_id,
        &Msg::new(key!("hagumi_homerun_title")),
        &Msg::new(key!("hagumi_homerun_give_n")).player_id("who", other),
        1,
        max,
    )?;
    if n <= 0 {
        return Ok(());
    }
    ctx::add_tok(player_id, HELD, -n, 10)?;
    ctx::add_tok(other, HELD, n, 10)?;
    ctx::log(
        player_id,
        &Msg::new(key!("hagumi_homerun_given"))
            .player_id("who", other)
            .i("n", n as i64),
    );
    // 「其他角色拥有4个可乐饼时自动移除所有可乐饼停留一回合」 -- the gift just
    // raised the recipient's held count (including when she hands over 4+).
    sweep_overflow(player_id)
}

/// 「育美可在主要阶段消耗4个可乐饼兑换1000资金」 -- the skill button's gate.
fn can_cash(player_id: i32) -> Option<Msg> {
    if ruleset_core::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if ctx::tok(player_id, HELD) < 4 {
        return Some(Msg::new(key!("hagumi_homerun_no_croquette")));
    }
    None
}

/// 「消耗4个可乐饼兑换1000资金」 -- a counter spend, not money. 1000 is already
/// a multiple of 10 (money rounds once after every add/mul).
fn cash(player_id: i32) -> card_sdk::Asked {
    if ctx::tok(player_id, HELD) < 4 {
        return Ok(());
    }
    ctx::add_tok(player_id, HELD, -4, 10)?;
    ctx::gain(player_id, 1000, &Msg::new(key!("hagumi_homerun_cash")))?;
    Ok(())
}

/// 「其他角色拥有4个可乐饼时自动移除所有可乐饼停留一回合」 -- automatic, no
/// choice: every other player at 4+ held croquettes loses them all and gains one
/// [停留] layer (「一回合」 is the stay layer's own `Tick::TurnEnd` wear-off).
///
/// Held-token writes raise only the pre-move `markerGain` window, not a
/// post-commit `*Changed` event, so this sweep is the count-changed clause and
/// runs after each of this skill's own croquette-raising writes (the only sites
/// that can raise the count). A `TODO(规则书)` stays on the missing host event.
fn sweep_overflow(player_id: i32) -> card_sdk::Asked {
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) {
            continue;
        }
        let n = ctx::tok(p, HELD);
        if n < 4 {
            continue;
        }
        // 效果适用: one entry naming the clause, who, and the outcome, before
        // the removal / stay land.
        ctx::log(
            player_id,
            &Msg::new(key!("hagumi_homerun_overflow"))
                .player_id("who", p)
                .i("n", n as i64),
        );
        ctx::add_tok(p, HELD, -n, 10)?;
        rules_status::stay::add(p, 1);
    }
    Ok(())
}

/// 「角色每拥有一个可乐饼，移动时多投掷1个1d2」 -- one extra d2 per croquette.
/// The host folds `extra_count`/`extra_sides` via `add_extra_dice`, so a single
/// `(n, 2)` pair is the same dice pool as the old `n` one-die calls.
fn on_plan(player_id: i32, mut v: FilterValue) -> FilterValue {
    let n = ctx::tok(player_id, HELD);
    if n > 0 {
        v.extra_count = n;
        v.extra_sides = 2;
    }
    v
}

// TODO(规则书): 「其他角色拥有4个可乐饼时」 is a standing count clause. Held
// tokens (`add_tok`) raise only the pre-move `markerGain` window -- there is no
// post-commit held-changed event to hang the sweep on, so it is re-run at this
// skill's own write sites (plus the turn-end tick). A host `heldChanged` event
// would let this be a pure event handler.

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn spawn_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_plan_kani(player_id: i32, v: FilterValue) -> FilterValue {
    on_plan(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_player_kani(player_id: i32) -> card_sdk::Asked {
    on_pass_player(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn cash_kani(player_id: i32) -> card_sdk::Asked {
    cash(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn sweep_overflow_kani(player_id: i32) -> card_sdk::Asked {
    sweep_overflow(player_id)
}
/// cfg(kani) direct-call wrapper (ppcost body-id pattern).
#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}
