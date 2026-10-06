//! `game-core`'s `CardRules` over the WASM card modules.
//!
//! The engine's routines and a card effect use the same replay model, so the
//! bridge is small: it runs the module against a copy of the world, and when the
//! module blocks on a prompt it raises that prompt as an engine `Ask`. The engine
//! halts the routine, collects the answer, and re-runs it -- which re-runs the
//! module too, rebuilding the answer list line by line as the prompts come up
//! again. Dice, events and prompts stay deterministic because the world copy
//! carries the RNG (see the replay note in `host.rs`).
//!
//! ```text
//! routine -> rules.play -> module run -> NeedInput(p)
//!                ^                        |
//!                |  cx.ask(p) = Halt ------+  (answer arrives, routine re-runs)
//! ```

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{Ask, CardRules, Cx, Dest, Flow, Trigger as CoreTrigger};
use game_core::msg::Msg;

use crate::host::{Call, HostRequest, Outcome, Prompt, PromptOption, RuleError, Ruleset};
use crate::world::{CardWorld, Trigger};
use crate::PromptKind;
use crate::{CardPile, TriggerKind};

/// One run of a card module against a copy of the match world.
/// C# `PlayCtx.Dest` values the module returns.
const DEST_GRAVEYARD: i32 = 0;
const DEST_FIELD: i32 = 2;
/// The run never named a fate. Distinct from [`DEST_GRAVEYARD`] on purpose: a
/// play with no opinion still lands in the discard (`dest_from`'s wildcard),
/// but a field effect with no opinion must leave its card where it is.
const DEST_UNSET: i32 = -1;

/// C# `MatchHost._reactDepth > 4` is a runaway net; a counter-war is bounded by
/// hands shrinking as cards declare. 16 is plenty for a legal exchange.
const MAX_REACT_DEPTH: u32 = 16;

#[derive(Clone)]
struct Run {
    world: game_core::engine::World,
    data: Arc<GameData>,
    trigger: Trigger,
    /// The card whose effect is running (`PlaceFromPlay(c)` / `Unplace(this)`).
    current_card: String,
    /// The **instance** this run is for. A card's name is not an identity --
    /// one player may hold several copies of the same card in play -- so the
    /// uid is. It is the one the dispatch named, and it follows the card if
    /// this run re-places it (C# `PlaceFromPlay(c)` moves the same object).
    current_uid: i32,
    /// `PlayCtx.Dest`, set by the module (`set_dest`).
    dest: i32,
    /// Whose pile the fate lands in, set by `set_transfer_to_dest`. `None` is
    /// the owner the instance leaves -- what `set_dest` names.
    dest_to: Option<i32>,
    /// Payments this run actually made, `(from, to, amount)` -- the engine
    /// raises `payAfter` / `paid` for each once the run commits.
    paid_log: Vec<(i32, i32, i32)>,
    /// Cards this run put into a discard pile, `(player_id, id)` -- the engine
    /// raises `discarded` for each once the run commits (C# `Discarded`).
    discard_log: Vec<(i32, String)>,
    /// Players whose deck this run shuffled -- `reshuffled` once it commits.
    reshuffle_log: Vec<i32>,
    /// Fire spent during the run, raised as `fireSpent` once the run commits
    /// (same shape as `paid_log`).
    fire_spent_log: Vec<(i32, i32)>,
    /// Houses added during the run, raised as `houseAdded` once it commits.
    house_log: Vec<(i32, i32, i32)>,
    /// Miracle-crystal writes this run made, `(owner, card, change)` -- the
    /// engine raises `crystalsChanged` for each once the run commits. `change`
    /// is the signed delta the write applied (0 = a write that landed on the
    /// same count, e.g. a card placed with none); the count it left is the
    /// instance's own `crystals()`.
    crystals_log: Vec<(i32, String, i32)>,
    /// C# `PlayCtx.Doubled`: which of the card's numbers this play doubles, or
    /// -1. Set by the doubling band skill, which is not ported yet.
    doubled: i32,
}

impl Run {
    /// Record a crystal write on the instance at `uid` so the commit point can
    /// raise `crystalsChanged` (the same shape as `fire_spent_log` /
    /// `house_log`). `was` and `now` bracket the write; only the delta rides
    /// the trigger, so a write that lands on the same count still raises --
    /// a card placed with no crystals has to hear about its own count. A write
    /// to an instance that is not on a field raises nothing.
    fn note_crystals(&mut self, uid: i32, was: i32, now: i32) {
        let Some(f) = self.world.field_by_uid(uid) else {
            return;
        };
        let (owner, card) = (f.owner, f.card.clone());
        self.crystals_log.push((owner, card, now - was));
    }

    /// Move the running instance to `dest` **now** -- `H.Unplace(this, "discard")`
    /// and kin, applied mid-effect rather than at the run's commit, for a card
    /// that must be gone before the rest of the effect runs. `to` is whose pile
    /// it lands in; `None` is the owner it leaves. Returns that owner, or `None`
    /// when it was not in play. Clears `current_uid`, so a later deferred fate
    /// finds no instance to move.
    fn move_now(&mut self, to: Option<i32>, dest: i32) -> Option<i32> {
        let left = self.unplace_card();
        if left < 0 {
            return None;
        }
        let who = to.unwrap_or(left);
        let card = self.current_card.clone();
        if dest == DEST_GRAVEYARD {
            self.to_discard(who, &card);
        } else if dest == 1 {
            self.add_to_hand(who, &card);
        }
        // Banished (「[移除]」) is just "gone" -- it left above and goes nowhere.
        Some(left)
    }
}

impl CardWorld for Run {
    fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32 {
        self.world.roll(player_id, count, sides)
    }

    fn effect(&mut self, player_id: i32, msg: Msg) {
        self.world.log("effect", player_id, msg);
    }

    fn extreme(&self) -> i32 {
        self.world.turn.extreme
    }
    fn set_extreme(&mut self, v: i32) {
        self.world.turn.extreme = v.signum();
    }
    fn play_from_hand(&self) -> bool {
        self.world.turn.play_from_hand
    }
    fn set_play_from_hand(&mut self, v: bool) {
        self.world.turn.play_from_hand = v;
    }
    fn gain_fixed(&mut self, player_id: i32, amount: i32, why: crate::Msg) -> i32 {
        self.world.gain_fixed(player_id, amount, why)
    }
    fn set_card_face_down(&mut self, player_id: i32, card: &str, down: bool) -> bool {
        self.world.set_card_face_down(player_id, card, down)
    }

    fn do_move_roll(&mut self, player_id: i32) -> i32 {
        self.world.do_move_roll(player_id)
    }

    fn log(&mut self, player_id: i32, msg: Msg) {
        self.world.log("text", player_id, msg);
    }

    // board -----------------------------------------------------------------
    fn tile_count(&self) -> i32 {
        self.data.tiles.len() as i32
    }
    fn tile_named(&self, name: &str) -> i32 {
        self.world.tile_named(&self.data, name)
    }
    fn tile_owner(&self, tile: i32) -> i32 {
        self.world.tile_owner(tile)
    }
    fn player_pos(&self, player_id: i32) -> i32 {
        self.world.player_pos(player_id)
    }
    fn tile_steps_ahead(&self, player_id: i32, steps: i32) -> i32 {
        self.world.tile_steps_ahead(&self.data, player_id, steps)
    }
    fn rent_of(&self, tile: i32) -> i32 {
        self.world.rent_of(&self.data, tile)
    }
    fn buy_price(&self, tile: i32) -> i32 {
        self.world.buy_price(&self.data, tile)
    }
    fn build_cost(&self, tile: i32) -> i32 {
        self.world.build_cost(&self.data, tile)
    }
    fn mortgage_value(&self, tile: i32) -> i32 {
        self.world.mortgage_value(&self.data, tile)
    }
    fn owned_count(&self, player_id: i32) -> i32 {
        self.world.owned_tiles(player_id).len() as i32
    }
    fn owned_at(&self, player_id: i32, index: i32) -> i32 {
        self.world
            .owned_tiles(player_id)
            .get(index.max(0) as usize)
            .copied()
            .unwrap_or(-1)
    }

    // players -----------------------------------------------------------------
    fn player_count(&self) -> i32 {
        self.world.st.players.len() as i32
    }
    fn player_out(&self, player_id: i32) -> i32 {
        self.world.player_out(player_id) as i32
    }
    fn others_count(&self, player_id: i32) -> i32 {
        self.world.others(player_id).len() as i32
    }
    fn others_at(&self, player_id: i32, index: i32) -> i32 {
        self.world
            .others(player_id)
            .get(index.max(0) as usize)
            .copied()
            .unwrap_or(-1)
    }
    fn money(&self, player_id: i32) -> i32 {
        self.world.player_money(player_id)
    }
    fn gain(&mut self, player_id: i32, amount: i32, src: Msg) -> i32 {
        self.world.gain_money(player_id, amount, src)
    }
    fn pay(&mut self, player_id: i32, amount: i32, src: Msg) -> i32 {
        let paid = self.world.pay_money(player_id, amount, src);
        if paid > 0 {
            self.paid_log.push((player_id, -1, paid));
        }
        paid
    }

