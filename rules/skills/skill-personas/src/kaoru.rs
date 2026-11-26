//! `skill:濑田薰:梦幻的王子殿下`
//!
//! 规则书（skill sheet, 濑田薰）:
//! > [资源]：
//! > （1）[火罐]：初始0，上限7。
//! > [自动]：
//! > （1）自己每次[经过]或被[经过]时：若场上不存在[怪盗标记]，则获得1个[火罐]；
//! > 若场上存在[怪盗标记]，则移除场上的1个[怪盗标记]。
//! > （2）自己非[传送]的[主要移动]不会[经过]移动阶段开始时场上拥有[怪盗标记]的
//! > 玩家所拥有的格子。
//! > [主动]：
//! > （1）自己的运营阶段：可使用7个[火罐]，依次进行以下操作：
//! > 1. 在与自己[绝对距离]最近的1名玩家场上放置3个[怪盗标记]。
//! > 2. 使此次移动以“微笑号”为起点。
//!
//! [自动]（1） is one clause with two branches on the same question -- is there
//! a 「[怪盗标记]」 anywhere on the board? The mark is planted 「向一名玩家场上」
//! (`HHW:（薰）怪盗hello happy` uses `add_held_by_name` on the chosen player), so the
//! question is a per-seat held-counter scan and the two branches are a pot or
//! a mark removal.
//!
//! [主动]（1） 「以微笑号为起点」 is the plan's start; 「在…玩家场上放置3个
//! [怪盗标记]」 puts three marks on that player's field (held counters), the same
//! plant the printed card uses.
//!
//! [自动]（2） 「自己非[传送]的[主要移动]不会[经过]移动阶段开始时场上拥有
//! [怪盗标记]的玩家所拥有的格子」 -- the holders are **snapshotted at plan time**
//! (the movement-phase window, `DiceRollPlan`), and every walk step onto a deed
//! of a snapshotted seat is skipped by extending the walk one step. No
//! per-tile route atom exists (`Filter::WalkPlan` is not in this SDK), so the
//! nearest sanctioned residual is the plan window for the snapshot plus a
//! `PassTile` extension; the skipped tile is still stepped over (its `[经过]`
//! effects fire) but it is not the landing.

use bandori_std::ctx::{self, plan, state};
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

/// The mark kind 「[怪盗标记]」 -- a held counter on the marked player
/// (「向一名玩家场上放置一个怪盗标记」).
///
/// [自动]（2） snapshots the mark-holding seats here, as a seat bitmask.
const BLOCKED: &str = "skill.kaoruPrince.marked";
const THIEF: &str = "怪盗标记";
/// Wire name `HHW:（薰）怪盗hello happy` stamps via `key!("kaoru_thief_tok")`.
/// Both names name the same mark; accept either so a plant from the printed
/// card and a plant from this skill are interchangeable.
const THIEF_WIRE: &str = "cards:card-hhw.kaoru_thief_tok";

pub const KAORU: CardDef = CardDef::new(
    "skill:濑田薰:梦幻的王子殿下",
    &[
        // [主动]（1） -- the skill press itself.
        On::Play("", Some(can_use), use_skill).label("[主动]（1）"),
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        // [自动]（1） 「每次[经过]…时」 -- "I pass" is `actor == owner` (`MINE`); an
        // unfiltered `Pass` fires for every passer's every step, so this half
        // is silent bookkeeping (the pot / mark state is the trace). The two
        // halves of 「[经过]或被[经过]」 never both fire on one step: `Pass`
        // here needs the owner to be the passer, and `PassTile` below needs
        // someone else to step onto the owner's tile.
        On::Action(Action::Pass(card_sdk::condition::MINE, None, on_pass))
            .label("[自动]（1）")
            .silent(),
        // [自动]（1） 「…或被[经过]时」 -- 行动阶段 12 [经过] (`SETTLE-STAGES.md` §4
        // M4), the passer's step onto this player's tile -- not the end-tile
        // [重叠]. `target` is not on a `passTile` payload, so the condition
        // reads the tile being entered.
        On::Action(Action::PassTile(
            "actor != owner && tile.id == owner.pos",
            None,
            on_passed,
        ))
        .label("[自动]（1）")
        .quiet_when_empty(),
        // [自动]（2） -- the snapshot at the move's `moveBefore`, before the
        // first step. A `DiceRollPlan` filter body cannot persist a state write
        // (filter bodies are pure), so the nearest sanctioned window is the
        // move hook right after the plan is fixed; the walk steps below re-check
        // the kind (a teleport main move walks nothing).
        On::Action(Action::MoveBefore(
            "actor == owner && move.is_main_move && move.kind == Walk",
            None,
            snapshot_marks,
        ))
        .silent(),
        // [自动]（2）「不会[经过]…格子」 -- a step onto a snapshotted seat's deed
        // extends the walk by one, so the walk continues past it.
        On::Action(Action::PassTile(
            "actor == owner && move.is_main_move && move.kind == Walk",
            Some(blocked_tile),
            skip_tile,
        ))
        .label("[自动]（2）"),
    ],
)
.rule()
// 「（上限7）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 0),
    (bandori_std::fire::FIRE_CAP, 7),
]);

