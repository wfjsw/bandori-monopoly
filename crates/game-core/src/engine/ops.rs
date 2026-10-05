//! The card-facing operation vocabulary (C# `H.*`) over the replayable [`World`].
//!
//! A rules host (`game-rules`) runs card modules against a copy of the world and
//! calls these; they are pure state moves and log lines, so a module's replay
//! stays deterministic. Anything that needs a player decision is *not* here --
//! those go through `Cx::ask`, which halts and replays like any engine routine.

use crate::data::GameData;
use crate::msg::Msg;
use crate::state::{Counter, FieldCard, TileMark};

use super::world::World;

impl World {
    // ------------------------------------------------------------ queries

    pub fn seat_out(&self, seat: i32) -> bool {
        usize::try_from(seat).is_ok_and(|i| self.st.seats.get(i).is_some_and(|s| s.out()))
    }

    pub fn seat_money(&self, seat: i32) -> i32 {
        self.seat(seat).map_or(0, |s| s.money)
    }

    pub fn seat_pos(&self, seat: i32) -> i32 {
        self.seat(seat).map_or(-1, |s| s.pos)
    }

    /// The other seats still in the game (`H.Others`).
    pub fn others(&self, seat: i32) -> Vec<i32> {
        (0..self.st.seats.len() as i32)
            .filter(|&i| i != seat && !self.seat_out(i))
            .collect()
    }

    /// Tile index of a tile name (a data key, without line breaks), or -1.
    pub fn tile_named(&self, data: &GameData, name: &str) -> i32 {
        data.tiles
            .iter()
            .position(|t| t.name.replace('\n', "") == name || t.name == name)
            .map_or(-1, |i| i as i32)
    }

    pub fn tile_owner(&self, tile: i32) -> i32 {
        usize::try_from(tile).ok().and_then(|i| self.st.owners.get(i)).copied().unwrap_or(-1)
    }

    pub fn owned_tiles(&self, seat: i32) -> Vec<i32> {
        (0..self.st.owners.len() as i32).filter(|&t| self.tile_owner(t) == seat).collect()
    }

    /// Rent of a tile as it stands right now (houses included; `H.RentOf`).
    pub fn rent_of(&self, data: &GameData, tile: i32) -> i32 {
        let Some(t) = usize::try_from(tile).ok().and_then(|i| data.tiles.get(i)) else { return 0 };
        let houses = usize::try_from(tile).ok().and_then(|i| self.st.houses.get(i)).copied().unwrap_or(0) as usize;
        if t.kind == "ring" {
            // RiNG rent is rolled at payment time; the table value is the base.
            t.price
        } else {
            t.rent.get(houses.min(t.rent.len().saturating_sub(1))).copied().unwrap_or(0)
        }
    }

    /// Price to buy a tile now (land + houses standing on it; `H.BuyPriceFor`).
    pub fn buy_price(&self, data: &GameData, tile: i32) -> i32 {
        let Some(t) = usize::try_from(tile).ok().and_then(|i| data.tiles.get(i)) else { return 0 };
        let houses = usize::try_from(tile).ok().and_then(|i| self.st.houses.get(i)).copied().unwrap_or(0);
        t.price + houses * t.house
    }

    pub fn build_cost(&self, data: &GameData, tile: i32) -> i32 {
        usize::try_from(tile).ok().and_then(|i| data.tiles.get(i)).map_or(0, |t| t.house)
    }

    pub fn mortgage_value(&self, data: &GameData, tile: i32) -> i32 {
        usize::try_from(tile).ok().and_then(|i| data.tiles.get(i)).map_or(0, |t| t.price / 2)
    }

    /// The tile `steps` ahead of a seat without passing others' logic
    /// (`H.NearestAhead`-lite): pure geometry on the ring.
    pub fn tile_steps_ahead(&self, data: &GameData, seat: i32, steps: i32) -> i32 {
        let n = data.tiles.len() as i32;
        let pos = self.seat_pos(seat);
        if n <= 0 || pos < 0 {
            return -1;
        }
        ((pos + steps) % n + n) % n
    }

    // -------------------------------------------------------- seat slots (V)

    pub fn slot(&self, seat: i32, key: &str) -> i32 {
        self.seat(seat).and_then(|s| s.slots.iter().find(|c| c.name == key)).map_or(0, |c| c.value)
    }

    pub fn set_slot(&mut self, seat: i32, key: &str, value: i32) {
        if let Some(s) = self.seat_mut(seat) {
            set_named(&mut s.slots, key, value);
        }
    }

    pub fn inc_slot(&mut self, seat: i32, key: &str, by: i32) -> i32 {
        let v = self.slot(seat, key) + by;
        self.set_slot(seat, key, v);
        v
    }

    // ------------------------------------------------------------- tokens

    pub fn tok(&self, seat: i32, name: &str) -> i32 {
        self.seat(seat).and_then(|s| s.tokens.iter().find(|c| c.name == name)).map_or(0, |c| c.value)
    }