    // hand / deck -----------------------------------------------------------
    fn draw(&mut self, player_id: i32, n: i32) -> i32 {
        self.world.draw_cards(player_id, n, true)
    }
    fn add_to_hand(&mut self, player_id: i32, card: &str) {
        self.world.add_to_hand(player_id, card);
    }
    fn take_card(&mut self, player_id: i32, pile: CardPile, card: &str) -> bool {
        let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) else {
            return false;
        };
        let list = match pile {
            CardPile::Hand => &mut h.hand,
            CardPile::Discard => &mut h.discard,
            CardPile::Deck => &mut h.draw,
            CardPile::Field => return self.world.unplace_card(player_id, card).is_some(),
        };
        match list.iter().position(|c| c == card) {
            Some(k) => {
                list.remove(k);
                true
            }
            None => false,
        }
    }
    fn cards_in(&self, player_id: i32, pile: CardPile) -> Vec<String> {
        let Some(h) = self.world.hidden.get(player_id.max(0) as usize) else {
            return Vec::new();
        };
        match pile {
            CardPile::Hand => h.hand.clone(),
            CardPile::Discard => h.discard.clone(),
            // The end of the vec is the top of the pile; list top first.
            CardPile::Deck => h.draw.iter().rev().cloned().collect(),
            CardPile::Field => self.world.placed_cards(player_id),
        }
    }
    fn add_to_deck(&mut self, player_id: i32, card: &str, shuffle: bool) {
        self.world.add_to_deck(player_id, card, shuffle);
    }
    fn add_to_deck_at(&mut self, player_id: i32, card: &str, pos: i32) {
        // C# `H.AddToDeck(seat, card, where)`: "top" = draw.Add (the end of the
        // vec is the top), "bottom" = Insert(0), anything else = add + shuffle.
        let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) else {
            return;
        };
        match pos {
            1 => h.draw.insert(0, card.to_string()),
            2 => {
                h.draw.push(card.to_string());
                self.world.rng.shuffle(&mut h.draw);
            }
            _ => h.draw.push(card.to_string()),
        }
    }
    fn to_discard(&mut self, player_id: i32, card: &str) {
        self.world.to_discard(player_id, card);
        self.discard_log.push((player_id, card.to_string()));
    }

    // field cards -----------------------------------------------------------
    fn can_build_on(&self, player_id: i32, tile: i32) -> bool {
        self.world
            .why_not_build_on(&self.data, player_id, tile)
            .is_none()
    }
    fn card_face_down(&self, player_id: i32, card: &str) -> bool {
        self.world.card_face_down(player_id, card)
    }
    fn place_card_on(&mut self, player_id: i32, tile: i32, card: &str, note: Msg) -> i32 {
        let uid = self.world.place_card_on(player_id, tile, card, note);
        if card == self.current_card {
            self.current_uid = uid;
        }
        uid
    }
    fn place_card(&mut self, player_id: i32, card: &str, note: Msg) -> i32 {
        let uid = self.world.place_card(player_id, card, note);
        if card == self.current_card {
            self.current_uid = uid;
        }
        uid
    }
    fn set_dest(&mut self, dest: i32) {
        self.dest = dest;
        self.dest_to = None;
    }
    fn set_transfer_to_dest(&mut self, to: i32, dest: i32) {
        self.dest = dest;
        self.dest_to = Some(to);
    }
    fn send_to_dest(&mut self, dest: i32) -> Option<i32> {
        self.move_now(None, dest)
    }
    fn transfer_to_dest(&mut self, to: i32, dest: i32) -> Option<i32> {
        self.move_now(Some(to), dest)
    }
    fn ring_multiplier(&self) -> i32 {
        self.world.ring_multiplier(&self.data)
    }
    fn add_ring_bonus(&mut self, n: i32) -> i32 {
        self.world.add_ring_bonus(n)
    }
    fn teleport_to(&mut self, player_id: i32, tile: i32) {
        self.world.teleport_to(player_id, tile);
    }
    fn unplace_card_named(&mut self, player_id: i32, card: &str) -> bool {
        self.world.unplace_card(player_id, card).is_some()
    }
    fn unplace_card(&mut self) -> i32 {
        let owner = self.world.unplace_at(self.current_uid);
        if owner >= 0 {
            self.current_uid = -1;
        }
        owner
    }
    fn self_tile(&self) -> i32 {
        if self.current_uid < 0 {
            return -2;
        }
        self.world.tile_at(self.current_uid)
    }
    fn set_self_tile(&mut self, tile: i32) -> bool {
        self.world.set_tile_at(self.current_uid, tile)
    }
    fn self_face_down(&self) -> bool {
        self.world.is_face_down_at(self.current_uid)
    }
    fn set_self_face_down(&mut self, on: bool) -> bool {
        self.world.set_face_down_at(self.current_uid, on)
    }
    fn self_immune(&self) -> bool {
        self.world.is_immune_at(self.current_uid)
    }
    fn set_self_immune(&mut self, on: bool) -> bool {
        self.world.set_immune_at(self.current_uid, on)
    }
    fn placed_cards(&self, player_id: i32) -> Vec<String> {
        self.world.placed_cards(player_id)
    }
    fn card_text_mentions(&self, card: &str, needle: &str) -> bool {
        self.data
            .card(card)
            .is_some_and(|c| c.text.contains(needle))
    }
    fn card_crystals(&self, player_id: i32, card: &str) -> i32 {
        self.world.card_crystals(player_id, card)
    }
    fn add_card_crystals(&mut self, player_id: i32, card: &str, n: i32, max: i32) -> i32 {
        let was = self.world.card_crystals(player_id, card);
        // Which instance the name resolves to, so the raise names the right one.
        let uid = self
            .world
            .field_instances(player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
        let now = self.world.add_card_crystals(player_id, card, n, max);
        self.note_crystals(uid, was, now);
        now
    }
    /// Is the running **instance** in play?
    ///
    /// This used to ask whether *some* card with the running card's name sat at
    /// `player_id`, which under the uid model answers the wrong question: with
    /// two copies of a card on one field, the copy that is not running would
    /// still pass the gate. It is the instance now, so it takes no locator --
    /// the run already knows which instance it is.
    fn is_placed(&self) -> i32 {
        (self.current_uid >= 0 && self.world.field_by_uid(self.current_uid).is_some()) as i32
    }
    fn crystals(&self) -> i32 {
        self.world.crystals_at(self.current_uid)
    }
    fn set_crystals(&mut self, n: i32) -> i32 {
        let was = self.world.crystals_at(self.current_uid);
        let now = self.world.set_crystals_at(self.current_uid, n);
        self.note_crystals(self.current_uid, was, now);
        now
    }
    fn add_crystals(&mut self, n: i32, max: i32) -> i32 {
        let was = self.world.crystals_at(self.current_uid);
        let now = self.world.add_crystals_at(self.current_uid, n, max);
        self.note_crystals(self.current_uid, was, now);
        now
    }
    fn field_instances(&self, player_id: i32) -> Vec<(i32, String)> {
        self.world.field_instances(player_id)
    }
    fn crystals_at(&self, uid: i32) -> i32 {
        self.world.crystals_at(uid)
    }
    fn add_crystals_at(&mut self, uid: i32, n: i32, max: i32) -> i32 {
        let was = self.world.crystals_at(uid);
        let now = self.world.add_crystals_at(uid, n, max);
        self.note_crystals(uid, was, now);
        now
    }
    fn unplace_at(&mut self, uid: i32) -> i32 {
        self.world.unplace_at(uid)
    }
    fn tile_at(&self, uid: i32) -> i32 {
        self.world.tile_at(uid)
    }
    fn set_tile_at(&mut self, uid: i32, tile: i32) -> bool {
        self.world.set_tile_at(uid, tile)
    }
    fn is_face_down_at(&self, uid: i32) -> bool {
        self.world.is_face_down_at(uid)
    }
    fn set_face_down_at(&mut self, uid: i32, on: bool) -> bool {
        self.world.set_face_down_at(uid, on)
    }
    fn is_immune_at(&self, uid: i32) -> bool {
        self.world.is_immune_at(uid)
    }
    fn set_immune_at(&mut self, uid: i32, on: bool) -> bool {
        self.world.set_immune_at(uid, on)
    }

    // marks & tokens --------------------------------------------------------
    fn add_mark(&mut self, tile: i32, player_id: i32, kind: &str, note: Msg) {
        self.world.add_mark(tile, player_id, kind, note);
    }
    fn count_marks(&self, tile: i32, kind: &str, owner: i32) -> i32 {
        self.world.count_marks(tile, kind, owner)
    }
    fn set_card_immune(&mut self, player_id: i32, card: &str, on: bool) -> bool {
        self.world.set_card_immune(player_id, card, on)
    }
    fn card_immune(&self, player_id: i32, card: &str) -> bool {
        self.world.card_immune(player_id, card)
    }
    fn set_card_tile(&mut self, player_id: i32, card: &str, tile: i32) -> bool {
        self.world.set_card_tile(player_id, card, tile)
    }
    fn bump_mark(&mut self, tile: i32, kind: &str, owner: i32, delta: i32) -> i32 {
        self.world.bump_mark(tile, kind, owner, delta)
    }
    fn remove_marks(&mut self, tile: i32, kind: &str, owner: i32) -> i32 {
        self.world.remove_marks(tile, kind, owner)
    }
    fn tok_names(&self, player_id: i32, prefix: &str) -> Vec<String> {
        self.world.tok_names(player_id, prefix)
    }
    fn tok(&self, player_id: i32, name: &str) -> i32 {
        self.world.tok(player_id, name)
    }
    fn set_tok(&mut self, player_id: i32, name: &str, value: i32) {
        self.world.set_tok(player_id, name, value);
    }
    fn add_tok(&mut self, player_id: i32, name: &str, n: i32, max: i32) -> i32 {
        self.world.add_tok(player_id, name, n, max)
    }

    // keyed state ------------------------------------------------------------
    fn state_var(&self, player_id: i32, key: &str) -> game_core::state::StateVar {
        self.world.state_var(player_id, key)
    }
    fn state_get(&self, player_id: i32, key: &str) -> i32 {
        self.world.state_get(player_id, key)
    }
    fn state_min(&self, player_id: i32, key: &str) -> i32 {
        self.world.state_min(player_id, key)
    }
    fn state_max(&self, player_id: i32, key: &str) -> i32 {
        self.world.state_max(player_id, key)
    }
    fn state_expires(&self, player_id: i32, key: &str) -> Option<game_core::state::Tick> {
        self.world.state_expires(player_id, key)
    }
    fn state_set(&mut self, player_id: i32, key: &str, value: i32) -> i32 {
        self.world.state_set(player_id, key, value)
    }
    fn state_add(&mut self, player_id: i32, key: &str, delta: i32) -> i32 {
        self.world.state_add(player_id, key, delta)
    }
    fn state_set_bounds(&mut self, player_id: i32, key: &str, min: i32, max: i32) {
        self.world.state_set_bounds(player_id, key, min, max);
    }
    fn state_set_expires(
        &mut self,
        player_id: i32,
        key: &str,
        expires: Option<game_core::state::Tick>,
    ) {
        self.world.state_set_expires(player_id, key, expires);
    }
    fn tick_state(&mut self, player_id: i32, when: game_core::state::Tick) -> Vec<(String, i32)> {
        self.world.tick_state(player_id, when)
    }

    // status pots -----------------------------------------------------------
    fn add_band_crystals(&mut self, player_id: i32, n: i32, max: i32) -> i32 {
        self.world.add_band_crystals(player_id, n, max)
    }
    fn gain_fire(&mut self, player_id: i32, n: i32, why: Msg) -> i32 {
        self.world.gain_fire(player_id, n, why)
    }
    fn give_stay(&mut self, player_id: i32, n: i32) {
        self.world.give_stay(player_id, n);
    }
    fn give_stun(&mut self, player_id: i32, n: i32) {
        self.world.give_stun(player_id, n);
    }
    fn give_exile(&mut self, player_id: i32, n: i32, to: i32) {
        self.world.give_exile(player_id, n, to);
    }
    fn give_extra_turn(&mut self, player_id: i32) {
        self.world.give_extra_turn(player_id as usize);
    }

    // board extensions ------------------------------------------------------
    fn is_buyable(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.is_buyable()) as i32
    }
    fn is_shop(&self, tile: i32) -> i32 {
        // C# `H.IsShop`: buyable and colour group 10 (the 商店街 deeds).
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.is_buyable() && t.group == 10) as i32
    }
    fn is_ring(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.kind == "ring") as i32
    }
    fn is_circle(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.kind == "circle") as i32
    }
    fn set_tile_color(&mut self, tile: i32, group: i32) {
        self.world.set_tile_color(tile, group)
    }
    fn set_extra_color(&mut self, player_id: i32, tile: i32, group: i32) {
        self.world.set_extra_color(player_id, tile, group)
    }
    fn is_color(&self, player_id: i32, tile: i32, group: i32) -> bool {
        if let Some(g) = self.world.color_override(player_id, tile) {
            return g == group || g == game_core::state::key::ALL_COLORS;
        }
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.group == group)
    }
    fn set_buy_discount(&mut self, n: i32) {
        self.world.turn.buy_discount = n.max(0);
    }
    fn paid_in_settle(&self) -> i32 {
        self.world.turn.paid_in_settle
    }
    fn set_free_buy(&mut self, on: bool) {
        self.world.turn.free_buy = on;
    }
    fn set_raze_on_buy(&mut self, on: bool) {
        self.world.turn.raze_on_buy = on;
    }
    fn turn_start_pos(&self, player_id: i32) -> i32 {
        self.world
            .turn
            .turn_start_pos
            .get(player_id.max(0) as usize)
            .copied()
            .unwrap_or(-1)
    }
    fn turn_rolls(&self) -> Vec<i32> {
        self.world.turn.turn_rolls.clone()
    }
    fn set_build_discount(&mut self, n: i32, layers: i32) {
        self.world.turn.build_discount = n.max(0);
        self.world.turn.build_discount_layers = layers.max(0);
    }
    fn set_build_cost_pct(&mut self, pct: i32) {
        self.world.turn.build_cost_pct = pct.clamp(0, 100);
    }
    fn turn_snap(&self, player_id: i32) -> (i32, i32, i32, i32) {
        self.world
            .turn
            .turn_snap
            .get(player_id.max(0) as usize)
            .map(|s| (s.pos, s.stay, s.stun, s.exile))
            .unwrap_or((-1, 0, 0, 0))
    }
    fn is_agent(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.kind == "agent") as i32
    }
    fn is_live_house(&self, tile: i32) -> i32 {
        // C# `H.IsLiveHouse` = `IsColor(t, 6) && IsBuyable(t)`. The `ExtraColor`
        // side is a per-player override, so see `is_live_house_for`.
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.is_buyable() && t.group == 6) as i32
    }
    fn is_live_house_for(&self, player_id: i32, tile: i32) -> i32 {
        (self.is_color(player_id, tile, 6)
            && self
                .data
                .tiles
                .get(tile.max(0) as usize)
                .is_some_and(|t| t.is_buyable())) as i32
    }
    fn tile_group(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.group)
            .unwrap_or(-1)
    }
    fn tile_price(&self, tile: i32) -> i32 {
        // `H._tiles[t].price` -- land alone (`buy_price` adds houses).
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.price)
            .unwrap_or(0)
    }
    fn houses_of(&self, tile: i32) -> i32 {
        self.world
            .st
            .houses
            .get(tile.max(0) as usize)
            .copied()
            .unwrap_or(0)
    }
    fn set_houses(&mut self, tile: i32, n: i32) {
        let cap = self
            .data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.rent.len().saturating_sub(1))
            .unwrap_or(0) as i32;
        if let Some(h) = self.world.st.houses.get_mut(tile.max(0) as usize) {
            *h = n.clamp(0, cap);
        }
    }
    fn add_house(&mut self, tile: i32, n: i32) -> i32 {
        // C# `AddHouse` caps at `rent.Length - 1` (the board's house max; RiNG
        // deeds have no rent table and never hold houses).
        let cap = self
            .data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.rent.len().saturating_sub(1))
            .unwrap_or(0) as i32;
        if let Some(h) = self.world.st.houses.get_mut(tile.max(0) as usize) {
            *h = (*h + n).clamp(0, cap);
            let now = *h;
            if n > 0 {
                let owner = self
                    .world
                    .st
                    .owners
                    .get(tile.max(0) as usize)
                    .copied()
                    .unwrap_or(-1);
                if owner >= 0 {
                    self.house_log.push((owner, tile, now));
                }
            }
            return now;
        }
        0
    }
    fn mortgaged_of(&self, tile: i32) -> i32 {
        self.world
            .st
            .mortgaged
            .get(tile.max(0) as usize)
            .copied()
            .unwrap_or(false) as i32
    }
    fn set_mortgaged(&mut self, tile: i32, v: i32) {
        if let Some(m) = self.world.st.mortgaged.get_mut(tile.max(0) as usize) {
            *m = v != 0;
        }
    }
    fn set_owner(&mut self, tile: i32, player_id: i32) {
        if let Some(o) = self.world.st.owners.get_mut(tile.max(0) as usize) {
            *o = player_id;
        }
    }
    fn dist(&self, a: i32, b: i32) -> i32 {
        // C# `H.Dist` -- the shorter way around the ring.
        let n = self.data.tiles.len() as i32;
        if n <= 0 {
            return 0;
        }
        let d = ((b - a) % n + n) % n;
        d.min(n - d)
    }
    fn tile_forward(&self, a: i32, b: i32) -> i32 {
        // C# `H.Forward` -- steps forward from a to b.
        let n = self.data.tiles.len() as i32;
        if n <= 0 {
            return 0;
        }
        ((b - a) % n + n) % n
    }
    fn neighbor(&self, player_id: i32, dir: i32) -> i32 {
        // C# `H.Neighbor` -- the next present player in turn order, wrapping.
        let n = self.world.st.players.len();
        if n == 0 {
            return -1;
        }
        let dir = if dir < 0 { -1 } else { 1 };
        for j in 1..n {
            let k = ((player_id + dir * j as i32) % n as i32 + n as i32) % n as i32;
            if self.present(k) {
                return k;
            }
        }
        -1
    }
    fn players_on_count(&self, tile: i32, except: i32) -> i32 {
        self.players_on_list(tile, except).len() as i32
    }
    fn players_on_at(&self, tile: i32, except: i32, index: i32) -> i32 {
        self.players_on_list(tile, except)
            .get(index.max(0) as usize)
            .copied()
            .unwrap_or(-1)
    }

    // hand & deck extensions -----------------------------------------------
    fn hand_count(&self, player_id: i32, card: &str) -> i32 {
        self.hands(player_id)
            .map(|h| h.iter().filter(|c| c.as_str() == card).count() as i32)
            .unwrap_or(0)
    }
    fn hand_size(&self, player_id: i32) -> i32 {
        self.hands(player_id).map(|h| h.len() as i32).unwrap_or(0)
    }
    fn discard_count(&self, player_id: i32, card: &str) -> i32 {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.discard.iter().filter(|c| c.as_str() == card).count() as i32)
            .unwrap_or(0)
    }
    fn deck_count(&self, player_id: i32) -> i32 {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.draw.len() as i32)
            .unwrap_or(0)
    }
    fn discard_size(&self, player_id: i32) -> i32 {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.discard.len() as i32)
            .unwrap_or(0)
    }
    fn discard_from_hand(&mut self, player_id: i32, card: &str) -> i32 {
        // C# `H.DiscardFromHand` -- one copy, hand -> discard pile.
        let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) else {
            return 0;
        };
        let Some(p) = h.hand.iter().position(|c| c == card) else {
            return 0;
        };
        h.hand.remove(p);
        h.discard.push(card.to_string());
        self.discard_log.push((player_id, card.to_string()));
        1
    }
    fn shuffle_into_deck(&mut self, player_id: i32, hand: bool, discard: bool) -> i32 {
        // C# `H.ShuffleAllIntoDeck(seat, hand, discard)`.
        let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) else {
            return 0;
        };
        let mut moved: Vec<String> = Vec::new();
        if hand {
            moved.append(&mut h.hand);
        }
        if discard {
            moved.append(&mut h.discard);
        }
        let n = moved.len() as i32;
        h.draw.append(&mut moved);
        self.world.rng.shuffle(&mut h.draw);
        self.reshuffle_log.push(player_id);
        n
    }
    fn abnormal_count(&self, player_id: i32) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.world.turn.abnormal.get(s).copied())
            .unwrap_or(0)
    }
    fn targeted_count(&self, player_id: i32) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.world.targeted.get(s).copied())
            .unwrap_or(0)
    }
    fn placed_tile(&self, player_id: i32, id: &str) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.world.st.players.get(s))
            .and_then(|s| s.field.iter().find(|f| f.card == id))
            .map_or(-2, |f| f.tile.max(-1))
    }
    fn play_doubled(&self) -> i32 {
        self.doubled
    }

    fn set_play_doubled(&mut self, n: i32) {
        self.doubled = n;
    }

    // status extensions -----------------------------------------------------
    fn can_pay(&self, player_id: i32) -> i32 {
        // C# `H.CanPay`: not out, not stunned, not exiled.
        let Some(s) = self.world.st.players.get(player_id.max(0) as usize) else {
            return 0;
        };
        (!s.out() && !s.stunned() && s.exile() == 0) as i32
    }
    fn cant_move(&self, player_id: i32) -> i32 {
        // C# `H.MoveWhyNot`.
        if self.world.st.turn != player_id {
            return 1;
        }
        if self.world.turn.main_moved {
            return 2;
        }
        if self.world.st.skip_move {
            return 3;
        }
        0
    }
    fn spend_fire(&mut self, player_id: i32, n: i32, why: Msg) -> i32 {
        // C# `H.SpendFire` -- pots first; logs the spend with its reason (the
        // caller's message is the log line, like `gain`/`pay`).
        if n <= 0 {
            return 1;
        }
        let Some(s) = self.world.st.players.get_mut(player_id.max(0) as usize) else {
            return 0;
        };
        // A consumer of the pots: all-or-nothing, and it logs the spend. It does
        // not consult any cap -- spending is not the capped direction.
        if s.fire() < n {
            return 0;
        }
        s.state_add(game_core::state::key::FIRE, -n);
        self.world.log("fire", player_id, why).value = -n;
        self.fire_spent_log.push((player_id, n));
        1
    }
    fn turn_player(&self) -> i32 {
        self.world.st.turn
    }
    fn round_no(&self) -> i32 {
        self.world.st.round
    }
    fn turn_key(&self) -> i32 {
        // C# `H.TurnKey => State.round * 100 + State.turn + 1`.
        self.world.st.round * 100 + self.world.st.turn + 1
    }
    fn character_is(&self, player_id: i32, name: &str) -> i32 {
        self.world
            .st
            .players
            .get(player_id.max(0) as usize)
            .is_some_and(|s| s.character == name) as i32
    }
    fn in_band(&self, player_id: i32, name: &str) -> i32 {
        // C# `H.BandOf(seat)` -- the character's band.
        let Some(s) = self.world.st.players.get(player_id.max(0) as usize) else {
            return 0;
        };
        self.data
            .characters
            .iter()
            .find(|c| c.name == s.character)
            .is_some_and(|c| c.band == name) as i32
    }

    // trigger ---------------------------------------------------------------
    fn trigger(&self) -> Trigger {
        self.trigger.clone()
    }
    fn set_trigger_move_roll(&mut self, roll: i32) {
        self.trigger.move_roll = Some(roll);
    }
    fn set_trigger_value(&mut self, value: i32) {
        self.trigger.value = value;
    }
    fn set_trigger_target(&mut self, to: i32) {
        self.trigger.target = to;
    }
    fn set_trigger_cancelled(&mut self) {
        self.trigger.negate_activation();
    }
    fn set_trigger_negate_effect(&mut self) {
        self.trigger.negate_effect();
    }
    fn set_trigger_spare(&mut self, seat: i32) {
        self.trigger.spare(seat);
    }
    fn declare_trigger_effect(&mut self, kind: i32, target: i32, from: i32, tile: i32, value: i32) {
        self.trigger.declare(game_core::engine::rules::Effect {
            kind: card_sdk::abi::TriggerKind::from_i32(kind).as_str(),
            target,
            from,
            tile,
            value,
        });
    }
    fn trig_card_is(&self, id: &str) -> i32 {
        (self.trigger.card == id) as i32
    }

    // turn plan & scheduling -------------------------------------------------
    fn schedule_turn_end(&mut self, player_id: i32, next_of_player: bool, early: bool) {
        let now = self.world.turn.player_id;
        let (target, skip) = if next_of_player {
            let s = player_id.max(0) as usize;
            // Scheduled during that player's own turn: let this turn end pass.
            (s, s == now)
        } else {
            (now, false)
        };
        self.world.scheduled.push(game_core::engine::Scheduled {
            card: self.current_card.clone(),
            owner: player_id,
            target,
            skip,
            early,
            // The instance asking for the callback, captured now -- the C#
            // `AtEnd.Add(() => ...)` closure captures that card object, so the
            // identity belongs to the entry rather than being resolved from the
            // field at fire time (which copy would it be?). `-1` when the card
            // is not in play: `On::AtEnd` may run for a card in a hand or pile.
            uid: self.current_uid,
        });
    }
    fn set_no_money_loss(&mut self, player_id: i32) {
        if let Ok(s) = usize::try_from(player_id) {
            if !self.world.turn.no_money_loss.contains(&s) {
                self.world.turn.no_money_loss.push(s);
            }
        }
    }
    fn set_fixed_roll(&mut self, n: i32) {
        self.world.turn.fixed_roll = Some(n.max(0));
    }
    fn fixed_roll(&self) -> i32 {
        self.world.turn.fixed_roll.unwrap_or(-1)
    }
    fn set_next_steps(&mut self, player_id: i32, n: i32) {
        if let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) {
            h.next_steps = Some(n.max(0));
        }
    }
    fn turn_main_steps(&self) -> i32 {
        self.world.turn.main_steps
    }
    fn add_fire_max(&mut self, player_id: i32, n: i32) -> i32 {
        self.world.add_fire_max(player_id, n)
    }
    fn enter_card(&mut self, id: &str) -> (String, i32, Option<i32>, i32) {
        let card = std::mem::replace(&mut self.current_card, id.to_string());
        let dest = std::mem::replace(&mut self.dest, DEST_UNSET);
        let dest_to = std::mem::replace(&mut self.dest_to, None);
        // Fresh instance (C# `NewCard`): the nested run is not the outer card's
        // field card, so it starts with no uid of its own until it places one.
        let uid = std::mem::replace(&mut self.current_uid, -1);
        (card, dest, dest_to, uid)
    }
    fn leave_card(&mut self, saved: (String, i32, Option<i32>, i32)) -> i32 {
        self.current_card = saved.0;
        self.current_uid = saved.3;
        self.dest_to = saved.2;
        std::mem::replace(&mut self.dest, saved.1)
    }
    // movement shaping ---------------------------------------------------------
    fn set_steps(&mut self, v: i32) {
        self.world.set_steps(v);
    }
    fn set_reverse(&mut self, v: bool) {
        self.world.set_reverse(v);
    }
    fn set_signed(&mut self, v: bool) {
        self.world.set_signed(v);
    }
    fn set_stop_at(&mut self, v: i32) {
        self.world.set_stop_at(v);
    }
    fn set_parity(&mut self, v: i32) {
        self.world.set_parity(v);
    }
    fn set_resolve(&mut self, v: bool) {
        self.world.set_resolve(v);
    }
    fn set_no_buy(&mut self, v: bool) {
        self.world.set_no_buy(v);
    }
    fn set_kind(&mut self, v: i32) {
        self.world.set_kind(v);
    }
    fn set_tag(&mut self, key: &str, v: i32) {
        self.world.set_tag(key, v);
    }
    fn move_tag(&self, key: &str) -> i32 {
        self.world.move_tag(key)
    }
    fn set_min_roll(&mut self, v: i32) {
        self.world.set_min_roll(v);
    }
    fn set_extra_steps(&mut self, v: i32) {
        self.world.set_extra_steps(v);
    }
    fn set_settle_tile(&mut self, v: i32) {
        self.world.set_settle_tile(v);
    }
    fn set_pay_factor(&mut self, v: i32) {
        self.world.set_pay_factor(v);
    }
    fn set_rent_factor(&mut self, v: i32) {
        self.world.set_rent_factor(v);
    }
    fn set_can_build(&mut self, v: bool) {
        self.world.set_can_build(v);
    }
    fn set_settle_as_agent(&mut self, v: bool) {
        self.world.set_settle_as_agent(v);
    }
    fn set_more_steps(&mut self, v: i32) {
        self.world.set_more_steps(v);
    }

    fn set_no_circle_reward(&mut self, v: bool) {
        self.world.set_no_circle_reward(v);
    }
    fn set_teleport_to(&mut self, v: i32) {
        self.world.set_teleport_to(v);
    }
    fn set_start(&mut self, v: i32, why: &str) {
        self.world.set_start(v, why);
    }
    fn clear_dice(&mut self) {
        self.world.clear_dice();
    }
    fn set_base_dice(&mut self, count: i32, sides: i32, why: &str) {
        self.world.set_base_dice(count, sides, why);
    }
    fn add_base_dice(&mut self, count: i32, sides: i32, why: &str) {
        self.world.add_base_dice(count, sides, why);
    }
    fn add_extra_dice(&mut self, count: i32, sides: i32, why: &str) {
        self.world.add_extra_dice(count, sides, why);
    }
    fn move_stopped(&self) -> bool {
        self.world.move_stopped()
    }
    fn move_stop_at(&self) -> i32 {
        self.world.move_stop_at()
    }
    fn move_parity(&self) -> i32 {
        self.world.move_parity()
    }
    fn move_resolve(&self) -> bool {
        self.world.move_resolve()
    }
    fn move_plan(&self) -> game_core::engine::MoveCtx {
        self.world.turn.plan.clone()
    }
    fn move_kind(&self) -> i32 {
        self.world.move_kind()
    }
    fn move_steps(&self) -> i32 {
        self.world.move_steps()
    }
    fn move_remaining(&self) -> i32 {
        self.world.move_remaining()
    }
    fn move_total(&self) -> i32 {
        self.world.move_total()
    }
    fn move_dir(&self) -> i32 {
        self.world.move_dir()
    }
}

