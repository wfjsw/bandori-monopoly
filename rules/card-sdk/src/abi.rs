//! Host/guest ABI. Shared verbatim by the guest (ruleset.wasm) and the host
//! (`game-rules`). Bump [`ABI_VERSION`] on any incompatible change; the host
//! refuses rulesets built against a different version.

#[cfg(target_arch = "wasm32")]
use alloc::{string::String, vec::Vec};

/// Increment on any change to imports, exports, or their semantics.
/// v2: `play_card` (cross-module card calls); one module per card.
/// v6: full `TriggerKind` set + `trig_target` / `trig_tile` / `trig_value`.
/// v7: board/hand/status query + op wave (is_buyable ... in_band, spend_fire,
///     sweep_to_deck, trig_card_is; `Trigger.card`).
/// v8: `bandori_cant_play` export (`CardDef.cant_play`) + `add_to_deck_at`.
/// v9: `cant_move` (H.MoveWhyNot), `hand_size`, `is_ring`/`is_circle`/`is_live_house`.
/// v10: `trig_step` (`Trigger.step`) so a counteraction can tell which turn step
///      (0/1/2/3) it fired in, for kinds that aren't step-specific (e.g.
///      `mortgage` fires during both step 1 and step 3); paired
///      `*Before`/`*After` kinds filling in the missing half of every
///      trigger point, plus new hooks for buy/build/discard/end turn/leave.
/// v11: `trig_by_card` (`Trigger.by_card`) -- the player whose card caused this
///      trigger, or -1 when it was not card-caused. This is what `H.HitByOtherCard`
///      keys on (`by_card >= 0 && by_card != player_id`).
/// v12: `trig_set_pay_amount` -- a counteraction to a `pay`/`paid` trigger may rewrite
///      the pending amount (0 = cancel the payment, C# `t.Pay.cancel` / `PayCtx.amount`).
///      Also: `set_move_roll` now actually reaches the engine (the write-back from
///      the counteraction's trigger to `Trigger.value` was missing).
/// v13: `trig_pay_is_rent` (`t.Pay.IsRent`) -- is this `pay`/`paid` trigger rent,
///      as opposed to a buy/build/forced loss. Card-driven payments are never rent.
/// v14: `t.Move` context -- `trig_move_flags` (a [`MoveFlags`] bitset), plus
///      `trig_move_main` and `trig_move_dir`. A move-caused trigger (moveRoll /
///      pass / settle*) can now say what kind of move it was, and any number of
///      orthogonal modifiers on it. Replaces the bespoke `trig_move_fire_roll`.
/// v15: `trig_set_cancelled` / `trig_cancelled` (`Trigger.Cancelled`) -- a
///      counteraction may negate the trigger's effect outright (C# `trigger.Cancelled
///      = true`): the engine then skips the effect body (land / event / play)
///      but still runs the point's Before/After hooks.
/// v16: `trig_set_pay_target` -- a counteraction may also redirect the payee of a
///      pending `pay` (C# `PayCtx.to`; -1 = the bank). The transfer amount
///      follows `set_pay_amount`, so a reduced payment credits the payee less too.
/// v17: field-card (`Fx`) hooks + per-card crystals. New `TriggerKind`s
///      (`TurnEnd`, `Drawn`, `PassTile`, `PayAfter`, `RollAfter`, `CardPlayed`,
///      `Targeted`, `PayChoose`) are **hook points**, not [反击] points: the
///      engine runs every *placed* card's `counteract` against them automatically.
///      `crystals` / `set_crystals` / `add_crystals` on the running field card.
/// v18: `take_from_hand`; `drawn` self-dispatches to the card named on `t.card`
///      (the one just drawn, still in hand).
/// v19: `cards_in(player_id, pile, buf, cap)` -- list a player's [`CardPile`] as a
///      postcard `Vec<String>` written into a guest-owned buffer (the first
///      host->guest string return). `take_card(player_id, pile, id)` replaces
///      `take_from_hand` and works on any pile.
/// v20: turn plan + scheduling -- `schedule_turn_end` (a card asks for a
///      `turnEnd` call at this turn's end or at the end of a player's next turn),
///      `set_no_money_loss`, `set_fixed_roll` / `fixed_roll`, `set_next_steps`,
///      `turn_main_steps`, `add_fire_max`, `card_replayable` (C# `H.CanReplay`).
///      `play_card` now returns the inner card's `Dest`, and the inner card runs
///      as itself (its own id and `Dest`, no longer the outer card's).
/// v21: `CardDef` standardized like triggers -- `id` + a table of `On` entry
///      points (`Play(gate, effect)`, `Counteract(kinds, guard, effect)`,
///      `Hook(kinds, effect)`, `AtEnd`). The manifest lists each entry with
///      its trigger kinds, so the host dispatches only to cards that declared
///      the kind at hand; one export `bandori_on(card, entry, op, player_id)`
///      replaces `bandori_play` / `_can_react` / `_react` / `_cant_play` (the
///      pre-v21 names, kept here as history).
/// v23: hook kinds from the C# call sites -- TurnEndBefore / TurnEndAfter,
///      PayAdd / PayMul / PayAt (the Money pipeline), Discarded, DeckBeforeGame /
///      DeckAtGameStart, Drew, Reshuffled, Bought, SettleInstead, BeforeOut,
///      Teleported. Move payload `trig_move_remaining` / `trig_move_total` and
///      `MoveFlags::TELEPORT_WALK`. `schedule_turn_end` takes a mode (bit 1 =
///      the player's next turn, bit 2 = before the wear-off, C# `AtEnd`).
///      RollAfter now fires before the moveRoll [反击] window (C# order).
///      `RollPlan` is the point `On::RollPlan` (v22) is dispatched at.
/// v24: movement shaping -- `set_steps` / `set_reverse` / `set_signed` /
///      `set_stop_at` / `set_parity` / `set_resolve` / `set_settle_tile` /
///      `set_pay_factor` / `set_rent_factor` / `set_no_buy` / `set_can_build` /
///      `set_can_build` / `set_teleport_walk` / `set_min_roll` /
///      `set_extra_steps` / `set_more_steps` / `set_fire_roll` /
///      `set_no_circle_reward` / `set_settle_as_agent` and the `move_*` getters,
///      on the move being planned (guest: `ctx::plan::*`).
/// v25: card-driven `give_stay` / `give_stun` / `give_exile` / `teleport_to`
///      go through the C# `AbnormalGate`: the `abnormalGuard` hook (new kind),
///      then the `abnormal` [反击] window when another player caused it; a blocked
///      effect does not apply. `abnormal_count(player_id)` (C# `_abnormalTurn`),
///      `placed_tile(player_id, id)`, `play_doubled()` (C# `PlayCtx.Doubled`) and
///      `trig_cards` (the drawn cards on a `drew` trigger).
/// v26: the C# targeting pipeline -- `target(player_id, tile, single)` pauses the
///      run with a `Target` host request (`H.Target` / `H.TargetTile`): exile,
///      the `immuneAll` / `untargetable` / `redirect` guard hooks (new kinds),
///      the `_targeted` counter (`targeted_count`), then the `targeted` hooks
///      and the `target` [反击] window. `immuneAll` also gates card-driven
///      abnormal effects and payments (C# `ImmuneAll`). `sweep_to_deck`
///      became `shuffle_into_deck(player_id, hand, discard)`.
/// v27: the move model. `MoveFlags` is gone: a move is exactly one `MoveKind`
///      (Walk | Teleport), and what it resolves is a separate category,
///      `Settle` (ROUTE = the tiles it passes resolve [经过]; DEST = the landing
///      resolves [结算]). C# `TeleportWalk` becomes a Teleport whose destination
///      is computed from the roll; C# `Resolve = false` clears DEST. FIRE_ROLL
///      is card-owned state now (`plan::set_tag` / `trigger::move_tag`), not an
///      engine flag. `ctx::card_move` / `ctx::agent_landing` run a move or an
///      agent landing from inside a card; the plan gained `set_kind` / `set_tag`
///      / `set_start` / `set_teleport_to` and the Base/Dice roll tables
///      (`set_base_dice` / `add_base_dice` / `add_extra_dice` -- a flat add is a
///      `0`-sided term, which is what C# `Bonus` was).
///      NOT yet in v27, though named in earlier drafts of this line: the play
///      context (`CardDef.targeting`, `trigger::play_*`, `set_immune`,
///      `add_mark_flags(.., NO_TARGET)`) and `plan::set_stopped`. Targeting is
///      the `HostRequest::Target` gate; the rest is unported.
/// v28: the [反击] vocabulary is `counteract` throughout. `On::CounterAct` /
///      `OnKind::CounterAct` / `Call::CounterAct` became `Counteract`, the
///      trigger-kind wire string `reacted` became `counteracted`, and the
///      guest entry points are `counteract` / `can_counteract` (formerly
///      `react` / `can_react`). Only the `counteracted` string is on the wire;
///      the rest is naming.
pub const ABI_VERSION: i32 = 28;

