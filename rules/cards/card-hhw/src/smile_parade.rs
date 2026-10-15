//! `HHW:笑容大游行` -- C# `CardSmileParade` (MatchHost.cs:3597-3717): [反击] onto
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:笑容大游行`）:
//! > 笑容大游行：
//! > （1）[反击]经过“弦卷集团”（#29格）时可将此卡放置在其上，当次移动的移动终点视为“弦卷集团”地产商；为此卡添加3个[奇迹水晶]，你的每回合结束时移除一个，奇迹水晶为0时若此卡仍位于“弦卷集团”则将其放入弃牌堆。
//! > （2）[持续] [触发结算]后可将“弦卷集团”格子上的此卡放置于[移动终点]格子上并移除其上全部奇迹水晶
//! > （3）[持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。
//!
//! 弦卷集团, the move settles as the agent tile; the card then swaps tiles around.
//!
//! （3） is a board **position** exchange (「交换位置」), not a colour override:
//! while the card sits on `e`, a settle on `e` runs as 弦卷集团 (its agent body /
//! ownership / rent) and a settle on 弦卷集团 runs as `e`. Field cards on the
//! position are untouched (「what the tile *is*, not what is on it」). After any
//! settle on the card's tile the card goes to the discard. The swap is one
//! level deep — the redirected settle must not swap back (the `smile_replay`
//! slot is consumed by that settle's `tileResolved`).

use card_sdk::abi::{ChainKind, HookKind};
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "HHW:笑容大游行";
/// One-shot move tag armed by the (1) counteract: this move's endpoint settles
/// as the 弦卷集团 agent (「当次移动的移动终点视为"弦卷集团"地产商」).
const TAG_AS_AGENT: &str = "smile_as_agent";
/// Slot: the tile of an in-flight settle redirect (（1） or （3）). Consumed by
/// that settle's `tileResolved` so neither hook re-enters (STACK-01).
const SLOT_REPLAY: &str = "smile_replay";

pub const SMILE_PARADE: CardDef = CardDef::new(
    "HHW:笑容大游行",
    &[
        // （1）「当次移动的移动终点视为"弦卷集团"地产商」 and （3）「那格视为与
        // "弦卷集团"格子交换位置」 share one `SettleBody` entry: the host runs
        // only the first hook of a kind per card, so the two body-replaces
        // cannot be separate entries.
        On::Hook(
            &[HookKind::SettleBody],
            "tile.id >= 0",
            Some(settle_body_guard),
            settle_body,
        ),
        // Consume an in-flight redirect at its `tileResolved`, and (3）「任何
        // 玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」. `tileResolved` is
        // the settle's terminal (SETTLE-STAGES §4 M5), so it is after the body
        // and after (2)'s `settleAfter` move -- the settle that *moves* the
        // card onto the endpoint sees the card still on 弦卷集团 here and does
        // not discard it.
        On::Hook(&[HookKind::TileResolved], "card.placed", Some(resolve_guard), on_resolved),
        // 规则书（2）: 「[触发结算]后可将"弦卷集团"格子上的此卡放置于[移动终点]格子上」
        // -- the offer exists only while this card sits on 弦卷集团 and the
        // settle is a real move's endpoint. `move.kind != Teleport` keeps the
        // (1)/(3) `card_settle_at` redirects (whose fake move is a Teleport
        // onto the card's own tile) from offering 「移动终点」 = 弦卷集团 and
        // clearing crystals while the card is still there (which would trip
        // the zero-crystal discard).
        On::Hook(
            &[HookKind::SettleAfter],
            "actor == owner && card.placed && card.tile == tile_named('弦卷集团') && tile.id >= 0 && move.kind != Teleport",
            None,
            move_after,
        ),
        On::Counteract(
            &[ChainKind::Pass],
            "actor == owner && tile.id == tile_named('弦卷集团') && move.kind != null",
            None,
            counteract,
        ),
        On::Hook(&[HookKind::TurnEnd], "actor == owner && card.placed", None, turn_end),
        On::Hook(&[HookKind::CounterChanged], "actor == owner && card.placed && counter_is('crystals') && card.counter('crystals') == 0 && value <= 0", Some(crystals_changed_guard), on_crystals_changed),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"笑容大游行"#)
        .raw_name(r#"笑容大游行"#)
        .tags(&[r#"反击"#, r#"持续"#])
        .text(r#"（1）[反击]经过“弦卷集团”（#29格）时可将此卡放置在其上，当次移动的移动终点视为“弦卷集团”地产商；为此卡添加3个[奇迹水晶]，你的每回合结束时移除一个，奇迹水晶为0时若此卡仍位于“弦卷集团”则将其放入弃牌堆。（2）[持续] [触发结算]后可将“弦卷集团”格子上的此卡放置于[移动终点]格子上并移除其上全部奇迹水晶（3）[持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。"#)
        .cell(r#"F3"#)
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

/// C# `CardSmileParade.Group` -- `H.TsurumakiAgent` = tile 弦卷集团 (#29).
fn group_tile() -> i32 {
    ctx::tile_named("弦卷集团")
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && tile.id == tile_named('弦卷集团') && move.kind != null`
    // is the pre; no residual guard (`tile_named` is -1 when unknown and
    // `tile.id == -1` is false).
    let group = group_tile();
    // 规则书（1）[反击]: 「可将此卡放置在其上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // Group, 3)` places the card on the 弦卷集团 tile, charged with 3 crystals.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card_on(
        player_id,
        group,
        ID,
        &Msg::new(key!("smile_parade_note")).tile("tile", group),
    );
    // 规则书（1）: 「为此卡添加3个[奇迹水晶]」 -- the placement's crystal charge
    // (C# `H.PlaceFromPlay(c, ..., 3)`).
    ctx::set_crystals(3);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_placed"))
            .player_id("who", player_id)
            .tile("tile", group),
    );
    // 规则书（1）: 「当次移动的移动终点视为”弦卷集团”地产商」 -- arm a one-shot
    // move tag so `settle_instead` replaces **that move's** endpoint settle and
    // not every later settle the owner makes. `set_settle_as_agent` is the
    // engine's matching plan flag (`H.AgentLanding`'s shape).
    ctx::plan::set_tag(TAG_AS_AGENT, 1);
    ctx::plan::set_settle_as_agent(true);
    Ok(())
}