impl Run {
    /// C# `H.Present(p)` -- in the game and not exiled.
    fn present(&self, s: i32) -> bool {
        self.world
            .st
            .players
            .get(s.max(0) as usize)
            .is_some_and(|x| !x.out() && x.exile() == 0)
    }

    /// C# `H.SeatsOn(tile, except)` -- present players standing on the tile.
    fn players_on_list(&self, tile: i32, except: i32) -> Vec<i32> {
        (0..self.world.st.players.len() as i32)
            .filter(|&p| {
                p != except && self.present(p) && self.world.st.players[p as usize].pos == tile
            })
            .collect()
    }

    /// The player's hand (C# `_hidden[s].hand`).
    fn hands(&self, player_id: i32) -> Option<&Vec<String>> {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| &h.hand)
    }
}

/// The card modules, ready to serve the match shell.
pub struct WasmRules {
    ruleset: Ruleset,
    data: Arc<GameData>,
}

impl WasmRules {
    pub fn new(ruleset: Ruleset, data: Arc<GameData>) -> Self {
        Self { ruleset, data }
    }

    /// Load every module listed in `<dir>/index.json` (tools/build-ruleset.sh).
    /// `Ok(None)` when the directory or index is missing -- the caller falls back
    /// to no card effects.
    pub fn load_dir(data: Arc<GameData>, dir: &std::path::Path) -> Result<Option<Self>, RuleError> {
        let index = dir.join("index.json");
        let Ok(text) = std::fs::read_to_string(&index) else {
            return Ok(None);
        };
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| RuleError::Load(format!("bad index: {e}")))?;
        let mut builder = Ruleset::builder();
        let mut any = false;
        for m in v["modules"]
            .as_array()
            .map(|a| a.iter())
            .into_iter()
            .flatten()
        {
            let file = m["file"]
                .as_str()
                .ok_or_else(|| RuleError::Load("module entry has no file".into()))?;
            let bytes = std::fs::read(dir.join(file))
                .map_err(|e| RuleError::Load(format!("{file}: {e}")))?;
            builder.add(&bytes)?;
            any = true;
        }
        if !any {
            return Ok(None);
        }
        Ok(Some(Self::new(builder.build()?, data)))
    }

    /// The ruleset (module list, hashes) for inspection.
    pub fn ruleset(&self) -> &Ruleset {
        &self.ruleset
    }

    /// Run one effect to completion, prompting through the engine as needed.
    /// Returns the card's destination (`PlayCtx.Dest`). A reroll the module made
    /// (`set_move_roll`) is written back to `trigger` (C# shares `t.Move`).
    fn drive(&self, cx: &mut Cx, call: Call, card_id: &str, uid: i32, trigger: &mut Trigger) -> Flow<i32> {
        let mut answers: Vec<i32> = Vec::new();
        loop {
            let run = Run {
                world: cx.world_copy(),
                data: self.data.clone(),
                trigger: trigger.clone(),
                current_card: card_id.to_string(),
                current_uid: uid,
                dest: DEST_UNSET,
                dest_to: None,
                paid_log: vec![],
                discard_log: vec![],
                reshuffle_log: vec![],
                fire_spent_log: vec![],
                house_log: vec![],
                crystals_log: vec![],
                doubled: -1,
            };
            match self.ruleset.run(&run, call, &answers) {
                Ok(Outcome::Done(after)) => {
                    let dest = after.dest;
                    *trigger = after.trigger;
                    cx.swap_world(after.world);
                    // What the effect did, raised now that it has committed --
                    // the same points `money()` / `discard()` raise (C# `Money`
                    // PayAfter + `paid`, and `Discarded`).
                    let by = Some(call.player_id());
                    for (from, to, amount) in after.paid_log {
                        for kind in ["payAfter", "paid"] {
                            self.raise_core(cx, kind, from, |t| {
                                t.target = to;
                                t.value = amount;
                                t.by_card = by;
                            })?;
                        }
                    }
                    for (player_id, id) in after.discard_log {
                        self.raise_core(cx, "discarded", player_id, |t| t.card = id)?;
                    }
                    for player_id in after.reshuffle_log {
                        self.raise_core(cx, "reshuffled", player_id, |_| {})?;
                    }
                    for (player_id, n) in after.fire_spent_log {
                        self.raise_core(cx, "fireSpent", player_id, |t| t.value = n)?;
                    }
                    for (player_id, tile, nth) in after.house_log {
                        self.raise_core(cx, "houseAdded", player_id, |t| {
                            t.tile = tile;
                            t.value = nth;
                        })?;
                    }
                    // `crystalsChanged` -- 「此卡上不再拥有[奇迹水晶]时」 (AG:绯红之魂
                    // (3) and kin) live here rather than at each spend site, so a
                    // count emptied by *any* path still leaves the field.
                    for (owner, card, change) in after.crystals_log {
                        self.raise_core(cx, "crystalsChanged", owner, |t| {
                            t.card = card;
                            t.value = change;
                            t.by_card = by;
                        })?;
                    }
                    // A run that has an instance owns that instance's fate:
                    // 「将此卡放入[使用者]弃卡区」 is `set_dest(Graveyard)` whether
                    // the card was just placed or has been in play all along.
                    // Reporting `DEST_FIELD` back tells the engine there is
                    // nothing left for it to move -- the hand card became this
                    // instance, and the host just moved that.
                    //
                    // `DEST_UNSET` is no opinion (a placed card stays put), and a
                    // run that already unplaced itself has no instance to move.
                    if after.current_uid >= 0 && dest >= 0 && dest != DEST_FIELD {
                        self.apply_dest(cx, after.current_uid, card_id, dest, after.dest_to)?;
                        return Ok(DEST_FIELD);
                    }
                    return Ok(dest);
                }
                Ok(Outcome::NeedInput(p)) => {
                    let player_id = p.player_id;
                    let ask = prompt_to_ask(p);
                    let reply = cx.ask(ask)?;
                    // A card prompt has one player: the module's i-th answer is that
                    // player's answer to its i-th prompt.
                    let v = reply
                        .a
                        .answers
                        .first()
                        .copied()
                        .filter(|&x| x >= 0)
                        .unwrap_or(reply.fallback);
                    answers.push(v);
                    let _ = player_id;
                }
                // A card-driven payment: the same C# `Money` pipeline as
                // `money()` -- PayAdd -> PayMul -> PayChoose -> PayAt -> the
                // `pay` [反击] window -- then replay the effect with the
                // adjudicated amount (0 = cancelled, and PayAfter runs with 0).
                Ok(Outcome::NeedHost(HostRequest::Gate { player_id, kind })) => {
                    let allowed = self.abnormal_gate(cx, player_id, kind, call.player_id())?;
                    answers.push(allowed as i32);
                }
                Ok(Outcome::NeedHost(HostRequest::Target {
                    player_id,
                    tile,
                    single,
                })) => {
                    let got = if tile >= 0 {
                        self.target_tile(cx, tile, call.player_id(), card_id)?
                    } else {
                        self.target_player(cx, player_id, call.player_id(), card_id, single)?
                    };
                    answers.push(got);
                }
                // C# `H.CardMove(c, m)`: the card shaped the plan and asked for
                // the move to run now. The engine runs it (it may prompt), then
                // the effect replays past this call.
                Ok(Outcome::NeedHost(HostRequest::Move { player_id, plan })) => {
                    cx.card_move(player_id.max(0) as usize, plan)?;
                    answers.push(1);
                }
                // C# `H.AgentLanding`: the 「星光代理」 landing routine.
                Ok(Outcome::NeedHost(HostRequest::AgentLanding { player_id, agent })) => {
                    cx.agent_landing(player_id.max(0) as usize, agent.max(0) as usize)?;
                    answers.push(1);
                }
                // The rest of the routine family -- see `HostRequest`.
                Ok(Outcome::NeedHost(HostRequest::SettleAt {
                    player_id,
                    tile,
                    main,
                })) => {
                    cx.card_settle_at(player_id.max(0) as usize, tile.max(0) as usize, main)?;
                    answers.push(1);
                }
                Ok(Outcome::NeedHost(HostRequest::Buy { player_id, tile })) => {
                    cx.card_buy(player_id.max(0) as usize, tile.max(0) as usize)?;
                    answers.push(1);
                }
                Ok(Outcome::NeedHost(HostRequest::Build { player_id, tile })) => {
                    cx.card_build(player_id.max(0) as usize, tile.max(0) as usize)?;
                    answers.push(1);
                }
                Ok(Outcome::NeedHost(HostRequest::OfferBuild { player_id, tiles })) => {
                    let tiles: Vec<usize> = tiles
                        .into_iter()
                        .filter(|&t| t >= 0)
                        .map(|t| t as usize)
                        .collect();
                    cx.card_offer_build(player_id.max(0) as usize, &tiles, card_id)?;
                    answers.push(1);
                }
                Ok(Outcome::NeedHost(HostRequest::Mortgage { player_id, tile })) => {
                    cx.card_mortgage(player_id.max(0) as usize, tile.max(0) as usize)?;
                    answers.push(1);
                }
                Ok(Outcome::NeedHost(HostRequest::Pay {
                    from,
                    to,
                    amount: asked,
                })) => {
                    let req = (from, to, asked);
                    let by = Some(call.player_id());
                    let (from, to, mut amount) = req;
                    // C# `Money`: a player immune to others' effects (`ImmuneAll`)
                    // is neither charged nor paid by another player's card --
                    // the payment simply does not happen.
                    let mut immune = false;
                    for x in [from, to] {
                        if x >= 0
                            && x != call.player_id()
                            && self.immune(cx, x, call.player_id())?
                        {
                            immune = true;
                            break;
                        }
                    }
                    if immune {
                        answers.push(0);
                        continue;
                    }
                    for kind in ["payAdd", "payMul", "payChoose", "payAt", "pay"] {
                        if amount <= 0 {
                            break;
                        }
                        let t = self.raise_core(cx, kind, from, |t| {
                            t.target = to;
                            t.value = amount;
                            t.by_card = by;
                        })?;
                        amount = t.value.max(0);
                    }
                    if amount <= 0 {
                        self.raise_core(cx, "payAfter", from, |t| {
                            t.target = to;
                            t.value = 0;
                            t.by_card = by;
                        })?;
                    }
                    answers.push(amount);
                }
                Err(e) => {
                    // A trapping module must not take the match down with it.
                    let detail = match &e {
                        RuleError::Trap(m) | RuleError::Load(m) | RuleError::DuplicateCard(m) => {
                            m.clone()
                        }
                        RuleError::NoSuchCard(i) => format!("no such card {i}"),
                        RuleError::GuardPrompted => "guard prompted".into(),
                    };
                    let player_id = call.player_id();
                    cx.log(
                        player_id,
                        Msg::new("log.card_trap")
                            .card("card", card_id)
                            .text("detail", detail),
                    );
                    return Ok(DEST_GRAVEYARD);
                }
            }
        }
    }

    /// The pure guard on an `On::Hook` entry -- and, with it, the **activation**
    /// gate. A `false` means the card is not activated at all: its body does not
    /// run and nothing about it reaches the UI. A `true` is the card firing, so
    /// it is announced and `drive` runs the body. An entry with no guard
    /// (`On::Gate` -- a question the card answers, not an activation) runs
    /// unannounced, exactly as it did before.
    ///
    /// Runs on a throwaway world copy (same contract as `can_react`); a trap or
    /// a prompting guard fails closed.
    fn hook_guard(
        &self,
        cx: &mut Cx,
        trigger: &Trigger,
        idx: i32,
        player_id: i32,
        card_id: &str,
        uid: i32,
    ) -> bool {
        let run = Run {
            world: cx.world_copy(),
            data: self.data.clone(),
            trigger: trigger.clone(),
            current_card: card_id.to_string(),
            current_uid: uid,
            dest: DEST_UNSET,
            dest_to: None,
            paid_log: vec![],
            discard_log: vec![],
            reshuffle_log: vec![],
            fire_spent_log: vec![],
            house_log: vec![],
            crystals_log: vec![],
            doubled: -1,
        };
        match self.ruleset.can_hook(&run, idx, player_id) {
            // No guard to ask (`On::Gate`): run it, and do not announce -- it is
            // answering a question, not activating.
            Ok(None) => true,
            Ok(Some(false)) => false,
            Ok(Some(true)) => {
                // Activated: this is the one moment the card shows itself.
                cx.log(
                    player_id,
                    Msg::new("log.hook_fire")
                        .player_id("who", player_id)
                        .card("card", card_id.to_string()),
                );
                true
            }
            // A trap or a prompting guard: fail closed, the card does not fire.
            Err(_) => false,
        }
    }

    /// C# `AbnormalGate`, for an effect a card run wants to apply to `player_id`
    /// (`by` = the card's player): an out player is never hit; field cards guard
    /// (`abnormalGuard`, block with `set_cancelled`); then, if another player
    /// caused it, the `abnormal` [反击] window (C# `ReactAbnormal`). What gets
    /// through bumps `player_id`'s abnormal counter for this turn.
    fn abnormal_gate(
        &self,
        cx: &mut Cx,
        player_id: i32,
        kind: crate::AbKind,
        by: i32,
    ) -> Flow<bool> {
        let Ok(s) = usize::try_from(player_id) else {
            return Ok(false);
        };
        if s >= cx.state().players.len() || cx.world_copy().out(s) {
            return Ok(false);
        }
        // `unstoppable` (「不可阻挡」) is a blanket bypass for the *movement*
        // abnormals -- the ones that put the player somewhere they did not
        // choose. Status (stay/stun/exile) still lands.
        let movement = matches!(
            kind,
            crate::AbKind::Teleport
                | crate::AbKind::Forced
                | crate::AbKind::Stop
                | crate::AbKind::Reverse
        );
        if movement && cx.state().players[s].unstoppable() > 0 {
            return Ok(false);
        }
        let on = |t: &mut CoreTrigger| {
            t.target = player_id;
            t.value = kind as i32;
            t.by_card = Some(by);
        };
        // `AbnormalGuard` is a gate at declaration: a field card may block the
        // effect before anyone answers it.
        if self.raise_core(cx, "abnormalGuard", by, on)?.is_cancelled() {
            return Ok(false);
        }
        // Then the chain: the recipient answers the effect declaration.
        if by != player_id {
            let declared = self.raise_core(cx, "effect", by, |t| {
                on(t);
                t.effects.push(game_core::engine::rules::Effect {
                    kind: "abnormal",
                    target: player_id,
                    from: -1,
                    tile: -1,
                    value: kind as i32,
                });
            })?;
            if declared.is_cancelled() {
                return Ok(false);
            }
        }
        // Then resolution: `ImmuneAll` lets the effect name them and still land
        // as nothing.
        if by != player_id && self.immune(cx, player_id, by)? {
            return Ok(false);
        }
        // Settlement: what actually landed. `abnormal` is a hook now -- it is
        // the outcome, and cannot be [反击]'d.
        if by != player_id {
            self.raise_core(cx, "abnormal", by, on)?;
        }
        let mut w = cx.world_copy();
        if w.turn.abnormal.len() <= s {
            w.turn.abnormal.resize(s + 1, 0);
        }
        w.turn.abnormal[s] += 1;
        cx.swap_world(w);
        Ok(true)
    }

    /// C# `AnyFx(player_id, f => f.ImmuneAll(player_id))`: does a field card make `player_id`
    /// untouchable by `by`'s effects? (`immuneAll` hook, claimed with
    /// `set_cancelled`.) Logged when it holds.
    fn immune(&self, cx: &mut Cx, player_id: i32, by: i32) -> Flow<bool> {
        let t = self.raise_core(cx, "immuneAll", player_id, |t| t.by_card = Some(by))?;
        if t.is_cancelled() {
            cx.log(
                player_id,
                Msg::new("log.immune_all").player_id("who", player_id),
            );
        }
        Ok(t.is_cancelled())
    }

    /// C# `H.Target(c, p)`: `by`'s card `card` tries to target player `p`.
    /// Answers the player actually targeted (a `redirect` hook may move a
    /// single-target hit), or -1. Not ported: the per-play `immune<p>` tags
    /// (C# `c.Tags`) and the `PendingReact` / `TargetsChosen` pass.
    fn target_player(&self, cx: &mut Cx, p: i32, by: i32, card: &str, single: bool) -> Flow<i32> {
        let Ok(s) = usize::try_from(p) else {
            return Ok(-1);
        };
        if s >= cx.state().players.len() || cx.world_copy().out(s) {
            return Ok(-1);
        }
        if p == by {
            return Ok(p);
        }
        if cx.state().players[s].exile() > 0 {
            return Ok(-1);
        }
        // ---- declaration ----------------------------------------------
        // `Untargetable` governs who may be *named*. Block here and the effect
        // cannot name `p` at all, so no chain forms against them.
        if self
            .raise_core(cx, "untargetable", p, |t| t.by_card = Some(by))?
            .is_cancelled()
        {
            cx.log(p, Msg::new("log.untargetable").player_id("who", p));
            return Ok(-1);
        }
        // `Redirect` is declaration re-naming: the recipient set is settled
        // before the chain opens, so whoever holds the name is who answers.
        let mut p = p;
        if single {
            let r = self.raise_core(cx, "redirect", by, |t| {
                t.target = p;
                t.card = card.to_string();
                t.by_card = Some(by);
            })?;
            let to = r.target;
            let live = usize::try_from(to)
                .is_ok_and(|x| x < cx.state().players.len() && !cx.world_copy().out(x));
            if to != p && to != by && live {
                cx.log(
                    to,
                    Msg::new("log.target_redirected")
                        .card("card", card)
                        .player_id("from", p)
                        .player_id("to", to),
                );
                p = to;
            }
        }
        // Named. Count it *after* naming, so a player who cannot be named does
        // not accrue 「成为目标」 counts (see `centrifugal`'s 「第二次」).
        if let Ok(s) = usize::try_from(p) {
            let mut w = cx.world_copy();
            if w.targeted.len() <= s {
                w.targeted.resize(s + 1, 0);
            }
            w.targeted[s] += 1;
            cx.swap_world(w);
        }
        // ---- chain ----------------------------------------------------
        // Counters answer the declaration; they may negate or spare it.
        if p != by && !self.target_window(cx, p, -1, by, card)? {
            return Ok(-1);
        }
        // ---- resolution -----------------------------------------------
        // `ImmuneAll` is asked here, not at declaration: the effect named its
        // recipient, the chain formed, and it still lands as nothing.
        if self.immune(cx, p, by)? {
            return Ok(-1);
        }
        Ok(p)
    }

    /// C# `H.TargetTile(c, tile)`: `by`'s card targets `tile`; another player's
    /// tile targets its owner too. Answers the tile, or -1.
    ///
    /// A tile marked [`card_sdk::abi::mark::NO_TARGET`] cannot be named at all --
    /// that is 「有标记时此地块不能被指定」. Not ported: the per-play `immune<p>` tags.
    fn target_tile(&self, cx: &mut Cx, tile: i32, by: i32, card: &str) -> Flow<i32> {
        if cx
            .world_copy()
            .count_marks(tile, card_sdk::abi::mark::NO_TARGET, -2)
            > 0
        {
            return Ok(-1);
        }
        let Some(&owner) = usize::try_from(tile)
            .ok()
            .and_then(|t| cx.state().owners.get(t))
        else {
            return Ok(-1);
        };
        let live = usize::try_from(owner).is_ok_and(|o| !cx.world_copy().out(o));
        if owner >= 0 && owner != by && live {
            // Chain first: the owner answers the declaration. Then resolution.
            if !self.target_window(cx, owner, tile, by, card)? {
                return Ok(-1);
            }
            if self.immune(cx, owner, by)? {
                return Ok(-1);
            }
        }
        Ok(tile)
    }

    /// The effect declaration for a targeting: `targeted` (a field hook), then
    /// the `effect` [反击] chain, then `target` as a settlement hook.
    ///
    /// The chain is keyed on the **effect**, not on the outcome: 「被其他玩家
    /// 的卡效果影响」 listens to `effect` and reads `ctx::effect` for the list.
    /// `target` fires only once the effect actually settles, so it can no
    /// longer be used to reconstruct that clause -- it is what happened, not
    /// what was declared. False when a counter negated the declaration.
    fn target_window(&self, cx: &mut Cx, p: i32, tile: i32, by: i32, card: &str) -> Flow<bool> {
        self.raise_core(cx, "targeted", by, |t| {
            t.target = p;
            t.tile = tile;
            t.card = card.to_string();
            t.by_card = Some(by);
        })?;
        let declared = self.raise_core(cx, "effect", by, |t| {
            t.target = p;
            t.tile = tile;
            t.card = card.to_string();
            t.by_card = Some(by);
            t.effects.push(game_core::engine::rules::Effect {
                kind: "target",
                target: p,
                from: -1,
                tile,
                value: 0,
            });
        })?;
        if declared.is_cancelled() {
            return Ok(false);
        }
        self.raise_core(cx, "target", by, |t| {
            t.target = p;
            t.tile = tile;
            t.card = card.to_string();
            t.by_card = Some(by);
        })?;
        Ok(true)
    }

    /// Raise an engine trigger from the bridge itself (what a committed card
    /// effect did), through the same dispatcher the engine's `raise!` reaches.
    /// `player_id` may be -1 (the bank's side of a payment).
    fn raise_core(
        &self,
        cx: &mut Cx,
        kind: &'static str,
        player_id: i32,
        set: impl FnOnce(&mut CoreTrigger),
    ) -> Flow<CoreTrigger> {
        let mut t = CoreTrigger::new(kind, player_id.max(0) as usize);
        t.player_id = player_id;
        t.step = cx.state().step;
        set(&mut t);
        self.react(cx, &mut t)?;
        Ok(t)
    }

    /// Apply a field effect's `Dest` to the instance its run was for -- C#
    /// `H.Unplace(this, "discard" / "hand" / "gone")`, the fate `set_dest` names.
    /// `to` is whose pile it lands in; `None` is the owner it leaves, the target
    /// `set_transfer_to_dest` names.
    fn apply_dest(&self, cx: &mut Cx, uid: i32, card: &str, dest: i32, to: Option<i32>) -> Flow<()> {
        let mut w = cx.world_copy();
        let left = w.unplace_at(uid);
        if left < 0 {
            // Not in play -- the run may have moved it already.
            cx.swap_world(w);
            return Ok(());
        }
        let who = to.unwrap_or(left);
        if dest == DEST_GRAVEYARD {
            w.to_discard(who, card);
        } else if dest == 1 {
            // Back to the hand (手牌).
            w.add_to_hand(who, card);
        }
        // Banished (「[移除]」) is just "gone" -- it left above and goes nowhere.
        cx.swap_world(w);
        if dest == DEST_GRAVEYARD {
            self.raise_core(cx, "discarded", who, |t| t.card = card.to_string())?;
        } else if dest != 1 {
            cx.log(
                left,
                Msg::new("log.card_removed").card("card", card.to_string()),
            );
        }
        Ok(())
    }

    /// The [反击] hand window (C# `MatchHost.React(Trigger)`): each player from the
    /// trigger's player around the table may answer with a reaction card from hand.
    /// Declarations are collected first (hands shrink as they declare) and then
    /// resolved in reverse declaration order (C# `declared[k]` from the end).
    ///
    /// # Ordering at one trigger (the standard)
    ///
    /// When more than one card responds to the same trigger, they resolve as a
    /// **LIFO stack**: declaration order is player order starting at the trigger's
    /// player and wrapping around the table; resolution is the exact reverse of
    /// declaration order. So the last card to answer the window is the first one
    /// to resolve, and any reaction it plays opens a nested counter-window before
    /// the earlier declarations get their turn.
    ///
    /// Known cases where this LIFO rule does *not* apply (left as-is for now,
    /// flagged for a later pass):
    ///
    /// * **The acting card's own follow-up** (`CardRules::react` step (1)) runs
    ///   before this window, outside the chain.
    ///
    /// The [反击] window is a Yu-Gi-Oh **chain**. `t` is **L1**, the effect
    /// declaration -- the card effect already aimed at its named recipients.
    /// Counters push onto it as L2, L3, ... and resolve **before** it, so a
    /// counter can invalidate the effect before it settles.
    ///
    /// * **Build** is YGO priority: after every declaration priority circulates
    ///   the table again, closing only when every player passes consecutively.
    ///   A P1<->P0 counter war runs as far as it needs to; there is no
    ///   single-pass limit and no seat-order asymmetry.
    /// * **Resolution is flat** over the closed link list. There are no nested
    ///   counter-windows: a response to a link is another link in *this* chain,
    ///   and a response to a link that has already resolved is a *new* chain.
    /// * Each counter answers the current top of the chain, and its guard reads
    ///   that link (`Run.trigger`) -- so `effect::` sees the effects that link
    ///   declared, and `set_cancelled` / `negate_effect` / `spare` land on it.
    /// * **One declaration per player per priority pass** -- a player with two
    ///   eligible cards must pick one.
    fn hand_reactions(
        &self,
        cx: &mut Cx,
        t: &CoreTrigger,
        trigger: &mut Trigger,
        depth: u32,
    ) -> Flow<()> {
        if depth >= MAX_REACT_DEPTH || !cx.playing() {
            return Ok(());
        }
        let n = cx.state().players.len();
        if n == 0 {
            return Ok(());
        }

        // The chain as it stands. Index 0 is L1, the effect declaration.
        let mut chain: Vec<Trigger> = vec![trigger.clone()];
        // (seat, card handle, card id, index into `chain` this counter answers)
        let mut decls: Vec<(usize, i32, String, usize)> = Vec::new();

        let start = if t.player_id >= 0 {
            t.player_id as usize % n
        } else {
            cx.state().turn.max(0) as usize % n
        };

        // ---- build: YGO priority passes ---------------------------------
        let mut passed = vec![false; n];
        let mut cursor = start;
        // Hard bound: the chain is only as long as the hands involved, so this
        // is a runaway guard, not a design limit.
        let mut budget = MAX_REACT_DEPTH * n as u32 + 8;
        while budget > 0 {
            budget -= 1;
            let answered = chain.len() - 1;
            let declared = if can_react_now(cx, cursor) {
                match self.declare_one(cx, cursor, &chain[answered], t)? {
                    Some((id, idx)) => {
                        // The declaration leaves the hand now (C# `_hidden[s].hand.Remove`).
                        let mut w = cx.world_copy();
                        if let Some(pos) = w.hidden[cursor].hand.iter().position(|c| c == &id) {
                            w.hidden[cursor].hand.remove(pos);
                        }
                        cx.swap_world(w);
                        let mut link = CoreTrigger::new("card", cursor);
                        link.card = id.clone();
                        link.step = cx.state().step;
                        link.by_card = Some(cursor as i32);
                        link.seq = (chain.len() + 1) as u32;
                        link.answers = answered as u32;
                        chain.push(bridge_trigger(&link));
                        decls.push((cursor, idx, id, answered));
                        true
                    }
                    None => false,
                }
            } else {
                false
            };
            if declared {
                // A declaration reopens priority for everyone.
                passed.fill(false);
            } else {
                passed[cursor] = true;
                if passed.iter().all(|&p| p) {
                    break;
                }
            }
            cursor = (cursor + 1) % n;
        }

        // ---- resolve: flat LIFO -----------------------------------------
        for (s, idx, id, answered) in decls.into_iter().rev() {
            cx.log(
                s as i32,
                Msg::new("log.play_react")
                    .player_id("who", s as i32)
                    .card("card", id.clone()),
            );
            // The counter's body runs against the link it answers, so its
            // `set_cancelled` / `negate_effect` / `spare` land there -- and the
            // effect settles only after every counter has had its say.
            let mut on_link = chain[answered].clone();
            let dest = self.drive(
                cx,
                Call::CounterAct {
                    card: idx,
                    player_id: s as i32,
                },
                &id,
                -1,
                &mut on_link,
            )?;
            chain[answered] = on_link;
            let mut w = cx.world_copy();
            let spent = matches!(dest_from(dest), Dest::Graveyard) && !w.out(s);
            match dest_from(dest) {
                Dest::Graveyard => {
                    if !w.out(s) {
                        w.hidden[s].discard.push(id.clone());
                    }
                }
                Dest::Hand => w.hidden[s].hand.push(id.clone()),
                Dest::Banished => {}
                Dest::Field => {}
            }
            cx.swap_world(w);
            if spent {
                self.raise_core(cx, "discarded", s as i32, |t| t.card = id.clone())?;
            }
        }

        // L1's fate is whatever the counters did to it.
        if let Some(root) = chain.into_iter().next() {
            *trigger = root;
        }
        Ok(())
    }

    /// Offer one player a [反击] window answering `top`. Returns the card they
    /// declared, if any. `t` is the chain root, for the prompt's description.
    fn declare_one(
        &self,
        cx: &mut Cx,
        s: usize,
        top: &Trigger,
        t: &CoreTrigger,
    ) -> Flow<Option<(String, i32)>> {
        // Hand cards that answer this link (C# `_hidden[s].hand.Distinct()`).
        let mut options: Vec<(String, i32)> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for id in hand_of(cx, s) {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id.clone());
            let Some(idx) = self.ruleset.card(&id) else {
                continue;
            };
            if !self.ruleset.cards()[idx as usize].counter_acts_to(top.kind) {
                continue;
            }
            let run = Run {
                world: cx.world_copy(),
                data: self.data.clone(),
                trigger: top.clone(),
                current_card: id.clone(),
                current_uid: -1,
                dest: DEST_UNSET,
                dest_to: None,
                paid_log: vec![],
                discard_log: vec![],
                reshuffle_log: vec![],
                fire_spent_log: vec![],
                house_log: vec![],
                crystals_log: vec![],
                doubled: -1,
            };
            if self.ruleset.can_react(&run, idx, s as i32).unwrap_or(false) {
                options.push((id, idx));
            }
        }
        if options.is_empty() {
            return Ok(None);
        }
        // C#: labels "打出「...」" + "不打"; the hint is the first ReactHint or
        // the trigger's description. ReactHint is not in the ABI yet (TODO).
        let mut labels: Vec<Msg> = options
            .iter()
            .map(|(id, _)| Msg::new("ask.react.play").card("card", id.clone()))
            .collect();
        labels.push(Msg::new("ask.react.skip"));
        let fallback = labels.len() as i32 - 1;
        let ask = Ask::choice(
            vec![s],
            Msg::new("ask.react.title"),
            Msg::new("ask.react.text").msg("detail", describe_trigger(t)),
            labels,
            fallback,
            12.0,
        );
        let reply = cx.ask(ask)?;
        let pick = reply
            .a
            .answers
            .first()
            .copied()
            .filter(|&x| x >= 0)
            .unwrap_or(reply.fallback) as usize;
        if pick >= options.len() {
            return Ok(None);
        }
        Ok(Some(options[pick].clone()))
    }
}

