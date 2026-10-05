//! The card-facing operation vocabulary (C# `H.*`) over the replayable [`World`].
//!
//! A rules host (`game-rules`) runs card modules against a copy of the world and
//! calls these; they are pure state moves and log lines, so a module's replay
//! stays deterministic. Anything that needs a player decision is *not* here --
//! those go through `Cx::ask`, which halts and replays like any engine routine.

use crate::data::GameData;
use crate::msg::Msg;
use crate::state::{key, Counter, FieldCard, StateVar, Tick, TileMark};

use super::world::World;

impl World {
    // ------------------------------------------- move shaping (C# `H.*`)

    // These shape the movement being planned (C# `MoveCtx` fields, set from a
    // card's RollPlan routine). They read/write `TurnCtx::plan`, which is the
    // live routine state; the broadcast summary is `MatchState::plan`
    // (`MoveCtx::to_plan`).

    /// `SetSteps` -- the walk's length; keeps the sign of the current roll.
    pub fn set_steps(&mut self, n: i32) {
        self.turn.plan.set_steps(n);
    }
    /// `Reverse` -- walk backwards (`Dir` becomes -1).
    pub fn set_reverse(&mut self, on: bool) {
        self.turn.plan.reverse = on;
    }
    /// `Signed` -- a negative roll walks backwards instead of clamping.
    pub fn set_signed(&mut self, on: bool) {
        self.turn.plan.signed = on;
    }
    /// `StopAt` -- force the walk to stop here; `-1` clears.
    pub fn set_stop_at(&mut self, tile: i32) {
        self.turn.plan.stop_at = tile;
    }
    /// `Parity` -- restrict the walk to odd/even tiles; `-1` either.
    pub fn set_parity(&mut self, n: i32) {
        self.turn.plan.parity = n;
    }
    /// C# `m.Resolve` -- settle where the move lands. Clear it to prevent
    /// settle at all; the player still moves.
    pub fn set_resolve(&mut self, on: bool) {
        self.turn.plan.resolve = on;
    }
    /// How the move gets there: 0 = walk the path, 1 = teleport (C# `m.Teleport`).
    pub fn set_kind(&mut self, kind: i32) {
        if let Some(k) = super::move_ctx::MoveKind::from_i32(kind) {
            self.turn.plan.kind = k;
        }
    }
    /// `NoBuy` -- the walk cannot buy where it lands.
    pub fn set_no_buy(&mut self, on: bool) {
        self.turn.plan.no_buy = on;
    }
    /// `NoBuild` -- the walk cannot build where it lands.
    pub fn set_no_build(&mut self, on: bool) {
        self.turn.plan.no_build = on;
    }

    /// `MinRoll` -- clamp the final face up to this after the reactions.
    pub fn set_min_roll(&mut self, n: i32) {
        self.turn.plan.min_roll = n;
    }
    /// `ExtraSteps` -- extra steps added to the walk (it grows as it runs).
    pub fn set_extra_steps(&mut self, n: i32) {
        self.turn.plan.extra_steps = n;
    }

    /// `NoCircleReward` -- passing CiRCLE pays nothing on this walk.
    pub fn set_no_circle_reward(&mut self, on: bool) {
        self.turn.plan.no_circle_reward = on;
    }

    /// `Start` -- the tile the walk begins on instead of where the player stands
    /// (`-1` = the player's own tile). `why` names the effect on the log line.
    pub fn set_start(&mut self, tile: i32, why: &str) {
        self.turn.plan.start = tile;
        self.turn.plan.start_why = why.to_string();
    }
    /// `TeleportTo` -- the movement is a teleport to this tile (`-1` = a walk).
    /// `card_move` branches on this.
    pub fn set_teleport_to(&mut self, tile: i32) {
        self.turn.plan.teleport_to = tile;
        // Naming a destination makes it a teleport (C# `m.Teleport`).
        self.turn.plan.kind = if tile >= 0 {
            super::move_ctx::MoveKind::Teleport
        } else {
            super::move_ctx::MoveKind::Walk
        };
    }
    /// `Base` -- replace the dice the roll starts from (default 1d20). Each
    /// entry is `count`d`sides`, summed into `Parts`.
    pub fn set_base_dice(&mut self, count: i32, sides: i32, why: &str) {
        self.turn.plan.base.clear();
        if count > 0 && sides > 0 {
            self.turn.plan.base.push(crate::engine::move_ctx::Roll {
                count,
                sides,
                why: why.to_string(),
            });
        }
    }
    /// `Base` -- add one more die group to the starting dice (3d20 = three of
    /// these, or one `count = 3`).
    pub fn add_base_dice(&mut self, count: i32, sides: i32, why: &str) {
        if count > 0 && sides > 0 {
            self.turn.plan.base.push(crate::engine::move_ctx::Roll {
                count,
                sides,
                why: why.to_string(),
            });
        }
    }
    /// `Dice` -- extra dice added to the roll (summed into `Extra`).
    pub fn add_extra_dice(&mut self, count: i32, sides: i32, why: &str) {
        if count > 0 && sides > 0 {
            self.turn.plan.dice.push(crate::engine::move_ctx::Roll {
                count,
                sides,
                why: why.to_string(),
            });
        }
    }

