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

/// The trigger a counteraction is checked against (C# `Trigger`).
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
    /// `t.Step` -- the turn stage (0 = no turn; 1 开始 / 2 运营 / 3 移动 / 4 结束) active when this trigger fired.
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
    /// How a counter invalidated this link -- see [`game_core::engine::rules::Negation`].
    pub negation: game_core::engine::rules::Negation,
    /// Recipients a counter spared from settlement.
    pub spared: Vec<i32>,
    /// Position within the current chain, 1-based. 0 = not on a chain.
    pub seq: u32,
    /// The link this one answers. 0 = the effect declaration itself.
    pub answers: u32,
    /// The effects this link declares, recipients already named.
    pub effects: Vec<game_core::engine::rules::Effect>,
    /// `t.Move.Remaining` / `t.Move.Path.Count`.
    pub move_remaining: i32,
    pub move_total: i32,
    /// The cards a `drew` trigger is about.
    pub cards: Vec<String>,
    /// `t.Roll.Source` -- where a `roll` / `moveRoll` face came from, as a
    /// `card_sdk::abi::roll_source` code (`0` = unattributed, `1` = a fire
    /// pot, `2` = a hand/field card, `3` = a skill press). 「当你使用火罐进行
    /// 掷骰时」 reads this.
    pub roll_source: i32,
    /// `t.Move.Roll`; `None` when there is no move or it was cancelled.
    pub move_roll: Option<i32>,
    /// `t.Card` -- the card id on card/event/counteracted triggers (`""` otherwise).
    pub card: String,
}

impl Trigger {
    /// Did a counter negate this link at all?
    pub fn is_cancelled(&self) -> bool {
        self.negation != game_core::engine::rules::Negation::None
    }

    /// Negate the activation: the link never happened.
    pub fn negate_activation(&mut self) {
        self.negation = game_core::engine::rules::Negation::Activation;
    }

    /// Negate the effect: it happened, but settles to nothing. Does not weaken
    /// a stronger negation already in place.
    pub fn negate_effect(&mut self) {
        use game_core::engine::rules::Negation;
        if self.negation == Negation::None {
            self.negation = Negation::Effect;
        }
    }

    /// Take one recipient out of settlement.
    pub fn spare(&mut self, seat: i32) {
        if !self.spared.contains(&seat) {
            self.spared.push(seat);
        }
    }

    /// Append a declared effect. The recipient is named now, at declaration.
    pub fn declare(&mut self, effect: game_core::engine::rules::Effect) {
        self.effects.push(effect);
    }
}