/// The module's view of an engine trigger (the bridge `Trigger`). `move_roll`
/// is C# `t.Move.Roll`, which a reaction may rewrite; the engine reads it back.
fn bridge_trigger(t: &CoreTrigger) -> Trigger {
    Trigger {
        kind: trigger_kind(t.kind),
        player_id: t.player_id,
        target: t.target,
        tile: t.tile,
        value: t.value,
        step: t.step,
        by_card: t.by_card,
        pay_is_rent: t.pay_is_rent,
        move_kind: t.move_kind.map(|k| match k {
            game_core::engine::MoveKind::Walk => card_sdk::abi::MoveKind::Walk,
            game_core::engine::MoveKind::Teleport => card_sdk::abi::MoveKind::Teleport,
        }),
        move_resolve: t.move_resolve,
        move_tags: t.move_tags.clone(),
        move_main: t.move_main,
        move_dir: t.move_dir,
        negation: t.negation,
        spared: t.spared.clone(),
        seq: t.seq,
        answers: t.answers,
        effects: t.effects.clone(),
        move_remaining: t.move_remaining,
        move_total: t.move_total,
        cards: t.cards.clone(),
        // Only a move trigger carries a roll; `paid`/`settle` carry value as an
        // amount, which must not read as a phantom move.
        move_roll: matches!(
            trigger_kind(t.kind),
            TriggerKind::MoveRoll | TriggerKind::Roll
        )
        .then_some(t.value),
        card: t.card.clone(),
    }
}

