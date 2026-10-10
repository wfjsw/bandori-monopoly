//! `skill:濑田薰:梦幻的王子殿下`
//!
//! 规则书（skill sheet, 濑田薰）:
//! > （1）每次[经过]或被[经过]时，若场上不存在[怪盗标记]，获得1火罐（上限7）；
//! > 若场上存在[怪盗标记]，则移除场上的一个[怪盗标记]
//! > （2）主要阶段可消耗7火罐在绝对距离最近的一名玩家场上放置3个[怪盗标记]，并使
//! > 你的本次移动以微笑号为起点。
//! > （3）你非传送的主要移动不会经过场上拥有[怪盗标记]的玩家所拥有的格子。
//!
//! （1） is one clause with two branches on the same question -- is there a
//! 「[怪盗标记]」 anywhere on the board? The mark is a tile mark, so the
//! question is a `count_marks` over the board and the two branches are a pot or
//! a mark removal.
//!
//! （2） 「以微笑号为起点」 is the plan's start; 「在…玩家场上放置3个[怪盗标记]」
//! puts three marks on one of that player's tiles.
//!
//! （3） 「不会经过…所拥有的格子」 is a walk that skips another player's deeds
//! when they hold a mark -- the plan's route filter.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「[怪盗标记]」.
const THIEF: &str = "怪盗标记";

pub const KAORU_PRINCE: CardDef = CardDef::new(
    "skill:濑田薰:梦幻的王子殿下",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Hook(&[HookKind::TurnStartBefore], "", None, declare_cap),
        // TODO(规则书): 「每次[经过]或被[经过]时」 -- the `Pass` half is "I pass",
        // which should filter `actor == owner`. The `""` pre currently fires for
        // every passer (a suspected bug). Not filtered here: adding the actor
        // clause would change behaviour for other-actor passes.
        On::Hook(&[HookKind::Pass], "", None, on_pass),
        // （1） 「…或被[经过]时」 -- 行动阶段 12 [经过] (`SETTLE-STAGES.md` §4
        // M4), the passer's step onto this player's tile -- not the end-tile
        // [重叠]. `target` is not on a `passTile` payload, so the condition
        // reads the tile being entered.
        On::Hook(
            &[HookKind::PassTile],
            "actor != owner && tile.id == owner.pos",
            None,
            on_passed,
        ),
    ],
);

/// 「上限7」 -- the clause gives no initial, so it starts empty.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    state::set_bounds(player_id, state_key::FIRE, 0, 7);
    Ok(())
}

/// 「被[经过]」 -- another player's step onto **my** tile (行动阶段 12,
/// `SETTLE-STAGES.md` §4 M4). `actor != owner && tile.id == owner.pos` is the pre.

/// （1） 「每次[经过]…时」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    settle(player_id)?;
    Ok(())
}

/// （1） 「…或被[经过]时」 -- someone passed this player.
fn on_passed(player_id: i32) -> card_sdk::Asked {
    settle(player_id)?;
    Ok(())
}

/// （1） 「若场上不存在[怪盗标记]，获得1火罐；若场上存在[怪盗标记]，则移除场上的
/// 一个[怪盗标记]」.
fn settle(player_id: i32) -> card_sdk::Asked {
    for t in 0..ctx::tile_count() {
        if ctx::count_marks(t, THIEF, -2) > 0 {
            // 「移除场上的一个[怪盗标记]」 -- one tick off a single mark.
            ctx::bump_mark(t, THIEF, -2, -1);
            ctx::log(
                player_id,
                &Msg::new(key!("kaoru_prince_removed")).tile("tile", t),
            );
            return Ok(());
        }
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("kaoru_prince_gain")))?;
    Ok(())
}

/// （2） 「主要阶段可消耗7火罐」.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 7 {
        return Some(Msg::new(key!("kaoru_prince_no_fire")));
    }
    None
}

/// （2）「在绝对距离最近的一名玩家场上放置3个[怪盗标记]，并使你的本次移动以微笑号
/// 为起点」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let at = ctx::player_pos(player_id);
    let mut best = i32::MAX;
    let mut near: alloc::vec::Vec<i32> = alloc::vec::Vec::new();
    for p in 0..ctx::player_count() {
        if p == player_id || ctx::player_out(p) {
            continue;
        }
        let d = ctx::dist(at, ctx::player_pos(p));
        if d < best {
            best = d;
            near.clear();
            near.push(p);
        } else if d == best {
            near.push(p);
        }
    }
    if near.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("kaoru_prince_title")),
        &Msg::new(key!("kaoru_prince_ask")),
        &near
            .iter()
            .map(|&p| Msg::new(key!("kaoru_prince_option")).player_id("who", p))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&who) = near.get(pick) else {
        return Ok(());
    };
    if !ctx::spend_fire(player_id, 7, &Msg::new(key!("kaoru_prince_spend")))? {
        return Ok(());
    }
    // 「在…玩家场上放置3个[怪盗标记]」 -- on one of that player's tiles.
    let theirs = ctx::owned_tiles(who);
    let Some(&tile) = theirs.first() else {
        return Ok(());
    };
    for _ in 0..3 {
        ctx::add_mark(tile, player_id, THIEF, &Msg::new(key!("kaoru_prince_note")));
    }
    // 「使你的本次移动以微笑号为起点」
    let ship = ctx::tile_named("微笑号");
    if ship >= 0 {
        ctx::plan::set_start(ship, "梦幻的王子殿下");
    }
    ctx::log(
        player_id,
        &Msg::new(key!("kaoru_prince_done"))
            .player_id("who", who)
            .tile("tile", tile),
    );
    Ok(())
}
