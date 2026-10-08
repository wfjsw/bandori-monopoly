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
use game_core::engine::{Ask, CardRules, Cx, Dest, Flow, Halt, Trigger as CoreTrigger};
use game_core::msg::Msg;

use crate::host::{
    Call, CardModules, HookRun, HostRequest, Outcome, Prompt, PromptOption, RuleError, Ruleset,
};
use crate::inline::InlineHost;
use crate::world::{CardWorld, Trigger};
use crate::PromptKind;
use crate::{CardPile, TriggerKind};


/// Declared static properties of one card, from a [`Run`]'s snapshot.
fn props_of(
    props: &std::collections::HashMap<String, std::collections::BTreeMap<String, i32>>,
    id: &str,
) -> std::collections::BTreeMap<String, i32> {
    props.get(id).cloned().unwrap_or_default()
}

/// One run of a card module against a copy of the match world.
/// C# `PlayCtx.Dest` values the module returns.
const DEST_GRAVEYARD: i32 = 0;
const DEST_FIELD: i32 = 2;
/// The run never named a fate. Distinct from [`DEST_GRAVEYARD`] on purpose: a
/// play with no opinion still lands in the discard (`dest_from`'s wildcard),
/// but a field effect with no opinion must leave its card where it is.
const DEST_UNSET: i32 = -1;

/// C# `MatchHost._counteractDepth > 4` is a runaway net; a counter-war is bounded by
/// hands shrinking as cards declare. 16 is plenty for a legal exchange.
const MAX_COUNTERACT_DEPTH: u32 = 16;

/// Hard cap on the offers one seat gets in a single visit of the ask ring
/// (ruling 2026-10-07: a seat keeps the floor until it passes or runs out of
/// eligible counteractions). The offer set shrinks as cards are declared, so a
/// visit terminates on its own; the cap is a runaway guard, not a design limit.
/// TODO(规则书): the book states no per-visit bound.
const MAX_COUNTERACT_PER_VISIT: u32 = 16;

#[derive(Clone)]
pub struct Run {
    world: game_core::engine::World,
    data: Arc<GameData>,
    /// Declared static properties (`card_props`) of every card in the loaded
    /// set, snapshotted at run creation -- the one thing a `Run` needs from the
    /// card modules. A snapshot (not a `CardModules` handle) keeps `Run`
    /// concrete so the native backend can hold one in a thread-local.
    props: Arc<std::collections::HashMap<String, std::collections::BTreeMap<String, i32>>>,
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
    /// Cards this run drew, `(player_id, id)` -- the engine raises `drawn` /
    /// `drew` for each once the run commits (the per-draw after points).
    draw_log: Vec<(i32, String)>,
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
    /// [CP点] writes this run made, `(owner, card, change)` -- the engine raises
    /// `cpChanged` for each once the run commits (the same shape as
    /// `crystals_log`). `change` is the signed delta the write applied to an
    /// instance's **on-card** [CP点] count (`FieldCard::cp`, 「自己[场上]N个
    /// [CP点]」 -- user ruling 2026-10-07); the count it left is `cp_attached()`
    /// on that instance. Tile-mark writes (`place_cp` / `clear_cp`) are the
    /// other [CP点] kind and do not ride this log.
    cp_log: Vec<(i32, String, i32)>,
    /// Players this run granted [除外] layers to -- the engine raises `exile`
    /// for each once the run commits (「任意玩家获得[除外]…时」 handlers).
    exile_log: Vec<i32>,
    /// C# `PlayCtx.Doubled`: which of the card's numbers this play doubles, or
    /// -1. Set by the doubling band skill, which is not ported yet.
    doubled: i32,
    /// Props the running card wants on the lingering instance `ctx::linger`
    /// binds (`docs/PURCHASE.md` P5). A hand play has no field instance for
    /// `set_prop` to write, so a `set_prop` against no instance parks the value
    /// here and `linger` carries it onto [`crate::world::Lingering::props`].
    linger_props: std::collections::BTreeMap<String, i32>,
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

