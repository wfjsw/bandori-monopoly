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
//! level deep.
//!
//! （1）「当次移动的移动终点视为"弦卷集团"地产商」 is the same exchange for one
//! settle: the move's endpoint is not the card's slot (the card is on
//! 弦卷集团 at that moment), so the slot binding alone cannot carry it. The
//! settle's own hook binds a one-shot **rule swap** between the endpoint and
//! 弦卷集团 (`ctx::tile_swap_rules`), the same settle then runs the agent body,
//! and the binding is dropped at its `tileResolved`. No nested settle and no
//! `settle_tile_at` (ABI-13).

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, ChainKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "HHW:笑容大游行";
/// One-shot move tag armed by the (1) counteract: this move's endpoint settles
/// as the 弦卷集团 agent (「当次移动的移动终点视为"弦卷集团"地产商」). Read by
/// [`settle_body_guard`] to install the one-shot rule swap and by
/// [`on_resolved`] to drop it again; dies with the move.
const TAG_AS_AGENT: &str = "smile_as_agent";
/// Card-instance latch set by (2) 「[触发结算]后可将…放置于[移动终点]」 when it
/// drops the card onto the endpoint, so that same settle's `tileResolved` does
/// not read as (3)'s 「在此卡放置的格子上[触发结算]后」. One-shot, same-settle
/// only (no nested settle between set and read). Dies with the instance.
const PROP_ARRIVED: &str = "smile_arrived";

