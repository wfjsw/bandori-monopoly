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

use crate::host::{Call, Outcome, Prompt, PromptOption, RuleError, Ruleset};
use crate::{PromptKind};
use crate::world::{CardWorld, Trigger};
use crate::TriggerKind;


/// One run of a card module against a copy of the match world.
/// C# `PlayCtx.Dest` values the module returns.
const DEST_GRAVEYARD: i32 = 0;
const DEST_FIELD: i32 = 2;

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
    /// `PlayCtx.Dest`, set by the module (`set_dest`).
    dest: i32,
}

impl CardWorld for Run {
    fn roll(&mut self, seat: i32, count: i32, sides: i32) -> i32 {
        let mut sum = 0;
        for _ in 0..count.max(0) {
            sum += self.world.rng.d(sides.max(1));
        }
        self.world.log("dice", seat, Msg::new("log.dice").seat("who", seat).i("count", count).i("sides", sides).i("sum", sum));
        sum
    }

    fn log(&mut self, seat: i32, msg: Msg) {
        self.world.log("text", seat, msg);
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
    fn seat_pos(&self, seat: i32) -> i32 {
        self.world.seat_pos(seat)
    }
    fn tile_steps_ahead(&self, seat: i32, steps: i32) -> i32 {
        self.world.tile_steps_ahead(&self.data, seat, steps)
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
    fn owned_count(&self, seat: i32) -> i32 {
        self.world.owned_tiles(seat).len() as i32
    }
    fn owned_at(&self, seat: i32, index: i32) -> i32 {
        self.world.owned_tiles(seat).get(index.max(0) as usize).copied().unwrap_or(-1)
    }

    // seats -----------------------------------------------------------------
    fn seat_count(&self) -> i32 {
        self.world.st.seats.len() as i32
    }
    fn seat_out(&self, seat: i32) -> i32 {
        self.world.seat_out(seat) as i32
    }
    fn others_count(&self, seat: i32) -> i32 {
        self.world.others(seat).len() as i32
    }
    fn others_at(&self, seat: i32, index: i32) -> i32 {
        self.world.others(seat).get(index.max(0) as usize).copied().unwrap_or(-1)
    }
    fn money(&self, seat: i32) -> i32 {
        self.world.seat_money(seat)
    }
    fn gain(&mut self, seat: i32, amount: i32, src: Msg) -> i32 {
        self.world.gain_money(seat, amount, src)
    }
    fn pay(&mut self, seat: i32, amount: i32, src: Msg) -> i32 {
        self.world.pay_money(seat, amount, src)
    }

    // hand / deck -----------------------------------------------------------
    fn draw(&mut self, seat: i32, n: i32) -> i32 {
        self.world.draw_cards(seat, n, true)
    }
    fn add_to_hand(&mut self, seat: i32, card: &str) {
        self.world.add_to_hand(seat, card);
    }
    fn add_to_deck(&mut self, seat: i32, card: &str, shuffle: bool) {
        self.world.add_to_deck(seat, card, shuffle);
    }
    fn add_to_deck_at(&mut self, seat: i32, card: &str, pos: i32) {
        // C# `H.AddToDeck(seat, card, where)`: "top" = draw.Add (the end of the
        // vec is the top), "bottom" = Insert(0), anything else = add + shuffle.
        let Some(h) = self.world.hidden.get_mut(seat.max(0) as usize) else { return };
        match pos {
            1 => h.draw.insert(0, card.to_string()),
            2 => {
                h.draw.push(card.to_string());
                self.world.rng.shuffle(&mut h.draw);
            }
            _ => h.draw.push(card.to_string()),
        }
    }
    fn to_discard(&mut self, seat: i32, card: &str) {
        self.world.to_discard(seat, card);
    }

    // field cards -----------------------------------------------------------
    fn place_card(&mut self, seat: i32, card: &str, note: Msg) {
        self.world.place_card(seat, card, note);
    }
    fn set_dest(&mut self, dest: i32) {
        self.dest = dest;
    }
    fn ring_multiplier(&self) -> i32 {
        self.world.ring_multiplier(&self.data)
    }
    fn add_ring_bonus(&mut self, n: i32) -> i32 {
        self.world.add_ring_bonus(n)
    }
    fn teleport_to(&mut self, seat: i32, tile: i32) {
        self.world.teleport_to(seat, tile);
    }
    fn unplace_card(&mut self, seat: i32) -> bool {
        let id = self.current_card.clone();
        self.world.unplace_card(seat, &id).is_some()
    }
    fn is_placed(&self, seat: i32) -> i32 {
        self.world.placed_cards(seat).contains(&self.current_card) as i32
    }


    // marks & tokens --------------------------------------------------------
    fn add_mark(&mut self, tile: i32, seat: i32, kind: &str, note: Msg) {
        self.world.add_mark(tile, seat, kind, note);
    }
    fn count_marks(&self, tile: i32, kind: &str, owner: i32) -> i32 {
        self.world.count_marks(tile, kind, owner)
    }
    fn remove_marks(&mut self, tile: i32, kind: &str, owner: i32) -> i32 {
        self.world.remove_marks(tile, kind, owner)
    }
    fn tok(&self, seat: i32, name: &str) -> i32 {
        self.world.tok(seat, name)
    }
    fn set_tok(&mut self, seat: i32, name: &str, value: i32) {
        self.world.set_tok(seat, name, value);
    }
    fn add_tok(&mut self, seat: i32, name: &str, n: i32, max: i32) -> i32 {
        self.world.add_tok(seat, name, n, max)
    }

    // per-seat slots (V) ----------------------------------------------------
    fn slot(&self, seat: i32, key: &str) -> i32 {
        self.world.slot(seat, key)
    }
    fn set_slot(&mut self, seat: i32, key: &str, value: i32) {
        self.world.set_slot(seat, key, value);
    }
    fn inc_slot(&mut self, seat: i32, key: &str, by: i32) -> i32 {
        self.world.inc_slot(seat, key, by)
    }

    // status pots -----------------------------------------------------------
    fn band_crystals(&self, seat: i32) -> i32 {
        self.world.band_crystals(seat)
    }
    fn add_band_crystals(&mut self, seat: i32, n: i32, max: i32) -> i32 {
        self.world.add_band_crystals(seat, n, max)
    }
    fn fire(&self, seat: i32) -> i32 {
        self.world.fire(seat)
    }
    fn fire_max(&self, seat: i32) -> i32 {
        self.world.fire_max(seat)
    }
    fn gain_fire(&mut self, seat: i32, n: i32, why: Msg) -> i32 {
        self.world.gain_fire(seat, n, why)
    }
    fn give_stay(&mut self, seat: i32, n: i32) {
        self.world.give_stay(seat, n);
    }
    fn give_stun(&mut self, seat: i32, n: i32) {
        self.world.give_stun(seat, n);
    }
    fn give_exile(&mut self, seat: i32, n: i32, to: i32) {
        self.world.give_exile(seat, n, to);
    }
    fn give_extra_turn(&mut self, seat: i32) {
        self.world.give_extra_turn(seat as usize);
    }

    // board extensions ------------------------------------------------------
    fn is_buyable(&self, tile: i32) -> i32 {
        self.data.tiles.get(tile.max(0) as usize).is_some_and(|t| t.is_buyable()) as i32
    }
    fn is_shop(&self, tile: i32) -> i32 {
        // C# `H.IsShop`: buyable and colour group 10 (the 商店街 deeds).
        self.data.tiles.get(tile.max(0) as usize).is_some_and(|t| t.is_buyable() && t.group == 10) as i32
    }
    fn tile_group(&self, tile: i32) -> i32 {
        self.data.tiles.get(tile.max(0) as usize).map(|t| t.group).unwrap_or(-1)
    }
    fn tile_price(&self, tile: i32) -> i32 {
        // `H._tiles[t].price` -- land alone (`buy_price` adds houses).
        self.data.tiles.get(tile.max(0) as usize).map(|t| t.price).unwrap_or(0)
    }
    fn houses_of(&self, tile: i32) -> i32 {
        self.world.st.houses.get(tile.max(0) as usize).copied().unwrap_or(0)
    }
    fn set_houses(&mut self, tile: i32, n: i32) {
        if let Some(h) = self.world.st.houses.get_mut(tile.max(0) as usize) {
            *h = n.max(0);
        }
    }
    fn add_house(&mut self, tile: i32, n: i32) -> i32 {
        if let Some(h) = self.world.st.houses.get_mut(tile.max(0) as usize) {
            *h = (*h + n).max(0);
            return *h;
        }
        0
    }
    fn mortgaged_of(&self, tile: i32) -> i32 {
        self.world.st.mortgaged.get(tile.max(0) as usize).copied().unwrap_or(false) as i32
    }
    fn set_mortgaged(&mut self, tile: i32, v: i32) {
        if let Some(m) = self.world.st.mortgaged.get_mut(tile.max(0) as usize) {
            *m = v != 0;
        }
    }
    fn set_owner(&mut self, tile: i32, seat: i32) {
        if let Some(o) = self.world.st.owners.get_mut(tile.max(0) as usize) {
            *o = seat;
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
    fn neighbor(&self, seat: i32, dir: i32) -> i32 {
        // C# `H.Neighbor` -- the next present seat in turn order, wrapping.
        let n = self.world.st.seats.len();
        if n == 0 {
            return -1;
        }
        let dir = if dir < 0 { -1 } else { 1 };
        for j in 1..n {
            let k = ((seat + dir * j as i32) % n as i32 + n as i32) % n as i32;
            if self.present(k) {
                return k;
            }
        }
        -1
    }
    fn seats_on_count(&self, tile: i32, except: i32) -> i32 {
        self.seats_on_list(tile, except).len() as i32
    }
    fn seats_on_at(&self, tile: i32, except: i32, index: i32) -> i32 {
        self.seats_on_list(tile, except).get(index.max(0) as usize).copied().unwrap_or(-1)
    }

    // hand & deck extensions -----------------------------------------------
    fn hand_count(&self, seat: i32, card: &str) -> i32 {
        self.hands(seat).map(|h| h.iter().filter(|c| c.as_str() == card).count() as i32).unwrap_or(0)
    }
    fn discard_count(&self, seat: i32, card: &str) -> i32 {
        self.world
            .hidden
            .get(seat.max(0) as usize)
            .map(|h| h.discard.iter().filter(|c| c.as_str() == card).count() as i32)
            .unwrap_or(0)
    }
    fn deck_count(&self, seat: i32) -> i32 {
        self.world.hidden.get(seat.max(0) as usize).map(|h| h.draw.len() as i32).unwrap_or(0)
    }
    fn discard_size(&self, seat: i32) -> i32 {
        self.world.hidden.get(seat.max(0) as usize).map(|h| h.discard.len() as i32).unwrap_or(0)
    }
    fn discard_from_hand(&mut self, seat: i32, card: &str) -> i32 {
        // C# `H.DiscardFromHand` -- one copy, hand -> discard pile.
        let Some(h) = self.world.hidden.get_mut(seat.max(0) as usize) else { return 0 };
        let Some(p) = h.hand.iter().position(|c| c == card) else { return 0 };
        h.hand.remove(p);
        h.discard.push(card.to_string());
        1
    }
    fn sweep_to_deck(&mut self, seat: i32) -> i32 {
        // C# `H.ShuffleAllIntoDeck` -- hand + discard into the draw pile, shuffled.
        let Some(h) = self.world.hidden.get_mut(seat.max(0) as usize) else { return 0 };
        let mut n = 0;
        for c in h.hand.drain(..).chain(h.discard.drain(..)) {
            h.draw.push(c);
            n += 1;
        }
        self.world.rng.shuffle(&mut h.draw);
        n
    }

    // status extensions -----------------------------------------------------
    fn can_pay(&self, seat: i32) -> i32 {
        // C# `H.CanPay`: not out, not stunned, not exiled.
        let Some(s) = self.world.st.seats.get(seat.max(0) as usize) else { return 0 };
        (!s.out() && !s.stunned() && s.exile == 0) as i32
    }
    fn spend_fire(&mut self, seat: i32, n: i32, why: Msg) -> i32 {
        // C# `H.SpendFire` -- pots first; logs the spend with its reason (the
        // caller's message is the log line, like `gain`/`pay`).
        if n <= 0 {
            return 1;
        }
        let Some(s) = self.world.st.seats.get_mut(seat.max(0) as usize) else { return 0 };
        if s.fire < n {
            return 0;
        }
        s.fire -= n;
        self.world.log("fire", seat, why).value = -n;
        1
    }
    fn stay_of(&self, seat: i32) -> i32 {
        self.world.st.seats.get(seat.max(0) as usize).map(|s| s.stay).unwrap_or(0)
    }
    fn stun_of(&self, seat: i32) -> i32 {
        self.world.st.seats.get(seat.max(0) as usize).map(|s| s.stun).unwrap_or(0)
    }
    fn turn_seat(&self) -> i32 {
        self.world.st.turn
    }
    fn round_no(&self) -> i32 {
        self.world.st.round
    }
    fn turn_key(&self) -> i32 {
        // C# `H.TurnKey => State.round * 100 + State.turn + 1`.
        self.world.st.round * 100 + self.world.st.turn + 1
    }
    fn character_is(&self, seat: i32, name: &str) -> i32 {
        self.world.st.seats.get(seat.max(0) as usize).is_some_and(|s| s.character == name) as i32
    }
    fn in_band(&self, seat: i32, name: &str) -> i32 {
        // C# `H.BandOf(seat)` -- the character's band.
        let Some(s) = self.world.st.seats.get(seat.max(0) as usize) else { return 0 };
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
    fn trig_card_is(&self, id: &str) -> i32 {
        (self.trigger.card == id) as i32
    }
}


impl Run {
    /// C# `H.Present(p)` -- in the game and not exiled.
    fn present(&self, s: i32) -> bool {
        self.world.st.seats.get(s.max(0) as usize).is_some_and(|x| !x.out() && x.exile == 0)
    }

    /// C# `H.SeatsOn(tile, except)` -- present seats standing on the tile.
    fn seats_on_list(&self, tile: i32, except: i32) -> Vec<i32> {
        (0..self.world.st.seats.len() as i32)
            .filter(|&p| p != except && self.present(p) && self.world.st.seats[p as usize].pos == tile)
            .collect()
    }

    /// The seat's hand (C# `_hidden[s].hand`).
    fn hands(&self, seat: i32) -> Option<&Vec<String>> {
        self.world.hidden.get(seat.max(0) as usize).map(|h| &h.hand)
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
        let Ok(text) = std::fs::read_to_string(&index) else { return Ok(None) };
        let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| RuleError::Load(format!("bad index: {e}")))?;
        let mut builder = Ruleset::builder();
        let mut any = false;
        for m in v["modules"].as_array().map(|a| a.iter()).into_iter().flatten() {
            let file = m["file"].as_str().ok_or_else(|| RuleError::Load("module entry has no file".into()))?;
            let bytes = std::fs::read(dir.join(file)).map_err(|e| RuleError::Load(format!("{file}: {e}")))?;
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
    fn drive(&self, cx: &mut Cx, call: Call, card_id: &str, trigger: &mut Trigger) -> Flow<i32> {
        let mut answers: Vec<i32> = Vec::new();
        loop {
            let run = Run {
                world: cx.world_copy(),
                data: self.data.clone(),
                trigger: trigger.clone(),
                current_card: card_id.to_string(),
                dest: DEST_GRAVEYARD,
            };
            match self.ruleset.run(&run, call, &answers) {
                Ok(Outcome::Done(after)) => {
                    let dest = after.dest;
                    *trigger = after.trigger;
                    cx.swap_world(after.world);
                    return Ok(dest);
                }
                Ok(Outcome::NeedInput(p)) => {
                    let seat = p.seat;
                    let ask = prompt_to_ask(p);
                    let reply = cx.ask(ask)?;
                    // A card prompt has one seat: the module's i-th answer is that
                    // seat's answer to its i-th prompt.
                    let v = reply.a.answers.first().copied().filter(|&x| x >= 0).unwrap_or(reply.fallback);
                    answers.push(v);
                    let _ = seat;
                }
                Err(e) => {
                    // A trapping module must not take the match down with it.
                    let detail = match &e {
                        RuleError::Trap(m) | RuleError::Load(m) | RuleError::DuplicateCard(m) => m.clone(),
                        RuleError::NoSuchCard(i) => format!("no such card {i}"),
                        RuleError::GuardPrompted => "guard prompted".into(),
                    };
                    let seat = match call { Call::Play { seat, .. } | Call::React { seat, .. } => seat };
                    cx.log(seat, Msg::new("log.card_trap").card("card", card_id).text("detail", detail));
                    return Ok(DEST_GRAVEYARD);
                }
            }
        }
    }

    /// The [反击] hand window (C# `MatchHost.React(Trigger)`): each seat from the
    /// trigger's seat around the table may answer with a reaction card from hand.
    /// Declarations are collected first (hands shrink as they declare) and then
    /// resolved in reverse declaration order (C# `declared[k]` from the end).
    fn hand_reactions(&self, cx: &mut Cx, t: &CoreTrigger, trigger: &mut Trigger, depth: u32) -> Flow<()> {
        if depth >= MAX_REACT_DEPTH || !cx.playing() {
            return Ok(());
        }

        let start = if t.seat >= 0 { t.seat as usize } else { cx.state().turn.max(0) as usize };
        let mut declared: Vec<(usize, i32, String)> = Vec::new();
        for s in cx.present_from(start) {
            if !can_react_now(cx, s) {
                continue;
            }
            // Hand cards that answer this trigger (C# `_hidden[s].hand.Distinct()`).
            let mut options: Vec<(String, i32)> = Vec::new();
            let mut seen: Vec<String> = Vec::new();
            for id in hand_of(cx, s) {
                if seen.contains(&id) {
                    continue;
                }
                seen.push(id.clone());
                let Some(idx) = self.ruleset.card(&id) else { continue };
                if !self.ruleset.cards()[idx as usize].react {
                    continue;
                }
                let run = Run {
                    world: cx.world_copy(),
                    data: self.data.clone(),
                    trigger: trigger.clone(),
                    current_card: id.clone(),
                    dest: DEST_GRAVEYARD,
                };
                if self.ruleset.can_react(&run, idx, s as i32).unwrap_or(false) {
                    options.push((id, idx));
                }
            }
            if options.is_empty() {
                continue;
            }
            // C#: labels "打出「…」" + "不打"; the hint is the first ReactHint or
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
            let pick = reply.a.answers.first().copied().filter(|&x| x >= 0).unwrap_or(reply.fallback) as usize;
            if pick >= options.len() {
                continue;
            }
            let (id, idx) = options[pick].clone();
            // The declaration leaves the hand now (C# `_hidden[s].hand.Remove`).
            let mut w = cx.world_copy();
            if let Some(p) = w.hidden[s].hand.iter().position(|c| c == &id) {
                w.hidden[s].hand.remove(p);
            }
            cx.swap_world(w);
            declared.push((s, idx, id));
        }

        for (s, idx, id) in declared.into_iter().rev() {
            // C# `PlayCard`: the play is logged, the play itself opens a
            // counter-reaction window, then the effect resolves.
            cx.log(s as i32, Msg::new("log.play_react").seat("who", s as i32).card("card", id.clone()));
            let mut answer = CoreTrigger::new("card", s);
            answer.card = id.clone();
            self.hand_reactions(cx, &answer, trigger, depth + 1)?;
            let dest = self.drive(cx, Call::React { card: idx, seat: s as i32 }, &id, trigger)?;
            let mut w = cx.world_copy();
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
            // C# `Trigger { Kind = "reacted", Seat = declared, Target = t.Seat }`.
            if t.seat >= 0 && s as i32 != t.seat {
                let mut r = CoreTrigger::new("reacted", s);
                r.target = t.seat;
                r.card = id.clone();
                self.hand_reactions(cx, &r, trigger, depth + 1)?;
            }
        }
        Ok(())
    }
}

/// A module prompt -> the engine's `Ask` (the player sees the same prompt style
/// the engine shows for its own questions).
fn prompt_to_ask(p: Prompt) -> Ask {
    let seats = vec![p.seat as usize];
    let Prompt { kind, seat, title, text, options, answer_slot: _ } = p;
    match kind {
        PromptKind::Yes => Ask::choice(seats, title, text, vec![Msg::new("ask.yes"), Msg::new("ask.no")], 1, 12.0),
        PromptKind::Pick => {
            let labels = options.into_iter().map(|o| match o {
                PromptOption::Str(m) => m,
                PromptOption::Int(i) => Msg::new("ask.intOption").i("n", i),
            }).collect();
            Ask::choice(seats, title, text, labels, 0, 15.0)
        }
        PromptKind::Tile => {
            let tiles: Vec<usize> = options.iter().map(|o| match o {
                PromptOption::Int(i) => *i as usize,
                PromptOption::Str(_) => 0,
            }).collect();
            let labels = tiles.iter().map(|&t| Msg::new("ask.tileOption").tile("tile", t as i32)).collect();
            Ask::tile(seat as usize, title, text, &tiles, labels)
        }
        PromptKind::Card => {
            // Pick one card out of a list; the options are ready-made labels.
            let labels = options.into_iter().map(|o| match o {
                PromptOption::Str(m) => m,
                PromptOption::Int(i) => Msg::new("ask.intOption").i("n", i),
            }).collect();
            Ask::choice(seats, title, text, labels, 0, 15.0)
        }
        PromptKind::Seat => {
            let labels = options.iter().map(|o| match o {
                PromptOption::Int(s) => Msg::new("ask.seat").seat("who", *s),
                PromptOption::Str(_) => Msg::new("ask.seat").seat("who", -1),
            }).collect();
            Ask::choice(seats, title, text, labels, 0, 15.0)
        }
    }
}

/// The engine's string trigger kinds -> the module's small enum.

/// C# `MatchHost.CanReactNow(s, t)`: out / exiled seats cannot declare a
/// reaction. (The C# also checks `CannotPlay` and a one-turn mute; neither has
/// an engine field yet.)
fn can_react_now(cx: &Cx, s: usize) -> bool {
    let Some(seat) = cx.state().seats.get(s) else { return false };
    // C# `CanReactNow`: out / AI / exiled seats never open a window, and
    // `CannotPlay` (stun, 飞鸟山之战's no-hand, Fx.CantPlayHand) blocks it too.
    // `_noReactTurn` and Fx.CantPlayHand have no engine field yet (TODO).
    !cx.world_copy().out(s) && !seat.ai && seat.exile == 0 && !seat.stunned() && seat.no_hand == 0
}

/// C# `_hidden[s].hand` -- the seat's private hand (order preserved).
fn hand_of(cx: &Cx, s: usize) -> Vec<String> {
    match cx.world_copy().hidden.get(s) {
        Some(h) => h.hand.clone(),
        None => Vec::new(),
    }
}

/// The one-line trigger description the [反击] prompt shows (C# `DescribeTrigger`).
fn describe_trigger(t: &CoreTrigger) -> Msg {
    let mut m = Msg::new("ask.react.detail").seat("who", t.seat);
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
    fn why_not(&self, cx: &Cx, seat: usize, card: &str) -> Option<Msg> {
        // A pure query on a throwaway copy (C# `Card.WhyNot`): a guard that
        // prompts, or a module that fails, never blocks play.
        let idx = self.ruleset.card(card)?;
        let run = Run {
            world: cx.world_copy(),
            data: self.data.clone(),
            trigger: Trigger::default(),
            current_card: card.to_string(),
            dest: DEST_GRAVEYARD,
        };
        self.ruleset.why_not(&run, idx, seat as i32).ok().flatten()
    }
    fn play(&self, cx: &mut Cx, seat: usize, card: &str) -> Flow<Dest> {
        let Some(idx) = self.ruleset.card(card) else {
            cx.log(seat as i32, Msg::new("log.card_not_ported").card("card", card));
            return Ok(Dest::Graveyard);
        };
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            seat: seat as i32,
            target: seat as i32,
            tile: -1,
            value: 0,
            move_roll: None,
            card: String::new(),
        };
        let dest = self.drive(cx, Call::Play { card: idx, seat: seat as i32 }, card, &mut trigger)?;
        Ok(dest_from(dest))
    }

    fn event(&self, cx: &mut Cx, seat: usize, id: &str) -> Flow<bool> {
        let Some(idx) = self.ruleset.card(id) else {
            cx.log(seat as i32, Msg::new("log.event_not_ported").event("event", id));
            return Ok(false);
        };
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            seat: seat as i32,
            target: seat as i32,
            tile: -1,
            value: 0,
            move_roll: None,
            card: String::new(),
        };
        let dest = self.drive(cx, Call::Play { card: idx, seat: seat as i32 }, id, &mut trigger)?;
        Ok(dest == DEST_FIELD)
    }

    fn react(&self, cx: &mut Cx, t: &mut CoreTrigger) -> Flow<()> {
        // The module's view of the trigger. `move_roll` is C# `t.Move.Roll`,
        // which a reaction may rewrite; the engine reads it back afterwards.
        let mut trigger = Trigger {
            kind: trigger_kind(t.kind),
            seat: t.seat,
            target: t.target,
            tile: t.tile,
            value: t.value,
            // Only a move trigger carries a roll; `paid`/`settle` carry value
            // as an amount, which must not read as a phantom move.
            move_roll: matches!(trigger_kind(t.kind), TriggerKind::MoveRoll | TriggerKind::Roll)
                .then_some(t.value),
            card: t.card.clone(),
        };
        // (1) The played card's own follow-up: the card named on the trigger runs
        // its `react` (C# `PlayCtx.AsReaction` for a card answering its own play).
        let own = t.card.clone();
        if !own.is_empty() {
            if let Some(idx) = self.ruleset.card(&own) {
                self.drive(cx, Call::React { card: idx, seat: t.seat }, &own, &mut trigger)?;
            }
        }
        // (2) The hand-reaction window (C# `MatchHost.React(Trigger)`): every seat
        // from the trigger's seat around the table may answer with a [反击] card
        // from hand; declarations resolve in reverse order.
        self.hand_reactions(cx, t, &mut trigger, 0)
    }
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
    #[test]
    fn every_engine_trigger_kind_maps() {
        for k in [
            "turnStart", "pass", "passSeat", "settleBefore", "settle", "mortgage",
            "pay", "paid", "bankrupt", "card", "event", "reacted", "moveRoll", "roll",
        ] {
            assert!(
                !matches!(trigger_kind(k), TriggerKind::None),
                "{k} folded to TriggerKind::None"
            );
        }
    }
}
