//! `event:协助CiRCLE重建` -- 事件卡「协助CiRCLE重建」（中立事件 A8）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `协助CiRCLE重建`）:
//! > 将此卡放置于场地中央，为其放置5个奇迹水晶，每次有人经过CiRCLE时移除一个，为0时放入事件弃牌，期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金（该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果），CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{prop, text_slot, ActionKind, Message, Target};
use card_sdk::{Action, CardDef, Msg, On};

use crate::util::{expire, keep};

const ID: &str = "协助CiRCLE重建";

/// 「CiRCLE咖啡厅」 -- inherits CiRCLE's original effects while the event is up.
const CAFE: &str = "CiRCLE 咖啡厅";

/// `tile:circle` -- the rule whose [经过] reward 「迁移至CiRCLE咖啡厅」. Named
/// here only to address the cross-card message; this crate does not depend on
/// `rules-tiles`.
const CIRCLE_RULE: &str = "tile:circle";

pub const CIRCLE_REBUILD: CardDef = CardDef::new(
    "event:协助CiRCLE重建",
    &[
        On::Play("", None, None, play),
        // 「每次有人经过CiRCLE时」 / 「CiRCLE的[触发结算]」 -- the two tiles this
        // event re-homes are the hooks' applicability, not their effect. The
        // body-top tile check is the condition now (`is_start(tile.id)` and
        // `tile.id == tile_named('CiRCLE 咖啡厅')`); the settle's `cancelled`
        // bail stays a residual guard (not in the CEL schema).
        On::Action(Action::PassTile("tile_is(tile.id, \"circle\") || tile.id == tile_named('CiRCLE 咖啡厅')", None, None, on_pass)),
        On::Action(Action::SettleBody("tile_is(tile.id, \"circle\") || tile.id == tile_named('CiRCLE 咖啡厅')", None, Some(settle_applies), on_settle_body)),
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
])
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"协助CiRCLE重建"#)
            .text(r#"将此卡放置于场地中央，为其放置5个奇迹水晶，每次有人经过CiRCLE时移除一个，为0时放入事件弃牌，期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金（该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果），CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果"#)
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
        // 「移除一个」 -- `ctx::decay` burns one crystal of this instance.
        let left = ctx::decay()?;
        // 「为0时放入事件弃牌」
        if left <= 0 {
            expire(ID);
            // Deliberately **not** re-arming `prop::NO_REWARD`. The veto lives
            // on the `tile:circle` instance -- the source owns the arming, the
            // reader is the tile -- so this instance dying does not take it
            // with it, and a surviving arm would silently eat the next pass's
            // reward. This pass is still suppressed: the reward step either
            // already consumed the armed veto (it is collected first) or will
            // consume it now (this handler is).
            ctx::log(
                trigger::player_id(),
                &Msg::new("log.event.rebuild_off").card("event", ID),
            );
            return Ok(());
        }
        // Re-arm the veto the reward step just consumed, so the next pass of
        // CiRCLE is silent too.
        ctx::set_tile_prop(at, prop::NO_REWARD, 1);
        return Ok(());
    }
    // 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」 -- the [经过]
    // reward is one of them. `tile:circle`'s own Pass entry only fires on
    // CiRCLE, so the cafe's pass asks that card for the reward: the generic
    // cross-card channel (`ctx::send` -> `On::Message("settleReward")`), and
    // the body runs as `tile:circle` with its own windows. Not a call into
    // another module -- see `docs/CARDS.md`.
    let who = trigger::player_id();
    // TODO(ABI) tile swap: the re-home is still hand-rolled -- this handler
    // stands in for 「CiRCLE原本的所有效果」 one clause at a time. A real
    // temporary tile-rule swap would carry every effect, and any instance
    // state bound to `tile:circle`, across in one move, applied before all
    // hooks that change settlement. Filed as ABI-13.
    if who >= 0 {
        let landing = trigger::move_resolve() && trigger::move_remaining() <= 0;
        ctx::send(
            &Target::Board(CIRCLE_RULE.into()),
            "settleReward",
            &Message {
                name: "settleReward".into(),
                seat: who,
                b: landing as i32,
                tile: at,
                ..Default::default()
            },
        )?;
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
/// TODO(ABI) ABI-13: rather than replacing the settle body by hand, swap the
/// CiRCLE square's tile-rule for one that implements this choice. The swap has
/// to keep the square's **identity** (`tile_is(.., "circle")` / `is_start` /
/// `guards::OWN_START`) -- that is what charge replenish keys on
/// (`skill-personas`: 火罐 on 「[经过]CiRCLE」), and those effects are their
/// own cards', not CiRCLE's, so they must stay put and must not ride the
/// migration to 「CiRCLE咖啡厅」. See BACKLOG.md ABI-13.
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
    ctx::log(who, &Msg::new("log.event.rebuild_cafe").tile("tile", cafe));
    ctx::draw(who, 1)?;
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
