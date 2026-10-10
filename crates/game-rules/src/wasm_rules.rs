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
use crate::world::{CardWorld, Trigger, TriggerBuy, TriggerMove, TriggerPay};
use crate::PromptKind;
use crate::{CardPile, TriggerKind};
use rules_cond::WindowScope;


/// Declared static properties of one card, from a [`Run`]'s snapshot.
fn props_of(
    props: &std::collections::HashMap<String, std::collections::BTreeMap<String, i32>>,
    id: &str,
) -> std::collections::BTreeMap<String, i32> {
    props.get(id).cloned().unwrap_or_default()
}

/// One run of a card module against a copy of the match world.
/// Dest fates the module returns via `set_dest` (see [`DEST_GRAVEYARD`] /
/// [`DEST_FIELD`] / [`DEST_UNSET`]).
const DEST_GRAVEYARD: i32 = 0;
const DEST_FIELD: i32 = 2;
/// The run never named a fate. Distinct from [`DEST_GRAVEYARD`] on purpose: a
/// play with no opinion still lands in the discard (`dest_from`'s wildcard),
/// but a field effect with no opinion must leave its card where it is.
const DEST_UNSET: i32 = -1;

/// A counter-war is bounded by hands shrinking as cards declare; 16 is
/// plenty for a legal exchange. A deeper nest is a runaway net.
const MAX_COUNTERACT_DEPTH: u32 = 16;

/// Hard cap on the offers one seat gets in a single visit of the ask ring
/// (ruling 2026-10-07: a seat keeps the floor until it passes or runs out of
/// eligible counteractions). The offer set shrinks as cards are declared, so a
/// visit terminates on its own; the cap is a runaway guard, not a design limit.
/// TODO(规则书): the book states no per-visit bound.
const MAX_COUNTERACT_PER_VISIT: u32 = 16;

/// Piles, randomness and their log at a host boundary. Replaying a card starts
/// from the pre-draw piles, then adopts each completed request at its original
/// statement. A discard/redraw effect cannot discard its freshly drawn hand.
///
/// A **snapshot** (deep copy at construction): the checkpoint outlives the
/// mutations that follow the boundary, so it cannot share the live handle.
#[derive(Clone)]
struct PileCheckpoint {
    world: Arc<game_core::engine::World>,
}
impl PileCheckpoint {
    fn new(world: game_core::engine::World) -> Self {
        Self {
            world: Arc::new(world),
        }
    }
    fn apply(&self, world: &mut game_core::engine::World) {
        world.hidden = self.world.hidden.clone();
        world.rng = self.world.rng.clone();
        world.recent = self.world.recent.clone();
        world.next_event = self.world.next_event;
    }
}

#[derive(Clone)]
pub struct Run {
    /// The world this run reads and writes. A [`SharedWorld`] (fix B): the
    /// drive's per-iteration copy and every pure check share the live world
    /// handle, and a body that writes detaches a private copy
    /// (`Arc::make_mut`) -- so a check whose body never writes never clones.
    /// Cloning a `Run` into a rules store is a refcount bump, not a deep copy.
    world: game_core::engine::SharedWorld,
    pile_checkpoints: Arc<std::collections::BTreeMap<usize, PileCheckpoint>>,
    data: Arc<GameData>,
    /// Declared static properties (`card_props`) of every card in the loaded
    /// set, snapshotted at run creation -- the one thing a `Run` needs from the
    /// card modules. A snapshot (not a `CardModules` handle) keeps `Run`
    /// concrete so the native backend can hold one in a thread-local.
    props: Arc<std::collections::HashMap<String, std::collections::BTreeMap<String, i32>>>,
    trigger: Trigger,
    /// The card whose effect is running -- the one `place_card` /
    /// `unplace_self` act on.
    current_card: String,
    /// The **instance** this run is for. A card's name is not an identity --
    /// one player may hold several copies of the same card in play -- so the
    /// uid is. It is the one the dispatch named, and it follows the card if
    /// this run re-places it (the same object moves with it).
    current_uid: i32,
    /// Where this card goes when the run finishes, set by `set_dest`.
    dest: i32,
    /// Whose pile the fate lands in, set by `set_transfer_to_dest`. `None` is
    /// the owner the instance leaves -- what `set_dest` names.
    dest_to: Option<i32>,
    /// Payments this run actually made, `(from, to, amount)` -- the engine
    /// raises `payAfter` / `paid` for each once the run commits.
    paid_log: Vec<(i32, i32, i32)>,
    /// Cards this run put into a discard pile, `(player_id, id)` -- the engine
    /// raises `discarded` for each once the run commits.
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
    /// Named-counter writes this run made, `(owner, card, name, change)` -- the
    /// engine raises `counterChanged` for each once the run commits. `change`
    /// is the signed delta the write applied (0 = a write that landed on the
    /// same count, e.g. a card placed with none); the count it left is the
    /// instance's own `counter(name)`. One log for every named counter:
    /// `counter::CRYSTALS`, `counter::CP`, and any card-declared name.
    counter_log: Vec<(i32, String, String, i32)>,
    /// Players this run granted [除外] layers to -- the engine raises `exile`
    /// for each once the run commits (「任意玩家获得[除外]…时」 handlers).
    exile_log: Vec<i32>,
    /// Which of the card's numbers this play doubles, or -1. Set by the
    /// doubling band skill, which is not ported yet.
    doubled: i32,
    /// Props the running card wants on the lingering instance `ctx::linger`
    /// binds (`docs/PURCHASE.md` P5). A hand play has no field instance for
    /// `set_prop` to write, so a `set_prop` against no instance parks the value
    /// here and `linger` carries it onto [`crate::world::Lingering::props`].
    linger_props: std::collections::BTreeMap<String, i32>,
    /// This run is an already-in-play card applying its effect (a field /
    /// tile / lingering hook, a settle body, a scheduled callback): its log
    /// lines say 「<卡名> 的效果：<what happened>」 rather than standing alone.
    /// A fresh activation (a hand play, a [反击] declaration, a skill press,
    /// a drawn event) logs its own action and leaves its outcome lines bare.
    wrap_effect: bool,
    /// Live-stream sink for write-through logs (`ctx::log`, the activation
    /// header). `None` outside a drive (probes, quotes). Each post appends to
    /// the **live** event tail at the call site, so a body that logs → pays →
    /// logs shows log, pay, log in code order. A multi-pass re-run's k-th log
    /// replaces [`Self::posted`]\[k\] instead of appending (same shape as the
    /// pre-filled `answers` log).
    live: Option<*mut Cx<'static>>,
    /// Event ids this drive has posted to the live stream, in call order.
    /// Survives across passes (the drive hands it back at each pause).
    posted: Vec<i32>,
    /// How many log calls this pass has consumed (index into [`Self::posted`]).
    log_idx: usize,
    /// The `"card"` activation event id this run groups under (`-1` = none).
    /// Every write-through event carries it, so the log nests the outcome
    /// under the header.
    parent: i32,
}

impl Run {
    /// An already-in-play card's log lines name their source: 「<卡名> 的效果：
    /// <what happened>」. A fresh activation (hand play, [反击], skill press,
    /// drawn event) leaves its outcome lines bare -- the play already said
    /// which card they belong to.
    fn attribute(&self, msg: Msg) -> Msg {
        if self.wrap_effect {
            Msg::new("log.card_effect")
                .card("card", self.current_card.clone())
                .msg("what", msg)
        } else {
            msg
        }
    }

    /// Post one log line **write-through** to the live event stream (the
    /// activation header, `ctx::log`, `ctx::effect`). Walk flushes happen on
    /// the live world first (`World::log`), so an approach always precedes the
    /// line that interrupted it.
    ///
    /// A multi-pass re-run's k-th call replaces the k-th already-posted line
    /// (same shape as the pre-filled `answers` log) instead of appending a
    /// duplicate. Without a live sink (probe / quote) the line lands on the
    /// guest copy only, as before.
    fn post_log(&mut self, kind: &str, player_id: i32, msg: Msg, card: &str) {
        if let Some(live) = self.live {
            let cx = unsafe { &mut *live };
            let parent = self.parent;
            if self.log_idx < self.posted.len() {
                let id = self.posted[self.log_idx];
                self.log_idx += 1;
                if !cx.relog_event(id, msg.clone()) {
                    // Truncated tail -- append a fresh one and retarget the slot.
                    let id = cx.log_event(kind, player_id, msg, parent, card);
                    self.posted[self.log_idx - 1] = id;
                }
                return;
            }
            self.log_idx += 1;
            let id = cx.log_event(kind, player_id, msg, parent, card);
            self.posted.push(id);
            return;
        }
        // No live sink: guest-copy only (probe / quote).
        let e = self.world.log(kind, player_id, msg);
        if !card.is_empty() {
            e.card = card.to_string();
        }
        if self.parent >= 0 {
            e.parent = self.parent;
        }
    }

    /// Post a `"card"` activation (the 「效果适用」 header / a skill 「发动」 /
    /// a negated flash) write-through. Returns the event id.
    fn post_card(
        &mut self,
        kind: &str,
        owner: i32,
        msg: Msg,
        card: &str,
        target: i32,
        tile: i32,
        negated: bool,
    ) -> i32 {
        if let Some(live) = self.live {
            let cx = unsafe { &mut *live };
            let parent = self.parent;
            if self.log_idx < self.posted.len() {
                let id = self.posted[self.log_idx];
                self.log_idx += 1;
                if cx.relog_event(id, msg.clone()) {
                    return id;
                }
                let id = cx.log_card_activation(
                    kind, owner, card, target, tile, negated, msg, parent,
                );
                self.posted[self.log_idx - 1] = id;
                return id;
            }
            self.log_idx += 1;
            let id =
                cx.log_card_activation(kind, owner, card, target, tile, negated, msg, parent);
            self.posted.push(id);
            return id;
        }
        let e = self
            .world
            .card_activation(kind, owner, card, target, tile, negated, msg);
        e.parent = self.parent;
        e.id
    }

    /// Record a crystal write on the instance at `uid` so the commit point can
    /// raise `crystalsChanged` (the same shape as `fire_spent_log` /
    /// `house_log`). `was` and `now` bracket the write; only the delta rides
    /// the trigger, so a write that lands on the same count still raises --
    /// a card placed with no crystals has to hear about its own count. A write
    /// to an instance that is not on a field raises nothing.
    fn note_counter_changed(&mut self, uid: i32, was: i32, now: i32, name: &str) {
        let Some(f) = self.world.field_by_uid(uid) else {
            return;
        };
        let (owner, card) = (f.owner, f.card.clone());
        self.counter_log.push((owner, card, name.to_string(), now - was));
    }

    /// Record an **on-card** [CP点] write against the instance at `uid` so the
    /// commit point can raise `cpChanged` (the same shape as
    /// [`Self::note_crystals`]). `was` and `now` bracket that instance's
    /// `FieldCard::cp`, so a write that lands on the same count still raises
    /// and a count emptied by *any* path -- this run's settle, another effect's
    /// removal -- leaves the field the same way. A write with no live instance
    /// raises nothing.


    /// Move the running instance to `dest` **now** -- unplace-and-send, applied
    /// mid-effect rather than at the run's commit, for a card that must be gone
    /// before the rest of the effect runs. `to` is whose pile it lands in;
    /// `None` is the owner it leaves. Returns that owner, or `None` when it was
    /// not in play. Clears `current_uid`, so a later deferred fate finds no
    /// instance to move.
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
    fn after_host(&mut self, answer: usize) {
        if let Some(checkpoint) = self.pile_checkpoints.get(&answer) {
            checkpoint.apply(&mut self.world);
            // These maintenance hooks already ran before this host request.
            self.reshuffle_log.clear();
        }
    }
    fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32 {
        self.world.roll(player_id, count, sides)
    }

    fn effect(&mut self, player_id: i32, msg: Msg) {
        let msg = self.attribute(msg);
        // Name the running card on the event, so the client can ride the line
        // on that card's flash (正论暴击 and kin) instead of popping the effect
        // modal. The id matches the `"card"` activation's (`event:` instances
        // are announced without the prefix); empty outside a card body keeps
        // today's popup.
        let src = self
            .current_card
            .strip_prefix("event:")
            .unwrap_or(&self.current_card)
            .to_string();
        self.post_log("effect", player_id, msg, &src);
    }

    fn extreme(&self) -> i32 {
        self.world.turn.extreme.as_i32()
    }
    fn set_extreme(&mut self, v: i32) {
        self.world.turn.extreme = game_core::engine::Extreme::from_i32(v.signum());
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
        let msg = self.attribute(msg);
        self.post_log("text", player_id, msg, "");
    }