/// Wasm import module name for every host function.
pub const IMPORT_MODULE: &str = "bandori";

/// Guest exports.
pub mod export {
    pub const ABI_VERSION: &str = "bandori_abi_version";
    /// `() -> i64` packed `(ptr << 32) | len` pointing at a UTF-8 JSON manifest.
    pub const MANIFEST: &str = "bandori_manifest";
    /// `(card: i32, entry: i32, op: i32, player_id: i32) -> i64` -- call entry
    /// `entry` (an index into the card's manifest `on` list). `op` is
    /// [`OP_RUN`] or, for a `Counteract` entry, [`OP_GUARD`]. Returns 0, the guard's
    /// 0/1, or (for a `Play` gate) a packed `(ptr << 32) | len` postcard `Msg`
    /// reason with 0 meaning "playable".
    pub const ON: &str = "bandori_on";
    pub const OP_RUN: i32 = 0;
    pub const OP_GUARD: i32 = 1;
    pub const MEMORY: &str = "memory";
}

/// The keys the engine's own game flow reads back (its turn rules and the
/// status gates). Content is free to invent more -- a state key is just a
/// string. Mirrors `game_core::state::key`.
pub mod state_key {
    /// `[停留]` -- layers that wear off at end of turn.
    pub const STAY: &str = "stay";
    /// `[晕眩]` -- layers that wear off at end of turn.
    pub const STUN: &str = "stun";
    /// `[晕眩]` applied this turn, which only starts counting next turn.
    pub const STUN_START: &str = "stunStart";
    /// `[移除]` -- turns spent off the board.
    pub const EXILE: &str = "exile";
    /// Tile the exile returns to, or -1 for none.
    pub const EXILE_TO: &str = "exileTo";
    /// Fire pots held. Its `max` is the mandated cap, written by the character
    /// skill -- and that is the one number to show.
    pub const FIRE: &str = "fire";
    /// Band crystals for band skills.
    pub const BAND_CRYSTALS: &str = "bandCrystals";
    /// Layers of "may hold no hand cards".
    pub const NO_HAND: &str = "noHand";
    /// Layers of "cannot be stopped".
    pub const UNSTOPPABLE: &str = "unstoppable";
    /// Hand size limit.
    pub const HAND_LIMIT: &str = "handLimit";
    /// Skill-system scratch.
    pub const SKILL_STATE: &str = "skillState";
    /// 「[拥有者]不可盖房」 -- `why_not_build_on` refuses while this is set.
    pub const NO_BUILD: &str = "noBuild";
}

