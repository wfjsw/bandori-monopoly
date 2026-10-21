//! The card-facing operation vocabulary over the replayable [`World`].
//!
//! A rules host (`game-rules`) runs card modules against a copy of the world and
//! calls these; they are pure state moves and log lines, so a module's replay
//! remains deterministic. Anything that needs a player decision is *not* here --
//! those go through `Cx::ask`, which halts and replays like any engine routine.

use std::collections::BTreeMap;

use crate::data::GameData;
use crate::msg::Msg;
use crate::state::{key, Counter, FieldCard, MarkFilter, StateVar, Tick, TileMark};

use super::world::World;
use crate::engine::world::Extreme;

impl World {
    // ------------------------------------------- move shaping

    // These shape the movement being planned (set from a card's DiceRollPlan
    // routine). They read/write `TurnCtx::plan`, which is the live routine
    // state; the broadcast summary is `MatchState::plan` (`MoveCtx::to_plan`).

    /// The walk's length; keeps the sign of the current roll.
    pub fn set_steps(&mut self, n: i32) {
        self.turn.plan.set_steps(n);
    }
    /// Walk backwards (direction becomes -1).
    pub fn set_reverse(&mut self, on: bool) {
        self.turn.plan.reverse = on;
    }
    /// A negative roll walks backwards instead of clamping.
    pub fn set_signed(&mut self, on: bool) {
        self.turn.plan.signed = on;
    }
    /// Force the walk to stop here; `None` clears.
    pub fn set_stop_at(&mut self, tile: Option<game_kinds::Tile>) {
        self.turn.plan.stop_at = tile;
    }
    /// Restrict the walk to odd/even tiles; `-1` either.
    pub fn set_parity(&mut self, n: i32) {
        self.turn.plan.parity = n;
    }
    /// Settle at the [move end] (「到达终点后是否settle」). Clear it to skip
    /// settle entirely; the player still moves.
    pub fn set_resolve(&mut self, on: bool) {
        self.turn.plan.resolve = on;
    }
    /// How the move gets there: 0 = walk the path, 1 = teleport.
    pub fn set_kind(&mut self, kind: i32) {
        if let Some(k) = super::move_ctx::MoveKind::from_i32(kind) {
            self.turn.plan.kind = k;
        }
    }
    /// The walk cannot buy where it lands.
    pub fn set_no_buy(&mut self, on: bool) {
        self.turn.plan.no_buy = on;
    }
    /// The walk cannot build where it lands.

    /// Clamp the final face up to this after the counteractions. `None` = no
    /// floor.
    pub fn set_min_roll(&mut self, n: Option<i32>) {
        self.turn.plan.min_roll = n;
    }
    /// Extra steps added to the walk (it grows as it runs).
    pub fn set_extra_steps(&mut self, n: i32) {
        self.turn.plan.extra_steps = n;
    }

    /// Passing the start square pays nothing on this walk.

    /// The tile the walk begins on instead of where the player stands
    /// (`None` = the player's own tile). `why` names the effect on the log line.
    pub fn set_start(&mut self, tile: Option<game_kinds::Tile>, why: &str) {
        self.turn.plan.start = tile;
        self.turn.plan.start_why = why.to_string();
    }
    /// The movement is a teleport to this tile (`None` = a walk / clear).
    /// `card_move` branches on this.
    pub fn set_teleport_to(&mut self, tile: Option<game_kinds::Tile>) {
        self.turn.plan.teleport_to = tile;
        // Naming a destination makes it a teleport.
        self.turn.plan.kind = if tile.is_some() {
            super::move_ctx::MoveKind::Teleport
        } else {
            super::move_ctx::MoveKind::Walk
        };
    }
    /// Replace the dice the roll starts from (default 1d20). Each
    /// entry is `count`d`sides`, summed into the face. `sides == 0` is a flat
    /// `count` (see [`crate::engine::move_ctx::Roll`]).
    pub fn set_base_dice(&mut self, count: i32, sides: i32, why: &str) {
        self.turn.plan.base.clear();
        if count > 0 && sides >= 0 {
            self.turn.plan.base.push(crate::engine::move_ctx::Roll::new(
                count, sides, why,
            ));
        }
    }
    /// Add one more die group to the starting dice (3d20 = three of
    /// these, or one `count = 3`). `sides == 0` is a flat `count`.
    pub fn add_base_dice(&mut self, count: i32, sides: i32, why: &str) {
        if count > 0 && sides >= 0 {
            self.turn.plan.base.push(crate::engine::move_ctx::Roll::new(
                count, sides, why,
            ));
        }
    }
    /// Drop every extra die another effect added. A rewrite that
    /// names the whole face (「骰点就是1d10」) means *exactly* that face, not
    /// 1d10 plus whatever else is stacked on.
    pub fn clear_dice(&mut self) {
        self.turn.plan.dice.clear();
    }
    /// Extra dice added to the roll (summed into the face). A flat add
    /// is a `0`-sided term: `add_extra_dice(n, 0, why)`.
    pub fn add_extra_dice(&mut self, count: i32, sides: i32, why: &str) {
        if count > 0 && sides >= 0 {
            self.turn.plan.dice.push(crate::engine::move_ctx::Roll::new(
                count, sides, why,
            ));
        }
    }

    /// Settle here instead of the landing; -1 clears.
    pub fn set_settle_tile(&mut self, tile: i32) {
        self.turn.plan.settle_tile = tile;
    }
    /// Scale money paid for this walk. Milli-units (500 = x0.5).
    pub fn set_pay_factor(&mut self, milli: i32) {
        self.turn.plan.pay_factor = f64::from(milli) / 1000.0;
    }
    /// Scale rent paid for this walk, same units.
    pub fn set_rent_factor(&mut self, milli: i32) {
        self.turn.plan.rent_factor = f64::from(milli) / 1000.0;
    }
    /// May build away from the landing (not just on it).
    pub fn set_can_build(&mut self, on: bool) {
        self.turn.plan.can_build = on;
    }
    /// `ctx::plan::set_no_build` (HOST-08): folded into `can_build = !on` --
    /// one flag is enough (`world.rs`).
    pub fn set_no_build(&mut self, on: bool) {
        self.turn.plan.can_build = !on;
    }
    /// `ctx::plan::set_build_anywhere` (HOST-08): this move may build away
    /// from the landing.
    pub fn set_build_anywhere(&mut self, on: bool) {
        self.turn.plan.build_anywhere = on;
    }
    /// Settle on another player's behalf.
    pub fn set_settle_as_agent(&mut self, on: bool) {
        self.turn.plan.settle_as_agent = on;
    }
    /// 「使你的下次主要移动结果对那些玩家一起执行」 -- record a follower of the
    /// move being planned. After the mover settles, the engine replays this
    /// move's result for each follower in the order recorded.
    pub fn plan_add_follower(&mut self, player_id: i32) {
        if player_id >= 0 && !self.turn.plan.followers.contains(&player_id) {
            self.turn.plan.followers.push(player_id);
        }
    }
    /// A queued second walk, in steps.
    pub fn set_more_steps(&mut self, n: i32) {
        self.turn.plan.more_steps = n;
    }
    /// Card-owned per-move state (resource-roll counters etc.).
    pub fn set_tag(&mut self, key: &str, value: i32) {
        self.turn.plan.set_tag(key, value);
    }
    pub fn move_tag(&self, key: &str) -> i32 {
        self.turn.plan.tag(key)
    }

    // getters the cards read back off the same move plan
    /// Did the walk stop before its full length? The walk loop sets it.
    pub fn move_stopped(&self) -> bool {
        self.turn.plan.stopped
    }

    /// Where the walk is forced to stop, or -1.
    pub fn move_stop_at(&self) -> Option<game_kinds::Tile> {
        self.turn.plan.stop_at
    }
    /// -1 either, 0 even, 1 odd.
    pub fn move_parity(&self) -> i32 {
        self.turn.plan.parity
    }
    /// Does the planned move settle at its [move end]?
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
    /// The planned length; [`crate::state::MovePlan::landing`] is where it ends.
    pub fn move_steps(&self) -> i32 {
        self.turn.plan.roll
    }
    /// How far along the walk is: steps left, and the path
    /// length (the walk's length, not the face that was rolled -- a
    /// shortened walk differs).
    pub fn move_remaining(&self) -> i32 {
        self.turn.plan.remaining
    }
    /// The walk's full length (not the rolled face).
    pub fn move_total(&self) -> i32 {
        self.turn.plan.total
    }
    /// +1 forwards, -1 backwards.
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

