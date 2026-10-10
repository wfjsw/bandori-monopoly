//! The world a card effect can see and change.
//!
//! `game-core`'s match state implements this in the `WasmRules` bridge. The
//! `Clone` bound is what makes replay work: every run starts from a clone of the
//! snapshot, and only a run that finishes replaces the real state. The RNG
//! **must** live inside the world so that a replayed run draws the same numbers.
//!
//! The methods are the card-facing vocabulary of the match: dice, board,
//! players, piles, field cards. Anything that needs a player decision is
//! deliberately absent: the host surfaces it as a [`crate::Prompt`] and the run
//! is replayed with the answer.

use game_core::msg::Msg;

/// `skill:<owner>:<skill>` -> `owner` (the character or band name).
/// `game_core::data::skill_id`'s format; neither name contains `:` in the
/// shipped data, so the first `:` split is the owner.
pub(crate) fn skill_id_owner(id: &str) -> Option<String> {
    let rest = id.strip_prefix("skill:")?;
    let (owner, _) = rest.split_once(':')?;
    if owner.is_empty() {
        None
    } else {
        Some(owner.to_string())
    }
}

/// The trigger a counteraction is checked against.
///
/// The card-world's view of [`game_core::engine::rules::Trigger`], with the
/// same grouping: the move payload is one optional [`TriggerMove`], the pay /
/// buy payloads one optional struct each. The guest ABI and CEL still see the
/// old flat encoding with its sentinels whenever the group is `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub kind: crate::TriggerKind,
    pub player_id: i32,
    /// The target seat (`t.Pay.to` on pay triggers).
    pub target: i32,
    /// The tile this trigger is about (settled on, passed, mortgaged, ...).
    pub tile: i32,
    /// The amount (`t.Pay.amount` on pay triggers).
    pub value: i32,
    /// The turn stage (0 = no turn; 1 开始 / 2 运营 / 3 移动 / 4 结束) active
    /// when this trigger fired.
    pub step: i32,
    /// The player whose card caused this trigger, or `None`.
    pub by_card: Option<i32>,
    /// Was this fired as part of the turn's [主要移动]? Meaningful even
    /// without a move payload (a tile settle body needs it).
    pub main: bool,
    /// The move that caused this trigger, when one did.
    pub mv: Option<TriggerMove>,
    /// The payment a `pay` / `paid` trigger is about.
    pub pay: Option<TriggerPay>,
    /// The purchase a buy trigger is about (`docs/PURCHASE.md`).
    pub buy: Option<TriggerBuy>,
    /// Where a `roll` / `moveRoll` face came from.
    pub roll_source: game_core::engine::rules::RollSource,
    /// How a counter invalidated this link -- see [`game_core::engine::rules::Negation`].
    pub negation: game_core::engine::rules::Negation,
    /// Recipients a counter spared from settlement.
    pub spared: Vec<i32>,
    /// Position within the current chain, 1-based. 0 = not on a chain.
    pub seq: u32,
    /// The link this one answers. 0 = this is the effect declaration itself.
    pub answers: u32,
    /// The effects this link declares, recipients already named.
    pub effects: Vec<game_core::engine::rules::Effect>,
    /// The cards a `drew` trigger is about.
    pub cards: Vec<String>,
    /// The card id on card/event/counteracted triggers.
    pub card: Option<String>,
    /// The counter name on a `CounterChanged` raise, or the message name on
    /// an `On::Message` dispatch.
    pub name: Option<String>,
    /// A `BuyGate` refusal's reason key.
    pub reason: Option<String>,
}