/// Well-known tile-mark kinds. A mark's `kind` is its identity; the engine
/// responds to these two by name so a card can arm a gate without the engine
/// hardcoding the card's own name (the C# checked `CountMarks(t, "高贵的微蓝")`).
pub mod mark {
    /// Carrying tiles cannot be named as a target (`H.TargetTile` answers -1).
    pub const NO_TARGET: &str = "noTarget";
}

/// `i32_exit` status the host uses to abort a run that reached an unanswered prompt.
/// Never observed by the guest.
pub const EXIT_NEED_INPUT: i32 = 0x0B_A0_D0;

/// Prompt kinds, mirroring `MatchPrompt.kind` in the C# (`MatchPrompt.cs`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    /// One of the pushed options (`AskPick`).
    Choice = 0,
    /// A yes / no question (`AskYes`); the options are implicit.
    YesNo = 1,
    /// Pick a tile index (`AskTileOf`).
    Tile = 2,
    /// Pick a player. C# `H.AskSeat` has no kind of its own -- it is
    /// `AskPick` over player-name options -- so this one is ours.
    Player = 3,
    /// Pick a card id (`AskCard`).
    Card = 4,
}

impl PromptKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Choice,
            1 => Self::YesNo,
            2 => Self::Tile,
            3 => Self::Player,
            4 => Self::Card,
            _ => return None,
        })
    }

    /// The C# `MatchPrompt.kind` string (the C# names: `"pick"`, `"yes"`, ...).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Choice => "pick",
            Self::YesNo => "yes",
            Self::Tile => "tile",
            Self::Player => "player",
            Self::Card => "card",
        }
    }
}

/// How the player gets there -- a move is exactly one of these (C# `MoveCtx.Teleport`
/// vs the walk loop). Mirrors `game_core::engine::move_ctx::MoveKind`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MoveKind {
    /// The player steps along the board, tile by tile.
    #[default]
    Walk = 0,
    /// The player jumps straight to the destination (C# `TeleportMove`). A C#
    /// `TeleportWalk` (「视为 [传送]（只触发终点）」) is this kind with the
    /// destination derived from the roll instead of named.
    Teleport = 1,
}

impl MoveKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Walk),
            1 => Some(Self::Teleport),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::Teleport => "teleport",
        }
    }
}

bitflags::bitflags! {
    /// What a move resolves as it goes -- the settle axis, its own category
    /// beside [`MoveKind`] (C# `m.Resolve` / `m.TeleportWalk` decomposed).
    /// An ordinary move settles both.
    ///
    /// The wire is a flat `i32`; [`Self::bits`] / [`Self::from_bits`] flatten it.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Settle: i32 {
        /// The tiles the move goes through resolve their [经过] effects. A walk:
        /// every tile it steps on. A teleport: the destination counts as passed
        /// when the teleport actually moves the player (`From != to`), or when
        /// this bit is forced (C# `TeleportWalk`, 「原地也算 [经过]」).
        const ROUTE = 1;
        /// The landing resolves (C# `m.Resolve`): `settleBefore` -> `settle` ->
        /// `land` -> `settleAfter`. On a teleport this gates the destination's
        /// [经过] too (C# 24371 returns before the pass block when `!m.Resolve`).
        const DEST = 2;
    }
}

/// An abnormal effect (C# `Abnormal.Kind`), carried on `abnormalGuard` /
/// `abnormal` triggers. C# `AbName` gives their names: [停留] / [晕眩] / [除外] /
/// [传送] / [强制移动] / [强制停下] / 反方向移动.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbKind {
    Stay = 1,
    Stun = 2,
    Exile = 3,
    Teleport = 4,
    Forced = 5,
    Stop = 6,
    Reverse = 7,
}

impl AbKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            1 => Self::Stay,
            2 => Self::Stun,
            3 => Self::Exile,
            4 => Self::Teleport,
            5 => Self::Forced,
            6 => Self::Stop,
            7 => Self::Reverse,
            _ => return None,
        })
    }

    /// The C# `Abnormal.Kind` string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stay => "stay",
            Self::Stun => "stun",
            Self::Exile => "exile",
            Self::Teleport => "teleport",
            Self::Forced => "forced",
            Self::Stop => "stop",
            Self::Reverse => "reverse",
        }
    }
}

/// One of a player's card piles (C# `_hidden[s].hand` / `.discard` / `.draw`,
/// and the placed field cards).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardPile {
    Hand = 0,
    Discard = 1,
    /// The draw pile, **top card first**.
    Deck = 2,
    /// Cards placed on the player's field, in placement order.
    Field = 3,
}

impl CardPile {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Hand,
            1 => Self::Discard,
            2 => Self::Deck,
            3 => Self::Field,
            _ => return None,
        })
    }
}

/// `circleAffected`'s `Trigger.value` -- which half of the CiRCLE reward was
/// taken. `0` is 「[获得]资金」, `1` is the card. The stunned path forces the
/// card, so a money-only clause cannot fire there.
///
/// **Must match the engine's `CIRCLE_REWARD_MONEY` / `CIRCLE_REWARD_CARD` in
/// `game-core/src/engine/play.rs`** (the same relation `Arg` has to
/// `game-core/src/msg.rs`): `game-core` is the source, this is the mirror.
/// Renumbering one means renumbering the other.
pub const REWARD_MONEY: i32 = 0;
pub const REWARD_CARD: i32 = 1;

