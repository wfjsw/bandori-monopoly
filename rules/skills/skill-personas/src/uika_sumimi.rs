//! `skill:三角初华（Sumimi）:成为偶像`
//!
//! 规则书（skill sheet, 三角初华（Sumimi））:
//! > （1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）
//! > （2）回合开始时可将一个火罐放置在与"主要街道"颜色相同的任一格，其他玩家经过
//! > 该格且到达移动终点后向你支付X*30资金并移除那个火罐，X为对方经过该格后移动的
//! > 剩余格数。
//!
//! （2） places a pot on a tile -- a mark, not a counter on a player -- and the
//! pay is `X*30` where X is how many steps were left *after* passing it. The
//! mark is spent when it pays out.

use bandori_std::ctx::{self, state};
use bandori_std::kind::Actions;
use bandori_std::state_key;
use card_sdk::abi::{ActionKind, MarkFilter};
use card_sdk::{key, Action, CardDef, Msg, On};

/// The mark kind 「火罐」 this skill parks on a tile.
const POT: &str = "skill.uikaIdol.pot";

pub const UIKA_SUMIMI: CardDef = CardDef::new(
    "skill:三角初华（Sumimi）:成为偶像",
    &[
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        On::Action(Action::Pass(bandori_std::guards::OWN_START, None, on_pass)),
        // （2）「回合开始时可将一个火罐放置在与"主要街道"颜色相同的任一格」 --
        // the offer exists only when such a tile exists. The colour-group
        // scan is residual (`is_color` / `tile_named`).
        On::Action(Action::TurnStartBefore(
            "actor == owner && charge(owner) >= 1",
            Some(has_street_color_tile),
            at_turn_start,
        )),
        // （2）「其他玩家经过该格且到达移动终点后向你支付X*30资金」 -- a pot on
        // the tile is the applicability (`count_marks` is not in the condition
        // vocabulary, so it is the residual guard). `actor != owner &&
        // move.remaining > 0` states the rest of the clause.
        On::Action(Action::PassTile(
            "actor != owner && move.remaining > 0",
            Some(on_pass_tile_has_pot),
            on_pass_tile,
        )),
        // 「自己被由自己以外的消耗火罐效果影响时移除等量的该标记代替[火罐]
        // 消耗」 -- a fire spend that is not this player's own skill cost (the
        // skill-cost window is `charge_is_skill`; a raw 消耗 by an outside
        // effect rides `ChargePaying` with the other source). The marker count
        // is the residual guard (`count_marks` is outside the vocabulary).
        On::Action(Actions::ChargePaying(
            "actor == owner && !charge_is_skill && value > 0",
            Some(has_any_pot),
            substitute,
        )),
    ],
)
.rule()
// 「（初始2，上限2）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 2),
    (bandori_std::fire::FIRE_CAP, 2),
]);

/// Residual guard for 「经过该格」 with a pot on it -- `count_marks` stays here
/// (not yet in the condition vocabulary).
fn on_pass_tile_has_pot(_player_id: i32) -> bool {
    ctx::count_marks(ctx::trigger::tile(), &MarkFilter::any().kind(POT)) > 0
}

/// The markers are the tile piles this rule parks; any of them enables the
/// substitution.
fn has_any_pot(_player_id: i32) -> bool {
    (0..ctx::tile_count()).any(|t| ctx::count_marks(t, &MarkFilter::any().kind(POT)) > 0)
}

/// 「获得时使自己[火罐]上限-1」 / 「移除时使自己[火罐]上限+1」 -- one cap
/// move per marker, tied to the marker's own gain and removal.
fn adjust_cap_for_marker(player_id: i32, delta: i32) {
    bandori_std::fire::adjust_cap(player_id, delta);
}

/// Remove one marker from `tile` and refund the cap it held down.
fn take_pot(player_id: i32, tile: i32) {
    ctx::bump_mark(tile, &MarkFilter::any().kind(POT), -1);
    adjust_cap_for_marker(player_id, 1);
}

/// 「移除等量的该标记」 -- take up to `n` markers off the tiles, first come.
fn take_pots(player_id: i32, mut n: i32) {
    for t in 0..ctx::tile_count() {
        while n > 0 && ctx::count_marks(t, &MarkFilter::any().kind(POT)) > 0 {
            take_pot(player_id, t);
            n -= 1;
        }
        if n == 0 {
            break;
        }
    }
}

/// 「初始2，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得1个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("uika_sumimi_gain")))?;
    Ok(())
}

