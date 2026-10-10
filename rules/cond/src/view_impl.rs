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
    fn card_cp(&self) -> i64 {
        self.cand.card_cp
    }
    fn slot(&self, name: &str) -> i64 {
        self.cand.slot(name)
    }
    fn tok(&self, kind: i64) -> i64 {
        self.cand.tok(kind)
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
    fn tok_table(&self) -> Vec<(i64, i64)> {
        self.cand.toks.iter().map(|(k, v)| (*k, *v)).collect()
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
}