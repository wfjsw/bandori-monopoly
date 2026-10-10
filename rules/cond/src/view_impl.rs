//! [`CondView`] over the eager snapshot ([`WindowCtx`] + [`CandidateCtx`]).
//!
//! This is the reference implementation: the tests and the precompiled-conds
//! runtime evaluate against a pre-filled snapshot. A live host (`game-rules`'s
//! `LiveSnap`) can implement [`CondView`] directly and skip the eager fill.

use crate::ctx::{CandidateCtx, WindowCtx};
use crate::view::{CondView, TileKind};

/// The view a condition evaluates against: window + candidate overlay.
pub struct SnapshotView<'a> {
    pub win: &'a WindowCtx,
    pub cand: &'a CandidateCtx,
}

impl CondView for SnapshotView<'_> {
    fn kind(&self) -> i64 {
        self.win.kind
    }
    fn actor(&self) -> i64 {
        self.win.actor
    }
    fn target(&self) -> i64 {
        self.win.target
    }
    fn tile_id(&self) -> i64 {
        self.win.tile.id
    }
    fn tile_owner(&self) -> i64 {
        self.win.tile.owner
    }
    fn tile_houses(&self) -> i64 {
        self.win.tile.houses
    }
    fn tile_mortgaged(&self) -> i64 {
        self.win.tile.mortgaged
    }
    fn tile_price(&self) -> i64 {
        self.win.tile.price
    }
    fn value(&self) -> i64 {
        self.win.value
    }
    fn step(&self) -> i64 {
        self.win.step
    }
    fn by(&self) -> i64 {
        self.win.by
    }
    fn pay_is_rent(&self) -> bool {
        self.win.pay_is_rent
    }
    fn move_roll(&self) -> i64 {
        self.win.mv.roll.unwrap_or(-1)
    }
    fn move_kind(&self) -> i64 {
        self.win.mv.kind.unwrap_or(-1)
    }
    fn move_remaining(&self) -> i64 {
        self.win.mv.remaining
    }
    fn move_main(&self) -> bool {
        self.win.mv.main
    }
    fn move_dir(&self) -> i64 {
        self.win.mv.dir
    }
    fn move_tag_named(&self, name: &str) -> i64 {
        self.win
            .mv
            .tags
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| *v)
            .unwrap_or(0)
    }
    fn move_tag_table(&self) -> Vec<(String, i64)> {
        self.win.mv.tags.clone()
    }
    fn roll_source(&self) -> i64 {
        self.win.roll_source
    }
    fn abnormal(&self) -> bool {
        self.win.abnormal
    }
    fn turn_player(&self) -> i64 {
        self.win.turn_player
    }
    fn turn_key(&self) -> i64 {
        self.win.turn_key
    }
    fn chain_count(&self) -> i64 {
        self.win.chain.len() as i64
    }
    fn chain_kinds(&self) -> Vec<i64> {
        self.win.chain.iter().map(|l| l.kind).collect()
    }
    fn chain_hits(&self) -> Vec<i64> {
        self.win.chain.iter().map(|l| l.hits).collect()
    }
    fn trigger_card(&self) -> i64 {
        self.win.trigger_card
    }
    fn counter_name(&self) -> i64 {
        if self.win.name.is_empty() {
            0
        } else {
            crate::id_of(&self.win.name)
        }
    }
    fn owner(&self) -> i64 {
        self.cand.owner
    }
    fn owner_money(&self) -> i64 {
        self.cand.owner_money
    }
    fn owner_fire(&self) -> i64 {
        self.cand.owner_fire
    }
    fn owner_crystals(&self) -> i64 {
        self.cand.owner_crystals
    }
    fn owner_hand(&self) -> i64 {
        self.cand.owner_hand
    }
    fn owner_pos(&self) -> i64 {
        self.cand.owner_pos
    }
    fn owner_out(&self) -> i64 {
        self.cand.owner_out
    }
    fn owner_stay(&self) -> i64 {
        self.cand.owner_stay
    }
    fn owner_stun(&self) -> i64 {
        self.cand.owner_stun
    }
    fn owner_exile(&self) -> i64 {
        self.cand.owner_exile
    }
    fn owner_no_hand(&self) -> i64 {
        self.cand.owner_no_hand
    }
    fn owner_character(&self) -> i64 {
        self.cand.owner_character
    }
    fn owner_band(&self) -> i64 {
        self.cand.owner_band
    }
    fn owner_tiles(&self) -> i64 {
        self.cand.owner_tiles
    }
    fn card_id(&self) -> i64 {
        self.cand.card_id
    }
    fn card_placed(&self) -> bool {
        self.cand.card_placed
    }
    fn card_counter(&self, name: &str) -> i64 {
        self.cand.card_counter(name)
    }
    fn slot(&self, name: &str) -> i64 {
        self.cand.slot(name)
    }
    fn tok_named(&self, name: &str) -> i64 {
        self.cand.tok_named(name)
    }
    fn blocked(&self, band: i64) -> bool {
        self.cand.blocked(band)
    }
    fn money(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.money).unwrap_or(0)
    }
    fn fire(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.fire).unwrap_or(0)
    }
    fn crystals(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.crystals).unwrap_or(0)
    }
    fn hand(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.hand).unwrap_or(0)
    }
    fn pos(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.pos).unwrap_or(0)
    }
    fn out(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.out).unwrap_or(0)
    }
    fn stay(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.stay).unwrap_or(0)
    }
    fn stun(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.stun).unwrap_or(0)
    }
    fn exile(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.exile).unwrap_or(0)
    }
    fn no_hand(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.no_hand).unwrap_or(0)
    }
    fn character(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.character).unwrap_or(0)
    }
    fn band(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.band).unwrap_or(0)
    }
    fn tiles(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.tiles).unwrap_or(0)
    }
    fn seat_count(&self) -> i64 {
        self.win.seat_count()
    }
    fn tile_named(&self, name: &str) -> i64 {
        self.win.tile_ids.get(name).copied().unwrap_or(-1)
    }
    fn is_circle(&self, tile: i64) -> bool {
        self.win.circle_tiles.contains(&tile)
    }
    fn is_ring(&self, tile: i64) -> bool {
        self.win.ring_tiles.contains(&tile)
    }
    fn is_live_house(&self, tile: i64) -> bool {
        self.win.live_house_tiles.contains(&tile)
    }
    fn is_buyable(&self, tile: i64) -> bool {
        self.win.buyable_tiles.contains(&tile)
    }
    fn slot_table(&self) -> Vec<(String, i64)> {
        self.cand.slots.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }
    fn tok_named_table(&self) -> Vec<(String, i64)> {
        self.cand
            .tok_names
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
    fn card_counter_table(&self) -> Vec<(String, i64)> {
        self.cand
            .card_counters
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
    fn blocked_bands(&self) -> Vec<i64> {
        self.cand.blocked_bands.clone()
    }
    fn tile_id_table(&self) -> Vec<(String, i64)> {
        self.win.tile_ids.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }
    fn tile_kind_list(&self, kind: TileKind) -> Vec<i64> {
        match kind {
            TileKind::Circle => self.win.circle_tiles.clone(),
            TileKind::Ring => self.win.ring_tiles.clone(),
            TileKind::LiveHouse => self.win.live_house_tiles.clone(),
            TileKind::Buyable => self.win.buyable_tiles.clone(),
        }
    }

    // -- turn plan / counters ----------------------------------------------
    fn plan_fixed_roll(&self) -> i64 {
        self.win.plan_fixed_roll.unwrap_or(-1)
    }
    fn gains_this_turn(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.gains).unwrap_or(0)
    }
    fn targeted_count(&self, seat: i64) -> i64 {
        self.win.player(seat).map(|p| p.targeted).unwrap_or(0)
    }

    // -- candidate instance -------------------------------------------------
    fn card_tile(&self) -> i64 {
        // Raw `ctx::self_tile()` host value: -2 = not placed, -1 = with owner.
        self.cand.card_tile.unwrap_or(-2)
    }

    // -- board geometry ----------------------------------------------------
    fn tile_count(&self) -> i64 {
        self.win.tile_count
    }
    fn dist(&self, a: i64, b: i64) -> i64 {
        let n = self.win.tile_count;
        if n <= 0 {
            return 0;
        }
        let d = ((b - a) % n + n) % n;
        d.min(n - d)
    }
    fn players_on(&self, tile: i64, except: i64) -> i64 {
        // `ctx::players_on` counts present (`!out && exile == 0`) standers.
        (0..self.win.seat_count())
            .filter(|&s| {
                s != except
                    && self.out(s) == 0
                    && self.exile(s) == 0
                    && self.pos(s) == tile
            })
            .count() as i64
    }
    fn next_dist(&self, p: i64, dir: i64) -> i64 {
        // meet_again's `next_dist`: `ctx::others` = `!out` (exiled still in).
        let n = self.win.tile_count;
        let pos = self.pos(p);
        if n <= 0 || pos < 0 {
            return -1;
        }
        let mut best = i64::MAX;
        for o in 0..self.win.seat_count() {
            if o == p || self.out(o) != 0 {
                continue;
            }
            let q = self.pos(o);
            if q < 0 {
                continue;
            }
            // C# `Next`: `tile_forward(pos, q)` forward, `tile_forward(q, pos)` back.
            let fwd = {
                let f = ((q - pos) % n + n) % n;
                if dir >= 0 {
                    f
                } else {
                    ((pos - q) % n + n) % n
                }
            };
            if fwd > 0 && fwd < best {
                best = fwd;
            }
        }
        if best == i64::MAX {
            -1
        } else {
            best
        }
    }
    fn others_within(&self, p: i64, radius: i64) -> i64 {
        // haruhikage's `within5`: `ctx::others` (`!out`), `d > 0 && d <= r`.
        let pos = self.pos(p);
        if pos < 0 {
            return 0;
        }
        (0..self.win.seat_count())
            .filter(|&o| {
                if o == p || self.out(o) != 0 {
                    return false;
                }
                let d = self.dist(pos, self.pos(o));
                d > 0 && d <= radius
            })
            .count() as i64
    }
    fn owned_within(&self, p: i64, radius: i64) -> i64 {
        // council_check's `near`: the seat's deeds within `radius` of its pos.
        let pos = self.pos(p);
        if pos < 0 {
            return 0;
        }
        (0..self.win.tile_count)
            .filter(|&t| self.win.tile_owners.get(t as usize).copied().unwrap_or(-1) == p
                && self.dist(pos, t) <= radius)
            .count() as i64
    }
    fn on_path(&self, me: i64, them: i64) -> i64 {
        // repaint's `on_path`: my deeds on `them`'s 1..=|move.roll| forward path.
        let Some(roll) = (self.move_roll() >= 0).then_some(self.move_roll()) else {
            return 0;
        };
        let steps = roll.abs();
        let n = self.win.tile_count;
        let pos = self.pos(them);
        if n <= 0 || pos < 0 {
            return 0;
        }
        (1..=steps)
            .filter(|i| {
                let t = ((pos + i) % n + n) % n;
                self.win.tile_owners.get(t as usize).copied().unwrap_or(-1) == me
            })
            .count() as i64
    }
    fn between(&self, p: i64) -> i64 {
        // misaki_card's `between`: rivals in the move span along `move.dir`.
        let Some(roll) = (self.move_roll() >= 0).then_some(self.move_roll()) else {
            return 0;
        };
        let roll = roll.abs();
        let start = self.pos(p);
        let n = self.win.tile_count;
        if n <= 0 || start < 0 {
            return 0;
        }
        let backward = self.move_dir() < 0;
        (0..self.win.seat_count())
            .filter(|&o| {
                if o == p || self.out(o) != 0 {
                    return false;
                }
                let q = self.pos(o);
                if q < 0 {
                    return false;
                }
                let fwd = ((q - start) % n + n) % n;
                let d = if backward { (n - fwd) % n } else { fwd };
                (1..=roll).contains(&d)
            })
            .count() as i64
    }
}