/// Tiles in 主要街道's colour group -- the offer's destinations.
fn street_color_tiles(player_id: i32) -> alloc::vec::Vec<i32> {
    let street = ctx::tile_named("主要街道");
    if street < 0 {
        return alloc::vec::Vec::new();
    }
    let g = ctx::tile_group(street);
    (0..ctx::tile_count())
        .filter(|&t| ctx::is_color(player_id, t, g))
        .collect()
}

/// Residual guard for [`at_turn_start`]: at least one such tile exists.
fn has_street_color_tile(player_id: i32) -> bool {
    !street_color_tiles(player_id).is_empty()
}

/// （2）「回合开始时可将一个火罐放置在与"主要街道"颜色相同的任一格」 -- the
/// offer is the turn-start moment; the mark is placed then and pays out later.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    // `charge(owner) >= 1` + `has_street_color_tile` are the gate.
    let pool = street_color_tiles(player_id);
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("uika_sumimi_title")),
        &Msg::new(key!("uika_sumimi_ask")),
    )? {
        return Ok(());
    }
    let pick = ctx::ask_choice(
        player_id,
        &Msg::new(key!("uika_sumimi_title")),
        &Msg::new(key!("uika_sumimi_which")),
        &pool
            .iter()
            .map(|&t| Msg::new(key!("uika_sumimi_option")).tile("tile", t))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&tile) = pool.get(pick) else {
        return Ok(());
    };
    if !bandori_std::fire::spend_skill(player_id, 1, &Msg::new(key!("uika_sumimi_spend")))? {
        return Ok(());
    }
    // One pot per placement, its own row (`place_mark_new`).
    ctx::place_mark(
        tile,
        POT,
        "",
        player_id,
        ctx::self_uid(),
        1,
        &Msg::new(key!("uika_sumimi_note")),
        card_sdk::abi::Stack::Fresh,
    );
    // 「获得时使自己[火罐]上限-1」 -- the held-in-tile marker costs a slot.
    adjust_cap_for_marker(player_id, -1);
    ctx::log(
        player_id,
        &Msg::new(key!("uika_sumimi_placed")).tile("tile", tile),
    );
    Ok(())
}

/// （2）「其他玩家经过该格且到达移动终点后向你支付X*30资金并移除那个火罐，X为
/// 对方经过该格后移动的剩余格数」 -- the pay is at the *end* of the move, so the
/// remaining count is what the walk has left at the pass. `actor != owner &&
/// move.remaining > 0` is the condition; the pot-on-tile check is the residual guard.
fn on_pass_tile(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    let mover = ctx::trigger::player_id();
    let x = ctx::trigger::move_remaining().max(0);
    // 「并移除那个火罐」 + 「移除时使自己[火罐]上限+1」.
    take_pot(player_id, t);
    let due = x * 30;
    ctx::transfer(
        mover,
        player_id,
        due,
        &Msg::new(key!("uika_sumimi_reason")).i("n", due as i64),
    )?;
    ctx::log(
        player_id,
        &Msg::new(key!("uika_sumimi_paid"))
            .player_id("who", mover)
            .i("n", due as i64),
    );
    Ok(())
}

/// 「自己被由自己以外的消耗火罐效果影响时移除等量的该标记代替[火罐]消耗」
/// -- cover as much of the cost as the markers allow, then let the uncovered
/// remainder run as fire. The `ChargePaying` window is cancelled for the
/// covered part (the outer `spend` treats the whole cost as substituted); the
/// remainders are spent straight through. The markers are removed first, so
/// the re-entrant window this opens stands down (`has_any_pot` is false).
fn substitute(player_id: i32) -> card_sdk::Asked {
    let cost = ctx::trigger::value();
    if cost <= 0 {
        return Ok(());
    }
    let have: i32 = (0..ctx::tile_count())
        .map(|t| ctx::count_marks(t, &MarkFilter::any().kind(POT)))
        .sum();
    if have <= 0 {
        return Ok(());
    }
    let cover = have.min(cost);
    take_pots(player_id, cover);
    ctx::trigger::set_cancelled();
    if cost > cover {
        bandori_std::fire::spend_passive(
            player_id,
            cost - cover,
            &Msg::new(key!("uika_sumimi_substitute")).i("n", cover as i64),
        )?;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("uika_sumimi_substitute")).i("n", cover as i64),
    );
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
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

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    on_pass_tile(player_id)
}