/// The move that caused a trigger.
///
/// Mirror of [`game_core::engine::rules::TriggerMove`] using the card-sdk
/// [`card_sdk::abi::MoveKind`], so guest-facing code does not reach into
/// `game-core`'s copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggerMove {
    pub kind: card_sdk::abi::MoveKind,
    /// Does the move settle where it lands?
    pub resolve: bool,
    /// Per-card counters on the move (a [火罐] roll is card-owned state).
    pub tags: Vec<(String, i32)>,
    /// Was this the turn's main move?
    pub main: bool,
    /// The direction the move travels.
    pub dir: game_core::engine::rules::Dir,
    /// The move's 移动起点.
    pub from: i32,
    /// Steps the move has left to walk.
    pub remaining: i32,
    /// The move's path length.
    pub total: i32,
    /// `t.Move.Roll`; `None` when there is no face to show.
    pub roll: Option<i32>,
}

/// The payment a `pay` / `paid` trigger is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TriggerPay {
    /// Is this payment rent, as against a buy / build / forced loss?
    pub is_rent: bool,
}

/// The purchase a buy trigger is about (`docs/PURCHASE.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerBuy {
    pub kind: card_sdk::abi::BuyKind,
    /// The payee (the bank for land / agent / card / auction buys).
    pub seller: game_core::engine::rules::Payee,
    /// The price the buyer would be charged.
    pub price: i32,
    /// Ownership after the deal. `None` = the deal keeps its default (the buyer).
    pub deal_owner: Option<i32>,
    /// Houses after the deal.
    pub deal_houses: i32,
    /// Mortgage after the deal.
    pub deal_mortgaged: bool,
}

impl Default for Trigger {
    fn default() -> Self {
        Self {
            kind: crate::TriggerKind::None,
            player_id: 0,
            target: 0,
            tile: 0,
            value: 0,
            step: 0,
            by_card: None,
            main: false,
            mv: None,
            pay: None,
            buy: None,
            roll_source: Default::default(),
            negation: Default::default(),
            spared: Vec::new(),
            seq: 0,
            answers: 0,
            effects: Vec::new(),
            cards: Vec::new(),
            card: None,
            name: None,
            reason: None,
        }
    }
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
    /// Announce which effect a card just applied. Unlike [`Self::log`] this
    /// reaches the player as a popup as well as a log line -- the two are
    /// additive. Defaults to a plain log so test worlds need not implement it.
    fn effect(&mut self, player_id: i32, msg: Msg) {
        self.log(player_id, msg);
    }

    fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32;
    /// 1 = settle number ranges at their max, -1 = min. 「以理论最大值或
    /// 最小值结算」.
    fn extreme(&self) -> i32 {
        0
    }
    fn set_extreme(&mut self, _v: i32) {}
    /// Did the play being resolved come from the hand?
    fn play_from_hand(&self) -> bool {
        true
    }
    fn set_play_from_hand(&mut self, _v: bool) {}
    /// A gain whose amount skills and crits may not bend.
    fn gain_fixed(&mut self, _player_id: i32, _amount: i32, _why: crate::Msg) -> i32 {
        0
    }
    /// Flip a placed card face-down / face-up.
    fn set_card_face_down(&mut self, _player_id: i32, _card: &str, _down: bool) -> bool {
        false
    }
    /// Sum the planned move's dice tables into one face.
    fn do_move_roll(&mut self, _player_id: i32) -> i32 {
        0
    }
    /// Log line (a localizable message from the card).
    fn log(&mut self, player_id: i32, msg: Msg);

