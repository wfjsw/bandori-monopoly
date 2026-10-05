//! The world a card effect can see and change.
//!
//! `game-core`'s match state implements this in the `WasmRules` bridge. The
//! `Clone` bound is what makes replay work: every run starts from a clone of the
//! snapshot, and only a run that finishes replaces the real state. The RNG
//! **must** live inside the world so that a replayed run draws the same numbers.
//!
//! The methods mirror the C# `H.*` vocabulary the card classes use. Anything
//! that needs a player decision is deliberately absent: the host surfaces it as
//! a [`crate::Prompt`] and the run is replayed with the answer.

use game_core::msg::Msg;

/// The trigger a reaction is checked against (C# `Trigger`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trigger {
    pub kind: crate::TriggerKind,
    pub player_id: i32,
    /// `t.Target` (`t.Pay.to` on pay triggers).
    pub target: i32,
    /// `t.Tile` (the tile settled on, passed, mortgaged, ...).
    pub tile: i32,
    /// `t.Value` (`t.Pay.amount` on pay triggers).
    pub value: i32,
    /// `t.Step` -- the turn step (0/1/2/3) active when this trigger fired.
    pub step: i32,
    /// `t.ByCard` -- the player whose card caused this trigger, or `None`.
    pub by_card: Option<i32>,
    /// `t.Pay.IsRent` on `pay`/`paid` triggers.
    pub pay_is_rent: bool,
    /// `t.Move` -- flags on the move that caused this trigger (C# `t.Move`);
    /// `None` when the move did not cause it. See [`card_sdk::abi::MoveKind`].
    pub move_kind: Option<card_sdk::abi::MoveKind>,
    /// `t.Move.Resolve` (C# `m.Resolve`) -- does that move settle where it
    /// lands? False = a card effect prevented settle at all.
    pub move_resolve: bool,
    /// `t.Move.Tags` -- per-card counters on the move (a [火罐] roll is
    /// card-owned state: whoever arms it tags the move, readers ask).
    pub move_tags: Vec<(String, i32)>,
    /// `t.Move.Main` -- was this the turn's main move (C# `MoveCtx.main`)?
    pub move_main: bool,
    /// `t.Move.Dir` -- 1 forward, -1 backward. Only meaningful when the move caused it.
    pub move_dir: i32,
    /// `Trigger.Cancelled` -- a reaction negated this trigger's effect.
    pub cancelled: bool,
    /// `t.Move.Remaining` / `t.Move.Path.Count`.
    pub move_remaining: i32,
    pub move_total: i32,
    /// The cards a `drew` trigger is about.
    pub cards: Vec<String>,
    /// `t.Move.Roll`; `None` when there is no move or it was cancelled.
    pub move_roll: Option<i32>,
    /// `t.Card` -- the card id on card/event/reacted triggers (`""` otherwise).
    pub card: String,
}