/// 规则书（1）: 「你的每回合结束时移除一个」 -- C# `CardSmileParade.TurnEnd`
/// (`turn == Player && Tile == Group && Crystals > 0` -> `AddCrystals(-1)`).
/// The 「奇迹水晶为0时若此卡仍位于"弦卷集团"则将其放入弃牌堆」 half is
/// [`on_crystals_changed`].
fn turn_end(_player_id: i32) -> card_sdk::Asked {
    // C# `Tile == Group` -- the decay only runs while the card still sits on
    // 弦卷集团.
    if !on_group() {
        return Ok(());
    }
    ctx::decay()?;
    Ok(())
}

/// Is the card still sitting on 「弦卷集团」?
fn on_group() -> bool {
    let group = ctx::tile_named("弦卷集团");
    match ctx::self_tile() {
        Some(t) => group >= 0 && t == group,
        None => false,
    }
}

/// 规则书（1）: 「奇迹水晶为0时若此卡仍位于"弦卷集团"则将其放入弃牌堆」 -- C#
/// `H.Unplace(this, "discard")`.
///
/// 「若此卡仍位于"弦卷集团"」 is a real condition, not a restatement of the
/// tick's own gate: （2） moves the card to the [移动终点] and strips its
/// crystals, and that emptying is *not* this discard.
/// Residual guard for [`on_crystals_changed`] -- `card_is`, the group check
/// and the counter name stay here (the name is not yet in the condition
/// vocabulary).
fn crystals_changed_guard(_player_id: i32) -> bool {
    trigger::card_is(ID)
        && trigger::name() == card_sdk::abi::counter::CRYSTALS
        && on_group()
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_decayed")).player_id("who", player_id),
    );
    Ok(())
}

/// Combined (1)/(3) residual: either the one-shot `smile_as_agent` tag, or a
/// real swap (card on a tile other than 弦卷集团, settle on one of the two).
/// Bail once another field card claimed the body (`trigger::cancelled()` --
/// Parking Space's (1)) or a redirect is already parked. `SLOT_REPLAY` is
/// stored as `tile + 1` (0 = unset; `state_get` defaults missing keys to 0).
fn settle_body_guard(owner: i32) -> bool {
    if trigger::cancelled() {
        return false;
    }
    if ctx::slot(owner, SLOT_REPLAY) > 0 {
        return false;
    }
    if trigger::move_tag(TAG_AS_AGENT) != 0 {
        return true;
    }
    let group = ctx::tile_named("弦卷集团");
    let Some(card_t) = ctx::self_tile() else {
        return false;
    };
    let at = trigger::tile();
    group >= 0 && card_t >= 0 && at >= 0 && card_t != group && (at == card_t || at == group)
}