    // ------------------------------------------------------------- board
    fn tile_count(&self) -> i32;
    /// Tile index of a name (a data key), or -1.
    fn tile_named(&self, name: &str) -> i32;
    /// Board name of `tile` (the `tile_named` spelling). Empty when unknown --
    /// the default for worlds without a data table.
    fn tile_name(&self, tile: i32) -> String {
        let _ = tile;
        String::new()
    }
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
    /// A deed tile (`TileData::is_buyable`).
    fn is_buyable(&self, tile: i32) -> i32;
    /// A 「商店街」 deed (buyable, colour group 10).
    fn is_shop(&self, tile: i32) -> i32;
    /// A RiNG deed (`TileKind::Ring`).
    fn is_ring(&self, tile: i32) -> i32;
    /// The CiRCLE tile (`TileKind::Circle`).
    fn is_circle(&self, tile: i32) -> i32;
    /// Does `tile` count as colour `group` for `player_id`?
    /// Reads the tile props (`prop::ANY_COLOR` / `prop::colorFor:<p>`).
    fn is_color(&self, _player_id: i32, _tile: i32, _group: i32) -> bool {
        false
    }
    /// What this turn's [触发结算]s have cost the player so far.
    fn paid_in_settle(&self) -> i32 {
        0
    }
    /// Where the player stood when the turn started.
    fn turn_start_pos(&self, _player_id: i32) -> i32 {
        -1
    }
    /// Every face rolled this turn.
    fn turn_rolls(&self) -> Vec<i32> {
        Vec::new()
    }
    /// 「下次盖房时减免N（可溢出），盖房后减少1层」.
    fn set_build_discount(&mut self, _n: i32, _layers: i32) {}
    /// 「加盖房屋时半价」 -- 100 = full, 50 = half, 0 = free.
    fn set_build_cost_pct(&mut self, _pct: i32) {}
    /// The four status values a turn-end undo restores.
    fn turn_snap(&self, _player_id: i32) -> (i32, i32, i32, i32) {
        (-1, 0, 0, 0)
    }
    /// The 「地产商」 tile (`TileKind::Agent`).
    fn is_agent(&self, _tile: i32) -> i32 {
        0
    }
    /// A Live House deed (buyable, colour group 6).
    fn is_live_house(&self, tile: i32) -> i32;
    /// [`Self::is_live_house`] for one player: the base check plus their
    /// colour override (「使其对你视为live house格子」).
    fn is_live_house_for(&self, _player_id: i32, tile: i32) -> i32 {
        self.is_live_house(tile)
    }
    /// Colour group (`TileData::group`; -1 for no tile).
    fn tile_group(&self, tile: i32) -> i32;
    /// The land price alone (cf. `buy_price`, which adds houses).
    fn tile_price(&self, tile: i32) -> i32;
    /// House count on the tile.
    fn houses_of(&self, tile: i32) -> i32;
    /// The house count a **rent** lookup reads (the counted value a
    /// 「房屋数视为…」 override may lift). Real houses are untouched.
    fn rent_houses_of(&self, tile: i32) -> i32 {
        self.houses_of(tile)
    }
    /// Set the tile's house count (house transfer effects).
    fn set_houses(&mut self, tile: i32, n: i32);
    /// Add (or remove, with negative `n`) houses; returns the new count.
    fn add_house(&mut self, tile: i32, n: i32) -> i32;
    /// Is the tile mortgaged? (1 = mortgaged.)
    fn mortgaged_of(&self, tile: i32) -> i32;
    /// Mortgage/redeem outright (transfer effects; no money moves).
    fn set_mortgaged(&mut self, tile: i32, v: i32);
    /// Hand a deed over outright (no money moves).
    fn set_owner(&mut self, tile: i32, player_id: i32);
    /// Undirected ring distance between two tiles.
    fn dist(&self, a: i32, b: i32) -> i32;
    /// Steps forward from `a` to `b` around the ring.
    fn tile_forward(&self, a: i32, b: i32) -> i32;
    /// Next present player in direction `dir` (±1), or -1.
    fn neighbor(&self, player_id: i32, dir: i32) -> i32;
    /// Present players on a tile, minus `except` (-1 = none).
    fn players_on_count(&self, tile: i32, except: i32) -> i32;
    fn players_on_at(&self, tile: i32, except: i32, index: i32) -> i32;

    // -------------------------------------------------------------- players
    fn player_count(&self) -> i32;
    fn player_out(&self, player_id: i32) -> i32;
    fn others_count(&self, player_id: i32) -> i32;
    fn others_at(&self, player_id: i32, index: i32) -> i32;
    fn money(&self, player_id: i32) -> i32;
    /// Money in, logged with its reason. Returns what was gained.
    fn gain(&mut self, player_id: i32, amount: i32, src: Msg) -> i32;
    /// Money out (what the player could pay), logged.
    fn pay(&mut self, player_id: i32, amount: i32, src: Msg) -> i32;

