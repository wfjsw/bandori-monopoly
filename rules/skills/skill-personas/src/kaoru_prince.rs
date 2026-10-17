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
//! 「[怪盗标记]」 anywhere on the board? The mark is planted 「向一名玩家场上」
//! (`HHW:（薰）怪盗hello happy` uses `add_tok` on the chosen player), so the
//! question is a per-seat held-counter scan and the two branches are a pot or
//! a mark removal.
//!
//! （2） 「以微笑号为起点」 is the plan's start; 「在…玩家场上放置3个[怪盗标记]」
//! puts three marks on that player's field (held counters), the same plant the
//! printed card uses.
//!
//! （3） 「不会经过…所拥有的格子」 is a walk that skips another player's deeds
//! when they hold a mark -- the plan's route filter.

use card_sdk::abi::{state_key, ActionKind};
use ruleset_core::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「[怪盗标记]」 -- a held counter on the marked player
/// (「向一名玩家场上放置一个怪盗标记」).
const THIEF: &str = "怪盗标记";
/// Wire name `HHW:（薰）怪盗hello happy` stamps via `key!("kaoru_thief_tok")`.
/// Both names name the same mark; accept either so a plant from the printed
/// card and a plant from this skill are interchangeable.
const THIEF_WIRE: &str = "cards:card-hhw.kaoru_thief_tok";

pub const KAORU_PRINCE: CardDef = CardDef::new(
    "skill:濑田薰:梦幻的王子殿下",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Action(&[ActionKind::TurnStartBefore], "", None, declare_cap),
        // （1） 「每次[经过]…时」 -- "I pass" is `actor == owner` (`MINE`); an
        // unfiltered `Pass` fires for every passer's every step. The two halves
        // of 「[经过]或被[经过]」 never both fire on one step: `Pass` here needs
        // the owner to be the passer, and `PassTile` below needs someone else
        // to step onto the owner's tile.
        On::Action(&[ActionKind::Pass], card_sdk::pre::MINE, None, on_pass),
        // （1） 「…或被[经过]时」 -- 行动阶段 12 [经过] (`SETTLE-STAGES.md` §4
        // M4), the passer's step onto this player's tile -- not the end-tile
        // [重叠]. `target` is not on a `passTile` payload, so the condition
        // reads the tile being entered.
        On::Action(
            &[ActionKind::PassTile],
            "actor != owner && tile.id == owner.pos",
            None,
            on_passed,
        ),
    ],
)
.labels(&[(0, "（2）"), (2, "（1）"), (3, "（1）")])
.silent(&[1])
.quiet_when_empty(&[2, 3]);

/// 「上限7」 -- the clause gives no initial, so it starts empty.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    state::set_bounds(player_id, state_key::CHARGE, 0, 7);
    Ok(())
}

/// 「被[经过]」 -- another player's step onto **my** tile (行动阶段 12,
/// `SETTLE-STAGES.md` §4 M4). `actor != owner && tile.id == owner.pos` is the pre.

/// （1） 「每次[经过]…时」 -- the owner's own step. `actor == owner` is the pre.
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
/// 一个[怪盗标记]」. The mark is a held counter on a player (「…玩家场上」),
/// the same plant `HHW:（薰）怪盗hello happy` makes.
fn settle(player_id: i32) -> card_sdk::Asked {
    for p in 0..ctx::player_count() {
        for name in [THIEF, THIEF_WIRE] {
            if ctx::tok(p, name) > 0 {
                // 「移除场上的一个[怪盗标记]」 -- one unit off that player's mark.
                ctx::add_tok(p, name, -1, i32::MAX)?;
                ctx::log(
                    player_id,
                    &Msg::new(key!("kaoru_prince_removed")).player_id("who", p),
                );
                return Ok(());
            }
        }
    }
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("kaoru_prince_gain")))?;
    Ok(())
}

/// （2） 「主要阶段可消耗7火罐」.
fn can_use(player_id: i32) -> Option<Msg> {
    if ruleset_core::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::CHARGE) < 7 {
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
    if !rules_status::fire::spend(player_id, 7, &Msg::new(key!("kaoru_prince_spend")))? {
        return Ok(());
    }
    // 「在…玩家场上放置3个[怪盗标记]」 -- three units on that player's field
    // (held counters, the same plant the printed card makes). Three units, not
    // three tile rows: 「玩家场上」 is the player's field.
    for _ in 0..3 {
        ctx::add_tok(who, THIEF, 1, i32::MAX)?;
    }
    // 「使你的本次移动以微笑号为起点」
    let ship = ctx::tile_named("微笑号");
    if ship >= 0 {
        ctx::plan::set_start(ship, "梦幻的王子殿下");
    }
    ctx::log(
        player_id,
        &Msg::new(key!("kaoru_prince_done")).player_id("who", who),
    );
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn on_passed_kani(player_id: i32) -> card_sdk::Asked {
    on_passed(player_id)
}