pub trait CardWorld: Clone + 'static {
    // ---------------------------------------------------------- dice & log
    /// Sum of `count` d`sides` from the match RNG; also records a dice event.
    fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32;
    /// Log line (a localizable message from the card).
    fn log(&mut self, player_id: i32, msg: Msg);

    // ------------------------------------------------------------- board
    fn tile_count(&self) -> i32;
    /// Tile index of a name (a data key), or -1.
    fn tile_named(&self, name: &str) -> i32;
    fn tile_owner(&self, tile: i32) -> i32;
    fn player_pos(&self, player_id: i32) -> i32;
    /// Tile `steps` around the ring from a player (-1 when the player is out).
    fn tile_steps_ahead(&self, player_id: i32, steps: i32) -> i32;
    fn rent_of(&self, tile: i32) -> i32;
    fn buy_price(&self, tile: i32) -> i32;
    fn build_cost(&self, tile: i32) -> i32;
    fn mortgage_value(&self, tile: i32) -> i32;
    fn owned_count(&self, player_id: i32) -> i32;
    fn owned_at(&self, player_id: i32, index: i32) -> i32;
    /// `TileData.IsBuyable` (a deed tile).
    fn is_buyable(&self, tile: i32) -> i32;
    /// C# `H.IsShop` -- a 商店街 deed (buyable, colour group 10).
    fn is_shop(&self, tile: i32) -> i32;
    /// `TileData.kind == "ring"` -- a RiNG deed.
    fn is_ring(&self, tile: i32) -> i32;
    /// `TileData.kind == "circle"` -- the CiRCLE tile.
    fn is_circle(&self, tile: i32) -> i32;
    /// C# `H.IsLiveHouse` -- a Live House deed (buyable, colour group 6).
    fn is_live_house(&self, tile: i32) -> i32;
    /// `TileData.group` -- colour group (-1 for no tile).
    fn tile_group(&self, tile: i32) -> i32;
    /// `H._tiles[t].price` -- the land price alone (cf. `buy_price`).
    fn tile_price(&self, tile: i32) -> i32;
    /// `H.State.houses[t]`.
    fn houses_of(&self, tile: i32) -> i32;
    /// Set the tile's house count (house transfer effects).
    fn set_houses(&mut self, tile: i32, n: i32);
    /// `H.AddHouse` -- returns the new count.
    fn add_house(&mut self, tile: i32, n: i32) -> i32;
    /// `H.State.mortgaged[t]` (1 = mortgaged).
    fn mortgaged_of(&self, tile: i32) -> i32;
    /// Mortgage/redeem outright (transfer effects; no money moves).
    fn set_mortgaged(&mut self, tile: i32, v: i32);
    /// `H.State.owners[t] = seat` -- hand a deed over outright.
    fn set_owner(&mut self, tile: i32, player_id: i32);
    /// C# `H.Dist` -- undirected ring distance.
    fn dist(&self, a: i32, b: i32) -> i32;
    /// C# `H.Forward` -- steps forward from `a` to `b`.
    fn tile_forward(&self, a: i32, b: i32) -> i32;
    /// C# `H.Neighbor` -- next present player in direction `dir` (±1), or -1.
    fn neighbor(&self, player_id: i32, dir: i32) -> i32;
    /// C# `H.SeatsOn` -- present players on a tile, minus `except` (-1 = none).
    fn players_on_count(&self, tile: i32, except: i32) -> i32;
    fn players_on_at(&self, tile: i32, except: i32, index: i32) -> i32;

    // -------------------------------------------------------------- players
    fn player_count(&self) -> i32;
    fn player_out(&self, player_id: i32) -> i32;
    fn others_count(&self, player_id: i32) -> i32;
    fn others_at(&self, player_id: i32, index: i32) -> i32;
    fn money(&self, player_id: i32) -> i32;
    /// `H.GainR` -- money in, logged with its reason. Returns what was gained.
    fn gain(&mut self, player_id: i32, amount: i32, src: Msg) -> i32;
    /// `H.PayR` -- money out (what the player could pay), logged.
    fn pay(&mut self, player_id: i32, amount: i32, src: Msg) -> i32;

    // ------------------------------------------------------------ hand/deck
    /// `H.DrawR` -- draw `n` cards (reshuffles when needed). Returns drawn count.
    fn draw(&mut self, player_id: i32, n: i32) -> i32;
    fn add_to_hand(&mut self, player_id: i32, card: &str);
    fn add_to_deck(&mut self, player_id: i32, card: &str, shuffle: bool);
    /// C# `H.AddToDeck(seat, card, where)` -- `pos`: 0 = top, 1 = bottom,
    /// 2 = shuffle in (the C# default).
    fn add_to_deck_at(&mut self, player_id: i32, card: &str, pos: i32);
    /// Take one copy of `card` out of `pile` *without* sending it anywhere
    /// (C# `_hidden[s].hand.Remove(card)`); returns whether it was there.
    fn take_card(&mut self, player_id: i32, pile: crate::CardPile, card: &str) -> bool;
    /// Every card id in a player's `pile`, in pile order (the deck top first).
    fn cards_in(&self, player_id: i32, pile: crate::CardPile) -> Vec<String>;
    fn to_discard(&mut self, player_id: i32, card: &str);
    /// Copies of `card` in hand (C# `_hidden[s].hand` count).
    fn hand_count(&self, player_id: i32, card: &str) -> i32;
    /// Total cards in hand (C# `_hidden[s].hand.Count`).
    fn hand_size(&self, player_id: i32) -> i32;
    /// Copies of `card` in the discard pile.
    fn discard_count(&self, player_id: i32, card: &str) -> i32;
    /// Draw-pile size.
    fn deck_count(&self, player_id: i32) -> i32;
    /// Discard-pile size (all ids).
    fn discard_size(&self, player_id: i32) -> i32;
    /// C# `H.DiscardFromHand` -- one copy hand -> discard; 1 when it was held.
    fn discard_from_hand(&mut self, player_id: i32, card: &str) -> i32;
    /// C# `H.ShuffleAllIntoDeck(seat, hand, discard)` -- the hand and/or the
    /// discard pile into the draw pile, shuffled.
    fn shuffle_into_deck(&mut self, player_id: i32, hand: bool, discard: bool) -> i32;

    // -------------------------------------------------------- field cards
    /// `H.PlaceCard` -- a card (usually this one) stays in play at the player.
    fn place_card(&mut self, player_id: i32, card: &str, note: Msg);
    /// `PlayCtx.Dest` -- where this card goes when its effect finishes.
    fn set_dest(&mut self, dest: i32);
    /// `H.Unplace` -- take the card out of play (`true` when it was there).
    fn unplace_card(&mut self, player_id: i32) -> bool;
    fn is_placed(&self, player_id: i32) -> i32;
    /// Miracle crystals on the *current* card placed at `player_id` (C# `Card.Crystals`).
    fn crystals(&self, player_id: i32) -> i32;
    /// Set the current card's crystals at `player_id`; returns the new count.
    fn set_crystals(&mut self, player_id: i32, n: i32) -> i32;
    /// `H.AddCrystals` -- adjust the current card's crystals at `player_id` by `n`,
    /// clamped at 0 and at `max` (0 = uncapped); returns the new count.
    fn add_crystals(&mut self, player_id: i32, n: i32, max: i32) -> i32;

    // ------------------------------------------------------ marks & tokens
    /// `H.AddMark` -- a marker on a tile (`kind` names it, `note` explains it).
    fn add_mark(&mut self, tile: i32, player_id: i32, kind: &str, note: Msg);
    fn count_marks(&self, tile: i32, kind: &str, owner: i32) -> i32;
    fn remove_marks(&mut self, tile: i32, kind: &str, owner: i32) -> i32;
    fn tok(&self, player_id: i32, name: &str) -> i32;
    fn set_tok(&mut self, player_id: i32, name: &str, value: i32);
    /// Returns how much the counter actually moved by.
    fn add_tok(&mut self, player_id: i32, name: &str, n: i32, max: i32) -> i32;

    // ---------------------------------------------------------- keyed state
    /// Dumb keyed storage: `{value, min, max, expires}` items. The engine holds
    /// them and enforces nothing (see [`game_core::state::StateVar`]) -- which
    /// keys mean what, and what their caps are, belongs to whoever uses them.
    /// The named accessors below (`stun_of`, `fire`, `slot`, ...) are sugar over
    /// this, not the other way round.
    fn state_var(&self, player_id: i32, key: &str) -> game_core::state::StateVar;
    fn state_get(&self, player_id: i32, key: &str) -> i32;
    /// The bound a consumer may enforce. 0 when none has been mandated.
    fn state_min(&self, player_id: i32, key: &str) -> i32;
    fn state_max(&self, player_id: i32, key: &str) -> i32;
    fn state_expires(&self, player_id: i32, key: &str) -> Option<game_core::state::Tick>;
    /// Write `value` raw. Deliberately not clamped to `min`/`max`.
    fn state_set(&mut self, player_id: i32, key: &str, value: i32) -> i32;
    /// Add `delta` raw. No clamping, no logging.
    fn state_add(&mut self, player_id: i32, key: &str, delta: i32) -> i32;
    /// Declare the bounds a consumer may enforce. Stored, not applied.
    fn state_set_bounds(&mut self, player_id: i32, key: &str, min: i32, max: i32);
    /// Declare when this counter wears off.
    fn state_set_expires(&mut self, player_id: i32, key: &str, expires: Option<game_core::state::Tick>);
    /// Tick every timed counter whose expiry is due; `(key, left)` for each.
    fn tick_state(&mut self, player_id: i32, when: game_core::state::Tick) -> Vec<(String, i32)>;

    // -------------------------------------------------- per-player slots (V)
    /// A free-form counter (C# `H.V`). Sugar over the keyed state.
    fn slot(&self, player_id: i32, key: &str) -> i32 {
        self.state_get(player_id, key)
    }
    fn set_slot(&mut self, player_id: i32, key: &str, value: i32) {
        self.state_set(player_id, key, value);
    }
    fn inc_slot(&mut self, player_id: i32, key: &str, by: i32) -> i32 {
        self.state_add(player_id, key, by)
    }

    // ------------------------------------------------------- status pots
    /// Sugar over the keyed state.
    fn band_crystals(&self, player_id: i32) -> i32 {
        self.state_get(player_id, game_core::state::key::BAND_CRYSTALS)
    }
    fn add_band_crystals(&mut self, player_id: i32, n: i32, max: i32) -> i32;
    /// Fire pots held. Sugar over the keyed state.
    fn fire(&self, player_id: i32) -> i32 {
        self.state_get(player_id, game_core::state::key::FIRE)
    }
    /// The mandated fire-pot cap -- the `max` of the `fire` item, which a
    /// character skill writes. The engine does not impose it.
    fn fire_max(&self, player_id: i32) -> i32 {
        self.state_max(player_id, game_core::state::key::FIRE)
    }
    fn gain_fire(&mut self, player_id: i32, n: i32, why: Msg) -> i32;
    fn give_stay(&mut self, player_id: i32, n: i32);
    fn give_stun(&mut self, player_id: i32, n: i32);
    fn give_exile(&mut self, player_id: i32, n: i32, to: i32);
    fn give_extra_turn(&mut self, player_id: i32);
    /// `H.CanPay` -- not out, not stunned, not exiled.
    fn can_pay(&self, player_id: i32) -> i32;
    /// C# `H.MoveWhyNot` -- 0 = the main move is still available, 1 = not your
    /// turn, 2 = already moved, 3 = cannot move this turn.
    fn cant_move(&self, player_id: i32) -> i32;
    /// C# `H.SpendFire` -- spend `n` [火罐]; 1 when the player had enough (logged).
    fn spend_fire(&mut self, player_id: i32, n: i32, why: Msg) -> i32;
    /// `[停留]` layers (C# `H.State.seats[s].stay`). Sugar over the keyed state.
    fn stay_of(&self, player_id: i32) -> i32 {
        self.state_get(player_id, game_core::state::key::STAY)
    }
    /// `[晕眩]` layers (C# `H.State.seats[s].stun`). Sugar over the keyed state.
    fn stun_of(&self, player_id: i32) -> i32 {
        self.state_get(player_id, game_core::state::key::STUN)
    }
    /// `H.State.turn` -- whose turn (-1 when none).
    fn turn_player(&self) -> i32;
    /// `H.State.round`.
    fn round_no(&self) -> i32;
    /// C# `H.TurnKey` -- `round * 100 + turn + 1` (once-per-turn latches).
    fn turn_key(&self) -> i32;
    /// Is the player's character exactly `name`?
    fn character_is(&self, player_id: i32, name: &str) -> i32;
    /// C# `H.BandOf(seat) == name`.
    fn in_band(&self, player_id: i32, name: &str) -> i32;

    // --------------------------------------------------- ring & movement
    /// `H.RingMultiplier` -- the RiNG rent multiplier right now.
    fn ring_multiplier(&self) -> i32;
    /// `H._ringBonus` -- nudge the RiNG multiplier; returns the new value.
    fn add_ring_bonus(&mut self, n: i32) -> i32;
    /// `H.ForceTeleport(..., resolve: false)` -- move a player without settling.
    fn teleport_to(&mut self, player_id: i32, tile: i32);

    // ---------------------------------------------------------- trigger
    fn trigger(&self) -> Trigger;
    fn set_trigger_move_roll(&mut self, roll: i32);
    /// Rewrite the pending amount on a `pay`/`paid` trigger (C# `PayCtx.amount`);
    /// 0 cancels the payment (C# `t.Pay.cancel`).
    fn set_trigger_value(&mut self, value: i32);
    /// Redirect the payee of a pending pay (C# `PayCtx.to`); -1 = the bank.
    fn set_trigger_target(&mut self, to: i32);
    /// Negate the trigger's effect (C# `trigger.Cancelled = true`).
    fn set_trigger_cancelled(&mut self);
    /// `t.Card == id` -- is this trigger about that card?
    fn trig_card_is(&self, id: &str) -> i32;

    // ----------------------------------------------- turn plan & scheduling
    // Defaults are no-ops so test worlds need not implement them.

    /// Call the running card's `react` (kind `turnEnd`) for `player_id` at a turn
    /// end: this turn's end, or -- `next_of_player` -- the end of `player_id`'s next
    /// turn (C# `TurnCtx.AfterEnd` / "你的下回合结束时").
    fn schedule_turn_end(&mut self, _player: i32, _next_of_player: bool, _early: bool) {}
    /// C# `TurnCtx.NoMoneyLoss` -- `player_id`'s money cannot drop this turn.
    fn set_no_money_loss(&mut self, _player: i32) {}
    /// C# `TurnCtx.Plan.FixedRoll` -- this turn's main-move roll.
    fn set_fixed_roll(&mut self, _n: i32) {}
    /// The fixed main-move roll, or -1 when none is set.
    fn fixed_roll(&self) -> i32 {
        -1
    }
    /// C# `NextStepsFx` -- `player_id`'s next main move walks exactly `n` steps.
    fn set_next_steps(&mut self, _player: i32, _n: i32) {}
    /// C# `TurnCtx.LastMain` -- steps this turn's main move walked (0 = none).
    fn turn_main_steps(&self) -> i32 {
        0
    }
    /// C# `Card.FireMaxDelta` -- adjust `player_id`'s [火罐] cap; returns the new cap.
    fn add_fire_max(&mut self, _player: i32, _n: i32) -> i32 {
        0
    }
    /// Make `id` the running card for a nested `play_card` (fresh `Dest`);
    /// returns what to hand back to [`Self::leave_card`].
    fn enter_card(&mut self, _id: &str) -> (String, i32) {
        (String::new(), 0)
    }
    /// Restore the outer card after a nested `play_card`; returns the inner
    /// card's `Dest`.
    fn leave_card(&mut self, _saved: (String, i32)) -> i32 {
        0
    }

    /// C# `_abnormalTurn[player_id]`.
    fn abnormal_count(&self, _player: i32) -> i32 {
        0
    }
    /// C# `H._targeted[player_id]`.
    fn targeted_count(&self, _player: i32) -> i32 {
        0
    }
    /// The tile `id` is bound to on `player_id`'s field (-1 = not on a tile), or -2
    /// when the player has no such card in play.
    fn placed_tile(&self, _player: i32, _id: &str) -> i32 {
        -2
    }
    /// C# `PlayCtx.Doubled`, or -1.
    fn play_doubled(&self) -> i32 {
        -1
    }

    // ----------------------------------------- movement shaping (TurnCtx.plan)
    // Defaults are no-ops so test worlds need not implement them.
    //
    // What lives here is the move's own plan/geometry plus what a card shapes.
    // Deliberately *not* here, and why:
    //   - `by` / `forced` -- who imposed the move: a property of the trigger,
    //     which the reaction already sees (`Trigger.by_card`).
    //   - `first_roll` -- history; the trigger's `value` at the `roll` point.
    //   - `roll_text` / `part_text` / `extra_text` -- presentation; the roll
    //     builds a `Msg` for its log line.
    //   - `heat` -- hardcoded card content (the 超燃甩头 event); the card does
    //     `add_extra_dice(1, 6, ...)`.
    //   - `more_steps` -- a queued follow-up walk, same store as `next_steps`.
    //   - `fixed_roll` -- the queue lives on TurnCtx; it seeds the plan.
    //   - `pay_factor` / `rent_factor` -- payment scalars: they act in the
    //     payment step, which also runs outside a move.
    //   - `settle_tile` -- the settle target is where the walk stops, so
    //     `stop_at` already names it.
    //   - `build_anywhere` -- the inverse of `no_build`; one flag is enough.
    /// C# `SetSteps` -- the planned length; keeps the sign of a reverse walk.
    fn set_steps(&mut self, _v: i32) {}
    /// C# `Reverse` -- walk backwards.
    fn set_reverse(&mut self, _v: bool) {}
    /// C# `Signed` -- a negative roll walks backwards instead of clamping to 0.
    fn set_signed(&mut self, _v: bool) {}
    /// C# `StopAt` -- force the walk to stop on this tile; -1 clears.
    fn set_stop_at(&mut self, _v: i32) {}
    /// C# `Parity` -- only odd (1) / even (0) tiles count; -1 either.
    fn set_parity(&mut self, _v: i32) {}
    /// C# `Resolve` -- settle where the walk lands. Clear it for 「不触发结算」.
    fn set_resolve(&mut self, _v: bool) {}
    /// How the move gets there (0 = walk, 1 = teleport).
    fn set_kind(&mut self, _v: i32) {}
    /// The teleport's destination; -1 derives it from the roll.
    fn set_teleport_to(&mut self, _v: i32) {}
    /// The tile the walk begins on instead of the player's own; -1 for the player.
    fn set_start(&mut self, _v: i32, _why: &str) {}
    /// Extra steps added mid-walk (the walk grows as it runs).
    fn set_extra_steps(&mut self, _v: i32) {}
    /// Floor applied to the final face (unsigned rolls only).
    fn set_min_roll(&mut self, _v: i32) {}
    /// The landing cannot be bought (「该次传送不可进行地契购买」).
    fn set_no_buy(&mut self, _v: bool) {}
    /// The landing cannot be built on.
    fn set_no_build(&mut self, _v: bool) {}
    /// Passing CiRCLE pays nothing on this walk.
    fn set_no_circle_reward(&mut self, _v: bool) {}
    /// Replace the starting dice (default 1d20). `sides == 0` is a flat `count`.
    fn set_base_dice(&mut self, _count: i32, _sides: i32, _why: &str) {}
    /// Add one more die group to the starting dice.
    fn add_base_dice(&mut self, _count: i32, _sides: i32, _why: &str) {}
    /// Add an extra die group to the roll.
    fn add_extra_dice(&mut self, _count: i32, _sides: i32, _why: &str) {}
    /// C# `SettleTile` -- settle on this tile instead of the landing; -1 clears.
    fn set_settle_tile(&mut self, _v: i32) {}
    /// C# `PayFactor` -- scale money paid for this walk. Milli-units (500 = x0.5).
    fn set_pay_factor(&mut self, _v: i32) {}
    /// C# `RentFactor` -- scale rent paid for this walk, same units.
    fn set_rent_factor(&mut self, _v: i32) {}
    /// C# `BuildAnywhere` -- may build away from the landing.
    fn set_build_anywhere(&mut self, _v: bool) {}
    /// C# `SettleAsAgent`.
    fn set_settle_as_agent(&mut self, _v: bool) {}
    /// C# `MoreSteps` -- a queued second walk, in steps.
    fn set_more_steps(&mut self, _v: i32) {}
    /// Card-owned per-move state (fire-roll counters etc.).
    fn set_tag(&mut self, _key: &str, _v: i32) {}
    fn move_tag(&self, _key: &str) -> i32 {
        0
    }
    /// C# `StopAt` -- where the walk is forced to stop, or -1.
    fn move_stop_at(&self) -> i32 {
        -1
    }
    /// C# `Parity`: -1 either, 0 even, 1 odd.
    fn move_parity(&self) -> i32 {
        -1
    }
    /// C# `m.Resolve` -- does the planned move settle where it lands?
    fn move_resolve(&self) -> bool {
        true
    }
    /// The move being planned (C# `TurnCtx.Plan`), captured whole so a
    /// `card_move` host request can hand it to the engine (the shaping a card
    /// did via the plan ops lives on this run's world copy, which is discarded
    /// when the run suspends).
    fn move_plan(&self) -> game_core::engine::MoveCtx {
        game_core::engine::MoveCtx::default()
    }
    /// How the planned move gets there (0 = walk, 1 = teleport).
    fn move_kind(&self) -> i32 {
        0
    }
    /// The planned length (`MovePlan.Landing` is where it ends).
    fn move_steps(&self) -> i32 {
        0
    }
    /// C# `Remaining` -- steps left to walk.
    fn move_remaining(&self) -> i32 {
        0
    }
    /// C# `Total` -- steps walked (`lastWalk` is written from this).
    fn move_total(&self) -> i32 {
        0
    }
    /// C# `Dir` -- +1 forwards, -1 backwards.
    fn move_dir(&self) -> i32 {
        0
    }
}