/// Trigger kinds a counteraction can be checked against (C# `Trigger.Kind`).
///
/// The values are a wire enum: the host fills them at the same points the C#
/// raises its `Trigger`s. Kinds the engine does not raise yet still exist here
/// so a card's `can_counteract` can state its real condition; it simply never sees
/// that kind until the engine raises it (TODO in `game-core`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TriggerKind {
    #[default]
    None = 0,
    Roll = 1,
    MoveRoll = 2,
    TurnStart = 3,
    Pass = 4,
    PassPlayer = 5,
    SettleBefore = 6,
    Settle = 7,
    Mortgage = 8,
    Pay = 9,
    Paid = 10,
    Bankrupt = 11,
    Card = 12,
    Event = 13,
    Abnormal = 14,
    Target = 15,
    Stop = 16,
    Teleport = 17,
    SkillTeleport = 18,
    Stun = 19,
    Stay = 20,
    Exile = 21,
    Forced = 22,
    State = 23,
    Counteracted = 24,
    DrawOut = 25,
    CircleAffected = 26,
    TwoCards = 27,
    /// v10: the missing pre-half of `TurnStart`.
    TurnStartBefore = 28,
    /// v10: the missing pre-half of `Pass`.
    PassBefore = 29,
    /// v10: the missing pre-half of `Mortgage`.
    MortgageBefore = 30,
    /// v10: the missing pre-half of `Bankrupt`.
    BankruptBefore = 31,
    /// v10: the missing post-half of `Card`.
    CardAfter = 32,
    /// v10: the missing post-half of `Event`.
    EventAfter = 33,
    /// v10: the missing post-half of `SettleBefore`/`Settle` (fires after
    /// `land()` resolves).
    SettleAfter = 34,
    /// v10: new action hook -- before `Card.Buy` resolves.
    BuyBefore = 35,
    /// v10: new action hook -- after `Card.Buy` resolves.
    BuyAfter = 36,
    /// v10: new action hook -- before building a house resolves.
    BuildBefore = 37,
    /// v10: new action hook -- after building a house resolves.
    BuildAfter = 38,
    /// v10: new action hook -- before a hand discard resolves.
    DiscardBefore = 39,
    /// v10: new action hook -- after a hand discard resolves.
    DiscardAfter = 40,
    /// v10: new action hook -- before ending the turn.
    EndTurnBefore = 41,
    /// v10: new action hook -- after ending the turn.
    EndTurnAfter = 42,
    /// v10: new action hook -- before a player forfeits.
    LeaveBefore = 43,
    /// v10: new action hook -- after a player forfeits.
    LeaveAfter = 44,

    // v17: field-card (`Fx`) hook points. These are NOT [反击] points -- the
    // engine runs every placed card's `counteract` against them automatically. They
    // are distinct kinds from the counteraction kinds above so a card can tell a
    // field effect from a hand counteraction by its kind alone.
    /// `Fx.TurnEnd` -- a turn just ended (any player's).
    TurnEnd = 45,
    /// `Fx.Drawn` -- the player drew cards.
    Drawn = 46,
    /// `Fx.PassTile` -- the player passed/stopped on a tile during a move.
    PassTile = 47,
    /// `Fx.PayAfter` -- a payment settled.
    PayAfter = 48,
    /// `Fx.RollAfter` -- a move roll resolved.
    RollAfter = 49,
    /// `Fx.CardPlayed` -- a card's hand effect resolved.
    CardPlayed = 50,
    /// `Fx.Targeted` -- the player was targeted.
    Targeted = 51,
    /// `Fx.PayChoose` -- the player is about to pay (may modify/decline).
    PayChoose = 52,
    /// v23: C# `Fx.TurnEndBefore` -- first step of a turn end, before the `AtEnd` callbacks and the status wear-off.
    TurnEndBefore = 53,
    /// v23: C# `Fx.TurnEndAfter` -- after `TurnEnd`; the `AfterEnd` callbacks run here.
    TurnEndAfter = 54,
    /// v23: C# `Fx.PayAdd` -- a payment's amount, first modifier pass (before any money moves).
    PayAdd = 55,
    /// v23: C# `Fx.PayMul` -- second modifier pass, after `PayAdd`.
    PayMul = 56,
    /// v23: C# `Fx.PayAt` -- after `PayChoose`, before the `pay` [反击] window.
    PayAt = 57,
    /// v23: C# `Card.OnDiscarded` -- this card (named on `t.card`) just went to the discard pile.
    Discarded = 58,
    /// v23: C# `Card.DeckBeforeGame` -- this card is in a draw pile before the opening deal.
    DeckBeforeGame = 59,
    /// v23: C# `Card.DeckAtGameStart` -- this card is in a draw pile or hand after the mulligan.
    DeckAtGameStart = 60,
    /// v23: C# `Fx.Drew` -- a player drew `t.value` cards (after each card's own `Drawn`).
    Drew = 61,
    /// v23: C# `Fx.Reshuffled` -- a player's discard pile was shuffled back into its deck.
    Reshuffled = 62,
    /// v23: C# `Fx.Bought` -- a player became the owner of `t.tile` (buy or auction).
    Bought = 63,
    /// v23: C# `Fx.SettleInstead` -- a field card may replace the landed tile's effect: do it and call `trigger::set_cancelled()`.
    SettleInstead = 64,
    /// v23: C# `Fx.BeforeOut` -- a player is about to leave the game (bankrupt or forfeit).
    BeforeOut = 65,
    /// v23: C# `Teleported` -- a teleport finished (after its settlement, or at once if it does not settle).
    Teleported = 66,
    /// v23: C# `RollMove`'s `RollPlan` pass -- before the main move's dice are
    /// rolled. The host runs every placed card's `On::RollPlan` here.
    RollPlan = 67,
    /// v25: C# `IAbnormalGuard.Guard` -- an abnormal effect is about to hit
    /// `t.target` (`trigger::abnormal_kind()` says which). A field card blocks it
    /// with `trigger::set_cancelled()`. Then, if someone else caused it, the
    /// `abnormal` [反击] window opens.
    AbnormalGuard = 68,
    /// v26: C# `Fx.ImmuneAll` -- is `t.player_id` untouchable by `t.by_card`'s
    /// effects? A field card claims it with `trigger::set_cancelled()`. Asked
    /// before targeting `t.player_id`, before an abnormal effect, and before a
    /// card-driven payment from/to `t.player_id`.
    ImmuneAll = 69,
    /// v26: C# `Fx.Untargetable(seat, by)` -- `t.by_card`'s card is about to
    /// target `t.player_id` (after the `_targeted` counter). Block with
    /// `trigger::set_cancelled()`.
    Untargetable = 70,
    /// v26: C# `IRedirect.Redirects` -- a single-target card is about to target
    /// `t.target`; a field card takes the hit with `trigger::set_target(player_id)`.
    Redirect = 71,
    /// v27: **a card effect is declared at someone**. This is the [反击] key for
    /// 「被其他玩家的卡效果影响」 -- one stable fact, not a union of outcome
    /// kinds. It is raised when a link's recipients are named, before any
    /// settlement, and carries the link's effect list (`ctx::effect`).
    ///
    /// The old `Target` / `Abnormal` / `Pay` windows are settlement hooks now:
    /// they fire as the effect settles and can no longer reconstruct this
    /// clause. A counter that means "an effect hit me" listens here and asks
    /// `effect::`; one that means "a payment settled" listens at [`HookKind::PayAfter`].
    Effect = 72,
    /// v28: C# `Fx.Built` / `HouseAdded` -- a house was just added to
    /// `t.tile` (now `t.value` houses) by `t.player_id`.
    HouseAdded = 75,
    /// v28: C# `Fx.FireSpent` -- `t.player_id` just spent `t.value` fire
    /// (「每当你消耗火罐时」). Fires after the spend commits.
    FireSpent = 73,
    /// v28: C# `Fx.SkillUsed` -- `t.player_id` just used their character skill
    /// (「使用自己原有的技能（2）时」). `t.card` is the skill rule's id.
    SkillUsed = 74,
    /// v29: a placed card's [奇迹水晶] count was just written (set or add).
    /// `t.card` is the card whose count moved, `t.player_id` its owner, and
    /// `t.value` is the **change applied** (0 = a write that landed on the same
    /// count, e.g. a card placed with none). The count after the write is
    /// `ctx::crystals()` on the instance itself.
    ///
    /// The 「此卡上不再拥有[奇迹水晶]时」 clauses (AG:绯红之魂 (3) and kin) listen
    /// here instead of testing the count at each spend site, so a count that is
    /// emptied by *any* path still leaves the field.
    CrystalsChanged = 76,
}