    /// Adopt the recorded pile state of an already-applied host request.
    /// The default keeps standalone host fixtures independent of engine replay.
    fn after_host(&mut self, _answer: usize) {}

    // ------------------------------------------------------------ hand/deck
    /// Draw `n` cards (reshuffles when needed). Returns drawn count.
    fn draw(&mut self, player_id: i32, n: i32) -> i32;
    fn add_to_hand(&mut self, player_id: i32, card: &str);
    fn add_to_deck(&mut self, player_id: i32, card: &str, shuffle: bool);
    /// Add a card to the draw pile at a position: `pos` 0 = top, 1 = bottom,
    /// 2 = shuffle in (the default).
    fn add_to_deck_at(&mut self, player_id: i32, card: &str, pos: i32);
    /// Take one copy of `card` out of `pile` *without* sending it anywhere;
    /// returns whether it was there.
    fn take_card(&mut self, player_id: i32, pile: crate::CardPile, card: &str) -> bool;
    /// Every card id in a player's `pile`, in pile order (the deck top first).
    fn cards_in(&self, player_id: i32, pile: crate::CardPile) -> Vec<String>;
    fn to_discard(&mut self, player_id: i32, card: &str);
    /// Copies of `card` in hand.
    fn hand_count(&self, player_id: i32, card: &str) -> i32;
    /// Total cards in hand.
    fn hand_size(&self, player_id: i32) -> i32;
    /// Copies of `card` in the discard pile.
    fn discard_count(&self, player_id: i32, card: &str) -> i32;
    /// Draw-pile size.
    fn deck_count(&self, player_id: i32) -> i32;
    /// Discard-pile size (all ids).
    fn discard_size(&self, player_id: i32) -> i32;
    /// One copy hand -> discard; 1 when it was held.
    fn discard_from_hand(&mut self, player_id: i32, card: &str) -> i32;
    /// Shuffle the hand and/or the discard pile into the draw pile.
    fn shuffle_into_deck(&mut self, player_id: i32, hand: bool, discard: bool) -> i32;