    /// The other players still in the game.
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
        usize::try_from(tile)
            .ok()
            .and_then(|i| self.st.owners.get(i))
            .copied()
            .unwrap_or(-1)
    }

    pub fn owned_tiles(&self, player_id: i32) -> Vec<i32> {
        (0..self.st.owners.len() as i32)
            .filter(|&t| self.tile_owner(t) == player_id)
            .collect()
    }

    /// The house count a **rent** lookup reads. Real
    /// `st.houses` is untouched by the override -- build caps, raze, sale and
    /// asset value all still see the standing houses. See
    /// [`crate::state::prop::RENT_HOUSES`]: presence on the tile's rule
    /// instance (`ctx::set_tile_prop`) or on a placed card of the tile's
    /// *owner* (`ctx::set_prop`, 「你的所有格子上的房屋数视为…」 -- gone with
    /// the card) is the override; otherwise the standing count.
    pub fn rent_houses(&self, tile: i32) -> i32 {
        let Ok(t) = usize::try_from(tile) else {
            return 0;
        };
        // A tile-scoped override wins (the source arms and disarms it).
        for f in self.st.board_field.iter().filter(|f| f.tile == tile) {
            if let Some(&v) = f.props.get(crate::state::prop::RENT_HOUSES) {
                return v.max(0);
            }
        }
        // Then a placed card of the owner (「你的所有格子」).
        if let Some(&o) = self.st.owners.get(t) {
            if let Ok(o) = usize::try_from(o) {
                if let Some(p) = self.st.players.get(o) {
                    for f in &p.field {
                        if let Some(&v) = f.props.get(crate::state::prop::RENT_HOUSES) {
                            return v.max(0);
                        }
                    }
                }
            }
        }
        self.st.houses.get(t).copied().unwrap_or(0)
    }

    /// Rent of a tile as it stands right now (houses included).
    /// The house count is the **counted** one ([`Self::rent_houses`]).
    pub fn rent_of(&self, data: &GameData, tile: i32) -> i32 {
        let Ok(i) = usize::try_from(tile) else {
            return 0;
        };
        let Some(t) = data.tiles.get(i) else {
            return 0;
        };
        // Dice rent (`prop::RENT_DICE_MULT`) is rolled at payment time; the
        // unit (`count_owned_kind × mult`) is the quoted base.
        let mult = self.tile_prop(tile, crate::state::prop::RENT_DICE_MULT);
        if mult > 0 {
            return super::play::purchase::dice_rent_unit(data, self, i, t.kind.as_str(), mult);
        }
        super::play::purchase::table_rent(data, self, i)
    }

    /// Price to buy a tile now (land + houses standing on it).
    /// [`super::play::purchase::quote_native`] is the single source.
    pub fn buy_price(&self, data: &GameData, tile: i32) -> i32 {
        let Ok(t) = usize::try_from(tile) else {
            return 0;
        };
        super::play::purchase::quote_native(data, &self.st, t).max(0)
    }

    /// [`super::play::purchase::build_cost`] is the single source.
    pub fn build_cost(&self, data: &GameData, tile: i32) -> i32 {
        let Ok(t) = usize::try_from(tile) else {
            return 0;
        };
        super::play::purchase::build_cost(data, t)
    }

    /// [`super::play::purchase::mortgage_value`] is the single source.
    pub fn mortgage_value(&self, data: &GameData, tile: i32) -> i32 {
        let Ok(t) = usize::try_from(tile) else {
            return 0;
        };
        super::play::purchase::mortgage_value(data, t)
    }

    /// Cash plus mortgageable deeds; construction cannot fund itself.
    pub(crate) fn purchase_funds(
        &self,
        data: &GameData,
        player: usize,
        exclude: Option<usize>,
    ) -> i32 {
        self.st.players[player].money
            + data.tiles.iter().enumerate()
                .filter(|&(t, tile)| {
                    Some(t) != exclude
                        && tile.is_buyable()
                        && self.tile_prop(t as i32, crate::state::prop::NO_MORTGAGE) == 0
                        && self.st.owners[t] == player as i32
                        && !self.st.mortgaged[t]
                })
                .map(|(_, tile)| tile.price / 2)
                .sum::<i32>()
    }

    /// The tile `steps` ahead of a player as pure geometry on the board loop -- no
    /// walk shaping, just wrap.
    pub fn tile_steps_ahead(&self, data: &GameData, player_id: i32, steps: i32) -> i32 {
        let n = data.tiles.len() as i32;
        let pos = self.player_pos(player_id);
        if n <= 0 || pos < 0 {
            return -1;
        }
        ((pos + steps) % n + n) % n
    }

    // -------------------------------------------------------- player slots (V)

    /// This player's money cannot drop this turn.
    pub fn money_locked(&self, player_id: i32) -> bool {
        usize::try_from(player_id).is_ok_and(|s| self.turn.no_money_loss.contains(&s))
    }

    // ------------------------------------------------------ keyed state map

    /// Per-player keyed state as `{value, min, max, expires}` items (status
    /// counters, spendable resources, free-form slots). The engine holds these and
    /// enforces nothing -- see [`crate::state::StateVar`]. There is no
    /// `match key` here and there must never be: which keys mean what, and
    /// what their caps are, belongs to whoever uses them. The status-layer / marker
    /// / ... accessors below are readers and verbs over this, not the other way
    /// round.

    /// The whole item (`StateVar::default()` when the key is absent).
    pub fn state_var(&self, player_id: i32, key: &str) -> StateVar {
        self.player_id(player_id)
            .map_or_else(StateVar::default, |s| s.state_var(key))
    }

    pub fn state_get(&self, player_id: i32, key: &str) -> i32 {
        self.state_var(player_id, key).value
    }

    /// Every `(key, value)` on this seat. Opaque names; the engine does not
    /// interpret them.
    pub fn state_pairs(&self, player_id: i32) -> Vec<(String, i32)> {
        self.player_id(player_id)
            .map(|s| s.state.iter().map(|(k, v)| (k.clone(), v.value)).collect())
            .unwrap_or_default()
    }

    /// Declare the generic [`crate::state::state_flag`] bits for a key.
    pub fn state_set_flags(&mut self, player_id: i32, key: &str, flags: u8) {
        if let Some(s) = self.player_mut(player_id) {
            s.state_set_flags(key, flags);
        }
    }

    pub fn state_min(&self, player_id: i32, key: &str) -> i32 {
        self.state_var(player_id, key).min
    }

    /// The advertised cap (`0` when none has been mandated). Display-only:
    /// the engine stores it and does not police it.
    pub fn state_max(&self, player_id: i32, key: &str) -> i32 {
        self.state_var(player_id, key).max
    }

    /// When this counter wears off (see [`crate::state::Tick`]), if ever.
    pub fn state_expires(&self, player_id: i32, key: &str) -> Option<Tick> {
        self.state_var(player_id, key).expires
    }

    /// Write `value`, honouring the writer-declared `NEVER_NEGATIVE` floor.
    /// The advertised `max` is not enforced here. See
    /// [`crate::state::MatchPlayer::state_set`].
    pub fn state_set(&mut self, player_id: i32, key: &str, value: i32) -> i32 {
        let Some(s) = self.player_mut(player_id) else {
            return 0;
        };
        s.state_set(key, value)
    }

    /// Add `delta`, floored like [`Self::state_set`]. No log.
    pub fn state_add(&mut self, player_id: i32, key: &str, delta: i32) -> i32 {
        let Some(s) = self.player_mut(player_id) else {
            return 0;
        };
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
        self.player_mut(player_id)
            .map_or_else(Vec::new, |s| s.tick_state(when))
    }

    // -------------------------------------------------- free-form slots (V)

    /// A free-form per-player counter. Sugar over the keyed map.
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

    /// Board markers, kept out of the keyed state on purpose: they render on
    /// the board rather than being a value a rule enforces.
    /// Names of the player's counters whose name starts with `prefix`, in the
    /// order they were added. The listing half of a counter query -- [`Self::tok`]
    /// reads one of them by name.
    /// Sum the move plan's `base` + `dice` tables into one
    /// face. Flat terms (`sides <= 0`) add their `count` directly, which is how a
    /// 「+2 to the roll」 effect rides along instead of a separate bonus field.
    /// Honours [`TurnCtx::extreme`]: a forced extreme settles the whole table at
    /// its theoretical max or min instead of rolling it.
    pub fn do_move_roll(&mut self, _player_id: i32) -> i32 {
        let plan = self.turn.plan.clone();
        if self.turn.extreme != Extreme::Plain {
            let mut lo = 0;
            let mut hi = 0;
            for t in plan.base.iter().chain(plan.dice.iter()) {
                if t.dice.sides <= 0 {
                    lo += t.dice.count;
                    hi += t.dice.count;
                } else {
                    lo += t.dice.count.max(0);
                    hi += t.dice.count.max(0) * t.dice.sides.max(1);
                }
            }
            return if self.turn.extreme == Extreme::Max { hi } else { lo };
        }
        let mut total = 0;
        for t in plan.base.iter().chain(plan.dice.iter()) {
            if t.dice.sides <= 0 {
                total += t.dice.count;
            } else {
                for _ in 0..t.dice.count.max(0) {
                    total += self.rng.d(t.dice.sides.max(1));
                }
            }
        }
        self.turn.turn_rolls.push(total);
        total
    }

    /// One `count`d`sides` roll, honouring [`TurnCtx::extreme`].
    pub fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32 {
        let total = self.roll_silent(player_id, count, sides);
        self.log(
            "dice",
            player_id,
            crate::msg::Msg::new("log.dice")
                .player_id("who", player_id)
                .i("count", count)
                .i("sides", sides)
                .opt("what", None)
                .i("sum", total)
                .opt("detail", None),
        )
        // The event carries the result so the client can show it on the dice
        // rather than only in the log line (same as `Cx::roll`).
        .value = total;
        total
    }

    /// [`Self::roll`] without the `"dice"` event -- for a caller that posts the
    /// event itself (write-through) and must not double-log.
    pub fn roll_silent(&mut self, player_id: i32, count: i32, sides: i32) -> i32 {
        let _ = player_id;
        let count = count.max(0);
        let sides = sides.max(1);
        let total = if self.turn.extreme == Extreme::Max {
            count * sides
        } else if self.turn.extreme == Extreme::Min {
            count
        } else {
            let mut sum = 0;
            for _ in 0..count {
                sum += self.rng.d(sides);
            }
            sum
        };
        self.turn.turn_rolls.push(total);
        total
    }

    /// Money moves, but no skill or crit may bend the figure
    /// (「立刻获得此次失去的资金金额」).
    ///
    /// The ledger still closes: the movement is logged as a `gain` event with
    /// its bank leg (`value`), even though it bypasses the `paySplitAdd`/`paySplitMul`/
    /// `paySplitChoose` modifier pipeline.
    pub fn gain_fixed(&mut self, player_id: i32, amount: i32, why: crate::msg::Msg) -> i32 {
        if amount == 0 {
            return 0;
        }
        if let Some(s) = self.player_mut(player_id) {
            s.money += amount;
        }
        // 「当前回合内你每获得过一次资金」 -- only money **in** counts; a
        // negative `gain_fixed` is a loss.
        if amount > 0 {
            self.bump_gain(player_id, 1);
        }
        let e = self.log("gain", player_id, why);
        e.value = amount;
        amount
    }

    pub fn tok_names(&self, player_id: i32, prefix: &str) -> Vec<String> {
        self.player_id(player_id)
            .map(|s| {
                s.tokens
                    .iter()
                    .filter(|c| c.name.starts_with(prefix) && c.value > 0)
                    .map(|c| c.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn tok(&self, player_id: i32, name: &str) -> i32 {
        self.player_id(player_id)
            .and_then(|s| s.tokens.iter().find(|c| c.name == name))
            .map_or(0, |c| c.value)
    }

    /// Set a name-keyed held counter. `instance` is the owning card instance
    /// stamped on a **new** row (user ruling 2026-10-10: every held unit is a
    /// unit of some instance's counter). An existing row keeps its instance.
    pub fn set_tok(&mut self, player_id: i32, name: &str, value: i32, instance: i32) {
        if let Some(s) = self.player_mut(player_id) {
            set_named(&mut s.tokens, name, value, instance);
        }
    }

    /// Returns how much it actually moved by. This is a *consumer*
    /// of the cap passed in -- the engine is not the one deciding to clamp.
    /// `instance` is stamped on a new row.
    pub fn add_tok(&mut self, player_id: i32, name: &str, n: i32, max: i32, instance: i32) -> i32 {
        let was = self.tok(player_id, name);
        let now = (was + n).clamp(0, max);
        self.set_tok(player_id, name, now, instance);
        now - was
    }

    // -------------------------------------------------------- group named counters

    /// The uid of `player_id`'s **group-skill field instance** -- the bound group
    /// skill's field card (`skill:<group>:<skill>`, [`FieldCard::group_skill`]),
    /// which is where the group-skill named counters live. -1 when the player has
    /// none (no group skill bound, or it has left the field).
    ///
    /// Edge cases (the write target is always this one instance):
    /// * **no group skill** -- every read is 0 and every write is a no-op;
    /// * **several group cards** (PPP:Returns borrows another group's card beside
    ///   its own; the book's 「this effect does not re-trigger with several group cards」 is about several
    ///   *players* holding the same group card) -- the **first in placement
    ///   order** is the bound one (`bind_skills` places it first; a later copy
    ///   is a borrow that has not replaced it);
    /// * **swapped or removed** -- the instance goes with its named counters, so a
    ///   swap starts the new card at 0 and the old count is gone with the old
    ///   card (Returns' 「replace and remove all counters on it」 is exactly that).
    pub fn group_skill_uid(&self, player_id: i32) -> i32 {
        self.player_id(player_id)
            .and_then(|s| s.field.iter().find(|f| f.group_skill))
            .map_or(-1, |f| f.uid)
    }

    /// A named pool on the player's group-skill field instance. 0 when there
    /// is no group skill. Same pool every group skill and every card that
    /// names that pool reads and writes.
    pub fn group_pool(&self, player_id: i32, name: &str) -> i32 {
        let uid = self.group_skill_uid(player_id);
        if uid < 0 {
            0
        } else {
            self.counter_at(uid, name)
        }
    }

    /// Sum of every named counter on the group-skill instance.
    pub fn group_pool_total(&self, player_id: i32) -> i32 {
        let uid = self.group_skill_uid(player_id);
        if uid < 0 {
            0
        } else {
            self.counter_names_at(uid)
                .into_iter()
                .map(|n| self.counter_at(uid, &n))
                .sum()
        }
    }

    /// Non-zero named counters on the instance at `uid`.
    pub fn counter_names_at(&self, uid: i32) -> Vec<String> {
        self.field_by_uid(uid)
            .map(|f| {
                f.counters
                    .iter()
                    .filter(|(_, v)| **v != 0)
                    .map(|(k, _)| k.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A consumer of the passed cap, not the engine deciding one (`max` > 0
    /// clamps, `max` = 0 is uncapped, exactly as
    /// [`Self::add_pool_at`]). Returns the new count; 0 when the player has
    /// no group skill (writes are no-ops).
    pub fn add_group_pool(&mut self, player_id: i32, name: &str, n: i32, max: i32) -> i32 {
        let uid = self.group_skill_uid(player_id);
        if uid < 0 {
            return 0;
        }
        self.add_counter_at(uid, name, n, max)
    }

    // ---------------------------------------------- skill / group attachments

    /// Rule id of the player's bound **group skill** (`skill:<group>:<skill>`),
    /// the one the group-skill named counters land on. `None` when there is none.
    pub fn group_skill_id(&self, player_id: i32) -> Option<String> {
        let uid = self.group_skill_uid(player_id);
        if uid < 0 {
            return None;
        }
        self.field_by_uid(uid).map(|f| f.card.clone())
    }

    /// Rule id of the player's **persona skill** (`skill:<persona>:<skill>`).
    /// A persona skill is a `skill:` instance that is
    /// *not* a group skill; `bind_skills` places it beside the group one.
    pub fn persona_skill_id(&self, player_id: i32) -> Option<String> {
        let s = self.player_id(player_id)?;
        s.field
            .iter()
            .find(|f| f.card.starts_with("skill:") && !f.group_skill)
            .map(|f| f.card.clone())
    }

    /// Every group-skill attachment on the player's field, as
    /// `(uid, rule id, extra)` in placement order.
    pub fn group_skills(&self, player_id: i32) -> Vec<(i32, String, i32)> {
        let Some(s) = self.player_id(player_id) else {
            return Vec::new();
        };
        s.field
            .iter()
            .filter(|f| f.group_skill)
            .map(|f| (f.uid, f.card.clone(), f.extra as i32))
            .collect()
    }

    /// Attach a group-skill instance.
    /// `extra` marks a taken copy: 「group-skill card effects do not stack」 (refused
    /// when an attachment of the same id is already there) and 「不视为那个group
    /// 的persona」 (`in_group` still reads only the persona). Returns the new uid,
    /// or -1 when refused.
    pub fn add_group_skill(
        &mut self,
        data: &crate::data::GameData,
        player_id: i32,
        id: &str,
        extra: bool,
        props: std::collections::BTreeMap<String, i32>,
    ) -> i32 {
        if !data.is_group_skill(id) {
            return -1;
        }
        if self
            .player_id(player_id)
            .is_some_and(|s| s.field.iter().any(|f| f.card == id))
        {
            return -1;
        }
        let uid = self.place_card(data, player_id, id, Msg::default(), props);
        if uid < 0 {
            return -1;
        }
        if extra {
            if let Some(f) = self.field_by_uid_mut(uid) {
                f.extra = true;
            }
        }
        uid
    }

    // ---------------------------------------------------------- named markers
    // Name-as-data: the counter is the frozen wire key. Cap / partial-gain
    // **policy** lives in the rules-layer kernel (`bandori_std::fire`), not
    // here. The advertised `max` is stored for display; this engine does not
    // police it.

    /// Move a named counter's advertised *cap* up or down. Storage-only: the
    /// pull-under is the rules-layer kernel's job (`bandori_std::fire`'s
    /// `pull_under_cap`, applied 「在任意时刻」). The name is data -- the frozen
    /// wire key is just one caller.
    pub fn add_marker_max(&mut self, player_id: i32, name: &str, n: i32) -> i32 {
        let cap = self.state_max(player_id, name).saturating_add(n).max(0);
        self.state_set_bounds(player_id, name, 0, cap);
        cap
    }

    /// Dumb add of `n` to a named counter; returns how many landed. No cap
    /// policy and no log line -- the rules-layer kernel clamps and narrates.
    pub fn gain_marker(&mut self, player_id: i32, name: &str, n: i32) -> i32 {
        let Some(s) = self.player_mut(player_id) else {
            return 0;
        };
        if n <= 0 || s.out() {
            return 0;
        }
        let before = s.state_get(name);
        let after = s.state_add(name, n);
        (after - before).max(0)
    }

    /// Spend `n` of a named counter; returns 1 when the player had enough.
    /// All-or-nothing. The name is data.
    pub fn spend_marker(&mut self, player_id: i32, name: &str, n: i32) -> i32 {
        let Some(s) = self.player_mut(player_id) else {
            return 0;
        };
        if n <= 0 {
            return 1;
        }
        if s.state_get(name) < n {
            return 0;
        }
        s.state_add(name, -n);
        1
    }

    // ---------------------------------------------------------- money moves

    /// Money in (「gain」), logged with its reason.
    pub fn gain_money(&mut self, player_id: i32, amount: i32, src: Msg) -> i32 {
        let Some(s) = self.player_mut(player_id) else {
            return 0;
        };
        if amount <= 0 || s.out() {
            return 0;
        }
        s.money += amount;
        self.bump_gain(player_id, 1);
        self.log(
            "gain",
            player_id,
            Msg::new("log.gain")
                .player_id("who", player_id)
                .n("amount", amount)
                .msg("src", Msg::new("log.part.why").msg("why", src)),
        );
        amount
    }

    /// Money out (「pay」), logged. Routines that may need to raise funds are
    /// driven through `Cx` instead (they prompt).
    pub fn pay_money(&mut self, player_id: i32, amount: i32, src: Msg) -> i32 {
        if amount > 0 && self.money_locked(player_id) {
            self.log(
                "text",
                player_id,
                Msg::new("log.money_locked")
                    .player_id("who", player_id)
                    .n("amount", amount),
            );
            return 0;
        }
        let Some(s) = self.player_mut(player_id) else {
            return 0;
        };
        if amount <= 0 || s.out() {
            return 0;
        }
        let paid = amount.min(s.money);
        s.money -= paid;
        self.log(
            "lose",
            player_id,
            Msg::new("log.lose")
                .player_id("who", player_id)
                .n("amount", paid)
                .msg("src", Msg::new("log.part.why").msg("why", src)),
        );
        paid
    }

    // ------------------------------------------------------------ card flow

    /// Restore an exhausted draw pile immediately, before any later effect.
    /// Returns whether a shuffle happened so the caller can raise its hook.
    pub fn refill_draw_pile(&mut self, i: usize) -> bool {
        if self.st.players.get(i).is_none_or(|s| s.out())
            || !self.hidden[i].draw.is_empty()
            || self.hidden[i].discard.is_empty()
        {
            return false;
        }
        let mut pile = std::mem::take(&mut self.hidden[i].discard);
        self.rng.shuffle(&mut pile);
        self.hidden[i].draw = pile;
        self.log("text", i as i32, Msg::new("log.reshuffle").player_id("who", i));
        true
    }

    /// Draw `n` cards, refilling as soon as the last card leaves.
    /// Returns how many were actually drawn.
    pub fn draw_cards(&mut self, player_id: i32, n: i32, over_hand_limit: bool) -> i32 {
        let Some(i) = usize::try_from(player_id).ok() else {
            return 0;
        };
        if self.st.players.get(i).is_none_or(|s| s.out()) {
            return 0;
        }
        let mut got = 0;
        for _ in 0..n.max(0) {
            self.refill_draw_pile(i);
            let Some(card) = self.hidden[i].draw.pop() else {
                break;
            };
            self.hidden[i].hand.push(card);
            self.refill_draw_pile(i);
            got += 1;
        }
        if got > 0 {
            let over = self.st.players[i]
                .over_hand(self.hidden[i].hand.len())
                .then(|| {
                    Msg::new("log.part.over_hand")
                        .i("limit", self.st.players[i].hand_limit().unwrap_or_default())
                });
            self.log(
                "draw",
                player_id,
                Msg::new("log.draw").player_id("who", player_id).i("n", got).opt("over", over),
            ).value = got;
        }
        let _ = over_hand_limit;
        got
    }

    /// Move a card id between zones (hand / draw pile / discard).
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

    pub fn to_discard(&mut self, player_id: i32, card: &str) -> bool {
        if let Ok(i) = usize::try_from(player_id) {
            if let Some(h) = self.hidden.get_mut(i) {
                if let Some(k) = h.hand.iter().position(|c| c == card) {
                    h.hand.remove(k);
                }
                h.discard.push(card.to_string());
                return self.refill_draw_pile(i);
            }
        }
        false
    }

    // -------------------------------------------------------- placed cards

    /// May this player build on this tile at all? The one
    /// build gate, asked both by the engine's build step and by a card choosing
    /// a destination. `None` = yes; otherwise the reason.
    pub fn why_not_build_on(
        &self,
        data: &crate::data::GameData,
        player_id: i32,
        tile: i32,
    ) -> Option<Msg> {
        let Some(t) = usize::try_from(tile).ok().and_then(|t| data.tiles.get(t)) else {
            return Some(Msg::new("err.build_not_own"));
        };
        let Some(s) = self.player_id(player_id) else {
            return Some(Msg::new("err.build_not_own"));
        };
        if !t.is_buyable() || self.st.owners.get(tile as usize).copied().unwrap_or(-1) != player_id
        {
            return Some(Msg::new("err.build_not_own"));
        }
        // 「owner不可盖房」 -- `prop::NO_BUILD` on a rule instance
        // (`docs/TILES.md`), not a keyed flag. The **source** owns the arming
        // and the disarming; the reader is the rule instance. Two placements
        // are read, the same shape as the the start square veto: the player's own field
        // instance (a per-player veto) and the tile's rule instance (a per-tile
        // veto). A hand card's 「本回合无法加盖房屋」 rides a lingering instance
        // instead -- see below.
        if s.field
            .iter()
            .any(|f| f.props.get(crate::state::prop::NO_BUILD).copied().unwrap_or(0) > 0)
            || self.st.board_field.iter().any(|f| {
                f.tile == tile
                    && f.props.get(crate::state::prop::NO_BUILD).copied().unwrap_or(0) > 0
            })
        {
            return Some(Msg::new("err.build_blocked"));
        }
        // 「不可在造价N及以上的格子上加盖房屋」 (卡池BUG) -- `prop::NO_BUILD_ABOVE`
        // on a board-owned instance that governs no single tile (`tile < 0`):
        // an active event. Refuses a build whose house cost is `>=` the prop.
        let house_cost = t.house;
        if house_cost > 0
            && self.st.board_field.iter().any(|f| {
                f.tile < 0
                    && f.props
                        .get(crate::state::prop::NO_BUILD_ABOVE)
                        .copied()
                        .unwrap_or(0)
                        > 0
                    && house_cost
                        >= f.props
                            .get(crate::state::prop::NO_BUILD_ABOVE)
                            .copied()
                            .unwrap_or(0)
            })
        {
            return Some(Msg::new("err.build_blocked"));
        }
        // 学生会的检查's 「本回合无法加盖房屋」 rides a **lingering** instance
        // carrying `prop::NO_BUILD` (`docs/PURCHASE.md` P5) -- the hand-card
        // home for a per-player veto, since a hand play has no field instance
        // for `ctx::set_prop` to write. It keeps its own message key so the
        // log line does not move.
        if self
            .turn
            .lingering
            .iter()
            .any(|l| l.owner == player_id && l.props.get(crate::state::prop::NO_BUILD).copied().unwrap_or(0) > 0)
        {
            return Some(Msg::new("err.build_denied"));
        }
        // No upgrade past the declared table (a rule may also stamp
        // `prop::BUILD_MAX = 0`).
        if t.rent.len() < 2 {
            return Some(Msg::new("err.build_no_upgrade"));
        }
        if self
            .st
            .mortgaged
            .get(tile as usize)
            .copied()
            .unwrap_or(false)
        {
            return Some(Msg::new("err.build_mortgaged"));
        }
        if self.st.houses.get(tile as usize).copied().unwrap_or(0) as usize >= t.rent.len() - 1 {
            return Some(Msg::new("err.build_full"));
        }
        // The payer's own gate (the pay-gate layers 「无法收付款」) is the caller's
        // `Gate::Pay` question: a World has no ruleset to ask.
        if s.out() {
            return Some(Msg::new("err.cannot_spend"));
        }
        None
    }

    /// Flip a placed card face-down / face-up.
    pub fn set_card_face_down(&mut self, player_id: i32, card: &str, down: bool) -> bool {
        let Some(s) = self.player_mut(player_id) else {
            return false;
        };
        let Some(f) = s.field.iter_mut().find(|f| f.card == card) else {
            return false;
        };
        f.face_down = down;
        true
    }

    /// Is this placed card face-down?
    pub fn card_face_down(&self, player_id: i32, card: &str) -> bool {
        self.player_id(player_id)
            .and_then(|s| s.field.iter().find(|f| f.card == card))
            .is_some_and(|f| f.face_down)
    }

    /// Place a field card on the player (`tile: -1` -- it sits with its owner).
    /// Returns the new instance's uid, which is what addresses it afterwards.
    ///
    /// `props` is the card rule's declared static property map
    /// (`CardRules::card_props`, see [`crate::state::prop`]). It rides on the
    /// instance, so a continuous property like `HAND_LIMIT_DELTA` is gone the
    /// moment the card leaves the field.
    pub fn place_card(
        &mut self,
        data: &GameData,
        player_id: i32,
        card: &str,
        note: Msg,
        props: std::collections::BTreeMap<String, i32>,
    ) -> i32 {
        self.place_card_on(data, player_id, -1, card, note, props)
    }

    /// Place a field card **on a tile** -- the
    /// mark sits on the board at `tile` rather than with its owner. `tile: -1`
    /// puts it with the owner, which is [`Self::place_card`].
    ///
    /// `player_id == ` [`crate::state::BOARD_OWNER`] places on the **board
    /// field** -- the neutral owner of tile rule instances (`bind_tiles`). The
    /// instance keeps `owner = user = -1`; `tile` names the board tile it governs.
    pub fn place_card_on(
        &mut self,
        data: &GameData,
        player_id: i32,
        tile: i32,
        card: &str,
        note: Msg,
        props: std::collections::BTreeMap<String, i32>,
    ) -> i32 {
        if player_id != crate::state::BOARD_OWNER && self.player_id(player_id).is_none() {
            return -1;
        }
        let uid = self.st.next_card_uid;
        self.st.next_card_uid += 1;
        // A group skill's instance is the the group-skill named counter holder; see
        // [`Self::group_skill_uid`].
        let group_skill = data.is_group_skill(card);
        // 「此卡不受任何其他效果影响」 is a static [特] property, not a play-body
        // side effect -- `place_raw` and engine placement must see it too.
        let immune = props
            .get(crate::state::prop::IMMUNE)
            .copied()
            .unwrap_or(0)
            != 0;
        let inst = FieldCard {
            uid,
            card: card.to_string(),
            owner: player_id,
            user: player_id,
            tile,
            counters: BTreeMap::new(),
            face_down: false,
            immune,
            props,
            group_skill,
            extra: false,
            note,
        };
        if player_id == crate::state::BOARD_OWNER {
            self.st.board_field.push(inst);
            return uid;
        }
        let Some(s) = self.player_mut(player_id) else {
            return -1;
        };
        s.field.push(inst);
        uid
    }

    /// The card instance at `uid`, wherever it sits -- a player's field or the
    /// board field. `uid` is what identifies a card in this match -- the *name*
    /// does not, because one player may hold several copies of the same card in
    /// play at once.
    pub fn field_by_uid(&self, uid: i32) -> Option<&FieldCard> {
        self.st
            .board_field
            .iter()
            .chain(self.st.players.iter().flat_map(|s| s.field.iter()))
            .find(|f| f.uid == uid)
    }

    /// The seat whose field the instance at `uid` sits on (`BOARD_OWNER` for
    /// the board field), or -1 when it is not in play. Distinct from
    /// [`FieldCard::owner`], which names who the card *belongs* to.
    pub fn holder_of(&self, uid: i32) -> i32 {
        if self.st.board_field.iter().any(|f| f.uid == uid) {
            return crate::state::BOARD_OWNER;
        }
        self.st
            .players
            .iter()
            .position(|s| s.field.iter().any(|f| f.uid == uid))
            .map_or(-1, |i| i as i32)
    }

    /// Re-attribute the instance at `uid` to the player who owns the card.
    /// Placement stamps `owner` / `user` with the field holder; a card parked
    /// on *another* player's field (「放置于你以外的一名玩家场上」) belongs to the
    /// player who played it, which the placement run records here.
    pub fn set_field_owner(&mut self, uid: i32, owner: i32) {
        if let Some(f) = self.field_by_uid_mut(uid) {
            f.owner = owner;
            f.user = owner;
        }
    }

    fn field_by_uid_mut(&mut self, uid: i32) -> Option<&mut FieldCard> {
        if let Some(f) = self.st.board_field.iter_mut().find(|f| f.uid == uid) {
            return Some(f);
        }
        self.st
            .players
            .iter_mut()
            .flat_map(|s| s.field.iter_mut())
            .find(|f| f.uid == uid)
    }

    /// Every card instance on `player_id`'s field, as `(uid, id)` in placement
    /// order. The dispatch walks this rather than the names, so two copies of
    /// the same card are two hooks. `BOARD_OWNER` lists the board field (the
    /// tile rule instances).
    pub fn field_instances(&self, player_id: i32) -> Vec<(i32, String)> {
        if player_id == crate::state::BOARD_OWNER {
            return self
                .st
                .board_field
                .iter()
                .map(|f| (f.uid, f.card.clone()))
                .collect();
        }
        self.player_id(player_id).map_or_else(Vec::new, |s| {
            s.field.iter().map(|f| (f.uid, f.card.clone())).collect()
        })
    }

    /// The rule instances on board tile `tile`, in placement order -- what the
    /// settle body runs. Board-owned only; a player's card *placed on* the tile
    /// (`place_card_on(player, tile, …)`) is on that player's field and hears
    /// the settle point through the normal field-hook dispatch.
    pub fn tile_rule_instances(&self, tile: i32) -> Vec<(i32, String)> {
        self.st
            .board_field
            .iter()
            .filter(|f| f.tile == tile)
            .map(|f| (f.uid, f.card.clone()))
            .collect()
    }

    /// The **event rule** instances on the neutral board owner, in activation
    /// order (`docs/EVENTS.md`). Board-owned like the tile rules, but with
    /// `tile = -1`: they govern a drawn event that is still in play, not a
    /// board square. Each is `card = event:<id>`, one per active event.
    pub fn event_rule_instances(&self) -> Vec<(i32, String)> {
        self.st
            .board_field
            .iter()
            .filter(|f| f.tile < 0 && f.card.starts_with("event:"))
            .map(|f| (f.uid, f.card.clone()))
            .collect()
    }

    /// The board-wide **mark owners** (`mark:*`, [`crate::data::mark_rule_ids`])
    /// -- one instance each on the neutral board owner, governing no single
    /// tile. the tile-mark owner is the [tile marks] tile-mark owner. Like
    /// [`Self::event_rule_instances`], these hear every trigger their rule
    /// declares wherever it points, not just the ones on "their" tile (they
    /// have none).
    pub fn mark_rule_instances(&self) -> Vec<(i32, String)> {
        self.st
            .board_field
            .iter()
            .filter(|f| f.tile < 0 && f.card.starts_with("mark:"))
            .map(|f| (f.uid, f.card.clone()))
            .collect()
    }

    /// The board-wide **status hosts** (`status:*`, [`crate::data::status_rule_ids`])
    /// -- one instance each on the neutral board owner, governing no single
    /// tile. Like [`Self::mark_rule_instances`], these hear every trigger their
    /// rule declares wherever it points (the hand-gate fold / clear runs on the
    /// turn-flow triggers, the [除外] tick on `turnStartBefore`). Collected
    /// **last** among the field hooks, after tile / event / mark, so sibling
    /// hooks still see pre-tick state.
    pub fn status_rule_instances(&self) -> Vec<(i32, String)> {
        self.st
            .board_field
            .iter()
            .filter(|f| f.tile < 0 && f.card.starts_with("status:"))
            .map(|f| (f.uid, f.card.clone()))
            .collect()
    }

    /// On-card named counter on the instance at `uid` (generic).
    pub fn pool_at(&self, uid: i32, name: &str) -> i32 {
        self.counter_at(uid, name)
    }

    /// One declared property of the instance at `uid` (`FieldCard::props`,
    /// `crate::state::prop` keys). Default `0`.
    pub fn prop_at(&self, uid: i32, key: &str) -> i32 {
        self.field_by_uid(uid)
            .and_then(|f| f.props.get(key).copied())
            .unwrap_or(0)
    }

    /// Write a property on the instance at `uid`; returns the stored value.
    pub fn set_prop_at(&mut self, uid: i32, key: &str, value: i32) -> i32 {
        let Some(f) = self.field_by_uid_mut(uid) else {
            return 0;
        };
        f.props.insert(key.to_string(), value);
        value
    }

    /// A declared property of the rule instance governing `tile` -- what a card
    /// that bends a tile writes instead of an engine flag (`docs/TILES.md`).
    /// Reads the first board-owned instance on `tile` that carries `key`; a tile
    /// with no rule instance reads as `0`.
    pub fn tile_prop(&self, tile: i32, key: &str) -> i32 {
        self.st
            .board_field
            .iter()
            .find(|f| f.tile == tile)
            .and_then(|f| f.props.get(key).copied())
            .unwrap_or(0)
    }

    /// Every prop key set on a board-owned rule instance governing `tile`
    /// (the keys [`Self::tile_prop`] can answer non-zero for).
    pub fn tile_prop_keys(&self, tile: i32) -> impl Iterator<Item = &str> {
        self.st
            .board_field
            .iter()
            .filter(move |f| f.tile == tile)
            .flat_map(|f| f.props.keys().map(String::as_str))
    }

    /// Write [`Self::tile_prop`] on every board-owned rule instance governing
    /// `tile`; returns the stored value. The source owns the arming and the
    /// disarming; the reader is the tile instance.
    pub fn set_tile_prop(&mut self, tile: i32, key: &str, value: i32) -> i32 {
        let mut any = false;
        for f in self.st.board_field.iter_mut().filter(|f| f.tile == tile) {
            f.props.insert(key.to_string(), value);
            any = true;
        }
        if any {
            value
        } else {
            0
        }
    }

    /// Add named counters on the instance at `uid`; `max` caps (0 = uncapped).
    ///
    /// Also mirrors the count into [`crate::state::MatchState::event_active`]
    /// when the instance is an event's board-owner rule (`docs/EVENTS.md`):
    /// `ActiveEvent::counter` is the public view of the instance's named counters,
    /// and the raw row is what clients (and tests) read.
    pub fn add_pool_at(&mut self, uid: i32, name: &str, n: i32, max: i32) -> i32 {
        self.add_counter_at(uid, name, n, max)
    }

    /// Set the instance at `uid`'s named counters; returns the new count.
    pub fn set_pool_at(&mut self, uid: i32, name: &str, n: i32) -> i32 {
        self.set_counter_at(uid, name, n)
    }

    /// Where the instance at `uid` sits, or -1 for "with its owner" / gone.
    pub fn tile_at(&self, uid: i32) -> i32 {
        self.field_by_uid(uid).map_or(-1, |f| f.tile)
    }

    /// Move the instance at `uid` to `tile` (-1 = back with its owner).
    pub fn set_tile_at(&mut self, uid: i32, tile: i32) -> bool {
        match self.field_by_uid_mut(uid) {
            Some(f) => {
                f.tile = tile;
                true
            }
            None => false,
        }
    }

    /// Is the instance at `uid` face-down?
    pub fn is_face_down_at(&self, uid: i32) -> bool {
        self.field_by_uid(uid).is_some_and(|f| f.face_down)
    }

    /// Flip the instance at `uid`; returns whether it exists.
    pub fn set_face_down_at(&mut self, uid: i32, on: bool) -> bool {
        match self.field_by_uid_mut(uid) {
            Some(f) => {
                f.face_down = on;
                true
            }
            None => false,
        }
    }

    /// Is the instance at `uid` marked 「不受任何效果影响」?
    pub fn is_immune_at(&self, uid: i32) -> bool {
        self.field_by_uid(uid).is_some_and(|f| f.immune)
    }

    /// Mark the instance at `uid` 「不受任何效果影响」; returns whether it exists.
    pub fn set_immune_at(&mut self, uid: i32, on: bool) -> bool {
        match self.field_by_uid_mut(uid) {
            Some(f) => {
                f.immune = on;
                true
            }
            None => false,
        }
    }

    /// Take the instance at `uid` off the field; returns the **player index**
    /// it left (or -1 when there was no such instance -- including a board-owned
    /// tile rule, which goes nowhere on removal). Dest routing
    /// (`to_discard` and kin) keys on the player index, not the room member id.
    ///
    /// An **event rule** instance leaving the field is the event expiring
    /// (`ctx::unplace_self` from its body): drop it from the active list too.
    /// Where it is filed (`event_discard` / `event_removed`) is the body's own
    /// `ctx::event_expire` call -- this only unbinds.
    pub fn unplace_at(&mut self, uid: i32) -> i32 {
        // One cleanup path keyed by the instance (user ruling 2026-10-10):
        // every bound counter unit -- tile mark or held token -- dies with it.
        self.destroy_instance_units(uid);
        if let Some(i) = self.st.board_field.iter().position(|f| f.uid == uid) {
            let card = self.st.board_field.remove(i).card;
            if let Some(id) = card.strip_prefix("event:") {
                self.st.event_active.retain(|e| e.id != id);
            }
            return crate::state::BOARD_OWNER;
        }
        for (pi, s) in self.st.players.iter_mut().enumerate() {
            if let Some(i) = s.field.iter().position(|f| f.uid == uid) {
                s.field.remove(i);
                return pi as i32;
            }
        }
        -1
    }

    /// Place the player's skill rules on their field. This is the binding:
    /// see [`crate::data::GameData::skill_rules_of`]. Once placed, `On::Action`
    /// reaches them like any other field card and named counter counters work on
    /// them. Idempotent -- calling it twice does not double-place.
    pub fn bind_skills(
        &mut self,
        data: &crate::data::GameData,
        rules: &dyn super::rules::CardRules,
        player_id: i32,
    ) {
        let persona = self
            .player_id(player_id)
            .map(|s| s.persona.clone())
            .unwrap_or_default();
        for id in data.skill_rules_of(&persona) {
            if !self.placed_cards(player_id).contains(&id) {
                let props = rules.card_props(&id);
                self.place_card(data, player_id, &id, Msg::default(), props);
            }
        }
    }

    /// Place the **tile rule** instances on the neutral board owner
    /// ([`crate::state::BOARD_OWNER`]) -- one per board tile, the way
    /// [`Self::bind_skills`] places a player's skills. Idempotent.
    ///
    /// The rule id comes from the ruleset (`CardRules::tile_rule_id`, the
    /// tile-kind table declared by `CardDef::governs` / `board_role`).
    /// Tile data (price, rent table, group, level cap) is stamped into the
    /// instance's `props`; a key the kind does not use is still stamped, so a
    /// card that retunes one reads and writes the same map the bodies do.
    ///
    /// A kind with no rule in the ruleset (e.g. `StubRules`) places nothing,
    /// and the engine's built-in `settle_tile` default handles it.
    pub fn bind_tiles(
        &mut self,
        data: &crate::data::GameData,
        rules: &dyn super::rules::CardRules,
    ) {
        use crate::state::prop;
        for (t, tile) in data.tiles.iter().enumerate() {
            let id = rules.tile_rule_id(tile.kind.as_str());
            if id.is_empty() {
                continue;
            }
            if self.tile_rule_instances(t as i32).iter().any(|(_, c)| c == &id) {
                continue;
            }
            if !rules.has_rule(&id) {
                // No such rule in this ruleset (StubRules). The engine's
                // built-in settlement covers it; nothing to bind.
                continue;
            }
            let mut props = rules.card_props(&id);
            props.insert(prop::PRICE.to_string(), tile.price);
            props.insert(prop::HOUSE.to_string(), tile.house);
            props.insert(prop::GROUP.to_string(), tile.group);
            props.insert(prop::BUYABLE.to_string(), tile.is_buyable() as i32);
            props.insert(prop::RENT_LEN.to_string(), tile.rent.len() as i32);
            props.insert(
                prop::BUILD_MAX.to_string(),
                tile.rent.len().saturating_sub(1) as i32,
            );
            for (n, r) in tile.rent.iter().enumerate() {
                props.insert(format!("{}{}", prop::RENT_PREFIX, n), *r);
            }
            self.place_card_on(
                data,
                crate::state::BOARD_OWNER,
                t as i32,
                &id,
                Msg::default(),
                props,
            );
        }
        // Board-wide **mark owners** (`CardRules::mark_rule_ids`) -- one
        // instance each on the neutral board owner, governing no single
        // tile (`tile = -1`). Idempotent.
        for id in rules.mark_rule_ids() {
            if !rules.has_rule(&id) {
                continue;
            }
            if self
                .st
                .board_field
                .iter()
                .any(|f| f.card == id && f.tile < 0)
            {
                continue;
            }
            let props = rules.card_props(&id);
            self.place_card_on(
                data,
                crate::state::BOARD_OWNER,
                -1,
                &id,
                Msg::default(),
                props,
            );
        }
        // Board-wide **status hosts** (`CardRules::status_rule_ids`) -- one
        // instance each on the neutral board owner. Idempotent, same path as
        // mark owners.
        for id in rules.status_rule_ids() {
            if !rules.has_rule(&id) {
                continue;
            }
            if self
                .st
                .board_field
                .iter()
                .any(|f| f.card == id && f.tile < 0)
            {
                continue;
            }
            let props = rules.card_props(&id);
            self.place_card_on(
                data,
                crate::state::BOARD_OWNER,
                -1,
                &id,
                Msg::default(),
                props,
            );
        }
    }

    /// Bind one **event rule** instance on the neutral board owner and record
    /// the event in [`crate::state::MatchState::event_active`] (`docs/EVENTS.md`).
    ///
    /// Called from `draw_event` when the event's rule exists in the ruleset, so
    /// the body's `On::Play` and any `On::Action` it declares run against a live
    /// instance (props, named counters, `unplace_self` = expire). Idempotent per
    /// event id: a second activation of the same event keeps the first
    /// instance. Returns the instance uid, or -1 when there is no rule
    /// (`StubRules`) -- the engine's built-in fallback files the card away and
    /// binds nothing.
    pub fn bind_event(
        &mut self,
        data: &crate::data::GameData,
        rules: &dyn super::rules::CardRules,
        player_id: i32,
        id: &str,
    ) -> i32 {
        let rid = crate::data::event_rule_id(id);
        if !rules.has_rule(&rid) {
            return -1;
        }
        if let Some((uid, _)) = self
            .event_rule_instances()
            .into_iter()
            .find(|(_, c)| c == &rid)
        {
            return uid;
        }
        let props = rules.card_props(&rid);
        let uid = self.place_card_on(
            data,
            crate::state::BOARD_OWNER,
            -1,
            &rid,
            Msg::default(),
            props,
        );
        self.st.event_active.push(crate::state::ActiveEvent {
            id: id.to_string(),
            player_id: player_id as i32,
            counter: 0,
            counter2: 0,
            note: Msg::default(),
            face_down: false,
        });
        uid
    }

    /// Expire an active event: drop it from [`crate::state::MatchState::event_active`]
    /// and unbind its rule instance. `removed` files it to `event_removed`
    /// (「永久移除」) rather than `event_discard`.
    ///
    /// This is `ctx::event_expire` -- the 「放入事件弃牌」 / 「永久移除」 half of an
    /// event's expiry clause. A one-shot event never gets here: `draw_event`
    /// files it away itself when the rule reports it does not remain.
    pub fn expire_event(&mut self, id: &str, removed: bool) {
        let rid = crate::data::event_rule_id(id);
        if let Some(i) = self
            .st
            .board_field
            .iter()
            .position(|f| f.card == rid && f.tile < 0)
        {
            let uid = self.st.board_field[i].uid;
            self.st.board_field.remove(i);
            self.destroy_instance_units(uid);
        }
        self.st.event_active.retain(|e| e.id != id);
        if removed {
            if !self.event_removed.iter().any(|e| e == id) {
                self.event_removed.push(id.to_string());
            }
        } else if !self.event_discard.iter().any(|e| e == id) {
            self.event_discard.push(id.to_string());
        }
    }

    /// An empty event deck takes the shuffled discard as the new deck. Called
    /// once a draw has fully resolved (the drawn card filed away included), and
    /// before a draw as a fallback; never mid-resolution. (Ruling 2026-10-07:
    /// "Wait for everything to resolve, only then reshuffle the event deck
    /// back" -- unlike the hand-card draw pile, which refills as soon as it
    /// empties.)
    pub fn refill_event_deck(&mut self) {
        if !self.event_deck.is_empty() || self.event_discard.is_empty() {
            return;
        }
        let mut deck = std::mem::take(&mut self.event_discard);
        self.rng.shuffle(&mut deck);
        self.event_deck = deck;
        self.log("text", -1, Msg::new("log.events_reshuffled"));
    }

    /// Is this event active and face-up? (`ctx::event_is_active`.)
    pub fn event_is_active(&self, id: &str) -> bool {
        self.st
            .event_active
            .iter()
            .any(|e| e.id == id && !e.face_down)
    }

    /// Take `id` out of the game for good (`ctx::event_banish`): off the deck,
    /// the discard and the active list. 「从所有非衍生事件中选择3个移除」.
    pub fn event_banish(&mut self, id: &str) {
        self.event_deck.retain(|e| e != id);
        self.event_discard.retain(|e| e != id);
        self.st.event_top.retain(|e| e != id);
        self.expire_event(id, true);
    }

    /// Push `id` onto the **top** of the event deck (`event_deck`'s end is the
    /// top). `face_down` records it on the public `event_top` view as a
    /// face-down slot (「背面朝上放置于事件牌堆顶部」) -- the draw still reveals
    /// it, matching the rulebook's 「抽取的事件卡不进入手卡并向所有玩家公开」.
    pub fn event_deck_push(&mut self, id: &str, face_down: bool) {
        self.event_deck.push(id.to_string());
        if face_down {
            self.st.event_top.push(id.to_string());
        }
    }

    /// Take a field card off; returns its card id.
    pub fn unplace_card(&mut self, player_id: i32, card: &str) -> Option<String> {
        let s = self.player_mut(player_id)?;
        let k = s.field.iter().position(|f| f.card == card)?;
        let uid = s.field[k].uid;
        let id = s.field.remove(k).card;
        self.destroy_instance_units(uid);
        Some(id)
    }

    pub fn placed_cards(&self, player_id: i32) -> Vec<String> {
        self.player_id(player_id).map_or_else(Vec::new, |s| {
            s.field.iter().map(|f| f.card.clone()).collect()
        })
    }

    /// Mark a placed field card as unaffected by other effects
    /// (「此卡不受…效果影响」). Effects that would touch it read
    /// [`Self::card_immune`] and skip.
    pub fn set_card_immune(&mut self, player_id: i32, card: &str, on: bool) -> bool {
        let Some(s) = self.player_mut(player_id) else {
            return false;
        };
        let Some(f) = s.field.iter_mut().find(|f| f.card == card) else {
            return false;
        };
        f.immune = on;
        true
    }

    /// Is this placed field card immune to other effects?
    pub fn card_immune(&self, player_id: i32, card: &str) -> bool {
        self.player_id(player_id)
            .and_then(|s| s.field.iter().find(|f| f.card == card))
            .is_some_and(|f| f.immune)
    }

    /// Move a placed field card to `tile`.
    /// The 「将此卡放置于X格子上」 re-placement hop: the card is already in play,
    /// it just sits somewhere else. `tile: -1` puts it back with its owner.
    pub fn set_card_tile(&mut self, player_id: i32, card: &str, tile: i32) -> bool {
        let Some(s) = self.player_mut(player_id) else {
            return false;
        };
        let Some(f) = s.field.iter_mut().find(|f| f.card == card) else {
            return false;
        };
        f.tile = tile;
        true
    }

    /// Set a named on-card counter of a placed card.
    pub fn set_card_counter(&mut self, player_id: i32, card: &str, name: &str, n: i32) {
        let have = self.card_counter(player_id, card, name);
        self.add_card_counter(player_id, card, name, n - have, 0);
    }

    /// On-card named counter of a placed card, looked up by card id (first
    /// field copy wins). Generic form of [`Self::card_pool`].
    pub fn card_counter(&self, player_id: i32, card: &str, name: &str) -> i32 {
        self.player_id(player_id)
            .and_then(|s| s.field.iter().find(|f| f.card == card))
            .map_or(0, |f| f.counters.get(name).copied().unwrap_or(0))
    }

    /// Adjust the on-card named counter of a placed card. Generic form of
    /// [`Self::add_card_counter`].
    pub fn add_card_counter(
        &mut self,
        player_id: i32,
        card: &str,
        name: &str,
        n: i32,
        max: i32,
    ) -> i32 {
        let Some(f) = self
            .player_mut(player_id)
            .and_then(|s| s.field.iter_mut().find(|f| f.card == card))
        else {
            return 0;
        };
        let uid = f.uid;
        self.add_counter_at(uid, name, n, max)
    }

    // --------------------------------------------------------------- marks
    // Generic tile-mark / bound-counter API (user ruling 2026-10-10).
    //
    // A **tile mark** is a unit of some card instance's named counter, bound to
    // a tile. The same counter may also have units bound to a holder
    // ([`Self::add_tok`] / `Counter`) or sitting on the card itself
    // ([`Self::counter_at`]). Destroying the owning instance destroys every
    // unit, wherever it sits ([`Self::destroy_instance_units`]).
    //
    // `MarkFilter::ANY` (empty kind/category, `None` owner/src/instance)
    // matches anything.

    /// Sum of `count` over the matching marks on `tile`.
    pub fn count_marks(&self, tile: i32, filter: &MarkFilter<'_>) -> i32 {
        self.st
            .marks
            .iter()
            .filter(|m| m.tile == tile && filter.matches(m))
            .map(|m| m.count)
            .sum()
    }

    /// Provenance (`TileMark::src`) of the first matching mark on `tile`,
    /// encoded as `i32` (`-1` when none) for the guest ABI.
    pub fn mark_src_at(&self, tile: i32, filter: &MarkFilter<'_>) -> i32 {
        self.st
            .marks
            .iter()
            .find(|m| m.tile == tile && filter.matches(m))
            .and_then(|m| m.src)
            .unwrap_or(-1)
    }

    /// Owning instance (`TileMark::instance`) of the first matching mark on
    /// `tile`, encoded as `i32` (`-1` when none) for the guest ABI.
    pub fn mark_instance_at(&self, tile: i32, filter: &MarkFilter<'_>) -> i32 {
        self.st
            .marks
            .iter()
            .find(|m| m.tile == tile && filter.matches(m))
            .and_then(|m| m.instance)
            .unwrap_or(-1)
    }

    /// Bind `count` more units of `instance`'s counter `kind` to `tile`.
    /// `owner` colours the mark in the view; `src` is provenance (which run
    /// placed it); `category` is the display category ([`crate::state::mark_category`]).
    /// `fresh`: `false` merges onto the first match (same tile/kind/category/
    /// instance/src), `true` always pushes a fresh row. Returns the count now
    /// on that row.
    pub fn place_mark(
        &mut self,
        instance: i32,
        kind: &str,
        category: &str,
        tile: i32,
        owner: i32,
        src: i32,
        count: i32,
        note: Msg,
        fresh: bool,
    ) -> i32 {
        if count <= 0 {
            return 0;
        }
        let instance = (instance >= 0).then_some(instance);
        let src = (src >= 0).then_some(src);
        let cat = if category.is_empty() {
            crate::state::mark_category::PLAYER
        } else {
            category
        };
        if !fresh {
            if let Some(m) = self.st.marks.iter_mut().find(|m| {
                m.tile == tile
                    && m.kind == kind
                    && m.category == cat
                    && m.instance == instance
                    && m.src == src
            }) {
                m.count += count;
                return m.count;
            }
        }
        let card = src
            .and_then(|s| self.field_by_uid(s))
            .map(|f| f.card.clone())
            .unwrap_or_default();
        let uid = self.st.marks.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
        self.st.marks.push(TileMark {
            uid,
            tile,
            kind: kind.to_string(),
            category: cat.to_string(),
            owner,
            count,
            card,
            src,
            instance,
            partner: None,
            note,
        });
        count
    }

    /// Partner tile (`TileMark::partner`) of the first matching mark on `tile`,
    /// encoded as `i32` (`-1` when none / unpaired) for the guest ABI.
    pub fn mark_partner(&self, tile: i32, filter: &MarkFilter<'_>) -> i32 {
        self.st
            .marks
            .iter()
            .find(|m| m.tile == tile && filter.matches(m))
            .and_then(|m| m.partner)
            .unwrap_or(-1)
    }

    /// Place **one** paired mark spanning tiles `a` and `b`: one row per tile,
    /// each row's [`TileMark::partner`] naming the other. Place-or-refuse
    /// atomic -- if either tile already carries a matching mark (same kind /
    /// category / owner), place **nothing** and return `false`. The pair cap
    /// 「放置1个…（上限1）」 is therefore on the pair, and a tile cannot sit in
    /// two links at once (either-end refusal). Returns `true` when both rows
    /// landed. `instance` / `src` / `note` stamp both rows like [`Self::place_mark`];
    /// both die with the creating instance via [`Self::destroy_instance_units`].
    pub fn place_mark_pair(
        &mut self,
        instance: i32,
        kind: &str,
        category: &str,
        a: i32,
        b: i32,
        owner: i32,
        src: i32,
        count: i32,
        note: Msg,
    ) -> bool {
        if count <= 0 || a < 0 || b < 0 || a == b {
            return false;
        }
        let instance = (instance >= 0).then_some(instance);
        let src = (src >= 0).then_some(src);
        let cat = if category.is_empty() {
            crate::state::mark_category::PLAYER
        } else {
            category
        };
        // Pair cap 1: refuse if either end already carries a matching mark for
        // that owner. Atomic -- neither row is placed when this trips.
        let marked = |tile: i32| {
            self.st.marks.iter().any(|m| {
                m.tile == tile && m.kind == kind && m.category == cat && m.owner == owner
            })
        };
        if marked(a) || marked(b) {
            return false;
        }
        let card = src
            .and_then(|s| self.field_by_uid(s))
            .map(|f| f.card.clone())
            .unwrap_or_default();
        let mut next = self.st.marks.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
        for (tile, partner) in [(a, b), (b, a)] {
            self.st.marks.push(TileMark {
                uid: next,
                tile,
                kind: kind.to_string(),
                category: cat.to_string(),
                owner,
                count,
                card: card.clone(),
                src,
                instance,
                partner: Some(partner),
                note: note.clone(),
            });
            next += 1;
        }
        true
    }

    /// Move one matching mark's `count` by `delta`; the row is dropped at 0.
    /// Returns the count now stored on that row (0 when nothing matched).
    pub fn bump_mark(&mut self, tile: i32, filter: &MarkFilter<'_>, delta: i32) -> i32 {
        let Some(m) = self
            .st
            .marks
            .iter_mut()
            .find(|m| m.tile == tile && filter.matches(m))
        else {
            return 0;
        };
        m.count = (m.count + delta).max(0);
        let now = m.count;
        let uid = m.uid;
        if now == 0 {
            self.st.marks.retain(|m| m.uid != uid);
        }
        now
    }

    /// Drop every matching mark on `tile`; returns how many rows were removed.
    pub fn remove_marks(&mut self, tile: i32, filter: &MarkFilter<'_>) -> i32 {
        let before = self.st.marks.len();
        self.st.marks.retain(|m| !(m.tile == tile && filter.matches(m)));
        (before - self.st.marks.len()) as i32
    }

    /// Destroy every counter unit owned by the instance at `uid` -- tile marks
    /// and player-held tokens alike. Called when the instance leaves the field
    /// (discard, unplace, player leave / bankrupt, event expire). The standing
    /// the tile-mark owner pseudo card is never destroyed, so its units remain.
    pub fn destroy_instance_units(&mut self, uid: i32) {
        if uid < 0 {
            return;
        }
        self.st.marks.retain(|m| m.instance != Some(uid));
        for p in self.st.players.iter_mut() {
            p.tokens.retain(|t| t.instance != Some(uid));
        }
    }

    // ------------------------------------------------- named card counters
    // The generic per-instance counter API. `name` is one of
    // [`crate::state::counter`] or a card-invented string. The **on-card**
    // location is the instance itself; tile / holder bindings live in
    // [`crate::state::TileMark`] / [`crate::state::Counter`].

    /// On-card count of `name` on the instance at `uid` (0 when absent).
    pub fn counter_at(&self, uid: i32, name: &str) -> i32 {
        let Some(f) = self.field_by_uid(uid) else {
            return 0;
        };
        f.counters.get(name).copied().unwrap_or(0)
    }

    /// Adjust the on-card count of `name` on the instance at `uid` by `n`,
    /// clamped at 0 and at `max` (`0` = uncapped). Returns the new count.
    pub fn add_counter_at(&mut self, uid: i32, name: &str, n: i32, max: i32) -> i32 {
        let Some(f) = self.field_by_uid_mut(uid) else {
            return 0;
        };
        let slot = f.counters.entry(name.to_string()).or_insert(0);
        *slot = (*slot + n).max(0);
        if max > 0 {
            *slot = (*slot).min(max);
        }
        let now = *slot;
        // Event instances mirror their named counter count into the public
        // `ActiveEvent::counter` row (`docs/EVENTS.md`).
        {
            let card = f.card.clone();
            let tile = f.tile;
            if tile < 0 && card.starts_with("event:") {
                let short = card.strip_prefix("event:").unwrap_or(&card);
                for e in self.st.event_active.iter_mut() {
                    if e.id == short {
                        e.counter = now;
                    }
                }
            }
        }
        now
    }

    /// Set the on-card count of `name` on the instance at `uid`.
    pub fn set_counter_at(&mut self, uid: i32, name: &str, n: i32) -> i32 {
        self.add_counter_at(uid, name, n - self.counter_at(uid, name), 0)
    }

    /// Units of `instance`'s counter `kind` bound to `tile`.
    pub fn count_bound_tile(&self, instance: i32, kind: &str, tile: i32) -> i32 {
        let instance = (instance >= 0).then_some(instance);
        self.st
            .marks
            .iter()
            .filter(|m| m.tile == tile && m.kind == kind && m.instance == instance)
            .map(|m| m.count)
            .sum()
    }

    /// Units of `instance`'s counter `name` held by `player_id`. Name-keyed
    /// fallback: a token name is unique to its creating rule, so a legacy row
    /// with `instance == -1` still counts.
    pub fn count_held(&self, instance: i32, name: &str, player_id: i32) -> i32 {
        let instance = (instance >= 0).then_some(instance);
        self.player_id(player_id)
            .and_then(|s| {
                s.tokens
                    .iter()
                    .find(|t| t.name == name && t.instance == instance)
                    .or_else(|| s.tokens.iter().find(|t| t.name == name))
            })
            .map_or(0, |t| t.value)
    }

    /// Bind `n` more units of `instance`'s counter `name` to `player_id`
    /// (a rule-created player token). Clamped at 0 / `max`. Returns how much
    /// actually moved.
    pub fn bind_held(&mut self, instance: i32, name: &str, player_id: i32, n: i32, max: i32) -> i32 {
        let instance = (instance >= 0).then_some(instance);
        let Some(s) = self.player_mut(player_id) else {
            return 0;
        };
        // Name-keyed: a token name is unique to its creating rule
        // (`marker_owner`), so a spend from any instance hits the same pool.
        // Prefer an exact (name, instance) row; fall back to any row with that
        // name (legacy `instance: None` rows from save migration).
        let idx = s
            .tokens
            .iter()
            .position(|t| t.name == name && t.instance == instance)
            .or_else(|| s.tokens.iter().position(|t| t.name == name));
        let slot = match idx {
            Some(i) => &mut s.tokens[i],
            None => {
                s.tokens.push(Counter {
                    name: name.to_string(),
                    value: 0,
                    instance,
                });
                s.tokens.last_mut().expect("just pushed")
            }
        };
        let before = slot.value;
        slot.value = (slot.value + n).max(0);
        if max > 0 {
            slot.value = slot.value.min(max);
        }
        slot.value - before
    }

    /// Move `n` units of `instance`'s counter `name` from one location to
    /// another. `from_tile`/`to_tile` >= 0 bind to that tile; use
    /// `from_player`/`to_player` >= 0 for a holder; the on-card pool is
    /// `from_tile == to_tile == -1 && from_player == to_player == -1` with the
    /// `*_on_card` flags. Simpler wire form: see `World::move_units`.
    pub fn move_units(
        &mut self,
        instance: i32,
        name: &str,
        from_tile: i32,
        from_player: i32,
        to_tile: i32,
        to_player: i32,
        n: i32,
    ) -> i32 {
        if n <= 0 || instance < 0 {
            return 0;
        }
        // Take from the source.
        let took = if from_tile >= 0 {
            let f = MarkFilter {
                kind: name,
                category: "",
                owner: None,
                src: None,
                instance: Some(instance),
            };
            let have = self.count_bound_tile(instance, name, from_tile);
            let take = n.min(have);
            if take <= 0 {
                return 0;
            }
            self.bump_mark(from_tile, &f, -take);
            take
        } else if from_player >= 0 {
            let have = self.count_held(instance, name, from_player);
            let take = n.min(have);
            if take <= 0 {
                return 0;
            }
            self.bind_held(instance, name, from_player, -take, 0);
            take
        } else {
            let have = self.counter_at(instance, name);
            let take = n.min(have);
            if take <= 0 {
                return 0;
            }
            self.add_counter_at(instance, name, -take, 0);
            take
        };
        // Put on the destination.
        if to_tile >= 0 {
            self.place_mark(instance, name, "", to_tile, -1, -1, took, Msg::default(), true);
        } else if to_player >= 0 {
            self.bind_held(instance, name, to_player, took, 0);
        } else {
            self.add_counter_at(instance, name, took, 0);
        }
        took
    }

    // ---------------------------------------------------------- tile marks
    // Under the bound-counter model (user ruling 2026-10-10) tile marks are
    // units of the **standing mark-owner pseudo card**'s named counter, bound
    // to tiles. The mark owner lives on the neutral board owner and is never
    // destroyed, so its units outlive any placer. Provenance (`TileMark::src`)
    // is the placing card instance; on-card marks of that card are its own
    // named counter and die with it.

    /// The uid of the standing mark-owner rule instance (`rule_id`) on the
    /// board owner, or -1 when the ruleset has no such rule (`StubRules`).
    pub fn mark_owner_uid(&self, rule_id: &str) -> i32 {
        self.mark_rule_instances()
            .into_iter()
            .find(|(_, c)| c == rule_id)
            .map(|(uid, _)| uid)
            .unwrap_or(-1)
    }

    // ------------------------------------------------------ generic layer writes

    /// One opaque layer write over the keyed state. The caller supplies the
    /// storage key; the engine never maps a kind id to a key name. Clamped at
    /// 0 only when the item declared [`crate::state::state_flag::NEVER_NEGATIVE`].
    ///
    /// The [反击] window is the separate generic `gate(p, kind_id)` verb.
    /// Wear-off is declared by the writer (`state_set_expires`).
    pub fn apply_layer(&mut self, player_id: i32, key: &str, n: i32) {
        if let Some(s) = self.player_mut(player_id) {
            s.state_add(key, n);
        }
    }

    /// Is the capability gate open for this player? Asks the ruleset (its
    /// CEL-only gate conditions). A ruleset with no opinion answers open --
    /// the engine keeps no named mirror.
    pub fn gate_open(&self, rules: &dyn super::CardRules, gate: super::Gate, player: usize) -> bool {
        if let Some(open) = rules.gate_open(self, gate, player) {
            return open;
        }
        true
    }

    /// Is the exclusive-use gate open for this player and card? The engine's
    /// generic own/base verdict is the caller's business (it is the default);
    /// this asks the ruleset's CEL-only `ExclusiveUse` entries (a group-skill
    /// card denying a variant its base persona's exclusives). `true` = no
    /// entry closed it.
    pub fn exclusive_use(
        &self,
        rules: &dyn super::CardRules,
        player: usize,
        ask: &super::ExclusiveAsk<'_>,
    ) -> bool {
        rules.exclusive_use(self, player, ask).unwrap_or(true)
    }

    /// Set this turn's main-move latch for `player_id`. `locked != 0` skips
    /// the move; `0` releases it. A generic turn-flow verb -- the timed-layer
    /// host's count-changed clause (rules.txt 48) drives it. **The off-board
    /// layer is not part of this latch**: it grants [无法移动] (glossary 13) but not
    /// 「跳过主要移动阶段」 (closeout-2 ruling). A mid-turn off-board entry
    /// (MyGO:无路矢) still owes the roll. Only the turn player's latch moves.
    pub fn skip_main_move(&mut self, player_id: i32, locked: i32) {
        if self.st.turn == player_id {
            self.st.skip_move = locked != 0;
        }
    }

    /// [`Self::skip_main_move`] with `locked = 0`.
    pub fn clear_move_skip(&mut self, player_id: i32) {
        self.skip_main_move(player_id, 0);
    }

    /// Generic "end this player's turn now" latch. A status host (or any
    /// guest) sets it during `turnStartBefore` / `turnStart`; `turn_start`
    /// honors it with the auto-skip pacing. No game vocabulary here.
    pub fn end_turn_now(&mut self, player_id: i32) {
        self.end_turn_request = Some(player_id);
    }

    /// The player gets another turn after this one.
    pub fn give_extra_turn(&mut self, player_id: usize) {
        if !self.extra_turns.contains(&player_id) {
            self.extra_turns.push(player_id);
        }
    }

    /// A generic named rule parameter in force: the match base
    /// ([`Self::rule_params`], or the code default when the map is empty /
    /// lacks the name) plus any card-applied bonus
    /// ([`Self::add_rule_param_bonus`]).
    pub fn rule_param(&self, name: &str) -> i32 {
        let base = self
            .rule_params
            .get(name)
            .copied()
            .unwrap_or_else(|| crate::data::default_rule_param(name));
        let bonus = self.rule_param_bonus.get(name).copied().unwrap_or(0);
        base + bonus
    }

    /// Add `n` to the named rule parameter's bonus; returns the new bonus.
    pub fn add_rule_param_bonus(&mut self, name: &str, n: i32) -> i32 {
        let e = self
            .rule_param_bonus
            .entry(name.to_string())
            .or_insert(0);
        *e += n;
        *e
    }

    /// Dice-rent unit multiplier (`rentUnitMult`): at least 1.
    pub fn rent_unit_mult(&self) -> i32 {
        self.rule_param("rentUnitMult").max(1)
    }

    pub fn add_rent_unit_bonus(&mut self, n: i32) -> i32 {
        self.add_rule_param_bonus("rentUnitMult", n)
    }

    /// Move a player to a tile without settling (a bare teleport).
    pub fn teleport_to(&mut self, player_id: i32, tile: i32) {
        if let Some(s) = self.player_mut(player_id) {
            s.pos = tile;
        }
    }

    // --------------------------------------------------------------- misc

    /// Something changed that only host-side state cares about -- bump the
    /// sequence so views and replays notice.
    pub fn touch(&mut self) {
        self.st.seq += 1;
    }

    fn player_id(&self, player_id: i32) -> Option<&crate::state::MatchPlayer> {
        usize::try_from(player_id)
            .ok()
            .and_then(|i| self.st.players.get(i))
    }

    fn player_mut(&mut self, player_id: i32) -> Option<&mut crate::state::MatchPlayer> {
        usize::try_from(player_id)
            .ok()
            .and_then(|i| self.st.players.get_mut(i))
    }
}

/// Zero-terminated named counters: keep only positive values.
fn set_named(list: &mut Vec<Counter>, name: &str, value: i32, instance: i32) {
    let v = value.max(0);
    match list.iter_mut().find(|c| c.name == name) {
        Some(c) => c.value = v,
        None => {
            if v > 0 {
                list.push(Counter {
                    name: name.to_string(),
                    value: v,
                    instance: (instance >= 0).then_some(instance),
                });
            }
        }
    }
    list.retain(|c| c.value > 0);
}