/// A module prompt -> the engine's `Ask` (the player sees the same prompt style
/// the engine shows for its own questions).
fn prompt_to_ask(p: Prompt) -> Ask {
    let players = vec![p.player_id as usize];
    let Prompt {
        kind,
        player_id,
        title,
        text,
        options,
        answer_slot: _,
    } = p;
    match kind {
        PromptKind::YesNo => Ask::choice(
            players,
            title,
            text,
            vec![Msg::new("ask.yes"), Msg::new("ask.no")],
            1,
            12.0,
        ),
        PromptKind::Choice => {
            let labels = options
                .into_iter()
                .map(|o| match o {
                    PromptOption::Str(m) => m,
                    PromptOption::Int(i) => Msg::new("ask.intOption").i("n", i),
                })
                .collect();
            Ask::choice(players, title, text, labels, 0, 15.0)
        }
        PromptKind::Tile => {
            let tiles: Vec<usize> = options
                .iter()
                .map(|o| match o {
                    PromptOption::Int(i) => *i as usize,
                    PromptOption::Str(_) => 0,
                })
                .collect();
            let labels = tiles
                .iter()
                .map(|&t| Msg::new("ask.tileOption").tile("tile", t as i32))
                .collect();
            Ask::tile(player_id as usize, title, text, &tiles, labels)
        }
        PromptKind::Card => {
            // Pick one card out of a list; the options are ready-made labels.
            let labels = options
                .into_iter()
                .map(|o| match o {
                    PromptOption::Str(m) => m,
                    PromptOption::Int(i) => Msg::new("ask.intOption").i("n", i),
                })
                .collect();
            Ask::choice(players, title, text, labels, 0, 15.0)
        }
        PromptKind::Player => {
            let labels = options
                .iter()
                .map(|o| match o {
                    PromptOption::Int(s) => Msg::new("ask.player").player_id("who", *s),
                    PromptOption::Str(_) => Msg::new("ask.player").player_id("who", -1),
                })
                .collect();
            Ask::choice(players, title, text, labels, 0, 15.0)
        }
    }
}