/// （1）「当次移动的移动终点视为"弦卷集团"地产商」 then （3）「那格视为与
/// "弦卷集团"格子交换位置」. One entry so both body-replaces run (the host
/// runs only the first hook of a kind).
fn settle_body(player_id: i32) -> card_sdk::Asked {
    let group = ctx::tile_named("弦卷集团");
    if group < 0 {
        return Ok(());
    }
    // （1）: the placement move's endpoint settles as the agent. Keep the
    // `smile_as_agent` tag set across the nested settle so its `tileResolved`
    // does not read as (3)'s 「在此卡放置的格子上[触发结算]」 (the card is on
    // 弦卷集团 right now).
    if trigger::move_tag(TAG_AS_AGENT) != 0 {
        ctx::trigger::set_cancelled();
        ctx::plan::set_settle_as_agent(true);
        ctx::card_settle_at(player_id, group, true);
        ctx::plan::set_tag(TAG_AS_AGENT, 0);
        ctx::log(
            player_id,
            &Msg::new(key!("smile_parade_agent")).tile("tile", group),
        );
        return Ok(());
    }
    // （3）: the card's tile behaves as 弦卷集团 -- a settle *on* the card's
    // tile runs the agent body, then the card is discarded. One level deep: we
    // only redirect card-tile → 弦卷集团, never back (a two-way swap here
    // re-enters and hits the settle-depth cap).
    // TODO(规则书): 「交换位置」 is a two-way exchange -- a settle on 弦卷集团
    // while the card sits elsewhere should run as the card's tile. That
    // direction is not expressible yet without a host-visible re-entrancy flag
    // (`SLOT_REPLAY` guest writes do not cross `card_settle_at`).
    let card_t = ctx::self_tile().unwrap_or(-1);
    let at = trigger::tile();
    if card_t < 0 || at < 0 || card_t == group || at != card_t {
        return Ok(());
    }
    ctx::trigger::set_cancelled();
    ctx::plan::set_settle_as_agent(true);
    ctx::card_settle_at(player_id, group, true);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_swapped"))
            .tile("a", at)
            .tile("b", group),
    );
    Ok(())
}

/// Consume an in-flight redirect at its `tileResolved`, and discard on a
/// settle of the card's own tile. Residual: either a redirect is parked for
/// this tile, or the settle is on the card's tile (and not a redirect).
fn resolve_guard(owner: i32) -> bool {
    let at = trigger::tile();
    if at < 0 {
        return false;
    }
    // (1)'s nested settle is the owner's own placement-move redirect; (3)'s
    // discard is for a settle that *lands on* the card's tile. Skip the owner
    // so (1) does not discard the card it just placed. TODO(规则书): 「任何
    // 玩家」 includes the owner -- once (1) is keyed off the placement move
    // alone, drop this exclusion.
    if trigger::player_id() == owner {
        return false;
    }
    if ctx::slot(owner, SLOT_REPLAY) == at + 1 {
        return true;
    }
    if ctx::slot(owner, SLOT_REPLAY) > 0 {
        return false;
    }
    match ctx::self_tile() {
        Some(card_t) => card_t >= 0 && card_t == at,
        None => false,
    }
}

fn on_resolved(owner: i32) -> card_sdk::Asked {
    let at = trigger::tile();
    if trigger::move_tag(TAG_AS_AGENT) != 0 {
        return Ok(());
    }
    if ctx::slot(owner, SLOT_REPLAY) == at + 1 {
        ctx::set_slot(owner, SLOT_REPLAY, 0);
        return Ok(());
    }
    // （3）「任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」.
    let Some(card_t) = ctx::self_tile() else {
        return Ok(());
    };
    if card_t >= 0 && card_t == at {
        ctx::set_dest(ctx::Dest::Graveyard);
        ctx::log(
            owner,
            &Msg::new(key!("smile_parade_swap_discard")).tile("tile", card_t),
        );
    }
    Ok(())
}

/// （2）「[触发结算]后可将"弦卷集团"格子上的此卡放置于[移动终点]格子上并移除其上
/// 全部奇迹水晶」.
/// （2）: applicability is the entry's condition (`card.tile ==
/// tile_named('弦卷集团') && tile.id >= 0`); the ask is the 「可」 opt-in.
fn move_after(player_id: i32) -> card_sdk::Asked {
    let to = ctx::trigger::tile();
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("smile_parade_title")),
        &Msg::new(key!("smile_parade_move")).tile("tile", to),
    )? {
        return Ok(());
    }
    ctx::set_self_tile(to);
    let n = ctx::crystals();
    if n > 0 {
        ctx::add_crystals(-n, i32::MAX)?;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_moved")).tile("tile", to),
    );
    Ok(())
}

// （2）「[持续] [触发结算]后可将"弦卷集团"格子上的此卡放置于[移动终点]格子上
// 并移除其上全部奇迹水晶」 -- a `settleAfter` press; `set_self_tile` is the
// move, and `place_card_on` is what put it on 弦卷集团 in the first place.
// （3） is `swap_body` / `discard_after_settle` above: a board position
// exchange for the settle body/ownership/rent reads, then the discard.

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn on_resolved_kani(player_id: i32) -> card_sdk::Asked {
    on_resolved(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_end_kani(player_id: i32) -> card_sdk::Asked {
    turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn move_after_kani(player_id: i32) -> card_sdk::Asked {
    move_after(player_id)
}