    // -------------------------------------------------------- field cards
    /// `WhyNotBuildOn` -- may this player build here? `true` = yes.
    fn can_build_on(&self, _player_id: i32, _tile: i32) -> bool {
        true
    }
    /// Is this placed card face-down? (Field filters ignore face-down cards.)
    fn card_face_down(&self, _player_id: i32, _card: &str) -> bool {
        false
    }
    /// Place a field card; returns the new instance's uid, which is what
    /// addresses it afterwards. The *name* is not an identity -- one player may
    /// hold several copies of the same card in play.
    fn place_card(&mut self, player_id: i32, card: &str, note: Msg) -> i32;
    /// Place the card **on a tile** rather than with its owner;
    /// `tile: -1` is [`Self::place_card`]. Returns the new instance's uid.
    fn place_card_on(&mut self, player_id: i32, tile: i32, card: &str, note: Msg) -> i32;
    /// Where this card goes when its effect finishes.
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
    /// Take the *running* instance off the field; returns the owner it left
    /// (or -1). No locator: the instance is the one the run was dispatched for.
    fn unplace_card(&mut self) -> i32;
    /// Take a *named* card off the player's field.
    fn unplace_card_named(&mut self, _player_id: i32, _card: &str) -> bool {
        false
    }
    // -------------------------------------------------------- active events
    // (`docs/EVENTS.md`) The engine owns the event deck and the active list;
    // these are the rule body's handles on it. Defaults are no-ops so test
    // worlds need not implement them.
    /// Expire the active event `id` (`ctx::event_expire`): unbind its rule
    /// instance and file it away. `removed` = 「永久移除」 rather than
    /// 「放入事件弃牌」.
    fn event_expire(&mut self, _id: &str, _removed: bool) {}
    /// Is `id` active and face-up? (`ctx::event_is_active`.)
    fn event_is_active(&self, _id: &str) -> bool {
        false
    }
    /// Put `id` on the top of the event deck (`ctx::event_deck_push`); the end
    /// of the deck list is the top. `face_down` = 「背面朝上放置于事件牌堆顶部」.
    fn event_deck_push(&mut self, _id: &str, _face_down: bool) {}
    /// Take `id` out of the event deck / discard / active list for good
    /// (`ctx::event_banish`) -- 「从所有非衍生事件中选择3个移除」.
    fn event_banish(&mut self, _id: &str) {}
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
    /// [奇迹水晶] on a named placed card.
    fn card_crystals(&self, _player_id: i32, _card: &str) -> i32 {
        0
    }
    /// Add [奇迹水晶] on a named placed card; `max` caps (0 = uncapped).
    fn add_card_crystals(&mut self, _player_id: i32, _card: &str, _n: i32, _max: i32) -> i32 {
        0
    }
    /// Every card instance on `player_id`'s field, as `(uid, id)` in placement
    /// order. The hook dispatch walks this so two copies of a card are two hooks.
    fn field_instances(&self, _player_id: i32) -> Vec<(i32, String)> {
        Vec::new()
    }
    /// Standing board-owned pseudo cards (`mark:cp` and kin), as `(uid, id)`.
    fn mark_rule_instances(&self) -> Vec<(i32, String)> {
        Vec::new()
    }
    /// The instance at `uid`, wherever it sits.
    fn crystals_at(&self, uid: i32) -> i32 {
        self.counter_at(uid, game_core::state::counter::CRYSTALS)
    }
    fn add_crystals_at(&mut self, uid: i32, n: i32, max: i32) -> i32 {
        self.add_counter_at(uid, game_core::state::counter::CRYSTALS, n, max)
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
    /// Miracle crystals on the *running card instance*. Sugar over
    /// [`Self::counter`] with `game_core::state::counter::CRYSTALS`.
    fn crystals(&self) -> i32 {
        self.counter(game_core::state::counter::CRYSTALS)
    }
    fn set_crystals(&mut self, n: i32) -> i32 {
        self.set_counter(game_core::state::counter::CRYSTALS, n)
    }
    fn add_crystals(&mut self, n: i32, max: i32) -> i32 {
        self.add_counter(game_core::state::counter::CRYSTALS, n, max)
    }

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
    // Named counters and their bound units (user ruling 2026-10-10).

    /// On-card named counter of the running instance.
    fn counter(&self, name: &str) -> i32 {
        let _ = name;
        0
    }
    fn set_counter(&mut self, name: &str, n: i32) -> i32 {
        let _ = (name, n);
        0
    }
    fn add_counter(&mut self, name: &str, n: i32, max: i32) -> i32 {
        let _ = (name, n, max);
        0
    }
    /// On-card named counter of the instance at `uid`.
    fn counter_at(&self, uid: i32, name: &str) -> i32 {
        let _ = (uid, name);
        0
    }
    fn add_counter_at(&mut self, uid: i32, name: &str, n: i32, max: i32) -> i32 {
        let _ = (uid, name, n, max);
        0
    }
    /// On-card named counter of a placed card (first copy).
    fn card_counter(&self, player_id: i32, card: &str, name: &str) -> i32 {
        let _ = (player_id, card, name);
        0
    }
    fn add_card_counter(&mut self, player_id: i32, card: &str, name: &str, n: i32, max: i32) -> i32 {
        let _ = (player_id, card, name, n, max);
        0
    }
    /// Bind `count` units of the running instance's counter `kind` to `tile`.
    /// `stack`: merge onto the first match, or always push a fresh row.
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
        let _ = (tile, kind, category, owner, src, count, note, fresh);
        0
    }
    fn count_marks(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        let _ = (tile, filter);
        0
    }
    fn bump_mark(&mut self, tile: i32, filter: &game_core::state::MarkFilter<'_>, delta: i32) -> i32 {
        let _ = (tile, filter, delta);
        0
    }
    fn remove_marks(&mut self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        let _ = (tile, filter);
        0
    }
    fn mark_src_at(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        let _ = (tile, filter);
        -1
    }
    fn mark_instance_at(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        let _ = (tile, filter);
        -1
    }
    /// Units of the running instance's counter `name` held by `player_id`.
    fn count_held(&self, name: &str, player_id: i32) -> i32 {
        let _ = (name, player_id);
        0
    }
    fn add_held(&mut self, name: &str, player_id: i32, n: i32, max: i32) -> i32 {
        let _ = (name, player_id, n, max);
        0
    }
    /// Name-keyed held count (any owner).
    fn count_held_name(&self, name: &str, player_id: i32) -> i32 {
        self.count_held(name, player_id)
    }
    fn set_held_name(&mut self, name: &str, player_id: i32, v: i32) {
        let _ = (name, player_id, v);
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
        let _ = (name, from_tile, from_player, to_tile, to_player, n);
        0
    }
    /// The running instance's `FieldCard::uid`.
    fn self_uid(&self) -> i32 {
        -1
    }
    /// Pin the running instance's uid (message dispatch: the handler runs as
    /// the receiver, not as a fresh nested card).
    fn set_self_uid(&mut self, uid: i32) {
        let _ = uid;
    }
    /// Names of the player's non-zero counters whose name starts with `prefix`.
    fn tok_names(&self, _player_id: i32, _prefix: &str) -> Vec<String> {
        Vec::new()
    }
    /// Legacy name-keyed token read.
    fn tok(&self, player_id: i32, name: &str) -> i32 {
        self.count_held_name(name, player_id)
    }
    fn set_tok(&mut self, player_id: i32, name: &str, value: i32) {
        self.set_held_name(name, player_id, value);
    }
    fn add_tok(&mut self, player_id: i32, name: &str, n: i32, max: i32) -> i32 {
        self.add_held(name, player_id, n, max)
    }

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
    /// A free-form counter. Sugar over the keyed state.
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
    // Band attachments on a placed card, and the bound band skill
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
    /// The player's character **name** -- the spelling [`Self::character_is`]
    /// matches. Defaults to the owner component of the character skill id
    /// (`skill:<character>:<skill>`; neither name contains `:` in the shipped
    /// data); override with the direct lookup where one exists.
    fn character_name(&self, player_id: i32) -> Option<String> {
        self.character_skill_id(player_id)
            .and_then(|id| skill_id_owner(&id))
    }
    /// The player's band **name** -- the spelling [`Self::in_band`] matches.
    /// Defaults to the owner component of the band skill id
    /// (`skill:<band>:<skill>`); override with the direct lookup where one
    /// exists.
    fn band_name(&self, player_id: i32) -> Option<String> {
        self.band_skill_id(player_id)
            .and_then(|id| skill_id_owner(&id))
    }
    /// Every band-skill attachment on the player's field, as
    /// `(uid, rule id, extra)` in placement order. `extra` is a 「拿取」ed copy.
    fn band_skills(&self, _player_id: i32) -> Vec<(i32, String, i32)> {
        Vec::new()
    }
    /// Attach a band-skill instance to a player's field.
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
    /// May this player act at all? Not out, not stunned, not exiled.
    fn can_pay(&self, player_id: i32) -> i32;
    /// May this player still take the [主要移动]? 0 = available, 1 = not your
    /// turn, 2 = already moved, 3 = cannot move this turn.
    fn cant_move(&self, player_id: i32) -> i32;
    /// Spend `n` [火罐]; 1 when the player had enough (logged).
    fn spend_fire(&mut self, player_id: i32, n: i32, why: Msg) -> i32;
    /// `[停留]` layers. Sugar over the keyed state.
    fn stay_of(&self, player_id: i32) -> i32 {
        self.state_get(player_id, game_core::state::key::STAY)
    }
    /// `[晕眩]` layers. Sugar over the keyed state.
    fn stun_of(&self, player_id: i32) -> i32 {
        self.state_get(player_id, game_core::state::key::STUN)
    }
    /// `[除外]` layers (mirrors [`Self::stun_of`]). Sugar over the keyed state.
    fn exile_of(&self, player_id: i32) -> i32 {
        self.state_get(player_id, game_core::state::key::EXILE)
    }
    /// Whose turn (-1 when none).
    fn turn_player(&self) -> i32;
    /// Current round number.
    fn round_no(&self) -> i32;
    /// `round * 100 + turn + 1` (once-per-turn latches).
    fn turn_key(&self) -> i32;
    /// Is the player's character exactly `name`?
    fn character_is(&self, player_id: i32, name: &str) -> i32;
    /// Does this player's character belong to band `name`?
    fn in_band(&self, player_id: i32, name: &str) -> i32;

