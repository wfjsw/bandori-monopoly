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

use card_sdk::abi::{state_key, HookKind, MarkFilter};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「火罐」 this skill parks on a tile.
const POT: &str = "skill.uikaIdol.pot";

pub const UIKA_IDOL: CardDef = CardDef::new(
    "skill:三角初华（Sumimi）:成为偶像",
    &[
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::Pass], "actor == owner && is_circle(tile.id)", None, on_pass),
        On::Hook(
            &[HookKind::TurnStartBefore],
            "actor == owner && fire(owner) >= 1",
            None,
            at_turn_start,
        ),
        // （2）「其他玩家经过该格且到达移动终点后向你支付X*30资金」 -- a pot on
        // the tile is the applicability (`count_marks` is not in the condition
        // vocabulary, so it is the residual guard). `actor != owner &&
        // move.remaining > 0` states the rest of the clause.
        On::Hook(
            &[HookKind::PassTile],
            "actor != owner && move.remaining > 0",
            Some(on_pass_tile_has_pot),
            on_pass_tile,
        ),
    ],
)
    .legacy(&[(1, legacy_mine), (2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// Residual guard for 「经过该格」 with a pot on it -- `count_marks` stays here
/// (not yet in the condition vocabulary).
fn on_pass_tile_has_pot(_player_id: i32) -> bool {
    ctx::count_marks(ctx::trigger::tile(), &MarkFilter::any().kind(POT)) > 0
}

/// 「初始2，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 2, 2);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得1个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    ctx::gain_fire(player_id, 1, &Msg::new(key!("uika_idol_gain")))?;
    Ok(())
}

/// （2）「回合开始时可将一个火罐放置在与"主要街道"颜色相同的任一格」 -- the
/// offer is the turn-start moment; the mark is placed then and pays out later.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    // `fire(owner) >= 1` is the pre (「可将一个火罐放置在…」).
    let street = ctx::tile_named("主要街道");
    if street < 0 {
        return Ok(());
    }
    let g = ctx::tile_group(street);
    let pool: alloc::vec::Vec<i32> = (0..ctx::tile_count())
        .filter(|&t| ctx::is_color(player_id, t, g))
        .collect();
    if pool.is_empty() {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("uika_idol_title")),
        &Msg::new(key!("uika_idol_ask")),
    )? {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("uika_idol_title")),
        &Msg::new(key!("uika_idol_which")),
        &pool
            .iter()
            .map(|&t| Msg::new(key!("uika_idol_option")).tile("tile", t))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&tile) = pool.get(pick) else {
        return Ok(());
    };
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("uika_idol_spend")))? {
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
        &Msg::new(key!("uika_idol_note")), card_sdk::abi::Stack::Fresh);
    ctx::log(
        player_id,
        &Msg::new(key!("uika_idol_placed")).tile("tile", tile),
    );
    Ok(())
}

/// （2）「其他玩家经过该格且到达移动终点后向你支付X*30资金并移除那个火罐，X为
/// 对方经过该格后移动的剩余格数」 -- the pay is at the *end* of the move, so the
/// remaining count is what the walk has left at the pass. `actor != owner &&
/// move.remaining > 0` is the pre; the pot-on-tile check is the residual guard.
fn on_pass_tile(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    let mover = ctx::trigger::player_id();
    let x = ctx::trigger::move_remaining().max(0);
    ctx::bump_mark(t, &MarkFilter::any().kind(POT), -1);
    let due = x * 30;
    ctx::transfer(
        mover,
        player_id,
        due,
        &Msg::new(key!("uika_idol_why")).i("n", due as i64),
    )?;
    ctx::log(
        player_id,
        &Msg::new(key!("uika_idol_paid"))
            .player_id("who", mover)
            .i("n", due as i64),
    );
    Ok(())
}
