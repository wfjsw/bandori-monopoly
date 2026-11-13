//! `event:协助CiRCLE重建` -- 事件卡「协助CiRCLE重建」（中立事件 A8）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `协助CiRCLE重建`）:
//! > 将此卡放置于场地中央，为其放置5个奇迹水晶，每次有人经过CiRCLE时移除一个，为0时放入事件弃牌，期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金（该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果），CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{text_slot, ActionKind};
use card_sdk::CardText;
use card_sdk::{Action, CardDef, Msg, On};

use crate::util::{expire, keep};

const ID: &str = "协助CiRCLE重建";

/// 「CiRCLE咖啡厅」 -- inherits CiRCLE's original effects while the event is up.
const CAFE: &str = "CiRCLE 咖啡厅";

pub const CIRCLE_REBUILD: CardDef = CardDef::new(
    "event:协助CiRCLE重建",
    &[
        On::Play("", None, play),
        // 「每次有人经过CiRCLE时」 / 「CiRCLE的[触发结算]」 -- both hooks stay on
        // the physical CiRCLE square. What changed there is the settle body, so
        // that half needs the replacement; the [经过] half only counts crystals
        // and the migrated `tile:circle` Pass entry pays the reward on the cafe.
        // The settle's `cancelled` bail stays a residual guard (not in the CEL
        // schema).
        // Deed-popup text (`docs/TILES.md` 「Tile text」): these replace
        // `tile:circle`'s two texts while the event is up -- the settle becomes
        // the rebuild choice and the [经过] reward is suppressed on CiRCLE (it
        // lives on the cafe). The cafe shows the swapped-in `tile:circle`
        // instance's own texts, which the rule swap re-homes onto it. When the
        // event expires its instance disappears and the base texts win again.
        On::Action(Action::PassTile("tile_is(tile.id, \"circle\")", None, on_pass))
            .texts(&[(text_slot::PASS, "CiRCLE", "")]),
        // SettleBody speaks for the [结算] half. Only CiRCLE: on the cafe the
        // swapped-in `tile:circle` body (CiRCLE's original settle) runs.
        On::Action(Action::SettleBody("tile_is(tile.id, \"circle\")", Some(settle_applies), on_settle_body))
            .texts(&[(text_slot::SETTLE, "CiRCLE", "deed.note.rebuildSettle")]),
    ],
).rule()
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"协助CiRCLE重建"#)
            .text(CardText::para(r#"将此卡放置于场地中央，为其放置5个奇迹水晶，每次有人经过CiRCLE时移除一个，为0时放入事件弃牌，期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金（该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果），CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果"#))
            // sheet cell A8
            .derived(false));

/// The settle not already claimed by an earlier replacement in the chain
/// (the `cancelled` bail below). The tile is the entry's condition.
fn settle_applies(_owner: i32) -> bool {
    !trigger::cancelled()
}

/// 规则书: 「将此卡放置于场地中央，为其放置5个奇迹水晶」 -- keep the event in
/// play and charge it with five crystals.
/// 规则书: 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」 --
/// one rule swap carries 「所有效果」, and every piece of instance state bound
/// to `tile:circle` (its `prop::NO_REWARD` arms included), across in one
/// world write: the `tile:circle` rule instance governs the cafe and the
/// cafe's `tile:event` instance governs CiRCLE. The swap is a prop pair on
/// this event instance, so it is in force from here to the event's expiry and
/// rides save / restore with the row (`docs/rulebook/spec/abi/
/// ab-tile-swap.md`). The squares' identity does not move: `tile_is(..,
/// "circle")`, `ctx::is_start` and `prop::START` keep reading the board, so
/// cards keyed *on* CiRCLE (the charge replenish) stay put.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::set_crystals(5);
    let circle = ctx::tile_named("CiRCLE");
    let cafe = ctx::tile_named(CAFE);
    if circle >= 0 && cafe >= 0 {
        ctx::tile_swap_rules(circle, cafe);
    }
    ctx::log(
        player_id,
        &Msg::new("log.event.rebuild_on").card("event", ID),
    );
    Ok(())
}

/// 规则书: 「每次有人经过CiRCLE时移除一个，为0时放入事件弃牌」 -- one crystal
/// per [经过] of CiRCLE; the emptying files the event away.
///
/// The [经过] reward needs no work here: the swap already moved
/// `tile:circle`'s Pass entry onto the cafe, so passing CiRCLE pays nothing
/// and passing the cafe pays the reward. Expiry on this pass reverses the
/// swap; the pass already ran on the board list collected while the event was
/// up, so it is not paid twice.
fn on_pass(_owner: i32) -> card_sdk::Asked {
    // 「移除一个」 -- `ctx::decay` burns one crystal of this instance.
    let left = ctx::remove_crystal()?;
    // 「为0时放入事件弃牌」
    if left <= 0 {
        expire(ID);
        ctx::log(
            trigger::player_id(),
            &Msg::new("log.event.rebuild_off").card("event", ID),
        );
    }
    Ok(())
}

/// 规则书: 「期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金」
/// -- replace the CiRCLE settle body with a two-way choice.
/// 规则书: 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」
/// -- the cafe runs CiRCLE's original settle (「CiRCLE的[结算]是：抽取一张手卡」)
/// because the swap moved the whole `tile:circle` body there; it needs no
/// branch here. The cafe's own settle (draw a hand card, then draw an event)
/// is overridden because its `tile:event` instance is swapped away onto
/// CiRCLE, where this hook cancels it.
/// TODO(规则书): 「该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果」
/// -- the engine runs hook instances in placement order, not by priority, so a
/// replacement that claims the body first still wins (the `cancelled` bail).
fn on_settle_body(_owner: i32) -> card_sdk::Asked {
    let who = trigger::player_id();
    trigger::set_cancelled();
    // 「选择获得2层[停留]或失去500资金」 -- the settler picks one.
    let pick = ctx::ask_choice(
        who,
        &Msg::new("log.event.rebuild_title"),
        &Msg::new("log.event.rebuild_ask"),
        &[
            Msg::new("log.event.rebuild_stay"),
            Msg::new("log.event.rebuild_pay"),
        ],
    )?;
    if pick == 0 {
        // 「获得2层[停留]」
        rules_status::stay::add(who, 2);
        ctx::log(
            who,
            &Msg::new("log.event.rebuild_stay").player_id("who", who),
        );
    } else {
        // 「失去500资金」
        ctx::pay(who, 500, &Msg::new("log.event.rebuild_pay"))?;
    }
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_settle_body_kani(player_id: i32) -> card_sdk::Asked {
    on_settle_body(player_id)
}