impl TriggerKind {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Roll,
            2 => Self::MoveRoll,
            3 => Self::TurnStart,
            4 => Self::Pass,
            5 => Self::PassPlayer,
            6 => Self::SettleBefore,
            7 => Self::Settle,
            8 => Self::Mortgage,
            9 => Self::Pay,
            10 => Self::Paid,
            11 => Self::Bankrupt,
            12 => Self::Card,
            13 => Self::Event,
            14 => Self::Abnormal,
            15 => Self::Target,
            16 => Self::Stop,
            17 => Self::Teleport,
            18 => Self::SkillTeleport,
            19 => Self::Stun,
            20 => Self::Stay,
            21 => Self::Exile,
            22 => Self::Forced,
            23 => Self::State,
            24 => Self::Counteracted,
            25 => Self::DrawOut,
            26 => Self::CircleAffected,
            27 => Self::TwoCards,
            28 => Self::TurnStartBefore,
            29 => Self::PassBefore,
            30 => Self::MortgageBefore,
            31 => Self::BankruptBefore,
            32 => Self::CardAfter,
            33 => Self::EventAfter,
            34 => Self::SettleAfter,
            35 => Self::BuyBefore,
            36 => Self::BuyAfter,
            37 => Self::BuildBefore,
            38 => Self::BuildAfter,
            39 => Self::DiscardBefore,
            40 => Self::DiscardAfter,
            41 => Self::EndTurnBefore,
            42 => Self::EndTurnAfter,
            43 => Self::LeaveBefore,
            44 => Self::LeaveAfter,
            45 => Self::TurnEnd,
            46 => Self::Drawn,
            47 => Self::PassTile,
            48 => Self::PayAfter,
            49 => Self::RollAfter,
            50 => Self::CardPlayed,
            51 => Self::Targeted,
            52 => Self::PayChoose,
            53 => Self::TurnEndBefore,
            54 => Self::TurnEndAfter,
            55 => Self::PayAdd,
            56 => Self::PayMul,
            57 => Self::PayAt,
            58 => Self::Discarded,
            59 => Self::DeckBeforeGame,
            60 => Self::DeckAtGameStart,
            61 => Self::Drew,
            62 => Self::Reshuffled,
            63 => Self::Bought,
            64 => Self::SettleInstead,
            65 => Self::BeforeOut,
            66 => Self::Teleported,
            67 => Self::RollPlan,
            68 => Self::AbnormalGuard,
            69 => Self::ImmuneAll,
            70 => Self::Untargetable,
            71 => Self::Redirect,
            72 => Self::Effect,
            75 => Self::HouseAdded,
            73 => Self::FireSpent,
            74 => Self::SkillUsed,
            76 => Self::CrystalsChanged,
            _ => Self::None,
        }
    }

    /// The C# `Trigger.Kind` string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Roll => "roll",
            Self::MoveRoll => "moveRoll",
            Self::TurnStart => "turnStart",
            Self::Pass => "pass",
            Self::PassPlayer => "passPlayer",
            Self::SettleBefore => "settleBefore",
            Self::Settle => "settle",
            Self::Mortgage => "mortgage",
            Self::Pay => "pay",
            Self::Paid => "paid",
            Self::Bankrupt => "bankrupt",
            Self::Card => "card",
            Self::Event => "event",
            Self::Abnormal => "abnormal",
            Self::Target => "target",
            Self::Stop => "stop",
            Self::Teleport => "teleport",
            Self::SkillTeleport => "skillTeleport",
            Self::Stun => "stun",
            Self::Stay => "stay",
            Self::Exile => "exile",
            Self::Forced => "forced",
            Self::State => "state",
            Self::Counteracted => "counteracted",
            Self::DrawOut => "drawOut",
            Self::CircleAffected => "circleAffected",
            Self::TwoCards => "twoCards",
            Self::TurnStartBefore => "turnStartBefore",
            Self::PassBefore => "passBefore",
            Self::MortgageBefore => "mortgageBefore",
            Self::BankruptBefore => "bankruptBefore",
            Self::CardAfter => "cardAfter",
            Self::EventAfter => "eventAfter",
            Self::SettleAfter => "settleAfter",
            Self::BuyBefore => "buyBefore",
            Self::BuyAfter => "buyAfter",
            Self::BuildBefore => "buildBefore",
            Self::BuildAfter => "buildAfter",
            Self::DiscardBefore => "discardBefore",
            Self::DiscardAfter => "discardAfter",
            Self::EndTurnBefore => "endTurnBefore",
            Self::EndTurnAfter => "endTurnAfter",
            Self::LeaveBefore => "leaveBefore",
            Self::LeaveAfter => "leaveAfter",
            Self::TurnEnd => "turnEnd",
            Self::Drawn => "drawn",
            Self::PassTile => "passTile",
            Self::PayAfter => "payAfter",
            Self::RollAfter => "rollAfter",
            Self::CardPlayed => "cardPlayed",
            Self::Targeted => "targeted",
            Self::PayChoose => "payChoose",
            Self::TurnEndBefore => "turnEndBefore",
            Self::TurnEndAfter => "turnEndAfter",
            Self::PayAdd => "payAdd",
            Self::PayMul => "payMul",
            Self::PayAt => "payAt",
            Self::Discarded => "discarded",
            Self::DeckBeforeGame => "deckBeforeGame",
            Self::DeckAtGameStart => "deckAtGameStart",
            Self::Drew => "drew",
            Self::Reshuffled => "reshuffled",
            Self::Bought => "bought",
            Self::SettleInstead => "settleInstead",
            Self::BeforeOut => "beforeOut",
            Self::Teleported => "teleported",
            Self::RollPlan => "rollPlan",
            Self::AbnormalGuard => "abnormalGuard",
            Self::ImmuneAll => "immuneAll",
            Self::Untargetable => "untargetable",
            Self::Redirect => "redirect",
            Self::Effect => "effect",
            Self::HouseAdded => "houseAdded",
            Self::FireSpent => "fireSpent",
            Self::SkillUsed => "skillUsed",
            Self::CrystalsChanged => "crystalsChanged",
        }
    }

    /// Inverse of [`as_str`] (`game-rules` maps engine trigger kinds through here).
    pub fn from_str(s: &str) -> Self {
        match s {
            "roll" => Self::Roll,
            "moveRoll" => Self::MoveRoll,
            "turnStart" => Self::TurnStart,
            "pass" => Self::Pass,
            "passPlayer" => Self::PassPlayer,
            "settleBefore" => Self::SettleBefore,
            "settle" => Self::Settle,
            "mortgage" => Self::Mortgage,
            "pay" => Self::Pay,
            "paid" => Self::Paid,
            "bankrupt" => Self::Bankrupt,
            "card" => Self::Card,
            "event" => Self::Event,
            "abnormal" => Self::Abnormal,
            "target" => Self::Target,
            "stop" => Self::Stop,
            "teleport" => Self::Teleport,
            "skillTeleport" => Self::SkillTeleport,
            "stun" => Self::Stun,
            "stay" => Self::Stay,
            "exile" => Self::Exile,
            "forced" => Self::Forced,
            "state" => Self::State,
            "counteracted" => Self::Counteracted,
            "drawOut" => Self::DrawOut,
            "circleAffected" => Self::CircleAffected,
            "twoCards" => Self::TwoCards,
            "turnStartBefore" => Self::TurnStartBefore,
            "passBefore" => Self::PassBefore,
            "mortgageBefore" => Self::MortgageBefore,
            "bankruptBefore" => Self::BankruptBefore,
            "cardAfter" => Self::CardAfter,
            "eventAfter" => Self::EventAfter,
            "settleAfter" => Self::SettleAfter,
            "buyBefore" => Self::BuyBefore,
            "buyAfter" => Self::BuyAfter,
            "buildBefore" => Self::BuildBefore,
            "buildAfter" => Self::BuildAfter,
            "discardBefore" => Self::DiscardBefore,
            "discardAfter" => Self::DiscardAfter,
            "endTurnBefore" => Self::EndTurnBefore,
            "endTurnAfter" => Self::EndTurnAfter,
            "leaveBefore" => Self::LeaveBefore,
            "leaveAfter" => Self::LeaveAfter,
            "turnEnd" => Self::TurnEnd,
            "drawn" => Self::Drawn,
            "passTile" => Self::PassTile,
            "payAfter" => Self::PayAfter,
            "rollAfter" => Self::RollAfter,
            "cardPlayed" => Self::CardPlayed,
            "targeted" => Self::Targeted,
            "payChoose" => Self::PayChoose,
            "turnEndBefore" => Self::TurnEndBefore,
            "turnEndAfter" => Self::TurnEndAfter,
            "payAdd" => Self::PayAdd,
            "payMul" => Self::PayMul,
            "payAt" => Self::PayAt,
            "discarded" => Self::Discarded,
            "deckBeforeGame" => Self::DeckBeforeGame,
            "deckAtGameStart" => Self::DeckAtGameStart,
            "drew" => Self::Drew,
            "reshuffled" => Self::Reshuffled,
            "bought" => Self::Bought,
            "settleInstead" => Self::SettleInstead,
            "beforeOut" => Self::BeforeOut,
            "teleported" => Self::Teleported,
            "rollPlan" => Self::RollPlan,
            "abnormalGuard" => Self::AbnormalGuard,
            "immuneAll" => Self::ImmuneAll,
            "untargetable" => Self::Untargetable,
            "redirect" => Self::Redirect,
            "effect" => Self::Effect,
            "houseAdded" => Self::HouseAdded,
            "fireSpent" => Self::FireSpent,
            "skillUsed" => Self::SkillUsed,
            "crystalsChanged" => Self::CrystalsChanged,
            _ => Self::None,
        }
    }

    /// Which of the three vocabularies this event belongs to. Derived from the
    /// typed views below -- there is no second list to keep in step.
    pub const fn role(self) -> EventRole {
        let v = self as i32;
        if GateKind::from_i32(v).is_some() {
            EventRole::Gate
        } else if ChainKind::from_i32(v).is_some() {
            EventRole::Link
        } else {
            EventRole::Hook
        }
    }
}

