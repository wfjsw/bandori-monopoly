//! `event:协助CiRCLE重建` -- 事件卡「协助CiRCLE重建」（中立事件 A8）.
//!
//! 事件文本（data/events.json, id `协助CiRCLE重建`）:
//! > 将此卡放置于场地中央，为其放置5个奇迹水晶，每次有人经过CiRCLE时移除一个，为0时放入事件弃牌，期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金（该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果），CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果

use card_sdk::abi::{prop, text_slot, ActionKind};
use ruleset_core::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep};

const ID: &str = "协助CiRCLE重建";

/// 「CiRCLE咖啡厅」 -- inherits CiRCLE's original effects while the event is up.
const CAFE: &str = "CiRCLE 咖啡厅";

pub const CIRCLE_REBUILD: CardDef = CardDef::new(
    "event:协助CiRCLE重建",
    &[
        On::Play("", None, play),
        // 「每次有人经过CiRCLE时」 / 「CiRCLE的[触发结算]」 -- the two tiles this
        // event re-homes are the hooks' applicability, not their effect. The
        // body-top tile check is the condition now (`is_start(tile.id)` and
        // `tile.id == tile_named('CiRCLE 咖啡厅')`); the settle's `cancelled`
        // bail stays a residual guard (not in the CEL schema).
        On::Action(
            &[ActionKind::PassTile],
            "tile_is(tile.id, \"circle\") || tile.id == tile_named('CiRCLE 咖啡厅')",
            None,
            on_pass,
        ),
        On::Action(
            &[ActionKind::SettleBody],
            "tile_is(tile.id, \"circle\") || tile.id == tile_named('CiRCLE 咖啡厅')",
            Some(settle_applies),
            on_settle_body,
        ),
    ],
)
// Deed-popup text (`docs/TILES.md` 「Tile text」): one hook covers both tiles,
// so the text hangs off the hook entries and names its tile. While the event
// is up these replace `tile:circle` / `tile:event`'s base texts -- CiRCLE's
// settle becomes the rebuild choice, its [经过] reward is suppressed (it is
// re-homed onto the cafe), and the cafe inherits CiRCLE's original texts.
// When the event expires its instance disappears and the base texts win again.
.texts(&[
    // PassTile (entry 1) speaks for the [经过] half.
    (1, text_slot::PASS, "CiRCLE", ""),
    (1, text_slot::PASS, CAFE, "deed.note.circlePass"),
    // SettleBody (entry 2) speaks for the [结算] half.
    (2, text_slot::SETTLE, "CiRCLE", "deed.note.rebuildSettle"),
    (2, text_slot::SETTLE, CAFE, "deed.note.draw"),
]);

/// The settle not already claimed by an earlier replacement in the chain
/// (the `cancelled` bail below). The tile is the entry's condition.
fn settle_applies(_owner: i32) -> bool {
    !trigger::cancelled()
}

/// 规则书: 「将此卡放置于场地中央，为其放置5个奇迹水晶」 -- keep the event in
/// play and charge it with five crystals.
/// 规则书: 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」 --
/// 「所有效果」 includes the [经过]CiRCLE reward, so the reward is suppressed on
/// CiRCLE itself (`prop::NO_REWARD` on the `tile:circle` instance,
/// `docs/TILES.md`) and re-homed onto the cafe below.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::set_crystals(5);
    let circle = ctx::tile_named("CiRCLE");
    if circle >= 0 {
        ctx::set_tile_prop(circle, prop::NO_REWARD, 1);
    }
    ctx::log(
        player_id,
        &Msg::new("log.event.rebuild_on").card("event", ID),
    );
    Ok(())
}

/// 规则书: 「每次有人经过CiRCLE时移除一个，为0时放入事件弃牌」 -- one crystal
/// per [经过] of CiRCLE; the emptying files the event away.
/// 规则书: 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅」 -- while the rebuild is up,
/// passing CiRCLE pays no reward (the `tile:circle` Pass entry reads
/// `prop::NO_REWARD`; re-arm it here because that step consumes it), and the
/// reward is paid out on a pass of the cafe instead.
fn on_pass(_owner: i32) -> card_sdk::Asked {
    let at = trigger::tile();
    if ctx::is_start(at) {
        // Re-arm the veto the reward step just consumed, so the next pass of
        // CiRCLE is silent too.
        ctx::set_tile_prop(at, prop::NO_REWARD, 1);
        // 「移除一个」 -- `ctx::decay` burns one crystal of this instance.
        let left = ctx::decay()?;
        // 「为0时放入事件弃牌」
        if left <= 0 {
            expire(ID);
            ctx::log(
                trigger::player_id(),
                &Msg::new("log.event.rebuild_off").card("event", ID),
            );
        }
        return Ok(());
    }
    // 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」 -- the [经过]
    // reward is one of them. `tile:circle`'s own Pass entry only fires on
    // CiRCLE, so the cafe's pass runs the same reward body here
    // (`rules_tiles::circle::settle_reward`).
    let who = trigger::player_id();
    if who >= 0 {
        let landing = trigger::move_resolve() && trigger::move_remaining() <= 0;
        rules_tiles::circle::settle_reward(who, landing)?;
    }
    Ok(())
}

/// 规则书: 「期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金」
/// -- replace the CiRCLE settle body with a two-way choice.
/// 规则书: 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」
/// -- the cafe's own settle (draw a hand card, then draw an event) is overridden
/// by CiRCLE's original settle (draw one hand card).
/// TODO(规则书): 「该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果」
/// -- the engine runs hook instances in placement order, not by priority, so a
/// replacement that claims the body first still wins (the `cancelled` bail).
fn on_settle_body(_owner: i32) -> card_sdk::Asked {
    let at = trigger::tile();
    let who = trigger::player_id();
    if ctx::is_start(at) {
        trigger::set_cancelled();
        // 「选择获得2层[停留]或失去500资金」 -- the settler picks one.
        let pick = ctx::ask_pick(
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
        return Ok(());
    }
    // 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」 -- the cafe runs
    // CiRCLE's original settle (「CiRCLE的[结算]是：抽取一张手卡」) instead of its
    // own draw-hand-plus-draw-event. [`settle_applies`] already put us here.
    let cafe = ctx::tile_named(CAFE);
    trigger::set_cancelled();
    ctx::log(
        who,
        &Msg::new("log.event.rebuild_cafe").tile("tile", cafe),
    );
    ctx::draw(who, 1)?;
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn on_settle_body_kani(player_id: i32) -> card_sdk::Asked {
    on_settle_body(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(owner: i32) -> card_sdk::Asked {
    on_pass(owner)
}
