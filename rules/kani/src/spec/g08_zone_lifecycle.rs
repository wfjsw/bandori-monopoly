//! Reference model for the G08 zone / attachment / creator lifecycle
//! (`docs/rulebook/spec/abi/G08-zone-lifecycle.md` §4 I5-I13, §7.1, §7.6,
//! §7.7).
//!
//! The engine-side writer is `game_core::engine::zone` (`zone_prepare` /
//! `zone_commit` / `zone_leave` / `zone_attach` / `zone_detach`) over
//! `game_core::engine::ops` (`resolve_zone_transition`, `drop_instance_edges`,
//! `apply_zone_row`, `set_tile_owner`). The host functions in
//! `crates/game-rules/src/hostfns.rs` only translate wire arguments into the
//! `HostRequest` the writer runs, so the obligations here are on the model
//! writer in [`crate::world`], which mirrors that writer call for call.
//!
//! What is modelled:
//!
//! * [`seed`] / [`seed_attached`] / [`seed_child`] — the row shapes.
//! * [`leave`] — the one transition out of play (cascade order, destination
//!   rewrite, `wasted` read-and-clear).
//! * [`attach_admitted`] — the attach prepare (cycle / dead-host refusal).
//! * [`earliest`] / [`sale`] / [`release`] — the acquisition ledger.
//! * [`visible_on_field`] / [`visible_on_tile`] — the `on < 0` listing rule.

use crate::intern;
use crate::world::{
    is_played, World, PROP_LEAVE_DEST, ZONE_ATTACHED, ZONE_GRAVEYARD, ZONE_HAND, ZONE_NOWHERE,
};

/// The card name slots a proof fixture uses (small, interned once).
pub const CARD_HOST: &str = "PP:[G08]host";
pub const CARD_ATTACHMENT: &str = "PP:[G08]attachment";
pub const CARD_CHILD: &str = "PP:[G08]child";
pub const CARD_GRANDCHILD: &str = "PP:[G08]grandchild";
pub const CARD_OTHER: &str = "PP:[G08]other";

fn name(card: &str) -> u16 {
    intern::intern(card).0
}

/// Plant an unattached row for `player` on `tile`; answers its uid.
pub fn seed(w: &mut World, player: i32, card: &str, tile: i32) -> i32 {
    w.make_row(name(card), player, player, tile)
}

/// Plant a row attached to `host` (same holder, same tile) and, when
/// `leave_dest >= 0`, give it that `prop::LEAVE_DEST`; answers its uid.
pub fn seed_attached(w: &mut World, host: i32, card: &str, leave_dest: i32) -> i32 {
    let Some(h) = w.field_by_uid(host).copied() else {
        return -1;
    };
    let uid = w.make_row(name(card), h.owner, h.player, h.tile);
    if let Some(f) = w.field_by_uid_mut(uid) {
        f.on = host;
        if leave_dest >= 0 {
            f.props.set(intern::intern(PROP_LEAVE_DEST), leave_dest);
        }
    }
    uid
}

/// Plant a row created by `creator` (same holder, same tile); answers its
/// uid.
pub fn seed_child(w: &mut World, creator: i32, card: &str) -> i32 {
    let Some(c) = w.field_by_uid(creator).copied() else {
        return -1;
    };
    let uid = w.make_row(name(card), c.owner, c.player, c.tile);
    if let Some(f) = w.field_by_uid_mut(uid) {
        f.created_by = creator;
    }
    uid
}

/// One leave of `uid` into `destination` (`-1` = its own leave destination).
pub fn leave(w: &mut World, uid: i32, destination: i32, cause: i32) -> i32 {
    w.zone_leave(uid, destination, -1, cause)
}

/// The attach prepare: `Some((from, holder))` iff the row is live, the host
/// is live and played, and the host is neither the card nor one of its
/// descendants.
pub fn attach_admitted(w: &World, uid: i32, host: i32) -> Option<(i32, i32)> {
    w.zone_prepare_attach(uid, host)
}

/// The earliest-owned tile (the ledger read).
pub fn earliest(w: &World, player: i32) -> i32 {
    w.earliest_owned_tile(player)
}

/// An ownership write to a player (a sale / acquisition).
pub fn sale(w: &mut World, tile: i32, player: i32) {
    w.set_tile_owner(tile, player);
}

/// An ownership release (`-1`).
pub fn release(w: &mut World, tile: i32) {
    w.set_tile_owner(tile, -1);
}

/// A row is visible on the ordinary field pass iff it is unattached.
pub fn visible_on_field(w: &World, uid: i32) -> bool {
    w.field_by_uid(uid).is_some_and(|f| f.on < 0)
}

/// Same for a board tile rule.
pub fn visible_on_tile(w: &World, uid: i32) -> bool {
    w.field_by_uid(uid)
        .is_some_and(|f| f.on < 0 && f.player == -1)
}

/// The row's real `(from, to)` pair for a leave into `destination`, computed
/// independently of the writer: `from` is `Attached` on the row, `to` is the
/// destination argument unless the row's own rewrite wins.
pub fn expected_to(w: &World, uid: i32, destination: i32) -> i32 {
    let direct = if destination < 0 {
        ZONE_GRAVEYARD
    } else {
        crate::world::dest_zone(destination)
    };
    let rewrite = w.leave_dest(uid);
    if rewrite != direct && direct != ZONE_ATTACHED {
        rewrite
    } else {
        direct
    }
}

/// The `wasted` bit a Graveyard arrival carries: exactly the cleared
/// `effective` bit.
pub fn wasted_for(w: &World, uid: i32) -> bool {
    w.field_by_uid(uid).is_some_and(|f| !f.effective)
}

/// Is `uid` attached at all right now?
pub fn is_attached(w: &World, uid: i32) -> bool {
    w.attachment_host(uid) >= 0
}

/// A row is played when its zone is `Field` or `Attached`.
pub fn played(w: &World, uid: i32) -> bool {
    is_played(w.zone_of(uid))
}

/// Convenience: the zone code a probe expects for a dead uid.
pub const NOWHERE: i32 = ZONE_NOWHERE;
/// Convenience: the zone code a probe expects for an attachment.
pub const ATTACHED: i32 = ZONE_ATTACHED;
/// Convenience: the zone code a probe expects for a pile landing.
pub const HAND: i32 = ZONE_HAND;
