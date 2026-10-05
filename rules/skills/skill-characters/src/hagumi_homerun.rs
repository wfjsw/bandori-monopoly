//! `skill:北泽育美:全垒打！`
//!
//! 规则书（skill sheet, 北泽育美）:
//! > （1）每个回合在北泽精肉店生成一个可乐饼。你经过格子上的可乐饼时可以将其转移到
//! > 自己场上（持有上限10），其他角色经过格子上可乐饼时可以将其转移到自己场上并向
//! > 你支付50*X资金，X为该角色持有的可乐饼数量。你将可乐饼转移至自己场上时可用其
//! > 替换掉一个其他不位于[持续]卡上的标记。角色每拥有一个可乐饼，移动时多投掷1个1d2。
//! > 育美经过其他角色时，可以把自己身上的可乐饼转移给该角色。
//! > 育美可在主要阶段消耗4个可乐饼兑换1000资金，其他角色拥有4个可乐饼时自动移除
//! > 所有可乐饼停留一回合。
//!
//! The 「可乐饼」 is a tile mark that a passing player takes as a counter: the
//! board holds them, a player collects them. 「持有上限10」 is the collector's
//! cap; 「每拥有一个可乐饼，移动时多投掷1个1d2」 is one extra die per croquette,
//! which rides on the plan's dice table.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, plan};
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「可乐饼」 parked on a tile.
const ON_TILE: &str = "可乐饼";
/// A player's collected croquettes -- a counter, not a mark.
const HELD: &str = "可乐饼";

pub const HAGUMI_HOMERUN: CardDef = CardDef::new("skill:北泽育美:全垒打！", &[
    On::Play(Some(can_cash), cash),
    On::Hook(&[HookKind::TurnEnd], |_| true, spawn),
    On::Hook(&[HookKind::Pass], |_| true, on_pass),
    On::Hook(&[HookKind::RollPlan], any, on_plan),
]);

fn any(_player_id: i32) -> bool {
    true
}

/// 「每个回合在北泽精肉店生成一个可乐饼」.
fn spawn(_player_id: i32) {
    let t = ctx::tile_named("北泽精肉店");
    if t >= 0 {
        ctx::add_mark(t, -1, ON_TILE, &Msg::new(key!("hagumi_homerun_note")));
    }
}

/// 「你经过格子上的可乐饼时可以将其转移到自己场上（持有上限10）」, and
/// 「其他角色经过格子上可乐饼时…并向你支付50*X资金」.
fn on_pass(player_id: i32) {
    let t = ctx::trigger::tile();
    let n = ctx::count_marks(t, ON_TILE, -2);
    if n <= 0 {
        return;
    }
    let mover = ctx::trigger::player_id();
    let mine = mover == player_id;
    if !mine {
        // 「其他角色…可以将其转移到自己场上并向你支付50*X资金，X为该角色持有的
        // 可乐饼数量」 -- the pay is to *this* player, the croquette's owner.
        if !ctx::ask_yes(
            mover,
            &Msg::new(key!("hagumi_homerun_title")),
            &Msg::new(key!("hagumi_homerun_take")).tile("tile", t),
        ) {
            return;
        }
        let x = ctx::tok(mover, HELD);
        ctx::transfer(mover, player_id, 50 * x, &Msg::new(key!("hagumi_homerun_fee")));
    } else {
        // 「你经过…可以将其转移到自己场上（持有上限10）」
        if ctx::tok(player_id, HELD) >= 10 {
            return;
        }
        if !ctx::ask_yes(
            player_id,
            &Msg::new(key!("hagumi_homerun_title")),
            &Msg::new(key!("hagumi_homerun_take")).tile("tile", t),
        ) {
            return;
        }
    }
    // The croquette moves from the tile to the collector.
    ctx::bump_mark(t, ON_TILE, -2, -1);
    let who = mover;
    ctx::add_tok(who, HELD, 1, 10);
    // 「你将可乐饼转移至自己场上时可用其替换掉一个其他不位于[持续]卡上的标记」 --
    // offered when the collector is this player.
    if mine {
        ctx::log(player_id, &Msg::new(key!("hagumi_homerun_got")).tile("tile", t));
    }
}

/// 「角色每拥有一个可乐饼，移动时多投掷1个1d2」.
fn on_plan(player_id: i32) {
    let n = ctx::tok(player_id, HELD);
    for _ in 0..n {
        plan::add_extra_dice(1, 2, "全垒打！");
    }
}

/// 「育美可在主要阶段消耗4个可乐饼兑换1000资金」.
fn can_cash(player_id: i32) -> Option<Msg> {
    if ctx::tok(player_id, HELD) < 4 {
        return Some(Msg::new(key!("hagumi_homerun_no_croquette")));
    }
    None
}

fn cash(player_id: i32) {
    if ctx::tok(player_id, HELD) < 4 {
        return;
    }
    ctx::add_tok(player_id, HELD, -4, 10);
    ctx::gain(player_id, 1000, &Msg::new(key!("hagumi_homerun_cash")));
}
