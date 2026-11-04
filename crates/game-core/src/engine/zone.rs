//! The single zone writer (design G08 §7.1).
//!
//! Every move of a card between zones is one [`ZoneTransition`] applied here:
//! the before hooks run on the live row, a responder may rewrite the
//! destination (`prop::LEAVE_DEST`), the instance edges cascade, the row changes
//! through [`World::apply_zone_row`], and the after hooks run. Callers never
//! remove or push a row on behalf of a card move.

use game_kinds::{Zone, ZoneChange, ZoneChangeCause};

use crate::engine::cx::Flow;
use crate::engine::rules::Trigger;
use crate::engine::world::{DeckPlacement, ZoneTransition};
use crate::engine::PlayCtx;
use crate::state::prop;

/// The zone (and, for a draw pile, the slot) a guest `Dest` wire code lands in.
fn dest_zone(dest: i32) -> (Zone, DeckPlacement) {
    match dest {
        1 => (Zone::Hand, DeckPlacement::default()),
        2 => (Zone::Field, DeckPlacement::default()),
        3 => (Zone::Banished, DeckPlacement::default()),
        4 => (Zone::Deck, DeckPlacement::Top),
        5 => (Zone::Deck, DeckPlacement::Bottom),
        6 => (Zone::Deck, DeckPlacement::Random),
        _ => (Zone::Graveyard, DeckPlacement::default()),
    }
}

