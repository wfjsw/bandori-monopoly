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
use ruleset_core::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind 「火罐」 this skill parks on a tile.
const POT: &str = "skill.uikaIdol.pot";

pub const UIKA_IDOL: CardDef = CardDef::new(
    "skill:三角初华（Sumimi）:成为偶像",
    &[
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::Pass], "actor == owner && is_circle(tile.id)", None, on_pass),
        // （2）「回合开始时可将一个火罐放置在与"主要街道"颜色相同的任一格」 --
        // the offer exists only when such a tile exists. The colour-group
        // scan is residual (`is_color` / `tile_named`).
        On::Hook(
            &[HookKind::TurnStartBefore],
            "actor == owner && fire(owner) >= 1",
            Some(has_street_color_tile),
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
    .silent(&[0]);

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
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("uika_idol_gain")))?;
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
    // `fire(owner) >= 1` + `has_street_color_tile` are the gate.
    let pool = street_color_tiles(player_id);
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
    if !rules_status::fire::spend(player_id, 1, &Msg::new(key!("uika_idol_spend")))? {
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

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: ruleset_core::roster::CharacterDef =
ruleset_core::roster::CharacterDef::new(r#"三角初华（Sumimi）"#)
    .display(r#"三角初华（Sumimi）"#)
    .band(r#"Sumimi"#)
    .color(r#"#BB9955"#)
    .skill(r#"成为偶像"#)
    .text(r#"（1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）（2）回合开始时可将一个火罐放置在与“主要街道”颜色相同的任一格，其他玩家经过该格且到达移动终点后向你支付X*30资金并移除那个火罐，X为对方经过该格后移动的剩余格数。"#)
    .cn_id("")
    .art(r#"337s"#)
    .costume(r#"337_sumimi"#)
    .exclusive_cards(&[r#"儿时玩伴的鼓励"#])
    .preset(&[])
    .variant_of(r#"三角初华"#)
    .variant_tag("");