/// The engine's string trigger kinds -> the module's small enum.

/// C# `MatchHost.CanReactNow(s, t)`: out / exiled players cannot declare a
/// reaction. (The C# also checks `CannotPlay` and a one-turn mute; neither has
/// an engine field yet.)
fn can_react_now(cx: &Cx, s: usize) -> bool {
    let Some(player_id) = cx.state().players.get(s) else {
        return false;
    };
    // C# `CanReactNow`: out / AI / exiled players never open a window, and
    // `CannotPlay` (stun, 飞鸟山之战's no-hand, Fx.CantPlayHand) blocks it too.
    // `_noReactTurn` and Fx.CantPlayHand have no engine field yet (TODO).
    !cx.world_copy().out(s)
        && !player_id.ai
        && player_id.exile() == 0
        && !player_id.stunned()
        && player_id.no_hand() == 0
}

/// C# `_hidden[s].hand` -- the player's private hand (order preserved).
fn hand_of(cx: &Cx, s: usize) -> Vec<String> {
    match cx.world_copy().hidden.get(s) {
        Some(h) => h.hand.clone(),
        None => Vec::new(),
    }
}

/// The one-line trigger description the [反击] prompt shows (C# `DescribeTrigger`).
fn describe_trigger(t: &CoreTrigger) -> Msg {
    let mut m = Msg::new("ask.react.detail").player_id("who", t.player_id);
    if !t.card.is_empty() {
        m = m.card("card", &t.card);
    }
    if t.tile >= 0 {
        m = m.tile("tile", t.tile);
    }
    m = m.i("n", t.value as i64);
    m
}

