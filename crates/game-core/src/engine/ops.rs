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

    /// `MinRoll` -- clamp the final face up to this after the counteractions.
    pub fn set_min_roll(&mut self, n: i32) {
        self.turn.plan.min_roll = n;
    }
    /// `ExtraSteps` -- extra steps added to the walk (it grows as it runs).
    pub fn set_extra_steps(&mut self, n: i32) {
        self.turn.plan.extra_steps = n;
    }

    /// `NoCircleReward` -- passing CiRCLE pays nothing on this walk.

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
    /// entry is `count`d`sides`, summed into `Parts`. `sides == 0` is a flat
    /// `count` (see [`crate::engine::move_ctx::Roll`]).
    pub fn set_base_dice(&mut self, count: i32, sides: i32, why: &str) {
        self.turn.plan.base.clear();
        if count > 0 && sides >= 0 {
            self.turn.plan.base.push(crate::engine::move_ctx::Roll {
                count,
                sides,
                why: why.to_string(),
            });
        }
    }
    /// `Base` -- add one more die group to the starting dice (3d20 = three of
    /// these, or one `count = 3`). `sides == 0` is a flat `count`.
    pub fn add_base_dice(&mut self, count: i32, sides: i32, why: &str) {
        if count > 0 && sides >= 0 {
            self.turn.plan.base.push(crate::engine::move_ctx::Roll {
                count,
                sides,
                why: why.to_string(),
            });
        }
    }
    /// `Dice` -- drop every extra die another effect added. A rewrite that
    /// names the whole face (「骰点就是1d10」) means *exactly* that face, not
    /// 1d10 plus whatever else is stacked on.
    pub fn clear_dice(&mut self) {
        self.turn.plan.dice.clear();
    }
    /// `Dice` -- extra dice added to the roll (summed into `Extra`). A flat add
    /// is a `0`-sided term: `add_extra_dice(n, 0, why)`.
    pub fn add_extra_dice(&mut self, count: i32, sides: i32, why: &str) {
        if count > 0 && sides >= 0 {
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
    /// `CanBuild` -- may build away from the landing (not just on it).
    pub fn set_can_build(&mut self, on: bool) {
        self.turn.plan.can_build = on;
    }
    /// `SettleAsAgent` -- settle on another player's behalf.
    pub fn set_settle_as_agent(&mut self, on: bool) {
        self.turn.plan.settle_as_agent = on;
    }
    /// 「使你的下次主要移动结果对那些玩家一起执行」 -- record a follower of the
    /// move being planned (C# `LeadFx.Who`). After the mover settles, the engine
    /// replays this move's result for each follower in the order recorded.
    pub fn plan_add_follower(&mut self, player_id: i32) {
        if player_id >= 0 && !self.turn.plan.followers.contains(&player_id) {
            self.turn.plan.followers.push(player_id);
        }
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

    /// The house count a **rent** lookup reads (`H.RentHouses`). Real
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

    /// Rent of a tile as it stands right now (houses included; `H.RentOf`).
    /// The house count is the **counted** one ([`Self::rent_houses`]).
    pub fn rent_of(&self, data: &GameData, tile: i32) -> i32 {
        let Some(t) = usize::try_from(tile).ok().and_then(|i| data.tiles.get(i)) else {
            return 0;
        };
        let houses = self.rent_houses(tile) as usize;
        if t.kind == "ring" {
            // RiNG rent is rolled at payment time; the table value is the base.
            t.price
        } else {
            t.rent
                .get(houses.min(t.rent.len().saturating_sub(1)))
                .copied()
                .unwrap_or(0)
        }
    }

    /// Price to buy a tile now (land + houses standing on it; `H.BuyPriceFor`).
    pub fn buy_price(&self, data: &GameData, tile: i32) -> i32 {
        let Some(t) = usize::try_from(tile).ok().and_then(|i| data.tiles.get(i)) else {
            return 0;
        };
        let houses = usize::try_from(tile)
            .ok()
            .and_then(|i| self.st.houses.get(i))
            .copied()
            .unwrap_or(0);
        t.price + houses * t.house
    }

    pub fn build_cost(&self, data: &GameData, tile: i32) -> i32 {
        usize::try_from(tile)
            .ok()
            .and_then(|i| data.tiles.get(i))
            .map_or(0, |t| t.house)
    }

    pub fn mortgage_value(&self, data: &GameData, tile: i32) -> i32 {
        usize::try_from(tile)
            .ok()
            .and_then(|i| data.tiles.get(i))
            .map_or(0, |t| t.price / 2)
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
                        && tile.kind != "ring"
                        && self.st.owners[t] == player as i32
                        && !self.st.mortgaged[t]
                })
                .map(|(_, tile)| tile.price / 2)
                .sum::<i32>()
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
        self.player_id(player_id)
            .map_or_else(StateVar::default, |s| s.state_var(key))
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

    /// Write `value`, clamped to the item's declared bounds (status floors at
    /// 0; `max > 0` is a real cap). See [`crate::state::MatchPlayer::state_set`].
    pub fn state_set(&mut self, player_id: i32, key: &str, value: i32) -> i32 {
        let Some(s) = self.player_mut(player_id) else {
            return 0;
        };
        s.state_set(key, value)
    }

    /// Add `delta`, clamped like [`Self::state_set`]. No log.
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
    /// Names of the player's counters whose name starts with `prefix`, in the
    /// order they were added. The listing half of a counter query -- [`Self::tok`]
    /// reads one of them by name.
    /// `H.DoMoveRoll` -- sum the move plan's `base` + `dice` tables into one
    /// face. Flat terms (`sides <= 0`) add their `count` directly, which is how a
    /// 「+2 to the roll」 effect rides along instead of a separate bonus field.
    /// Honours [`TurnCtx::extreme`]: a forced extreme settles the whole table at
    /// its theoretical max or min instead of rolling it.
    pub fn do_move_roll(&mut self, _player_id: i32) -> i32 {
        let plan = self.turn.plan.clone();
        if self.turn.extreme != 0 {
            let mut lo = 0;
            let mut hi = 0;
            for t in plan.base.iter().chain(plan.dice.iter()) {
                if t.sides <= 0 {
                    lo += t.count;
                    hi += t.count;
                } else {
                    lo += t.count.max(0);
                    hi += t.count.max(0) * t.sides.max(1);
                }
            }
            return if self.turn.extreme > 0 { hi } else { lo };
        }
        let mut total = 0;
        for t in plan.base.iter().chain(plan.dice.iter()) {
            if t.sides <= 0 {
                total += t.count;
            } else {
                for _ in 0..t.count.max(0) {
                    total += self.rng.d(t.sides.max(1));
                }
            }
        }
        self.turn.turn_rolls.push(total);
        total
    }

    /// One `count`d`sides` roll, honouring [`TurnCtx::extreme`].
    pub fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32 {
        let count = count.max(0);
        let sides = sides.max(1);
        let total = if self.turn.extreme > 0 {
            count * sides
        } else if self.turn.extreme < 0 {
            count
        } else {
            let mut sum = 0;
            for _ in 0..count {
                sum += self.rng.d(sides);
            }
            sum
        };
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
        self.turn.turn_rolls.push(total);
        total
    }

    /// C# `H.GainR` with `fixedAmount` -- the money moves, but no skill or crit
    /// may bend the figure (「立刻获得此次失去的资金金额」).
    ///
    /// The ledger still closes: the movement is logged as a `gain` event with
    /// its bank leg (`value`), even though it bypasses the `payAdd`/`payMul`/
    /// `payChoose` modifier pipeline.
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

    /// The uid of `player_id`'s **band-skill field instance** -- the bound band
    /// skill's field card (`skill:<band>:<skill>`, [`FieldCard::band_skill`]),
    /// which is where 「乐队卡 / 团卡」 crystals live. -1 when the player has
    /// none (no band skill bound, or it has left the field).
    ///
    /// Edge cases (the write target is always this one instance):
    /// * **no band skill** -- every read is 0 and every write is a no-op;
    /// * **several band cards** (PPP:Returns borrows another band's card beside
    ///   its own; the book's 「多张…乐队卡时此效果不重复发动」 is about several
    ///   *players* holding the same band card) -- the **first in placement
    ///   order** is the bound one (`bind_skills` places it first; a later copy
    ///   is a borrow that has not replaced it);
    /// * **swapped or removed** -- the instance goes with its crystals, so a
    ///   swap starts the new card at 0 and the old count is gone with the old
    ///   card (Returns' 「替换并移除上面的所有[奇迹水晶]」 is exactly that).
    pub fn band_skill_uid(&self, player_id: i32) -> i32 {
        self.player_id(player_id)
            .and_then(|s| s.field.iter().find(|f| f.band_skill))
            .map_or(-1, |f| f.uid)
    }

    /// 「乐队卡 / 团卡」 crystals -- the count on the player's band-skill field
    /// instance. 0 when there is no band skill. Same pool every band skill and
    /// every card that mentions the band card's crystals reads and writes.
    pub fn band_crystals(&self, player_id: i32) -> i32 {
        let uid = self.band_skill_uid(player_id);
        if uid < 0 {
            0
        } else {
            self.crystals_at(uid)
        }
    }

    /// C# `AddBandCrystals` -- a consumer of the passed cap, not the engine
    /// deciding one (`max` > 0 clamps, `max` = 0 is uncapped, exactly as
    /// [`Self::add_crystals_at`]). Returns the new count; 0 when the player has
    /// no band skill (writes are no-ops).
    pub fn add_band_crystals(&mut self, player_id: i32, n: i32, max: i32) -> i32 {
        let uid = self.band_skill_uid(player_id);
        if uid < 0 {
            return 0;
        }
        self.add_crystals_at(uid, n, max)
    }

    // ---------------------------------------------- skill / band attachments

    /// Rule id of the player's bound **band skill** (`skill:<band>:<skill>`),
    /// the one 「乐队卡 / 团卡」 crystals land on. `None` when there is none.
    pub fn band_skill_id(&self, player_id: i32) -> Option<String> {
        let uid = self.band_skill_uid(player_id);
        if uid < 0 {
            return None;
        }
        self.field_by_uid(uid).map(|f| f.card.clone())
    }

    /// Rule id of the player's **character skill** (`skill:<character>:<skill>`,
    /// C# `H._fx[i].skill`). A character skill is a `skill:` instance that is
    /// *not* a band skill; `bind_skills` places it beside the band one.
    pub fn character_skill_id(&self, player_id: i32) -> Option<String> {
        let s = self.player_id(player_id)?;
        s.field
            .iter()
            .find(|f| f.card.starts_with("skill:") && !f.band_skill)
            .map(|f| f.card.clone())
    }

    /// Every band-skill attachment on the player's field, as
    /// `(uid, rule id, extra)` in placement order (C# `H._fx[i].bands`).
    pub fn band_skills(&self, player_id: i32) -> Vec<(i32, String, i32)> {
        let Some(s) = self.player_id(player_id) else {
            return Vec::new();
        };
        s.field
            .iter()
            .filter(|f| f.band_skill)
            .map(|f| (f.uid, f.card.clone(), f.extra as i32))
            .collect()
    }

    /// Attach a band-skill instance (C# `H.MakeBand(band, user, extra)`).
    /// `extra` marks a 「拿取」ed copy: 「相同乐队技能卡的效果不可叠加」 (refused
    /// when an attachment of the same id is already there) and 「不视为那个乐队
    /// 的角色」 (`in_band` still reads only the character). Returns the new uid,
    /// or -1 when refused.
    pub fn add_band_skill(
        &mut self,
        data: &crate::data::GameData,
        player_id: i32,
        id: &str,
        extra: bool,
        props: std::collections::BTreeMap<String, i32>,
    ) -> i32 {
        if !data.is_band_skill(id) {
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
        let Some(s) = self.player_mut(player_id) else {
            return 0;
        };
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
        self.log(
            "fire",
            player_id,
            Msg::new("log.gain_fire")
                .text("who", name)
                .i("n", got)
                .msg("why", why)
                .i("fire", fire)
                .i("max", cap),
        );
        got
    }

    // ---------------------------------------------------------- money moves

    /// `H.GainR` -- money in, logged with its reason.
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

    /// `H.PayR` -- money out, logged. Routines that may need to raise funds are
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

    /// `H.DrawR` -- draw `n` cards, refilling as soon as the last card leaves.
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
            let over = (self.hidden[i].hand.len() > self.st.players[i].hand_limit() as usize)
                .then(|| Msg::new("log.part.over_hand").i("limit", self.st.players[i].hand_limit()));
            self.log(
                "draw",
                player_id,
                Msg::new("log.draw").player_id("who", player_id).i("n", got).opt("over", over),
            ).value = got;
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

    /// `H.PlaceFromPlay` -- the card becomes a field card at the player.
    /// `WhyNotBuildOn` -- may this player build on this tile at all? The one
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
        // 「[拥有者]不可盖房」 -- `prop::NO_BUILD` on a rule instance
        // (`docs/TILES.md`), not a keyed flag. The **source** owns the arming
        // and the disarming; the reader is the rule instance. Two placements
        // are read, the same shape as the CiRCLE veto: the player's own field
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
        if t.kind == "ring" || t.rent.len() < 2 {
            return Some(Msg::new("err.build_ring"));
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
        if s.out() || s.stunned() || s.exile() > 0 {
            return Some(Msg::new("err.cannot_spend"));
        }
        None
    }

    /// Is this placed card face-down? (`!p.FaceDown` in the C# field filters.)
    /// Flip a placed card face-down / face-up (C# `H.SwitchState`).
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

    /// Place a field card **on a tile** (C# `H.PlaceFromPlay(c, i, tile)`) -- the
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
        // A band skill's instance is the 「乐队卡 / 团卡」 crystal holder; see
        // [`Self::band_skill_uid`].
        let band_skill = data.is_band_skill(card);
        let inst = FieldCard {
            uid,
            card: card.to_string(),
            owner: player_id,
            user: player_id,
            tile,
            crystals: 0,
            cp: 0,
            face_down: false,
            immune: false,
            props,
            band_skill,
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
    /// tile. `mark:cp` is the [CP点] tile-mark owner. Like
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

    /// Miracle crystals on the instance at `uid` (C# `Card.Crystals`).
    pub fn crystals_at(&self, uid: i32) -> i32 {
        self.field_by_uid(uid).map_or(0, |f| f.crystals)
    }

    /// On-card [CP点] on the instance at `uid` (`FieldCard::cp`) -- 「自己[场上]
    /// N个[CP点]」, the CP points attached to *that* card (user ruling
    /// 2026-10-07). Crystals-like: the card rule's own stock, as against the
    /// neutral [CP点] tile marks below.
    pub fn cp_at(&self, uid: i32) -> i32 {
        self.field_by_uid(uid).map_or(0, |f| f.cp)
    }

    /// Adjust the on-card [CP点] on the instance at `uid` by `n`, clamped at 0
    /// and at `max` (`0` = uncapped); returns the new count. The `cpChanged`
    /// follow-up (通用:该清CP了's graveyard rule) hears about this write like
    /// any other.
    pub fn add_cp_at(&mut self, uid: i32, n: i32, max: i32) -> i32 {
        let Some(f) = self.field_by_uid_mut(uid) else {
            return 0;
        };
        f.cp = (f.cp + n).max(0);
        if max > 0 {
            f.cp = f.cp.min(max);
        }
        f.cp
    }

    /// Set the on-card [CP点] on the instance at `uid`; returns the new count.
    pub fn set_cp_at(&mut self, uid: i32, n: i32) -> i32 {
        self.add_cp_at(uid, n - self.cp_at(uid), 0)
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

    /// `H.AddCrystals` on the instance at `uid`; `max` caps (0 = uncapped).
    ///
    /// Also mirrors the count into [`crate::state::MatchState::event_active`]
    /// when the instance is an event's board-owner rule (`docs/EVENTS.md`):
    /// `ActiveEvent::counter` is the public view of the instance's crystals,
    /// and the raw row is what clients (and tests) read.
    pub fn add_crystals_at(&mut self, uid: i32, n: i32, max: i32) -> i32 {
        let Some(f) = self.field_by_uid_mut(uid) else {
            return 0;
        };
        f.crystals = (f.crystals + n).max(0);
        if max > 0 {
            f.crystals = f.crystals.min(max);
        }
        let now = f.crystals;
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
        now
    }

    /// Set the instance at `uid`'s crystals; returns the new count.
    pub fn set_crystals_at(&mut self, uid: i32, n: i32) -> i32 {
        self.add_crystals_at(uid, n - self.crystals_at(uid), 0)
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

    /// Place the player's skill rules on their field (C# `Fx`). This is the
    /// binding: see [`crate::data::GameData::skill_rules_of`]. Once placed,
    /// `On::Hook` reaches them like any other field card and `Card.Crystals`
    /// works on them. Idempotent -- calling it twice does not double-place.
    pub fn bind_skills(
        &mut self,
        data: &crate::data::GameData,
        rules: &dyn super::rules::CardRules,
        player_id: i32,
    ) {
        let character = self
            .player_id(player_id)
            .map(|s| s.character.clone())
            .unwrap_or_default();
        for id in data.skill_rules_of(&character) {
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
    /// The rule id comes from the tile's `kind` (`docs/TILES.md`):
    /// `tile:property` / `tile:ring` / `tile:agent` / `tile:circle` /
    /// `tile:edogawa` / `tile:event` (`cafe` and `ryuseido` share one).
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
            let id = crate::data::tile_rule_id(&tile.kind);
            if id.is_empty() {
                continue;
            }
            if self.tile_rule_instances(t as i32).iter().any(|(_, c)| c == id) {
                continue;
            }
            if !rules.has_rule(id) {
                // No such rule in this ruleset (StubRules). The engine's
                // built-in settlement covers it; nothing to bind.
                continue;
            }
            let mut props = rules.card_props(id);
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
            if tile.kind == "ring" {
                props.insert(prop::RING_MULT.to_string(), data.match_rules.ring_multiplier);
            }
            self.place_card_on(
                data,
                crate::state::BOARD_OWNER,
                t as i32,
                id,
                Msg::default(),
                props,
            );
        }
        // Board-wide **mark owners** (`mark:*`, `crate::data::mark_rule_ids`) --
        // one instance each on the neutral board owner, governing no single
        // tile (`tile = -1`). `mark:cp` is the [CP点] tile-mark owner
        // (`rules/tiles/src/cp.rs`): a board-wide category, so it is not bound
        // per board tile the way `tile:*` is. Idempotent.
        for id in crate::data::mark_rule_ids() {
            if !rules.has_rule(id) {
                continue;
            }
            if self
                .st
                .board_field
                .iter()
                .any(|f| f.card == *id && f.tile < 0)
            {
                continue;
            }
            let props = rules.card_props(id);
            self.place_card_on(
                data,
                crate::state::BOARD_OWNER,
                -1,
                id,
                Msg::default(),
                props,
            );
        }
    }

    /// Bind one **event rule** instance on the neutral board owner and record
    /// the event in [`crate::state::MatchState::event_active`] (`docs/EVENTS.md`).
    ///
    /// Called from `draw_event` when the event's rule exists in the ruleset, so
    /// the body's `On::Play` and any `On::Hook` it declares run against a live
    /// instance (props, crystals, `unplace_self` = expire). Idempotent per
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
    /// files it away itself when the rule reports it does not stay.
    pub fn expire_event(&mut self, id: &str, removed: bool) {
        let rid = crate::data::event_rule_id(id);
        if let Some(i) = self
            .st
            .board_field
            .iter()
            .position(|f| f.card == rid && f.tile < 0)
        {
            self.st.board_field.remove(i);
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

    /// `H.Unplace` -- take a field card off; returns its card id.
    pub fn unplace_card(&mut self, player_id: i32, card: &str) -> Option<String> {
        let s = self.player_mut(player_id)?;
        let k = s.field.iter().position(|f| f.card == card)?;
        Some(s.field.remove(k).card)
    }

    pub fn placed_cards(&self, player_id: i32) -> Vec<String> {
        self.player_id(player_id).map_or_else(Vec::new, |s| {
            s.field.iter().map(|f| f.card.clone()).collect()
        })
    }

    /// C# `Card.Immune` -- mark a placed field card as unaffected by other
    /// effects (「此卡不受…效果影响」). Effects that would touch it read
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

    /// Move a placed field card to `tile` (C# `card.Tile = t` / `H.Touch()`).
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

    /// Miracle crystals on a placed card (C# `Card.Crystals`).
    pub fn card_crystals(&self, player_id: i32, card: &str) -> i32 {
        self.player_id(player_id)
            .and_then(|s| s.field.iter().find(|f| f.card == card))
            .map_or(0, |f| f.crystals)
    }

    /// `H.AddCrystals` on a placed card; `max` caps the result (0 = uncapped).
    pub fn add_card_crystals(&mut self, player_id: i32, card: &str, n: i32, max: i32) -> i32 {
        let Some(f) = self
            .player_mut(player_id)
            .and_then(|s| s.field.iter_mut().find(|f| f.card == card))
        else {
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
            .filter(|m| {
                m.tile == tile
                    && (kind.is_empty() || m.kind == kind)
                    && (owner == -2 || m.owner == owner)
            })
            .map(|m| m.count)
            .sum()
    }

    /// Move one matching mark's `count` by `delta` (C# `mark.count--` for
    /// 「移除一个」). The mark is dropped when its count reaches 0. Returns the
    /// count now stored. Unlike [`Self::remove_marks`] this touches a single
    /// mark rather than clearing every match.
    pub fn bump_mark(&mut self, tile: i32, kind: &str, owner: i32, delta: i32) -> i32 {
        let Some(m) = self
            .st
            .marks
            .iter_mut()
            .find(|m| m.tile == tile && m.kind == kind && (owner < 0 || m.owner == owner))
        else {
            return 0;
        };
        m.count = (m.count + delta).max(0);
        let now = m.count;
        if now == 0 {
            self.st.marks.retain(|m| {
                !(m.tile == tile
                    && m.kind == kind
                    && (owner < 0 || m.owner == owner)
                    && m.count == 0)
            });
        }
        now
    }

    pub fn remove_marks(&mut self, tile: i32, kind: &str, owner: i32) -> i32 {
        let before = self.st.marks.len();
        self.st.marks.retain(|m| {
            !(m.tile == tile
                && (kind.is_empty() || m.kind == kind)
                && (owner == -2 || m.owner == owner))
        });
        (before - self.st.marks.len()) as i32
    }

    pub fn add_mark(&mut self, tile: i32, player_id: i32, kind: &str, note: Msg) {
        let uid = self.st.marks.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
        self.st.marks.push(TileMark {
            uid,
            tile,
            kind: kind.to_string(),
            category: crate::state::mark_category::PLAYER.to_string(),
            owner: player_id,
            count: 1,
            card: String::new(),
            src: -1,
            note,
        });
    }

    // ------------------------------------------------------- [CP点] marks
    // `docs/TILES.md` / `rules/tiles/src/cp.rs`: [CP点] is its own tile-mark
    // category, owned by the `mark:cp` rule instance on the neutral board
    // owner -- never by a player. Cards touch it only through this small API
    // (`place_cp` / `count_cp` / `clear_cp` / the attached counts), which is
    // what stamps the category and keeps `owner` at [`crate::state::BOARD_OWNER`].
    //
    // Provenance is `src` (the placing card instance's `FieldCard::uid`) plus
    // `card` (its card id, for the log / the view's 「来自」) -- not `owner`.
    // 通用:该清CP了 (1) 「此卡在格子上添加的[CP点]及其产物」 keys on `src`.

    /// Place one [CP点] on `tile`, attached to the card instance at `src_uid`
    /// (provenance; `-1` when not attached to an instance). The mark is neutral
    /// (`owner` = [`crate::state::BOARD_OWNER`]) regardless of who played the
    /// card. `card_id` is the provenance string the view shows as 「来自」.
    ///
    /// Placement semantics (通用:该清CP了 [手] 「在任意一个没有角色和[CP点]的
    /// 格子上添加1个[CP点]」) are the *caller's* gate -- the rule that says where
    /// a CP may go lives with `mark:cp`, not with every writer.
    pub fn add_cp_mark(&mut self, tile: i32, src_uid: i32, card_id: &str, note: Msg) -> i32 {
        // 「添加1个[CP点]」 -- stacking is +1 on the tile's single CP mark, so a
        // tile holds one mark object whose `count` is the [CP点] it carries.
        if let Some(m) = self.st.marks.iter_mut().find(|m| m.tile == tile && m.is_cp()) {
            m.count += 1;
            return m.count;
        }
        let uid = self.st.marks.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
        self.st.marks.push(TileMark {
            uid,
            tile,
            kind: crate::state::mark_kind::CP.to_string(),
            category: crate::state::mark_category::CP.to_string(),
            // Neutral: 「These marks should not be owned by any player」.
            owner: crate::state::BOARD_OWNER,
            count: 1,
            card: card_id.to_string(),
            src: src_uid,
            note,
        });
        1
    }

    /// [CP点] on `tile`, any provenance. `H.CountMarks` for the CP category.
    pub fn count_cp(&self, tile: i32) -> i32 {
        self.st
            .marks
            .iter()
            .filter(|m| m.tile == tile && m.is_cp())
            .map(|m| m.count)
            .sum()
    }

    /// [CP点] on `tile` that the card instance at `src_uid` placed (and its
    /// products) -- 通用:该清CP了 (1) 「此卡在格子上添加的[CP点]及其产物」.
    pub fn count_cp_from(&self, tile: i32, src_uid: i32) -> i32 {
        self.st
            .marks
            .iter()
            .filter(|m| m.tile == tile && m.is_cp() && m.src == src_uid)
            .map(|m| m.count)
            .sum()
    }

    /// Total [CP点] the card instance at `src_uid` placed anywhere on the board
    /// -- the tile marks carrying it as provenance (「此卡在格子上添加的[CP点]
    /// 及其产物」). Distinct from [`Self::cp_at`], the **on-card** [CP点] on the
    /// instance itself (user ruling 2026-10-07's two kinds).
    pub fn count_cp_from_all(&self, src_uid: i32) -> i32 {
        self.st
            .marks
            .iter()
            .filter(|m| m.is_cp() && m.src == src_uid)
            .map(|m| m.count)
            .sum()
    }

    /// The card instance a [CP点] on `tile` is attached to (`TileMark.src`),
    /// or `-1` when the tile has none. Read before a write: dropping the last
    /// mark takes its provenance with it.
    pub fn cp_src_at(&self, tile: i32) -> i32 {
        self.st
            .marks
            .iter()
            .find(|m| m.tile == tile && m.is_cp())
            .map_or(-1, |m| m.src)
    }

    /// Remove one [CP点] from `tile` (通用:该清CP了 [手] 「移除格子上的个[CP点]」
    /// -- C# `tileMark.count--`, at most one per [结算]). Returns how many are
    /// left there. The mark is dropped at 0. This is a **tile-mark** write: it
    /// does not touch the card's on-card [CP点] ([`Self::cp_at`]) and so does
    /// not raise `cpChanged` -- the graveyard rule watches the on-card count.
    pub fn clear_cp(&mut self, tile: i32) -> i32 {
        self.bump_cp(tile, -1)
    }

    /// Move the [CP点] on `tile` by `delta`; the mark is dropped at 0. Returns
    /// the count now on the tile.
    pub fn bump_cp(&mut self, tile: i32, delta: i32) -> i32 {
        let Some(m) = self.st.marks.iter_mut().find(|m| m.tile == tile && m.is_cp())
        else {
            return 0;
        };
        m.count = (m.count + delta).max(0);
        let now = m.count;
        let src = m.src;
        if now == 0 {
            self.st
                .marks
                .retain(|m| !(m.tile == tile && m.is_cp() && m.src == src && m.count == 0));
        }
        now
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
            // 「[停留]：处于该状态时[无法移动]」 -- the skip is a *consequence* of
            // the state, not a separate latch. Dropping the last layer (with no
            // [除外] holding the move either) un-skips the turn's main move, the
            // same as the C#'s `H.State.skipMove = false` when 壱雫空 clears it.
            let unskip = v == 0 && s.exile() == 0;
            if unskip && self.st.turn == player_id {
                self.st.skip_move = false;
            }
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

/// Zero-terminated named counters: keep only positive values (`H.SetTok`).
fn set_named(list: &mut Vec<Counter>, name: &str, value: i32) {
    let v = value.max(0);
    match list.iter_mut().find(|c| c.name == name) {
        Some(c) => c.value = v,
        None => {
            if v > 0 {
                list.push(Counter {
                    name: name.to_string(),
                    value: v,
                });
            }
        }
    }
    list.retain(|c| c.value > 0);
}