    /// `SettleTile` -- settle here instead of the landing; -1 clears.
    pub fn set_settle_tile(&mut self, tile: i32) {
        self.turn.plan.settle_tile = tile;
    }
    /// `PayFactor` -- scale money paid for this walk. Milli-units (500 = x0.5).
    pub fn set_pay_factor(&mut self, milli: i32) {
        self.turn.plan.pay_factor = f64::from(milli) / 1000.0;
    }
    /// `RentFactor` -- scale rent paid for this walk, same units.
    pub fn set_rent_factor(&mut self, milli: i32) {
        self.turn.plan.rent_factor = f64::from(milli) / 1000.0;
    }
    /// `BuildAnywhere` -- may build away from the landing (not just on it).
    pub fn set_build_anywhere(&mut self, on: bool) {
        self.turn.plan.build_anywhere = on;
    }
    /// `SettleAsAgent` -- settle on another player's behalf.
    pub fn set_settle_as_agent(&mut self, on: bool) {
        self.turn.plan.settle_as_agent = on;
    }
    /// `MoreSteps` -- a queued second walk, in steps.
    pub fn set_more_steps(&mut self, n: i32) {
        self.turn.plan.more_steps = n;
    }
    /// `SetTag` / `Tag` -- card-owned per-move state (fire-roll counters etc.).
    pub fn set_tag(&mut self, key: &str, value: i32) {
        self.turn.plan.set_tag(key, value);
    }
    pub fn move_tag(&self, key: &str) -> i32 {
        self.turn.plan.tag(key)
    }

    // getters the cards read back off the same MoveCtx
    /// `StopAt` -- where the walk is forced to stop, or -1.
    /// Did the walk stop before its full length? The walk loop sets it.
    pub fn move_stopped(&self) -> bool {
        self.turn.plan.stopped
    }

    pub fn move_stop_at(&self) -> i32 {
        self.turn.plan.stop_at
    }
    /// `Parity` -- -1 either, 0 even, 1 odd.
    pub fn move_parity(&self) -> i32 {
        self.turn.plan.parity
    }
    /// C# `m.Resolve` -- does the planned move settle where it lands?
    pub fn move_resolve(&self) -> bool {
        self.turn.plan.resolve
    }
    /// How the move gets there: 0 = walk, 1 = teleport.
    pub fn move_kind(&self) -> i32 {
        match self.turn.plan.kind {
            super::move_ctx::MoveKind::Walk => 0,
            super::move_ctx::MoveKind::Teleport => 1,
        }
    }
    /// `Steps` -- the planned length; `MovePlan.Landing` is where it ends.
    pub fn move_steps(&self) -> i32 {
        self.turn.plan.roll
    }
    /// `Remaining` / `Total` -- how far along the walk is (C# `NoteWalk` writes
    /// `lastWalk` from `Total`, not from `Roll`: a shortened walk differs).
    pub fn move_remaining(&self) -> i32 {
        self.turn.plan.remaining
    }
    pub fn move_total(&self) -> i32 {
        self.turn.plan.total
    }
    /// `Dir` -- +1 forwards, -1 backwards.
    pub fn move_dir(&self) -> i32 {
        self.turn.plan.dir()
    }

    // ------------------------------------------------------------ queries