    // board -----------------------------------------------------------------
    fn tile_count(&self) -> i32 {
        self.data.tiles.len() as i32
    }
    fn tile_named(&self, name: &str) -> i32 {
        self.world.tile_named(&self.data, name)
    }
    fn tile_name(&self, tile: i32) -> String {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.name.replace('\n', ""))
            .unwrap_or_default()
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
        let will_refill = self
            .world
            .hidden
            .get(player_id.max(0) as usize)
            .is_some_and(|h| !h.discard.is_empty() && n > 0 && h.draw.len() <= n as usize);
        let got = self.world.draw_cards(player_id, n, true);
        if will_refill && got > 0 {
            self.reshuffle_log.push(player_id);
        }
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
                if pile == CardPile::Deck && self.world.refill_draw_pile(player_id.max(0) as usize) {
                    self.reshuffle_log.push(player_id);
                }
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
        // `pos`: top = draw.Add (the end of the vec is the top),
        // bottom = Insert(0), anything else = add + shuffle.
        let idx = player_id.max(0) as usize;
        if self.world.hidden.get(idx).is_none() {
            return;
        }
        match pos {
            1 => self.world.hidden[idx].draw.insert(0, card.to_string()),
            2 => {
                self.world.hidden[idx].draw.push(card.to_string());
                let mut draw = std::mem::take(&mut self.world.hidden[idx].draw);
                self.world.rng.shuffle(&mut draw);
                self.world.hidden[idx].draw = draw;
            }
            _ => self.world.hidden[idx].draw.push(card.to_string()),
        }
    }
    fn to_discard(&mut self, player_id: i32, card: &str) {
        if self.world.to_discard(player_id, card) {
            self.reshuffle_log.push(player_id);
        }
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
    fn card_counter(&self, player_id: i32, card: &str, name: &str) -> i32 {
        let uid = self
            .world
            .field_instances(player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
        self.world.counter_at(uid, name)
    }
    fn add_card_counter(&mut self, player_id: i32, card: &str, name: &str, n: i32, max: i32) -> i32 {
        let uid = self
            .world
            .field_instances(player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
        let was = self.world.counter_at(uid, name);
        let now = self.world.add_counter_at(uid, name, n, max);
        self.note_counter_changed(uid, was, now, name);
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
    fn counter(&self, name: &str) -> i32 {
        self.world.counter_at(self.current_uid, name)
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
    fn set_counter(&mut self, name: &str, n: i32) -> i32 {
        let uid = self.current_uid;
        let was = self.world.counter_at(uid, name);
        let now = self.world.set_counter_at(uid, name, n);
        self.note_counter_changed(uid, was, now, name);
        now
    }
    fn add_counter(&mut self, name: &str, n: i32, max: i32) -> i32 {
        let uid = self.current_uid;
        let was = self.world.counter_at(uid, name);
        let now = self.world.add_counter_at(uid, name, n, max);
        self.note_counter_changed(uid, was, now, name);
        now
    }
    fn counter_at(&self, uid: i32, name: &str) -> i32 {
        self.world.counter_at(uid, name)
    }
    fn add_counter_at(&mut self, uid: i32, name: &str, n: i32, max: i32) -> i32 {
        let was = self.world.counter_at(uid, name);
        let now = self.world.add_counter_at(uid, name, n, max);
        self.note_counter_changed(uid, was, now, name);
        now
    }
    fn field_instances(&self, player_id: i32) -> Vec<(i32, String)> {
        self.world.field_instances(player_id)
    }
    fn crystals_at(&self, uid: i32) -> i32 {
        self.world.crystals_at(uid)
    }
    fn self_uid(&self) -> i32 {
        self.current_uid
    }
    fn set_self_uid(&mut self, uid: i32) {
        self.current_uid = uid;
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

    // marks & bound units (user ruling 2026-10-10) ---------------------------
    /// Bind `count` units of the running instance's counter `kind` to `tile`.
    /// Marker ownership (user ruling 2026-10-07): the rule that creates a
    /// mark owns it -- `skill:要乐奈` owns every 抹茶芭菲 copy, on any tile
    /// or player counter. `instance` is stamped as the current uid and `src`
    /// (provenance, -1 → current uid).
    fn place_mark(
        &mut self,
        tile: i32,
        kind: &str,
        category: &str,
        owner: i32,
        src: i32,
        count: i32,
        note: Msg,
        fresh: bool,
    ) -> i32 {
        let who = self.current_card.clone();
        self.world.note_marker_owner(kind, &who);
        let me = self.current_uid;
        let src = if src < 0 { me } else { src };
        self.world
            .place_mark(me, kind, category, tile, owner, src, count, note, fresh)
    }
    fn count_marks(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        self.world.count_marks(tile, filter)
    }
    fn bump_mark(
        &mut self,
        tile: i32,
        filter: &game_core::state::MarkFilter<'_>,
        delta: i32,
    ) -> i32 {
        self.world.bump_mark(tile, filter, delta)
    }
    fn remove_marks(&mut self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        self.world.remove_marks(tile, filter)
    }
    fn mark_src_at(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        self.world.mark_src_at(tile, filter)
    }
    fn mark_instance_at(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        self.world.mark_instance_at(tile, filter)
    }
    /// Units of the running instance's counter `name` held by `player_id`.
    fn count_held(&self, name: &str, player_id: i32) -> i32 {
        self.world.count_held(self.current_uid, name, player_id)
    }
    fn add_held(&mut self, name: &str, player_id: i32, n: i32, max: i32) -> i32 {
        // Marker ownership: the rule that creates a marker owns it.
        let who = self.current_card.clone();
        self.world.note_marker_owner(name, &who);
        self.world
            .bind_held(self.current_uid, name, player_id, n, max)
    }
    fn count_held_name(&self, name: &str, player_id: i32) -> i32 {
        // Name-keyed (any owner / instance): the legacy `tok` lookup.
        self.world.tok(player_id, name)
    }
    fn set_held_name(&mut self, name: &str, player_id: i32, v: i32) {
        let who = self.current_card.clone();
        self.world.note_marker_owner(name, &who);
        // Stamp the owning instance on a new row (user ruling 2026-10-10).
        self.world.set_tok(player_id, name, v, self.current_uid);
    }
    fn move_units(
        &mut self,
        name: &str,
        from_tile: i32,
        from_player: i32,
        to_tile: i32,
        to_player: i32,
        n: i32,
    ) -> i32 {
        self.world.move_units(
            self.current_uid,
            name,
            from_tile,
            from_player,
            to_tile,
            to_player,
            n,
        )
    }
    fn mark_rule_instances(&self) -> Vec<(i32, String)> {
        self.world.mark_rule_instances()
    }
    fn tok_names(&self, player_id: i32, prefix: &str) -> Vec<String> {
        self.world.tok_names(player_id, prefix)
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
    fn character_name(&self, player_id: i32) -> Option<String> {
        // The `character_is` spelling: `MatchPlayer::character`.
        self.world
            .st
            .players
            .get(player_id.max(0) as usize)
            .map(|s| s.character.clone())
            .filter(|c| !c.is_empty())
    }
    fn band_name(&self, player_id: i32) -> Option<String> {
        // The `in_band` spelling: the character's band from the data table.
        let s = self.world.st.players.get(player_id.max(0) as usize)?;
        self.data
            .characters
            .iter()
            .find(|c| c.name == s.character)
            .map(|c| c.band.clone())
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
        // A 「商店街」 deed: buyable and colour group 10.
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.is_buyable() && t.group == 10) as i32
    }
    fn is_ring(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.kind == game_core::data::TileKind::Ring) as i32
    }
    fn is_circle(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.kind == game_core::data::TileKind::Circle) as i32
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
            .is_some_and(|t| t.kind == game_core::data::TileKind::Agent) as i32
    }
    fn is_live_house(&self, tile: i32) -> i32 {
        // Live House: buyable and colour group 6. The per-player colour
        // override is a tile prop, so see `is_live_house_for`.
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
        // Land price alone (`buy_price` adds houses).
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
        // Cap at `rent.Length - 1` (the board's house max; RiNG deeds have no
        // rent table and never hold houses).
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
        // The shorter way around the ring.
        let n = self.data.tiles.len() as i32;
        if n <= 0 {
            return 0;
        }
        let d = ((b - a) % n + n) % n;
        d.min(n - d)
    }
    fn tile_forward(&self, a: i32, b: i32) -> i32 {
        // Steps forward from a to b around the ring.
        let n = self.data.tiles.len() as i32;
        if n <= 0 {
            return 0;
        }
        ((b - a) % n + n) % n
    }
    fn neighbor(&self, player_id: i32, dir: i32) -> i32 {
        // The next present player in turn order, wrapping.
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
        // One copy, hand -> discard pile.
        let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) else {
            return 0;
        };
        let Some(p) = h.hand.iter().position(|c| c == card) else {
            return 0;
        };
        h.hand.remove(p);
        h.discard.push(card.to_string());
        self.discard_log.push((player_id, card.to_string()));
        if self.world.refill_draw_pile(player_id.max(0) as usize) {
            self.reshuffle_log.push(player_id);
        }
        1
    }
    fn shuffle_into_deck(&mut self, player_id: i32, hand: bool, discard: bool) -> i32 {
        // Shuffle the requested piles into the draw pile.
        let idx = player_id.max(0) as usize;
        if self.world.hidden.get(idx).is_none() {
            return 0;
        }
        let mut moved: Vec<String> = Vec::new();
        if hand {
            moved.append(&mut self.world.hidden[idx].hand);
        }
        if discard {
            moved.append(&mut self.world.hidden[idx].discard);
        }
        let n = moved.len() as i32;
        self.world.hidden[idx].draw.append(&mut moved);
        let mut draw = std::mem::take(&mut self.world.hidden[idx].draw);
        self.world.rng.shuffle(&mut draw);
        self.world.hidden[idx].draw = draw;
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
        // A play whose card declares recipients (`DESIGNATES`) names the other
        // living players of its user (「[指定][使用者]以外的所有玩家」 /
        // 「其他玩家[分摊]」). The play's card is the one the trigger names.
        let card = self.trigger.card.as_deref().unwrap_or("");
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
        self.world.turn.play_doubled.unwrap_or(-1)
    }

    fn set_play_doubled(&mut self, n: i32) {
        self.world.turn.play_doubled = (n >= 0).then_some(n);
    }

    // status extensions -----------------------------------------------------
    fn can_pay(&self, player_id: i32) -> i32 {
        // Not out, not stunned, not exiled.
        let Some(s) = self.world.st.players.get(player_id.max(0) as usize) else {
            return 0;
        };
        (!s.out() && !s.stunned() && s.exile() == 0) as i32
    }
    fn cant_move(&self, player_id: i32) -> i32 {
        // Why the [主要移动] is still unavailable (0 = free).
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
        // Spend pots first; logs the spend with its reason (the caller's
        // message is the log line, like `gain`/`pay`).
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
        // `round * 100 + turn + 1` -- once-per-turn latches.
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
        // The character's band.
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
        // Rewrite the face on the move payload when one exists. A card-driven
        // `roll` raise has no `t.Move`; there the face rides `value` and the
        // writeback reads it from there (Y.O.L.O on 热气球's 4x3d20).
        if let Some(m) = &mut self.trigger.mv {
            m.roll = Some(roll);
        } else {
            self.trigger.value = roll;
        }
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
        (self.trigger.card.as_deref() == Some(id)) as i32
    }
    fn set_trigger_price(&mut self, v: i32) {
        if let Some(b) = &mut self.trigger.buy {
            b.price = v;
        }
    }
    fn set_trigger_deal_owner(&mut self, v: i32) {
        if let Some(b) = &mut self.trigger.buy {
            b.deal_owner = Some(v);
        }
    }
    fn set_trigger_deal_houses(&mut self, v: i32) {
        if let Some(b) = &mut self.trigger.buy {
            b.deal_houses = v;
        }
    }
    fn set_trigger_deal_mortgaged(&mut self, v: i32) {
        if let Some(b) = &mut self.trigger.buy {
            b.deal_mortgaged = v != 0;
        }
    }
    fn set_trigger_reason(&mut self, reason: &str) {
        self.trigger.reason = Some(reason.to_string());
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
            // The instance asking for the callback, captured now. The
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
        // Fresh instance: the nested run is not the outer card's field card,
        // so it starts with no uid of its own until it places one.
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
        // Keep the shared MoveBefore link coherent for later counters and
        // hooks. Its base distance is already final; a plan rewrite extends
        // this move rather than rewriting a roll that has already resolved.
        if self.trigger.kind == TriggerKind::MoveBefore {
            let delta = v.max(0) - self.trigger.value.max(0);
            self.trigger.value = v.max(0);
            if let Some(m) = &mut self.trigger.mv {
                m.total = (m.total + delta).max(0);
                m.remaining = (m.remaining + delta).max(0);
            }
        }
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
    /// In the game and not exiled.
    fn present(&self, s: i32) -> bool {
        self.world
            .st
            .players
            .get(s.max(0) as usize)
            .is_some_and(|x| !x.out() && x.exile() == 0)
    }

    /// Present players standing on the tile, minus `except`.
    fn players_on_list(&self, tile: i32, except: i32) -> Vec<i32> {
        (0..self.world.st.players.len() as i32)
            .filter(|&p| {
                p != except && self.present(p) && self.world.st.players[p as usize].pos == tile
            })
            .collect()
    }

    /// The player's private hand, in order.
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
        let kind = p.kind;
        let ask = prompt_to_ask(p);
        let items = ask.view.items.clone();
        match cx.ask(ask) {
            Ok(reply) => {
                let v = reply
                    .a
                    .answers
                    .first()
                    .copied()
                    .filter(|&x| x >= 0)
                    .unwrap_or(reply.fallback);
                let v = recorded_answer(kind, &items, v);
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
        // Draws mutate piles and run after-hooks immediately. Re-enter the
        // guest from the engine's resulting snapshot, so both execution modes
        // see those changes before subsequent guest reads and prompts.
        let run = Self::as_run(any);
        if matches!(req, HostRequest::Draw { .. }) || !run.pile_checkpoints.is_empty() {
            return None;
        }
        let cx = unsafe { &mut *self.cx };
        let bridge = unsafe { &*self.bridge };
        let card_id: &str = unsafe { &*self.card_id };
        cx.adopt_turn_policy(&run.world);
        cx.overlay_guest_state(&run.world);
        let linger = run.linger_props.clone();
        let live_before = cx.live_event_count();
        match bridge.apply_host_request(cx, req, &linger, self.call, card_id) {
            Ok(v) => {
                cx.stamp_parent_since(live_before, run.parent);
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

    /// Does this drive's log lines name their source (「<卡名> 的效果：…」)?
    ///
    /// **No.** The 「效果适用」 header at body entry names the card and the
    /// trigger; the body's own lines sit under that header as its results, so
    /// repeating 「<卡名> 的效果：」 on every line is noise. A fresh activation
    /// (hand play, [反击], skill press, drawn event) already logged its action
    /// and likewise leaves its outcome lines bare.
    fn wraps_effect(_call: Call, _card_id: &str) -> bool {
        false
    }

    /// One `"card"` activation event for a body that is about to run: the
    /// client's card flash plus its 「效果适用」 line. `call` / `card_id` /
    /// `uid` are the drive's own context; `trigger` says why it ran; `entry`
    /// names which clause of a multi-entry card (its manifest label, if any).
    ///
    /// Written into the world copy `w` (the guest's), then captured as a
    /// guest-side burst ([`Run::capture_guest`]) so [`Self::commit_after`] can
    /// re-insert it **before** the host-effect events the learn pass already
    /// logged -- the same code-order merge that sequences body logs around
    /// pays. The return value is the event's id, used as the `parent` of
    /// everything this body produces.
    ///
    /// Called exactly at **body entry** (the `on_body` callback
    /// [`Self::drive_inner_body`] hands to `run` / `run_hook`): after the CEL
    /// `pre` and the residual guard admitted, after any play gate, and only
    /// when the entry exists -- so a guard reject, a probe, a quote or a
    /// bodyless drive never reaches the UI. One drive's multi-pass re-runs
    /// announce on the copy that lands (the others are thrown away with their
    /// pass).
    ///
    /// A tile's own settle body (`tile:*`) is a board square, not a card, so it
    /// does not flash -- a *card* placed on the square still does. `On::Message`
    /// never reaches here (internal plumbing, no `Call::Message`).
    fn announce_drive(
        &self,
        w: &mut crate::Run,
        call: Call,
        card_id: &str,
        uid: i32,
        target: i32,
        tile: i32,
        trigger: &Trigger,
        entry: i32,
    ) -> i32 {
        if matches!(call, Call::Settle { .. }) && card_id.starts_with("tile:") {
            return -1;
        }
        use game_core::state::card_trigger;
        let kind = match call {
            Call::Counteract { .. } => card_trigger::COUNTER,
            Call::Play { .. } => {
                if card_id.starts_with("event:") {
                    card_trigger::EVENT
                } else if card_id.starts_with("skill:") {
                    card_trigger::SKILL
                } else {
                    card_trigger::PLAY
                }
            }
            // `On::Hook` / `RollPlan` / `AtEnd` (and a card's settle body):
            // a placed card's effect firing on someone else's turn.
            _ => card_trigger::HOOK,
        };
        // An event instance is bound under `event:<id>`; the draw's own event
        // names it without the prefix, so the flash and the two log lines agree.
        let cid = card_id.strip_prefix("event:").unwrap_or(card_id);
        let owner = if uid >= 0 {
            w.world
                .field_by_uid(uid)
                .map(|f| f.owner)
                .unwrap_or(call.player_id())
        } else {
            call.player_id()
        };
        let who = if owner >= 0 { owner } else { target };
        // Entry label (「（1）」 and kin) from the manifest, when the card
        // declared one for this entry.
        let label = self.entry_label(card_id, call, entry);
        // Wording:
        // * a skill press has no declaration line of its own -> 「发动」
        // * a hand play / a [反击] declaration / a drawn event already logged
        //   their action (打出 / 抽到); their flash keeps no line of its own
        //   (the player's action is the why)
        // * everything else that runs as an already-in-play card or a
        //   resolution -> 「效果适用」 + why + (on commit) the outcome
        let msg = match kind {
            card_trigger::SKILL => Msg::new("log.card_activated")
                .player_id("who", who)
                .card("card", cid),
            card_trigger::HOOK | card_trigger::COUNTER => {
                let why = if matches!(call, Call::Counteract { .. }) {
                    // Resolution names the answered link's card and what it
                    // was doing (the declaration already logged 打出[反击]).
                    crate::counteract_reason(
                        trigger.card.as_deref().unwrap_or(""),
                        trigger,
                    )
                } else {
                    crate::trigger_reason(trigger)
                };
                Msg::new("log.effect_applied")
                    .card("card", cid)
                    .text("label", label)
                    .msg("why", why)
            }
            _ => Msg::default(),
        };
        // Write-through: the header posts to the live stream at body entry,
        // so it precedes every host-effect and body line this run causes.
        // A multi-pass re-run's header replaces the first post in place.
        w.post_card(kind, owner, msg, cid, target, tile, false)
    }

    /// The manifest label of `card_id`'s `entry` (「（1）」 / 「（2）」), or `""`.
    fn entry_label(&self, card_id: &str, call: Call, entry: i32) -> String {
        let _ = call;
        let Some(idx) = self.ruleset.card(card_id) else {
            return String::new();
        };
        self.ruleset
            .cards()
            .get(idx as usize)
            .and_then(|c| c.on.get(entry.max(0) as usize))
            .and_then(|o| o.label.clone())
            .unwrap_or_default()
    }

    /// Run one effect to completion, prompting through the engine as needed.
    /// Returns the card's destination (`set_dest`). A reroll the module made
    /// (`set_move_roll`) is written back to `trigger` -- the shared `t.Move` the
    /// counteractions also read.
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
    /// cost. A body that runs announces itself once, on the first pass (a
    /// prompt re-runs the module) -- see [`Self::announce_drive`].
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
        let mut pile_base: Option<PileCheckpoint> = None;
        let mut pile_checkpoints = std::collections::BTreeMap::new();
        // Guest-state overlays this drive pushed (one per `NeedHost`). Dropped
        // at the top of the next iteration -- the routine has finished and the
        // replay re-applies the guest writes from the restored world. Nested
        // drives record their own base, so they never pop an outer one.
        let overlay_base = cx.guest_overlay_depth();
        // The activation announcement fires at **body entry** (see
        // [`Self::announce_drive`]): `run` / `run_hook` call this on the world
        // copy the body will write into, only once the entry exists and the
        // condition + guard admitted. Multi-pass re-runs announce only the
        // first time each entry runs (the landing pass inherits that announce
        // through the code-order merge in [`Self::commit_after`]).
        let mut announced: std::collections::BTreeSet<i32> = Default::default();
        let mut activation_id = cx.activation.last().copied().unwrap_or(-1);
        let mut posted_ids: Vec<i32> = Vec::new();
        let (target, tile) = (trigger.player_id, trigger.tile);
        loop {
            cx.restore_guests_to(overlay_base);
            // Share the live world (fix B): the body's writes detach a private
            // copy; a body that never writes and has no pile base never clones.
            let mut world = cx.share_world();
            if let Some(base) = &pile_base {
                base.apply(&mut world);
            }
            let mut run = Run {
                world,
                pile_checkpoints: Arc::new(pile_checkpoints.clone()),
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
                counter_log: vec![],
                exile_log: vec![],
                doubled: -1,
                linger_props: Default::default(),
                wrap_effect: Self::wraps_effect(call, card_id),
                live: Some(cx as *mut Cx<'_> as *mut Cx<'static>),
                posted: std::mem::take(&mut posted_ids),
                log_idx: 0,
                parent: activation_id,
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
            let cx_ptr = cx as *mut Cx<'_>;
            let mut announce = |w: &mut crate::Run, entry: i32| {
                if !announced.insert(entry) {
                    // A multi-pass re-run: the first pass already announced
                    // this entry. Consume the header's replace-log slot so the
                    // body's first line does not overwrite the header.
                    w.parent = activation_id;
                    w.live = Some(cx_ptr as *mut Cx<'static>);
                    if w.log_idx < w.posted.len() {
                        w.log_idx += 1;
                    }
                    return;
                }
                w.parent = activation_id;
                w.live = Some(cx_ptr as *mut Cx<'static>);
                let id = self.announce_drive(w, call, card_id, uid, target, tile, trigger, entry);
                if id >= 0 && activation_id < 0 {
                    activation_id = id;
                    unsafe { (*cx_ptr).activation.push(id) };
                }
            };
            let mut guest = || -> Result<Option<Result<Outcome<Run>, RuleError>>, RuleError> {
                if guarded {
                    match self
                        .ruleset
                        .run_hook(&run, call, &answers, Some(&mut announce))
                    {
                        Err(e) => Ok(Some(Err(e))),
                        // Not activated -- the guard refused. Nothing ran, and
                        // nothing about the card reaches the UI.
                        Ok(None) => Ok(None),
                        Ok(Some(hr)) => Ok(Some(Ok(hr.outcome))),
                    }
                } else {
                    Ok(Some(
                        self.ruleset.run(&run, call, &answers, Some(&mut announce)),
                    ))
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
                // A halted drive's write-through lines must not survive: the
                // engine routine re-runs from its snapshot, and a second post
                // would duplicate them.
                cx.drop_events(&posted_ids);
                posted_ids.clear();
                if activation_id >= 0 {
                    cx.activation.retain(|&id| id != activation_id);
                }
                return Err(h);
            }
            // Prompts answered inline this run go on the drive's log **in
            // consumption order**, before whatever pause ended the run appends
            // its own answer -- the same bookkeeping the `NeedInput` arm does.
            if let Some(h) = inline.as_mut() {
                answers.append(&mut h.answers);
            }
            let outcome = match wrapped {
                Ok(Some(outcome)) => outcome,
                // Guard refused.
                Ok(None) => return Ok(DEST_UNSET),
                Err(e) => Err(e),
            };
            match outcome {
                Ok(Outcome::Done(mut after)) => {
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
                        let mut after = after;
                        posted_ids = std::mem::take(&mut after.posted);
                        return self.commit_after(cx, after, call, card_id, trigger);
                    }
                    // Recover the learn pass's write-through log so the commit
                    // pass replaces those lines instead of appending duplicates.
                    posted_ids = std::mem::take(&mut after.posted);
                    let mut world = cx.share_world();
                    if let Some(base) = &pile_base {
                        base.apply(&mut world);
                    }
                    // This pass's world is the one that lands: the learn pass's
                    // copy above is dropped when any pause was answered. Its
                    // write-through lines are already on the live stream, so
                    // this pass **replaces** them in place (same `posted` log)
                    // rather than appending duplicates; its guest writes land
                    // on top of the host effects.
                    let mut run = Run {
                        world,
                        pile_checkpoints: Arc::new(pile_checkpoints.clone()),
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
                        counter_log: vec![],
                        exile_log: vec![],
                        doubled: -1,
                        linger_props: Default::default(),
                        wrap_effect: Self::wraps_effect(call, card_id),
                        live: Some(cx as *mut Cx<'_> as *mut Cx<'static>),
                        posted: std::mem::take(&mut posted_ids),
                        log_idx: 0,
                        parent: activation_id,
                    };
                    let mut announce = |w: &mut crate::Run, _entry: i32| {
                        // Already announced on the learn pass. Consume the
                        // header's slot in the replace log without re-posting
                        // (a second header would duplicate the flash; skipping
                        // the slot would make the first body line overwrite
                        // the header).
                        w.parent = activation_id;
                        w.live = Some(cx_ptr as *mut Cx<'static>);
                        if w.log_idx < w.posted.len() {
                            w.log_idx += 1;
                        }
                    };
                    let outcome2 = if guarded {
                        match self
                            .ruleset
                            .run_hook(&run, call, &answers, Some(&mut announce))
                        {
                            Err(e) => Err(e),
                            Ok(None) => return Ok(DEST_UNSET),
                            Ok(Some(hr)) => Ok(hr.outcome),
                        }
                    } else {
                        self.ruleset
                            .run(&run, call, &answers, Some(&mut announce))
                    };
                    match outcome2 {
                        Ok(Outcome::Done(after2)) => {
                            let mut after2 = after2;
                            posted_ids = std::mem::take(&mut after2.posted);
                            return self.commit_after(cx, after2, call, card_id, trigger);
                        }
                        // The commit pass must not pause: the learn pass
                        // already answered everything. Anything else is a
                        // body that is nondeterministic across the two passes;
                        // fall back to the learn pass's world so the effect
                        // still lands.
                        _ => {
                            let mut after = after;
                            posted_ids = std::mem::take(&mut after.posted);
                            return self.commit_after(cx, after, call, card_id, trigger);
                        }
                    }
                }
                Ok(Outcome::NeedInput(p)) => {
                    let player_id = p.player_id;
                    let kind = p.kind;
                    let ask = prompt_to_ask(p);
                    let items = ask.view.items.clone();
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
                    answers.push(recorded_answer(kind, &items, v));
                    let _ = player_id;
                }
                // A card-driven payment: the same money pipeline as `money()`
                // -- PayAdd -> PayMul -> PayChoose -> PayAt -> the `pay` [反击]
                // window -- then replay the effect with the adjudicated amount
                // (0 = cancelled, and PayAfter runs with 0).
                Ok(Outcome::NeedHost(req, mut run)) => {
                    // Keep the write-through log across the re-run.
                    posted_ids = std::mem::take(&mut run.posted);
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
                    if matches!(req, HostRequest::Draw { .. }) && pile_base.is_none() {
                        pile_base = Some(PileCheckpoint::new(cx.world_copy()));
                    }
                    if pile_base.is_some() {
                        // Make preceding pile writes and their refill visible now.
                        // Replay reconstructs the prefix from pile_base, then skips
                        // already-applied writes by adopting these checkpoints.
                        let mut w = cx.world_copy();
                        PileCheckpoint::new(run.world.deep_clone()).apply(&mut w);
                        cx.swap_world(w);
                        for &player in &run.reshuffle_log {
                            self.raise_core(cx, "reshuffled", player, |_| {})?;
                        }
                    }
                    let live_before = cx.live_event_count();
                    let v = self.apply_host_request(cx, req, &run.linger_props, call, card_id)?;
                    // Group the host-effect events under this activation.
                    cx.stamp_parent_since(live_before, activation_id);
                    if pile_base.is_some() {
                        pile_checkpoints.insert(answers.len(), PileCheckpoint::new(cx.world_copy()));
                    }
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
        // The card shaped the plan and asked for the move to run now.
        // The engine runs it (it may prompt), then the effect replays past
        // this call.
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
            } else {
                // The gate refused the move: drop the one-shot `teleport_to`
                // the body wrote for it so it cannot shape a later move.
                let mut w = cx.world_copy();
                w.turn.plan.teleport_to = -1;
                cx.swap_world(w);
            }
            return Ok(allowed as i32);
        }
        // A position write with no settle: the gate and the write land on
        // the **live** world here (not on the run's copy), so a `card_move`
        // that follows in the same body starts at the destination -- and
        // the replay skips the call (the answer below), so it cannot
        // re-teleport over a move the engine already ran.
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
            player_id,
            agent: _,
            tile,
            kind,
        } => {
            // The chosen branch of the agent offer (the post-pick commit):
            // buy the unowned tile, or build one level on an own one. `kind`
            // names the branch the body saw (0 = buy, 1 = build); the engine
            // re-checks the tile's current state so a commit-pass re-run of
            // the body -- whose options list the just-committed effect already
            // reshaped -- cannot double-act (the answer is in the log, so this
            // arm does not re-execute).
            let p = player_id.max(0) as usize;
            let t = tile.max(0) as usize;
            match kind {
                1 => {
                    cx.card_build(p, t)?;
                }
                _ => {
                    cx.card_buy(p, t, 1)?; // `BuyKind::Agent`
                }
            }
            return Ok(1);
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
                t.roll_source = game_core::engine::rules::RollSource::from_i32(source);
            })?;
            return Ok(t.value.max(0));
        }
        // The card announces an acquisition it performed outside the buy
        // routine (tomoe_savior). The `bought` hook chain runs over the
        // field; `by_card` is the run's player.
        HostRequest::RaiseBought { player_id, tile } => {
            cx.card_raise_bought(
                player_id.max(0) as usize,
                tile.max(0) as usize,
                call.player_id().max(0) as usize,
            )?;
            return Ok(1);
        }
        // Guest-raised trigger points (`ctx::raise`): `circleAffected` from
        // `tile:circle` and kin. The kind is the wire name
        // (`TriggerKind::as_str`); the link settles unless a counteraction
        // cancelled it.
        HostRequest::Raise {
            player_id,
            kind,
            value,
        } => {
            let kind = card_sdk::abi::TriggerKind::from_i32(kind).as_str();
            let t = self.raise_core(cx, kind, player_id, |t| {
                t.value = value;
            })?;
            return Ok((!t.is_cancelled()) as i32);
        }
        // Which option the player's AI would take among agent-offer tiles.
        HostRequest::AiAgentChoice { player_id, tiles } => {
            let tiles: Vec<usize> = tiles
                .into_iter()
                .filter(|&t| t >= 0)
                .map(|t| t as usize)
                .collect();
            let pick = cx.ai_agent_choice(player_id.max(0) as usize, &tiles);
            return Ok(pick);
        }
        // `ctx::gain_typed`: a bank print through the money pipeline as
        // `Pay::new(_, "gain")` (so the 「支付」 scalars do not reach a print),
        // carrying the Pay's event `typ` and a replacement log line.
        HostRequest::GainTyped {
            player_id,
            amount,
            typ,
            text,
        } => {
            let mut p = game_core::engine::Pay::new(amount, game_core::engine::PayKind::Gain);
            p.to = Some(player_id.max(0) as usize);
            // `Pay::typ` is a [`game_core::engine::PayEvent`]; the guest's
            // event-type key is the short fixed vocabulary the wire spelling
            // names (`"pass"` / `"lose"` / `"gain"` / `"pay"` / `"rent"`).
            p.typ = Some(match typ.as_str() {
                "pass" => game_core::engine::PayEvent::Pass,
                "lose" => game_core::engine::PayEvent::Lose,
                "pay" => game_core::engine::PayEvent::Pay,
                "rent" => game_core::engine::PayEvent::Rent,
                _ => game_core::engine::PayEvent::Gain,
            });
            p.text = text;
            p.by_card = Some(call.player_id());
            let paid = cx.money(p)?;
            return Ok(paid.moved());
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
                t.card = Some(name.clone());
            })?;
            return Ok(if t.is_cancelled() { 0 } else { 1 });
        }
        // Run the entire draw now, including immediate empty-deck maintenance
        // and per-card hooks. A later card/event prompt must see the new deck.
        // The negative answer records an already-applied draw so guest replay
        // reports the count without moving those cards a second time.
        HostRequest::Draw { player_id, n } => {
            let Ok(player) = usize::try_from(player_id) else {
                return Ok(-1);
            };
            let got = cx.draw_cards_with_hooks(player, n.max(0) as usize, true)?;
            return Ok(-got - 1);
        }
        HostRequest::Pay {
            from,
            to,
            amount: asked,
            src,
            total_stage,
        } => {
            let by = Some(call.player_id());
            // A player immune to others' effects (`ImmuneAll`) is neither
            // charged nor paid by another player's card -- the payment
            // simply does not happen.
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
            let mut p = game_core::engine::Pay::new(asked, game_core::engine::PayKind::Card);
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
    ///
    /// Write-through logs and host-effect events already sit on the live
    /// stream in code order; the guest copy carries only state writes, so this
    /// adopts the live tail before the swap. Then it completes the activation
    /// entry with its outcome (「无事发生」 when the body changed nothing).
    fn commit_after(
        &self,
        cx: &mut Cx,
        after: Run,
        call: Call,
        card_id: &str,
        trigger: &mut Trigger,
    ) -> Flow<i32> {
            let mut after = after;
            let dest = after.dest;
            *trigger = after.trigger.clone();
            // Keep the live event tail (write-through order) over the guest
            // copy's stale one. The guest copy is the state. The pending walk
            // also stays on the live world -- swapping in a stale `WalkFlush`
            // would re-publish the approach from its origin after a mid-walk
            // flash (the walk-before-flash invariant).
            after.world.recent = cx.world().recent.clone();
            after.world.next_event = after.world.next_event.max(cx.world().next_event);
            after.world.walk_flush = cx.world().walk_flush.clone();
            let activation_id = after.parent;
            // Complete the activation: 「无事发生」 when nothing followed.
            if activation_id >= 0 {
                let children = cx.child_event_count(activation_id, activation_id);
                let results = if children == 0 {
                    vec![Msg::new("log.effect_nothing")]
                } else {
                    Vec::new()
                };
                cx.set_event_results(activation_id, results);
                after.world.recent = cx.world().recent.clone();
                cx.activation.retain(|&id| id != activation_id);
            }
            cx.swap_shared(after.world);
            // Empty-deck maintenance precedes all settlement after-hooks.
            for player_id in after.reshuffle_log {
                self.raise_core(cx, "reshuffled", player_id, |_| {})?;
            }
            // What the effect did, raised now that it has committed --
            // the same points `money()` / `discard()` raise (PayAfter +
            // `paid`, and `discarded`).
            let by = Some(call.player_id());
            for (from, to, amount) in after.paid_log {
                // 资金变动 (rulebook 支付阶段 7) fires on **any** money
                // change, merged into `payAfter` per the doc's
                // 「合并到[支付后]?」 -- a print (game -> player) included.
                // The `paid` [反击] window opens only when money left a
                // player, matching 再次牵起手来 / 游击演出.
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
                self.raise_core(cx, "discarded", player_id, |t| t.card = Some(id))?;
            }
            // The per-draw after points (`drawn` = the drawn card's own
            // hook, `drew` = the field-card per-draw point), one raise
            // per single card.
            for (player_id, id) in after.draw_log {
                self.raise_core(cx, "drawn", player_id, |t| {
                    t.card = Some(id.clone());
                    t.value = 1;
                })?;
                self.raise_core(cx, "drew", player_id, |t| {
                    t.card = Some(id.clone());
                    t.value = 1;
                    t.cards = vec![id];
                })?;
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
            for (owner, card, name, change) in after.counter_log {
                self.raise_core(cx, "counterChanged", owner, |t| {
                    t.card = Some(card);
                    t.name = Some(name);
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
                trigger.card = Some(id.clone());
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
                trigger.card = Some(id.clone());
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

    /// The abnormal-move gate, for an effect a card run wants to apply to
    /// `player_id` (`by` = the card's player): an out player is never hit;
    /// field cards guard (`abnormalGuard`, block with `set_cancelled`); then,
    /// if another player caused it, the `abnormal` [反击] window. What gets
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
        if s >= cx.state().players.len() || cx.world().out(s) {
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

    /// Does a field card make `player_id` untouchable by `by`'s effects?
    /// (`immuneAll` hook, claimed with `set_cancelled`.) Logged when it holds.
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

    /// `by`'s card `card` tries to target player `p`. Answers the player
    /// actually targeted (a `redirect` hook may move a single-target hit), or
    /// -1. Not ported: the per-play `immune<p>` tags and the
    /// `PendingCounteract` / `TargetsChosen` pass.
    fn target_player(&self, cx: &mut Cx, p: i32, by: i32, card: &str, single: bool) -> Flow<i32> {
        let Ok(s) = usize::try_from(p) else {
            return Ok(-1);
        };
        if s >= cx.state().players.len() || cx.world().out(s) {
            return Ok(-1);
        }
        // Per-pair cancel (「取消其对目标之一的[指定]」,
        // `play.Tags["immune"+seat]`): this designation was cancelled; the
        // rest still land.
        if cx.world().turn.cancelled_designations.contains(&p) {
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
                t.card = Some(card.to_string());
                t.by_card = Some(by);
            })?;
            let to = r.target;
            let live = usize::try_from(to)
                .is_ok_and(|x| x < cx.state().players.len() && !cx.world().out(x));
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

    /// `by`'s card targets `tile`; another player's tile targets its owner
    /// too. Answers the tile, or -1.
    ///
    /// A tile marked [`card_sdk::abi::mark::NO_TARGET`] cannot be named at all --
    /// that is 「有标记时此地块不能被指定」. Not ported: the per-play `immune<p>` tags.
    fn target_tile(&self, cx: &mut Cx, tile: i32, by: i32, card: &str) -> Flow<i32> {
        let no_target = game_core::state::MarkFilter {
            kind: card_sdk::abi::mark::NO_TARGET,
            category: "",
            owner: None,
            src: None,
            instance: None,
        };
        if cx.world_copy().count_marks(tile, &no_target) > 0 {
            return Ok(-1);
        }
        let Some(&owner) = usize::try_from(tile)
            .ok()
            .and_then(|t| cx.state().owners.get(t))
        else {
            return Ok(-1);
        };
        let live = usize::try_from(owner).is_ok_and(|o| !cx.world().out(o));
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
            t.card = Some(card.to_string());
            t.by_card = Some(by);
        })?;
        let declared = self.raise_core(cx, "effect", by, |t| {
            t.target = p;
            t.tile = tile;
            t.card = Some(card.to_string());
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
            t.card = Some(card.to_string());
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

    /// Apply a field effect's `Dest` to the instance its run was for -- the
    /// unplace-and-send fate `set_dest` names (discard / hand / gone).
    /// `to` is whose pile it lands in; `None` is the owner it leaves, the
    /// target `set_transfer_to_dest` names.
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
        let mut refilled = false;
        if dest == DEST_GRAVEYARD {
            refilled = w.to_discard(who, card);
        } else if dest == 1 {
            // Back to the hand (手牌).
            w.add_to_hand(who, card);
        }
        // Banished (「[移除]」) is just "gone" -- it left above and goes nowhere.
        cx.swap_world(w);
        if refilled {
            self.raise_core(cx, "reshuffled", who, |_| {})?;
        }
        if dest == DEST_GRAVEYARD {
            self.raise_core(cx, "discarded", who, |t| t.card = Some(card.to_string()))?;
        } else if dest != 1 {
            cx.log(
                left,
                Msg::new("log.card_removed").card("card", card.to_string()),
            );
        }
        Ok(())
    }

    /// The [反击] hand window: one **round per timing**, per rulebook clauses
    /// 32 and 89.
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
    /// order from there. Out / exiled / stunned / no-hand players are
    /// skipped (`can_counteract_now`) -- a bot seat is **not** skipped; it is
    /// offered and answers through `Cx::fill_ai` -- and a seat with no eligible card is skipped
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

        // Valid-option-first (docs/GUARDS.md §4.5): open a [反击] window only
        // when someone can respond. Cheap static index first (the per-card kind
        // bitmask + seat eligibility), then the compiled conditions against a
        // window context built straight from the live world -- no `Run`, no
        // world copy, and the CEL scope only if a candidate actually declares a
        // condition. Nothing qualifies => skip the whole ring: no chain clone,
        // no `Priority`, no per-visit offers. That is every raise nobody
        // answers, and one quiet lap today would close the round identically.
        if !counteract_slow_path() && !self.any_counter_candidate(cx, trigger) {
            #[cfg(feature = "bot-cost")]
            crate::host::bot_cost::COUNTERACT_WINDOWS_SKIPPED
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }
        #[cfg(feature = "bot-cost")]
        crate::host::bot_cost::COUNTERACT_WINDOWS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // The answer tree. Index 0 is L1, the effect declaration; every other
        // node is one declared counter.
        let mut chain = vec![ChainLink {
            seat: chain_starter(t, cx.state().turn, n),
            idx: -1,
            id: t.card.clone().unwrap_or_default(),
            uid: -1,
            move_extension: 0,
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
        // One processed set per window (this ring, across its laps): a later
        // lap reuses a probe's verdict when the inputs it reads cannot have
        // changed (docs/GUARDS.md §4.5).
        let mut memo = ProbeMemo::new();
        'visits: while *budget > 0 {
            let cursor = priority.seat();
            // Ruling 2026-10-07: the visit keeps the floor until this seat
            // passes (the offer's `ask.counteract.skip`) or holds no eligible
            // counteraction left -- every declaration re-offers the same seat.
            // Each offer is its own prompt, so the AI answer (chaos re-rolls
            // its counter chance per offer) is recomputed per offer in
            // `Cx::ask` / `fill_ai`.
            let mut declared_any = false;
            if can_counteract_now(cx, cursor)
                || self.has_field_counter(cx, cursor, &chain[timing].link)
            {
                for _ in 0..MAX_COUNTERACT_PER_VISIT {
                    if *budget == 0 {
                        break 'visits;
                    }
                    *budget -= 1;
                    // One source per shared group, reserved at declaration (before
                    // any of the bodies settle). Duplicate cards cannot re-offer it.
                    let groups: Vec<i32> = chain[timing]
                        .answers
                        .iter()
                        .filter(|&&at| chain[at].seat == cursor)
                        .filter_map(|&at| {
                            self.ruleset.cards()[chain[at].idx as usize]
                                .props
                                .get(card_sdk::abi::prop::COUNTERACT_GROUP)
                                .copied()
                        })
                        .collect();
                    let Some((id, idx, uid, move_extension)) =
                        self.declare_one(cx, cursor, &chain[timing].link, &mut memo, &groups)?
                    else {
                        // Explicit pass, or nothing eligible left: the visit
                        // ends and priority advances.
                        break;
                    };
                    // The declaration leaves the hand now.
                    if uid < 0 {
                        let mut w = cx.world_copy();
                        if let Some(pos) = w.hidden[cursor].hand.iter().position(|c| c == &id) {
                            w.hidden[cursor].hand.remove(pos);
                        }
                        cx.swap_world(w);
                    }
                    // A hand changed: hand-sensitive probe verdicts and the
                    // scope's `_hand` table are stale from here.
                    memo.on_declaration();
                    let mut link =
                        CoreTrigger::new(if uid >= 0 { "skillUsed" } else { "card" }, cursor);
                    if uid >= 0 {
                        link.cards = vec![id.clone()];
                    }
                    link.card = Some(id.clone());
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
                        uid,
                        move_extension,
                        answered: timing,
                        link: bridge_trigger(&link),
                        answers: Vec::new(),
                    });
                    #[cfg(feature = "bot-cost")]
                    crate::host::bot_cost::COUNTERACT_DECLARED
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
        let uid = chain[idx].uid;
        if uid < 0 {
            cx.log(
                seat as i32,
                Msg::new("log.play_counteract")
                    .player_id("who", seat as i32)
                    .card("card", id.clone()),
            );
        }
        let dest = if negated {
            // The body does not run, so the card has no fate of its own: it was
            // played (it left the hand at declaration) and is spent. It still
            // flashes -- marked 无效 -- so the negation is visible.
            let link = &chain[answered].link;
            cx.card_activated(
                game_core::state::card_trigger::COUNTER,
                seat as i32,
                &id,
                link.player_id,
                link.tile,
                true,
            );
            DEST_UNSET
        } else {
            // The counter's body runs against the link it answers, so its
            // `set_cancelled` / `negate_effect` / `spare` land there -- and the
            // effect settles only after every counter has had its say.
            let mut on_link = chain[answered].link.clone();
            if chain[idx].move_extension > 0 {
                // The extension is a move-tag rewrite; it needs a move payload.
                if let Some(m) = &mut on_link.mv {
                    m.tags.push((
                        card_sdk::abi::COUNTERACT_MOVE_EXTENSION.to_string(),
                        chain[idx].move_extension,
                    ));
                }
            }
            let dest = self.drive(
                cx,
                Call::Counteract {
                    card,
                    player_id: seat as i32,
                },
                &id,
                uid,
                &mut on_link,
            )?;
            if let Some(m) = &mut on_link.mv {
                m.tags.retain(|(key, _)| key != card_sdk::abi::COUNTERACT_MOVE_EXTENSION);
            }
            chain[answered].link = on_link;
            dest
        };
        // A field skill remains in place, even when its activation is negated.
        if uid >= 0 {
            return Ok(());
        }
        let mut w = cx.world_copy();
        let spent = matches!(dest_from(dest), Dest::Graveyard) && !w.out(seat);
        let mut refilled = false;
        match dest_from(dest) {
            Dest::Graveyard => {
                if !w.out(seat) {
                    w.hidden[seat].discard.push(id.clone());
                    refilled = w.refill_draw_pile(seat);
                }
            }
            Dest::Hand => w.hidden[seat].hand.push(id.clone()),
            Dest::Banished => {}
            Dest::Field => {}
        }
        cx.swap_world(w);
        if refilled {
            self.raise_core(cx, "reshuffled", seat as i32, |_| {})?;
        }
        if spent {
            self.raise_core(cx, "discarded", seat as i32, |t| t.card = Some(id.clone()))?;
        }
        Ok(())
    }

    /// The throwaway `Run` a residual wasm guard probe fires up: a shared
    /// handle to the live world (fix B -- the probe is a pure check; its body
    /// copies only if it writes), the answered link as the trigger. Built
    /// **only** for a candidate whose condition already admitted and whose
    /// entry still carries a guard (G4 deleted the rest).
    fn probe_run(&self, cx: &Cx, top: &Trigger, card: &str) -> Run {
        Run {
            world: cx.share_world(),
            pile_checkpoints: Default::default(),
            data: self.data.clone(),
            props: self.modules_props(),
            trigger: top.clone(),
            current_card: card.to_string(),
            current_uid: -1,
            dest: DEST_UNSET,
            dest_to: None,
            paid_log: vec![],
            discard_log: vec![],
            draw_log: vec![],
            reshuffle_log: vec![],
            fire_spent_log: vec![],
            house_log: vec![],
            counter_log: vec![],
            exile_log: vec![],
            doubled: -1,
            linger_props: Default::default(),
            wrap_effect: false,
            live: None,
            posted: vec![],
            log_idx: 0,
            parent: -1,
        }
    }

    /// Only sources explicitly opting into the shared declaration menu.
    fn has_field_counter(&self, cx: &Cx, seat: usize, top: &Trigger) -> bool {
        !cx.world().out(seat)
            && cx
                .world()
                .field_instances(seat as i32)
                .iter()
                .any(|(_, id)| {
                    let Some(idx) = self.ruleset.card(id) else {
                        return false;
                    };
                    self.ruleset.counteracts_to(idx, top.kind)
                        && self.ruleset.cards()[idx as usize]
                            .props
                            .get(card_sdk::abi::prop::COUNTERACT_FROM_FIELD)
                            .copied()
                            .unwrap_or(0)
                            != 0
                        && !cx.world().card_face_down(seat as i32, id)
                })
    }

    /// Valid-option-first pre-scan (docs/GUARDS.md §4.5): could **any** seat
    /// respond to `top` at all? Cheap static index first -- the per-card kind
    /// bitmask and seat eligibility -- then the compiled conditions against a
    /// window context built straight from the live world. The residual wasm
    /// guards are not run here: a condition survivor is enough to open the
    /// ring, and a card the guard would reject simply contributes no option
    /// (exactly as today's quiet lap).
    ///
    /// When this returns `false`, `hand_counteractions` skips the whole
    /// window -- no chain clone, no `Priority`, no per-visit offers, no world
    /// copies. During the scan nothing mutates the world, so one shared CEL
    /// scope answers every condition (and is only built if some candidate
    /// declares one).
    fn any_counter_candidate(&self, cx: &Cx, top: &Trigger) -> bool {
        let n = cx.state().players.len();
        let live = LiveSnap {
            world: cx.world(),
            data: &self.data,
            trigger: top,
            cand: None,
        };
        let mut scope: Option<WindowScope> = None;
        for s in 0..n {
            if self.has_field_counter(cx, s, top) {
                return true;
            }
            if !can_counteract_now(cx, s) {
                continue;
            }
            let mut seen: Vec<&str> = Vec::new();
            for id in hand_of(cx, s) {
                if seen.contains(&id.as_str()) {
                    continue;
                }
                seen.push(id.as_str());
                let Some(idx) = self.ruleset.card(id) else {
                    continue;
                };
                // Cached counteraction index (BOT-RESEARCH.md #1): one shift of
                // the per-card kind bitmask -- no entry-table scan, no `Run`.
                if !self.ruleset.counteracts_to(idx, top.kind) {
                    continue;
                }
                let Some(entry) = self.ruleset.cards()[idx as usize]
                    .entry(card_sdk::abi::OnKind::Counteract, Some(top.kind))
                else {
                    continue;
                };
                let Some(pre) = self.ruleset.pre(idx, entry) else {
                    // No condition: the card always offers (subject to the
                    // guard, which the ring itself asks).
                    return true;
                };
                let cand = crate::cond_pre::fill_candidate(&live, s as i32, id, false);
                let sc = scope.get_or_insert_with(|| {
                    crate::cond_pre::window_scope(&crate::cond_pre::fill_window(&live))
                });
                if crate::cond_pre::condition_allows(Some(pre), Some(sc), &cand) {
                    return true;
                }
            }
        }
        false
    }

    /// Offer one player a [反击] window answering `top`. Returns the card they
    /// declared, if any. The prompt describes the **answered** link, so a
    /// player answering a counter is told which counter they are answering.
    fn declare_one(
        &self,
        cx: &mut Cx,
        s: usize,
        top: &Trigger,
        memo: &mut ProbeMemo,
        groups: &[i32],
    ) -> Flow<Option<(String, i32, i32, i32)>> {
        // Hand cards that answer this link, deduped to one entry per id.
        // Ordered by `effect_order_key` (Q5): group `Hand`, source = the card's
        // index in the hand `Vec` (the authoritative state list), decl = 0 (one
        // entry per card after the dedupe). The sort is stable and the source
        // component *is* today's hand order, so the offer list is unchanged --
        // the key just makes the guarantee explicit.
        //
        // Stage 1 is the cheap static index (BOT-RESEARCH.md #1): the per-card
        // kind bitmask, no world copy, no scope, no `Run`. A hand with nothing
        // that answers this kind returns before any per-window machinery
        // exists -- the common case for most seats of most raises.
        let mut candidates: Vec<((u8, u32, u32), String, i32, i32)> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for (hand_pos, id) in hand_of(cx, s)
            .iter()
            .enumerate()
            .filter(|_| can_counteract_now(cx, s))
        {
            if seen.contains(id) {
                continue;
            }
            seen.push(id.clone());
            let Some(idx) = self.ruleset.card(id) else {
                continue;
            };
            if !self.ruleset.counteracts_to(idx, top.kind) {
                continue;
            }
            candidates.push((
                effect_order_key(LookupGroup::Hand, hand_pos as u32, 0),
                id.clone(),
                idx,
                -1,
            ));
        }
        if !cx.world().out(s) {
            for (field_pos, (uid, id)) in
                cx.world().field_instances(s as i32).into_iter().enumerate()
            {
                let Some(idx) = self.ruleset.card(&id) else {
                    continue;
                };
                if self.ruleset.cards()[idx as usize]
                    .props
                    .get(card_sdk::abi::prop::COUNTERACT_FROM_FIELD)
                    .copied()
                    .unwrap_or(0)
                    == 0
                    || !self.ruleset.counteracts_to(idx, top.kind)
                    || cx.world().card_face_down(s as i32, &id)
                {
                    continue;
                }
                candidates.push((
                    effect_order_key(source_group(cx.world(), uid), field_pos as u32, 0),
                    id,
                    idx,
                    uid,
                ));
            }
        }
        candidates.retain(|(_, _, idx, _)| {
            let group = self.ruleset.cards()[*idx as usize]
                .props
                .get(card_sdk::abi::prop::COUNTERACT_GROUP)
                .copied()
                .unwrap_or(0);
            group == 0 || !groups.contains(&group)
        });
        if candidates.is_empty() {
            return Ok(None);
        }

        // `BGD_COUNTERACT_SLOW=1` reproduces the pre-optimisation shape
        // exactly (docs/GUARDS.md §4.5): a world copy + `Run` per offer for the
        // window context, and a `Run` per condition survivor for the guard --
        // the `examples/ckpt_equiv.rs` A/B's this against the fast path.
        if counteract_slow_path() {
            let win_run = Run {
                world: cx.share_world(),
                pile_checkpoints: Default::default(),
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
                counter_log: vec![],
                exile_log: vec![],
                doubled: -1,
                linger_props: Default::default(),
                wrap_effect: false,
                live: None,
                posted: vec![],
                log_idx: 0,
                parent: -1,
            };
            let win_scope = crate::cond_pre::window_scope(&crate::cond_pre::fill_window(&win_run));
            let mut options: Vec<(String, i32, i32)> = Vec::new();
            let mut order: Vec<(u8, u32, u32)> = Vec::new();
            for (key, id, idx, uid) in candidates {
                if self
                    .ruleset
                    .counteract_pre_allows(&win_run, idx, s as i32, &win_scope)
                    .is_none()
                {
                    continue;
                }
                let mut run = self.probe_run(cx, top, &id);
                run.current_uid = uid;
                if self
                    .ruleset
                    .can_counteract_scoped(&run, idx, s as i32, &win_scope)
                    .unwrap_or(false)
                {
                    order.push(key);
                    options.push((id, idx, uid));
                }
            }
            return self.finish_offer(cx, s, top, order, options);
        }

        // Stage 2: the compiled conditions against a window context built
        // straight from the live world (`LiveSnap`, no clone). The CEL scope is
        // built lazily and only if some candidate actually declares a condition
        // (docs/GUARDS.md §4.4 item 1); it is shared across the whole ring via
        // `memo` and only rebuilt when a probe reads `hand(p)` after a
        // declaration.
        let live = LiveSnap {
            world: cx.world(),
            data: &self.data,
            trigger: top,
            cand: None,
        };
        let mut options: Vec<(String, i32, i32)> = Vec::new();
        let mut order: Vec<(u8, u32, u32)> = Vec::new();
        for (key, id, idx, uid) in candidates {
            #[cfg(feature = "bot-cost")]
            crate::host::bot_cost::COUNTERACT_PROBES
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let Some(entry) = self.ruleset.cards()[idx as usize]
                .entry(card_sdk::abi::OnKind::Counteract, Some(top.kind))
            else {
                continue;
            };
            let pre = self.ruleset.pre(idx, entry);
            let guard_is_none = self.ruleset.entry_guard_is_none(idx, entry);
            // A verdict is hand-sensitive when the residual wasm guard may read
            // hands (it can read anything) or the condition names a hand field.
            // Only those are stamped with `hand_gen`; the rest are stable for
            // the whole window (a ring's only world change is a declaration).
            let hand_sensitive =
                !guard_is_none || pre.is_some_and(|p| crate::cond_pre::cond_reads_hand(&p.cond));
            if let Some(&(gen, eligible, hs)) = memo.verdicts.get(&(s, id.clone())) {
                if !counteract_slow_path() && (!hs || gen == memo.hand_gen) {
                    #[cfg(feature = "bot-cost")]
                    crate::host::bot_cost::COUNTERACT_PROBE_MEMO_HITS
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if eligible {
                        order.push(key);
                        options.push((id, idx, uid));
                    }
                    continue;
                }
            }
            let eligible = match pre {
                None => {
                    if guard_is_none {
                        // No condition, no guard: always answers this kind.
                        true
                    } else {
                        // No condition but a residual guard: the only path that
                        // still needs a `Run` (the guard's wasm store).
                        let mut run = self.probe_run(cx, top, &id);
                        run.current_uid = uid;
                        let scope = memo.scope_for(&live, true);
                        self.ruleset
                            .can_counteract_scoped(&run, idx, s as i32, scope)
                            .unwrap_or(false)
                    }
                }
                Some(pre) => {
                    let need_fresh = crate::cond_pre::cond_reads_window_hand(&pre.cond);
                    let scope = memo.scope_for(&live, need_fresh);
                    let cand = crate::cond_pre::fill_candidate(&live, s as i32, &id, false);
                    if !crate::cond_pre::condition_allows(Some(pre), Some(scope), &cand) {
                        false
                    } else if guard_is_none {
                        // G4: the condition alone decides -- no `Run`, no world
                        // copy, no wasm instantiation.
                        true
                    } else {
                        let mut run = self.probe_run(cx, top, &id);
                        run.current_uid = uid;
                        self.ruleset
                            .can_counteract_scoped(&run, idx, s as i32, scope)
                            .unwrap_or(false)
                    }
                }
            };
            memo.verdicts
                .insert((s, id.clone()), (memo.hand_gen, eligible, hand_sensitive));
            if eligible {
                order.push(key);
                options.push((id, idx, uid));
            }
        }
        self.finish_offer(cx, s, top, order, options)
    }

    /// The prompt half of [`Self::declare_one`]: order the offer list by the
    /// Q5 key, show the [反击] prompt (options + 「不打」), and return the card
    /// the seat declared (if any).
    fn finish_offer(
        &self,
        cx: &mut Cx,
        s: usize,
        top: &Trigger,
        order: Vec<(u8, u32, u32)>,
        options: Vec<(String, i32, i32)>,
    ) -> Flow<Option<(String, i32, i32, i32)>> {
        if options.is_empty() {
            return Ok(None);
        }
        // Q5: order the offer list by the explicit key. Stable, and the key's
        // source component is the hand position, so this is today's order.
        let mut keyed: Vec<((u8, u32, u32), (String, i32, i32))> =
            order.into_iter().zip(options).collect();
        keyed.sort_by_key(|(k, _)| *k);
        let mut options: Vec<(String, i32, i32)> = keyed.into_iter().map(|(_, o)| o).collect();
        let shared_move = top.kind == TriggerKind::MoveBefore
            && options.iter().any(|(_, idx, _)| {
                self.ruleset.cards()[*idx as usize]
                    .props
                    .get(card_sdk::abi::prop::COUNTERACT_GROUP)
                    .copied()
                    .unwrap_or(0)
                    > 0
            });
        if shared_move {
            // Payment order: dedicated card, fire pots, then back.
            options.sort_by_key(|(_, _, uid)| *uid >= 0);
        }
        // Labels "打出「...」" + "不打"; the hint is the first CounteractHint or
        // the trigger's description. CounteractHint is not in the ABI yet (TODO).
        let mut labels: Vec<Msg> = options
            .iter()
            .map(|(id, idx, uid)| {
                if *uid >= 0 {
                    let cost = self.ruleset.cards()[*idx as usize]
                        .props
                        .get(card_sdk::abi::prop::COUNTERACT_FIRE_COST)
                        .copied()
                        .unwrap_or(0);
                    Msg::new("ask.counteract.skill")
                        .card("card", id.clone())
                        .i("n", cost as i64)
                } else {
                    Msg::new("ask.counteract.play").card("card", id.clone())
                }
            })
            .collect();
        if shared_move {
            let landing = |extra: i32| {
                let total = top.mv.as_ref().map(|m| m.total).unwrap_or(0);
                let dir = top.mv.as_ref().map(|m| m.dir.as_i32()).unwrap_or(1);
                (top.tile + (total + extra) * dir).rem_euclid(self.data.tiles.len() as i32)
            };
            loop {
                // No declaration or resource change until the payment is confirmed.
                // Keep the source metadata for the existing bot counteraction policy.
                let distances = vec![
                    Msg::new("ask.counteract.extend")
                        .i("n", 2)
                        .tile("tile", landing(2))
                        .card("card", options[0].0.clone()),
                    Msg::new("ask.counteract.extend")
                        .i("n", 1)
                        .tile("tile", landing(1))
                        .card("card", options[0].0.clone()),
                    Msg::new("ask.counteract.skip"),
                ];
                let ask = Ask::choice(
                    vec![s],
                    Msg::new("ask.counteract.title"),
                    Msg::new("ask.counteract.move_extension")
                        .i("n", top.mv.as_ref().map(|m| m.total).unwrap_or(0) as i64)
                        .tile("tile", landing(0)),
                    distances,
                    2,
                    12.0,
                );
                let reply = cx.ask(ask)?;
                let pick = reply
                    .a
                    .answers
                    .first()
                    .copied()
                    .filter(|&x| x >= 0)
                    .unwrap_or(reply.fallback);
                let extension = match pick {
                    0 => 2,
                    1 => 1,
                    _ => return Ok(None),
                };
                let mut payments = labels.clone();
                payments.push(Msg::new("ask.counteract.back"));
                let ask = Ask::choice(
                    vec![s],
                    Msg::new("ask.counteract.title"),
                    Msg::new("ask.counteract.move_extension_payment")
                        .i("n", extension as i64)
                        .tile("tile", landing(extension)),
                    payments,
                    options.len() as i32,
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
                if let Some((id, idx, uid)) = options.get(pick) {
                    return Ok(Some((id.clone(), *idx, *uid, extension)));
                }
                // Back (also the payment timeout) reopens the distance choice.
            }
        }
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
        Ok(options
            .get(pick)
            .map(|(id, idx, uid)| (id.clone(), *idx, *uid, 0)))
    }
}

/// The module's view of an engine trigger (the bridge `Trigger`). The move
/// payload's `roll` is the guest-visible `t.Move.Roll`, which a counteraction
/// may rewrite; the engine reads it back.
fn bridge_trigger(t: &CoreTrigger) -> Trigger {
    let is_roll_kind = matches!(
        trigger_kind(t.kind),
        TriggerKind::MoveRoll | TriggerKind::Roll | TriggerKind::RollAfter
    );
    // Only a move trigger carries a roll; `paid`/`settle` carry value as an
    // amount, which must not read as a phantom move. `RollAfter` is the
    // post-roll hook and does carry the face (「若移动掷骰出目为16及以上」).
    // A card-driven `roll` raise has no move payload (`mv: None`); its face
    // stays on `value` and the writeback reads it from there.
    let mv = t.mv.as_ref().map(|m| TriggerMove {
        kind: match m.kind {
            game_core::engine::MoveKind::Walk => card_sdk::abi::MoveKind::Walk,
            game_core::engine::MoveKind::Teleport => card_sdk::abi::MoveKind::Teleport,
        },
        resolve: m.resolve,
        tags: m.tags.clone(),
        // Mirror of `Trigger::main`.
        main: t.main,
        dir: m.dir,
        from: m.from,
        remaining: m.remaining,
        total: m.total,
        roll: is_roll_kind.then_some(t.value),
    });
    let buy = t.buy.map(|b| TriggerBuy {
        kind: card_sdk::abi::BuyKind::from_i32(b.kind.as_i32())
            .unwrap_or(card_sdk::abi::BuyKind::Land),
        seller: b.seller,
        price: b.price,
        deal_owner: b.deal_owner,
        deal_houses: b.deal_houses,
        deal_mortgaged: b.deal_mortgaged,
    });
    Trigger {
        kind: trigger_kind(t.kind),
        player_id: t.player_id,
        target: t.target,
        tile: t.tile,
        value: t.value,
        step: t.step,
        by_card: t.by_card,
        main: t.main,
        mv,
        pay: t.pay.map(|p| TriggerPay { is_rent: p.is_rent }),
        buy,
        negation: t.negation,
        spared: t.spared.clone(),
        seq: t.seq,
        answers: t.answers,
        effects: t.effects.clone(),
        cards: t.cards.clone(),
        card: t.card.clone(),
        name: t.name.clone(),
        roll_source: t.roll_source,
        reason: t.reason.clone(),
    }
}

/// Map a raw prompt answer to what the answer log should carry.
/// `PromptKind::TileId` answers with the **tile id** (`Ask::tile`'s
/// `items[i]`), or `-1` for the 「不选」 fallback -- stable across the
/// commit-pass re-run of a body whose options the just-committed host
/// effect reshaped. Every other kind keeps the raw index.
fn recorded_answer(kind: PromptKind, items: &[String], v: i32) -> i32 {
    if kind != PromptKind::TileId {
        return v;
    }
    if v < 0 {
        return -1;
    }
    items
        .get(v as usize)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(-1)
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
        prices,
        ai_hint,
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
                    PromptOption::Tile { label, .. } => label,
                })
                .collect();
            Ask::choice(players, title, text, labels, 0, 15.0)
        }
        PromptKind::Tile | PromptKind::TileId => {
            // Bare-Int options keep the auto `ask.tileOption` labels so
            // existing `ask_tile` callers are unchanged; `PromptOption::Tile`
            // carries its own label (`ask_tiles` / `opt_tile`).
            let mut tiles: Vec<usize> = Vec::new();
            let mut labels: Vec<Msg> = Vec::new();
            for o in options {
                match o {
                    PromptOption::Int(i) => {
                        tiles.push(i as usize);
                        labels.push(Msg::new("ask.tileOption").tile("tile", i));
                    }
                    PromptOption::Str(_) => {
                        tiles.push(0);
                        labels.push(Msg::new("ask.tileOption").tile("tile", 0));
                    }
                    PromptOption::Tile { tile, label } => {
                        tiles.push(tile as usize);
                        labels.push(label);
                    }
                }
            }
            let mut ask = Ask::tile(player_id as usize, title, text, &tiles, labels);
            ask.view.prices = prices;
            if ai_hint >= 0 {
                ask = ask.with_ai(|_| ai_hint);
            }
            ask
        }
        PromptKind::Card => {
            // Pick one card out of a list; the options are ready-made labels.
            let labels = options
                .into_iter()
                .map(|o| match o {
                    PromptOption::Str(m) => m,
                    PromptOption::Int(i) => Msg::new("ask.intOption").i("n", i),
                    PromptOption::Tile { label, .. } => label,
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
                    PromptOption::Tile { tile, .. } => Msg::new("ask.player").player_id("who", *tile),
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
    /// Field instance being activated; -1 for a hand card or the root timing.
    uid: i32,
    /// Extension chosen before declaring this movement source; zero otherwise.
    move_extension: i32,
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

/// Test seam for the [反击] window fast path (docs/GUARDS.md §4.5). When the
/// environment variable `BGD_COUNTERACT_SLOW=1` is set, the valid-option-first
/// pre-scan never skips a window and the per-window processed set is bypassed,
/// reproducing the pre-optimisation control flow. The two paths must produce
/// identical `Match::save()` checkpoints -- `examples/ckpt_equiv.rs` A/B's them.
fn counteract_slow_path() -> bool {
    static SLOW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SLOW.get_or_init(|| std::env::var("BGD_COUNTERACT_SLOW").is_ok_and(|v| v == "1"))
}

/// May this seat declare a counteraction right now? Out / exiled players
/// cannot. (`CannotPlay` and a one-turn mute have no engine field yet.)
fn can_counteract_now(cx: &Cx, s: usize) -> bool {
    let Some(player_id) = cx.state().players.get(s) else {
        return false;
    };
    // Rulebook eligibility only: out / [除外] cannot declare, and `CannotPlay`
    // (stun, 飞鸟山之战's no-hand, Fx.CantPlayHand) blocks it too.
    // `_noCounteractTurn` and Fx.CantPlayHand have no engine field yet (TODO).
    // A bot is still a player, so the window opens for every mentality and
    // the seat answers through `Cx::fill_ai` (standard: `CounterParams`
    // propensity, default never; chaos: `CHAOS_COUNTER_CHANCE`) exactly like
    // a human's offer. Out / exiled remain skipped.
    // `World::out` is `st.players[i].out()`, so the live state answers it --
    // no world copy (this runs on every visit of every ring).
    !player_id.out()
        && player_id.exile() == 0
        && !player_id.stunned()
        && player_id.no_hand() == 0
}

/// The player's private hand (order preserved).
fn hand_of<'a>(cx: &'a Cx, s: usize) -> &'a [String] {
    cx.world()
        .hidden
        .get(s)
        .map(|h| h.hand.as_slice())
        .unwrap_or(&[])
}

/// The live-world snapshot source (docs/GUARDS.md §4.2): reads the engine
/// `World` in place -- **no clone** -- so the counteract pre-filter can build a
/// window / candidate context before any `Run` exists. Only the reads
/// [`crate::cond_pre::fill_window`] / [`crate::cond_pre::fill_candidate`] make.
pub struct LiveSnap<'a> {
    pub world: &'a game_core::engine::World,
    pub data: &'a GameData,
    pub trigger: &'a Trigger,
    /// Candidate being probed, for the [`rules_cond::view::CondView`] impl:
    /// `(owner seat, card id, placed)`. `None` on the window-only form
    /// [`crate::cond_pre::fill_window`] takes.
    pub cand: Option<(i32, &'a str, bool)>,
}

impl crate::cond_pre::SnapSrc for LiveSnap<'_> {
    #[inline]
    fn trigger(&self) -> Trigger {
        self.trigger.clone()
    }
    #[inline]
    fn tile_named(&self, name: &str) -> i32 {
        self.world.tile_named(self.data, name)
    }
    #[inline]
    fn tile_count(&self) -> i32 {
        self.data.tiles.len() as i32
    }
    #[inline]
    fn tile_name(&self, tile: i32) -> String {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.name.replace('\n', ""))
            .unwrap_or_default()
    }
    #[inline]
    #[inline]
    fn is_circle(&self, tile: i32) -> bool {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.kind == game_core::data::TileKind::Circle)
    }
    #[inline]
    fn is_ring(&self, tile: i32) -> bool {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.kind == game_core::data::TileKind::Ring)
    }
    #[inline]
    fn is_live_house(&self, tile: i32) -> bool {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.is_buyable() && t.group == 10)
    }
    #[inline]
    fn is_buyable(&self, tile: i32) -> bool {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.is_buyable())
    }
    fn player_count(&self) -> i32 {
        self.world.st.players.len() as i32
    }
    #[inline]
    fn money(&self, player_id: i32) -> i32 {
        self.world.player_money(player_id)
    }
    #[inline]
    fn fire(&self, player_id: i32) -> i32 {
        self.world.fire(player_id)
    }
    #[inline]
    fn band_crystals(&self, player_id: i32) -> i32 {
        self.world.band_crystals(player_id)
    }
    #[inline]
    fn hand_size(&self, player_id: i32) -> i32 {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.hand.len() as i32)
            .unwrap_or(0)
    }
    #[inline]
    fn player_pos(&self, player_id: i32) -> i32 {
        self.world.player_pos(player_id)
    }
    #[inline]
    fn player_out(&self, player_id: i32) -> i32 {
        self.world.player_out(player_id) as i32
    }
    #[inline]
    fn stay_of(&self, player_id: i32) -> i32 {
        self.world.state_get(player_id, game_core::state::key::STAY)
    }
    #[inline]
    fn stun_of(&self, player_id: i32) -> i32 {
        self.world.state_get(player_id, game_core::state::key::STUN)
    }
    #[inline]
    fn state_get(&self, player_id: i32, key: &str) -> i32 {
        self.world.state_get(player_id, key)
    }
    #[inline]
    fn character_skill_id(&self, player_id: i32) -> Option<String> {
        self.world.character_skill_id(player_id)
    }
    #[inline]
    fn band_skill_id(&self, player_id: i32) -> Option<String> {
        self.world.band_skill_id(player_id)
    }
    #[inline]
    fn character_name(&self, player_id: i32) -> Option<String> {
        self.world
            .st
            .players
            .get(player_id.max(0) as usize)
            .map(|s| s.character.clone())
            .filter(|c| !c.is_empty())
    }
    #[inline]
    fn band_name(&self, player_id: i32) -> Option<String> {
        // Same lookup `CardWorld::in_band` does: the character's band.
        let s = self.world.st.players.get(player_id.max(0) as usize)?;
        self.data
            .characters
            .iter()
            .find(|c| c.name == s.character)
            .map(|c| c.band.clone())
    }
    #[inline]
    fn tok(&self, player_id: i32, name: &str) -> i32 {
        self.world.tok(player_id, name)
    }
    #[inline]
    fn tok_names(&self, player_id: i32, prefix: &str) -> Vec<String> {
        self.world.tok_names(player_id, prefix)
    }
    #[inline]
    fn owned_count(&self, player_id: i32) -> i32 {
        self.world.owned_tiles(player_id).len() as i32
    }
    #[inline]
    fn card_crystals(&self, player_id: i32, card: &str) -> i32 {
        self.world.card_crystals(player_id, card)
    }
    #[inline]
    fn tile_owner(&self, tile: i32) -> i32 {
        self.world.tile_owner(tile)
    }
    #[inline]
    fn houses_of(&self, tile: i32) -> i32 {
        self.world
            .st
            .houses
            .get(tile.max(0) as usize)
            .copied()
            .unwrap_or(0)
    }
    #[inline]
    fn mortgaged_of(&self, tile: i32) -> i32 {
        self.world
            .st
            .mortgaged
            .get(tile.max(0) as usize)
            .copied()
            .unwrap_or(false) as i32
    }
    #[inline]
    fn tile_price(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.price)
            .unwrap_or(0)
    }
    #[inline]
    fn turn_player(&self) -> i32 {
        self.world.st.turn
    }
    #[inline]
    fn turn_key(&self) -> i32 {
        self.world.st.round * 100 + self.world.st.turn + 1
    }
    #[inline]
    fn fixed_roll(&self) -> i32 {
        self.world.turn.fixed_roll.unwrap_or(-1)
    }
    #[inline]
    fn gains_this_turn(&self, player_id: i32) -> i32 {
        self.world.gains_this_turn(player_id)
    }
    #[inline]
    fn targeted_count(&self, player_id: i32) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.world.targeted.get(s).copied())
            .unwrap_or(0)
    }
    #[inline]
    fn card_tile(&self, player_id: i32, card: &str) -> i32 {
        self.world
            .field_instances(player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-2, |(uid, _)| self.world.tile_at(uid))
    }
}

/// `CondView` over the live world (docs/GUARDS.md §4.2b). Every method calls
/// `self.world.*` / `self.data.*` **directly** -- never `self.<SnapSrc::method>`,
/// which would re-enter this trait and overflow the stack.
impl rules_cond::view::CondView for LiveSnap<'_> {
    fn kind(&self) -> i64 {
        self.trigger.kind as i64
    }
    fn actor(&self) -> i64 {
        self.trigger.player_id as i64
    }
    fn target(&self) -> i64 {
        self.trigger.target as i64
    }
    fn tile_id(&self) -> i64 {
        self.trigger.tile as i64
    }
    fn tile_owner(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.world.tile_owner(self.trigger.tile) as i64
        } else {
            -1
        }
    }
    fn tile_houses(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.world
                .st
                .houses
                .get(self.trigger.tile.max(0) as usize)
                .copied()
                .unwrap_or(0) as i64
        } else {
            0
        }
    }
    fn tile_mortgaged(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.world
                .st
                .mortgaged
                .get(self.trigger.tile.max(0) as usize)
                .copied()
                .unwrap_or(false) as i64
        } else {
            0
        }
    }
    fn tile_price(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.data
                .tiles
                .get(self.trigger.tile.max(0) as usize)
                .map(|t| t.price)
                .unwrap_or(0) as i64
        } else {
            0
        }
    }
    fn value(&self) -> i64 {
        self.trigger.value as i64
    }
    fn step(&self) -> i64 {
        self.trigger.step as i64
    }
    fn by(&self) -> i64 {
        self.trigger.by_card.map(|b| b as i64).unwrap_or(-1)
    }
    fn pay_is_rent(&self) -> bool {
        self.trigger.pay.map(|p| p.is_rent).unwrap_or(false)
    }
    fn move_roll(&self) -> i64 {
        self.trigger
            .mv
            .as_ref()
            .and_then(|m| m.roll)
            .filter(|&r| r >= 0)
            .map(|r| r as i64)
            .unwrap_or(-1)
    }
    fn move_kind(&self) -> i64 {
        self.trigger.mv.as_ref().map(|m| m.kind as i64).unwrap_or(-1)
    }
    fn move_remaining(&self) -> i64 {
        self.trigger.mv.as_ref().map(|m| m.remaining).unwrap_or(0) as i64
    }
    fn move_main(&self) -> bool {
        self.trigger.main
    }
    fn move_dir(&self) -> i64 {
        self.trigger
            .mv
            .as_ref()
            .map(|m| m.dir.as_i32() as i64)
            .unwrap_or(1)
    }
    fn move_tag_named(&self, name: &str) -> i64 {
        self.trigger
            .mv
            .as_ref()
            .and_then(|m| m.tags.iter().find(|(k, _)| k == name))
            .map(|(_, v)| *v as i64)
            .unwrap_or(0)
    }
    fn move_tag_table(&self) -> Vec<(String, i64)> {
        self.trigger
            .mv
            .as_ref()
            .map(|m| m.tags.iter().map(|(k, v)| (k.clone(), *v as i64)).collect())
            .unwrap_or_default()
    }
    fn roll_source(&self) -> i64 {
        self.trigger.roll_source.as_i32() as i64
    }
    fn abnormal(&self) -> bool {
        matches!(self.trigger.kind, crate::TriggerKind::Abnormal)
    }
    fn turn_player(&self) -> i64 {
        self.world.st.turn as i64
    }
    fn turn_key(&self) -> i64 {
        (self.world.st.round * 100 + self.world.st.turn + 1) as i64
    }
    fn chain_count(&self) -> i64 {
        self.trigger.effects.len() as i64
    }
    fn chain_kinds(&self) -> Vec<i64> {
        self.trigger
            .effects
            .iter()
            .map(|e| crate::TriggerKind::from_str(e.kind) as i64)
            .collect()
    }
    fn chain_hits(&self) -> Vec<i64> {
        self.trigger.effects.iter().map(|e| e.target as i64).collect()
    }
    fn trigger_card(&self) -> i64 {
        match self.trigger.card.as_deref() {
            Some(card) if !card.is_empty() => crate::cond_pre::id_of(card),
            _ => 0,
        }
    }
    fn counter_name(&self) -> i64 {
        // `t.Name` on a `CounterChanged` hook / `On::Message` entry.
        match self.trigger.name.as_deref() {
            Some(name) if !name.is_empty() => crate::cond_pre::id_of(name),
            _ => 0,
        }
    }
    fn owner(&self) -> i64 {
        self.cand.map(|(o, _, _)| o as i64).unwrap_or(-1)
    }
    fn owner_money(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.player_money(o as i32) as i64
        } else {
            0
        }
    }
    fn owner_fire(&self) -> i64 {
        let o = self.owner();
        if o >= 0 { self.world.fire(o as i32) as i64 } else { 0 }
    }
    fn owner_crystals(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.band_crystals(o as i32) as i64
        } else {
            0
        }
    }
    fn owner_hand(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world
                .hidden
                .get(o.max(0) as usize)
                .map(|h| h.hand.len() as i64)
                .unwrap_or(0)
        } else {
            0
        }
    }
    fn owner_pos(&self) -> i64 {
        let o = self.owner();
        if o >= 0 { self.world.player_pos(o as i32) as i64 } else { 0 }
    }
    fn owner_out(&self) -> i64 {
        let o = self.owner();
        if o >= 0 { self.world.player_out(o as i32) as i64 } else { 0 }
    }
    fn owner_stay(&self) -> i64 {
        let o = self.owner();
        if o >= 0 { self.world.state_get(o as i32, game_core::state::key::STAY) as i64 } else { 0 }
    }
    fn owner_stun(&self) -> i64 {
        let o = self.owner();
        if o >= 0 { self.world.state_get(o as i32, game_core::state::key::STUN) as i64 } else { 0 }
    }
    fn owner_exile(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.state_get(o as i32, game_core::state::key::EXILE) as i64
        } else {
            0
        }
    }
    fn owner_no_hand(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.state_get(o as i32, game_core::state::key::NO_HAND) as i64
        } else {
            0
        }
    }
    fn owner_character(&self) -> i64 {
        let o = self.owner();
        if o < 0 {
            return 0;
        }
        // The character **name** (`ctx::character_is` spelling), not the
        // skill id -- `character_is(p, "名")` hashes the name.
        let name = self
            .world
            .st
            .players
            .get(o.max(0) as usize)
            .map(|s| s.character.clone())
            .unwrap_or_default();
        crate::cond_pre::id_of(&name)
    }
    fn owner_band(&self) -> i64 {
        let o = self.owner();
        if o < 0 {
            return 0;
        }
        // The band **name** (`ctx::in_band` spelling).
        let band = self
            .world
            .st
            .players
            .get(o.max(0) as usize)
            .and_then(|s| {
                self.data
                    .characters
                    .iter()
                    .find(|c| c.name == s.character)
                    .map(|c| c.band.clone())
            })
            .unwrap_or_default();
        crate::cond_pre::id_of(&band)
    }
    fn owner_tiles(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.owned_tiles(o as i32).len() as i64
        } else {
            0
        }
    }
    fn card_id(&self) -> i64 {
        self.cand
            .map(|(_, c, _)| crate::cond_pre::id_of(c))
            .unwrap_or(0)
    }
    fn card_placed(&self) -> bool {
        self.cand.map(|(_, _, p)| p).unwrap_or(false)
    }
    fn card_counter(&self, name: &str) -> i64 {
        let (Some((o, c, _)), true) = (self.cand, self.owner() >= 0) else {
            return 0;
        };
        self.world.card_counter(o, c, name) as i64
    }
    fn slot(&self, name: &str) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.state_get(o as i32, name) as i64
        } else {
            0
        }
    }
    fn tok_named(&self, name: &str) -> i64 {
        // Same data `ctx::tok(owner, name)` reads.
        let o = self.owner();
        if o >= 0 {
            self.world.tok(o as i32, name) as i64
        } else {
            0
        }
    }
    fn blocked(&self, _band: i64) -> bool {
        false
    }
    fn money(&self, seat: i64) -> i64 {
        self.world.player_money(seat as i32) as i64
    }
    fn fire(&self, seat: i64) -> i64 {
        self.world.fire(seat as i32) as i64
    }
    fn crystals(&self, seat: i64) -> i64 {
        self.world.band_crystals(seat as i32) as i64
    }
    fn hand(&self, seat: i64) -> i64 {
        self.world
            .hidden
            .get(seat.max(0) as usize)
            .map(|h| h.hand.len() as i64)
            .unwrap_or(0)
    }
    fn pos(&self, seat: i64) -> i64 {
        self.world.player_pos(seat as i32) as i64
    }
    fn out(&self, seat: i64) -> i64 {
        self.world.player_out(seat as i32) as i64
    }
    fn stay(&self, seat: i64) -> i64 {
        self.world.state_get(seat as i32, game_core::state::key::STAY) as i64
    }
    fn stun(&self, seat: i64) -> i64 {
        self.world.state_get(seat as i32, game_core::state::key::STUN) as i64
    }
    fn exile(&self, seat: i64) -> i64 {
        self.world
            .state_get(seat as i32, game_core::state::key::EXILE) as i64
    }
    fn no_hand(&self, seat: i64) -> i64 {
        self.world
            .state_get(seat as i32, game_core::state::key::NO_HAND) as i64
    }
    fn character(&self, seat: i64) -> i64 {
        let name = self
            .world
            .st
            .players
            .get(seat.max(0) as usize)
            .map(|s| s.character.clone())
            .unwrap_or_default();
        crate::cond_pre::id_of(&name)
    }
    fn band(&self, seat: i64) -> i64 {
        let band = self
            .world
            .st
            .players
            .get(seat.max(0) as usize)
            .and_then(|s| {
                self.data
                    .characters
                    .iter()
                    .find(|c| c.name == s.character)
                    .map(|c| c.band.clone())
            })
            .unwrap_or_default();
        crate::cond_pre::id_of(&band)
    }
    fn tiles(&self, seat: i64) -> i64 {
        self.world.owned_tiles(seat as i32).len() as i64
    }
    fn seat_count(&self) -> i64 {
        self.world.st.players.len() as i64
    }
    fn tile_named(&self, name: &str) -> i64 {
        self.world.tile_named(self.data, name) as i64
    }
    fn is_circle(&self, tile: i64) -> bool {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.kind == game_core::data::TileKind::Circle)
    }
    fn is_ring(&self, tile: i64) -> bool {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.kind == game_core::data::TileKind::Ring)
    }
    fn is_live_house(&self, tile: i64) -> bool {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.is_buyable() && t.group == 10)
    }
    fn is_buyable(&self, tile: i64) -> bool {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .is_some_and(|t| t.is_buyable())
    }
    fn slot_table(&self) -> Vec<(String, i64)> {
        crate::cond_pre::SLOT_NAMES
            .iter()
            .map(|n| (n.to_string(), self.slot(n)))
            .collect()
    }
    fn tok_named_table(&self) -> Vec<(String, i64)> {
        let o = self.owner();
        if o < 0 {
            return Vec::new();
        }
        self.world
            .tok_names(o as i32, "")
            .into_iter()
            .map(|n| {
                let v = self.world.tok(o as i32, &n) as i64;
                (n, v)
            })
            .collect()
    }
    fn card_counter_table(&self) -> Vec<(String, i64)> {
        // The two on-card wire names (`"cp"` / `"crystals"`) plus any
        // card-declared named counter on the candidate instance.
        let (Some((o, c, _)), true) = (self.cand, self.owner() >= 0) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let cp = self.world.card_counter(o, c, "cp") as i64;
        if cp != 0 {
            out.push(("cp".to_string(), cp));
        }
        let crystals = self.world.card_counter(o, c, "crystals") as i64;
        if crystals != 0 {
            out.push(("crystals".to_string(), crystals));
        }
        out
    }
    fn blocked_bands(&self) -> Vec<i64> {
        Vec::new()
    }
    fn tile_id_table(&self) -> Vec<(String, i64)> {
        crate::cond_pre::collect_tile_ids(self).into_iter().collect()
    }
    fn tile_kind_list(&self, kind: rules_cond::view::TileKind) -> Vec<i64> {
        let mut out = Vec::new();
        for tile in 0..self.data.tiles.len() as i64 {
            let hit = match kind {
                rules_cond::view::TileKind::Circle => self.is_circle(tile),
                rules_cond::view::TileKind::Ring => self.is_ring(tile),
                rules_cond::view::TileKind::LiveHouse => self.is_live_house(tile),
                rules_cond::view::TileKind::Buyable => self.is_buyable(tile),
            };
            if hit {
                out.push(tile);
            }
        }
        out
    }

    // -- turn plan / counters ----------------------------------------------
    fn plan_fixed_roll(&self) -> i64 {
        self.world.turn.fixed_roll.map(|n| n as i64).unwrap_or(-1)
    }
    fn gains_this_turn(&self, seat: i64) -> i64 {
        if seat < 0 {
            return 0;
        }
        self.world.gains_this_turn(seat as i32) as i64
    }
    fn targeted_count(&self, seat: i64) -> i64 {
        if seat < 0 {
            return 0;
        }
        usize::try_from(seat as i32)
            .ok()
            .and_then(|s| self.world.targeted.get(s).copied())
            .unwrap_or(0) as i64
    }

    // -- candidate instance -------------------------------------------------
    fn card_tile(&self) -> i64 {
        let Some((o, c, _)) = self.cand else {
            return -2;
        };
        if o < 0 {
            return -2;
        }
        self.world
            .field_instances(o)
            .into_iter()
            .find(|(_, id)| id == c)
            .map_or(-2i64, |(uid, _)| self.world.tile_at(uid) as i64)
    }

    // -- board geometry ----------------------------------------------------
    fn tile_count(&self) -> i64 {
        self.data.tiles.len() as i64
    }
    fn dist(&self, a: i64, b: i64) -> i64 {
        let n = self.data.tiles.len() as i64;
        if n <= 0 {
            return 0;
        }
        let d = ((b - a) % n + n) % n;
        d.min(n - d)
    }
    fn players_on(&self, tile: i64, except: i64) -> i64 {
        // Matches `players_on_list`: present (`!out && exile == 0`) standers.
        (0..self.world.st.players.len() as i64)
            .filter(|&s| {
                s != except
                    && self.out(s) == 0
                    && self.exile(s) == 0
                    && self.pos(s) == tile
            })
            .count() as i64
    }
    fn next_dist(&self, p: i64, dir: i64) -> i64 {
        let n = self.data.tiles.len() as i64;
        let pos = self.pos(p);
        if n <= 0 || pos < 0 {
            return -1;
        }
        let mut best = i64::MAX;
        for o in 0..self.world.st.players.len() as i64 {
            if o == p || self.out(o) != 0 {
                continue;
            }
            let q = self.pos(o);
            if q < 0 {
                continue;
            }
            let fwd = if dir >= 0 {
                ((q - pos) % n + n) % n
            } else {
                ((pos - q) % n + n) % n
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
        let pos = self.pos(p);
        if pos < 0 {
            return 0;
        }
        (0..self.world.st.players.len() as i64)
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
        let pos = self.pos(p);
        if pos < 0 {
            return 0;
        }
        self.world
            .owned_tiles(p as i32)
            .into_iter()
            .filter(|&t| self.dist(pos, t as i64) <= radius)
            .count() as i64
    }
    fn on_path(&self, me: i64, them: i64) -> i64 {
        let roll = self.move_roll();
        if roll < 0 {
            return 0;
        }
        let steps = roll.abs();
        let n = self.data.tiles.len() as i64;
        let pos = self.pos(them);
        if n <= 0 || pos < 0 {
            return 0;
        }
        (1..=steps)
            .filter(|i| {
                let t = ((pos + i) % n + n) % n;
                self.world.tile_owner(t as i32) as i64 == me
            })
            .count() as i64
    }
    fn between(&self, p: i64) -> i64 {
        let roll = self.move_roll();
        if roll < 0 {
            return 0;
        }
        let roll = roll.abs();
        let start = self.pos(p);
        let n = self.data.tiles.len() as i64;
        if n <= 0 || start < 0 {
            return 0;
        }
        let backward = self.move_dir() < 0;
        (0..self.world.st.players.len() as i64)
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

/// Per-window processed set (docs/GUARDS.md §4.5): one trigger's ring, across
/// its laps. Remembers each `(seat, card)` probe's verdict so a later lap does
/// not re-run its condition / guard when the inputs it reads cannot have
/// changed.
///
/// The ring's only world change is a declaration removing a card from a hand
/// (`build_round`); no resolving link runs until the whole answer tree is
/// built. So a verdict that reads no hand field (`cond_reads_hand`, and any
/// residual wasm guard -- it may read anything) is stable for the window, and
/// only hand-sensitive verdicts are stamped with [`Self::hand_gen`].
struct ProbeMemo {
    /// Bumps whenever a declaration removes a card from a hand.
    hand_gen: u64,
    /// `(seat, card id) -> (hand_gen at probe, eligible, hand-sensitive)`.
    verdicts: std::collections::HashMap<(usize, String), (u64, bool, bool)>,
    /// Lazily built CEL scope, stamped with the `hand_gen` its `_hand` table
    /// reflects. A probe that does not call `hand(p)` accepts a stale scope --
    /// its window inputs are unchanged, and the candidate overlay is rebuilt
    /// per probe from the live world.
    scope: Option<(u64, WindowScope)>,
}

impl ProbeMemo {
    fn new() -> Self {
        Self {
            hand_gen: 0,
            verdicts: std::collections::HashMap::new(),
            scope: None,
        }
    }

    fn on_declaration(&mut self) {
        self.hand_gen += 1;
    }

    /// The shared window scope, built only when some candidate actually has a
    /// condition (docs/GUARDS.md §4.4 item 1). `need_fresh_hand` forces a
    /// rebuild when the probe reads `hand(p)` and a declaration has moved the
    /// hand table since the scope was built.
    fn scope_for<S: crate::cond_pre::SnapSrc>(
        &mut self,
        world: &S,
        need_fresh_hand: bool,
    ) -> &WindowScope {
        let fresh = self
            .scope
            .as_ref()
            .is_some_and(|(g, _)| *g == self.hand_gen);
        if self.scope.is_none() || (need_fresh_hand && !fresh) {
            let win = crate::cond_pre::fill_window(world);
            self.scope = Some((self.hand_gen, crate::cond_pre::window_scope(&win)));
        }
        &self.scope.as_ref().expect("just built").1
    }
}

/// The one-line description of the answered link the [反击] prompt shows.
///
/// The key is kind-specific so the client can say **what** is being answered
/// (「{{who}} 打出了「{{card}}」」 / 「{{who}} 将支付 {{n}} 给 {{to}}」 /
/// 「{{who}} 将移动 {{n}} 格」) instead of a bare 「{{who}} 的触发」.
/// `ask.counteract.detail` stays the fallback for the long tail of kinds -- and
/// for prompts already on the wire, so old replays keep rendering.
fn describe_trigger(t: &Trigger) -> Msg {
    let effect_kind = t.effects.first().map(|e| e.kind).unwrap_or("");
    let card = t.card.as_deref().unwrap_or("");
    let mut m = match t.kind {
        // A play (L1) or a declared counter -- both are `card` links.
        TriggerKind::Card if !card.is_empty() => Msg::new("ask.counteract.detail.play"),
        TriggerKind::MoveBefore | TriggerKind::MoveAfter | TriggerKind::Roll | TriggerKind::MoveRoll => {
            Msg::new("ask.counteract.detail.move")
        }
        // The [反击] key for a payment / a targeted effect is `effect`
        // (`docs` on `TriggerKind::Effect`); the declared effect's own kind
        // says which.
        TriggerKind::Effect if effect_kind == "pay" => {
            if t.pay.map(|p| p.is_rent).unwrap_or(false) && t.target >= 0 {
                Msg::new("ask.counteract.detail.rent")
            } else if t.target >= 0 {
                Msg::new("ask.counteract.detail.pay")
            } else {
                Msg::new("ask.counteract.detail.pay_out")
            }
        }
        TriggerKind::Effect if effect_kind != "" && !card.is_empty() => {
            Msg::new("ask.counteract.detail.effect")
        }
        _ => Msg::new("ask.counteract.detail"),
    };
    m = m.player_id("who", t.player_id);
    if !card.is_empty() {
        m = m.card("card", card);
    }
    if t.tile >= 0 {
        m = m.tile("tile", t.tile);
    }
    if t.target >= 0 {
        m = m.player_id("to", t.target);
    }
    m = m.i("n", t.value as i64);
    m
}

/// Every wire `Trigger.Kind` string the engine raises (turnStart / pass /
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
        buy: Some(TriggerBuy {
            kind: card_sdk::abi::BuyKind::from_i32(q.kind.as_i32())
                .unwrap_or(card_sdk::abi::BuyKind::Land),
            seller: game_core::engine::rules::Payee::from_i32(q.seller),
            price,
            deal_owner: Some(q.player as i32),
            deal_houses: st.houses.get(t).copied().unwrap_or(0),
            deal_mortgaged: st.mortgaged.get(t).copied().unwrap_or(false),
        }),
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
        // Fix C: the manifest's declared kinds first. No card in the set hooks
        // a buy kind -> no live instance can either, so skip the walk.
        if !self.ruleset.declares_buy() {
            return Vec::new();
        }
        let declares = |id: &str| {
            self.ruleset.card(id).is_some_and(|i| {
                crate::host::BUY_HOOK_KINDS
                    .iter()
                    .any(|&k| self.ruleset.hooks_to(i, k))
            })
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
        if !self.ruleset.hooks_to(idx, kind) {
            return;
        }
        // The run is a throwaway: only the trigger it rewrote is read back, and
        // the world `run_hook` produces is dropped. `run_hook` clones the run
        // into the store it fires up, so the caller shares one `Run` across the
        // whole quote rather than cloning per hook.
        run.trigger = trig.clone();
        run.current_card = card.to_string();
        run.current_uid = uid;
        // `None` for `on_body`: a quote is a preview, never an activation --
        // the commit's own re-quote is the run that may flash.
        if let Ok(Some(hr)) = self.ruleset.run_hook(
            &*run,
            Call::Hook {
                card: idx,
                kind,
                player_id: owner,
            },
            &[],
            None,
        ) {
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

    /// [`game_core::engine::CardRules::has_play`] -- the rule's `On::Play`
    /// entry. Passive / hook-only skills never appear in a skill list.
    fn has_play(&self, card: &str) -> bool {
        self.ruleset
            .card(card)
            .is_some_and(|idx| self.ruleset.has_play_entry(idx))
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
        let instances = cx.world().tile_rule_instances(tile as i32);
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
        // No move payload (`mv: None`): the tile body reads `main`, not a move.
        // (The old shape stamped `move_resolve: true` with `move_kind: None`;
        // `move_resolve` now lives on the move payload and reads false when
        // there is no move.)
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: owner,
            tile: tile as i32,
            step: cx.state().step,
            main,
            ..Trigger::default()
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
        // A pure query ("why can this not be played?"): a guard that prompts,
        // or a module that fails, never blocks play. Fix B -- the probe shares
        // the live world handle (`share_world`, no copy); the gate body
        // detaches a private copy only if it writes.
        let idx = self.ruleset.card(card)?;
        // Fix A cheap pre-filter: no `On::Play` entry, or G4-deleted gate with
        // no condition -- the verdict is always "playable". Skip the uid
        // lookup, the `Run` and the CEL scope entirely (the condition-only /
        // declares-bitmask shape REPORT.md's A asks for).
        if self.ruleset.play_gate_vanishes(idx) {
            return None;
        }
        // Bind the instance when the card is already on the field (a skill
        // press), so `is_placed` / `crystals` answer for it and not for a void
        // `current_uid = -1`. A read of the live world -- no copy.
        let uid = cx
            .world()
            .field_instances(player_id as i32)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
        let run = Run {
            world: cx.share_world(),
            pile_checkpoints: Default::default(),
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
            counter_log: vec![],
            exile_log: vec![],
            doubled: -1,
            linger_props: Default::default(),
            wrap_effect: false,
            live: None,
            posted: vec![],
            log_idx: 0,
            parent: -1,
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
        // Cheap path (fix C): the manifest's declared kinds say no card in the
        // set hooks a buy kind, so no live instance can either -- the quote is
        // the native formula and neither the instance walk nor a module runs.
        if !self.ruleset.declares_buy() {
            return q.tiles.iter().map(|&t| native(t)).collect();
        }
        // Cheap path: no live instance declares a buy hook, so the quote is the
        // native formula and no module is fired up.
        let instances = self.buy_hook_instances(w);
        if instances.is_empty() {
            return q.tiles.iter().map(|&t| native(t)).collect();
        }
        // Only the stages somebody actually declares are run -- a `BuySet`
        // nobody hooks is a no-op that would cost a fire-up per tile. The
        // per-card `hook_mask` shift (fix C), not an entry-table scan.
        let declares = |kind: TriggerKind| {
            instances
                .iter()
                .any(|(_, _, card)| self.ruleset.card(card).is_some_and(|i| self.ruleset.hooks_to(i, kind)))
        };
        let gate_decl = declares(TriggerKind::BuyGate);
        // One throwaway run for the whole quote (fix B): the world it carries
        // is a COW handle to `w` (one deep copy per quote batch, not per hook);
        // `run_hook`'s per-hook store clone is a refcount bump, and a hook body
        // that writes detaches its own copy which is then dropped.
        let mut run = Run {
            world: game_core::engine::SharedWorld::new(w.clone()),
            pile_checkpoints: Default::default(),
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
            counter_log: vec![],
            exile_log: vec![],
            doubled: -1,
            linger_props: Default::default(),
            wrap_effect: false,
            live: None,
            posted: vec![],
            log_idx: 0,
            parent: -1,
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
                    price = at(&mut run, kind, price)
                        .buy
                        .map(|b| b.price)
                        .unwrap_or(price)
                        .max(0);
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
            .world()
            .field_instances(player_id as i32)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            step: cx.state().step,
            // The card is playing itself, so it is its own cause.
            by_card: Some(player_id as i32),
            ..Trigger::default()
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
            .world()
            .event_rule_instances()
            .into_iter()
            .find(|(_, c)| c == &rid)
            .map_or(-1, |(uid, _)| uid);
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            step: cx.state().step,
            ..Trigger::default()
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
        // The module's view of the trigger. `move_roll` is the guest-visible
        // `t.Move.Roll`, which a counteraction may rewrite; the engine reads
        // it back afterwards.
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
        // (1) The played card's own follow-up: the card named on the trigger
        // runs its `counteract` entry (a card answering its own play). Only at
        // the play itself -- `cardAfter` / `cardPlayed` / `eventAfter` /
        // `drawn` also name a card on `t.card`, and must not re-run it here.
        let own = t.card.clone().unwrap_or_default();
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
        // (2) The hand-counteraction window: one round per timing, from the
        // seat after the timing's player around the table; counters settle
        // newest-first before the timing they answer (see
        // `hand_counteractions`). Not at hook-only points.
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
                // These run on a fresh instance of the card named on
                // `t.card` (`Drawn`) -- it is in a hand / pile, not placed.
                // `Discarded` is *not* here: a placed field card hears about
                // discards (MyGO band (3) 「每次你的卡在未生效的情况下进入弃牌
                // 堆时」), so it rides the field-card path below.
                if let Some(card_id) = t.card.as_deref().filter(|s| !s.is_empty()) {
                    if let Some(idx) = self
                        .ruleset
                        .card(card_id)
                        .filter(|&i| self.ruleset.cards()[i as usize].hooks(kind))
                    {
                        self.drive_hook(
                            cx,
                            Call::Hook {
                                card: idx,
                                kind,
                                player_id: t.player_id,
                            },
                            card_id,
                            -1,
                            &mut trigger,
                        )?;
                    }
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
                        // `RollPlan` runs on every live Fx before the dice --
                        // `On::RollPlan` shapes the move.
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
            // Scheduled turn-end callbacks (「你的下回合结束时」 and kin): the
            // ones due at this player's turn end run once and are dropped. They
            // are taken out of the world *before* running, so a callback that
            // schedules again lands in the next round.
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
        // Write back whatever a counteraction rewrote. `set_move_roll` lands
        // in `trigger.move_roll` (the shared `t.Move` the counteractions also
        // read); the pay amount is rewritten in `trigger.value`. The engine
        // reads the result back off `t.value` after `counteract` returns.
        t.value = trigger.mv.as_ref().and_then(|m| m.roll).unwrap_or(trigger.value);
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
///
/// `PassTile` / `PassPlayer` are **not** in here: ABI v43 made them
/// [`ChainKind`]s too (「当你经过一名角色时」 -- `AG:刻入天穹傲岸的烈光`),
/// so the hand [反击] window opens at them. The valid-option-first pre-scan
/// skips the whole ring when no seat holds a card that answers, which is the
/// common case on a `passTile` (it fires on every tile walked).
fn is_hook_only(kind: &str) -> bool {
    matches!(
        trigger_kind(kind),
        TriggerKind::TurnEnd
            | TriggerKind::Drawn
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
            | TriggerKind::CounterChanged
    )
}

/// Kinds that run on a fresh instance of one card (named on `t.card`) rather
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