impl PlayCtx<'_> {
    /// Resolve a transition against the live rows: its real `from`, its
    /// landing seat, and a refusal for a dead instance, a dead host or a cycle.
    pub fn zone_prepare(&self, t: ZoneTransition) -> Option<ZoneTransition> {
        self.w.resolve_zone_transition(t)
    }

    /// The row's own leave destination (`prop::LEAVE_DEST`), else the Graveyard.
    fn leave_dest(&self, uid: i32) -> (Zone, DeckPlacement) {
        let dest = self
            .w
            .field_by_uid(uid)
            .and_then(|f| f.props.get(prop::LEAVE_DEST))
            .copied()
            .unwrap_or(0);
        dest_zone(dest)
    }

    /// Raise one zone-family hook on `instance`.
    fn zone_raise(
        &mut self,
        kind: &str,
        seat: i32,
        instance: i32,
        card: Option<String>,
        zone: ZoneChange,
        value: i32,
    ) -> Flow<Trigger> {
        let mut t = Trigger::new(kind, seat.max(0) as usize);
        t.instance = instance;
        t.card = card;
        t.zone = Some(zone);
        t.value = value;
        let t = self.raise(t)?;
        Ok(t)
    }

    /// Apply one resolved transition: before hook, destination rewrite,
    /// cascade, row change, after hooks. Answers the departed / landing seat.
    ///
    /// The caller returns early on a refused prepare; a commit on a row that
    /// vanished mid-window answers `-1`.
    pub fn zone_commit(&mut self, mut t: ZoneTransition) -> Flow<i32> {
        let leaving = t.from.is_played();
        let Some(row) = self.w.field_by_uid(t.uid).cloned() else {
            return Ok(-1);
        };
        let seat = self.w.holder_of(t.uid);
        if leaving {
            // The pre-hook bit; the `discarded` raise uses the post-hook one.
            let wasted = !row.effective;
            let change = ZoneChange {
                from: t.from,
                to: t.to,
                cause: t.cause,
                holder: t.holder,
                wasted,
                counted: t.counted,
            };
            // The prop is the *pending* destination: it starts the window
            // unset, so only a write made in this `zoneLeft` is a rewrite. A
            // value left behind by an earlier leave is stale and must not
            // override the destination this caller named.
            if let Some(f) = self.w.field_by_uid_mut(t.uid) {
                f.props.remove(prop::LEAVE_DEST);
            }
            // `zoneLeft` runs on the live row. A body may rewrite the pending
            // destination; the re-read below is authoritative.
            eprintln!(
                "BEFORE_RAISE uid={} uids={:?}",
                t.uid,
                self.w
                    .st
                    .players
                    .iter()
                    .flat_map(|p| p.field.iter())
                    .map(|f| f.uid)
                    .collect::<Vec<_>>()
            );
            self.zone_raise(
                "zoneLeft",
                seat,
                t.uid,
                Some(row.card.clone()),
                change,
                t.to.as_i32(),
            )?;
            let rewritten = self
                .w
                .field_by_uid(t.uid)
                .and_then(|f| f.props.get(prop::LEAVE_DEST))
                .copied();
            if let Some((to, deck)) = rewritten.map(dest_zone) {
                if to != t.to && t.to != Zone::Attached {
                    t.to = to;
                    t.deck = deck;
                    if !t.to.is_pile() || t.holder < 0 {
                        // The leaving body may have re-owned the row (「视为拥有」,
                        // then a `Field` leave destination): the clone above is
                        // the pre-hook state, so read the owner back.
                        t.holder = self.w.field_by_uid(t.uid).map_or(row.owner, |f| f.owner);
                    }
                }
            }
            // A leave rewritten back onto the own field of a new owner
            // (「视为拥有」) never really left: the row keeps its seat edges,
            // so no cascade; the subtree merely follows the new holder.
            if !(t.to == Zone::Field && t.from == Zone::Field) {
                // Attachments first, then created children, then the leaver.
                let edges = self.w.drop_instance_edges(t.uid);
                for e in edges {
                    if let Some(e) = self.zone_prepare(e) {
                        self.zone_commit(e)?;
                    }
                }
            }
        }
        // A discard that never took effect says so on its `discarded` raise,
        // and the bit dies with the entry. Read after the leave hooks: the
        // `zoneLeft` body is where a card marks itself (「视为此卡未生效」).
        let wasted = !self
            .w
            .field_by_uid(t.uid)
            .map_or(row.effective, |f| f.effective);
        eprintln!(
            "WASTED uid={} uids={:?} w={:p}",
            t.uid,
            self.w
                .st
                .players
                .iter()
                .flat_map(|p| p.field.iter())
                .map(|f| f.uid)
                .collect::<Vec<_>>(),
            &*self.w as *const _
        );
        let stay = leaving && t.from == Zone::Field && t.to == Zone::Field;
        let attached = if stay {
            self.w.attached_cards(t.uid)
        } else {
            Vec::new()
        };
        let holder = self.w.apply_zone_row(&t);
        let Some(holder) = holder else {
            return Ok(-1);
        };
        if stay {
            // `apply_zone_row` drops the row's edges unconditionally; put the
            // attachments back on it before moving the subtree.
            for a in attached {
                if let Some(f) = self.w.field_by_uid_mut(a) {
                    f.on = t.uid;
                }
            }
            self.w.rehold_subtree(t.uid, holder);
        }
        if leaving {
            let change = ZoneChange {
                from: t.from,
                to: t.to,
                cause: t.cause,
                holder,
                wasted,
                counted: t.counted,
            };
            self.zone_raise(
                "zoneLeftAfter",
                seat,
                t.uid,
                Some(row.card.clone()),
                change,
                t.to.as_i32(),
            )?;
            if t.from == Zone::Attached {
                self.zone_raise(
                    "cardDetached",
                    seat,
                    t.uid,
                    Some(row.card.clone()),
                    change,
                    0,
                )?;
            }
        }
        match t.to {
            // Every hand arrival hears it; `counted` says whether it was a
            // draw (the arrival counts toward 「每次加手」 only then).
            Zone::Hand => {
                let change = ZoneChange {
                    from: t.from,
                    to: t.to,
                    cause: t.cause,
                    holder,
                    wasted,
                    counted: t.counted,
                };
                self.zone_raise("handChanged", holder, -1, Some(row.card.clone()), change, 0)?;
            }
            _ => {}
        }
        if t.to == Zone::Attached {
            let change = ZoneChange {
                from: t.from,
                to: t.to,
                cause: t.cause,
                holder,
                wasted,
                counted: false,
            };
            self.zone_raise(
                "cardAttached",
                holder,
                t.uid,
                Some(row.card.clone()),
                change,
                0,
            )?;
        }
        if t.to == Zone::Graveyard {
            let change = ZoneChange {
                from: t.from,
                to: t.to,
                cause: t.cause,
                holder,
                wasted,
                counted: false,
            };
            eprintln!(
                "DISCARDED_RAISE uid={} holder={} wasted={}",
                t.uid, holder, wasted
            );
            self.zone_raise(
                "discarded",
                holder,
                t.uid,
                Some(row.card.clone()),
                change,
                0,
            )?;
        }
        Ok(holder)
    }

    /// Take the instance at `uid` off play into `destination` (`-1` = its own
    /// leave destination). Answers the departed owner, `-1` when it was not in
    /// play.
    pub fn zone_leave(
        &mut self,
        uid: i32,
        destination: i32,
        to_seat: i32,
        cause: i32,
    ) -> Flow<i32> {
        if self.w.field_by_uid(uid).is_none() {
            return Ok(-1);
        }
        let from = self.w.zone_of(uid);
        if !from.is_played() {
            return Ok(-1);
        }
        let (to, deck) = if destination < 0 {
            self.leave_dest(uid)
        } else {
            dest_zone(destination)
        };
        let mut t = ZoneTransition::new(uid, from, to);
        t.deck = deck;
        t.holder = to_seat;
        t.cause = ZoneChangeCause::from_i32(cause);
        let Some(t) = self.zone_prepare(t) else {
            return Ok(-1);
        };
        self.zone_commit(t)
    }

    /// Flip the instance at `uid`; answers `1` on a live row, `0` otherwise.
    pub fn zone_flip(&mut self, uid: i32, face_down: bool, _cause: i32) -> Flow<i32> {
        let Some(row) = self.w.field_by_uid(uid).cloned() else {
            return Ok(0);
        };
        let seat = self.w.holder_of(uid);
        let face = face_down as i32;
        self.zone_raise(
            "cardFaceFlipped",
            seat,
            uid,
            Some(row.card.clone()),
            ZoneChange::default(),
            face,
        )?;
        if !self.w.set_face_down_at(uid, face_down) {
            return Ok(0);
        }
        self.zone_raise(
            "cardFaceFlippedAfter",
            seat,
            uid,
            Some(row.card),
            ZoneChange::default(),
            face,
        )?;
        Ok(1)
    }

    /// Put `card` onto `host`. A live instance of the name is re-parented as
    /// it stands; a copy in a pile has no row and is filed fresh. Answers the
    /// attached uid, `-1` when refused (a cycle, a dead host, no copy).
    ///
    /// A copy in the caller's hand is the one taken; a name with no hand copy
    /// falls back to the caller's first live instance (the re-parent). That
    /// order is what keeps a caller that just drew a copy (Here the world,
    /// ONE OF US's parked deeds) on the copy it means.
    pub fn zone_attach(
        &mut self,
        player_id: i32,
        host: i32,
        card: &str,
        cause: i32,
        face_down: bool,
    ) -> Flow<i32> {
        let Some(host_row) = self.w.field_by_uid(host).cloned() else {
            return Ok(-1);
        };
        let in_hand = self
            .w
            .hidden
            .get(player_id.max(0) as usize)
            .is_some_and(|h| h.hand.iter().any(|c| c == card));
        // Rows stay rows: `from` is real and the cycle guard has something to
        // walk. Counters and the creator edges ride along.
        if !in_hand {
            if let Some(uid) = self.w.live_instance_of(player_id, card, host) {
                let mut t = ZoneTransition::new(uid, self.w.zone_of(uid), Zone::Attached);
                t.host = host;
                t.face_down = face_down;
                t.cause = ZoneChangeCause::from_i32(cause);
                let Some(t) = self.w.resolve_zone_transition(t) else {
                    return Ok(-1);
                };
                let seat = self.w.holder_of(uid);
                if let Some(f) = self.w.field_by_uid_mut(uid) {
                    f.on = host;
                    f.face_down = face_down;
                    f.tile = host_row.tile;
                }
                let change = ZoneChange {
                    from: t.from,
                    to: t.to,
                    cause: t.cause,
                    holder: t.holder,
                    wasted: false,
                    counted: false,
                };
                self.zone_raise("cardAttached", seat, uid, Some(card.to_string()), change, 0)?;
                return Ok(uid);
            }
        }
        let Some(h) = self.w.hidden.get_mut(player_id.max(0) as usize) else {
            return Ok(-1);
        };
        let Some(k) = h.hand.iter().position(|c| c == card) else {
            return Ok(-1);
        };
        h.hand.remove(k);
        let props = self.rules.card_props(card);
        let uid = self.w.place_card_on(
            self.data,
            player_id,
            host_row.tile,
            card,
            crate::msg::Msg::default(),
            props,
        );
        if uid < 0 {
            return Ok(-1);
        }
        if let Some(f) = self.w.field_by_uid_mut(uid) {
            f.on = host;
            f.face_down = face_down;
        }
        let change = ZoneChange {
            from: Zone::Nowhere,
            to: Zone::Attached,
            cause: ZoneChangeCause::from_i32(cause),
            holder: self.w.holder_of(host),
            wasted: false,
            counted: false,
        };
        self.zone_raise(
            "cardAttached",
            player_id,
            uid,
            Some(card.to_string()),
            change,
            0,
        )?;
        Ok(uid)
    }

    /// Take the instance off its host into `destination` (`-1` = its own leave
    /// destination). Answers the owner, `-1` when it was not attached.
    pub fn zone_detach(&mut self, uid: i32, destination: i32, cause: i32) -> Flow<i32> {
        if self.w.attachment_host(uid) < 0 {
            return Ok(-1);
        }
        self.zone_leave(uid, destination, -1, cause)
    }
}