    pub fn player_out(&self, player_id: i32) -> bool {
        usize::try_from(player_id).is_ok_and(|i| self.st.players.get(i).is_some_and(|s| s.out()))
    }

    pub fn player_money(&self, player_id: i32) -> i32 {
        self.player_id(player_id).map_or(0, |s| s.money)
    }

    pub fn player_pos(&self, player_id: i32) -> i32 {
        self.player_id(player_id).map_or(-1, |s| s.pos)
    }

    /// The other players still in the game (`H.Others`).
    pub fn others(&self, player_id: i32) -> Vec<i32> {
        (0..self.st.players.len() as i32)
            .filter(|&i| i != player_id && !self.player_out(i))
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

    pub fn owned_tiles(&self, player_id: i32) -> Vec<i32> {
        (0..self.st.owners.len() as i32).filter(|&t| self.tile_owner(t) == player_id).collect()
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

    /// The tile `steps` ahead of a player without passing others' logic
    /// (`H.NearestAhead`-lite): pure geometry on the ring.
    pub fn tile_steps_ahead(&self, data: &GameData, player_id: i32, steps: i32) -> i32 {
        let n = data.tiles.len() as i32;
        let pos = self.player_pos(player_id);
        if n <= 0 || pos < 0 {
            return -1;
        }
        ((pos + steps) % n + n) % n
    }

    // -------------------------------------------------------- player slots (V)

    /// C# `TurnCtx.NoMoneyLoss` -- this player's money cannot drop this turn.
    pub fn money_locked(&self, player_id: i32) -> bool {
        usize::try_from(player_id).is_ok_and(|s| self.turn.no_money_loss.contains(&s))
    }

    // ------------------------------------------------------ keyed state map

    /// Per-player keyed state as `{value, min, max, expires}` items (C#
    /// `MatchPlayer`'s pots plus the `H.V` slots). The engine holds these and
    /// enforces nothing -- see [`crate::state::StateVar`]. There is no
    /// `match key` here and there must never be: which keys mean what, and
    /// what their caps are, belongs to whoever uses them. The `stun_of` / `fire`
    /// / ... accessors below are readers and verbs over this, not the other way
    /// round.

    /// The whole item (`StateVar::default()` when the key is absent).
    pub fn state_var(&self, player_id: i32, key: &str) -> StateVar {
        self.player_id(player_id).map_or_else(StateVar::default, |s| s.state_var(key))
    }

    pub fn state_get(&self, player_id: i32, key: &str) -> i32 {
        self.state_var(player_id, key).value
    }

    pub fn state_min(&self, player_id: i32, key: &str) -> i32 {
        self.state_var(player_id, key).min
    }

    /// The cap a consumer may enforce -- `0` when none has been mandated.
    pub fn state_max(&self, player_id: i32, key: &str) -> i32 {
        self.state_var(player_id, key).max
    }

    /// When this counter wears off (see [`crate::state::Tick`]), if ever.
    pub fn state_expires(&self, player_id: i32, key: &str) -> Option<Tick> {
        self.state_var(player_id, key).expires
    }

    /// Write `value` raw and return it. **Not** clamped to `min`/`max`: the
    /// engine is the holder, not the enforcer.
    pub fn state_set(&mut self, player_id: i32, key: &str, value: i32) -> i32 {
        let Some(s) = self.player_mut(player_id) else { return 0 };
        s.state_set(key, value)
    }

    /// Add `delta` raw and return the value now stored. No clamping, no log.
    pub fn state_add(&mut self, player_id: i32, key: &str, delta: i32) -> i32 {
        let Some(s) = self.player_mut(player_id) else { return 0 };
        s.state_add(key, delta)
    }

    /// Declare the bounds a consumer may enforce. Stored, not applied.
    pub fn state_set_bounds(&mut self, player_id: i32, key: &str, min: i32, max: i32) {
        if let Some(s) = self.player_mut(player_id) {
            s.state_set_bounds(key, min, max);
        }
    }

    /// Declare when this counter wears off (see [`crate::state::Tick`]).
    pub fn state_set_expires(&mut self, player_id: i32, key: &str, expires: Option<Tick>) {
        if let Some(s) = self.player_mut(player_id) {
            s.state_set_expires(key, expires);
        }
    }

    /// Tick every timed counter whose expiry is due. No key names -- the item
    /// says when it wears off.
    pub fn tick_state(&mut self, player_id: i32, when: Tick) -> Vec<(String, i32)> {
        self.player_mut(player_id).map_or_else(Vec::new, |s| s.tick_state(when))
    }

    // -------------------------------------------------- free-form slots (V)

    /// C# `H.V` -- a free-form per-player counter. Sugar over the keyed map.
    pub fn slot(&self, player_id: i32, key: &str) -> i32 {
        self.state_get(player_id, key)
    }

    pub fn set_slot(&mut self, player_id: i32, key: &str, value: i32) {
        self.state_set(player_id, key, value);
    }

    pub fn inc_slot(&mut self, player_id: i32, key: &str, by: i32) -> i32 {
        self.state_add(player_id, key, by)
    }

    // ------------------------------------------------------------- tokens

    /// Board markers (C# `Counter`), kept out of the keyed state on purpose:
    /// they render on the board rather than being a value a rule enforces.
    pub fn tok(&self, player_id: i32, name: &str) -> i32 {
        self.player_id(player_id).and_then(|s| s.tokens.iter().find(|c| c.name == name)).map_or(0, |c| c.value)
    }

    pub fn set_tok(&mut self, player_id: i32, name: &str, value: i32) {
        if let Some(s) = self.player_mut(player_id) {
            set_named(&mut s.tokens, name, value);
        }
    }

    /// Returns how much it actually moved by (`H.AddTok`). This is a *consumer*
    /// of the cap passed in -- the engine is not the one deciding to clamp.
    pub fn add_tok(&mut self, player_id: i32, name: &str, n: i32, max: i32) -> i32 {
        let was = self.tok(player_id, name);
        let now = (was + n).clamp(0, max);
        self.set_tok(player_id, name, now);
        now - was
    }

    // -------------------------------------------------------- band crystals

    pub fn band_crystals(&self, player_id: i32) -> i32 {
        self.state_get(player_id, key::BAND_CRYSTALS)
    }

    /// C# `AddBandCrystals` -- a consumer of the passed cap, not the engine
    /// deciding one. Returns how much it actually moved by.
    pub fn add_band_crystals(&mut self, player_id: i32, n: i32, max: i32) -> i32 {
        let was = self.band_crystals(player_id);
        let now = (was + n).clamp(0, max);
        self.state_set(player_id, key::BAND_CRYSTALS, now);
        now - was
    }

    // ------------------------------------------------------------- fire pots

    /// Fire pots held (C# `fire`).
    pub fn fire(&self, player_id: i32) -> i32 {
        self.state_get(player_id, key::FIRE)
    }

    /// The mandated fire-pot cap (C# `fireMax`) -- the `max` of the `fire` item,
    /// which a character skill writes. The engine never imposes it.
    pub fn fire_max(&self, player_id: i32) -> i32 {
        self.state_max(player_id, key::FIRE)
    }

    /// `Card.FireMaxDelta` -- move the *cap* up or down (C# `FireMaxDelta`).
    /// A consumer of the cap, so it is the one that pulls the value back under
    /// the new ceiling; [`Self::state_add`] would not.
    pub fn add_fire_max(&mut self, player_id: i32, n: i32) -> i32 {
        let cap = self.fire_max(player_id).saturating_add(n).max(0);
        self.state_set_bounds(player_id, key::FIRE, 0, cap);
        let have = self.fire(player_id).min(cap);
        self.state_set(player_id, key::FIRE, have);
        cap
    }

    /// Gained `n` (at most up to the mandated cap) and logged it; 0 if nothing.
    /// This is a consumer choosing to honour [`Self::fire_max`].
    pub fn gain_fire(&mut self, player_id: i32, n: i32, why: Msg) -> i32 {
        let Some(s) = self.player_mut(player_id) else { return 0 };
        if n <= 0 || s.out() {
            return 0;
        }
        let cap = s.fire_max();
        if cap <= 0 {
            return 0;
        }
        let got = n.clamp(0, (cap - s.fire()).max(0));
        if got <= 0 {
            return 0;
        }
        s.state_add(key::FIRE, got);
        let (fire, name) = (s.fire(), s.player.clone());
        self.log("fire", player_id, Msg::new("log.gain_fire").text("who", name).i("n", got).msg("why", why).i("fire", fire).i("max", cap));
        got
    }

    // ---------------------------------------------------------- money moves

    /// `H.GainR` -- money in, logged with its reason.
    pub fn gain_money(&mut self, player_id: i32, amount: i32, src: Msg) -> i32 {
        let Some(s) = self.player_mut(player_id) else { return 0 };
        if amount <= 0 || s.out() {
            return 0;
        }
        s.money += amount;
        self.log("gain", player_id, Msg::new("log.gain").player_id("who", player_id).n("amount", amount).msg("src", Msg::new("log.part.why").msg("why", src)));
        amount
    }

    /// `H.PayR` -- money out, logged. Routines that may need to raise funds are
    /// driven through `Cx` instead (they prompt).
    pub fn pay_money(&mut self, player_id: i32, amount: i32, src: Msg) -> i32 {
        if amount > 0 && self.money_locked(player_id) {
            self.log("text", player_id, Msg::new("log.money_locked").player_id("who", player_id).n("amount", amount));
            return 0;
        }
        let Some(s) = self.player_mut(player_id) else { return 0 };
        if amount <= 0 || s.out() {
            return 0;
        }
        let paid = amount.min(s.money);
        s.money -= paid;
        self.log("lose", player_id, Msg::new("log.lose").player_id("who", player_id).n("amount", paid).msg("src", Msg::new("log.part.why").msg("why", src)));
        paid
    }

    // ------------------------------------------------------------ card flow

    /// `H.DrawR` -- draw `n` cards, reshuffling the discard pile when needed.
    /// Returns how many were actually drawn.
    pub fn draw_cards(&mut self, player_id: i32, n: i32, over_hand_limit: bool) -> i32 {
        let Some(i) = usize::try_from(player_id).ok() else { return 0 };
        if self.st.players.get(i).is_none_or(|s| s.out()) {
            return 0;
        }
        let mut got = 0;
        for _ in 0..n.max(0) {
            if self.hidden[i].draw.is_empty() && !self.hidden[i].discard.is_empty() {
                let mut pile = std::mem::take(&mut self.hidden[i].discard);
                self.rng.shuffle(&mut pile);
                self.hidden[i].draw = pile;
                self.log("text", player_id, Msg::new("log.reshuffle").player_id("who", player_id));
            }
            let Some(card) = self.hidden[i].draw.pop() else { break };
            self.hidden[i].hand.push(card);
            got += 1;
        }
        if got > 0 {
            self.log("draw", player_id, Msg::new("log.draw").player_id("who", player_id).i("n", got));
        }
        let _ = over_hand_limit;
        got
    }

    /// `H.AddToHand` / `AddToDeck` / `ToDiscard` -- move a card id between zones.
    pub fn add_to_hand(&mut self, player_id: i32, card: &str) {
        if let Ok(i) = usize::try_from(player_id) {
            if let Some(h) = self.hidden.get_mut(i) {
                h.hand.push(card.to_string());
            }
        }
    }

    pub fn add_to_deck(&mut self, player_id: i32, card: &str, shuffle: bool) {
        if let Ok(i) = usize::try_from(player_id) {
            if let Some(h) = self.hidden.get_mut(i) {
                h.draw.push(card.to_string());
                if shuffle {
                    self.rng.shuffle(&mut h.draw);
                }
            }
        }
    }

    pub fn to_discard(&mut self, player_id: i32, card: &str) {
        if let Ok(i) = usize::try_from(player_id) {
            if let Some(h) = self.hidden.get_mut(i) {
                if let Some(k) = h.hand.iter().position(|c| c == card) {
                    h.hand.remove(k);
                }
                h.discard.push(card.to_string());
            }
        }
    }

    // -------------------------------------------------------- placed cards

    /// `H.PlaceFromPlay` -- the card becomes a field card at the player.
    pub fn place_card(&mut self, player_id: i32, card: &str, note: Msg) {
        let Some(s) = self.player_mut(player_id) else { return };
        let uid = s.field.iter().map(|f| f.uid).max().unwrap_or(0) + 1;
        s.field.push(FieldCard { uid, card: card.to_string(), owner: player_id, user: player_id, tile: -1, crystals: 0, face_down: false, note });
    }

    /// `H.Unplace` -- take a field card off; returns its card id.
    pub fn unplace_card(&mut self, player_id: i32, card: &str) -> Option<String> {
        let s = self.player_mut(player_id)?;
        let k = s.field.iter().position(|f| f.card == card)?;
        Some(s.field.remove(k).card)
    }

    pub fn placed_cards(&self, player_id: i32) -> Vec<String> {
        self.player_id(player_id).map_or_else(Vec::new, |s| s.field.iter().map(|f| f.card.clone()).collect())
    }

    /// Miracle crystals on a placed card (C# `Card.Crystals`).
    pub fn card_crystals(&self, player_id: i32, card: &str) -> i32 {
        self.player_id(player_id)
            .and_then(|s| s.field.iter().find(|f| f.card == card))
            .map_or(0, |f| f.crystals)
    }

    /// `H.AddCrystals` on a placed card; `max` caps the result (0 = uncapped).
    pub fn add_card_crystals(&mut self, player_id: i32, card: &str, n: i32, max: i32) -> i32 {
        let Some(f) = self.player_mut(player_id).and_then(|s| s.field.iter_mut().find(|f| f.card == card)) else {
            return 0;
        };
        f.crystals = (f.crystals + n).max(0);
        if max > 0 {
            f.crystals = f.crystals.min(max);
        }
        f.crystals
    }

    pub fn set_card_crystals(&mut self, player_id: i32, card: &str, n: i32) {
        self.add_card_crystals(player_id, card, n - self.card_crystals(player_id, card), 0);
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

    pub fn add_mark(&mut self, tile: i32, player_id: i32, kind: &str, note: Msg) {
        let uid = self.st.marks.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
        self.st.marks.push(TileMark { uid, tile, kind: kind.to_string(), owner: player_id, count: 1, card: String::new(), note });
    }

    // ------------------------------------------------------ status effects

    /// `H.GiveStay` / `GiveStun` / `GiveExile` -- layered status on a player.
    ///
    /// These are *consumers* of the keyed state, and the two that are timed say
    /// so on the item (`expires: TurnEnd`) rather than the engine remembering
    /// which keys wear off. They clamp at 0 because that is the status rule,
    /// not because the engine enforces bounds -- [`Self::state_add`] would not.
    pub fn give_stay(&mut self, player_id: i32, n: i32) {
        if let Some(s) = self.player_mut(player_id) {
            let v = (s.stay() + n).max(0);
            s.state_set(key::STAY, v);
            s.state_set_expires(key::STAY, Some(Tick::TurnEnd));
        }
    }

    pub fn give_stun(&mut self, player_id: i32, n: i32) {
        if let Some(s) = self.player_mut(player_id) {
            let v = (s.stun() + n).max(0);
            s.state_set(key::STUN, v);
            s.state_set_expires(key::STUN, Some(Tick::TurnEnd));
        }
    }

    pub fn give_exile(&mut self, player_id: i32, n: i32, to: i32) {
        if let Some(s) = self.player_mut(player_id) {
            let v = (s.exile() + n).max(0);
            s.state_set(key::EXILE, v);
            s.state_set(key::EXILE_TO, to);
        }
    }

    /// `H.GiveExtraTurn` -- the player gets another turn after this one.
    pub fn give_extra_turn(&mut self, player_id: usize) {
        if !self.extra_turns.contains(&player_id) {
            self.extra_turns.push(player_id);
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

    /// `H.ForceTeleport(..., resolve: false)` -- move a player without settling.
    pub fn teleport_to(&mut self, player_id: i32, tile: i32) {
        if let Some(s) = self.player_mut(player_id) {
            s.pos = tile;
        }
    }

    // --------------------------------------------------------------- misc

    /// `H.Touch` -- something changed that only host-side state cares about.
    pub fn touch(&mut self) {
        self.st.seq += 1;
    }

    fn player_id(&self, player_id: i32) -> Option<&crate::state::MatchPlayer> {
        usize::try_from(player_id).ok().and_then(|i| self.st.players.get(i))
    }

    fn player_mut(&mut self, player_id: i32) -> Option<&mut crate::state::MatchPlayer> {
        usize::try_from(player_id).ok().and_then(|i| self.st.players.get_mut(i))
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