/// Every C# `Trigger.Kind` string the engine raises (turnStart / pass /
/// settleBefore / settle / mortgage / pay / paid / bankrupt / card / event /
/// abnormal / target / stop / teleport / ...) maps through the shared table.
/// Folding unknown kinds to `None` would make `can_react` guards silently
/// never match.
fn trigger_kind(kind: &str) -> TriggerKind {
    TriggerKind::from_str(kind)
}

impl CardRules for WasmRules {
    fn cant_play(&self, cx: &Cx, player_id: usize, card: &str) -> Option<Msg> {
        // A pure query on a throwaway copy (C# `Card.WhyNot`): a guard that
        // prompts, or a module that fails, never blocks play.
        let idx = self.ruleset.card(card)?;
        let run = Run {
            world: cx.world_copy(),
            data: self.data.clone(),
            trigger: Trigger::default(),
            current_card: card.to_string(),
            current_uid: -1,
            dest: DEST_UNSET,
            dest_to: None,
            paid_log: vec![],
            discard_log: vec![],
            reshuffle_log: vec![],
            fire_spent_log: vec![],
            house_log: vec![],
            crystals_log: vec![],
            doubled: -1,
        };
        self.ruleset
            .cant_play(&run, idx, player_id as i32)
            .ok()
            .flatten()
    }
    fn play(&self, cx: &mut Cx, player_id: usize, card: &str) -> Flow<Dest> {
        let Some(idx) = self.ruleset.card(card) else {
            cx.log(
                player_id as i32,
                Msg::new("log.card_not_ported").card("card", card),
            );
            return Ok(Dest::Graveyard);
        };
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            value: 0,
            step: cx.state().step,
            // The card is playing itself, so it is its own cause.
            by_card: Some(player_id as i32),
            pay_is_rent: false,
            move_kind: None,
            move_resolve: false,
            move_tags: Vec::new(),
            move_main: false,
            move_dir: 1,
            negation: Default::default(),
            spared: Vec::new(),
            seq: 0,
            answers: 0,
            effects: Vec::new(),
            move_remaining: 0,
            move_total: 0,
            cards: Vec::new(),
            move_roll: None,
            card: String::new(),
        };
        let dest = self.drive(
            cx,
            Call::Play {
                card: idx,
                player_id: player_id as i32,
            },
            card,
            -1,
            &mut trigger,
        )?;
        Ok(dest_from(dest))
    }

    fn event(&self, cx: &mut Cx, player_id: usize, id: &str) -> Flow<bool> {
        let Some(idx) = self.ruleset.card(id) else {
            cx.log(
                player_id as i32,
                Msg::new("log.event_not_ported").event("event", id),
            );
            return Ok(false);
        };
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            value: 0,
            step: cx.state().step,
            // An event is not a card, so nothing "card-caused" this.
            by_card: None,
            pay_is_rent: false,
            move_kind: None,
            move_resolve: false,
            move_tags: Vec::new(),
            move_main: false,
            move_dir: 1,
            negation: Default::default(),
            spared: Vec::new(),
            seq: 0,
            answers: 0,
            effects: Vec::new(),
            move_remaining: 0,
            move_total: 0,
            cards: Vec::new(),
            move_roll: None,
            card: String::new(),
        };
        let dest = self.drive(
            cx,
            Call::Play {
                card: idx,
                player_id: player_id as i32,
            },
            id,
            -1,
            &mut trigger,
        )?;
        Ok(dest == DEST_FIELD)
    }

    fn react(&self, cx: &mut Cx, t: &mut CoreTrigger) -> Flow<()> {
        // Nothing in the set declares an entry at this kind, so no hook, no
        // gate and no [反击] can fire: skip building the bridge trigger at all.
        // This is most raises even with cards in play, and every raise for a
        // kind nobody listens to. Nothing could have rewritten `t`, so the
        // write-back below is a no-op and is safe to skip with it.
        if !self.ruleset.declares(trigger_kind(t.kind)) {
            return Ok(());
        }
        // The module's view of the trigger. `move_roll` is C# `t.Move.Roll`,
        // which a reaction may rewrite; the engine reads it back afterwards.
        let mut trigger = bridge_trigger(t);
        // Ordering at one trigger (the standard; see `hand_reactions`):
        // (1) the acting card's own follow-up resolves FIRST, outside the LIFO
        //     stack -- a card answering its own play is not competing with the
        //     reactions to it, so it never loses its place to them;
        // (2) then every declared reaction resolves in LIFO order (reverse
        //     declaration order), each free to open a nested counter-window.
        //
        // (1) The played card's own follow-up: the card named on the trigger runs
        // its `react` (C# `PlayCtx.AsReaction` for a card answering its own play).
        // Only at the play itself -- `cardAfter` / `cardPlayed` / `eventAfter` /
        // `drawn` also name a card on `t.card`, and must not re-run it here.
        let own = t.card.clone();
        let own_play = matches!(trigger_kind(t.kind), TriggerKind::Card | TriggerKind::Event);
        if own_play && !own.is_empty() {
            if let Some(idx) = self
                .ruleset
                .card(&own)
                .filter(|&i| self.ruleset.cards()[i as usize].counter_acts_to(trigger.kind))
            {
                self.drive(
                    cx,
                    Call::CounterAct {
                        card: idx,
                        player_id: t.player_id,
                    },
                    &own,
                    -1,
                    &mut trigger,
                )?;
            }
        }
        // (2) Field-card (`Fx`) hooks: at a hook-point kind, every *placed* card
        // runs its `react` automatically, in placement order per player. No player
        // declaration -- this is the persistent-effect path, as against the
        // [反击] window below.
        // Any trigger kind can carry a hook; the manifest says which cards
        // declared one, so nothing else is instantiated.
        let kind = trigger.kind;
        {
            if is_own_card_kind(kind) {
                // C# runs these on a fresh instance of the card named on
                // `t.card` (`Drawn`, `OnDiscarded`, `DeckBeforeGame`,
                // `DeckAtGameStart`) -- it is in a hand / pile, not placed.
                if let Some(idx) = self
                    .ruleset
                    .card(&t.card)
                    .filter(|&i| self.ruleset.cards()[i as usize].hooks(kind))
                {
                    if self.hook_guard(cx, &trigger, idx, t.player_id, &t.card, -1) {
                        self.drive(
                            cx,
                            Call::Hook {
                                card: idx,
                                kind,
                                player_id: t.player_id,
                            },
                            &t.card,
                            -1,
                            &mut trigger,
                        )?;
                    }
                }
            } else {
                let world = cx.world_copy();
                for player_id in 0..world.player_count() {
                    for (uid, id) in world.field_instances(player_id as i32) {
                        let Some(idx) = self.ruleset.card(&id) else {
                            continue;
                        };
                        // 「场上所有背面朝上的卡无法产生效果」 -- a face-down
                        // card on the field is inert, so its hooks must not run.
                        if self.ruleset.cards()[idx as usize].hooks(kind)
                            && !world.card_face_down(player_id as i32, &id)
                            && self.hook_guard(cx, &trigger, idx, player_id as i32, &id, uid)
                        {
                            self.drive(
                                cx,
                                Call::Hook {
                                    card: idx,
                                    kind,
                                    player_id: player_id as i32,
                                },
                                &id,
                                uid,
                                &mut trigger,
                            )?;
                        }
                        // C# `RollMove`: `RollPlan` runs on every live Fx before
                        // the dice -- `On::RollPlan` shapes the move.
                        if kind == TriggerKind::RollPlan
                            && self.ruleset.cards()[idx as usize]
                                .entry(card_sdk::abi::OnKind::RollPlan, None)
                                .is_some()
                        {
                            self.drive(
                                cx,
                                Call::RollPlan {
                                    card: idx,
                                    player_id: player_id as i32,
                                },
                                &id,
                                uid,
                                &mut trigger,
                            )?;
                        }
                    }
                }
            }
            // Scheduled turn-end callbacks (C# `TurnCtx.AfterEnd` and "the end
            // of your next turn"): the ones due at this player's turn end run once
            // and are dropped. They are taken out of the world *before* running,
            // so a callback that schedules again lands in the next round.
            let phase = match kind {
                TriggerKind::TurnEndBefore => Some(true),
                TriggerKind::TurnEndAfter => Some(false),
                _ => None,
            };
            if let (Some(early), true) = (phase, t.player_id >= 0) {
                let ended = t.player_id as usize;
                let mut w = cx.world_copy();
                let mut due = Vec::new();
                w.scheduled.retain_mut(|s| {
                    if s.target != ended || s.early != early {
                        return true;
                    }
                    if s.skip {
                        s.skip = false;
                        return true;
                    }
                    due.push((s.card.clone(), s.owner, s.uid));
                    false
                });
                cx.swap_world(w);
                for (id, owner, uid) in due {
                    if let Some(idx) = self
                        .ruleset
                        .card(&id)
                        .filter(|&i| self.ruleset.cards()[i as usize].has_at_end())
                    {
                        self.drive(
                            cx,
                            Call::AtEnd {
                                card: idx,
                                player_id: owner,
                            },
                            &id,
                            uid,
                            &mut trigger,
                        )?;
                    }
                }
            }
        }
        // (3) The hand-reaction window (C# `MatchHost.React(Trigger)`): every player
        // from the trigger's player around the table may answer with a [反击] card
        // from hand; declarations resolve in reverse order. Not at hook-only
        // points.
        if !is_hook_only(t.kind) {
            self.hand_reactions(cx, t, &mut trigger, 0)?;
        }
        // Write back whatever a reaction rewrote. `set_move_roll` lands in
        // `trigger.move_roll` (C# shares `t.Move` with the reactions); the pay
        // amount is rewritten in `trigger.value`. The engine reads the result
        // back off `t.value` after `react` returns.
        t.value = trigger.move_roll.unwrap_or(trigger.value);
        if trigger.negation != Default::default() {
            t.negation = trigger.negation;
        }
        for s in trigger.spared {
            t.spare(s);
        }
        t.target = trigger.target;
        Ok(())
    }
}