    // --------------------------------------------------- ring & movement
    /// The RiNG rent multiplier right now.
    fn ring_multiplier(&self) -> i32;
    /// Nudge the RiNG multiplier; returns the new value.
    fn add_ring_bonus(&mut self, n: i32) -> i32;
    /// Move a player without settling (`resolve: false`).
    fn teleport_to(&mut self, player_id: i32, tile: i32);

    // ---------------------------------------------------------- trigger
    fn trigger(&self) -> Trigger;
    fn set_trigger_move_roll(&mut self, roll: i32);
    /// Rewrite the pending amount on a `pay`/`paid` trigger (`PayCtx.amount`);
    /// 0 cancels the payment (`t.Pay.cancel`).
    fn set_trigger_value(&mut self, value: i32);
    /// Redirect the payee of a pending pay (`PayCtx.to`); -1 = the bank.
    fn set_trigger_target(&mut self, to: i32);
    /// Negate the trigger's effect (`trigger.Cancelled = true`).
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
    /// `t.Buy.Price` -- rewrite the quoted price (the `BuyAdd` / `BuyMul` /
    /// `BuySet` stages). Also the generic `set_price` on a buy trigger.
    fn set_trigger_price(&mut self, v: i32);
    /// `t.Buy.DealOwner` -- rewrite the deal's post-commit owner (`BuyAssign`).
    fn set_trigger_deal_owner(&mut self, v: i32);
    /// `t.Buy.DealHouses` -- rewrite the deal's post-commit house count.
    fn set_trigger_deal_houses(&mut self, v: i32);
    /// `t.Buy.DealMortgaged` -- rewrite the deal's post-commit mortgage flag.
    fn set_trigger_deal_mortgaged(&mut self, v: i32);
    /// A `BuyGate` refusal's reason key, shown instead of a bare cancel.
    fn set_trigger_reason(&mut self, reason: &str);