/// 「上限7」 -- the clause gives no initial, so it starts empty.
/// The numbers ride this face's own `CardDef` (`.props(&[(bandori_std::fire::FIRE_INITIAL, 0), (bandori_std::fire::FIRE_CAP, 7)])` above) so
/// the parenthetical has one source of truth.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// 「被[经过]」 -- another player's step onto **my** tile (行动阶段 12,
/// `SETTLE-STAGES.md` §4 M4). `actor != owner && tile.id == owner.pos` is the condition.

/// [自动]（1） 「每次[经过]…时」 -- the owner's own step. `actor == owner` is the condition.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    settle(player_id)?;
    Ok(())
}

/// [自动]（1） 「…或被[经过]时」 -- someone passed this player.
fn on_passed(player_id: i32) -> card_sdk::Asked {
    settle(player_id)?;
    Ok(())
}

/// [自动]（1） 「若场上不存在[怪盗标记]，获得1火罐；若场上存在[怪盗标记]，则移除场上的
/// 一个[怪盗标记]」. The mark is a held counter on a player (「…玩家场上」),
/// the same plant `HHW:（薰）怪盗hello happy` makes.
fn settle(player_id: i32) -> card_sdk::Asked {
    for p in 0..ctx::player_count() {
        for name in [THIEF, THIEF_WIRE] {
            if ctx::held_count(p, name) > 0 {
                // 「移除场上的一个[怪盗标记]」 -- one unit off that player's mark.
                // Silent entry, so no line of its own.
                ctx::add_held_by_name(p, name, -1, i32::MAX)?;
                return Ok(());
            }
        }
    }
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("kaoru_gain")))?;
    Ok(())
}

/// [主动]（1） 「主要阶段可消耗7火罐」.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if bandori_std::fire::get(player_id) < 7 {
        return Some(Msg::new(key!("kaoru_no_fire")));
    }
    None
}

/// [主动]（1）「在绝对距离最近的一名玩家场上放置3个[怪盗标记]，并使你的本次移动以微笑号
/// 为起点」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    // 「绝对距离最近的一名玩家」 -- [`skill_candidates`] answers the whole tied
    // set in seat order, plus any seat a bond copy exempts (「无视【距离最近】」
    // applies to skills), so the existing enumeration below is unchanged.
    //
    // [`skill_candidates`]: bandori_std::nearest::skill_candidates
    let near = bandori_std::nearest::skill_candidates(player_id, ctx::DistanceKind::Shortest);
    if near.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_choice(
        player_id,
        &Msg::new(key!("kaoru_title")),
        &Msg::new(key!("kaoru_ask")),
        &near
            .iter()
            .map(|&p| Msg::new(key!("kaoru_option")).player_id("who", p))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&who) = near.get(pick) else {
        return Ok(());
    };
    if !bandori_std::fire::spend_skill(player_id, 7, &Msg::new(key!("kaoru_spend")))? {
        return Ok(());
    }
    // 「在…玩家场上放置3个[怪盗标记]」 -- three units on that player's field
    // (held counters, the same plant the printed card makes). Three units, not
    // three tile rows: 「玩家场上」 is the player's field.
    for _ in 0..3 {
        ctx::add_held_by_name(who, THIEF, 1, i32::MAX)?;
    }
    // 「使你的本次移动以微笑号为起点」
    let ship = ctx::tile_named("微笑号");
    if ship >= 0 {
        ctx::plan::set_start(ship, "梦幻的王子殿下");
    }
    ctx::log(
        player_id,
        &Msg::new(key!("kaoru_done")).player_id("who", who),
    );
    Ok(())
}

/// [自动]（2） the movement-phase snapshot: pack the seats that hold a mark
/// into [`BLOCKED`]. Runs at the plan window, so a mark planted or removed later
/// in the move does not change this move's route (「移动阶段开始时」).
fn snapshot_marks(player_id: i32) -> card_sdk::Asked {
    let mut mask = 0i32;
    for p in 0..ctx::player_count() {
        if [THIEF, THIEF_WIRE]
            .iter()
            .any(|name| ctx::held_count(p, name) > 0)
        {
            mask |= 1 << p;
        }
    }
    state::set(player_id, BLOCKED, mask);
    Ok(())
}

/// Residual guard for [`skip_tile`]: the entered tile is owned by one of the
/// snapshotted seats. Deeds of unmarked players stay walkable.
fn blocked_tile(player_id: i32) -> bool {
    let owner = ctx::tile_owner(ctx::trigger::tile());
    owner >= 0 && (state::get(player_id, BLOCKED) >> owner) & 1 != 0
}

/// [自动]（2）「不会[经过]…格子」 -- one more step, so this tile is passed over
/// rather than landed on.
fn skip_tile(player_id: i32) -> card_sdk::Asked {
    plan::add_extra_steps(1);
    Ok(())
}

//

//

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
pub fn snapshot_marks_kani(player_id: i32) -> card_sdk::Asked {
    snapshot_marks(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn skip_tile_kani(player_id: i32) -> card_sdk::Asked {
    skip_tile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_passed_kani(player_id: i32) -> card_sdk::Asked {
    on_passed(player_id)
}