/// Field-card (`Fx`) hook points (ABI v17). These are **not** [反击] points:
/// the engine runs every *placed* card's `react` against them automatically,
/// with no player declaration. They are distinct `TriggerKind`s from the
/// reaction kinds so a card can tell a field effect from a hand reaction by its
/// kind alone.
/// Hook points that are *only* field-card points: no [反击] window opens at
/// them. (`turnStart` / `settleAfter` are both, so they are not in here.)
/// Besides keeping the semantics clean this matters for cost -- `passTile` fires
/// on every tile walked, and a window instantiates a module per hand card.
fn is_hook_only(kind: &str) -> bool {
    matches!(
        trigger_kind(kind),
        TriggerKind::TurnEnd
            | TriggerKind::Drawn
            | TriggerKind::PassTile
            | TriggerKind::PayAfter
            | TriggerKind::RollAfter
            | TriggerKind::CardPlayed
            | TriggerKind::Targeted
            | TriggerKind::PayChoose
            | TriggerKind::TurnEndBefore
            | TriggerKind::TurnEndAfter
            | TriggerKind::PayAdd
            | TriggerKind::PayMul
            | TriggerKind::PayAt
            | TriggerKind::Discarded
            | TriggerKind::DeckBeforeGame
            | TriggerKind::DeckAtGameStart
            | TriggerKind::Drew
            | TriggerKind::Reshuffled
            | TriggerKind::Bought
            | TriggerKind::SettleInstead
            | TriggerKind::BeforeOut
            | TriggerKind::Teleported
            | TriggerKind::RollPlan
            | TriggerKind::AbnormalGuard
            | TriggerKind::ImmuneAll
            | TriggerKind::Untargetable
            | TriggerKind::Redirect
            | TriggerKind::CrystalsChanged
    )
}

/// Kinds C# runs on a fresh instance of one card (named on `t.card`) rather
/// than on the cards in play.
fn is_own_card_kind(kind: TriggerKind) -> bool {
    matches!(
        kind,
        TriggerKind::Drawn
            | TriggerKind::Discarded
            | TriggerKind::DeckBeforeGame
            | TriggerKind::DeckAtGameStart
    )
}

fn dest_from(v: i32) -> Dest {
    match v {
        1 => Dest::Hand,
        DEST_FIELD => Dest::Field,
        3 => Dest::Banished,
        _ => Dest::Graveyard,
    }
}

#[cfg(test)]
mod tests {
    use super::trigger_kind;
    use crate::TriggerKind;

    /// The engine raises these kinds; folding any of them to `None` would make
    /// `can_react` guards silently never match (34 cards depend on them).
    /// Includes both the original single-fire kinds and the v10 pre/post
    /// halves added to pair every trigger point.
    #[test]
    fn every_engine_trigger_kind_maps() {
        for k in [
            // original single-fire kinds
            "turnStart",
            "pass",
            "settleBefore",
            "settle",
            "mortgage",
            "paid",
            "bankrupt",
            "card",
            "event",
            "moveRoll",
            // reserved-but-declared kinds (engine does not raise all of these
            // yet, but `can_react` guards match on them so they must map)
            "passPlayer",
            "pay",
            "roll",
            "reacted",
            // v10 pre/post halves + new action hooks
            "turnStartBefore",
            "passBefore",
            "settleAfter",
            "mortgageBefore",
            "bankruptBefore",
            "cardAfter",
            "eventAfter",
            "buyBefore",
            "buyAfter",
            "buildBefore",
            "buildAfter",
            "discardBefore",
            "discardAfter",
            "endTurnBefore",
            "endTurnAfter",
            "leaveBefore",
            "leaveAfter",
            // v17 field-card (Fx) hook points
            "turnEnd",
            "drawn",
            "passTile",
            "payAfter",
            "rollAfter",
            "cardPlayed",
            "targeted",
            "payChoose",
            // v23 hook points
            "turnEndBefore",
            "turnEndAfter",
            "payAdd",
            "payMul",
            "payAt",
            "discarded",
            "deckBeforeGame",
            "deckAtGameStart",
            "drew",
            "reshuffled",
            "bought",
            "circleAffected",
            "settleInstead",
            "beforeOut",
            "teleported",
            "rollPlan",
            "abnormalGuard",
            "immuneAll",
            "untargetable",
            "redirect",
            // v28 post-commit points (what a committed effect did)
            "fireSpent",
            "skillUsed",
            "houseAdded",
            // v29: crystal writes
            "crystalsChanged",
        ] {
            assert!(
                !matches!(trigger_kind(k), TriggerKind::None),
                "{k} folded to TriggerKind::None"
            );
        }
    }
}
