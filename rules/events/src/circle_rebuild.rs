//! `event:协助CiRCLE重建` -- 事件卡「协助CiRCLE重建」（中立事件 A8）.
//!
//! 事件文本（data/events.json, id `协助CiRCLE重建`）:
//! > 将此卡放置于场地中央，为其放置5个奇迹水晶，每次有人经过CiRCLE时移除一个，为0时放入事件弃牌，期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金（该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果），CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果

use card_sdk::abi::{prop, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep};

const ID: &str = "协助CiRCLE重建";

/// 「CiRCLE咖啡厅」 -- inherits CiRCLE's original effects while the event is up.
const CAFE: &str = "CiRCLE 咖啡厅";

pub const CIRCLE_REBUILD: CardDef = CardDef::new(
    "event:协助CiRCLE重建",
    &[
        On::Play("", None, play),
        On::Hook(&[HookKind::PassTile], "", Some(always), on_pass),
        On::Hook(&[HookKind::SettleBody], "", Some(always), on_settle_body),
    ],
);

fn always(_player_id: i32) -> bool {
    true
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
    if at < 0 {
        return Ok(());
    }
    if ctx::is_circle(at) {
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
    // CiRCLE, so the cafe's pass pays it out here.
    let cafe = ctx::tile_named(CAFE);
    if cafe >= 0 && at == cafe {
        let who = trigger::player_id();
        if who >= 0 {
            let landing = trigger::move_resolve() && trigger::move_remaining() <= 0;
            ctx::settle_circle_reward(who, landing)?;
        }
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
    if at < 0 {
        return Ok(());
    }
    let who = trigger::player_id();
    if ctx::is_circle(at) {
        if trigger::cancelled() {
            return Ok(());
        }
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
            ctx::give_stay(who, 2);
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
    // own draw-hand-plus-draw-event.
    let cafe = ctx::tile_named(CAFE);
    if cafe >= 0 && at == cafe && !trigger::cancelled() {
        trigger::set_cancelled();
        ctx::log(
            who,
            &Msg::new("log.event.rebuild_cafe").tile("tile", cafe),
        );
        ctx::draw(who, 1)?;
    }
    Ok(())
}