    pub fn set_tok(&mut self, seat: i32, name: &str, value: i32) {
        if let Some(s) = self.seat_mut(seat) {
            set_named(&mut s.tokens, name, value);
        }
    }

    /// Returns how much it actually moved by (`H.AddTok`).
    pub fn add_tok(&mut self, seat: i32, name: &str, n: i32, max: i32) -> i32 {
        let was = self.tok(seat, name);
        let now = (was + n).clamp(0, max);
        self.set_tok(seat, name, now);
        now - was
    }

    // -------------------------------------------------------- band crystals

    pub fn band_crystals(&self, seat: i32) -> i32 {
        self.seat(seat).map_or(0, |s| s.band_crystals)
    }

    pub fn add_band_crystals(&mut self, seat: i32, n: i32, max: i32) -> i32 {
        let Some(s) = self.seat_mut(seat) else { return 0 };
        let was = s.band_crystals;
        s.band_crystals = (was + n).clamp(0, max);
        s.band_crystals - was
    }

    // ------------------------------------------------------------- fire pots

    pub fn fire(&self, seat: i32) -> i32 {
        self.seat(seat).map_or(0, |s| s.fire)
    }

    pub fn fire_max(&self, seat: i32) -> i32 {
        self.seat(seat).map_or(0, |s| s.fire_max)
    }

    /// Gained `n` (at most one less than the cap) and logged it; 0 if nothing.
    pub fn gain_fire(&mut self, seat: i32, n: i32, why: Msg) -> i32 {
        let Some(s) = self.seat_mut(seat) else { return 0 };
        if n <= 0 || s.out() || s.fire_max <= 0 {
            return 0;
        }
        let cap = s.fire_max;
        let got = n.clamp(0, (cap - s.fire).max(0));
        if got <= 0 {
            return 0;
        }
        s.fire += got;
        let (fire, name) = (s.fire, s.player.clone());
        self.log("fire", seat, Msg::new("log.gain_fire").text("who", name).i("n", got).msg("why", why).i("fire", fire).i("max", cap));
        got
    }

    // ---------------------------------------------------------- money moves

    /// `H.GainR` -- money in, logged with its reason.
    pub fn gain_money(&mut self, seat: i32, amount: i32, src: Msg) -> i32 {
        let Some(s) = self.seat_mut(seat) else { return 0 };
        if amount <= 0 || s.out() {
            return 0;
        }
        s.money += amount;
        self.log("gain", seat, Msg::new("log.gain").seat("who", seat).n("amount", amount).msg("src", Msg::new("log.part.why").msg("why", src)));
        amount
    }

    /// `H.PayR` -- money out, logged. Routines that may need to raise funds are
    /// driven through `Cx` instead (they prompt).
    pub fn pay_money(&mut self, seat: i32, amount: i32, src: Msg) -> i32 {
        let Some(s) = self.seat_mut(seat) else { return 0 };
        if amount <= 0 || s.out() {
            return 0;
        }
        let paid = amount.min(s.money);
        s.money -= paid;
        self.log("lose", seat, Msg::new("log.lose").seat("who", seat).n("amount", paid).msg("src", Msg::new("log.part.why").msg("why", src)));
        paid
    }

    // ------------------------------------------------------------ card flow

    /// `H.DrawR` -- draw `n` cards, reshuffling the discard pile when needed.
    /// Returns how many were actually drawn.
    pub fn draw_cards(&mut self, seat: i32, n: i32, over_hand_limit: bool) -> i32 {
        let Some(i) = usize::try_from(seat).ok() else { return 0 };
        if self.st.seats.get(i).is_none_or(|s| s.out()) {
            return 0;
        }
        let mut got = 0;
        for _ in 0..n.max(0) {
            if self.hidden[i].draw.is_empty() && !self.hidden[i].discard.is_empty() {
                let mut pile = std::mem::take(&mut self.hidden[i].discard);
                self.rng.shuffle(&mut pile);
                self.hidden[i].draw = pile;
                self.log("text", seat, Msg::new("log.reshuffle").seat("who", seat));
            }
            let Some(card) = self.hidden[i].draw.pop() else { break };
            self.hidden[i].hand.push(card);
            got += 1;
        }
        if got > 0 {
            self.log("draw", seat, Msg::new("log.draw").seat("who", seat).i("n", got));
        }
        let _ = over_hand_limit;
        got
    }

    /// `H.AddToHand` / `AddToDeck` / `ToDiscard` -- move a card id between zones.
    pub fn add_to_hand(&mut self, seat: i32, card: &str) {
        if let Ok(i) = usize::try_from(seat) {
            if let Some(h) = self.hidden.get_mut(i) {
                h.hand.push(card.to_string());
            }
        }
    }

    pub fn add_to_deck(&mut self, seat: i32, card: &str, shuffle: bool) {
        if let Ok(i) = usize::try_from(seat) {
            if let Some(h) = self.hidden.get_mut(i) {
                h.draw.push(card.to_string());
                if shuffle {
                    self.rng.shuffle(&mut h.draw);
                }
            }
        }
    }

