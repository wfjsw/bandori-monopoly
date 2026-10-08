//! `event:修复公告` -- 事件卡「修复公告」（中立事件 A25，衍生）.
//!
//! 事件文本（data/events.json, id `修复公告`）:
//! > 前2个房屋造价为2000的格子和前1个房屋造价为1500的格子的加盖金额减半，且累计在上述格子上加盖后将此卡移除。

use card_sdk::abi::{prop, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep};

const ID: &str = "修复公告";

/// The three designated tiles, on the instance's props (`-1` when the board
/// does not have one of the named costs).
const T0: &str = "t0";
const T1: &str = "t1";
const T2: &str = "t2";
/// Builds still owed on the designated tiles before the card leaves.
const REMAIN: &str = "remain";

pub const FIX_NOTE: CardDef = CardDef::new(
    "event:修复公告",
    &[
        On::Play("", None, play),
        On::Hook(&[HookKind::BuildBefore], "", Some(always), before_build),
        On::Hook(&[HookKind::BuildAfter], "", Some(always), after_build),
    ],
);

fn always(_player_id: i32) -> bool {
    true
}

/// 规则书: 「前2个房屋造价为2000的格子和前1个房屋造价为1500的格子」
/// -- the first two tiles in board order whose house cost (`TileData.house`,
/// stamped as `prop::HOUSE`) is 2000, and the first whose house cost is 1500.
/// 「加盖金额减半」 and 「累计在上述格子上加盖后将此卡移除」 then key off that set.
/// TODO(规则书): 「累计…加盖后将此卡移除」 does not say how many builds settle
/// the count. This body takes 3 (one per designated tile) and expires when the
/// total builds on those tiles reach 3; a single build anywhere on them is the
/// other plausible reading.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    let mut t0 = -1;
    let mut t1 = -1;
    let mut t2 = -1;
    let mut n2000 = 0;
    let mut n1500 = 0;
    for t in 0..ctx::tile_count() {
        // 「房屋造价」 is the printed per-level price, not a modified build cost.
        let house = ctx::tile_prop(t, prop::HOUSE);
        if house == 2000 && n2000 < 2 {
            if n2000 == 0 {
                t0 = t;
            } else {
                t1 = t;
            }
            n2000 += 1;
        } else if house == 1500 && n1500 < 1 {
            t2 = t;
            n1500 += 1;
        }
        if n2000 >= 2 && n1500 >= 1 {
            break;
        }
    }
    ctx::set_prop(T0, t0);
    ctx::set_prop(T1, t1);
    ctx::set_prop(T2, t2);
    ctx::set_prop(REMAIN, 3);
    ctx::log(
        player_id,
        &Msg::new("log.event.fix_note_on")
            .tile("a", t0)
            .tile("b", t1)
            .tile("c", t2),
    );
    Ok(())
}

/// Is `t` one of the three designated tiles?
fn is_target(t: i32) -> bool {
    t >= 0 && (t == ctx::prop(T0) || t == ctx::prop(T1) || t == ctx::prop(T2))
}

/// 规则书: 「…的加盖金额减半」 -- a one-layer cut of half the tile's build cost,
/// charged against this build (`ctx::set_build_discount`, the same surface
/// 「加盖房屋时半价」 uses).
fn before_build(_owner: i32) -> card_sdk::Asked {
    let t = trigger::tile();
    if !is_target(t) {
        return Ok(());
    }
    let half = ctx::build_cost(t) / 2;
    ctx::set_build_discount(half, 1);
    ctx::log(
        trigger::player_id(),
        &Msg::new("log.event.fix_note_half")
            .player_id("who", trigger::player_id())
            .tile("tile", t)
            .i("n", half as i64),
    );
    Ok(())
}

/// 规则书: 「累计在上述格子上加盖后将此卡移除」 -- each committed build on a
/// designated tile counts down; at zero the card leaves.
fn after_build(_owner: i32) -> card_sdk::Asked {
    let t = trigger::tile();
    if !is_target(t) {
        return Ok(());
    }
    let left = ctx::prop(REMAIN) - 1;
    ctx::set_prop(REMAIN, left);
    if left <= 0 {
        expire(ID);
        ctx::log(
            trigger::player_id(),
            &Msg::new("log.event.fix_note_off").card("event", ID),
        );
    }
    Ok(())
}