/// Which vocabulary an event kind belongs to.
///
/// [`TriggerKind`] is the *wire* enum: every event the engine raises, and what
/// crosses the guest boundary. Cards do not declare against it -- they declare
/// against one of the three typed views, and the type says what the event is
/// for:
///
/// * [`ChainKind`] -- a chain link answers it, so a [反击] window opens.
/// * [`HookKind`] -- a field-card (`Fx`) settlement point; placed cards run
///   automatically, no window.
/// * [`GateKind`] -- a question posed to placed cards at declaration or at
///   resolution (`may this effect name S?` / `does anything land on S?`).
///
/// The `matches!` list that used to be `is_hook_only` is now
/// [`TriggerKind::role`] reading these types.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventRole {
    /// Opens a [反击] window -- a chain link can answer it.
    Link = 0,
    /// Field-card hook only -- no window.
    Hook = 1,
    /// A query to placed cards -- no window.
    Gate = 2,
}

macro_rules! declare_kinds {
    ($(#[$doc:meta])* $name:ident { $($(#[$vdoc:meta])* $variant:ident = $val:expr,)* }) => {
        $(#[$doc])*
        #[repr(i32)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($(#[$vdoc])* $variant = $val,)*
        }

        impl $name {
            /// The wire value, shared with [`TriggerKind`].
            pub const fn as_i32(self) -> i32 {
                self as i32
            }

            pub const fn from_i32(v: i32) -> Option<Self> {
                Some(match v {
                    $($val => Self::$variant,)*
                    _ => return None,
                })
            }
        }

        impl PartialEq<TriggerKind> for $name {
            fn eq(&self, other: &TriggerKind) -> bool {
                *self as i32 == *other as i32
            }
        }

        impl PartialEq<$name> for TriggerKind {
            fn eq(&self, other: &$name) -> bool {
                *self as i32 == *other as i32
            }
        }
    };
}

declare_kinds! {
    /// What a chain link answers: the [反击] selectors.
    ///
    /// A card declares these on `On::Counteract`. These are the moments at which a
    /// counter may be played -- an effect being *declared*, not an outcome
    /// having settled. [`HookKind`] and [`GateKind`] values are deliberately
    /// absent: a counteraction cannot be offered at a settlement hook or at a gate.
    ChainKind {
        Roll = 1,
        MoveRoll = 2,
        TurnStart = 3,
        Pass = 4,
        PassPlayer = 5,
        SettleBefore = 6,
        Settle = 7,
        Mortgage = 8,
        Paid = 10,
        Bankrupt = 11,
        Card = 12,
        Event = 13,
        Stop = 16,
        Teleport = 17,
        SkillTeleport = 18,
        Stun = 19,
        Stay = 20,
        Exile = 21,
        Forced = 22,
        State = 23,
        Counteracted = 24,
        DrawOut = 25,
        CircleAffected = 26,
        TwoCards = 27,
        TurnStartBefore = 28,
        PassBefore = 29,
        MortgageBefore = 30,
        BankruptBefore = 31,
        CardAfter = 32,
        EventAfter = 33,
        SettleAfter = 34,
        BuyBefore = 35,
        BuyAfter = 36,
        BuildBefore = 37,
        BuildAfter = 38,
        DiscardBefore = 39,
        DiscardAfter = 40,
        EndTurnBefore = 41,
        EndTurnAfter = 42,
        LeaveBefore = 43,
        LeaveAfter = 44,
        /// The effect declaration itself -- the [反击] key for 「被…效果影响」.
        Effect = 72,
        HouseAdded = 75,
        FireSpent = 73,
        SkillUsed = 74,
    }
}

declare_kinds! {
    /// What a placed card can hook: everything except the [`GateKind`] questions.
    ///
    /// A card declares these on `On::Hook`. The engine runs every *placed*
    /// card's entry against them automatically -- no declaration, no prompt.
    ///
    /// This **overlaps** [`ChainKind`] on purpose. A kind may be both a [反击]
    /// point and a hook point -- `SettleBefore` is a moment at which a hand card
    /// can be played *and* a field card can counteract -- and the two declarations
    /// are distinct entries (`On::Counteract` vs `On::Hook`). The engine's dispatch
    /// has always worked this way; the types now say so. Kinds that are *only*
    /// hooks (most of `Fx`) simply have no [`ChainKind`] counterpart.
    HookKind {
        Roll = 1,
        MoveRoll = 2,
        TurnStart = 3,
        Pass = 4,
        PassPlayer = 5,
        SettleBefore = 6,
        Settle = 7,
        Mortgage = 8,
        Pay = 9,
        Paid = 10,
        Bankrupt = 11,
        Card = 12,
        Event = 13,
        Abnormal = 14,
        Target = 15,
        Stop = 16,
        Teleport = 17,
        SkillTeleport = 18,
        Stun = 19,
        Stay = 20,
        Exile = 21,
        Forced = 22,
        State = 23,
        Counteracted = 24,
        DrawOut = 25,
        CircleAffected = 26,
        TwoCards = 27,
        TurnStartBefore = 28,
        PassBefore = 29,
        MortgageBefore = 30,
        BankruptBefore = 31,
        CardAfter = 32,
        EventAfter = 33,
        SettleAfter = 34,
        BuyBefore = 35,
        BuyAfter = 36,
        BuildBefore = 37,
        BuildAfter = 38,
        DiscardBefore = 39,
        DiscardAfter = 40,
        EndTurnBefore = 41,
        EndTurnAfter = 42,
        LeaveBefore = 43,
        LeaveAfter = 44,
        TurnEnd = 45,
        Drawn = 46,
        PassTile = 47,
        PayAfter = 48,
        RollAfter = 49,
        CardPlayed = 50,
        Targeted = 51,
        PayChoose = 52,
        TurnEndBefore = 53,
        TurnEndAfter = 54,
        PayAdd = 55,
        PayMul = 56,
        PayAt = 57,
        Discarded = 58,
        DeckBeforeGame = 59,
        DeckAtGameStart = 60,
        Drew = 61,
        Reshuffled = 62,
        Bought = 63,
        SettleInstead = 64,
        BeforeOut = 65,
        Teleported = 66,
        RollPlan = 67,
        FireSpent = 73,
        SkillUsed = 74,
        HouseAdded = 75,
        CrystalsChanged = 76,
    }
}

declare_kinds! {
    /// Questions posed to placed cards, at declaration or at resolution.
    ///
    /// A card declares these on `On::Gate`. These are not occurrences -- nothing
    /// "happened" -- so they are not triggers and cannot be [反击]'d. They ask
    /// a placed card whether an effect may name someone, or whether anything
    /// lands on them.
    GateKind {
        /// An abnormal effect is about to hit `t.target`; block with
        /// `trigger::set_cancelled()`. Then, if someone else caused it, the
        /// `abnormal` [反击] window opens.
        AbnormalGuard = 68,
        /// Is `t.player_id` untouchable by `t.by_card`'s effects? Asked at
        /// **resolution** -- the effect is named, the chain forms, and it lands
        /// as nothing. Claim with `trigger::set_cancelled()`.
        ImmuneAll = 69,
        /// `t.by_card`'s card is about to name `t.player_id`. Asked at
        /// **declaration** -- block and the effect cannot name them at all, so
        /// no chain forms against them. Claim with `trigger::set_cancelled()`.
        Untargetable = 70,
        /// A single-target effect is about to name `t.target`; a placed card
        /// takes the hit with `trigger::set_target(player_id)`. **Declaration
        /// re-naming**: the recipient set is settled before the chain opens, so
        /// whoever holds the name is the one who answers.
        Redirect = 71,
    }
}

pub fn pack(ptr: u32, len: u32) -> i64 {
    (((ptr as u64) << 32) | len as u64) as i64
}

pub fn unpack(v: i64) -> (u32, u32) {
    let v = v as u64;
    ((v >> 32) as u32, (v & 0xFFFF_FFFF) as u32)
}

/// One entry of the guest card manifest (`rt::manifest`). The index in the
/// array is the card handle the host passes back. Shared with the host so the
/// two sides cannot disagree on the wire layout.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    /// The card's entry points, in declaration order (the `entry` index the
    /// host passes back to `bandori_on`).
    pub on: Vec<ManifestOn>,
}

/// One entry point in the manifest: what it is and which trigger kinds it
/// answers (empty for non-trigger entries).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestOn {
    /// An [`OnKind`] as `i32`.
    pub kind: i32,
    /// `TriggerKind`s as `i32`.
    pub triggers: Vec<i32>,
}

/// What a card entry point is (`card_sdk::On`'s variants).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnKind {
    Play = 0,
    // 1 was `CantPlay`, folded into `Play`'s gate.
    Counteract = 2,
    Hook = 3,
    AtEnd = 4,
    RollPlan = 5,
    Gate = 6,
}

impl OnKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Play,
            2 => Self::Counteract,
            3 => Self::Hook,
            4 => Self::AtEnd,
            5 => Self::RollPlan,
            6 => Self::Gate,
            _ => return None,
        })
    }
}