    // ----------------------------------------------- turn plan & scheduling
    // Defaults are no-ops so test worlds need not implement them.

    /// Call the running card's `counteract` (kind `turnEnd`) for `player_id` at a turn
    /// end: this turn's end, or -- `next_of_player` -- the end of `player_id`'s next
    /// turn (「你的下回合结束时」).
    fn schedule_turn_end(&mut self, _player: i32, _next_of_player: bool, _early: bool) {}
    /// `player_id`'s money cannot drop this turn.
    fn set_no_money_loss(&mut self, _player: i32) {}
    /// This turn's main-move roll (a fixed face, not a dice table).
    fn set_fixed_roll(&mut self, _n: i32) {}
    /// The fixed main-move roll, or -1 when none is set.
    fn fixed_roll(&self) -> i32 {
        -1
    }
    /// `player_id`'s next main move walks exactly `n` steps.
    fn set_next_steps(&mut self, _player: i32, _n: i32) {}
    /// Steps this turn's main move walked (0 = none).
    fn turn_main_steps(&self) -> i32 {
        0
    }
    /// Adjust `player_id`'s [火罐] cap; returns the new cap.
    fn add_fire_max(&mut self, _player: i32, _n: i32) -> i32 {
        0
    }
    /// Make `id` the running card for a nested `play_card`: a **fresh** instance
    /// a fresh instance, with a fresh `Dest`. The uid goes to -1 rather than being
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