    pub fn to_discard(&mut self, seat: i32, card: &str) {
        if let Ok(i) = usize::try_from(seat) {
            if let Some(h) = self.hidden.get_mut(i) {
                if let Some(k) = h.hand.iter().position(|c| c == card) {
                    h.hand.remove(k);
                }
                h.discard.push(card.to_string());
            }
        }
    }

    // -------------------------------------------------------- placed cards

    /// `H.PlaceFromPlay` -- the card becomes a field card at the seat.
    pub fn place_card(&mut self, seat: i32, card: &str, note: Msg) {
        let Some(s) = self.seat_mut(seat) else { return };
        let uid = s.field.iter().map(|f| f.uid).max().unwrap_or(0) + 1;
        s.field.push(FieldCard { uid, card: card.to_string(), owner: seat, user: seat, tile: -1, crystals: 0, face_down: false, note });
    }

    /// `H.Unplace` -- take a field card off; returns its card id.
    pub fn unplace_card(&mut self, seat: i32, card: &str) -> Option<String> {
        let s = self.seat_mut(seat)?;
        let k = s.field.iter().position(|f| f.card == card)?;
        Some(s.field.remove(k).card)
    }

    pub fn placed_cards(&self, seat: i32) -> Vec<String> {
        self.seat(seat).map_or_else(Vec::new, |s| s.field.iter().map(|f| f.card.clone()).collect())
    }

    // --------------------------------------------------------------- marks

    /// `H.CountMarks` -- marks on a tile, optionally only one kind/owner.
    pub fn count_marks(&self, tile: i32, kind: &str, owner: i32) -> i32 {
        self.st
            .marks
            .iter()
            .filter(|m| m.tile == tile && (kind.is_empty() || m.kind == kind) && (owner == -2 || m.owner == owner))
            .map(|m| m.count)
            .sum()
    }

    pub fn remove_marks(&mut self, tile: i32, kind: &str, owner: i32) -> i32 {
        let before = self.st.marks.len();
        self.st.marks.retain(|m| !(m.tile == tile && (kind.is_empty() || m.kind == kind) && (owner == -2 || m.owner == owner)));
        (before - self.st.marks.len()) as i32
    }

    pub fn add_mark(&mut self, tile: i32, seat: i32, kind: &str, note: Msg) {
        let uid = self.st.marks.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
        self.st.marks.push(TileMark { uid, tile, kind: kind.to_string(), owner: seat, count: 1, card: String::new(), note });
    }

    // ------------------------------------------------------ status effects

    /// `H.GiveStay` / `GiveStun` / `GiveExile` -- layered status on a seat.
    pub fn give_stay(&mut self, seat: i32, n: i32) {
        if let Some(s) = self.seat_mut(seat) {
            s.stay = (s.stay + n).max(0);
        }
    }

    pub fn give_stun(&mut self, seat: i32, n: i32) {
        if let Some(s) = self.seat_mut(seat) {
            s.stun = (s.stun + n).max(0);
        }
    }

    pub fn give_exile(&mut self, seat: i32, n: i32, to: i32) {
        if let Some(s) = self.seat_mut(seat) {
            s.exile = (s.exile + n).max(0);
            s.exile_to = to;
        }
    }

    /// `H.GiveExtraTurn` -- the seat gets another turn after this one.
    pub fn give_extra_turn(&mut self, seat: usize) {
        if !self.extra_turns.contains(&seat) {
            self.extra_turns.push(seat);
        }
    }

    /// The RiNG rent multiplier in force (`H.RingMultiplier`).
    pub fn ring_multiplier(&self, data: &GameData) -> i32 {
        (data.match_rules.ring_multiplier + self.ring_bonus).max(1)
    }

    pub fn add_ring_bonus(&mut self, n: i32) -> i32 {
        self.ring_bonus += n;
        self.ring_bonus
    }

    /// `H.ForceTeleport(..., resolve: false)` -- move a seat without settling.
    pub fn teleport_to(&mut self, seat: i32, tile: i32) {
        if let Some(s) = self.seat_mut(seat) {
            s.pos = tile;
        }
    }

    // --------------------------------------------------------------- misc

    /// `H.Touch` -- something changed that only host-side state cares about.
    pub fn touch(&mut self) {
        self.st.seq += 1;
    }

    fn seat(&self, seat: i32) -> Option<&crate::state::MatchSeat> {
        usize::try_from(seat).ok().and_then(|i| self.st.seats.get(i))
    }

    fn seat_mut(&mut self, seat: i32) -> Option<&mut crate::state::MatchSeat> {
        usize::try_from(seat).ok().and_then(|i| self.st.seats.get_mut(i))
    }
}

/// Zero-terminated named counters: keep only positive values (`H.SetTok`).
fn set_named(list: &mut Vec<Counter>, name: &str, value: i32) {
    let v = value.max(0);
    match list.iter_mut().find(|c| c.name == name) {
        Some(c) => c.value = v,
        None => {
            if v > 0 {
                list.push(Counter { name: name.to_string(), value: v });
            }
        }
    }
    list.retain(|c| c.value > 0);
}