pub const SMILE_PARADE: CardDef = CardDef::new(
    "HHW:笑容大游行",
    &[
        // （1）「当次移动的移动终点视为"弦卷集团"地产商」 and （3）「那格视为与
        // "弦卷集团"格子交换位置」 share one `SettleBody` entry so (1) can
        // return early: the dispatcher runs every admitted entry of a kind, so
        // a separate (3) entry would swap a second time.
        On::Action(Action::SettleBody("tile.id >= 0", Some(settle_body_guard), settle_body)),
        // Drop (1)'s one-shot rule swap, consume (2)'s arrival latch, and
        // (3）「任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」.
        // `tileResolved` is the settle's terminal (SETTLE-STAGES §4 M5), after
        // the body and after (2)'s `settleAfter` move. (2) parks
        // `PROP_ARRIVED` when it drops the card onto the endpoint so this same
        // settle does not read as (3)'s discard; every real settle on the
        // card's slot does, owner and other players alike.
        On::Action(Action::TileResolved("card.placed", Some(resolve_guard), on_resolved)),
        // 规则书（2）: 「[触发结算]后可将“弦卷集团”格子上的此卡放置于[移动终点]格子上」
        // -- the offer exists only while this card sits on 弦卷集团 and the
        // settle is a real move's endpoint, a teleport's endpoint included. A
        // card-initiated settle (`settle_tile_at`) is not a move and has no
        // [移动终点] at all, so [`settle_after_guard`] keeps those out. The
        // (1) endpoint **is** a move's endpoint -- with no nested redirect any
        // more it is this very settle, and 移动终点 is read off its tile, so
        // (2) is offered on it exactly as the book reads.
        On::Action(Action::SettleAfter("card.placed && tile.id >= 0", Some(settle_after_guard), move_after)),
        // 规则书（1）[反击]: 「经过“弦卷集团”（#29格）时可将此卡放置在其上」
        On::Counteract(
            &[ChainKind::PassTile],
            "actor == owner && tile.id == tile_named('弦卷集团') && move.kind != null",
            None,
            counteract).counter_reason(key!("smile_parade_when")),
        On::Action(Action::TurnEnd("actor == owner && card.placed", None, turn_end)),
        On::Action(Action::CounterChanged("actor == owner && card.placed && counter_is('crystals') && card.counter('crystals') == 0 && value <= 0", Some(crystals_changed_guard), on_crystals_changed)),
    ],
)
// 规则书（1）「3个[奇迹水晶]」: the `⌛3` marker the client shows.
.props(&[(bandori_std::marker::TURNS_LEFT, 3)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"笑容大游行"#)
        .raw_name(r#"笑容大游行"#)
        .tags(&[r#"反击"#, r#"持续"#])
        .text(CardText::items(&[
            it(r#"[反击]经过“弦卷集团”（#29格）时可将此卡放置在其上，当次移动的移动终点视为“弦卷集团”地产商；为此卡添加3个[奇迹水晶]，你的每回合结束时移除一个，奇迹水晶为0时若此卡仍位于“弦卷集团”则将其放入弃牌堆。"#),
            it(r#"[持续] [触发结算]后可将“弦卷集团”格子上的此卡放置于[移动终点]格子上并移除其上全部奇迹水晶"#),
            it(r#"[持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。"#),
        ]))
        // sheet cell F3
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

fn group_tile() -> i32 {
    ctx::tile_named("弦卷集团")
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && tile.id == tile_named('弦卷集团') && move.kind != null`
    // is the condition; no residual guard (`tile_named` is -1 when unknown and
    // `tile.id == -1` is false).
    let group = group_tile();
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::place_card_on(
        player_id,
        group,
        ID,
        &Msg::new(key!("smile_parade_note")).tile("tile", group),
    );
    // （3）「那格视为与"弦卷集团"格子交换位置」 -- one prop write. The binding
    // is instance-owned and anchored at the instance's slot, so (2)'s
    // `set_self_tile` carries it to the endpoint and the discard drops it
    // (设计 6.2).
    ctx::set_swapped_tile(group_tile());
    ctx::set_crystals(3);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_placed"))
            .player_id("who", player_id)
            .tile("tile", group),
    );
    // 规则书（1）: 「当次移动的移动终点视为“弦卷集团”地产商」 -- arm a one-shot move tag that
    // [`settle_body_guard`] reads so **that move's** endpoint settle (and not every
    // later settle the owner makes) runs as the agent. The endpoint itself is
    // bound at that settle (the card is on 弦卷集团 until (2) moves it).
    ctx::plan::set_tag(TAG_AS_AGENT, 1);
    Ok(())
}

/// The printed tick is **unconditional**; only the zero-crystal discard names
/// 「仍位于"弦卷集团"」. The 「奇迹水晶为0时若此卡仍位于"弦卷集团"则将其放入弃牌堆」 half is
/// [`on_crystals_changed`].
fn turn_end(_player_id: i32) -> card_sdk::Asked {
    // 「你的每回合结束时移除一个」 -- the tick.
    ctx::dec_turns_left();
    ctx::remove_crystal()?;
    Ok(())
}

/// Is the card still sitting on 「弦卷集团」's own square?
///
/// 「仍位于」 is the physical square, not the displayed tile: （2） moves the
/// card onto the [移动终点] and the swap rides along, so after （2） that square
/// *displays* 弦卷集团 while the card has left the group's square. The
/// qualifier exists exactly so （2）'s 「移除其上全部奇迹水晶」 does not discard;
/// only a tick while the card still lies on the group's own square does.
fn on_group() -> bool {
    let group = ctx::tile_named("弦卷集团");
    group >= 0 && ctx::self_position() == group
}

///
/// 「若此卡仍位于"弦卷集团"」 is a real condition, not a restatement of the
/// tick's own gate: （2） moves the card to the [移动终点] and strips its
/// crystals, and that emptying is *not* this discard.
/// Residual guard for [`on_crystals_changed`] -- `card_is`, the group check
/// and the counter name stay here (the name is not yet in the condition
/// vocabulary).
fn crystals_changed_guard(_player_id: i32) -> bool {
    let g = on_group();
    trigger::card_is(ID) && trigger::name() == bandori_std::counter::CRYSTALS && g
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_decayed")).player_id("who", player_id),
    );
    Ok(())
}

/// (1)'s residual: the one-shot `smile_as_agent` tag must be armed, and
/// another field card must not have claimed the body already
/// (`trigger::cancelled()` -- Parking Space's (1)). (3) needs no branch: its
/// slot binding already resolved the settle to the partner at dispatch.
fn settle_body_guard(_owner: i32) -> bool {
    !trigger::cancelled() && trigger::move_tag(TAG_AS_AGENT) != 0
}

/// （1）「当次移动的移动终点视为"弦卷集团"地产商」 -- bind the settle's **tile**
/// to 弦卷集团 for this one settle (`ctx::tile_swap_rules`), so the tile stage
/// that follows this hook runs the agent body (its ownership / rent / offers)
/// while the square keeps its identity. [`on_resolved`] drops the binding.
///
/// The bind is taken on this card instance, so it dies with the card as well.
/// `trigger::tile()` (not the slot) is used: the endpoint is what the settle
/// governs, and it may itself stand elsewhere through another card's slot
/// binding.
fn settle_body(player_id: i32) -> card_sdk::Asked {
    let group = ctx::tile_named("弦卷集团");
    let at = trigger::tile();
    if group < 0 || at < 0 {
        return Ok(());
    }
    ctx::tile_swap_rules(at, group);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_agent")).tile("tile", group),
    );
    Ok(())
}

/// Consume (1)'s one-shot binding and (2)'s arrival latch at `tileResolved`,
/// and discard on a settle of the card's own tile.
/// 规则书（3）: 「任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」 --
/// 「任何玩家」 includes the owner.
fn resolve_guard(_owner: i32) -> bool {
    // (1)'s settle terminal must run the body to drop the binding.
    if trigger::move_tag(TAG_AS_AGENT) != 0 {
        return true;
    }
    // (2) 「[触发结算]后可将…放置于[移动终点]」 arrived during this settle's
    // `settleAfter`. Enter `on_resolved` so the latch is cleared; do not
    // discard.
    if ctx::prop(PROP_ARRIVED) != 0 {
        return true;
    }
    // 「在此卡放置的格子上」: the settle's *slot* against the slot this card
    // was placed on (`self_position`). A settle at 弦卷集团's home slot runs
    // this card's tile through the swap but must not discard (设计 6.2).
    let at = ctx::trigger::position();
    at >= 0 && ctx::self_position() == at
}

fn on_resolved(owner: i32) -> card_sdk::Asked {
    // (1)'s one-shot rule swap ends with its settle: the card keeps only the
    // persistent (3) slot binding. The tag is one-shot; clear it too.
    if trigger::move_tag(TAG_AS_AGENT) != 0 {
        ctx::tile_clear_rule_swap();
        ctx::plan::set_tag(TAG_AS_AGENT, 0);
    }
    // (2)'s arrival latch: clear it and leave the card in play. Later settles
    // discard as (3) says, owner included.
    if ctx::prop(PROP_ARRIVED) != 0 {
        ctx::set_prop(PROP_ARRIVED, 0);
        return Ok(());
    }
    // （3）「任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」 --
    // 「任何玩家」 includes the owner.
    let at = ctx::trigger::position();
    if at >= 0 && ctx::self_position() == at {
        let card_t = ctx::self_tile().unwrap_or(-1);
        ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
        ctx::log(
            owner,
            &Msg::new(key!("smile_parade_swap_discard")).tile("tile", card_t),
        );
    }
    Ok(())
}

/// Residual guard for [`move_after`]: a card-initiated settle has no move, so
/// no [移动终点] -- and there is no CEL binding for the flag, so it cannot ride
/// the condition. A real teleport is a move and stays in.
fn settle_after_guard(_owner: i32) -> bool {
    // A card-initiated settle (`settle_tile_at`) is not a move and has no
    // [移动终点] (`is_settle_redirect`).
    if trigger::is_settle_redirect() {
        return false;
    }
    // 「“弦卷集团”格子上的此卡」: after (2) the card's own slot *displays* the
    // group, so `card.tile` alone would still offer a second move on the very
    // next settle. The slot the card was placed on is `self_position`; a
    // settle there is the endpoint it already stands at.
    trigger::position() != ctx::self_position()
}

/// （2）「[触发结算]后可将"弦卷集团"格子上的此卡放置于[移动终点]格子上并移除其上
/// 全部奇迹水晶」.
/// （2）: applicability is the entry's condition (`card.placed && card.tile ==
/// tile_named('弦卷集团') && tile.id >= 0`) plus [`settle_after_guard`]; the
/// ask is the 「可」 opt-in. `player_id` is the card owner (the field
/// instance's holder), which is who gets the ask for any player's settle.
fn move_after(player_id: i32) -> card_sdk::Asked {
    // 「[移动终点]」 -- the endpoint's governing tile; the binding is anchored
    // at the slot, so this leaves (endpoint, group) behind (设计 6.2).
    let to = ctx::trigger::tile();
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("smile_parade_title")),
        &Msg::new(key!("smile_parade_move")).tile("tile", to),
    )? {
        return Ok(());
    }
    let ok = ctx::set_self_tile(to);
    // The card just arrived on this settle's tile. (3) 「任何玩家在此卡放置的格子上
    // [触发结算]后」 is for a settle *on* the card's tile -- not the settle whose
    // `settleAfter` placed it here. Park a one-shot card-instance latch so this
    // settle's `tileResolved` consumes it instead of discarding; later settles
    // discard as (3) says, owner included.
    ctx::set_prop(PROP_ARRIVED, 1);
    let n = ctx::crystals();
    if n > 0 {
        ctx::add_crystals(-n, i32::MAX)?;
    }
    // 「移除其上全部奇迹水晶」: the (1) countdown is spent with them, so the
    // face's `⌛N` goes too.
    ctx::set_turns_left(0);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_moved")).tile("tile", to),
    );
    Ok(())
}

// （2）「[持续] [触发结算]后可将"弦卷集团"格子上的此卡放置于[移动终点]格子上
// 并移除其上全部奇迹水晶」 -- a `settleAfter` press; `set_self_tile` is the
// move, and `place_card_on` is what put it on 弦卷集团 in the first place.
// （3） is `settle_body` / `on_resolved` above: a two-way board position
// exchange for the settle body/ownership/rent reads, then the discard.

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn settle_body_kani(player_id: i32) -> card_sdk::Asked {
    settle_body(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_resolved_kani(player_id: i32) -> card_sdk::Asked {
    on_resolved(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn move_after_kani(player_id: i32) -> card_sdk::Asked {
    move_after(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
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