    /// Whether `player_id` is mid-abnormal-move this turn.
    fn abnormal_count(&self, _player: i32) -> i32 {
        0
    }
    /// Whether `player_id` was named as a [指定] target this play.
    fn targeted_count(&self, _player: i32) -> i32 {
        0
    }
    /// 「当前回合内你每获得过一次资金」 -- money-ins for `player_id` this turn.
    fn gains_this_turn(&self, _player: i32) -> i32 {
        0
    }
    /// The **static targeting query**: which players the play being resolved
    /// (by `player_id`) designates (card `Targeting` + other players).
    /// Empty when the play names nobody.
    fn designations(&self, _player_id: i32) -> Vec<i32> {
        Vec::new()
    }
    /// Per-pair cancel (`play.Tags["immune"+seat]`): mark `seat`'s
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
    /// Which of the play's tagged `ctx::n` numbers the band skill doubled, or -1.
    fn play_doubled(&self) -> i32 {
        -1
    }

    /// Arm the doubling for the run in progress. A third party -- CiRCLE's
    /// band skill -- sets this on the *playing* card's context, which is why it
    /// is a setter on the shared world rather than a field the card owns.
    /// Default no-op so test worlds need not implement it.
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
    /// The planned length; keeps the sign of a reverse walk.
    fn set_steps(&mut self, _v: i32) {}
    /// 「上一名玩家代替进行此次投掷」 (幻觉来了) -- attribute the roll to another
    /// seat. Shown on the log line as 「by」; the move still belongs to the
    /// original player (and 「影响投掷的效果服从于原本进行投掷的玩家」).
    fn set_roller(&mut self, _v: i32) {}
    /// Walk backwards.
    fn set_reverse(&mut self, _v: bool) {}
    /// A negative roll walks backwards instead of clamping to 0.
    fn set_signed(&mut self, _v: bool) {}
    /// Force the walk to stop on this tile; -1 clears.
    fn set_stop_at(&mut self, _v: i32) {}
    /// Only odd (1) / even (0) tiles count; -1 either.
    fn set_parity(&mut self, _v: i32) {}
    /// Settle where the walk lands. Clear it for 「不触发结算」.
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
    /// Settle on this tile instead of the landing; -1 clears.
    fn set_settle_tile(&mut self, _v: i32) {}
    /// Scale money paid for this walk. Milli-units (500 = x0.5).
    fn set_pay_factor(&mut self, _v: i32) {}
    /// Scale rent paid for this walk, same units.
    fn set_rent_factor(&mut self, _v: i32) {}
    /// May build away from the landing.
    fn set_can_build(&mut self, _v: bool) {}
    /// Settle this walk as if landing on the 「地产商」.
    fn set_settle_as_agent(&mut self, _v: bool) {}
    /// 「使你的下次主要移动结果对那些玩家一起执行」 -- record a follower of the
    /// move being planned (the lead / follow pairing). After the mover settles
    /// the engine replays this move's result for each follower, in the order
    /// they were added.
    fn plan_add_follower(&mut self, _player_id: i32) {}
    /// A queued second walk, in steps.
    fn set_more_steps(&mut self, _v: i32) {}
    /// Card-owned per-move state (fire-roll counters etc.).
    fn set_tag(&mut self, _key: &str, _v: i32) {}
    fn move_tag(&self, _key: &str) -> i32 {
        0
    }
    /// Where the walk is forced to stop, or -1.
    fn move_stop_at(&self) -> i32 {
        -1
    }
    /// Parity: -1 either, 0 even, 1 odd.
    fn move_parity(&self) -> i32 {
        -1
    }
    /// Does the planned move settle where it lands?
    fn move_resolve(&self) -> bool {
        true
    }
    /// The move being planned, captured whole so a
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
    /// Steps left to walk.
    fn move_remaining(&self) -> i32 {
        0
    }
    /// Steps walked (`lastWalk` is written from this).
    fn move_total(&self) -> i32 {
        0
    }
    /// +1 forwards, -1 backwards.
    fn move_dir(&self) -> i32 {
        0
    }
}
