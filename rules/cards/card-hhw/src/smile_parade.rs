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
use card_sdk::ctx::{self, trigger};
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
        // （1）「当次移动的移动终点视为"弦卷集团"地产商」 -- only the move that
        // just counteracted (the `smile_as_agent` tag the Pass [反击] arms).
        On::Hook(
            &[HookKind::SettleBody],
            "move_tag('smile_as_agent') != 0 && tile.id >= 0",
            Some(settle_instead_guard),
            settle_instead,
        ),
        // （3）「那格视为与"弦卷集团"格子交换位置」 -- settle body / ownership /
        // rent reads are the other tile's. Bail if another field card (Parking
        // Space) already claimed the body, or if this settle is already a
        // redirect.
        On::Hook(
            &[HookKind::SettleBody],
            "card.placed && tile.id >= 0",
            Some(swap_body_guard),
            swap_body,
        ),
        // Consume an in-flight redirect, or (3）「任何玩家在此卡放置的格子上
        // [触发结算]后此卡放入弃牌堆」. Declared so the settle that *moves* the
        // card via (2) does not discard it (the card is still on 弦卷集团 at
        // `settleAfter`; this is `tileResolved` and (2) has already run -- so
        // the discard clause keys off `settleAfter` instead, below).
        On::Hook(&[HookKind::TileResolved], "card.placed", Some(replay_guard), consume_replay),
        // （3） discard after any settle on the card's tile. Runs at
        // `settleAfter` *before* (2), so the settle that moves the card onto
        // the endpoint does not discard it in the same breath.
        On::Hook(
            &[HookKind::SettleAfter],
            "card.placed && tile.id == card.tile",
            Some(discard_after_settle_guard),
            discard_after_settle,
        ),
        // 规则书（2）: 「[触发结算]后可将"弦卷集团"格子上的此卡放置于[移动终点]格子上」
        // -- the offer exists only while this card sits on 弦卷集团 and the
        // settle has an endpoint to move it to. CEL: `card.tile` / `tile_named`
        // / `tile.id`.
        On::Hook(
            &[HookKind::SettleAfter],
            "actor == owner && card.placed && card.tile == tile_named('弦卷集团') && tile.id >= 0",
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
)
    .legacy(&[(0, legacy_mine), (4, legacy_mine)]);

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
    ctx::place_card(player_id, ID, &Msg::new(key!("smile_parade_note")));
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

/// （1） residual: the one-shot `smile_as_agent` tag, and not already a redirect.
fn settle_instead_guard(owner: i32) -> bool {
    trigger::move_tag(TAG_AS_AGENT) != 0 && ctx::slot(owner, SLOT_REPLAY) < 0
}

/// （1）「当次移动的移动终点视为"弦卷集团"地产商」 -- `H.AgentLanding(m.Seat, Group)`.
/// The pre is the `smile_as_agent` move tag the (1) counteract armed, so this
/// replaces only that move's endpoint settle.
fn settle_instead(player_id: i32) -> card_sdk::Asked {
    let group = ctx::tile_named("弦卷集团");
    if group < 0 {
        return Ok(());
    }
    ctx::trigger::set_cancelled();
    ctx::plan::set_settle_as_agent(true);
    // Consume the one-shot; park the redirect target so neither (1) nor (3)
    // re-enters (STACK-01).
    ctx::plan::set_tag(TAG_AS_AGENT, 0);
    ctx::set_slot(player_id, SLOT_REPLAY, group);
    ctx::card_settle_at(player_id, group, true);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_agent")).tile("tile", group),
    );
    Ok(())
}

/// （3） swap residual guard: the card sits on a tile that is *not* 弦卷集团
/// (a real exchange), the settle is on one of the two swapped tiles, and this
/// settle is not already a redirect. Also bail once another field card has
/// claimed the body (`trigger::cancelled()` -- Parking Space's (1)).
fn swap_body_guard(owner: i32) -> bool {
    if trigger::cancelled() {
        return false;
    }
    if ctx::slot(owner, SLOT_REPLAY) >= 0 {
        return false;
    }
    let group = ctx::tile_named("弦卷集团");
    let Some(card_t) = ctx::self_tile() else {
        return false;
    };
    let at = trigger::tile();
    group >= 0 && card_t >= 0 && at >= 0 && card_t != group && (at == card_t || at == group)
}

/// （3）「当此卡位于格子上时，那格视为与"弦卷集团"格子交换位置」 -- the settle
/// body / ownership / rent reads are the other tile's. A settle on the card's
/// tile `e` runs as 弦卷集团 (agent); a settle on 弦卷集团 runs as `e`. The
/// redirected settle is one level deep (`SLOT_REPLAY` keeps it from swapping
/// back and overflowing the stack).
fn swap_body(player_id: i32) -> card_sdk::Asked {
    let group = ctx::tile_named("弦卷集团");
    let Some(card_t) = ctx::self_tile() else {
        return Ok(());
    };
    let at = trigger::tile();
    if group < 0 || card_t < 0 || at < 0 || card_t == group {
        return Ok(());
    }
    let to = if at == card_t { group } else { card_t };
    ctx::trigger::set_cancelled();
    ctx::set_slot(player_id, SLOT_REPLAY, to);
    ctx::card_settle_at(player_id, to, true);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_swapped"))
            .tile("a", at)
            .tile("b", to),
    );
    Ok(())
}

/// Consume an in-flight redirect at its `tileResolved` (the redirected settle
/// itself). Residual: a redirect is parked for this tile.
fn replay_guard(owner: i32) -> bool {
    ctx::slot(owner, SLOT_REPLAY) == trigger::tile() && trigger::tile() >= 0
}

fn consume_replay(owner: i32) -> card_sdk::Asked {
    ctx::set_slot(owner, SLOT_REPLAY, -1);
    Ok(())
}

/// （3） discard residual: not the redirected settle (that one is the other
/// tile's body and already consumed its replay slot at `tileResolved` -- at
/// `settleAfter` the slot is still parked, so skip).
fn discard_after_settle_guard(owner: i32) -> bool {
    ctx::slot(owner, SLOT_REPLAY) < 0
}

/// （3）「任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」 -- after any
/// player's settle on the tile the card sits on. Declared before (2) so the
/// settle that *moves* the card onto the endpoint does not discard it in the
/// same breath (at that moment the card is still on 弦卷集团).
fn discard_after_settle(player_id: i32) -> card_sdk::Asked {
    let Some(card_t) = ctx::self_tile() else {
        return Ok(());
    };
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_swap_discard")).tile("tile", card_t),
    );
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

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}