pub trait CardWorld: Clone + 'static {
    // ---------------------------------------------------------- dice & log
    /// Sum of `count` d`sides` from the match RNG; also records a dice event.
    /// Announce which effect a card just applied (`H.Effect`). Unlike [`Self::log`]
    /// this reaches the player as a popup as well as a log line -- the two are
    /// additive. Defaults to a plain log so test worlds need not implement it.
    fn effect(&mut self, player_id: i32, msg: Msg) {
        self.log(player_id, msg);
    }

    fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32;
    /// C# `PlayCtx.Extreme` -- 1 = settle number ranges at their max, -1 = min.
    fn extreme(&self) -> i32 {
        0
    }
    fn set_extreme(&mut self, _v: i32) {}
    /// Did the play being resolved come from the hand?
    fn play_from_hand(&self) -> bool {
        true
    }
    fn set_play_from_hand(&mut self, _v: bool) {}
    /// A gain skills / crits may not bend (C# `fixedAmount`).
    fn gain_fixed(&mut self, _player_id: i32, _amount: i32, _why: crate::Msg) -> i32 {
        0
    }
    /// Flip a placed card face-down / face-up (C# `H.SwitchState`).
    fn set_card_face_down(&mut self, _player_id: i32, _card: &str, _down: bool) -> bool {
        false
    }
    /// `H.DoMoveRoll` -- sum the planned move's dice tables into one face.
    fn do_move_roll(&mut self, _player_id: i32) -> i32 {
        0
    }
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
    /// C# `_tileColors[t]` / `Fx.ExtraColor` -- re-colour a tile for everyone /
    /// for one player. `-1` clears; `key::ALL_COLORS` = every colour.
    fn set_tile_color(&mut self, _tile: i32, _group: i32) {}
    fn set_extra_color(&mut self, _player_id: i32, _tile: i32, _group: i32) {}
    /// C# `H.IsColor` -- does `tile` count as colour `group` for `player_id`?
    fn is_color(&self, _player_id: i32, _tile: i32, _group: i32) -> bool {
        false
    }
    /// The turn's buy discount (C# `TurnCtx.BuyDiscount`).
    fn set_buy_discount(&mut self, _n: i32) {}
    /// What this turn's [触发结算]s have cost the player so far.
    fn paid_in_settle(&self) -> i32 {
        0
    }
    /// 「本回合购买格子不[消耗]资金」 / 「如果购买则拆除那个格子上的所有房屋」.
    fn set_free_buy(&mut self, _on: bool) {}
    fn set_raze_on_buy(&mut self, _on: bool) {}
    /// C# `_turnSnap[i].pos` -- where the player stood when the turn started.
    fn turn_start_pos(&self, _player_id: i32) -> i32 {
        -1
    }
    /// C# `_turnCtx.Rolls` -- every face rolled this turn.
    fn turn_rolls(&self) -> Vec<i32> {
        Vec::new()
    }
    /// 「下次盖房时减免N（可溢出），盖房后减少1层」.
    fn set_build_discount(&mut self, _n: i32, _layers: i32) {}
    /// 「加盖房屋时半价」 -- 100 = full, 50 = half, 0 = free.
    fn set_build_cost_pct(&mut self, _pct: i32) {}
    /// C# `_turnSnap[i]` -- the four status values a turn-end undo restores.
    fn turn_snap(&self, _player_id: i32) -> (i32, i32, i32, i32) {
        (-1, 0, 0, 0)
    }
    /// C# `H._tiles[t].kind == "agent"` -- the 地产商 tile.
    fn is_agent(&self, _tile: i32) -> i32 {
        0
    }
    fn is_live_house(&self, tile: i32) -> i32;
    /// `H.IsLiveHouse` for one player: the base check plus their `Fx.ExtraColor`
    /// (「使其对你视为live house格子」).
    fn is_live_house_for(&self, _player_id: i32, tile: i32) -> i32 {
        self.is_live_house(tile)
    }
    /// `TileData.group` -- colour group (-1 for no tile).
    fn tile_group(&self, tile: i32) -> i32;
    /// `H._tiles[t].price` -- the land price alone (cf. `buy_price`).
    fn tile_price(&self, tile: i32) -> i32;
    /// `H.State.houses[t]`.
    fn houses_of(&self, tile: i32) -> i32;
    /// `H.RentHouses` -- the house count a **rent** lookup reads (the counted
    /// value a 「房屋数视为…」 override may lift). Real houses are untouched.
    fn rent_houses_of(&self, tile: i32) -> i32 {
        self.houses_of(tile)
    }
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
    /// `WhyNotBuildOn` -- may this player build here? `true` = yes.
    fn can_build_on(&self, _player_id: i32, _tile: i32) -> bool {
        true
    }
    /// Is this placed card face-down? (`!p.FaceDown` in the C# field filters.)
    fn card_face_down(&self, _player_id: i32, _card: &str) -> bool {
        false
    }
    /// Place a field card; returns the new instance's uid, which is what
    /// addresses it afterwards. The *name* is not an identity -- one player may
    /// hold several copies of the same card in play.
    fn place_card(&mut self, player_id: i32, card: &str, note: Msg) -> i32;
    /// Place the card **on a tile** rather than with its owner (C#
    /// `H.PlaceFromPlay(c, i, tile)`); `tile: -1` is [`Self::place_card`].
    /// Place a field card on a tile; returns the new instance's uid.
    fn place_card_on(&mut self, player_id: i32, tile: i32, card: &str, note: Msg) -> i32;
    /// `PlayCtx.Dest` -- where this card goes when its effect finishes.
    fn set_dest(&mut self, dest: i32);
    /// [`Self::set_dest`] aimed at another player's pile: `to` is whose
    /// discard/hand/deck it lands in, for 「将此卡放入[使用者]弃卡区」 when
    /// [使用者] is not the one holding it.
    fn set_transfer_to_dest(&mut self, to: i32, dest: i32);
    /// Move the running instance to `dest` **now** rather than when its effect
    /// finishes -- for a card that must be gone before the rest of the effect
    /// runs (a move or a settle follows). Lands in its own owner's pile;
    /// returns the owner it left, or `None` when it was not in play.
    fn send_to_dest(&mut self, dest: i32) -> Option<i32>;
    /// [`Self::send_to_dest`] aimed at another player's pile: `to` is whose.
    fn transfer_to_dest(&mut self, to: i32, dest: i32) -> Option<i32>;
    /// `H.Unplace` -- take the card out of play (`true` when it was there).
    /// Take the *running* instance off the field; returns the owner it left
    /// (or -1). No locator: the instance is the one the run was dispatched for.
    fn unplace_card(&mut self) -> i32;
    /// Take a *named* card off the player's field (C# `H.Unplace(card, ...)`).
    fn unplace_card_named(&mut self, _player_id: i32, _card: &str) -> bool {
        false
    }
    fn is_placed(&self) -> i32;
    /// Where the *running* instance sits (-1 = with its owner / gone).
    fn self_tile(&self) -> i32 {
        -1
    }
    fn set_self_tile(&mut self, _tile: i32) -> bool {
        false
    }
    fn self_face_down(&self) -> bool {
        false
    }
    fn set_self_face_down(&mut self, _on: bool) -> bool {
        false
    }
    fn self_immune(&self) -> bool {
        false
    }
    fn set_self_immune(&mut self, _on: bool) -> bool {
        false
    }
    /// Ids of the player's placed field cards, in placement order.
    fn placed_cards(&self, _player_id: i32) -> Vec<String> {
        Vec::new()
    }
    /// Does this card's rule text mention `needle`? 「所有效果包含[奇迹水晶]的卡」.
    fn card_text_mentions(&self, _card: &str, _needle: &str) -> bool {
        false
    }
    /// C# `Card.Crystals` on a named placed card.
    fn card_crystals(&self, _player_id: i32, _card: &str) -> i32 {
        0
    }
    /// C# `Card.AddCrystals` on a named placed card; `max` caps (0 = uncapped).
    fn add_card_crystals(&mut self, _player_id: i32, _card: &str, _n: i32, _max: i32) -> i32 {
        0
    }
    /// Every card instance on `player_id`'s field, as `(uid, id)` in placement
    /// order. The hook dispatch walks this so two copies of a card are two hooks.
    fn field_instances(&self, _player_id: i32) -> Vec<(i32, String)> {
        Vec::new()
    }
    /// The instance at `uid`, wherever it sits.
    fn crystals_at(&self, _uid: i32) -> i32 {
        0
    }
    fn add_crystals_at(&mut self, _uid: i32, _n: i32, _max: i32) -> i32 {
        0
    }
    /// One declared property of the instance at `uid` (`FieldCard::props`,
    /// `card_sdk::abi::prop` keys). Default `0`.
    fn prop_at(&self, _uid: i32, _key: &str) -> i32 {
        0
    }
    fn set_prop_at(&mut self, _uid: i32, _key: &str, _value: i32) -> i32 {
        0
    }
    /// A declared property of the rule instance governing `tile`
    /// (`FieldCard::props`, `card_sdk::abi::prop` keys). This is what a card
    /// that bends a tile writes instead of an engine flag (`docs/TILES.md`):
    /// 「无法获取[CiRCLE奖励]」 arms `prop::NO_REWARD` on the tile's `tile:circle`
    /// instance, （soyo） writes `prop::GROUP`, and so on. Reads the first
    /// instance on the tile that carries the key; a tile with no rule instance
    /// reads as the key's default (`0`).
    fn tile_prop(&self, _tile: i32, _key: &str) -> i32 {
        0
    }
    /// Write a [`Self::tile_prop`] on every rule instance governing `tile`.
    /// Returns the stored value. The **source** owns the arming and the
    /// disarming; the reader is the tile instance.
    fn set_tile_prop(&mut self, _tile: i32, _key: &str, _value: i32) -> i32 {
        0
    }
    /// The [经过] CiRCLE reward (`H.CircleReward`) -- `docs/TILES.md`'s
    /// `ctx::settle_circle_reward`, the body of `tile:circle`'s Pass entry.
    /// The engine runs the whole reward step (suppression, the choice, the
    /// `circleAffected` window, the payout) and answers `1`.
    fn settle_circle_reward(&mut self, _player_id: i32, _landing: bool) -> i32 {
        1
    }
    fn unplace_at(&mut self, _uid: i32) -> i32 {
        -1
    }
    fn tile_at(&self, _uid: i32) -> i32 {
        -2
    }
    fn set_tile_at(&mut self, _uid: i32, _tile: i32) -> bool {
        false
    }
    fn is_face_down_at(&self, _uid: i32) -> bool {
        false
    }
    fn set_face_down_at(&mut self, _uid: i32, _on: bool) -> bool {
        false
    }
    fn is_immune_at(&self, _uid: i32) -> bool {
        false
    }
    fn set_immune_at(&mut self, _uid: i32, _on: bool) -> bool {
        false
    }
    /// Miracle crystals on the *running card instance* (C# `Card.Crystals`).
    /// The instance is the one the run was dispatched for -- its placement is
    /// not a parameter, because the host already knows it from the dispatch and
    /// the same card id can sit on several players' fields at once.
    fn crystals(&self) -> i32;
    /// Set the running card instance's crystals; returns the new count.
    fn set_crystals(&mut self, n: i32) -> i32;
    /// `H.AddCrystals` -- adjust the running card instance's crystals by `n`,
    /// clamped at 0 and at `max` (0 = uncapped); returns the new count.
    fn add_crystals(&mut self, n: i32, max: i32) -> i32;

    /// One declared **property** of the running rule instance
    /// (`FieldCard::props`, `card_sdk::abi::prop` keys). Default `0`.
    fn self_prop(&self, key: &str) -> i32 {
        let _ = key;
        0
    }
    fn set_self_prop(&mut self, key: &str, value: i32) -> i32 {
        let _ = (key, value);
        0
    }

    // ------------------------------------------------------ marks & tokens
    /// `H.AddMark` -- a marker on a tile (`kind` names it, `note` explains it).
    fn add_mark(&mut self, tile: i32, player_id: i32, kind: &str, note: Msg);
    fn count_marks(&self, tile: i32, kind: &str, owner: i32) -> i32;
    /// C# `Card.Immune` -- 「此卡不受…效果影响」.
    fn set_card_immune(&mut self, _player_id: i32, _card: &str, _on: bool) -> bool {
        false
    }
    fn card_immune(&self, _player_id: i32, _card: &str) -> bool {
        false
    }
    /// Move a placed field card to `tile` (`tile: -1` = back with its owner).
    fn set_card_tile(&mut self, _player_id: i32, _card: &str, _tile: i32) -> bool {
        false
    }
    /// Move one matching mark's `count` by `delta`, dropping it at 0.
    fn bump_mark(&mut self, _tile: i32, _kind: &str, _owner: i32, _delta: i32) -> i32 {
        0
    }
    fn remove_marks(&mut self, tile: i32, kind: &str, owner: i32) -> i32;
    /// Names of the counters whose name starts with `prefix` and is non-zero.
    fn tok_names(&self, _player_id: i32, _prefix: &str) -> Vec<String> {
        Vec::new()
    }
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
    fn state_set_expires(
        &mut self,
        player_id: i32,
        key: &str,
        expires: Option<game_core::state::Tick>,
    );
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
    /// The uid of `player_id`'s **band-skill field instance** (`skill:<band>:
    /// <skill>`, `FieldCard::band_skill`), or -1 when the player has none. This
    /// is the 「乐队卡 / 囡卡」 crystal holder -- see [`Self::band_crystals`].
    fn band_skill_uid(&self, _player_id: i32) -> i32 {
        -1
    }
    /// 「乐队卡 / 囡卡」 crystals -- the count on the player's band-skill field
    /// instance ([`Self::band_skill_uid`]). 0 when there is no band skill; a
    /// write with no band skill is a no-op. The same pool every band skill and
    /// every card that mentions the band card's crystals reads and writes, so
    /// `add_crystals` inside a band skill's own handler and `add_band_crystals`
    /// from a card land on one count.
    fn band_crystals(&self, player_id: i32) -> i32 {
        let uid = self.band_skill_uid(player_id);
        if uid < 0 {
            0
        } else {
            self.crystals_at(uid)
        }
    }
    /// Add to that instance's crystals. `max` > 0 clamps, `max` = 0 is uncapped
    /// (exactly as [`Self::add_crystals_at`]); returns the new count, or 0 when
    /// the player has no band skill.
    fn add_band_crystals(&mut self, player_id: i32, n: i32, max: i32) -> i32 {
        let uid = self.band_skill_uid(player_id);
        if uid < 0 {
            0
        } else {
            self.add_crystals_at(uid, n, max)
        }
    }
    // -------------------------------------------- skill / band attachments
    // C# `H._fx[i].bands` / `.skill` and `H.MakeBand`. The *bound* band skill
    // (the first in placement order, [`Self::band_skill_uid`]) and the character
    // skill are named; `band_skills` lists every band attachment, including the
    // 「拿取」ed `extra` copies.
    /// Rule id of the player's bound **band skill** (`skill:<band>:<skill>`).
    fn band_skill_id(&self, player_id: i32) -> Option<String> {
        let uid = self.band_skill_uid(player_id);
        if uid < 0 {
            None
        } else {
            self.field_instances(player_id)
                .into_iter()
                .find(|(u, _)| *u == uid)
                .map(|(_, id)| id)
        }
    }
    /// Rule id of the player's **character skill** (`skill:<character>:<skill>`).
    fn character_skill_id(&self, _player_id: i32) -> Option<String> {
        None
    }
    /// Every band-skill attachment on the player's field, as
    /// `(uid, rule id, extra)` in placement order. `extra` is a 「拿取」ed copy.
    fn band_skills(&self, _player_id: i32) -> Vec<(i32, String, i32)> {
        Vec::new()
    }
    /// Attach a band-skill instance (C# `H.MakeBand(band, user, extra)`).
    /// Returns the new uid, or -1 when refused (not a band skill, player gone,
    /// or 「相同乐队技能卡的效果不可叠加」 -- an attachment of the same id is
    /// already there).
    fn add_band_skill(&mut self, _player_id: i32, _id: &str, _extra: bool) -> i32 {
        -1
    }
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
    /// Yu-Gi-Oh's *negate the effect*: the link happened, but settles to nothing.
    fn set_trigger_negate_effect(&mut self);
    /// Take one recipient out of settlement; the effect still settles for the rest.
    fn set_trigger_spare(&mut self, seat: i32);
    /// Append a declared effect to the current chain link, naming its recipient
    /// now -- at declaration, not at settlement. `kind` is a `TriggerKind` wire
    /// value.
    fn declare_trigger_effect(&mut self, kind: i32, target: i32, from: i32, tile: i32, value: i32);
    /// `t.Card == id` -- is this trigger about that card?
    fn trig_card_is(&self, id: &str) -> i32;

    // ----------------------------------------------- turn plan & scheduling
    // Defaults are no-ops so test worlds need not implement them.

    /// Call the running card's `counteract` (kind `turnEnd`) for `player_id` at a turn
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
    /// Make `id` the running card for a nested `play_card`: a **fresh** instance
    /// (C# `NewCard`), with a fresh `Dest`. The uid goes to -1 rather than being
    /// inherited -- otherwise the inner run would read the *outer* instance's
    /// crystals under its own name, which is not what "the inner card runs as
    /// itself" means. Returns what to hand back to [`Self::leave_card`]:
    /// `(card, dest, dest_to, uid)`.
    fn enter_card(&mut self, _id: &str) -> (String, i32, Option<i32>, i32) {
        (String::new(), 0, None, -1)
    }
    /// Restore the outer card after a nested `play_card`; returns the inner
    /// card's `Dest`. (The inner card's transfer target is not carried out --
    /// only the fate -- so a nested card that names another player's pile
    /// should use [`Self::transfer_to_dest`] to land there itself.)
    fn leave_card(&mut self, _saved: (String, i32, Option<i32>, i32)) -> i32 {
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
    /// 「当前回合内你每获得过一次资金」 -- money-ins for `player_id` this turn.
    fn gains_this_turn(&self, _player: i32) -> i32 {
        0
    }
    /// The **static targeting query**: which players the play being resolved
    /// (by `player_id`) designates (C# `H.Db.Card(id).Targeting` + `H.Others`).
    /// Empty when the play names nobody.
    fn designations(&self, _player_id: i32) -> Vec<i32> {
        Vec::new()
    }
    /// Per-pair cancel (C# `play.Tags["immune"+seat]`): mark `seat`'s
    /// designation on the play being resolved as cancelled. The rest land.
    fn cancel_designation(&mut self, _seat: i32) {}
    /// Is `seat`'s designation on the play being resolved cancelled?
    fn designation_cancelled(&self, _seat: i32) -> bool {
        false
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

    /// Arm the doubling for the run in progress (`PlayCtx.Doubled = n`). A
    /// third party -- CiRCLE's band skill -- sets this on the *playing* card's
    /// context, which is why it is a setter on the shared world rather than a
    /// field the card owns. Default no-op so test worlds need not implement it.
    fn set_play_doubled(&mut self, n: i32) {
        let _ = n;
    }

    // ----------------------------------------- movement shaping (TurnCtx.plan)
    // Defaults are no-ops so test worlds need not implement them.
    //
    // What lives here is the move's own plan/geometry plus what a card shapes.
    // Deliberately *not* here, and why:
    //   - `by` / `forced` -- who imposed the move: a property of the trigger,
    //     which the counteraction already sees (`Trigger.by_card`).
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
    //   - `no_build` -- folded into `can_build = false`; one flag is enough.
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
    /// Did the walk stop before its full length? The walk loop sets it.
    fn move_stopped(&self) -> bool {
        false
    }
    /// The tile the walk begins on instead of the player's own; -1 for the player.
    fn set_start(&mut self, _v: i32, _why: &str) {}
    /// Extra steps added mid-walk (the walk grows as it runs).
    fn set_extra_steps(&mut self, _v: i32) {}
    /// Floor applied to the final face (unsigned rolls only).
    fn set_min_roll(&mut self, _v: i32) {}
    /// The landing cannot be bought (「该次传送不可进行地契购买」).
    fn set_no_buy(&mut self, _v: bool) {}
    /// The landing cannot be built on.
    /// Passing CiRCLE pays nothing on this walk.
    /// Replace the starting dice (default 1d20). `sides == 0` is a flat `count`.
    fn set_base_dice(&mut self, _count: i32, _sides: i32, _why: &str) {}
    /// Drop every extra die another effect added to the plan.
    fn clear_dice(&mut self) {}
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
    /// C# `CanBuild` -- may build away from the landing.
    fn set_can_build(&mut self, _v: bool) {}
    /// C# `SettleAsAgent`.
    fn set_settle_as_agent(&mut self, _v: bool) {}
    /// 「使你的下次主要移动结果对那些玩家一起执行」 -- record a follower of the
    /// move being planned (C# `LeadFx.Who` / `Follow`). After the mover settles
    /// the engine replays this move's result for each follower, in the order
    /// they were added.
    fn plan_add_follower(&mut self, _player_id: i32) {}
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