    /// Record an **on-card** [CP点] write against the instance at `uid` so the
    /// commit point can raise `cpChanged` (the same shape as
    /// [`Self::note_crystals`]). `was` and `now` bracket that instance's
    /// `FieldCard::cp`, so a write that lands on the same count still raises
    /// and a count emptied by *any* path -- this run's settle, another effect's
    /// removal -- leaves the field the same way. A write with no live instance
    /// raises nothing.
    fn note_cp(&mut self, uid: i32, was: i32, now: i32) {
        let Some(f) = self.world.field_by_uid(uid) else {
            return;
        };
        let (owner, card) = (f.owner, f.card.clone());
        self.cp_log.push((owner, card, now - was));
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
        let got = self.world.gain_fixed(player_id, amount, why);
        // 资金变动 (rulebook 支付阶段 7): a fixed gain/loss is still a money
        // change. Logged as `(-1, player, n)` for money in and `(player, -1, n)`
        // for money out, so the Done handler can raise `payAfter` for both and
        // open the `paid` [反击] window only for the loss.
        if got > 0 {
            self.paid_log.push((-1, player_id, got));
        } else if got < 0 {
            self.paid_log.push((player_id, -1, -got));
        }
        got
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
        let got = self.world.gain_money(player_id, amount, src);
        if got > 0 {
            self.paid_log.push((-1, player_id, got));
        }
        got
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
        let before = self
            .world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.hand.len())
            .unwrap_or(0);
        let got = self.world.draw_cards(player_id, n, true);
        if got > 0 {
            if let Some(h) = self.world.hidden.get(player_id.max(0) as usize) {
                for id in &h.hand[before..] {
                    self.draw_log.push((player_id, id.clone()));
                }
            }
        }
        got
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
        let props = props_of(&self.props, card);
        let uid = self.world.place_card_on(&self.data, player_id, tile, card, note, props);
        if card == self.current_card {
            self.current_uid = uid;
        }
        uid
    }
    fn place_card(&mut self, player_id: i32, card: &str, note: Msg) -> i32 {
        let props = props_of(&self.props, card);
        let uid = self.world.place_card(&self.data, player_id, card, note, props);
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
    fn event_expire(&mut self, id: &str, removed: bool) {
        self.world.expire_event(id, removed);
    }
    fn event_is_active(&self, id: &str) -> bool {
        self.world.event_is_active(id)
    }
    fn event_deck_push(&mut self, id: &str, face_down: bool) {
        self.world.event_deck_push(id, face_down);
    }
    fn event_banish(&mut self, id: &str) {
        self.world.event_banish(id);
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
        if let Some(c) = self.data.card(card) {
            return c.text.contains(needle);
        }
        // A `skill:<owner>:<skill>` rule id is not in `cards.json`; the skill's
        // text is the owner's (character's or band's) `text` field
        // (发送熊饼表情 「所有技能中含有火罐的玩家」).
        if let Some(rest) = card.strip_prefix("skill:") {
            if let Some((owner, _)) = rest.split_once(':') {
                if let Some(c) = self.data.character(owner) {
                    return c.text.contains(needle);
                }
                if let Some(b) = self.data.bands.iter().find(|b| b.name == owner) {
                    return b.text.contains(needle);
                }
            }
        }
        false
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
    fn self_prop(&self, key: &str) -> i32 {
        self.world.prop_at(self.current_uid, key)
    }
    fn set_self_prop(&mut self, key: &str, value: i32) -> i32 {
        self.world.set_prop_at(self.current_uid, key, value)
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
    fn prop_at(&self, uid: i32, key: &str) -> i32 {
        self.world.prop_at(uid, key)
    }
    fn set_prop_at(&mut self, uid: i32, key: &str, value: i32) -> i32 {
        if uid < 0 {
            // No field instance to carry the prop (a hand play, or the run is
            // binding a lingering instance). Park it for `ctx::linger`, which
            // puts it on the turn-scoped instance (`docs/PURCHASE.md` P5).
            self.linger_props.insert(key.to_string(), value);
            return value;
        }
        self.world.set_prop_at(uid, key, value)
    }
    fn tile_prop(&self, tile: i32, key: &str) -> i32 {
        self.world.tile_prop(tile, key)
    }
    fn set_tile_prop(&mut self, tile: i32, key: &str, value: i32) -> i32 {
        self.world.set_tile_prop(tile, key, value)
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
        // Marker ownership (user ruling 2026-10-07): the rule that creates a
        // mark owns it -- `skill:要乐奈` owns every 抹茶芭菲 copy, on any tile
        // or player counter.
        let who = self.current_card.clone();
        self.world.note_marker_owner(kind, &who);
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
    // [CP点] -- the two kinds (user ruling 2026-10-07). **Tile marks** are the
    // `mark:cp` owner's API (`place_cp` / `count_cp` / `count_cp_from` /
    // `clear_cp` / `cp_src_at`): neutral marks with the placing instance as
    // provenance, never owned by a player. **On-card** [CP点] (`cp_attached` /
    // `add_cp` / `cp_at` / `add_cp_at`) is `FieldCard::cp` -- 「自己[场上]N个
    // [CP点]」, the CP points attached to the card itself, the card rule's own
    // stock. Only on-card writes ride `note_cp` (→ `cpChanged`, the graveyard
    // rule); a tile-mark write is the other kind and raises nothing.
    fn place_cp(&mut self, tile: i32, note: Msg) -> i32 {
        let me = self.current_uid;
        let card = self
            .world
            .field_by_uid(me)
            .map(|f| f.card.clone())
            .unwrap_or_default();
        // A fresh mark attaches to the placer. Stacking onto an existing mark
        // keeps that mark's attachment (the placement rule forbids it -- 「没有
        // [CP点]的格子」 -- so this is the defensive path).
        self.world.add_cp_mark(tile, me, &card, note)
    }
    fn count_cp(&self, tile: i32) -> i32 {
        self.world.count_cp(tile)
    }
    fn count_cp_from(&self, tile: i32) -> i32 {
        self.world.count_cp_from(tile, self.current_uid)
    }
    fn clear_cp(&mut self, tile: i32) -> i32 {
        self.world.clear_cp(tile)
    }
    fn cp_src_at(&self, tile: i32) -> i32 {
        self.world.cp_src_at(tile)
    }
    fn cp_attached(&self) -> i32 {
        self.world.cp_at(self.current_uid)
    }
    fn add_cp(&mut self, n: i32, max: i32) -> i32 {
        let uid = self.current_uid;
        let was = self.world.cp_at(uid);
        let now = self.world.add_cp_at(uid, n, max);
        self.note_cp(uid, was, now);
        now
    }
    fn cp_at(&self, uid: i32) -> i32 {
        self.world.cp_at(uid)
    }
    fn add_cp_at(&mut self, uid: i32, n: i32, max: i32) -> i32 {
        let was = self.world.cp_at(uid);
        let now = self.world.add_cp_at(uid, n, max);
        self.note_cp(uid, was, now);
        now
    }
    fn tok_names(&self, player_id: i32, prefix: &str) -> Vec<String> {
        self.world.tok_names(player_id, prefix)
    }
    fn tok(&self, player_id: i32, name: &str) -> i32 {
        self.world.tok(player_id, name)
    }
    fn set_tok(&mut self, player_id: i32, name: &str, value: i32) {
        // Marker ownership (user ruling 2026-10-07): the rule that creates a
        // marker owns it, wherever its copies sit.
        let who = self.current_card.clone();
        self.world.note_marker_owner(name, &who);
        self.world.set_tok(player_id, name, value);
    }
    fn add_tok(&mut self, player_id: i32, name: &str, n: i32, max: i32) -> i32 {
        let who = self.current_card.clone();
        self.world.note_marker_owner(name, &who);
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
    fn band_skill_uid(&self, player_id: i32) -> i32 {
        self.world.band_skill_uid(player_id)
    }
    fn band_skill_id(&self, player_id: i32) -> Option<String> {
        self.world.band_skill_id(player_id)
    }
    fn character_skill_id(&self, player_id: i32) -> Option<String> {
        self.world.character_skill_id(player_id)
    }
    fn band_skills(&self, player_id: i32) -> Vec<(i32, String, i32)> {
        self.world.band_skills(player_id)
    }
    fn add_band_skill(&mut self, player_id: i32, id: &str, extra: bool) -> i32 {
        let props = props_of(&self.props, id);
        self.world
            .add_band_skill(&self.data, player_id, id, extra, props)
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
        if n > 0 {
            // 「任意玩家获得[除外]…时」 -- the grant is a raise point (embers).
            self.exile_log.push(player_id);
        }
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
    fn is_color(&self, player_id: i32, tile: i32, group: i32) -> bool {
        // The tile-prop colour reader (`docs/PURCHASE.md`): the tile's own
        // `TileData.group`, `prop::ANY_COLOR` (「该格获得所有颜色」), or this
        // player's `colorFor:<p>` -- which may name `group` or
        // `prop::ALL_COLORS` for "all colours".
        let any = self.world.tile_prop(tile, game_core::state::prop::ANY_COLOR) != 0;
        let color_for = self.world.tile_prop(
            tile,
            &format!("{}{}", game_core::state::prop::COLOR_FOR_PREFIX, player_id),
        );
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| {
                t.group == group
                    || t.group == game_core::state::prop::ALL_COLORS
                    || any
                    || color_for == group
                    || color_for == game_core::state::prop::ALL_COLORS
            })
    }
    fn paid_in_settle(&self) -> i32 {
        self.world.turn.paid_in_settle
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
        // C# `H.IsLiveHouse` = `IsColor(t, 6) && IsBuyable(t)`. The per-player
        // colour override is a tile prop, so see `is_live_house_for`.
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
    fn rent_houses_of(&self, tile: i32) -> i32 {
        self.world.rent_houses(tile)
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
    fn gains_this_turn(&self, player_id: i32) -> i32 {
        self.world.gains_this_turn(player_id)
    }
    fn designations(&self, player_id: i32) -> Vec<i32> {
        // `Card.Def.Targeting` + `H.Others`: a play that names recipients names
        // the other living players of its user (「[指定][使用者]以外的所有玩家」
        // / 「其他玩家[分摊]」). The play's card is the one the trigger names.
        let card = &self.trigger.card;
        let designates = props_of(&self.props, card)
            .get(game_core::state::prop::DESIGNATES)
            .copied()
            .unwrap_or(0)
            != 0;
        if !designates {
            return Vec::new();
        }
        self.world.others(player_id)
    }
    fn cancel_designation(&mut self, seat: i32) {
        if !self.world.turn.cancelled_designations.contains(&seat) {
            self.world.turn.cancelled_designations.push(seat);
        }
    }
    fn designation_cancelled(&self, seat: i32) -> bool {
        self.world.turn.cancelled_designations.contains(&seat)
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
    fn set_trigger_price(&mut self, v: i32) {
        self.trigger.price = v;
    }
    fn set_trigger_deal_owner(&mut self, v: i32) {
        self.trigger.deal_owner = v;
    }
    fn set_trigger_deal_houses(&mut self, v: i32) {
        self.trigger.deal_houses = v;
    }
    fn set_trigger_deal_mortgaged(&mut self, v: i32) {
        self.trigger.deal_mortgaged = v != 0;
    }
    fn set_trigger_reason(&mut self, reason: &str) {
        self.trigger.reason = reason.to_string();
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
    fn set_roller(&mut self, v: i32) {
        // `plan::set_roller` -- the stand-in roller for 「上一名玩家代替进行此次
        // 投掷」. `MoveCtx::roller` rides `TurnCtx.plan`, so `main_move`'s
        // post-`rollPlan` re-clone picks it up.
        if v >= 0 {
            self.world.turn.plan.roller = v as usize;
        }
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
    fn plan_add_follower(&mut self, player_id: i32) {
        self.world.plan_add_follower(player_id);
    }
    fn set_more_steps(&mut self, v: i32) {
        self.world.set_more_steps(v);
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
pub struct RulesBridge<M: CardModules> {
    ruleset: M,
    data: Arc<GameData>,
}

/// The sandboxed bridge: `CardRules` over wasmi/wasmtime card modules.
pub type WasmRules = RulesBridge<Ruleset>;

impl RulesBridge<Ruleset> {
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

}

/// The inline host one drive installs around its guest call in simulation
/// mode (`docs/BOT.md` §3.2). Holds raw pointers to the drive's locals (the
/// bridge, the engine `Cx`, the card id, the drive's answer log) so it can be
/// `'static` on the thread-local stack; all of them outlive the
/// `with_inline_host` frame the drive wraps the guest call in.
///
/// **Prompts inline, host requests through the loop.** A player prompt is
/// answered as it happens ([`Cx::ask`] against the provider), so it no longer
/// costs a body re-run -- the big win B2 is after, since a routine with *p*
/// prompts and *h* host requests used to run *p*+*h*+1 times. A
/// [`HostRequest`] still pauses and is applied by the drive's `NeedHost` arm
/// against the live world before the body re-runs: that is what makes the
/// final state `guest_writes(live + host_effects)` -- the guest's writes *on
/// top* of the host effects -- exactly as the replay model derives it. A
/// single forward pass that applied the requests to the guest's own world
/// interleaves the two and diverges (the dice stream ordering changes too).
struct DriveInline<M: CardModules> {
    bridge: *const RulesBridge<M>,
    cx: *mut Cx<'static>,
    call: Call,
    card_id: *const str,
    halt: Option<Halt>,
    /// Prompts this run answered inline, in consumption order -- appended to
    /// the drive's answer log when the guest call returns, so the next run
    /// (there is one per `HostRequest`) finds them instead of asking again.
    answers: Vec<i32>,
}

impl<M: CardModules> DriveInline<M> {
    /// The type-erased world `HostCtx` hands back is always a [`Run`]:
    /// `CardModules` pins it. (Both backends share `hostfns`, which stays
    /// generic over `HostCtx::World`.)
    fn as_run<'r>(any: &'r mut dyn std::any::Any) -> &'r mut Run {
        any.downcast_mut::<Run>()
            .expect("CardModules pins HostCtx::World = Run")
    }
}

impl<M: CardModules> crate::inline::InlineHost for DriveInline<M> {
    /// Answer a player prompt through the engine's [`Cx::ask`] -- the provider
    /// decides inline, or declines and this pauses. Runs against the **live**
    /// world (no swap), so `fill_ai` / `ask_seq` advance exactly where the
    /// replay model's `NeedInput` arm advances them. The answer is appended to
    /// the drive's log so a re-run (caused by a later `HostRequest`) finds it.
    fn answer(&mut self, _any: &mut dyn std::any::Any, p: Prompt) -> Option<i32> {
        let cx = unsafe { &mut *self.cx };
        let ask = prompt_to_ask(p);
        match cx.ask(ask) {
            Ok(reply) => {
                let v = reply
                    .a
                    .answers
                    .first()
                    .copied()
                    .filter(|&x| x >= 0)
                    .unwrap_or(reply.fallback);
                self.answers.push(v);
                Some(v)
            }
            Err(h) => {
                self.halt = Some(h);
                None
            }
        }
    }

    /// Apply a [`HostRequest`] against the **live** world, with the same
    /// `adopt_turn_policy` / `overlay_guest_state` preamble the replay's
    /// `NeedHost` arm uses. The body keeps going (one forward pass); the drive
    /// then runs one commit pass with the answers pre-filled so the guest's
    /// writes land on top of the host effects, same as the replay's last pass.
    /// Two guest runs per body, not *k*+1.
    fn apply(&mut self, any: &mut dyn std::any::Any, req: HostRequest) -> Option<i32> {
        let run = Self::as_run(any);
        let cx = unsafe { &mut *self.cx };
        let bridge = unsafe { &*self.bridge };
        let card_id: &str = unsafe { &*self.card_id };
        cx.adopt_turn_policy(&run.world);
        cx.overlay_guest_state(&run.world);
        let linger = run.linger_props.clone();
        match bridge.apply_host_request(cx, req, &linger, self.call, card_id) {
            Ok(v) => {
                self.answers.push(v);
                Some(v)
            }
            Err(h) => {
                self.halt = Some(h);
                None
            }
        }
    }

    fn take_halt(&mut self) -> Option<Halt> {
        self.halt.take()
    }
}

impl<M: CardModules> RulesBridge<M> {
    pub fn new(ruleset: M, data: Arc<GameData>) -> Self {
        Self { ruleset, data }
    }

    /// The ruleset (module list, hashes) for inspection.
    pub fn ruleset(&self) -> &M {
        &self.ruleset
    }

    /// Snapshot every card's declared static properties for a [`Run`].
    fn modules_props(
        &self,
    ) -> Arc<std::collections::HashMap<String, std::collections::BTreeMap<String, i32>>> {
        let mut m = std::collections::HashMap::new();
        for c in self.ruleset.cards() {
            m.insert(c.id.clone(), c.props.clone());
        }
        Arc::new(m)
    }

    /// Run one effect to completion, prompting through the engine as needed.
    /// Returns the card's destination (`PlayCtx.Dest`). A reroll the module made
    /// (`set_move_roll`) is written back to `trigger` (C# shares `t.Move`).
    fn drive(
        &self,
        cx: &mut Cx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
    ) -> Flow<i32> {
        self.drive_inner(cx, call, card_id, uid, trigger, false)
    }

    /// As [`drive`], but ask the hook's guard first and skip the body when it
    /// refuses -- on the same instantiation, so a fired hook costs one fire-up
    /// and one world copy rather than the two a `hook_guard` + `drive` pair
    /// cost. The "a guard passed" announcement is made here, once, on the first
    /// pass (a prompt re-runs the module).
    fn drive_hook(
        &self,
        cx: &mut Cx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
    ) -> Flow<i32> {
        // A hook cannot re-trigger on its own money movement.
        if uid >= 0 && is_money_hook_kind(trigger.kind) && cx.reentrant_hooks.contains(&uid) {
            return Ok(0);
        }
        self.drive_inner(cx, call, card_id, uid, trigger, true)
    }

    fn drive_inner(
        &self,
        cx: &mut Cx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
        guarded: bool,
    ) -> Flow<i32> {
        // A hook cannot re-trigger on its own movement: while this uid's hook
        // runs, nested money pipelines skip it. This is the termination
        // argument for `MAX_MONEY_DEPTH` (see `play.rs`).
        if uid >= 0 {
            cx.reentrant_hooks.push(uid);
        }
        let result = self.drive_inner_body(cx, call, card_id, uid, trigger, guarded);
        if uid >= 0 {
            cx.reentrant_hooks.pop();
        }
        result
    }

    fn drive_inner_body(
        &self,
        cx: &mut Cx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
        guarded: bool,
    ) -> Flow<i32> {
        let mut answers: Vec<i32> = Vec::new();
        // Guest-state overlays this drive pushed (one per `NeedHost`). Dropped
        // at the top of the next iteration -- the routine has finished and the
        // replay re-applies the guest writes from the restored world. Nested
        // drives record their own base, so they never pop an outer one.
        let overlay_base = cx.guest_overlay_depth();
        loop {
            cx.restore_guests_to(overlay_base);
            let run = Run {
                world: cx.world_copy(),
                data: self.data.clone(),
                props: self.modules_props(),
                trigger: trigger.clone(),
                current_card: card_id.to_string(),
                current_uid: uid,
                dest: DEST_UNSET,
                dest_to: None,
                paid_log: vec![],
                discard_log: vec![],
                draw_log: vec![],
                reshuffle_log: vec![],
                fire_spent_log: vec![],
                house_log: vec![],
                crystals_log: vec![],
                cp_log: vec![],
            exile_log: vec![],
                doubled: -1,
                linger_props: Default::default(),
            };
            // Simulation mode (`docs/BOT.md` §3.2): with a provider installed
            // the body must run **once** -- every pause is answered inline
            // (see `crate::inline` / [`DriveInline`]), so the loop below runs
            // a single pass. The default (no provider) path is unchanged.
            //
            // The guest call is the only thing the inline host wraps: it must
            // not hold a borrow of `cx` across it (the host reaches `cx`
            // through a raw pointer from inside the guest's host imports).
            let mut inline = cx.has_provider().then(|| DriveInline {
                bridge: self as *const _,
                cx: cx as *mut Cx<'_> as *mut Cx<'static>,
                call,
                card_id: card_id as *const str,
                halt: None,
                answers: Vec::new(),
            });
            let mut announced = false;
            let mut guest = || -> Result<Option<Result<Outcome<Run>, RuleError>>, RuleError> {
                if guarded {
                    match self.ruleset.run_hook(&run, call, &answers) {
                        Err(e) => Ok(Some(Err(e))),
                        // Not activated -- the guard refused. Nothing ran, and
                        // nothing about the card reaches the UI.
                        Ok(None) => Ok(None),
                        Ok(Some(hr)) => {
                            if hr.announced && answers.is_empty() {
                                announced = true;
                            }
                            Ok(Some(Ok(hr.outcome)))
                        }
                    }
                } else {
                    Ok(Some(self.ruleset.run(&run, call, &answers)))
                }
            };
            // `run_hook` cannot actually fail here (its `Err` arm above is
            // unreachable in practice); the double `Result` is so the closure
            // stays a plain `FnOnce` over the borrows it needs.
            let wrapped = match inline.as_mut() {
                Some(h) => crate::inline::with_inline_host(h, guest),
                None => guest(),
            };
            // A nested prompt the provider declined parks a [`Halt`] on the
            // inline host; propagate it and drop the body (the engine routine
            // re-runs from its snapshot, as today).
            if let Some(h) = inline.as_mut().and_then(|h| h.take_halt()) {
                return Err(h);
            }
            // Prompts answered inline this run go on the drive's log **in
            // consumption order**, before whatever pause ended the run appends
            // its own answer -- the same bookkeeping the `NeedInput` arm does.
            if let Some(h) = inline.as_mut() {
                answers.append(&mut h.answers);
            }
            if announced {
                cx.log(
                    call.player_id(),
                    Msg::new("log.hook_fire")
                        .player_id("who", call.player_id())
                        .card("card", card_id.to_string()),
                );
            }
            let outcome = match wrapped {
                Ok(Some(outcome)) => outcome,
                // Guard refused.
                Ok(None) => return Ok(DEST_UNSET),
                Err(e) => Err(e),
            };
            match outcome {
                Ok(Outcome::Done(after)) => {
                    // Simulation mode (`docs/BOT.md` §3.2): the learn pass
                    // above ran the body once with every pause answered
                    // inline against the **live** world, so the host effects
                    // are already on `cx.w` and this `after` carries only the
                    // guest's writes. The replay model commits
                    // `guest_writes(live + host_effects)` -- the guest's writes
                    // *on top* of the host effects -- so when any pause was
                    // answered we run one commit pass with the collected
                    // answers pre-filled and commit that instead. Two guest
                    // runs per body, not *k*+1, and the same final world.
                    if answers.is_empty() {
                        return self.commit_after(cx, after, call, card_id, trigger);
                    }
                    let run = Run {
                        world: cx.world_copy(),
                        data: self.data.clone(),
                        props: self.modules_props(),
                        trigger: trigger.clone(),
                        current_card: card_id.to_string(),
                        current_uid: uid,
                        dest: DEST_UNSET,
                        dest_to: None,
                        paid_log: vec![],
                        discard_log: vec![],
                        draw_log: vec![],
                        reshuffle_log: vec![],
                        fire_spent_log: vec![],
                        house_log: vec![],
                        crystals_log: vec![],
                        cp_log: vec![],
                        exile_log: vec![],
                        doubled: -1,
                        linger_props: Default::default(),
                    };
                    let outcome2 = if guarded {
                        match self.ruleset.run_hook(&run, call, &answers) {
                            Err(e) => Err(e),
                            Ok(None) => return Ok(DEST_UNSET),
                            Ok(Some(hr)) => Ok(hr.outcome),
                        }
                    } else {
                        self.ruleset.run(&run, call, &answers)
                    };
                    match outcome2 {
                        Ok(Outcome::Done(after2)) => {
                            return self.commit_after(cx, after2, call, card_id, trigger);
                        }
                        // The commit pass must not pause: the learn pass
                        // already answered everything. Anything else is a
                        // body that is nondeterministic across the two passes;
                        // fall back to the learn pass's world so the effect
                        // still lands.
                        _ => {
                            return self.commit_after(cx, after, call, card_id, trigger);
                        }
                    }
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
                Ok(Outcome::NeedHost(req, mut run)) => {
                    // The host routine runs against the **live** world and is
                    // not replayed, so the turn-ctx policy the card just set up
                    // (build/buy discounts, free buy, ...) has to cross now.
                    // Progress counters stay on the copy: the replay redoes them.
                    cx.adopt_turn_policy(&run.world);
                    // The guest's pending player state (a status clear, e.g.
                    // 壱雫空 zeroing [晕眩] and then paying) is overlaid for the
                    // routine's duration -- `can_pay` must see it -- and dropped
                    // at the next iteration's top, so the replay re-applies it
                    // rather than double-counting. See `Cx::overlay_guest_state`.
                    cx.overlay_guest_state(&run.world);
                    let v = self.apply_host_request(cx, req, &run.linger_props, call, card_id)?;
                    answers.push(v);
                }
                Err(e) => {
                    // A trapping module must not take the match down with it.
                    let detail = match &e {
                        RuleError::Trap(m) | RuleError::Load(m) | RuleError::DuplicateCard(m) => {
                            m.clone()
                        }
                        RuleError::NoSuchCard(i) => format!("no such card {i}"),
                        RuleError::GuardPrompted => "guard prompted".into(),
                        RuleError::BadPre {
                            card,
                            entry,
                            source,
                            err,
                        } => format!("bad guard pre on {card}/{entry}: {err} ({source})"),
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


    /// Apply one [`HostRequest`] against `cx` and answer the guest with the
    /// value it wants back (`paid.moved()`, the gate's 0/1, ...).
    ///
    /// Shared by the replay path (the drive loop pauses, applies, and re-runs
    /// the body with the answer appended) and the simulation path (the inline
    /// host applies it mid-body against the guest's own world -- see
    /// [`crate::inline`] and `docs/BOT.md` §3.2). The caller runs
    /// `adopt_turn_policy` / `overlay_guest_state` first in the replay path
    /// only: in a single forward pass the guest's writes are already real on
    /// the world the routine mutates.
    fn apply_host_request(
        &self,
        cx: &mut Cx,
        req: HostRequest,
        linger_props: &std::collections::BTreeMap<String, i32>,
        call: Call,
        card_id: &str,
    ) -> Flow<i32> {
        match req {
        HostRequest::Gate { player_id, kind } => {
            let allowed = self.abnormal_gate(cx, player_id, kind, call.player_id())?;
            return Ok(allowed as i32);
        }
        HostRequest::Target {
            player_id,
            tile,
            single,
        } => {
            let got = if tile >= 0 {
                self.target_tile(cx, tile, call.player_id(), card_id)?
            } else {
                self.target_player(cx, player_id, call.player_id(), card_id, single)?
            };
            return Ok(got);
        }
        // C# `H.CardMove(c, m)`: the card shaped the plan and asked for
        // the move to run now. The engine runs it (it may prompt), then
        // the effect replays past this call.
        HostRequest::Move { player_id, mut plan } => {
            // An event-driven move is not a main move (`docs/EVENTS.md`):
            // 「移动X」 / 「移动1d20」 go through even when the turn's
            // main move is already spent. Mark it so `card_move` does
            // not consume (or refuse on) `main_moved`.
            if card_id.starts_with("event:") {
                plan.forced = true;
            }
            // Forced moves go through the abnormal gate so [反击] cards
            // (安可, 像往常一样) get their window. `AbKind::Forced`
            // covers both other-player and self-applied forced moves.
            let allowed = self.abnormal_gate(
                cx,
                player_id,
                crate::AbKind::Forced,
                call.player_id(),
            )?;
            if allowed {
                cx.card_move(player_id.max(0) as usize, plan)?;
            }
            return Ok(allowed as i32);
        }
        // C# `H.ForceTeleport(..., resolve: false)` / a bare `pos`
        // write: the gate and the write land on the **live** world here
        // (not on the run's copy), so a `card_move` that follows in the
        // same body starts at the destination -- and the replay skips
        // the call (the answer below), so it cannot re-teleport over a
        // move the engine already ran.
        HostRequest::Teleport { player_id, tile } => {
            let allowed = self.abnormal_gate(
                cx,
                player_id,
                crate::AbKind::Teleport,
                call.player_id(),
            )?;
            if allowed {
                let mut w = cx.world_copy();
                w.teleport_to(player_id, tile);
                cx.swap_world(w);
            }
            return Ok(allowed as i32);
        }
        // C# `H.AgentLanding`: the 「星光代理」 landing routine.
        HostRequest::AgentLanding { player_id, agent } => {
            cx.agent_landing(player_id.max(0) as usize, agent.max(0) as usize)?;
            return Ok(1);
        }
        // The rest of the routine family -- see `HostRequest`.
        HostRequest::SettleAt {
            player_id,
            tile,
            main,
        } => {
            cx.card_settle_at(player_id.max(0) as usize, tile.max(0) as usize, main)?;
            return Ok(1);
        }
        HostRequest::Buy {
            player_id,
            tile,
            kind,
        } => {
            cx.card_buy(player_id.max(0) as usize, tile.max(0) as usize, kind)?;
            return Ok(1);
        }
        // v40 purchase surface. P0: stubs; engine-side dispatch lands
        // with P1 (property buy) / P2 (agent) / P3 (force & acquire) /
        // P5 (linger).
        HostRequest::BuyQuotes {
            player_id: _,
            kind: _,
            tiles,
            out: _,
        } => {
            // P0: every tile quotes at its native price, eligible if
            // buyable. The hook-aware quote lands at P1.
            let _ = tiles;
            return Ok(1);
        }
        HostRequest::Acquire {
            player_id,
            from,
            tile,
            price,
        } => {
            cx.card_acquire(
                player_id.max(0) as usize,
                from.max(0) as usize,
                tile.max(0) as usize,
                price,
            )?;
            return Ok(1);
        }
        HostRequest::AgentOffer {
            player_id: _,
            agent: _,
            tile: _,
            kind: _,
        } => {
            // P2 wires the agent offer; P0 is a no-op.
            return Ok(0);
        }
        HostRequest::Linger {
            player_id,
            expires,
        } => {
            // The instance binds the **running** card's def, and carries
            // whatever props the body parked via `set_prop` against no
            // field instance (`docs/PURCHASE.md` P5).
            cx.card_linger(
                player_id.max(0) as usize,
                expires,
                card_id,
                linger_props.clone(),
            );
            return Ok(1);
        }
        HostRequest::Build { player_id, tile } => {
            cx.card_build(player_id.max(0) as usize, tile.max(0) as usize)?;
            return Ok(1);
        }
        HostRequest::OfferBuild { player_id, tiles } => {
            let tiles: Vec<usize> = tiles
                .into_iter()
                .filter(|&t| t >= 0)
                .map(|t| t as usize)
                .collect();
            cx.card_offer_build(player_id.max(0) as usize, &tiles, card_id)?;
            return Ok(1);
        }
        HostRequest::Mortgage { player_id, tile } => {
            cx.card_mortgage(player_id.max(0) as usize, tile.max(0) as usize)?;
            return Ok(1);
        }
        HostRequest::DrawEvent { player_id } => {
            cx.card_draw_event(player_id.max(0) as usize)?;
            return Ok(1);
        }
        HostRequest::PayRent {
            player_id,
            tile,
            half,
        } => {
            cx.card_pay_rent(player_id.max(0) as usize, tile.max(0) as usize, half)?;
            return Ok(1);
        }
        HostRequest::OfferBuy { player_id, tile } => {
            cx.card_offer_buy(player_id.max(0) as usize, tile.max(0) as usize)?;
            return Ok(1);
        }
        HostRequest::OfferForceBuy { player_id, tile } => {
            cx.card_offer_force_buy(player_id.max(0) as usize, tile.max(0) as usize)?;
            return Ok(1);
        }
        HostRequest::OfferBuildOne { player_id, tile } => {
            cx.card_offer_build_one(player_id.max(0) as usize, tile.max(0) as usize)?;
            return Ok(1);
        }
        // `H.CircleReward`: the [经过] CiRCLE reward. The engine runs
        // the whole step -- suppression via `prop::NO_REWARD` on the
        // tile's `tile:circle` instance, the choice, the `circleAffected`
        // window, the payout. `docs/TILES.md`'s `ctx::settle_circle_reward`.
        HostRequest::CircleReward {
            player_id,
            landing,
        } => {
            cx.card_circle_reward(player_id.max(0) as usize, landing)?;
            return Ok(1);
        }
        // A card- or skill-driven dice roll. The engine rolls and raises
        // the `Roll` chain link -- the 「掷骰结算前」 [反击] window (Y.O.L.O
        // 「你的任意掷骰结算前」, 寄于指尖的执念 「当你使用火罐进行掷骰时」)
        // -- with the roller, the face and the `roll_source` code, then
        // answers with whatever a counteraction left on `value`.
        HostRequest::Roll {
            player_id,
            count,
            sides,
            source,
        } => {
            // `sides == 0` is the `do_move_roll` shape: sum the move
            // plan's dice tables instead of `count`d`sides`.
            let face = if sides == 0 {
                cx.card_do_move_roll(player_id.max(0) as usize)
            } else {
                cx.card_roll(player_id.max(0) as usize, count, sides)
            };
            let t = self.raise_core(cx, "roll", player_id, |t| {
                t.value = face;
                t.roll_source = source;
            })?;
            return Ok(t.value.max(0));
        }
        // C# `f.Bought(i, t)` -- the card announces an acquisition it
        // performed outside the buy routine (tomoe_savior). The `bought`
        // hook chain runs over the field; `by_card` is the run's player.
        HostRequest::RaiseBought { player_id, tile } => {
            cx.card_raise_bought(
                player_id.max(0) as usize,
                tile.max(0) as usize,
                call.player_id().max(0) as usize,
            )?;
            return Ok(1);
        }
        // Marker spend / gain (user ruling 2026-10-07): the marker's own
        // [反击] window opens **before** the markers move. A counteraction
        // that cancels the link stops the move outright (answer `0`); nothing
        // is spent then. Marker *costs* otherwise keep today's timing --
        // spent as the effect resolves, and a whole-effect negation before
        // the body already prevents them.
        HostRequest::Marker {
            player_id,
            name,
            delta,
        } => {
            let kind = if delta < 0 { "markerSpend" } else { "markerGain" };
            let t = self.raise_core(cx, kind, player_id, |t| {
                t.value = delta;
                t.card = name.clone();
            })?;
            return Ok(if t.is_cancelled() { 0 } else { 1 });
        }
        // `H.DrawR`: the engine runs the draw itself -- one card at a
        // time, raising the per-draw points (`drewBefore` / `drawn` /
        // `drew`) on each -- so a card-driven draw fires the same
        // per-draw effects an engine draw does, and a `drewBefore` hook
        // may replace a card of it. The engine adjudicates each card's
        // `drewBefore` here and answers with how many are plain draws;
        // the effect's replay moves those on its own world copy (the
        // same shape as `Pay`). A hook that replaces a draw has already
        // put its card in hand, so the after points (`drawn` / `drew`)
        // fire for it here.
        HostRequest::Draw { player_id, n } => {
            let p = player_id.max(0) as usize;
            let mut plain = 0i32;
            for _ in 0..n.max(0) {
                let world = cx.world_copy();
                let top = world
                    .hidden
                    .get(p)
                    .and_then(|h| h.draw.last().cloned())
                    .unwrap_or_default();
                let hand_before = world.hidden.get(p).map(|h| h.hand.len()).unwrap_or(0);
                let t = self.raise_core(cx, "drewBefore", player_id, |t| {
                    t.card = top;
                    t.value = 1;
                })?;
                if t.is_cancelled() {
                    // The hook replaced this draw. Whatever it added to
                    // the hand is the replacement draw (「此次加手视为
                    // 抽卡动作」): the after points fire for it.
                    let world = cx.world_copy();
                    if let Some(h) = world.hidden.get(p) {
                        for id in h.hand[hand_before..].to_vec() {
                            self.raise_core(cx, "drawn", player_id, |t| {
                                t.card = id.clone();
                                t.value = 1;
                            })?;
                            self.raise_core(cx, "drew", player_id, |t| {
                                t.card = id.clone();
                                t.value = 1;
                                t.cards = vec![id];
                            })?;
                        }
                    }
                } else {
                    plain += 1;
                }
            }
            return Ok(plain);
        }
        HostRequest::Pay {
            from,
            to,
            amount: asked,
            src,
            total_stage,
        } => {
            let by = Some(call.player_id());
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
                return Ok(0);
            }
            // One pipeline: `Cx::money` runs the staged adjustment
            // points (pre-split `effect` [反击] window -> payTotalAdd /
            // payTotalMul / payTotalCancel -> payAdd / payMul /
            // payChoose / payAt -> split -> post-split `pay` /
            // `payAfter` / `paid`) whatever the cause -- print (game ->
            // player), delete (player -> game) or pay-player. The
            // [反击] window opens here the same way it does for rent, so
            // any card-caused payment is answerable. The money move
            // happens inside, so the guest's `pay`/`gain` resume is
            // answered with the amount and does not move it again.
            let mut p = game_core::engine::Pay::new(asked, "card");
            if from >= 0 {
                p.from = Some(from as usize);
            }
            if to >= 0 {
                p.to = Some(to as usize);
            }
            p.by_card = by;
            p.total_stage = total_stage;
            // Q1 / B1 (`PIPELINE-AUDIT` P11a): a card's forced
            // 「[支付]/[消耗]」 runs the same raise-funds-then-bankrupt
            // path rent does (规则书 L16 「当玩家无法支付某笔支出时（包括
            // 抵押）」 → 折现 + 破产, L76 the mortgage offer). It used to
            // settle as `must = false`, which silently underpaid and
            // could never eliminate anyone. Voluntary purchases
            // (buy/build) keep their pre-gates and stay `must = false`.
            p.must = true;
            // The card's `why` is a **reason** (「登上武道馆」), not a log
            // line: it names the cause and says nothing about who paid
            // what. It rides the standard `log.pay` / `log.lose` /
            // `log.gain` line as the parenthetical `{{src}}`, the same
            // way `Pay::source` carries `src.*`. (It used to be assigned
            // to `Pay::text`, which *replaces* the line -- so every
            // card-driven move logged as the bare card name and a card
            // that moves money several times spammed that name.)
            p.reason = src;
            let paid = cx.money(p)?;
            return Ok(paid.moved());
        }
        HostRequest::PayTotal {
            from,
            to,
            amount: asked,
            src: _,
        } => {
            // `PIPELINE-AUDIT` Q2: the command-wide pre-split stage on
            // its own. No money moves; the answer is the shaped total
            // (or -1 when a `payTotalCancel` hook dropped the command).
            let from = (from >= 0).then_some(from as usize);
            let to = (to >= 0).then_some(to as usize);
            let shaped = cx.pay_total(from, to, asked, Some(call.player_id()))?;
            return Ok(shaped.unwrap_or(-1));
        }
        }
    }


    /// The commit half of a drive: adopt the guest's finished [`Run`] as the
    /// live world and raise everything it logged. Shared by the replay path's
    /// `Done` arm and the simulation path's commit pass (`docs/BOT.md` §3.2).
    fn commit_after(
        &self,
        cx: &mut Cx,
        after: Run,
        call: Call,
        card_id: &str,
        trigger: &mut Trigger,
    ) -> Flow<i32> {
            let dest = after.dest;
            *trigger = after.trigger;
            cx.swap_world(after.world);
            // What the effect did, raised now that it has committed --
            // the same points `money()` / `discard()` raise (C# `Money`
            // PayAfter + `paid`, and `Discarded`).
            let by = Some(call.player_id());
            for (from, to, amount) in after.paid_log {
                // 资金变动 (rulebook 支付阶段 7) fires on **any** money
                // change, merged into `payAfter` per the doc's
                // 「合并到[支付后]?」 -- a print (game -> player) included.
                // The `paid` [反击] window opens only when money left a
                // player (C# 25234), matching 再次牵起手来 / 游击演出.
                let side = if from >= 0 { from } else { to };
                self.raise_core(cx, "payAfter", side, |t| {
                    t.player_id = from;
                    t.target = to;
                    t.value = amount;
                    t.by_card = by;
                })?;
                if from >= 0 {
                    self.raise_core(cx, "paid", from, |t| {
                        t.target = to;
                        t.value = amount;
                        t.by_card = by;
                    })?;
                }
            }
            for (player_id, id) in after.discard_log {
                self.raise_core(cx, "discarded", player_id, |t| t.card = id)?;
            }
            // The per-draw after points (`drawn` = the drawn card's own
            // hook, `drew` = the field-card per-draw point), one raise
            // per single card.
            for (player_id, id) in after.draw_log {
                self.raise_core(cx, "drawn", player_id, |t| {
                    t.card = id.clone();
                    t.value = 1;
                })?;
                self.raise_core(cx, "drew", player_id, |t| {
                    t.card = id.clone();
                    t.value = 1;
                    t.cards = vec![id];
                })?;
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
            // `cpChanged` -- the 该清CP了 「its on-card [CP点] is empty →
            // graveyard」 rule (user ruling 2026-10-07) lives here, for
            // the same reason: a count emptied by *any* write (this
            // run's `add_cp_at`, another effect's removal) leaves the
            // field. The count watched is `FieldCard::cp`, the CP points
            // attached to the card -- not the tile marks.
            for (owner, card, change) in after.cp_log {
                self.raise_core(cx, "cpChanged", owner, |t| {
                    t.card = card;
                    t.value = change;
                    t.by_card = by;
                })?;
            }
            // `exile` -- 「任意玩家获得[除外]…时」 (火种燃尽之后会怎么样呢？
            // 「移除所有此卡的复制品」) listens here rather than at each
            // grant site, so a layer granted by *any* path still fires.
            for player_id in after.exile_log {
                self.raise_core(cx, "exile", player_id, |t| {
                    t.value = 1;
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

    /// The match-start points (`deckBeforeGame` / `deckAtGameStart`): dispatch
    /// to **every effect source** of `player_id` -- field cards including
    /// skills (per player, in field order) and the card ids in that player's
    /// piles/hands -- each source running its *own* hook with `t.card` naming
    /// it. Up to 5 passes, so sources a hook adds (a card that places itself,
    /// a pile a hook swells) get their turn too.
    ///
    /// `DeckBeforeGame` reaches the draw pile (before the opening deal);
    /// `DeckAtGameStart` reaches the draw pile and the hand (after the mulligan).
    fn game_start_hooks(
        &self,
        cx: &mut Cx,
        player_id: i32,
        kind: TriggerKind,
        trigger: &mut Trigger,
    ) -> Flow<()> {
        let Ok(p) = usize::try_from(player_id) else {
            return Ok(());
        };
        let mut seen_uids: Vec<i32> = Vec::new();
        let mut seen_ids: Vec<String> = Vec::new();
        for _ in 0..5 {
            let mut fresh = false;
            let world = cx.world_copy();
            if p >= world.player_count() {
                return Ok(());
            }
            // Field cards including skills, in field order.
            for (uid, id) in world.field_instances(player_id) {
                if seen_uids.contains(&uid) {
                    continue;
                }
                seen_uids.push(uid);
                fresh = true;
                let Some(idx) = self.ruleset.card(&id) else {
                    continue;
                };
                // 「场上所有背面朝上的卡无法产生效果」.
                if !self.ruleset.cards()[idx as usize].hooks(kind)
                    || world.card_face_down(player_id, &id)
                {
                    continue;
                }
                trigger.card = id.clone();
                self.drive_hook(
                    cx,
                    Call::Hook {
                        card: idx,
                        kind,
                        player_id,
                    },
                    &id,
                    uid,
                    trigger,
                )?;
            }
            // Card ids in the piles (and, after the mulligan, the hand).
            let mut ids: Vec<String> = Vec::new();
            for id in &world.hidden[p].draw {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
            if kind == TriggerKind::DeckAtGameStart {
                for id in &world.hidden[p].hand {
                    if !ids.contains(id) {
                        ids.push(id.clone());
                    }
                }
            }
            for id in ids {
                if seen_ids.contains(&id) {
                    continue;
                }
                seen_ids.push(id.clone());
                fresh = true;
                let Some(idx) = self.ruleset.card(&id) else {
                    continue;
                };
                if !self.ruleset.cards()[idx as usize].hooks(kind) {
                    continue;
                }
                trigger.card = id.clone();
                self.drive_hook(
                    cx,
                    Call::Hook {
                        card: idx,
                        kind,
                        player_id,
                    },
                    &id,
                    -1,
                    trigger,
                )?;
            }
            if !fresh {
                break;
            }
        }
        Ok(())
    }

    /// C# `AbnormalGate`, for an effect a card run wants to apply to `player_id`
    /// (`by` = the card's player): an out player is never hit; field cards guard
    /// (`abnormalGuard`, block with `set_cancelled`); then, if another player
    /// caused it, the `abnormal` [反击] window (C# `CounteractAbnormal`). What gets
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
        // choose. Status (stay/stun/exile) still lands. A self-caused move
        // (`by == player_id`) is *chosen*, not 「受到的」 (glossary 51 「可选择
        // 受到的[传送]，[强制移动]，[强制停下]效果是否生效」), so [不可阻挡] does
        // not refuse it -- a card's own `card_move` (里美's 「期间[不可阻挡]」)
        // must still walk.
        let movement = matches!(
            kind,
            crate::AbKind::Teleport
                | crate::AbKind::Forced
                | crate::AbKind::Stop
                | crate::AbKind::Reverse
        );
        if movement && by != player_id && cx.state().players[s].unstoppable() > 0 {
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
        // Self-applied abnormals (「因任何原因」/「包括你的技能」) open the
        // window too; cards filter by their guards (`effect::has(Abnormal)`).
        {
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
        // the outcome, and cannot be [反击]'d. It fires for **every** landing,
        // self-applied included (「场上每有玩家获得」 counts each layer, and
        // 「每次受到…影响」 counts one's own too).
        self.raise_core(cx, "abnormal", by, on)?;
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
    /// (C# `c.Tags`) and the `PendingCounteract` / `TargetsChosen` pass.
    fn target_player(&self, cx: &mut Cx, p: i32, by: i32, card: &str, single: bool) -> Flow<i32> {
        let Ok(s) = usize::try_from(p) else {
            return Ok(-1);
        };
        if s >= cx.state().players.len() || cx.world_copy().out(s) {
            return Ok(-1);
        }
        // Per-pair cancel (「取消其对目标之一的[指定]」, C# `play.Tags["immune"+
        // seat]`): this designation was cancelled; the rest still land.
        if cx.world_copy().turn.cancelled_designations.contains(&p) {
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
        self.counteract(cx, &mut t)?;
        Ok(t)
    }

    /// Apply a field effect's `Dest` to the instance its run was for -- C#
    /// `H.Unplace(this, "discard" / "hand" / "gone")`, the fate `set_dest` names.
    /// `to` is whose pile it lands in; `None` is the owner it leaves, the target
    /// `set_transfer_to_dest` names.
    fn apply_dest(
        &self,
        cx: &mut Cx,
        uid: i32,
        card: &str,
        dest: i32,
        to: Option<i32>,
    ) -> Flow<()> {
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

    /// The [反击] hand window (C# `MatchHost.Counteract(Trigger)`): one **round
    /// per timing**, per rulebook clauses 32 and 89.
    ///
    /// **32:** 「[反击]：带有"[反击]X：Y"效果在X发生时打出并触发效果Y，且结算优先于X」.
    /// **89:** 「如果有多名玩家可在同一时间发动[反击]效果则从行动顺序上在触发[反击]时点
    /// 的玩家的**下一位**玩家开始依次决定是否使用[反击]效果。多个效果可[反击]同一个时点，
    /// 所有玩家同意所有对一个时点的[反击]已发动后可对新的时点发动[反击]。」
    ///
    /// # Rulings (what the book leaves open)
    ///
    /// * **Resolution order:** counters to one timing resolve **newest first
    ///   (LIFO)**, and every counter still settles before the timing (effect) it
    ///   answers.
    /// * **Ask ring and per-visit floor** (user ruling 2026-10-07; supersedes
    ///   the 2026-10-06 ileuxali/bangdream-monopoly-derived "one activation per
    ///   visit, ring starts after the trigger's player"): the ring starts with
    ///   the **initial user** -- the player whose action or effect raised the
    ///   link, `chain_starter`'s seat -- and each seat **exhausts its
    ///   counteractions before priority moves on**. On a visit the seat keeps
    ///   being offered its eligible cards until it explicitly passes
    ///   (voluntarily abandons) or holds none left; every declaration adds its
    ///   link. The pass is one explicit "not playing" option on the offer
    ///   (`ask.counteract.skip`) and ends the visit.
    /// * **Round closing:** laps of the ring continue until a **full lap
    ///   brings no new declaration** (「所有玩家同意…已发动后」). A lap that
    ///   carried a declaration never closes the round, even if every seat
    ///   passed after it -- the next full lap must be quiet. A later seat's
    ///   declaration can re-open earlier seats on the next lap.
    ///
    /// # Round on timing X
    ///
    /// `t` is **L1**, the effect declaration -- the card effect already aimed at
    /// its named recipients. One round runs on it. The ring starts at the
    /// initial user (`chain_starter`: the player whose card caused the link; a
    /// board-driven link -- rent, buy, build, turn flow -- is a system / tile
    /// event and starts at the active turn player) and runs forward in turn
    /// order from there. Out / AI / exiled / stunned / no-hand players are
    /// skipped (`can_counteract_now`), a seat with no eligible card is skipped
    /// without a prompt, and `can_counteract` is re-evaluated before every
    /// offer. Each visit, the responder declares every eligible hand card
    /// answering **X** it wants -- so several players may counter the same
    /// effect, each answering X rather than each other -- or passes to end the
    /// visit; only then does priority advance to the next responder.
    ///
    /// # Counters to counters (「…后可对新的时点发动[反击]」)
    ///
    /// Once the round on X closes, the counters it collected are new timings.
    /// **Design choice (ours):** they are taken **newest first**, each with its
    /// own round (same rules; the "initial user" of that round is *that*
    /// counter's declarer), recursively; answers become timings in turn. Newest
    /// first keeps answering consistent with LIFO resolution -- the newest
    /// counter settles first, so it is the one answered first. Termination is
    /// natural (a declaration removes a card from a hand);
    /// `MAX_COUNTERACT_DEPTH` / `MAX_COUNTERACT_PER_VISIT` are the runaway
    /// guards.
    ///
    /// # Resolution
    ///
    /// LIFO over the resulting answer tree: a post-order walk that visits each
    /// node's answers **newest first**. The newest counter to a timing (and its
    /// whole subtree) settles first, then the next newest, ...; a counter's own
    /// answers settle before the counter; the timing itself settles last of its
    /// subtree. (This shape is ours -- the book only orders same-timing
    /// counters, and the walk is the composition of the two ruling clauses.) A
    /// counter's body runs against the link it answers, so `set_cancelled` /
    /// `negate_effect` / `spare` land on that link and its guard read what that
    /// link declared (`effect::` sees the list). A counter whose own link one of
    /// its answers negated does not run its body; the declaration still stands
    /// and the card is spent to its default fate (the discard). `Dest`
    /// handling, the `discarded` raise and `log.play_counteract` are unchanged.
    ///
    /// Known cases outside this (left as-is, flagged for a later pass):
    ///
    /// * **The acting card's own follow-up** (`CardRules::counteract` step (1)) runs
    ///   before this window, outside the chain.
    fn hand_counteractions(
        &self,
        cx: &mut Cx,
        t: &CoreTrigger,
        trigger: &mut Trigger,
        depth: u32,
    ) -> Flow<()> {
        if depth >= MAX_COUNTERACT_DEPTH || !cx.playing() {
            return Ok(());
        }
        let n = cx.state().players.len();
        if n == 0 {
            return Ok(());
        }

        // The answer tree. Index 0 is L1, the effect declaration; every other
        // node is one declared counter.
        let mut chain = vec![ChainLink {
            seat: chain_starter(t, cx.state().turn, n),
            idx: -1,
            id: t.card.clone(),
            answered: 0,
            link: trigger.clone(),
            answers: Vec::new(),
        }];

        // Hard bound: the tree is only as deep and as wide as the hands
        // involved, so this is a runaway guard, not a design limit. One round
        // is at most (declarations + a quiet lap) visits, each visit at most
        // `MAX_COUNTERACT_PER_VISIT` offers, over `MAX_COUNTERACT_DEPTH + 1`
        // rounds.
        let mut budget = (MAX_COUNTERACT_DEPTH + 1) * n as u32 * (MAX_COUNTERACT_PER_VISIT + 1) + 8;
        self.build_round(cx, &mut chain, 0, depth, &mut budget)?;
        self.resolve_rounds(cx, &mut chain, 0)?;

        // L1's fate is whatever the counters did to it.
        if let Some(root) = chain.into_iter().next() {
            *trigger = root.link;
        }
        Ok(())
    }

    /// One round on `chain[timing]`, then -- newest first, the book's 「…后可对
    /// 新的时点发动[反击]」 -- one round per counter it collected, recursively.
    /// See [`Self::hand_counteractions`].
    fn build_round(
        &self,
        cx: &mut Cx,
        chain: &mut Vec<ChainLink>,
        timing: usize,
        depth: u32,
        budget: &mut u32,
    ) -> Flow<()> {
        if depth >= MAX_COUNTERACT_DEPTH {
            return Ok(());
        }
        let n = cx.state().players.len();
        // The ring starts at the **initial user** (ruling 2026-10-07): the
        // player the timing belongs to -- `chain_starter` at the root, the
        // declarer on a counter's own round.
        let mut priority = Priority::new(chain[timing].seat, n);
        // Counters this round collected, in declaration order.
        let mut round: Vec<usize> = Vec::new();
        'visits: while *budget > 0 {
            let cursor = priority.seat();
            // Ruling 2026-10-07: the visit keeps the floor until this seat
            // passes (the offer's `ask.counteract.skip`) or holds no eligible
            // counteraction left -- every declaration re-offers the same seat.
            // Each offer is its own prompt, so the AI answer (chaos re-rolls
            // its counter chance per offer) is recomputed per offer in
            // `Cx::ask` / `fill_ai`.
            let mut declared_any = false;
            if can_counteract_now(cx, cursor) {
                for _ in 0..MAX_COUNTERACT_PER_VISIT {
                    if *budget == 0 {
                        break 'visits;
                    }
                    *budget -= 1;
                    let Some((id, idx)) = self.declare_one(cx, cursor, &chain[timing].link)? else {
                        // Explicit pass, or nothing eligible left: the visit
                        // ends and priority advances.
                        break;
                    };
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
                    let at = chain.len();
                    link.seq = (at + 1) as u32;
                    link.answers = timing as u32;
                    round.push(at);
                    chain[timing].answers.push(at);
                    chain.push(ChainLink {
                        seat: cursor,
                        idx,
                        id,
                        answered: timing,
                        link: bridge_trigger(&link),
                        answers: Vec::new(),
                    });
                    declared_any = true;
                }
            }
            if !priority.answered(declared_any) {
                break;
            }
        }

        // The counters just declared are themselves timings. Answer the newest
        // first (LIFO), each with its own round starting at its declarer (the
        // new round's initial user).
        for &child in round.iter().rev() {
            self.build_round(cx, chain, child, depth + 1, budget)?;
        }
        Ok(())
    }

    /// Resolve the answer tree LIFO: post-order, answers newest-first -- a
    /// counter's own answers settle before it, and sibling counters settle
    /// newest first. Each counter's body runs against the link it answers; one
    /// whose own link an answer negated does not run.
    fn resolve_rounds(&self, cx: &mut Cx, chain: &mut Vec<ChainLink>, idx: usize) -> Flow<()> {
        let answers = chain[idx].answers.clone();
        for &child in answers.iter().rev() {
            self.resolve_rounds(cx, chain, child)?;
        }
        if idx == 0 {
            // L1 settles in the engine; its link is already whatever the
            // counters made of it.
            return Ok(());
        }
        let seat = chain[idx].seat;
        let card = chain[idx].idx;
        let id = chain[idx].id.clone();
        let answered = chain[idx].answered;
        // What the counter's own answers did to its link: a negation there
        // voids the counter's settlement (`set_cancelled` -- the activation
        // never happened -- or `negate_effect`, which settles to nothing).
        let negated = chain[idx].link.is_cancelled();
        cx.log(
            seat as i32,
            Msg::new("log.play_counteract")
                .player_id("who", seat as i32)
                .card("card", id.clone()),
        );
        let dest = if negated {
            // The body does not run, so the card has no fate of its own: it was
            // played (it left the hand at declaration) and is spent.
            DEST_UNSET
        } else {
            // The counter's body runs against the link it answers, so its
            // `set_cancelled` / `negate_effect` / `spare` land there -- and the
            // effect settles only after every counter has had its say.
            let mut on_link = chain[answered].link.clone();
            let dest = self.drive(
                cx,
                Call::Counteract {
                    card,
                    player_id: seat as i32,
                },
                &id,
                -1,
                &mut on_link,
            )?;
            chain[answered].link = on_link;
            dest
        };
        let mut w = cx.world_copy();
        let spent = matches!(dest_from(dest), Dest::Graveyard) && !w.out(seat);
        match dest_from(dest) {
            Dest::Graveyard => {
                if !w.out(seat) {
                    w.hidden[seat].discard.push(id.clone());
                }
            }
            Dest::Hand => w.hidden[seat].hand.push(id.clone()),
            Dest::Banished => {}
            Dest::Field => {}
        }
        cx.swap_world(w);
        if spent {
            self.raise_core(cx, "discarded", seat as i32, |t| t.card = id.clone())?;
        }
        Ok(())
    }

    /// Offer one player a [反击] window answering `top`. Returns the card they
    /// declared, if any. The prompt describes the **answered** link, so a
    /// player answering a counter is told which counter they are answering.
    fn declare_one(
        &self,
        cx: &mut Cx,
        s: usize,
        top: &Trigger,
    ) -> Flow<Option<(String, i32)>> {
        // Hand cards that answer this link (C# `_hidden[s].hand.Distinct()`).
        // Ordered by `effect_order_key` (Q5): group `Hand`, source = the card's
        // index in the hand `Vec` (the authoritative state list), decl = 0 (one
        // entry per card after the `Distinct` dedupe). The sort is stable and
        // the source component *is* today's hand order, so the offer list is
        // unchanged -- the key just makes the guarantee explicit.
        let mut options: Vec<(String, i32)> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let mut order: Vec<(u8, u32, u32)> = Vec::new();
        // docs/GUARDS.md §4.2/§4.4 (1) + BOT-RESEARCH.md #1: build the window
        // context **once** per trigger window and reuse it across every
        // candidate probe in this offer. The per-card kind bitmask
        // (`Ruleset::counteracts_to`) and the condition pre-filter
        // (`counteract_pre_allows`) run **before** any `Run` / `world_copy`, so
        // a card that cannot answer this kind -- or whose condition rejects --
        // never instantiates the guard.
        let win_run = Run {
            world: cx.world_copy(),
            data: self.data.clone(),
            props: self.modules_props(),
            trigger: top.clone(),
            current_card: String::new(),
            current_uid: -1,
            dest: DEST_UNSET,
            dest_to: None,
            paid_log: vec![],
            discard_log: vec![],
            draw_log: vec![],
            reshuffle_log: vec![],
            fire_spent_log: vec![],
            house_log: vec![],
            crystals_log: vec![],
            cp_log: vec![],
            exile_log: vec![],
            doubled: -1,
            linger_props: Default::default(),
        };
        let win_scope = crate::cond_pre::window_scope(&crate::cond_pre::fill_window(&win_run));
        for (hand_pos, id) in hand_of(cx, s).into_iter().enumerate() {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id.clone());
            let Some(idx) = self.ruleset.card(&id) else {
                continue;
            };
            // Cached counteraction index (BOT-RESEARCH.md #1): one shift of the
            // per-card kind bitmask -- no entry-table scan, no `Run`.
            if !self.ruleset.counteracts_to(idx, top.kind) {
                continue;
            }
            // Condition pre-filter against the shared window scope. Rejects
            // most probes before any sandbox / native run.
            if self
                .ruleset
                .counteract_pre_allows(&win_run, idx, s as i32, &win_scope)
                .is_none()
            {
                continue;
            }
            let run = Run {
                world: cx.world_copy(),
                data: self.data.clone(),
                props: self.modules_props(),
                trigger: top.clone(),
                current_card: id.clone(),
                current_uid: -1,
                dest: DEST_UNSET,
                dest_to: None,
                paid_log: vec![],
                discard_log: vec![],
                draw_log: vec![],
                reshuffle_log: vec![],
                fire_spent_log: vec![],
                house_log: vec![],
                crystals_log: vec![],
                cp_log: vec![],
            exile_log: vec![],
                doubled: -1,
                linger_props: Default::default(),
            };
            if self
                .ruleset
                .can_counteract_scoped(&run, idx, s as i32, &win_scope)
                .unwrap_or(false)
            {
                order.push(effect_order_key(
                    LookupGroup::Hand,
                    hand_pos as u32,
                    0,
                ));
                options.push((id, idx));
            }
        }
        if options.is_empty() {
            return Ok(None);
        }
        // Q5: order the offer list by the explicit key. Stable, and the key's
        // source component is the hand position, so this is today's order.
        let mut keyed: Vec<((u8, u32, u32), (String, i32))> =
            order.into_iter().zip(options).collect();
        keyed.sort_by_key(|(k, _)| *k);
        let options: Vec<(String, i32)> = keyed.into_iter().map(|(_, o)| o).collect();
        // C#: labels "打出「...」" + "不打"; the hint is the first CounteractHint or
        // the trigger's description. CounteractHint is not in the ABI yet (TODO).
        let mut labels: Vec<Msg> = options
            .iter()
            .map(|(id, _)| Msg::new("ask.counteract.play").card("card", id.clone()))
            .collect();
        labels.push(Msg::new("ask.counteract.skip"));
        let fallback = labels.len() as i32 - 1;
        let ask = Ask::choice(
            vec![s],
            Msg::new("ask.counteract.title"),
            Msg::new("ask.counteract.text").msg("detail", describe_trigger(top)),
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
/// is C# `t.Move.Roll`, which a counteraction may rewrite; the engine reads it back.
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
        // Only a move trigger carries a roll; `paid`/`settle` carry value as an
        // amount, which must not read as a phantom move. `RollAfter` is the
        // post-roll hook and does carry the face (「若移动掷骰出目为16及以上」).
        move_roll: matches!(
            trigger_kind(t.kind),
            TriggerKind::MoveRoll | TriggerKind::Roll | TriggerKind::RollAfter
        )
        .then_some(t.value),
        card: t.card.clone(),
        roll_source: t.roll_source,
        buy_kind: t.buy_kind,
        seller: t.seller,
        price: t.price,
        deal_owner: t.deal_owner,
        deal_houses: t.deal_houses,
        deal_mortgaged: t.deal_mortgaged,
        reason: t.reason.clone(),
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

/// The initial user of a [反击] round at the chain root (ruling 2026-10-07):
/// the player whose action or effect raised the link. That is `by_card` when a
/// card caused the trigger -- **not** the trigger's own player, which for a
/// payment is the payer: seat 2's card making seat 0 pay starts at seat 2, not
/// seat 0. A board-driven trigger (rent, buy, build, turn flow -- `by_card` is
/// `None`, docs/CARDS.md) is a system / tile event, so the link has no player
/// and the ring starts at the active turn player. The ring starts **at** this
/// seat (superseding clause 89's 「下一位」).
fn chain_starter(t: &CoreTrigger, turn: i32, n: usize) -> usize {
    let seat = match t.by_card {
        Some(by) if by >= 0 => by,
        _ => turn.max(0),
    };
    seat as usize % n
}

/// One node of the [反击] answer tree (see `hand_counteractions`). Index 0 is
/// the timing the chain answers (L1); every other node is one declared counter.
struct ChainLink {
    /// Who declared it. For L1, the initial user of the round
    /// (`chain_starter`).
    seat: usize,
    /// The declared card's ruleset handle (-1 for L1).
    idx: i32,
    /// The declared card's id (L1: the trigger's own card).
    id: String,
    /// Node index of the timing this answers.
    answered: usize,
    /// This node's own link -- what its answers read and rewrite.
    link: Trigger,
    /// Nodes that answered this one, in declaration order.
    answers: Vec<usize>,
}

/// Who is asked next during a round on one timing (see `hand_counteractions`).
///
/// Ruling 2026-10-07 (supersedes clause 89's 「下一位」 start and the
/// 2026-10-06 one-activation-per-visit cap): the ring starts **at** the initial
/// user and runs forward in turn order; a visit holds the floor until the seat
/// passes or runs out of eligible counteractions (`build_round`). The round
/// closes at the end of a **lap** -- one full cycle of all seats starting from
/// the initial user -- that brings no new declaration (「所有玩家同意…已发动
/// 后」). A lap that carried a declaration never closes the round, even if
/// every seat passed after it; the next full lap must be quiet.
struct Priority {
    n: usize,
    cursor: usize,
    /// Visits completed in the current lap (0..n). A lap is one full cycle of
    /// the ring from the initial user.
    in_lap: usize,
    /// Did any visit in the current lap declare a counter?
    lap_carried: bool,
}

impl Priority {
    /// A round on `initial`'s timing: starts at the initial user's seat.
    fn new(initial: usize, n: usize) -> Self {
        Self { n, cursor: initial % n, in_lap: 0, lap_carried: false }
    }

    /// The seat to ask now.
    fn seat(&self) -> usize {
        self.cursor
    }

    /// The current seat's visit ended -- `declared` if it added at least one
    /// counter. Returns whether the round is still open.
    fn answered(&mut self, declared: bool) -> bool {
        self.cursor = (self.cursor + 1) % self.n;
        if declared {
            self.lap_carried = true;
        }
        self.in_lap += 1;
        if self.in_lap < self.n {
            return true;
        }
        // Lap boundary: a lap that brought no declaration closes the round.
        self.in_lap = 0;
        std::mem::take(&mut self.lap_carried)
    }
}

/// C# `MatchHost.CanCounteractNow(s, t)`: out / exiled players cannot declare a
/// counteraction. (The C# also checks `CannotPlay` and a one-turn mute; neither has
/// an engine field yet.)
fn can_counteract_now(cx: &Cx, s: usize) -> bool {
    let Some(player_id) = cx.state().players.get(s) else {
        return false;
    };
    // C# `CanCounteractNow`: out / AI / exiled players never open a window, and
    // `CannotPlay` (stun, 飞鸟山之战's no-hand, Fx.CantPlayHand) blocks it too.
    // `_noCounteractTurn` and Fx.CantPlayHand have no engine field yet (TODO).
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

/// The one-line description of the answered link the [反击] prompt shows (C#
/// `DescribeTrigger`).
fn describe_trigger(t: &Trigger) -> Msg {
    let mut m = Msg::new("ask.counteract.detail").player_id("who", t.player_id);
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
/// Folding unknown kinds to `None` would make `can_counteract` guards silently
/// never match.
fn trigger_kind(kind: &str) -> TriggerKind {
    TriggerKind::from_str(kind)
}

/// The buy price stages, in the order 支付阶段 2 / 4 / 5 run them
/// (`docs/PURCHASE.md`): `BuyAdd` (fixed ±) → `BuyMul` (×) → `BuySet` (free /
/// fixed), each floored at 0.
const BUY_STAGES: [TriggerKind; 3] =
    [TriggerKind::BuyAdd, TriggerKind::BuyMul, TriggerKind::BuySet];

/// The bridge trigger a buy hook sees (`docs/PURCHASE.md`): the kind, who is
/// buying what from whom, and the running price the stage rewrites with
/// `set_price`. `value` mirrors `price` so a generic reader sees the figure.
fn buy_trigger(
    kind: TriggerKind,
    q: &game_core::engine::purchase::BuyQuery,
    t: usize,
    price: i32,
    st: &game_core::state::MatchState,
) -> Trigger {
    Trigger {
        kind,
        player_id: q.player as i32,
        tile: t as i32,
        value: price,
        step: st.step,
        buy_kind: q.kind.as_i32(),
        seller: q.seller,
        price,
        deal_owner: q.player as i32,
        deal_houses: st.houses.get(t).copied().unwrap_or(0),
        deal_mortgaged: st.mortgaged.get(t).copied().unwrap_or(false),
        ..Trigger::default()
    }
}

impl<M: CardModules> RulesBridge<M> {
    /// The live instances that could carry a buy hook: each player's field
    /// instances in placement order, then the board field (the tile / event /
    /// mark rules), then the turn's **lingering** instances -- the hand-card
    /// home for 「本回合」 buy effects (`docs/PURCHASE.md` P5). The order is the
    /// `counteract` dispatch's, so a quote and the commit see the same chain.
    ///
    /// Only instances whose rule actually declares a buy hook come back, so
    /// the cheap path can ask this once and skip the whole machinery when it is
    /// empty (the `cant_play` shape: nothing is instantiated for a question
    /// nobody answers).
    fn buy_hook_instances(&self, world: &game_core::engine::World) -> Vec<(i32, i32, String)> {
        let declares = |id: &str| {
            self.ruleset
                .card(id)
                .is_some_and(|i| self.ruleset.cards()[i as usize].hooks(TriggerKind::BuyGate)
                    || BUY_STAGES.iter().any(|&k| self.ruleset.cards()[i as usize].hooks(k)))
        };
        let mut keyed: Vec<((u8, u32, u32), (i32, i32, String))> = Vec::new();
        let mut push = |key: (u8, u32, u32), item: (i32, i32, String), out: &mut Vec<_>| {
            if declares(&item.2) {
                out.push((key, item));
            }
        };
        for p in 0..world.player_count() {
            // B2 (`PIPELINE-AUDIT` K6/K14) -- 规则书 L81 「所有其正在生效的卡，技能
            // 效果停止生效」: a bankrupt / left seat's instances never answer.
            // (`remove_from_game` also clears the field; this is the dispatch's
            // own guard for anything a later path re-places.)
            if world.out(p) {
                continue;
            }
            for (n, (uid, id)) in world.field_instances(p as i32).into_iter().enumerate() {
                let group = source_group(world, uid);
                // Q5: (group, source = player-major placement, decl = 0).
                let key = effect_order_key(group, (p as u32) << 16 | n as u32, 0);
                push(key, (uid, p as i32, id), &mut keyed);
            }
        }
        for (n, (uid, id)) in world
            .field_instances(game_core::state::BOARD_OWNER)
            .into_iter()
            .enumerate()
        {
            let key = effect_order_key(LookupGroup::Board, n as u32, 0);
            push(key, (uid, game_core::state::BOARD_OWNER, id), &mut keyed);
        }
        for (n, l) in world.turn.lingering.iter().enumerate() {
            let key = effect_order_key(LookupGroup::Field, (l.owner as u32) << 16 | n as u32, 0);
            push(key, (-1, l.owner, l.card.clone()), &mut keyed);
        }
        keyed.sort_by_key(|(k, _)| *k);
        keyed.into_iter().map(|(_, item)| item).collect()
    }

    /// One hook, run in **pure guard mode** against `world` -- the `cant_play`
    /// shape: a throwaway copy, so the run cannot change the match and never
    /// prompts. What survives is only the trigger the hook rewrote (the price
    /// stages' `set_price`, a `BuyGate`'s `set_cancelled`); the world the run
    /// produced is dropped.
    ///
    /// A body that would prompt or ask the host contributes nothing here: a
    /// quote is a preview, and the commit re-quotes, so such a hook is answered
    /// (if at all) on the real run and not on a price the UI is merely showing.
    fn pure_buy_hook(
        &self,
        run: &mut Run,
        uid: i32,
        owner: i32,
        card: &str,
        kind: TriggerKind,
        trig: &mut Trigger,
    ) {
        let Some(idx) = self.ruleset.card(card) else {
            return;
        };
        if !self.ruleset.cards()[idx as usize].hooks(kind) {
            return;
        }
        // The run is a throwaway: only the trigger it rewrote is read back, and
        // the world `run_hook` produces is dropped. `run_hook` clones the run
        // into the store it fires up, so the caller shares one `Run` across the
        // whole quote rather than cloning per hook.
        run.trigger = trig.clone();
        run.current_card = card.to_string();
        run.current_uid = uid;
        if let Ok(Some(hr)) =
            self.ruleset
                .run_hook(&*run, Call::Hook { card: idx, kind, player_id: owner }, &[])
        {
            if let Outcome::Done(after) = hr.outcome {
                *trig = after.trigger;
            }
        }
    }
}

impl<M: CardModules> CardRules for RulesBridge<M> {
    fn ruleset_sha256(&self) -> Option<&str> {
        self.ruleset.sha256()
    }

    /// The card rule's declared static properties (`CardDef::props`), see
    /// [`game_core::engine::CardRules::card_props`].
    fn card_props(&self, card: &str) -> std::collections::BTreeMap<String, i32> {
        self.ruleset.card_props(card)
    }

    fn has_rule(&self, id: &str) -> bool {
        self.ruleset.card(id).is_some()
    }

    /// The settle body (`docs/TILES.md`): run the tile's **rule instances**
    /// (board-owned, placed by `bind_tiles`) in instance order. Falls back to
    /// the engine's built-in `land_at` body for a tile with none -- `StubRules`
    /// binds nothing, and a kind with no rule in this ruleset has no instance.
    fn settle_tile(
        &self,
        cx: &mut Cx,
        player_id: usize,
        tile: usize,
        main: bool,
    ) -> Flow<()> {
        let instances = cx.world_copy().tile_rule_instances(tile as i32);
        if instances.is_empty() {
            return cx.land_at_built_in(player_id, tile, main);
        }
        if !cx.prep_land(player_id, tile, main) {
            return Ok(());
        }
        let owner = cx.state().owners.get(tile).copied().unwrap_or(-1);
        // `On::Settle` dispatches by entry kind (`Call::Settle`), not by a
        // trigger kind, so the link's kind is `None` -- it is the body of the
        // settle the engine already raised (`settle` / `settleBefore` /
        // `settleAfter` frame it), not a named point of its own. `docs/TILES.md`.
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: owner,
            tile: tile as i32,
            value: 0,
            step: cx.state().step,
            by_card: None,
            pay_is_rent: false,
            move_kind: None,
            move_resolve: true,
            move_tags: Vec::new(),
            move_main: main,
            move_dir: 1,
            negation: Default::default(),
            spared: Vec::new(),
            seq: 0,
            answers: 0,
            effects: Vec::new(),
            move_remaining: 0,
            move_total: 0,
            cards: Vec::new(),
            roll_source: 0,
            move_roll: None,
            card: String::new(),
            buy_kind: 0,
            seller: -1,
            price: 0,
            deal_owner: -1,
            deal_houses: 0,
            deal_mortgaged: false,
            reason: String::new(),
        };
        for (uid, id) in instances {
            let Some(idx) = self.ruleset.card(&id) else {
                continue;
            };
            self.drive(
                cx,
                Call::Settle {
                    card: idx,
                    player_id: player_id as i32,
                },
                &id,
                uid,
                &mut trigger,
            )?;
            if trigger.is_cancelled() {
                break;
            }
        }
        Ok(())
    }

    fn cant_play(&self, cx: &Cx, player_id: usize, card: &str) -> Option<Msg> {
        // A pure query on a throwaway copy (C# `Card.WhyNot`): a guard that
        // prompts, or a module that fails, never blocks play.
        let idx = self.ruleset.card(card)?;
        // Bind the instance when the card is already on the field (a skill
        // press), so `is_placed` / `crystals` answer for it and not for a void
        // `current_uid = -1`.
        let uid = cx
            .world_copy()
            .field_instances(player_id as i32)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
        let run = Run {
            world: cx.world_copy(),
            data: self.data.clone(),
            props: self.modules_props(),
            trigger: Trigger::default(),
            current_card: card.to_string(),
            current_uid: uid,
            dest: DEST_UNSET,
            dest_to: None,
            paid_log: vec![],
            discard_log: vec![],
                draw_log: vec![],
            reshuffle_log: vec![],
            fire_spent_log: vec![],
            house_log: vec![],
            crystals_log: vec![],
            cp_log: vec![],
            exile_log: vec![],
            doubled: -1,
                linger_props: Default::default(),
        };
        self.ruleset
            .cant_play(&run, idx, player_id as i32)
            .ok()
            .flatten()
    }

    /// The purchase quote (`docs/PURCHASE.md`): what would `q.player` be
    /// charged for each tile, and may they buy it at all?
    ///
    /// Runs the `BuyGate` / `BuyAdd` → `BuyMul` → `BuySet` hooks in pure guard
    /// mode against a world copy (like [`Self::cant_play`]), **only** when a
    /// hooking instance exists -- otherwise the plain rulebook formula answers
    /// and nothing is instantiated, which is what `StubRules` and the sim run.
    /// The commit re-quotes ([`game_core::engine`]'s `buy`), so the quoted price
    /// is the price charged.
    fn buy_quote(
        &self,
        w: &game_core::engine::World,
        data: &GameData,
        q: &game_core::engine::purchase::BuyQuery,
    ) -> Vec<game_core::engine::purchase::Quote> {
        use game_core::engine::purchase::{base_quote, Quote};
        let st = &w.st;
        let native = |t: usize| {
            let price = base_quote(data, st, t, q.kind);
            Quote {
                price,
                eligible: price >= 0,
            }
        };
        // Cheap path: no live instance declares a buy hook, so the quote is the
        // native formula and no module is fired up.
        let instances = self.buy_hook_instances(w);
        if instances.is_empty() {
            return q.tiles.iter().map(|&t| native(t)).collect();
        }
        // Only the stages somebody actually declares are run -- a `BuySet`
        // nobody hooks is a no-op that would cost a fire-up per tile.
        let declares = |kind: TriggerKind| {
            instances.iter().any(|(_, _, card)| {
                self.ruleset
                    .card(card)
                    .is_some_and(|i| self.ruleset.cards()[i as usize].hooks(kind))
            })
        };
        let gate_decl = declares(TriggerKind::BuyGate);
        // One throwaway run for the whole quote: the world it carries is read
        // by the hooks and the world they produce is dropped. `run_hook` clones
        // per hook run, so this is the quote's single world clone.
        let mut run = Run {
            world: w.clone(),
            data: self.data.clone(),
            props: self.modules_props(),
            trigger: Trigger::default(),
            current_card: String::new(),
            current_uid: -1,
            dest: DEST_UNSET,
            dest_to: None,
            paid_log: vec![],
            discard_log: vec![],
            draw_log: vec![],
            reshuffle_log: vec![],
            fire_spent_log: vec![],
            house_log: vec![],
            crystals_log: vec![],
            cp_log: vec![],
            exile_log: vec![],
            doubled: -1,
            linger_props: Default::default(),
        };
        q.tiles
            .iter()
            .map(|&t| {
                let base = base_quote(data, st, t, q.kind);
                if base < 0 {
                    return Quote {
                        price: -1,
                        eligible: false,
                    };
                }
                let mut at = |run: &mut Run, kind: TriggerKind, price: i32| {
                    let mut trig = buy_trigger(kind, q, t, price, st);
                    for &(uid, owner, ref card) in &instances {
                        self.pure_buy_hook(run, uid, owner, card, kind, &mut trig);
                    }
                    trig
                };
                // `BuyGate` -- may this player buy this tile at all? Runs for
                // every [`card_sdk::abi::BuyKind`], Force included (Poppin's
                // hill lock), so a quote is never a price for an impossible buy.
                if gate_decl && at(&mut run, TriggerKind::BuyGate, base).is_cancelled() {
                    return Quote {
                        price: -1,
                        eligible: false,
                    };
                }
                // `BuyAdd` → `BuyMul` → `BuySet`, each floored at 0.
                let mut price = base;
                for &kind in &BUY_STAGES {
                    if !declares(kind) {
                        continue;
                    }
                    price = at(&mut run, kind, price).price.max(0);
                }
                Quote {
                    price,
                    eligible: true,
                }
            })
            .collect()
    }
    fn play(&self, cx: &mut Cx, player_id: usize, card: &str) -> Flow<Dest> {
        let Some(idx) = self.ruleset.card(card) else {
            cx.log(
                player_id as i32,
                Msg::new("log.card_not_ported").card("card", card),
            );
            return Ok(Dest::Graveyard);
        };
        // A skill press (`use_skill`) runs a *placed* card's play entry: bind
        // the instance so `is_placed` / `crystals` read it and not a void
        // `current_uid = -1`. A hand play has no instance yet (uid -1).
        let uid = cx
            .world_copy()
            .field_instances(player_id as i32)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
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
            roll_source: 0,
            move_roll: None,
            card: String::new(),
            buy_kind: 0,
            seller: -1,
            price: 0,
            deal_owner: -1,
            deal_houses: 0,
            deal_mortgaged: false,
            reason: String::new(),
        };
        let dest = self.drive(
            cx,
            Call::Play {
                card: idx,
                player_id: player_id as i32,
            },
            card,
            uid,
            &mut trigger,
        )?;
        Ok(dest_from(dest))
    }

    fn event(&self, cx: &mut Cx, player_id: usize, id: &str) -> Flow<bool> {
        // The rule id is `event:<id>` (`docs/EVENTS.md`); the engine binds its
        // instance on the board owner before this runs, so the body's
        // `On::Play` sees a live instance (`is_placed` / `crystals` / props).
        let rid = game_core::data::event_rule_id(id);
        let Some(idx) = self.ruleset.card(&rid) else {
            cx.log(
                player_id as i32,
                Msg::new("log.event_not_ported").event("event", id),
            );
            return Ok(false);
        };
        let uid = cx
            .world_copy()
            .event_rule_instances()
            .into_iter()
            .find(|(_, c)| c == &rid)
            .map_or(-1, |(uid, _)| uid);
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
            roll_source: 0,
            move_roll: None,
            card: String::new(),
            buy_kind: 0,
            seller: -1,
            price: 0,
            deal_owner: -1,
            deal_houses: 0,
            deal_mortgaged: false,
            reason: String::new(),
        };
        let dest = self.drive(
            cx,
            Call::Play {
                card: idx,
                player_id: player_id as i32,
            },
            &rid,
            uid,
            &mut trigger,
        )?;
        Ok(dest == DEST_FIELD)
    }

    fn counteract(&self, cx: &mut Cx, t: &mut CoreTrigger) -> Flow<()> {
        // Nothing in the set declares an entry at this kind, so no hook, no
        // gate and no [反击] can fire: skip building the bridge trigger at all.
        // This is most raises even with cards in play, and every raise for a
        // kind nobody listens to. Nothing could have rewritten `t`, so the
        // write-back below is a no-op and is safe to skip with it.
        if !self.ruleset.declares(trigger_kind(t.kind)) {
            return Ok(());
        }
        // The module's view of the trigger. `move_roll` is C# `t.Move.Roll`,
        // which a counteraction may rewrite; the engine reads it back afterwards.
        let mut trigger = bridge_trigger(t);
        // Ordering at one trigger (the standard; see `hand_counteractions`):
        // (1) the acting card's own follow-up resolves FIRST, outside the
        //     answer tree -- a card answering its own play is not competing with
        //     the counteractions to it, so it never loses its place to them;
        // (2) then the hand-counteraction [反击] round on the timing, and
        //     counters to counters, settle LIFO -- newest first -- each before
        //     the link it answers;
        // (3) only then the field/tile/event/lingering hooks, and only when
        //     the trigger was not cancelled by (2).
        //
        // `NEGATION-AUDIT` V1: the [反击] round runs **before** hook dispatch
        // (规则书 L32 「[反击]…结算优先于X」 -- the counteraction settles before
        // X, including X's triggered [持续] settlement). A spend inside a hook
        // body must not settle before any seat can counteract the trigger that
        // carries it, and a cancelled trigger runs no hook body at all.
        //
        // (1) The played card's own follow-up: the card named on the trigger runs
        // its `counteract` (C# `PlayCtx.AsCounteraction` for a card answering its own play).
        // Only at the play itself -- `cardAfter` / `cardPlayed` / `eventAfter` /
        // `drawn` also name a card on `t.card`, and must not re-run it here.
        let own = t.card.clone();
        let own_play = matches!(trigger_kind(t.kind), TriggerKind::Card | TriggerKind::Event);
        if own_play && !own.is_empty() {
            if let Some(idx) = self
                .ruleset
                .card(&own)
                .filter(|&i| self.ruleset.cards()[i as usize].counteracts_to(trigger.kind))
            {
                self.drive(
                    cx,
                    Call::Counteract {
                        card: idx,
                        player_id: t.player_id,
                    },
                    &own,
                    -1,
                    &mut trigger,
                )?;
            }
        }
        // (2) The hand-counteraction window (C# `MatchHost.Counteract(Trigger)`): one
        // round per timing, from the seat after the timing's player around the
        // table; counters settle newest-first before the timing they answer
        // (see `hand_counteractions`). Not at hook-only points.
        if !is_hook_only(t.kind) {
            self.hand_counteractions(cx, t, &mut trigger, 0)?;
        }
        // (3) Field-card (`Fx`) hooks: at a hook-point kind, every *placed* card
        // runs its `counteract` automatically, in placement order per player. No player
        // declaration -- this is the persistent-effect path, as against the
        // [反击] window below.
        // Any trigger kind can carry a hook; the manifest says which cards
        // declared one, so nothing else is instantiated.
        //
        // V1: a trigger the [反击] round already cancelled runs **no** hook
        // body -- the spend inside one is effect content of the card that
        // wrote it and must not settle past a negation of the trigger.
        let kind = trigger.kind;
        if !trigger.is_cancelled() {
        {
            if is_game_start_kind(kind) {
                // Match-start points reach **every effect source** of the
                // player the engine raised them for: field cards including
                // skills (in field order) and the card ids in that player's
                // piles/hands (as before). Each source runs its *own* hook,
                // with `t.card` naming it -- so a card that has no game-start
                // clause is never instantiated for one.
                self.game_start_hooks(cx, t.player_id, kind, &mut trigger)?;
            } else if is_own_card_kind(kind) {
                // C# runs these on a fresh instance of the card named on
                // `t.card` (`Drawn`) -- it is in a hand / pile, not placed.
                // `Discarded` is *not* here: a placed field card hears about
                // discards (MyGO band (3) 「每次你的卡在未生效的情况下进入弃牌
                // 堆时」), so it rides the field-card path below.
                if let Some(idx) = self
                    .ruleset
                    .card(&t.card)
                    .filter(|&i| self.ruleset.cards()[i as usize].hooks(kind))
                {
                    self.drive_hook(
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
            } else {
                let world = cx.world_copy();
                for player_id in 0..world.player_count() {
                    // B2 (`PIPELINE-AUDIT` K6/K14) -- 规则书 L81 「所有其正在生效
                    // 的卡，技能效果停止生效」: a bankrupt / left seat's field
                    // hooks never run. (`remove_from_game` clears `s.field`;
                    // this is the dispatch's own guard.)
                    if world.out(player_id) {
                        continue;
                    }
                    for (uid, id) in world.field_instances(player_id as i32) {
                        // A hook cannot re-trigger on its own money movement
                        // (the termination argument for nested money).
                        if is_money_hook_kind(kind) && cx.reentrant_hooks.contains(&uid) {
                            continue;
                        }
                        let Some(idx) = self.ruleset.card(&id) else {
                            continue;
                        };
                        // 「场上所有背面朝上的卡无法产生效果」 -- a face-down
                        // card on the field is inert, so its hooks must not run.
                        if self.ruleset.cards()[idx as usize].hooks(kind)
                            && !world.card_face_down(player_id as i32, &id)
                        {
                            self.drive_hook(
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
                // Board-owned rule instances (`docs/TILES.md`, `docs/EVENTS.md`)
                // hear the hooks they declare, the same as a player's field
                // cards do. They run **after** the player fields so a suppressing
                // card can arm `prop::NO_REWARD` on the tile instance in its own
                // hook before `tile:circle`'s Pass entry reads it.
                //
                // A tile-carrying trigger (`passTile`, `settle`, a rent `pay`)
                // reaches only the instances governing *that* tile -- the cheap
                // path, since `passTile` fires on every step of every walk and
                // the board holds one instance per tile. **Event** instances
                // (`tile = -1`, `event:*`) are not tile-governed: they hear
                // every trigger their rule declares, which is how an active
                // event listens to a pass / settle / roll anywhere on the board.
                // A trigger with no tile reaches every board instance, but only
                // when some `tile:*` or `event:*` rule actually declares the
                // hook (otherwise the board list is never touched).
                let mut board: Vec<(i32, String)> = if t.tile >= 0 {
                    world.tile_rule_instances(t.tile)
                } else if self
                    .ruleset
                    .cards()
                    .iter()
                    .any(|c| c.id.starts_with("tile:") && c.hooks(kind))
                {
                    world.field_instances(game_core::state::BOARD_OWNER)
                } else {
                    Vec::new()
                };
                if self
                    .ruleset
                    .cards()
                    .iter()
                    .any(|c| c.id.starts_with("event:") && c.hooks(kind))
                {
                    for ent in world.event_rule_instances() {
                        if !board.iter().any(|(u, _)| *u == ent.0) {
                            board.push(ent);
                        }
                    }
                }
                // **Mark** owners (`mark:*`, `mark:cp` = the [CP点] tile-mark
                // owner) are board-wide the same way: they govern no single
                // tile, so they hear every trigger their rule declares
                // wherever it points -- `mark:cp`'s settle clause is 「在拥有
                // [CP]点的格子上[结算]时」, any tile.
                if self
                    .ruleset
                    .cards()
                    .iter()
                    .any(|c| c.id.starts_with("mark:") && c.hooks(kind))
                {
                    for ent in world.mark_rule_instances() {
                        if !board.iter().any(|(u, _)| *u == ent.0) {
                            board.push(ent);
                        }
                    }
                }
                for (uid, id) in board {
                    if is_money_hook_kind(kind) && cx.reentrant_hooks.contains(&uid) {
                        continue;
                    }
                    let Some(idx) = self.ruleset.card(&id) else {
                        continue;
                    };
                    if self.ruleset.cards()[idx as usize].hooks(kind)
                        && !world.card_face_down(game_core::state::BOARD_OWNER, &id)
                    {
                        self.drive_hook(
                            cx,
                            Call::Hook {
                                card: idx,
                                kind,
                                player_id: game_core::state::BOARD_OWNER,
                            },
                            &id,
                            uid,
                            &mut trigger,
                        )?;
                    }
                }
                // **Lingering** instances (`TurnCtx.lingering`, `docs/PURCHASE.md`
                // P5) run last: the hand-card home for 「本回合」 effects, which
                // would otherwise have no instance to carry them. A lingering
                // `BuyAdd` is how @Tsugu ycm's 「本回合购买格子时[消耗]资金降低1500」
                // reaches a buy. `uid = -1`: there is no field instance, so
                // `is_placed` answers false and the money re-entrancy guard does
                // not apply to it.
                for l in world.turn.lingering.clone() {
                    let Some(idx) = self.ruleset.card(&l.card) else {
                        continue;
                    };
                    if !self.ruleset.cards()[idx as usize].hooks(kind) {
                        continue;
                    }
                    self.drive_hook(
                        cx,
                        Call::Hook {
                            card: idx,
                            kind,
                            player_id: l.owner,
                        },
                        &l.card,
                        -1,
                        &mut trigger,
                    )?;
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
        } // end if !trigger.is_cancelled() -- V1 hook skip
        // Write back whatever a counteraction rewrote. `set_move_roll` lands in
        // `trigger.move_roll` (C# shares `t.Move` with the counteractions); the pay
        // amount is rewritten in `trigger.value`. The engine reads the result
        // back off `t.value` after `counteract` returns.
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

/// Money-pipeline hook kinds: a hook of one of these kinds cannot re-trigger
/// on its own money movement (the termination argument for nested money).
fn is_money_hook_kind(kind: TriggerKind) -> bool {
    matches!(
        kind,
        TriggerKind::PayAdd
            | TriggerKind::PayMul
            | TriggerKind::PayChoose
            | TriggerKind::PayAt
            | TriggerKind::PayAfter
            | TriggerKind::PayTotalAdd
            | TriggerKind::PayTotalMul
            | TriggerKind::PayTotalCancel
    )
}

/// `PIPELINE-AUDIT` Q5 -- the **lookup group** half of the deterministic
/// ordering key (EFFECT_ACTIVATION `E6`): sources answering the same window are
/// scanned in this order. `Board` is our addition for the neutral `tile:*` /
/// `event:*` / `mark:*` rules, which are not player sources; it sorts after the
/// player groups, matching the dispatch's "player fields, then the board" walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(crate) enum LookupGroup {
    Status = 0,
    Band = 1,
    Character = 2,
    Field = 3,
    Hand = 4,
    Discard = 5,
    Deck = 6,
    Removed = 7,
    Board = 8,
}

/// The deterministic order key for two effect sources answering the same
/// window (`PIPELINE-AUDIT` Q5 / EFFECT_ACTIVATION `E6`/`E7`):
/// **(lookup group, source order, declaration order)**.
///
/// * **lookup group** -- [`LookupGroup`], the spec's scan order
///   status → band → character → field → hand → discard → deck → removed.
/// * **source order** -- the candidate's index in that group's authoritative
///   state list: player-major then placement order for the field-ish groups
///   (`players[p].field`), the hand `Vec` index for `hand`, `board_field` order
///   for the board group.
/// * **declaration order** -- the entry index within the source's manifest
///   `on` list.
///
/// Pure: no `HashMap` iteration, no clock. Two runs of the same authoritative
/// state produce the same order. Field hooks before the hand counteraction
/// ring is already group `Field`/`Band`/`Character` < `Hand`, so wiring the
/// key does not move anything today (`docs/ENGINE.md` 「Effect ordering」).
pub(crate) fn effect_order_key(group: LookupGroup, source: u32, decl: u32) -> (u8, u32, u32) {
    (group as u8, source, decl)
}

/// The lookup group of one placed source (`LookupGroup::Band` for a band-skill
/// instance, `Character` for a character skill, `Field` for anything else).
/// Reads the instance's `band_skill` stamp (`GameData::is_band_skill`), which
/// is the authoritative classification.
fn source_group(world: &game_core::engine::World, uid: i32) -> LookupGroup {
    match world.field_by_uid(uid) {
        Some(f) if f.band_skill => LookupGroup::Band,
        Some(f) if f.card.starts_with("skill:") => LookupGroup::Character,
        _ => LookupGroup::Field,
    }
}

/// Field-card (`Fx`) hook points (ABI v17). These are **not** [反击] points:
/// the engine runs every *placed* card's `counteract` against them automatically,
/// with no player declaration. They are distinct `TriggerKind`s from the
/// counteraction kinds so a card can tell a field effect from a hand counteraction by its
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
            | TriggerKind::PassPlayer
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
            | TriggerKind::DrewBefore
            | TriggerKind::Reshuffled
            | TriggerKind::Bought
            | TriggerKind::BeforeOut
            | TriggerKind::Teleported
            | TriggerKind::RollPlan
            | TriggerKind::AbnormalGuard
            | TriggerKind::ImmuneAll
            | TriggerKind::Untargetable
            | TriggerKind::Redirect
            | TriggerKind::CrystalsChanged
            | TriggerKind::CpChanged
    )
}

/// Kinds C# runs on a fresh instance of one card (named on `t.card`) rather
/// than on the cards in play.
fn is_own_card_kind(kind: TriggerKind) -> bool {
    // `Drawn` is the drawn card's own hook (it is still in hand). `Discarded`
    // is a field-card point instead -- a placed card hears about discards.
    matches!(kind, TriggerKind::Drawn)
}

/// The match-start points. They reach **every effect source** of the player
/// the engine raised them for -- field cards including skills (in field order)
/// and the card ids in that player's piles/hands -- not just the card named on
/// `t.card`. See [`WasmRules::game_start_hooks`].
fn is_game_start_kind(kind: TriggerKind) -> bool {
    matches!(
        kind,
        TriggerKind::DeckBeforeGame | TriggerKind::DeckAtGameStart
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
    use super::{chain_starter, trigger_kind, CoreTrigger, Priority};
    use crate::TriggerKind;

    #[test]
    fn a_round_belongs_to_the_player_who_activated_the_effect() {
        // Seat 2's card makes seat 0 pay: the `effect` is the payer's (player 0),
        // but seat 2 activated it, so the round is seat 2's -- and the ring
        // starts at seat 2 (ruling 2026-10-07).
        let mut pay = CoreTrigger::new("effect", 0);
        pay.by_card = Some(2);
        assert_eq!(chain_starter(&pay, 1, 4), 2);
        // Rent is board-driven (`by_card` is None -- a system / tile event):
        // the link has no player, so the ring starts at the active turn player
        // (ruling 2026-10-07), not at the payer (player 3).
        let rent = CoreTrigger::new("effect", 3);
        assert_eq!(chain_starter(&rent, 1, 4), 1);
        // A bank-side link with neither: whoever's turn it is.
        let mut bank = CoreTrigger::new("effect", 0);
        bank.player_id = -1;
        assert_eq!(chain_starter(&bank, 1, 4), 1);
    }

    /// Drive one round's visit order. `script` gives, per visit in order, how
    /// many counters that visit declares -- a visit keeps the floor until the
    /// seat passes or runs out (ruling 2026-10-07; `build_round`), so several
    /// declarations in one entry are one visit. Returns every seat visited.
    /// `initial` is the round's initial user; the ring starts at their seat and
    /// a lap is one full cycle from there.
    fn visited(initial: usize, n: usize, script: &[usize]) -> Vec<usize> {
        let mut p = Priority::new(initial, n);
        let mut out = vec![];
        let mut i = 0;
        loop {
            let s = p.seat();
            out.push(s);
            let declared = script.get(i).copied().unwrap_or(0) > 0;
            i += 1;
            if !p.answered(declared) {
                return out;
            }
        }
    }

    #[test]
    fn a_round_opens_at_the_initial_user() {
        // Seat 2 raised the timing: the ring asks 2 first, then 3, 0, 1 --
        // superseding clause 89's 「下一位」 start (2026-10-07).
        assert_eq!(visited(2, 4, &[]), [2, 3, 0, 1]);
        // A one-player ring asks that seat once.
        assert_eq!(visited(0, 1, &[]), [0]);
    }

    #[test]
    fn a_quiet_lap_closes_the_round_and_a_carried_lap_does_not() {
        // No declarations: one lap and done.
        assert_eq!(visited(2, 4, &[]), [2, 3, 0, 1]);
        // A declaration anywhere in lap 1 carries it -- even at its last visit
        // -- so the round runs lap 2 in full and closes only when that lap is
        // quiet (ruling 2026-10-07: "a lap that carried a declaration never
        // closes the round").
        assert_eq!(visited(2, 4, &[0, 0, 0, 1]), [2, 3, 0, 1, 2, 3, 0, 1]);
        // A declaration at the initial user's own visit carries lap 1 the same
        // way; the visits after it in that lap do not close the round.
        assert_eq!(visited(2, 4, &[1]), [2, 3, 0, 1, 2, 3, 0, 1]);
        // Declarations in two consecutive laps need a third quiet one.
        assert_eq!(
            visited(2, 4, &[0, 1, 0, 0, 1]),
            [2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1]
        );
        // A one-player ring: a declaring visit is carried, the next one closes.
        assert_eq!(visited(0, 1, &[1, 0]), [0, 0]);
    }

    /// The engine raises these kinds; folding any of them to `None` would make
    /// `can_counteract` guards silently never match (34 cards depend on them).
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
            // yet, but `can_counteract` guards match on them so they must map)
            "passPlayer",
            "pay",
            "roll",
            "counteracted",
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
            "settleBody",
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
            // v36: [CP点] writes
            "cpChanged",
            // v42: the command-wide pre-split payment stage + terminals
            "payTotalAdd",
            "payTotalMul",
            "payTotalCancel",
            "tileResolved",
            "moveResolved",
            "bankruptResolved",
            // v43: the move-head / move-tail pair
            "moveBefore",
            "moveAfter",
        ] {
            assert!(
                !matches!(trigger_kind(k), TriggerKind::None),
                "{k} folded to TriggerKind::None"
            );
        }
    }
}
