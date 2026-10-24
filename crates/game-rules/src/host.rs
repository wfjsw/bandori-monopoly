//! wasmi host: loading a set of card modules, the `ruleset` import table, replay,
//! and nested cross-module card calls.

#[cfg(feature = "sandbox")]
use std::collections::HashMap;
use std::fmt;
#[cfg(feature = "sandbox")]
use std::sync::Arc;

#[cfg(feature = "sandbox")]
use card_sdk::abi::{self, export, ABI_VERSION, IMPORT_MODULE};
use card_sdk::abi::{AbKind, ManifestOn, ManifestTileText, OnKind, PromptKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
// The engine behind the host: wasmi (an interpreter) in the browser, wasmtime
// (the optimizing compiler) on the server. Same modules and same behaviour.
// Compiled only with the `sandbox` feature -- the linked path (`native-rules`)
// has no wasm engine at all.
#[cfg(all(
    feature = "sandbox",
    any(target_arch = "wasm32", feature = "wasmi-native")
))]
#[path = "be_wasmi.rs"]
mod be;
#[cfg(all(
    feature = "sandbox",
    not(target_arch = "wasm32"),
    not(feature = "wasmi-native")
))]
#[path = "be_wasmtime.rs"]
mod be;

#[cfg(feature = "sandbox")]
use be::{
    err, error_text, has_func, instantiate, is_need_input, need_input, read_guest, read_mem,
    set_fuel, write_guest,
};
#[cfg(feature = "sandbox")]
use be::{Caller, Engine, Error, Linker, Module, Store};

/// Default [`RulesHandle`] for [`HostState`]: the sandbox's module index.
/// Outside the sandbox there is no engine-owned index -- callers name their
/// own handle (e.g. `rules-native`'s `NativeRulesHandle`).
#[cfg(feature = "sandbox")]
pub type DefaultRules = Arc<Inner>;
#[cfg(not(feature = "sandbox"))]
pub type DefaultRules = ();

#[cfg(feature = "sandbox")]
use crate::hostfns;
use crate::world::CardWorld;

/// Measurement counters for `docs/BOT.md` §5 (B0). Compiled out unless the
/// `bot-cost` feature is on; nothing behavioural either way.
#[cfg(feature = "bot-cost")]
pub mod bot_cost {
    use std::sync::atomic::AtomicU64;
    /// Card module instantiations (a fresh `Store` + `Instance` per card run,
    /// hook, gate or guard probe).
    pub static INSTANTIATIONS: AtomicU64 = AtomicU64::new(0);
    /// ns spent creating the store and instantiating the module.
    pub static INSTANTIATE_NS: AtomicU64 = AtomicU64::new(0);
    /// ns spent building the per-run `Store` (state, limiter, fuel).
    pub static STORE_NS: AtomicU64 = AtomicU64::new(0);
    /// ns spent inside the guest entry call (includes host import callbacks).
    pub static GUEST_NS: AtomicU64 = AtomicU64::new(0);
    /// `World` clones at the card-host boundary (one per store built).
    pub static HOST_WORLD_CLONES: AtomicU64 = AtomicU64::new(0);
    /// counteraction window opens: `hand_counteractions` rings that passed the
    /// valid-option-first pre-scan (at least one candidate survived the kind
    /// bitmask + seat eligibility + condition).
    pub static COUNTERACT_WINDOWS: AtomicU64 = AtomicU64::new(0);
    /// counteraction windows skipped whole because no candidate could respond.
    pub static COUNTERACT_WINDOWS_SKIPPED: AtomicU64 = AtomicU64::new(0);
    /// Per-candidate counteraction probes (condition / guard evaluations).
    pub static COUNTERACT_PROBES: AtomicU64 = AtomicU64::new(0);
    /// Probes answered from the per-window processed set (no re-evaluation).
    pub static COUNTERACT_PROBE_MEMO_HITS: AtomicU64 = AtomicU64::new(0);
    /// counteraction cards actually declared (left a hand for a counter link).
    pub static COUNTERACT_DECLARED: AtomicU64 = AtomicU64::new(0);
}

/// Fuel per top-level effect run, shared by any nested `play_card` calls. Plenty
/// for straight-line card logic; stops runaway loops at the same instruction on
/// every machine.
pub const DEFAULT_FUEL: u64 = 5_000_000;

#[cfg(feature = "sandbox")]
/// Fuel for the one-off `ruleset_metadata` export, which builds the whole
/// roster (every card, persona and group) and so dwarfs a single effect run.
const METADATA_FUEL: u64 = 500_000_000;

/// Maximum `play_card` nesting (card A plays B plays C ...).
pub const MAX_NESTING: u32 = 8;

/// One card, as declared in its module's manifest: its id and its entry-point
/// table (`card_sdk::On`), each entry with the trigger kinds it answers, plus
/// the card's declared static **properties**.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInfo {
    pub id: String,
    pub on: Vec<ManifestOn>,
    /// The card rule's declared static properties (`CardDef::props`), `key ->
    /// value`. Keys are `game_core::state::prop` / `card_sdk::abi::prop`
    /// constants; a key the card does not declare is absent and reads as its
    /// default (`0`). A `BTreeMap` (not a `HashMap`) so iteration and
    /// serialization remain deterministic.
    #[serde(default)]
    pub props: std::collections::BTreeMap<String, i32>,
    /// Tile texts this card's entries speak (`On::texts`, ABI v59), in
    /// declaration order. See [`ManifestTileText`].
    #[serde(default)]
    pub texts: Vec<ManifestTileText>,
    /// Board role (`CardDef::board_role`, ABI v66): `""` / `"tile"` / `"mark"` /
    /// `"status"`.
    #[serde(default)]
    pub board_role: String,
    /// Tile kinds this rule governs (`CardDef::governs`, ABI v66).
    #[serde(default)]
    pub governs: Vec<String>,
    /// Gate-refusal i18n keys (`CardDef::gate_msgs`, ABI v89): `(gate kind i32, key)`.
    #[serde(default)]
    pub gate_msgs: Vec<(i32, String)>,
    /// Turn-skip i18n keys (`CardDef::gate_skip_msgs`).
    #[serde(default)]
    pub gate_skip_msgs: Vec<(i32, String)>,
    /// Card prop that exempts a play from this rule's act gate (`CardDef::exemption_prop`).
    #[serde(default)]
    pub exemption_prop: String,
}

impl CardInfo {
    /// Entries of `kind` answering `trigger`, in declaration order, walked
    /// lazily. The single home of the kind/trigger predicate: [`Self::entry`]
    /// takes the first and [`Self::entries`] collects. Kept private so the
    /// public surface stays `Option`/`Vec` and callers keep `len` / `is_empty` /
    /// indexing without ceremony.
    fn entry_iter(
        &self,
        kind: OnKind,
        trigger: Option<crate::TriggerKind>,
    ) -> impl Iterator<Item = i32> + '_ {
        self.on
            .iter()
            .enumerate()
            .filter(move |(_, o)| {
                o.kind == kind as i32
                    && trigger.is_none_or(|t| o.triggers.iter().any(|s| s == &t.spelling()))
            })
            .map(|(i, _)| i as i32)
    }

    /// The first entry of `kind` (answering `trigger`, for trigger-keyed kinds).
    pub fn entry(&self, kind: OnKind, trigger: Option<crate::TriggerKind>) -> Option<i32> {
        self.entry_iter(kind, trigger).next()
    }

    /// The entry named by a stable internal identifier (`ManifestOn::id`,
    /// docs/ACTIVATION.md §3). `""` and unknown ids answer `None` -- the
    /// caller's query is then `false`, which is the point: an unnamed entry is
    /// not externally addressable.
    pub fn entry_by_id(&self, id: &str) -> Option<i32> {
        if id.is_empty() {
            return None;
        }
        self.on
            .iter()
            .position(|o| o.id.as_deref() == Some(id))
            .map(|i| i as i32)
    }

    /// **Every** entry of `kind` answering `trigger`, in declaration order.
    /// A card may carry several independent effects on one timing (「初始1，
    /// 上限2」 plus 「每回合开始时」); all of them run.
    pub fn entries(&self, kind: OnKind, trigger: Option<crate::TriggerKind>) -> Vec<i32> {
        self.entry_iter(kind, trigger).collect()
    }

    /// The card's first declared Play entry, or `None` when it has none.
    ///
    /// A card may declare several `On::Play` entries as alternative
    /// activations (each with its own gate -- 「可以…传送到…」 vs 「将此卡放置到场上」).
    /// Which mode actually runs is a *runtime* question -- the first gated
    /// entry whose gate admits, failing that the first ungated -- and is
    /// answered by the play-gate walk (`play_entry_for`), not by this lookup.
    /// This answers the static question "which entry does `entry::PRESS`
    /// name", i.e. the first declared one.
    pub fn play_entry_preferring_gated(&self) -> Option<i32> {
        self.entry(OnKind::Play, None)
    }

    pub fn has_play(&self) -> bool {
        self.entry(OnKind::Play, None).is_some()
    }

    /// Declares a counteraction at this trigger kind.
    pub fn counteracts_to(&self, trigger: crate::TriggerKind) -> bool {
        self.entry(OnKind::Counteract, Some(trigger)).is_some()
    }

    /// Declares a field-card hook at this trigger kind.
    pub fn hooks(&self, trigger: crate::TriggerKind) -> bool {
        self.hook_entry(trigger).is_some()
    }

    /// The field-card entries that answer `trigger`, whether declared as a
    /// settlement hook (`On::Action` / `On::Filter`) or as a gate (`On::Gate`).
    /// All run automatically on a placed card and none opens a counteraction
    /// window; they are separate declarations only so the type can say which
    /// is a question and which is a value modifier.
    /// **All** of them run, in declaration order -- each carries its own
    /// condition / guard and at most one body entry per entry per raise.
    pub fn hook_entries(&self, trigger: crate::TriggerKind) -> Vec<i32> {
        // Action entries first, then Filter -- the order callers have always
        // seen. Do not "fix" this to global declaration order without a
        // rulebook check: the doc above says 「按声明顺序」 and this is not it
        // when a card declares a Filter before an Action.
        let mut v: Vec<i32> = self.entry_iter(OnKind::Action, Some(trigger)).collect();
        v.extend(self.entry_iter(OnKind::Filter, Some(trigger)));
        v
    }

    /// The first field-card entry answering `trigger` (see [`Self::hook_entries`]):
    /// the first `On::Action` entry if there is one, else the first `On::Filter`.
    pub fn hook_entry(&self, trigger: crate::TriggerKind) -> Option<i32> {
        self.entry(OnKind::Action, Some(trigger))
            .or_else(|| self.entry(OnKind::Filter, Some(trigger)))
    }

    /// Scheduled turn-end callback (`ctx::at_turn_end`): an `On::Schedule`
    /// entry. True iff the card has any `OnKind::Schedule` entry.
    pub fn has_at_end(&self) -> bool {
        self.entry(OnKind::Schedule, None).is_some()
    }
}

/// What to run. `Ruleset` resolves it to the card's matching entry point; a
/// card without one simply does nothing.
#[derive(Debug, Clone)]
pub enum Call {
    /// `On::Play` -- the effect when played from hand. `arg` is the activation
    /// argument the play path's `OP_DECLARE` collected (docs/ACTIVATION.md §2)
    /// as `postcard` bytes; empty when the entry has no declare.
    Play {
        card: i32,
        player_id: i32,
        arg: Vec<u8>,
    },
    /// `On::Counteract` -- the counteraction effect, for the world's current
    /// trigger kind. `arg` is the argument `finish_offer`'s `OP_DECLARE`
    /// recorded on the chain link.
    Counteract {
        card: i32,
        player_id: i32,
        arg: Vec<u8>,
    },
    /// `On::Action` -- the field-card hook for `kind`.
    Hook {
        card: i32,
        kind: crate::TriggerKind,
        player_id: i32,
    },
    /// Scheduled turn-end callback (`On::Schedule`). `entry` is the resolved
    /// manifest entry index (carried on the call so the run path does not
    /// re-look-up by kind).
    AtEnd {
        card: i32,
        player_id: i32,
        entry: i32,
    },
    /// Movement routine (`On::Filter(Filter::DiceRollPlan(…))`), called while the walk
    /// is being planned.
    DiceRollPlan { card: i32, player_id: i32 },
    /// `On::Settle` -- a rule's **settle body** (`docs/TILES.md`), run for the
    /// tile the rule instance governs. `player_id` is the player settling.
    Settle { card: i32, player_id: i32 },
    /// `On::Mortgage` -- the rule's mortgage body (规则书 基础规则 5). `tile`
    /// is the deed; `reason` is why (0 = player act, 1 = raise-funds,
    /// 2 = optional funding prompt).
    Mortgage {
        card: i32,
        player_id: i32,
        tile: i32,
        reason: i32,
    },
    /// `On::Redeem` -- the rule's redeem body (规则书 基础规则 6).
    Redeem {
        card: i32,
        player_id: i32,
        tile: i32,
    },
    /// `On::Buy` -- the rule's buy body (v80). `kind` is the
    /// [`card_sdk::abi::BuyKind`] wire value; `reason` is the source
    /// (0 = settle offer, 1 = card-driven, 2 = agent, 3 = acquire / auction).
    Buy {
        card: i32,
        player_id: i32,
        tile: i32,
        kind: i32,
        reason: i32,
    },
    /// `On::Build` -- the rule's build body (v80).
    Build {
        card: i32,
        player_id: i32,
        tile: i32,
        reason: i32,
    },
}

impl Call {
    /// The player whose card this is -- the cause (`t.ByCard`) of anything the
    /// effect does.
    pub fn player_id(&self) -> i32 {
        match *self {
            Call::Play { player_id, .. }
            | Call::Counteract { player_id, .. }
            | Call::Hook { player_id, .. }
            | Call::AtEnd { player_id, .. }
            | Call::DiceRollPlan { player_id, .. }
            | Call::Settle { player_id, .. }
            | Call::Mortgage { player_id, .. }
            | Call::Redeem { player_id, .. } => player_id,
            Call::Buy { player_id, .. } => player_id,
            Call::Build { player_id, .. } => player_id,
        }
    }

    pub fn card(&self) -> i32 {
        match *self {
            Call::Play { card, .. }
            | Call::Counteract { card, .. }
            | Call::Hook { card, .. }
            | Call::AtEnd { card, .. }
            | Call::DiceRollPlan { card, .. }
            | Call::Settle { card, .. }
            | Call::Mortgage { card, .. }
            | Call::Redeem { card, .. } => card,
            Call::Buy { card, .. } => card,
            Call::Build { card, .. } => card,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptOption {
    Int(i32),
    /// A labelled option (`ask_pick`).
    Str(crate::Msg),
    /// A tile ask option carrying its own label (`ask_tiles` / `opt_tile`).
    Tile {
        tile: i32,
        label: crate::Msg,
    },
}

/// A prompt the effect is blocked on -- the engine surfaces it to the
/// player (or bot) and logs the answer for the replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub kind: PromptKind,
    pub player_id: i32,
    pub title: crate::Msg,
    pub text: crate::Msg,
    pub options: Vec<PromptOption>,
    /// Position in the effect's answer log this prompt will fill.
    pub answer_slot: usize,
    /// Per-option prices for a tile ask (`opt_price` / `ask_tiles`).
    pub prices: Vec<i32>,
    /// The AI's preferred index on a tile ask (`opt_ai`); -1 = let the host fill.
    pub ai_hint: i32,
    /// How many of the options a multi-select wants (`opt_count` /
    /// `ask_pick_multi`). 0 = unused (every other prompt kind).
    pub count: i32,
}

/// Engine work a card effect needs mid-run (it may open counteraction windows or
/// prompt), paused so the engine can do it before the effect continues:
/// trigger (and the counteraction window) before the money moves. The engine answers
/// with the final amount to move -- `0` means the payment was cancelled.
// `Move` carries a `MoveCtx`, which has `f64` factors: no `Eq`.
#[derive(Debug, Clone, PartialEq)]
pub enum HostRequest {
    /// A payment (`t.Pay`): the engine runs the money pipeline on it and
    /// answers with the final amount (0 = cancelled). `from < 0` is a print
    /// (game -> `to`), `to < 0` a delete (`from` -> game), both >= 0 a
    /// pay-player. `src` is the reason line the money log carries.
    Pay {
        from: i32,
        to: i32,
        amount: i32,
        src: Option<crate::Msg>,
        /// Run the command-wide **pre-split** stage (`payCombinedAdd` /
        /// `payCombinedMul` / `payCombinedCancel`) on this payment's amount. `true`
        /// for every ordinary payment; a 「[分摊]」 leg sets it `false` because
        /// the card body has already shaped the command total through
        /// [`Self::PayTotal`] before dividing (`PIPELINE-AUDIT` Q2).
        total_stage: bool,
    },
    /// The command-wide **pre-split** stage alone (`PIPELINE-AUDIT` Q2): the
    /// engine runs `payCombinedAdd` → `payCombinedMul` → `payCombinedCancel` on `total`
    /// and answers with the shaped figure, or `-1` when a `payCombinedCancel`
    /// hook dropped the whole command. No money moves here -- the card body
    /// divides the answer and runs each share through [`Self::Pay`] with
    /// `total_stage: false`.
    PayTotal {
        from: i32,
        to: i32,
        amount: i32,
        src: Option<crate::Msg>,
    },
    /// An abnormal effect about to hit `player_id`: the engine runs the
    /// abnormal counteraction gate and answers 1 (it goes through) or 0 (blocked).
    Gate { player_id: i32, kind: AbKind },
    /// A card targeting a player (`tile < 0`) or a tile: the engine runs the
    /// targeting pipeline and answers with the player (or tile) actually
    /// targeted, or -1 when the targeting failed. `single` lets a field
    /// `redirect` hook move the hit (single-target plays only).
    Target {
        player_id: i32,
        tile: i32,
        single: bool,
    },
    /// The card shaped the move (via the plan ops) and asked for it to run
    /// now. The engine runs `Cx::card_move` -- it may open prompts -- and the
    /// effect then replays past this call. `plan` is the plan as the card
    /// left it.
    Move {
        player_id: i32,
        plan: game_core::engine::MoveCtx,
    },
    /// A position write with no settle and no move bookkeeping (a forced
    /// teleport). The engine runs the abnormal counteraction gate and applies the
    /// write to the **live** world, so a move that follows in the same effect
    /// starts at the destination -- and the replay skips the call (an answer
    /// is logged), so it cannot re-teleport over a move that already ran.
    /// Answers 1 (it went through) or 0 (the gate blocked it).
    Teleport { player_id: i32, tile: i32 },
    /// A teleport whose move tail runs through the engine's `teleport_as`
    /// **without** the abnormal [反击] window: the status's own return clause
    /// (「状态结束后在玩家回合开始时要[传送]到的格子」) is not a forced move
    /// the responder answers. `as_main` marks the move as the turn's
    /// [主要移动] and sets `turn.main_moved`. Answers 1 (it ran) or 0.
    TeleportAs {
        player_id: i32,
        tile: i32,
        resolve: bool,
        as_main: bool,
    },
    /// A full [触发结算] of `tile` for this player, without moving them.
    SettleAt {
        player_id: i32,
        tile: i32,
        main: bool,
    },
    /// The purchase itself (`docs/PURCHASE.md`).
    Buy {
        player_id: i32,
        tile: i32,
        /// A [`card_sdk::abi::BuyKind`] as `i32` (`0` = land).
        kind: i32,
    },
    /// v40: 「收购」 -- take a deed from its owner at a price.
    Acquire {
        player_id: i32,
        from: i32,
        tile: i32,
        price: i32,
    },
    /// v40: the agent offer's chosen branch. `kind` names it:
    /// `0` = buy an unowned tile, `1` = build on an own one, `2` = half-rent
    /// settle on another player's deed (the agent's 「half-charge的settle」).
    AgentOffer {
        player_id: i32,
        agent: i32,
        tile: i32,
        kind: i32,
    },
    /// v40: bind the running card as a turn-scoped lingering instance.
    Linger { player_id: i32, expires: i32 },
    /// Pay and raise one house on `tile`.
    Build { player_id: i32, tile: i32 },
    /// Prompt to build on one of `tiles`, then build.
    OfferBuild { player_id: i32, tiles: Vec<i32> },
    /// Mortgage one of the player's deeds.
    Mortgage { player_id: i32, tile: i32 },
    /// Apply a mortgage the tile rule already approved: flip the mortgage
    /// flag and move `amount` money directly (规则书 基础规则 6: mortgage money
    /// is not affected by effects). Answers the amount actually credited.
    ApplyMortgage {
        player_id: i32,
        tile: i32,
        amount: i32,
    },
    /// Lift a mortgage the tile rule already approved: clear the flag and
    /// charge `amount` directly. Answers the amount actually charged.
    ApplyRedeem {
        player_id: i32,
        tile: i32,
        amount: i32,
    },
    /// Pay the buy price through the money pipeline (`Cx::money` with
    /// `PayKind::Buy`). `kind` is the [`card_sdk::abi::BuyKind`] wire value;
    /// `allow_mortgage` is the Land/Agent funding arm. Answers the amount
    /// actually paid (0 = refused / cancelled).
    PayBuy {
        player_id: i32,
        tile: i32,
        amount: i32,
        kind: i32,
    },
    /// Pay the build cost through the money pipeline (`PayKind::Build`).
    /// Answers the amount actually paid (0 = refused / cancelled).
    PayBuild {
        player_id: i32,
        tile: i32,
        amount: i32,
    },
    /// Write the deed after a buy (`purchase::assign_deed`). Mechanism only --
    /// the tile rule already decided owner / houses / mortgaged.
    AssignDeed {
        player_id: i32,
        tile: i32,
        owner: i32,
        houses: i32,
        mortgaged: i32,
    },
    /// Draw `n` cards. The engine runs the draw itself -- one card at
    /// a time, raising the per-draw points (`drewBefore` / `drawn` / `drew`) on
    /// each, so a `drewBefore` hook may replace a card of it -- and answers with
    /// how many were actually drawn. The effect then replays past this call and
    /// does **not** move the cards again (they are already in the live world).
    Draw { player_id: i32, n: i32 },
    /// Draw the top event and resolve it (「抽取一个事件卡」).
    /// The engine runs the whole event pipeline (public reveal, immediate
    /// effect, filing to the event discard, the reshuffle-when-empty half).
    /// `docs/TILES.md`'s `ctx::draw_event`, what `tile:event`'s body calls.
    DrawEvent { player_id: i32 },
    /// 「pay拥有格子的玩家格子地契所标记的现等级地租」 -- the rent
    /// pipeline (rent table / the flat deed dice / the agent's 「half-charge」 half-flag).
    /// `docs/TILES.md`'s `ctx::pay_rent`, what the deed tile rule / the flat-deed tile rule /
    /// the agent tile rule bodies call.
    PayRent {
        player_id: i32,
        tile: i32,
        half: bool,
    },
    /// A rent-shaped 「收费」 at `milli_scale` (1000 = ×1.0) of the tile's
    /// ordinary-settle rent, outside a full [触发结算] (`ctx::charge_rent`).
    /// 规则书 (`RAS:练习室里的风暴` (2)): 「对那个玩家进行一次相当于此卡所在格子
    /// 普通[结算]的(4-X)/4倍价格的收费」. Computes the same figure `pay_rent`
    /// does and runs it through the money pipeline as `PayKind::Rent`, scaled
    /// at `scale_settle_payment` -- no settle hooks, no buy/build offer.
    /// Answers the amount actually paid (0 = refused / cancelled).
    ChargeRent {
        /// The player charged.
        player_id: i32,
        tile: i32,
        /// Milli-units (500 = ×0.5); `RAS:练习室里的风暴` passes `250 * (4-X)`.
        milli_scale: i32,
    },
    /// 「可选择spend购买格子地契和建造已有房子的资金总价」 on a
    /// non-main landing on unowned land. `ctx::offer_buy`.
    OfferBuy { player_id: i32, tile: i32 },
    /// 「可选择pay…资金总价的两倍，从该玩家处强行购买」
    /// on a mortgaged deed. `ctx::offer_force_buy`.
    OfferForceBuy { player_id: i32, tile: i32 },
    /// 「可选择spend…房屋建筑费进行升级建造」 on a non-main
    /// landing on one's own land. `ctx::offer_build`.
    OfferBuildOne { player_id: i32, tile: i32 },
    /// A card- or skill-driven dice roll (`ctx::roll` / `ctx::roll_direct`).
    /// The engine rolls (Effect = `count`d`sides`, Move = the move plan's dice
    /// tables), logs one `"dice"` event, and -- when `window` -- raises the
    /// `Roll` chain link (the 「掷骰结算前」 counteraction window -- Y.O.L.O
    /// 「你的任意掷骰结算前」, 寄于指尖的执念 「当你使用火罐进行掷骰时」) with the
    /// roller, the face and `source`, then answers with the face any
    /// counteraction left on the link. `purpose` is a
    /// `card_sdk::abi::RollPurpose` wire code; `source` is a
    /// [`crate::abi::roll_source`] code (0 = unattributed, 1 = marker-funded, ...).
    /// `window: false` is `ctx::roll_direct` -- the face is final as cast.
    Roll {
        player_id: i32,
        count: i32,
        sides: i32,
        purpose: i32,
        source: i32,
        window: bool,
    },
    /// A card handed a deed over (tomoe_savior's 「从该玩家处收购该地契」) and
    /// is announcing the acquisition: the engine raises the `bought` hook chain
    /// over the field (`TriggerKind::Bought`), with `by_card` = the run's own
    /// player. `buy()` raises the same hook itself after an ordinary purchase;
    /// this is the card-driven half.
    RaiseBought { player_id: i32, tile: i32 },
    /// Raise a trigger point from a guest body (`ctx::raise`). `kind` is a
    /// [`TriggerKind`] as `i32` (mapped through `TriggerKind::as_str` for the
    /// wire name, e.g. `PassReward` → `"the pass-reward trigger"`); `value` is the
    /// trigger's `value`. Answers 1 when the link settled, 0 when a
    /// counteraction cancelled it.
    ///
    /// v78: when `kind` is a filter (`TriggerKind::is_filter`), the host folds
    /// the filter chain instead of opening a counteraction window. A
    /// `DealValue` kind (BuyAssign) carries the default deal in and answers
    /// the packed rewritten deal (see `hostfns::pack_deal` / `unpack_deal`):
    /// bit 0 = cancelled, bits 1-4 = houses, bit 5 = mortgaged, bits 6-31 =
    /// owner + 2 (0 = unset, 1 = bank / -1, 2+ = seat + 2).
    Raise {
        player_id: i32,
        /// Wire **spelling** (`TriggerKind::spelling`).
        kind: String,
        value: i32,
        /// The tile the raise is about (`t.tile`) -- filter pres like
        /// `tile.id == tile_named('…')` read it.
        tile: i32,
        /// BuyAssign filter deal (in): owner after (`i32::MIN` = unset).
        deal_owner: i32,
        /// BuyAssign filter deal (in): houses after.
        deal_houses: i32,
        /// BuyAssign filter deal (in): mortgaged after (0/1).
        deal_mortgaged: i32,
    },
    /// Which option `player_id`'s AI would take among `tiles` on an agent offer
    /// (`ctx::ai_agent_choice`). Answers the index; `tiles.len()` = skip.
    AiAgentChoice { player_id: i32, tiles: Vec<i32> },
    /// A bank gain with the Pay's event `typ` and a replacement log line
    /// (`Pay::text`) -- `ctx::gain_typed`. Builds `Pay::new(amount, "gain")` so
    /// the 「支付」 scalars (`scale_settle_payment`) do not reach a print.
    GainTyped {
        player_id: i32,
        amount: i32,
        typ: String,
        text: Option<crate::Msg>,
    },
    /// A **raw** money adjustment -- `ctx::adjust_money` / `ctx::gain_raw` /
    /// `ctx::lose_raw`. Positive `delta` is a gain, negative a loss. Moves the
    /// balance directly (`World::gain_money` / a raw decrement) with a
    /// `log.gain` / `log.lose` line, **without** the `HostRequest::Pay` money
    /// pipeline: no `paySplitAdd` / `paySplitMul` / `paySplitChoose` / `paySplitAt`, no `effect`
    /// counteraction window, no `payAfter`. 规则书 「不受其他任何效果影响」 wants the
    /// amount a designate split moves to land as-is (ONE OF US's 「被分享方资金
    /// 直接增加」). Answers the amount actually moved (0 = the seat was out).
    AdjustMoney {
        player_id: i32,
        delta: i32,
        src: Option<crate::Msg>,
    },
    /// Marker spend / gain (user ruling 2026-10-07): the engine raises
    /// `markerSpend` / `markerGain` -- the marker's own counteraction window --
    /// **before** the markers move. Answers `1` (the move goes through) or `0`
    /// (a counteraction cancelled it, nothing moves). Marker *costs* otherwise
    /// keep today's timing: they are spent as the effect resolves and a
    /// whole-effect negation before the body already prevents them.
    Marker {
        player_id: i32,
        /// Marker name (opaque data). The frozen webui key `charge` is one
        /// caller; named counters / kind hosts pass their own.
        name: String,
        /// Signed delta. Negative = spend (raises `markerSpend`), positive =
        /// gain (raises `markerGain`).
        delta: i32,
    },
}

#[derive(Debug, Clone)]
pub enum RuleError {
    /// A module is not a valid card module (bad wasm, missing export, ABI mismatch, bad manifest).
    Load(String),
    /// Two modules declare the same card id.
    DuplicateCard(String),
    /// Unknown card handle.
    NoSuchCard(i32),
    /// The effect trapped, panicked, ran out of fuel, or nested too deep.
    Trap(String),
    /// A guard (`can_counteract`) tried to prompt. Guards must be pure queries.
    GuardPrompted,
    /// A guard **condition** (docs/GUARDS.md §4.3) failed to compile: parse
    /// error, unknown variable/function, or a float literal (int-only). Fail
    /// closed at build -- never "treat as true"; the clauses the condition
    /// carries are gone from the guard.
    BadPre {
        card: String,
        entry: i32,
        source: String,
        err: String,
    },
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(m) => write!(f, "card module load error: {m}"),
            Self::DuplicateCard(id) => write!(f, "card {id:?} is declared by more than one module"),
            Self::NoSuchCard(c) => write!(f, "no card with handle {c}"),
            Self::Trap(m) => write!(f, "card effect failed: {m}"),
            Self::GuardPrompted => write!(f, "can_counteract tried to prompt a player"),
            Self::BadPre {
                card,
                entry,
                source,
                err,
            } => write!(
                f,
                "card {card:?} entry {entry}: bad condition {source:?}: {err}"
            ),
        }
    }
}

impl std::error::Error for RuleError {}

/// Result of one effect run.
#[derive(Debug)]
pub enum Outcome<W> {
    /// Finished: commit this world.
    Done(W),
    /// Blocked on a prompt with no answer yet. Nothing was committed. The run's
    /// world rides along (when the backend has it) so the drive can recover the
    /// write-through lines the pass already posted: the re-run replaces them in
    /// place instead of appending a second copy.
    NeedInput(Prompt, Option<W>),
    /// Blocked on a host routine (buy/build/mortgage/move/pay/...) with no
    /// answer yet. The run's world rides along so the host can read the
    /// turn-ctx policy the card set up (`build_discount`, `lingering`, ...)
    /// before the routine runs against the live world -- the routine is not
    /// replayed, so those writes have to cross now.
    NeedHost(HostRequest, W),
}

/// One card's answer to a hook point, from [`Ruleset::run_hook`].
pub struct HookRun<W> {
    /// The entry's condition + residual guard admitted -- the one moment the
    /// card shows itself. `false` only when nothing ran at all.
    pub announced: bool,
    /// What the body did. The body ran on the same instantiation as the guard.
    pub outcome: Outcome<W>,
}

/// What a bare [`Ruleset::declare_entry`] run collected, before the point of
/// no return (docs/ACTIVATION.md §2).
pub enum DeclareOut {
    /// Proceed; the body receives these `postcard` bytes as its activation
    /// argument (docs/ACTIVATION.md §2).
    Arg(Vec<u8>),
    /// The player declined: the entry does not fire at all.
    Declined,
    /// A prompt is pending -- the caller pauses and re-runs with the answer.
    Paused(Prompt),
}

/// One entry's applicability verdict, from [`Ruleset::entry_admitted`].
///
/// Carries the shared **answer-log cursor** across `admits` + `OP_DECLARE` in
/// one drive: `entry_admitted` returns where the next read must start, and the
/// `OP_RUN` that follows picks up from there. A separate `HostState` per phase
/// would re-read answer 0 (the latent bug the guard-prompt session hit).
/// Per-drive, not global: `finish_offer` and `resolve_jobs` are separate calls
/// and each starts at 0 over its own `answers` slice (docs/ACTIVATION.md §2).
pub enum Admitted<W> {
    /// The entry applies. `next_answer` is the cursor **after** `admits` and
    /// `OP_DECLARE`; `arg` is the activation argument [`OP_DECLARE`] collected
    /// (0 when the entry has no declare).
    Yes { next_answer: usize, arg: Vec<u8> },
    /// The entry does not apply (condition / guard / a declined declare).
    /// `next_answer` is the cursor after whatever prompts ran -- a decline
    /// that *came from a prompt* has consumed its answer-log slot and must not
    /// be dropped, or a replay would re-prompt the player.
    No { next_answer: usize },
    /// A prompt / host request is pending. The first pause owns the outcome.
    Paused(Outcome<W>),
}

#[cfg(feature = "sandbox")]
struct LoadedModule {
    module: Module,
    sha256: String,
}

#[cfg(feature = "sandbox")]
/// Where a card lives.
#[derive(Clone, Copy)]
struct Slot {
    module: usize,
    /// Handle inside that module's manifest.
    local: i32,
}

#[cfg(feature = "sandbox")]
pub(crate) struct Inner {
    /// Unique across the process. Caches keyed on a set of modules must not
    /// survive one `Inner` into another that lands at the same address, so they
    /// key on this rather than on a pointer.
    id: u64,
    engine: Engine,
    modules: Vec<LoadedModule>,
    cards: Vec<CardInfo>,
    /// Compiled guard **conditions** (docs/GUARDS.md G0), parallel to
    /// `cards[i].on[j]`. `None` = no condition (the guard alone decides).
    /// Built once in [`RulesetBuilder::build`]; every guard call site reads
    /// this through [`crate::cond_pre::admits`].
    pre: Vec<Vec<Option<crate::cond_pre::CompiledPre>>>,
    slots: Vec<Slot>,
    by_id: HashMap<String, i32>,
    sha256: String,
    /// Bitset of every trigger kind any card declares, so `counteract` can skip the
    /// whole bridge when nothing in the set listens. Engine ids and ruleset ids
    /// each get their own `u128` word (ids fit in `0..128`).
    declared: KindMask,
    /// Cached counteraction index (BOT-RESEARCH.md #1 / GUARDS.md G3): per-card
    /// bitmask of trigger kinds its `On::Counteract` entries answer. The offer
    /// loop tests one shift instead of scanning the entry table per probe.
    counteract_mask: Vec<KindMask>,
    /// Per-card bitmask of trigger kinds its hook/gate entries answer. Same
    /// shape as `counteract_mask`; the hook dispatch uses it to skip cards that
    /// can never run at a kind.
    hook_mask: Vec<KindMask>,
    /// Roster metadata (ABI v63) gathered from every module's
    /// `ruleset_metadata`. Empty on pre-v63 / meta-less sets.
    metadata: card_sdk::meta::MetaDoc,
    /// The CEL-only gates (`TriggerKind::{ActGate, PayGate, MoveGate,
    /// ExclusiveUse}`): per gate kind, the `(card, entry)` pairs whose
    /// condition closes it. Built once at load so a gate read walks only these.
    gates: [Vec<(i32, i32)>; GATE_KINDS.len()],
}

/// Two-namespace kind mask. Engine ids use `1u128 << id` in `engine` (and a
/// kind outside `0..128` is a load error -- never silently dropped). Ruleset
/// kinds are opaque tokens: an insertion-ordered set of [`TriggerKind`]s, no
/// id-as-bit and no range check.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KindMask {
    /// [`card_sdk::abi::KindNs::Engine`] ids.
    pub engine: u128,
    /// [`card_sdk::abi::KindNs::Ruleset`] kinds, insertion order, de-duplicated.
    ruleset: Vec<crate::TriggerKind>,
}

impl KindMask {
    /// Record `k`. Engine ids outside `0..128` are a load error; ruleset kinds
    /// have no range check (they are opaque).
    pub fn insert(&mut self, k: crate::TriggerKind) -> Result<(), String> {
        use card_sdk::abi::KindNs;
        match k.ns {
            KindNs::Engine => {
                if !(0..128).contains(&k.id) {
                    return Err(format!(
                        "trigger kind {} (id {}) outside 0..128 mask range",
                        k.spelling(),
                        k.id
                    ));
                }
                self.engine |= 1u128 << k.id;
            }
            KindNs::Ruleset(_) => {
                if !self.ruleset.contains(&k) {
                    self.ruleset.push(k);
                }
            }
        }
        Ok(())
    }

    /// Does this mask contain `k`?
    pub fn contains(&self, k: crate::TriggerKind) -> bool {
        use card_sdk::abi::KindNs;
        match k.ns {
            KindNs::Engine => (0..128).contains(&k.id) && (self.engine & (1u128 << k.id)) != 0,
            KindNs::Ruleset(_) => self.ruleset.contains(&k),
        }
    }
}

/// The gate kinds answered natively from conditions alone (see
/// [`gate_open_over`]). Index = position in this array.
pub const GATE_KINDS: [crate::TriggerKind; 7] = [
    crate::TriggerKind::ActGate,
    crate::TriggerKind::PayGate,
    crate::TriggerKind::MoveGate,
    crate::TriggerKind::OnBoardGate,
    crate::TriggerKind::TargetableGate,
    crate::TriggerKind::ExclusiveUse,
    crate::TriggerKind::PlayGate,
];

fn gate_index(kind: crate::TriggerKind) -> Option<usize> {
    GATE_KINDS.iter().position(|&k| k == kind)
}

/// `(card, entry)` of every gate entry answering `kind`, found by scanning the
/// manifest. The uncached form of [`Inner::gates`].
pub fn gate_entries_scan(cards: &[CardInfo], kind: crate::TriggerKind) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for (ci, c) in cards.iter().enumerate() {
        for (ei, o) in c.on.iter().enumerate() {
            if o.kind == OnKind::Filter as i32 && o.triggers.iter().any(|s| s == &kind.spelling()) {
                out.push((ci as i32, ei as i32));
            }
        }
    }
    out
}

/// A capability gate entry is **CEL-only**: its condition is the whole answer, so
/// the host can evaluate it natively in a pure read. A residual wasm guard (or
/// a missing condition) would be invisible to that read -- refuse at load.
pub fn validate_cel_only_gates(cards: &[CardInfo]) -> Result<(), RuleError> {
    for c in cards {
        for (ei, o) in c.on.iter().enumerate() {
            if o.kind != OnKind::Filter as i32 {
                continue;
            }
            if !o
                .triggers
                .iter()
                .any(|t| GATE_KINDS.iter().any(|&k| &*k.spelling() == t.as_str()))
            {
                continue;
            }
            if o.has_guard || o.pre.is_none() {
                return Err(RuleError::Load(format!(
                    "{}/{ei}: an ActGate/PayGate/MoveGate/ExclusiveUse entry must be CEL-only (a condition, no residual guard)",
                    c.id
                )));
            }
        }
    }
    Ok(())
}

/// Is the gate open? Closed when any entry's condition holds against `win`
/// (`actor` = the asked player). Pure and native: no wasm, no mutation.
pub fn gate_open_over<'a>(
    entries: impl IntoIterator<Item = (i32, i32)>,
    pre: impl Fn(i32, i32) -> Option<&'a crate::cond_pre::CompiledPre>,
    win: &rules_cond::WindowCtx,
) -> bool {
    let mut scope = None;
    for (c, e) in entries {
        let Some(p) = pre(c, e) else { continue };
        let sc = scope.get_or_insert_with(|| crate::cond_pre::window_scope(win));
        let cand = rules_cond::CandidateCtx {
            owner: win.actor,
            ..Default::default()
        };
        if crate::cond_pre::condition_allows(Some(p), Some(sc), &cand) {
            return false;
        }
    }
    true
}

#[cfg(feature = "sandbox")]
/// Builds a [`Ruleset`] from any number of card modules (normally one per card).
pub struct RulesetBuilder {
    engine: Engine,
    modules: Vec<LoadedModule>,
    cards: Vec<CardInfo>,
    slots: Vec<Slot>,
    /// Roster metadata rows gathered from every module's `ruleset_metadata`
    /// (ABI v63). Merged in [`RulesetBuilder::build`].
    metadata: Vec<card_sdk::meta::MetaRow>,
    /// Lean compiled condition blobs (`Cond::to_bytes(false)`), keyed by
    /// `(card id, entry index)`. The runtime-only (browser) path fills these
    /// from the ruleset index instead of compiling -- see
    /// [`RulesetBuilder::precompiled`].
    precompiled: HashMap<(String, i32), Vec<u8>>,
}

#[cfg(feature = "sandbox")]
impl RulesetBuilder {
    /// Compile and validate one card module (ABI version, exports, manifest) and add
    /// its cards. Returns the module's SHA-256.
    pub fn add(&mut self, wasm: &[u8]) -> Result<String, RuleError> {
        self.add_inner(wasm, None)
    }

    /// [`Self::add`] with the module's lowercase-hex SHA-256 supplied by the
    /// caller, who computed it over exactly `wasm` (the browser has a native
    /// `crypto.subtle.digest`; hashing 0.5 MB in wasm cost ~30 ms of main
    /// thread). The hash names the module in the ruleset identity, so a wrong
    /// one would mislabel the set -- only pass a digest you just computed.
    pub fn add_hashed(&mut self, wasm: &[u8], sha256: String) -> Result<String, RuleError> {
        self.add_inner(wasm, Some(sha256))
    }

    fn add_inner(&mut self, wasm: &[u8], sha256: Option<String>) -> Result<String, RuleError> {
        let (module, cards, meta) = inspect(&self.engine, wasm)?;
        let sha256 = sha256.unwrap_or_else(|| hex_sha256(wasm));
        let m = self.modules.len();
        for (local, c) in cards.into_iter().enumerate() {
            self.slots.push(Slot {
                module: m,
                local: local as i32,
            });
            self.cards.push(c);
        }
        self.metadata.extend(meta.rows);
        self.modules.push(LoadedModule {
            module,
            sha256: sha256.clone(),
        });
        Ok(sha256)
    }

    /// Hand the builder a precompiled condition blob (`Cond::to_bytes(false)`)
    /// for `card`'s `entry`, for the runtime-only (browser) path where the
    /// `cel` parser is not linked. The host compile path ignores these and
    /// compiles the manifest's `pre` source itself.
    pub fn precompiled(&mut self, card: &str, entry: i32, blob: Vec<u8>) {
        self.precompiled.insert((card.to_string(), entry), blob);
    }

    pub fn build(self) -> Result<Ruleset, RuleError> {
        crate::cond_pre::install_vocab();
        let mut by_id = HashMap::new();
        for (i, c) in self.cards.iter().enumerate() {
            if by_id.insert(c.id.clone(), i as i32).is_some() {
                return Err(RuleError::DuplicateCard(c.id.clone()));
            }
        }
        let mut declared = KindMask::default();
        let mut counteract_mask = Vec::with_capacity(self.cards.len());
        let mut hook_mask = Vec::with_capacity(self.cards.len());
        for c in &self.cards {
            let mut cm = KindMask::default();
            let mut hm = KindMask::default();
            // ABI v98 entry ids: stable internal slugs, identity not display
            // (docs/ACTIVATION.md §3). Unique per card.
            let mut seen_ids = std::collections::BTreeSet::new();
            for (ei, o) in c.on.iter().enumerate() {
                let ok = OnKind::from_i32(o.kind);
                // ABI v73: kind/variant agreement -- a Filter may only list
                // filter kinds and an Action only non-filter kinds.
                // Ruleset kinds are exempt: `is_filter` answers false for them,
                // so the host treats OnKind as authoritative (a later phase
                // emits a registration table).
                if matches!(ok, Some(OnKind::Action) | Some(OnKind::Filter)) {
                    let want_filter = ok == Some(OnKind::Filter);
                    for s in &o.triggers {
                        let tk = crate::TriggerKind::from_str(s);
                        if tk.ns == card_sdk::abi::KindNs::Engine && tk.is_filter() != want_filter {
                            return Err(RuleError::Load(format!(
                                "{} entry {ei}: {} lists trigger {} which is filter={} (see TriggerKind::is_filter)",
                                c.id,
                                if want_filter { "On::Filter" } else { "On::Action" },
                                tk.spelling(),
                                tk.is_filter(),
                            )));
                        }
                    }
                }
                // ABI v98: a filter body is a pure value fold and cannot pause
                // for a player (docs/ACTIVATION.md "On::Filter cannot declare").
                // `On::declare` panics at the card table's compile time; this
                // is the second net for a hand-built manifest.
                if ok == Some(OnKind::Filter) && o.has_declare {
                    return Err(RuleError::Load(format!(
                        "{} entry {ei}: On::Filter cannot declare -- use an On::Action pre-stage instead",
                        c.id,
                    )));
                }
                // ABI v102: a `OnKind::Schedule` entry must carry a non-empty
                // slug `id` -- the scheduling family names which entry to run.
                if ok == Some(OnKind::Schedule) && o.id.as_deref().filter(|s| !s.is_empty()).is_none()
                {
                    return Err(RuleError::Load(format!(
                        "{} entry {ei}: On::Schedule requires a non-empty id slug (the schedule call names it)",
                        c.id,
                    )));
                }
                // ABI v98: an entry id is a stable internal slug, never display
                // text (docs/ACTIVATION.md §3 "Identity is not display text").
                // `entry::PRESS` is reserved for the press entry and cannot be
                // declared; the rest must match `[A-Za-z0-9_][A-Za-z0-9_.:-]*`
                // and be unique per card.
                if let Some(id) = o.id.as_deref().filter(|s| !s.is_empty()) {
                    if id == card_sdk::abi::entry::PRESS {
                        return Err(RuleError::Load(format!(
                            "{} entry {ei}: id {id:?} is reserved for the press entry (entry::PRESS)",
                            c.id,
                        )));
                    }
                    if !is_entry_slug(id) {
                        return Err(RuleError::Load(format!(
                            "{} entry {ei}: id {id:?} is not an ASCII slug ([A-Za-z0-9_][A-Za-z0-9_.:-]*) -- identity, not display text (use `label`)",
                            c.id,
                        )));
                    }
                    if !seen_ids.insert(String::from(id)) {
                        return Err(RuleError::Load(format!(
                            "{} entry {ei}: duplicate entry id {id:?}",
                            c.id,
                        )));
                    }
                }
                for s in &o.triggers {
                    let tk = crate::TriggerKind::from_str(s);
                    declared.insert(tk).map_err(RuleError::Load)?;
                    match ok {
                        Some(OnKind::Counteract) => {
                            cm.insert(tk).map_err(RuleError::Load)?;
                        }
                        Some(OnKind::Action) | Some(OnKind::Filter) => {
                            hm.insert(tk).map_err(RuleError::Load)?;
                        }
                        _ => {}
                    }
                }
            }
            counteract_mask.push(cm);
            hook_mask.push(hm);
        }
        validate_cel_only_gates(&self.cards)?;
        let gates = GATE_KINDS.map(|k| gate_entries_scan(&self.cards, k));
        // Compile every condition once (docs/GUARDS.md §4.3). Fail-closed: a
        // parse / unknown-var / float error is a build error, never "treat as
        // true". On the runtime-only (browser) path a source `pre` without a
        // precompiled blob is a build error for the same reason. A precompiled
        // blob maps 1:1 onto a declared `pre` -- a blob for an entry that
        // declares no condition (or does not exist) is a mismatched blob and
        // refused, so a stale `conds-*.bin` cannot ride along silently.
        let mut pre = Vec::with_capacity(self.cards.len());
        let mut used_precompiled = 0usize;
        for c in &self.cards {
            let mut row = Vec::with_capacity(c.on.len());
            for (ei, o) in c.on.iter().enumerate() {
                let supplied = self.precompiled.get(&(c.id.clone(), ei as i32));
                let compiled = match &o.pre {
                    Some(src) => {
                        // The only kind of entry a blob is consumed by.
                        if supplied.is_some() {
                            used_precompiled += 1;
                        }
                        Some(compile_pre(&c.id, ei as i32, src, supplied)?)
                    }
                    None => None,
                };
                row.push(compiled);
            }
            pre.push(row);
        }
        if used_precompiled != self.precompiled.len() {
            let mut stale: Vec<String> = self
                .precompiled
                .keys()
                .filter(|(card, ei)| {
                    !by_id
                        .get(card)
                        .and_then(|&ci| self.cards.get(ci as usize))
                        .and_then(|c| c.on.get(*ei as usize))
                        .map_or(false, |o| o.pre.is_some())
                })
                .map(|(card, ei)| format!("{card}/{ei}"))
                .collect();
            stale.sort();
            return Err(RuleError::Load(format!(
                "precompiled conds blob lists entries with no condition in the modules: {}",
                stale.join(", ")
            )));
        }
        // Order-independent identity of the whole set, and of the compiled
        // conditions (docs/GUARDS.md §8.2): a change in any lean `Cond` blob
        // must change the ruleset sha, so record stamps / bundle ids follow.
        // Sets without conditions keep the plain module-hash recipe.
        let mut parts: Vec<String> = self.modules.iter().map(|m| m.sha256.clone()).collect();
        parts.sort_unstable();
        let conds = crate::cond_pre::PrecompiledConds::collect(&self.cards, &pre);
        let cond_lines = conds.identity_lines();
        if !cond_lines.is_empty() {
            parts.push("conds".to_string());
            parts.extend(cond_lines);
        }
        let sha256 = hex_sha256(parts.join("\n").as_bytes());
        // Keep the metadata rows sorted the way the export produced them
        // (`kind`, `ordinal`) so a multi-module merge remains deterministic.
        let mut metadata = self.metadata;
        metadata.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then(a.ordinal.cmp(&b.ordinal))
                .then(a.id.cmp(&b.id))
        });
        Ok(Ruleset {
            inner: Arc::new(Inner {
                id: next_inner_id(),
                engine: self.engine,
                modules: self.modules,
                cards: self.cards,
                pre,
                slots: self.slots,
                by_id,
                sha256,
                declared,
                counteract_mask,
                hook_mask,
                gates,
                metadata: card_sdk::meta::MetaDoc { rows: metadata },
            }),
            fuel: DEFAULT_FUEL,
        })
    }
}

#[cfg(feature = "sandbox")]
/// Compile one condition source (host `compile` path) or load a precompiled
/// blob (runtime-only path). Both end at the same `CompiledPre`, so the
/// browser evaluates exactly what the host compiled.
///
/// Native hosts compile the source (authoritative). When a blob is supplied
/// alongside it (the shipped `conds-*.bin`, or a test), it must be byte-equal
/// to the host compile -- that is the sha check that a shipped blob agrees
/// with what this build would have produced.
fn compile_pre(
    card: &str,
    entry: i32,
    src: &str,
    precompiled: Option<&Vec<u8>>,
) -> Result<crate::cond_pre::CompiledPre, RuleError> {
    let bad = |err: String| RuleError::BadPre {
        card: card.to_string(),
        entry,
        source: src.to_string(),
        err,
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let compiled =
            crate::cond_pre::CompiledPre::compile(src).map_err(|e| bad(e.to_string()))?;
        if let Some(blob) = precompiled {
            if compiled.blob != *blob {
                return Err(bad(
                    "precompiled blob does not match the host compile of this condition source"
                        .into(),
                ));
            }
        }
        Ok(compiled)
    }
    #[cfg(target_arch = "wasm32")]
    {
        // No parser in the browser build (docs/GUARDS.md §8.3): the compiled
        // form must arrive via `RulesetBuilder::precompiled`.
        match precompiled {
            Some(blob) => {
                crate::cond_pre::CompiledPre::from_bytes(blob).map_err(|e| bad(e.to_string()))
            }
            None => Err(bad(
                "browser build cannot compile a condition source; supply a precompiled blob \
                 (RulesetBuilder::precompiled) -- see docs/GUARDS.md §8.2"
                    .into(),
            )),
        }
    }
}

/// The buy-quote hook kinds (`docs/PURCHASE.md`): the gate that may cancel a
/// buy, plus the three price stages (`BuyAdd` → `BuyMul` → `BuySet`).
/// [`Ruleset::declares_buy`] ORs these against the manifest's declared-kind
/// bitmask (fix C).
pub const BUY_HOOK_KINDS: [crate::TriggerKind; 4] = [
    crate::TriggerKind::BuyGate,
    crate::TriggerKind::BuyAdd,
    crate::TriggerKind::BuyMul,
    crate::TriggerKind::BuySet,
];

#[cfg(feature = "sandbox")]
/// A loaded, validated set of card modules.
#[derive(Clone)]
pub struct Ruleset {
    inner: Arc<Inner>,
    fuel: u64,
}

#[cfg(feature = "sandbox")]
impl Ruleset {
    pub fn builder() -> RulesetBuilder {
        RulesetBuilder {
            engine: be::new_engine(),
            modules: vec![],
            cards: vec![],
            slots: vec![],
            metadata: vec![],
            precompiled: HashMap::new(),
        }
    }

    /// Convenience: a ruleset made of exactly one module.
    pub fn load(wasm: &[u8]) -> Result<Self, RuleError> {
        let mut b = Self::builder();
        b.add(wasm)?;
        b.build()
    }

    /// Load every module listed in `<dir>/index.json` (tools/build-ruleset.sh)
    /// without binding to a [`game_core::data::GameData`]. `Ok(None)` when the
    /// directory or index is missing -- the caller falls back to no card
    /// effects. Hosts use this to take [`Self::roster`] *before* loading the
    /// game tables (the tables' roster comes from the ruleset).
    pub fn load_dir(dir: &std::path::Path) -> Result<Option<Self>, RuleError> {
        let index = dir.join("index.json");
        let Ok(text) = std::fs::read_to_string(&index) else {
            return Ok(None);
        };
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| RuleError::Load(format!("bad index: {e}")))?;
        let mut builder = Self::builder();
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
        Ok(Some(builder.build()?))
    }

    /// Override the per-run fuel limit.
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    /// Hex SHA-256 identifying the whole set (independent of load order). Server
    /// and clients compare this to make sure they run the same rules.
    pub fn sha256(&self) -> &str {
        &self.inner.sha256
    }

    /// Hex SHA-256 of the module that implements `card`.
    pub fn module_sha256(&self, card: i32) -> Option<&str> {
        let slot = self.inner.slots.get(card as usize)?;
        Some(&self.inner.modules[slot.module].sha256)
    }

    pub fn module_count(&self) -> usize {
        self.inner.modules.len()
    }

    pub fn cards(&self) -> &[CardInfo] {
        &self.inner.cards
    }

    /// Roster metadata (ABI v63): every card / persona / group row the
    /// modules declared via `CardDef::meta` / the skill crates' tables.
    /// Sorted by `(kind, ordinal)` -- the data-file order. Convert with
    /// [`crate::roster::roster_from_meta`] to build a `GameData`.
    pub fn metadata(&self) -> &card_sdk::meta::MetaDoc {
        &self.inner.metadata
    }

    /// The same rows as [`Self::metadata`], as the game's `Roster` (ready for
    /// [`game_core::data::GameData::load`]).
    pub fn roster(&self) -> game_core::data::Roster {
        crate::roster::roster_from_meta(self.metadata())
    }

    /// Does any card in the set declare an entry at this kind? The cheap
    /// whole-set question that lets `counteract` skip building the bridge trigger at
    /// all when the answer is no -- with `StubRules`, or at a kind nothing
    /// listens to, that is every raise.
    pub fn declares(&self, kind: crate::TriggerKind) -> bool {
        self.inner.declared.contains(kind)
    }

    /// Does the **manifest** declare any buy hook at all (fix C,
    /// `docs/PURCHASE.md`)? One shift of the whole-set `declared` bitmask per
    /// buy kind -- the quote skips its live-instance walk, the `Run` and every
    /// `pure_buy_hook` when no card in the set can answer a buy hook, the same
    /// way `declares` lets `counteract` skip a raise nothing listens to.
    pub fn declares_buy(&self) -> bool {
        BUY_HOOK_KINDS.iter().any(|&k| self.declares(k))
    }

    /// Does `card` declare a buy hook at `kind`? The per-card `hook_mask`
    /// shift (the counteract index's `counteracts_to` shape) instead of an
    /// entry-table scan.
    pub fn hooks_buy(&self, card: i32, kind: crate::TriggerKind) -> bool {
        self.hooks_to(card, kind)
    }

    /// Fix A cheap pre-filter: does this card's play gate vanish entirely
    /// (no `On::Play` entry, or G4-deleted gate with no condition)? Then the
    /// verdict is always "playable" and [`Self::cant_play`] would return
    /// `Ok(None)` without asking anything -- callers can skip the uid lookup,
    /// the `Run` and the CEL scope too.
    pub fn play_gate_vanishes(&self, card: i32) -> bool {
        let Some(info) = self.inner.cards.get(card as usize) else {
            return true;
        };
        let Some(entry) = info.entry(OnKind::Play, None) else {
            return true;
        };
        self.guard_is_none(card, entry) && self.pre_at(card, entry).is_none()
    }

    /// Does this rule declare an activatable `On::Play` entry (a skill button /
    /// card play)? Passive / hook-only rules answer `false`. Unlike
    /// [`Self::play_gate_vanishes`] this is only about the entry existing --
    /// a gated entry whose gate currently refuses still has one.
    pub fn has_play_entry(&self, card: i32) -> bool {
        self.inner
            .cards
            .get(card as usize)
            .is_some_and(|info| info.has_play())
    }

    /// Cached counteraction index (BOT-RESEARCH.md #1): does this card declare
    /// a counteraction at `kind`? One shift of the per-card bitmask -- the offer loop
    /// never scans the entry table and never builds a `Run` for a card that
    /// cannot answer.
    pub fn counteracts_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        self.inner
            .counteract_mask
            .get(card as usize)
            .is_some_and(|m| m.contains(kind))
    }

    /// Hook/gate twin of [`Self::counteracts_to`].
    pub fn hooks_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        self.inner
            .hook_mask
            .get(card as usize)
            .is_some_and(|m| m.contains(kind))
    }

    /// Handle for a card id from the roster, e.g. `"AG:Y.O.L.O"`.
    pub fn card(&self, id: &str) -> Option<i32> {
        self.inner.by_id.get(id).copied()
    }

    /// The card rule's declared static **properties** (`CardDef::props`), see
    /// [`CardInfo::props`]. Empty when the card declares none. A property of
    /// the card *rule*, never derived from rulebook prose.
    pub fn card_props(&self, id: &str) -> std::collections::BTreeMap<String, i32> {
        self.card(id)
            .and_then(|i| self.inner.cards.get(i as usize))
            .map_or_else(Default::default, |c| c.props.clone())
    }

    /// The card rule's declared **tile texts** (`On::texts`), see
    /// [`CardInfo::texts`]. Empty when the card declares none.
    pub fn card_texts(&self, id: &str) -> Vec<ManifestTileText> {
        self.card(id)
            .and_then(|i| self.inner.cards.get(i as usize))
            .map_or_else(Default::default, |c| c.texts.clone())
    }

    /// One declared property; `0` when the card does not declare it (the
    /// defined default for every key the engine reads).
    pub fn card_prop(&self, id: &str, key: &str) -> i32 {
        self.card(id)
            .and_then(|i| self.inner.cards.get(i as usize))
            .and_then(|c| c.props.get(key).copied())
            .unwrap_or(0)
    }

    /// The rule id that settles a board tile of this opaque kind (ABI v66).
    /// From `CardDef::governs` / `board_role == "tile"`; empty when none.
    pub fn tile_rule_id(&self, kind: &str) -> String {
        for c in &self.inner.cards {
            if c.board_role != "tile" {
                continue;
            }
            if c.governs.iter().any(|g| g == kind) {
                return c.id.clone();
            }
            // Default: a single kind derived from the id's `tile:` suffix.
            if c.governs.is_empty() {
                if let Some(k) = c.id.strip_prefix("tile:") {
                    if k == kind {
                        return c.id.clone();
                    }
                }
            }
        }
        String::new()
    }

    /// Standing **mark-owner** rule ids (`board_role == "mark"`).
    pub fn mark_rule_ids(&self) -> Vec<String> {
        self.inner
            .cards
            .iter()
            .filter(|c| c.board_role == "mark")
            .map(|c| c.id.clone())
            .collect()
    }

    /// Standing **status-host** rule ids (`board_role == "status"`).
    pub fn status_rule_ids(&self) -> Vec<String> {
        self.inner
            .cards
            .iter()
            .filter(|c| c.board_role == "status")
            .map(|c| c.id.clone())
            .collect()
    }

    /// Run an effect from `world` with the answers collected so far.
    ///
    /// `world` is never modified. On [`Outcome::Done`] the caller commits the returned
    /// world; on [`Outcome::NeedInput`] it publishes the prompt, and once the answer
    /// arrives calls again with the **same** `world` and `answers` + the new answer.
    ///
    /// `on_body` runs exactly when the effect **body** is entered -- the entry
    /// exists and `OP_RUN` is about to call it -- with the world copy the body
    /// will write into. The activation announcement (the client's card flash)
    /// goes there, so a drive with no entry, a guard reject or a probe is silent.
    ///
    /// `AtEnd` / `DiceRollPlan` / `Settle` entries carry their admission here
    /// (category → condition → guard → body, docs/GUARDS.md §4.3): a rejecting
    /// entry runs no body and so never flashes. `Play` / `Counteract` keep their
    /// own separate admission (the play gate / `can_counteract` at offer time).
    pub fn run<W: CardWorld>(
        &self,
        world: &W,
        call: Call,
        answers: &[i32],
        mut on_body: Option<&mut dyn FnMut(&mut W, i32)>,
    ) -> Result<Outcome<W>, RuleError> {
        let (card, player_id) = (call.card(), call.player_id());
        self.check(card)?;
        let info = &self.inner.cards[card as usize];
        let entry = match call {
            Call::Play { .. } => info.entry(OnKind::Play, None),
            // Effect-on-DESIGNATES-play (CROSS-TESTS C13): the offer scan may
            // have admitted an `Effect` entry on a `card` raise; resolve it too.
            Call::Counteract { .. } => self.counteract_entry(card, &world.trigger()),
            Call::Hook { kind, .. } => info.hook_entry(kind),
            // v75: AtEnd / DiceRollPlan are ordinary kind-keyed entries
            // (`On::Schedule` / `On::Filter(Filter::DiceRollPlan(…))`).
            // v102: the schedule call carries the resolved entry index.
            Call::AtEnd { entry, .. } => Some(entry),
            Call::DiceRollPlan { .. } => info.hook_entry(crate::TriggerKind::DiceRollPlan),
            Call::Settle { .. } => info.entry(OnKind::Settle, None),
            Call::Mortgage { .. } => info.entry(OnKind::Mortgage, None),
            Call::Redeem { .. } => info.entry(OnKind::Redeem, None),
            Call::Buy { .. } => info.entry(OnKind::Buy, None),
            Call::Build { .. } => info.entry(OnKind::Build, None),
        };
        let Some(entry) = entry else {
            return Ok(Outcome::Done(world.clone()));
        };
        // v49: AtEnd / DiceRollPlan / Settle answer "does this entry apply?" here.
        // (`Play`'s gate and `Counteract`'s `can_counteract` are their own
        // admission and are not re-checked at body time; their `arg` was
        // collected at the offer layer and rides the `Call` -- `resolve_jobs`
        // never re-enters `OP_DECLARE`.)
        let mut arg = match &call {
            Call::Play { arg, .. } | Call::Counteract { arg, .. } => arg.clone(),
            _ => Vec::new(),
        };
        let mut next_answer = 0;
        if matches!(
            call,
            Call::AtEnd { .. }
                | Call::DiceRollPlan { .. }
                | Call::Settle { .. }
                | Call::Mortgage { .. }
                | Call::Redeem { .. }
                | Call::Buy { .. }
                | Call::Build { .. }
        ) {
            let pre = self
                .inner
                .pre
                .get(card as usize)
                .and_then(|r| r.get(entry as usize))
                .and_then(|p| p.as_ref());
            let scope = if pre.is_some() {
                Some(crate::cond_pre::window_scope_owned(
                    crate::cond_pre::fill_window(world),
                ))
            } else {
                None
            };
            let cand =
                crate::cond_pre::fill_candidate(world, player_id, &info.id, world.is_placed() != 0);
            match self.entry_admitted(
                world,
                card,
                entry,
                player_id,
                scope.as_ref(),
                &cand,
                answers,
                0,
            )? {
                Admitted::No { .. } => return Ok(Outcome::Done(world.clone())),
                Admitted::Paused(o) => return Ok(o),
                Admitted::Yes {
                    next_answer: after,
                    arg: declared,
                } => {
                    next_answer = after;
                    arg = declared;
                }
            }
        }
        let mut store = self.store(world.clone(), answers, next_answer)?;
        if let Some(cb) = on_body.as_deref_mut() {
            cb(store.data_mut().w(), entry);
        }
        store.data_mut().declared = arg.clone();
        let res = call_card(
            &self.inner,
            &mut store,
            card,
            entry,
            export::OP_RUN,
            player_id,
        );
        finish(store, res)
    }

    /// Category → condition → guard → **declare** → body
    /// (docs/GUARDS.md §4.3, docs/ACTIVATION.md §2) for one entry: does
    /// `entry` admit against `world` / `player_id`, and with what activation
    /// argument? A rejecting condition skips the wasm guard entirely
    /// (`admits_pre`); a residual guard runs on a throwaway copy; the
    /// `OP_DECLARE` collector (if any) runs before the point of no return and
    /// may pause. Used by [`Self::run_hook`] (Hook + Gate) and [`Self::run`]
    /// (AtEnd / DiceRollPlan / Settle).
    ///
    /// One `next_answer` cursor crosses `admits` + `OP_DECLARE` on one
    /// `HostState` (see [`Admitted`]).
    fn entry_admitted<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        entry: i32,
        player_id: i32,
        scope: Option<&rules_cond::WindowScope>,
        cand: &crate::cond_pre::CandidateCtx,
        answers: &[i32],
        start_answer: usize,
    ) -> Result<Admitted<W>, RuleError> {
        let pre = self
            .inner
            .pre
            .get(card as usize)
            .and_then(|r| r.get(entry as usize))
            .and_then(|p| p.as_ref());
        if self.guard_is_none(card, entry) {
            return Ok(if crate::cond_pre::admits_pre(pre, scope, cand) {
                self.declare_after(world, card, entry, player_id, answers, start_answer)?
            } else {
                Admitted::No {
                    next_answer: start_answer,
                }
            });
        }
        // The guard's store is the only one this entry's *guard* sees; the
        // cursor (not the world) is what crosses. Keep it in an `Option` so a
        // pause can `finish` the store away while the `admits` closure still
        // needs to hand it back on the verdict paths.
        let mut store_for_guard = Some(self.store(world.clone(), answers, start_answer)?);
        // A pause is not a verdict: park the `Outcome` and short-circuit
        // `admits` with `Err`. A rejecting condition never reaches the closure,
        // so the cursor stays at `start_answer`.
        let mut paused = None;
        let asked: Result<bool, ()> = crate::cond_pre::admits(pre, scope, cand, || {
            let mut store = store_for_guard.take().expect("guard store present");
            match crate::inline::with_no_inline(|| {
                call_card(
                    &self.inner,
                    &mut store,
                    card,
                    entry,
                    export::OP_GUARD,
                    player_id,
                )
            }) {
                Ok(v) => {
                    store_for_guard = Some(store);
                    Ok(v != 0)
                }
                Err(e) if is_need_input(&e) => {
                    paused = Some(finish(store, Err(e)));
                    Err(())
                }
                Err(_) => {
                    // Historical dispatch leniency: a trapping guard declines
                    // the entry (probes propagate the trap instead -- see
                    // [`Self::can_counteract_scoped`]). Keep the store so the
                    // swallow check below still runs.
                    store_for_guard = Some(store);
                    Ok(false)
                }
            }
        });
        if let Some(p) = paused {
            return match p {
                Ok(o) => Ok(Admitted::Paused(o)),
                Err(e) => Err(e),
            };
        }
        let store = store_for_guard.take().expect("guard store present");
        let st = store.into_data();
        // `finish`'s boundary, applied to the guard too: a residual that
        // published a prompt and then returned a bool has swallowed an answer
        // nobody gave.
        if st.asked.is_some() || st.host_request.is_some() {
            return Err(RuleError::Trap(
                "card published a prompt and kept going".into(),
            ));
        }
        Ok(match asked {
            Ok(true) => {
                self.declare_after(world, card, entry, player_id, answers, st.next_answer)?
            }
            _ => Admitted::No {
                next_answer: st.next_answer,
            },
        })
    }

    /// The `OP_DECLARE` half of [`Self::entry_admitted`], run when the entry
    /// already admitted. Shares the caller's cursor: `start_answer` is where
    /// `admits` left off, and the returned [`Admitted`] carries it forward.
    ///
    /// A decline (`EXIT_DECLINED`) is [`Admitted::No`]. A decline that came
    /// from a prompt has already advanced `next_answer` -- that slot is part
    /// of the drive's log and must be kept, or a replay would re-ask.
    fn declare_after<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        entry: i32,
        player_id: i32,
        answers: &[i32],
        start_answer: usize,
    ) -> Result<Admitted<W>, RuleError> {
        if !self.has_declare(card, entry) {
            return Ok(Admitted::Yes {
                next_answer: start_answer,
                arg: Vec::new(),
            });
        }
        let mut store = self.store(world.clone(), answers, start_answer)?;
        let res = crate::inline::with_no_inline(|| {
            call_card_declare(&self.inner, &mut store, card, entry, player_id)
        });
        let after = store.data().next_answer;
        match res {
            // Decline: the entry does not fire. The cursor has already
            // advanced past any prompt the collector made.
            Ok((code, _bytes)) if code == abi::EXIT_DECLINED as i64 => {
                let _ = finish(store, Ok(0))?;
                Ok(Admitted::No {
                    next_answer: after,
                })
            }
            Ok((_code, bytes)) => {
                let _ = finish(store, Ok(0))?;
                Ok(Admitted::Yes {
                    next_answer: after,
                    arg: bytes,
                })
            }
            Err(e) => match finish(store, Err(e)) {
                Ok(o) => Ok(Admitted::Paused(o)),
                // A trapping `OP_DECLARE` never commits and never costs the
                // card (docs/ACTIVATION.md §2): the collector runs *before*
                // the point of no return, so there is nothing to graveyard
                // and the card is still in hand / still in play. Propagating
                // here would fall into the body-trap arm (`log.card_trap` +
                // `DEST_GRAVEYARD`) and destroy a card whose body never ran.
                // Matches the residual guard's own leniency a few lines up.
                Err(_e2) => Ok(Admitted::No {
                    next_answer: after,
                }),
            },
        }
    }

    /// Run just the `OP_DECLARE` collector of one entry, before the activation
    /// commits (docs/ACTIVATION.md §2). `entry` is the manifest index the
    /// caller resolved: `OnKind::Play` for a play / skill press,
    /// `OnKind::Counteract` at the trigger for a [反击] declaration. `Ok(None)`
    /// = declined; `Ok(Some(arg))` = proceed with `arg`; a pause is folded to
    /// [`DeclareOut::Paused`].
    ///
    /// **Standalone**: it starts its own cursor at 0 over `answers` and does
    /// not inherit one from a surrounding drive. Callers that loop on a
    /// multi-step declare own the vec and push each pause's answer
    /// (`declare_for_card` / `declare_activation`); do not call this mid-drive
    /// expecting to continue an existing `next_answer`.
    pub fn declare_entry<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        entry: i32,
        player_id: i32,
        answers: &[i32],
    ) -> Result<DeclareOut, RuleError> {
        if !self.has_declare(card, entry) {
            return Ok(DeclareOut::Arg(Vec::new()));
        }
        let mut store = self.store(world.clone(), answers, 0)?;
        let res = crate::inline::with_no_inline(|| {
            call_card_declare(&self.inner, &mut store, card, entry, player_id)
        });
        match res {
            Ok((code, bytes)) if code == abi::EXIT_DECLINED as i64 => {
                let _ = finish(store, Ok(0))?;
                Ok(DeclareOut::Declined)
            }
            Ok((code, bytes)) if code == abi::EXIT_NEED_INPUT as i64 => {
                let _ = finish(store, Ok(0))?;
                // `finish` already consumed the pause into `Outcome`; fall
                // through to the error arm shape by reporting a generic pause.
                Ok(DeclareOut::Arg(bytes))
            }
            Ok((_code, bytes)) => {
                let _ = finish(store, Ok(0))?;
                Ok(DeclareOut::Arg(bytes))
            }
            Err(e) => match finish(store, Err(e)) {
                Ok(Outcome::NeedInput(p, _)) => Ok(DeclareOut::Paused(p)),
                Ok(_) => Ok(DeclareOut::Arg(Vec::new())),
                Err(e2) => Err(e2),
            },
        }
    }
    fn has_declare(&self, card: i32, entry: i32) -> bool {
        self.inner
            .cards
            .get(card as usize)
            .and_then(|c| c.on.get(entry as usize))
            .map(|o| o.has_declare)
            .unwrap_or(false)
    }

    /// Ask one card's hook guards and run every admitted body -- each entry
    /// on the same instantiation sequence, in declaration order. A card may
    /// carry several independent effects on one timing (「初始1，上限2」 plus
    /// 「每回合开始时」); each entry has its own condition / guard and runs at
    /// most once per raise.
    ///
    /// Returns `Ok(None)` when no entry is admitted (condition rejected, guard
    /// refused, or the card has no entry at this kind). [`HookRun::announced`]
    /// is the "the condition + guard admitted" moment. `on_body` runs exactly
    /// when an effect **body** is entered (after that entry's condition and
    /// guard admitted). The first entry whose body pauses (prompt / host) owns
    /// the outcome; the drive's multi-pass loop re-runs this whole sequence
    /// with the new answers.
    ///
    /// `On::Gate` entries share this path with `On::Action` (v49): a gate whose
    /// condition rejects does not answer at all.
    pub fn run_hook<W: CardWorld>(
        &self,
        world: &W,
        call: Call,
        answers: &[i32],
        mut on_body: Option<&mut dyn FnMut(&mut W, i32)>,
    ) -> Result<Option<HookRun<W>>, RuleError> {
        let (card, player_id) = (call.card(), call.player_id());
        self.check(card)?;
        let kind = world.trigger().kind;
        let info = &self.inner.cards[card as usize];
        let entries = info.hook_entries(kind);
        if entries.is_empty() {
            return Ok(None);
        }
        // Built only if some entry actually declares a condition: the window
        // snapshot + CEL root are large, and an unconditioned hook must not
        // pay for them (STACK-01's small-thread settle chain has no room).
        let mut scope: Option<rules_cond::WindowScope> = None;
        // `card.placed` must answer for the **running instance** (the guest's
        // `ctx::is_placed()`): a game-start hook also runs for the same id
        // while it sits in a pile / hand (`uid = -1`).
        let cand =
            crate::cond_pre::fill_candidate(world, player_id, &info.id, world.is_placed() != 0);
        let mut cur: W = world.clone();
        // One `next_answer` cursor threads the whole call: every guard, every
        // `OP_DECLARE` and every body reads the shared answer log in order, so
        // a prompt in entry 1 is never re-read by entry 2 (see [`Admitted`]).
        let mut next_answer = 0;
        let mut announced = false;
        let mut ran_any = false;
        for entry in entries {
            // v49: `On::Action` and `On::Gate` both carry the condition /
            // residual-guard pair; a rejecting entry is skipped (and so never
            // flashes). The `OP_DECLARE` collector runs here too, before the
            // announce -- a decline is silent.
            let pre = self
                .inner
                .pre
                .get(card as usize)
                .and_then(|r| r.get(entry as usize))
                .and_then(|p| p.as_ref());
            if pre.is_some() && scope.is_none() {
                scope = Some(crate::cond_pre::window_scope_owned(
                    crate::cond_pre::fill_window(world),
                ));
            }
            let (arg, after_admit) = match self.entry_admitted(
                world,
                card,
                entry,
                player_id,
                scope.as_ref(),
                &cand,
                answers,
                next_answer,
            )? {
                // A guard / declare prompt is still applicability: the body
                // has not run and `on_body` has not fired, so `announced`
                // keeps whatever earlier entries set.
                Admitted::Paused(p) => {
                    return Ok(Some(HookRun {
                        announced,
                        outcome: p,
                    }));
                }
                Admitted::No {
                    next_answer: after,
                } => {
                    next_answer = after;
                    continue;
                }
                Admitted::Yes {
                    next_answer: after,
                    arg,
                } => (arg, after),
            };
            next_answer = after_admit;
            announced = true;
            let mut store = self.store(cur, answers, next_answer)?;
            if let Some(cb) = on_body.as_deref_mut() {
                cb(store.data_mut().w(), entry);
            }
            store.data_mut().declared = arg.clone();
            let res = call_card(
                &self.inner,
                &mut store,
                card,
                entry,
                export::OP_RUN,
                player_id,
            );
            let after_body = store.data().next_answer;
            let outcome = finish(store, res)?;
            ran_any = true;
            match outcome {
                Outcome::Done(w) => {
                    // Later entries see earlier writes and earlier answers.
                    cur = w;
                    next_answer = after_body;
                }
                paused => {
                    return Ok(Some(HookRun {
                        announced,
                        outcome: paused,
                    }));
                }
            }
        }
        if !ran_any {
            return Ok(None);
        }
        Ok(Some(HookRun {
            announced,
            outcome: Outcome::Done(cur),
        }))
    }

    /// The compiled condition of one guarded entry, if it declares one
    /// (docs/GUARDS.md G0). `None` = no condition.
    pub fn pre_of(
        &self,
        card: i32,
        kind: OnKind,
        trigger: Option<crate::TriggerKind>,
    ) -> Option<&crate::cond_pre::CompiledPre> {
        let c = self.inner.cards.get(card as usize)?;
        let e = c.entry(kind, trigger)?;
        self.inner.pre.get(card as usize)?.get(e as usize)?.as_ref()
    }

    /// The compiled condition of `cards[card].on[entry]`, if it declares one.
    pub fn pre_at(&self, card: i32, entry: i32) -> Option<&crate::cond_pre::CompiledPre> {
        self.inner
            .pre
            .get(card as usize)?
            .get(entry as usize)?
            .as_ref()
    }

    /// Every compiled guard condition of this set, keyed by `(card id, entry
    /// index)` and sorted -- the `conds-<sha>.bin` blob `tools/build-ruleset.mjs`
    /// ships for the browser glue (`docs/GUARDS.md` §8.2). The bytes are this
    /// host compile's lean `Cond::to_bytes(false)`.
    pub fn precompiled_conds(&self) -> crate::cond_pre::PrecompiledConds {
        crate::cond_pre::PrecompiledConds::collect(&self.inner.cards, &self.inner.pre)
    }

    /// `Card.CanCounteract` against the current trigger. Runs on a throwaway copy, so it
    /// cannot change the world even if the card calls a mutating function.
    ///
    /// Goes through [`crate::cond_pre::admits`] (docs/GUARDS.md §4.4): the
    /// entry's condition is evaluated first and a rejecting one skips the wasm
    /// guard entirely.
    pub fn can_counteract<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        player_id: i32,
    ) -> Result<bool, RuleError> {
        let scope = crate::cond_pre::window_scope_owned(crate::cond_pre::fill_window(world));
        self.can_counteract_scoped(world, card, player_id, &scope)
    }

    /// [`Self::can_counteract`] with a window scope built once per trigger /
    /// chain window and reused across every candidate probe in that window
    /// (docs/GUARDS.md §4.2/§4.4 item 1 -- the 48 900-call path).
    pub fn can_counteract_scoped<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        player_id: i32,
        scope: &rules_cond::WindowScope,
    ) -> Result<bool, RuleError> {
        self.check(card)?;
        // Only a card that declared a counteraction at this kind is ever asked.
        let Some(entry) = self.counteract_entry(card, &world.trigger()) else {
            return Ok(false);
        };
        let pre = self
            .inner
            .pre
            .get(card as usize)
            .and_then(|r| r.get(entry as usize))
            .and_then(|p| p.as_ref());
        let cand = crate::cond_pre::fill_candidate(
            world,
            player_id,
            &self.inner.cards[card as usize].id,
            false,
        );
        let ok = if self.guard_is_none(card, entry) {
            // G4 deleted the residual: the condition alone decides.
            crate::cond_pre::admits_pre(pre, Some(scope), &cand)
        } else {
            crate::cond_pre::admits(pre, Some(scope), &cand, || {
                let mut store = self.store(world.clone(), &[], 0)?;
                match crate::inline::with_no_inline(|| {
                    call_card(
                        &self.inner,
                        &mut store,
                        card,
                        entry,
                        export::OP_GUARD,
                        player_id,
                    )
                }) {
                    Ok(v) => Ok(v != 0),
                    Err(e) if is_need_input(&e) => Err(RuleError::GuardPrompted),
                    Err(e) => Err(trap(e)),
                }
            })?
        };
        Ok(ok)
    }

    /// Condition-only pre-filter (BOT-RESEARCH.md #1): evaluate the entry's
    /// condition for `(card, seat)` against a shared window scope **without**
    /// building a `Run` or instantiating the guard. `Some(entry)` when the
    /// condition admits (or the entry has none) and the residual guard must be
    /// asked; `None` when the card has no counteraction at this kind or the condition
    /// rejects it. The offer loop calls this first and only builds the `Run`
    /// for the survivors.
    pub fn counteract_pre_allows<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        player_id: i32,
        scope: &rules_cond::WindowScope,
    ) -> Option<i32> {
        let t = world.trigger();
        if !self.counteracts_to_play(card, &t) {
            return None;
        }
        let entry = self.counteract_entry(card, &t)?;
        let pre = self
            .inner
            .pre
            .get(card as usize)
            .and_then(|r| r.get(entry as usize))
            .and_then(|p| p.as_ref());
        let cand = crate::cond_pre::fill_candidate(
            world,
            player_id,
            &self.inner.cards[card as usize].id,
            false,
        );
        if crate::cond_pre::condition_allows(pre, Some(scope), &cand) {
            Some(entry)
        } else {
            None
        }
    }

    /// Is the residual wasm guard deleted (G4: `On::*` guard is `None`)? The
    /// host skips the `OP_GUARD` instantiation when the condition alone decides.
    fn guard_is_none(&self, card: i32, entry: i32) -> bool {
        self.inner
            .cards
            .get(card as usize)
            .and_then(|c| c.on.get(entry as usize))
            .is_some_and(|o| !o.has_guard)
    }

    /// `Card.WhyNot` -- the reason the card cannot be played now, or `None` when
    /// it can. Also a pure query on a throwaway copy (a prompting guard reports
    /// [`RuleError::GuardPrompted`]). This is the `Play` **gate**
    /// (`Option<fn(i32) -> Option<Msg>>`), so it runs under
    /// [`export::OP_GUARD`] -- the effect body does not run here.
    ///
    /// Goes through [`crate::cond_pre::admits_gate`] (docs/GUARDS.md §4.4 item
    /// 3): a rejecting condition **is** a block (`err.play_pre`).
    ///
    /// Play-mode selection (a card may declare several `On::Play` entries as
    /// alternative activations, each with its own gate): the first **gated**
    /// entry whose gate admits; failing that, the first ungated entry. `None`
    /// when the card has no Play entry. [`Self::run`] (Play) picks through
    /// here. [`Self::cant_play`] does **not** -- it evaluates only the first
    /// declared Play entry (see the note there: play-mode merges are a
    /// per-card problem, not a dispatch one), so on a card whose first Play
    /// gate is closed and whose second would admit, the two disagree until
    /// those merges land.
    fn play_entry_for<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        player_id: i32,
    ) -> Result<Option<i32>, RuleError> {
        let info = &self.inner.cards[card as usize];
        let mut first_play = None;
        let mut first_ungated = None;
        for entry in info.entry_iter(OnKind::Play, None) {
            first_play.get_or_insert(entry);
            if self.guard_is_none(card, entry) && self.pre_at(card, entry).is_none() {
                // Ungated entry: the fallback mode.
                first_ungated.get_or_insert(entry);
                continue;
            }
            // Gated entry: ask the gate.
            let pre = self.pre_at(card, entry);
            let scope;
            let scope: Option<&crate::cond_pre::WindowScope> = if pre.is_some() {
                scope = crate::cond_pre::window_scope_owned(crate::cond_pre::fill_window_ambient(
                    world, player_id,
                ));
                Some(&scope)
            } else {
                None
            };
            let cand = crate::cond_pre::fill_candidate(world, player_id, &info.id, false);
            let admits = if self.guard_is_none(card, entry) {
                crate::cond_pre::condition_allows(pre, scope, &cand)
            } else {
                // A `Play` gate answers `Option<Msg>`: `None` (= packed 0) is
                // "playable", `Some(why)` is the block. `call_card_msg` decodes
                // that shape -- `call_card`'s 0/non-zero bool convention is the
                // Hook-guard one and would invert the verdict.
                let mut store = self.store(world.clone(), &[], 0)?;
                let gate_ok = match crate::inline::with_no_inline(|| {
                    call_card_msg(
                        &self.inner,
                        &mut store,
                        card,
                        entry,
                        export::OP_GUARD,
                        player_id,
                    )
                }) {
                    Ok(None) => true,
                    Ok(Some(_)) => false,
                    Err(e) if is_need_input(&e) => return Err(RuleError::GuardPrompted),
                    Err(_) => false,
                };
                gate_ok && crate::cond_pre::condition_allows(pre, scope, &cand)
            };
            if admits {
                return Ok(Some(entry));
            }
        }
        Ok(first_ungated.or(first_play))
    }

    /// is skipped. The play-gate window has no `Trigger` (`kind` absent).
    pub fn cant_play<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        player_id: i32,
    ) -> Result<Option<crate::Msg>, RuleError> {
        self.check(card)?;
        // Play duplicates are a per-card merge problem (modes vs contexts),
        // not a dispatch one -- first entry until the merges land.
        let Some(entry) = self.inner.cards[card as usize].entry(OnKind::Play, None) else {
            return Ok(None);
        };
        let pre = self
            .inner
            .pre
            .get(card as usize)
            .and_then(|r| r.get(entry as usize))
            .and_then(|p| p.as_ref());
        // The CEL scope is only read when a condition exists -- with `pre ==
        // None` nothing evaluates it, so skip the build. That is the
        // `cant_play` hot path (ai_step / view extras ask it per hand card).
        let scope;
        let scope: Option<&crate::cond_pre::WindowScope> = if pre.is_some() {
            scope = crate::cond_pre::window_scope_owned(crate::cond_pre::fill_window_ambient(
                world, player_id,
            ));
            Some(&scope)
        } else {
            None
        };
        let cand = crate::cond_pre::fill_candidate(
            world,
            player_id,
            &self.inner.cards[card as usize].id,
            false,
        );
        let why = if self.guard_is_none(card, entry) {
            // G4 deleted the gate: the condition alone decides. A rejecting
            // condition is still a block; an admitting one is playable.
            if crate::cond_pre::condition_allows(pre, scope, &cand) {
                None
            } else {
                Some(crate::Msg::new("err.play_pre"))
            }
        } else {
            crate::cond_pre::admits_gate(
                pre,
                scope,
                &cand,
                || crate::Msg::new("err.play_pre"),
                || {
                    let mut store = self.store(world.clone(), &[], 0)?;
                    match crate::inline::with_no_inline(|| {
                        call_card_msg(
                            &self.inner,
                            &mut store,
                            card,
                            entry,
                            export::OP_GUARD,
                            player_id,
                        )
                    }) {
                        Ok(v) => Ok(v),
                        Err(e) if is_need_input(&e) => Err(RuleError::GuardPrompted),
                        Err(e) => Err(trap(e)),
                    }
                },
            )?
        };
        Ok(why)
    }

    fn check(&self, card: i32) -> Result<(), RuleError> {
        if card < 0 || card as usize >= self.inner.cards.len() {
            return Err(RuleError::NoSuchCard(card));
        }
        Ok(())
    }

    fn store<W: CardWorld>(
        &self,
        world: W,
        answers: &[i32],
        start_answer: usize,
    ) -> Result<Store<HostState<W>>, RuleError> {
        #[cfg(feature = "bot-cost")]
        let t0 = std::time::Instant::now();
        #[cfg(feature = "bot-cost")]
        bot_cost::HOST_WORLD_CLONES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut state = HostState::new(self.inner.clone(), world, answers.to_vec(), 0);
        // Resume the shared answer-log cursor where the previous guard /
        // declare / body left off (see [`Admitted`]). `HostState::new` always
        // starts at 0; only a nested `play_card` re-aligns it from its parent.
        state.next_answer = start_answer;
        let mut store = new_store(&self.inner.engine, state);
        store.set_fuel(self.fuel).map_err(trap)?;
        #[cfg(feature = "bot-cost")]
        bot_cost::STORE_NS.fetch_add(
            t0.elapsed().as_nanos() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        Ok(store)
    }
}

#[cfg(feature = "sandbox")]
fn trap(e: Error) -> RuleError {
    RuleError::Trap(be::error_text(&e))
}

/// Fold one guest call's result into an [`Outcome`] against the store it ran in.
#[cfg(feature = "sandbox")]
fn finish<W>(store: Store<HostState<W>>, res: Result<i64, Error>) -> Result<Outcome<W>, RuleError> {
    let state = store.into_data();
    match res {
        Ok(_) => {
            // A prompt was published and the run kept going: the guest
            // swallowed a `Prompt` (`card_sdk` marks it `#[must_use]`; this
            // is the boundary check the lint cannot reach). Carrying on would
            // run the effect on an answer nobody gave.
            if state.asked.is_some() || state.host_request.is_some() {
                return Err(RuleError::Trap(
                    "card published a prompt and kept going".into(),
                ));
            }
            Ok(Outcome::Done(
                state.world.expect("world is restored after nested calls"),
            ))
        }
        Err(e) if is_need_input(&e) => {
            if let Some(req) = state.host_request {
                let world = state.world.expect("world is restored after nested calls");
                Ok(Outcome::NeedHost(req, world))
            } else if let Some(p) = state.asked {
                Ok(Outcome::NeedInput(p, state.world))
            } else {
                // `Prompt` is a unit struct, so a card can fabricate one
                // without a host call having published a question. Refuse it
                // rather than pausing the match on a prompt that no client
                // can answer.
                Err(RuleError::Trap("need-input exit without a prompt".into()))
            }
        }
        Err(e) => Err(trap(e)),
    }
}

pub(crate) fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// ---------------------------------------------------------------------------
// `CardModules`: what the `CardRules` bridge needs from a loaded card set.
// `Ruleset` is the sandbox implementation; `rules-native` provides its own
// over the linked `RULESET` tables. See `docs/BOT.md` §3.1 (B1).
// ---------------------------------------------------------------------------

/// The loaded card set, in the shape [`crate::WasmRules`] (the `CardRules`
/// bridge) consumes. Split out of [`Ruleset`] so the native backend can
/// implement it over statically linked tables without a wasm engine.
pub trait CardModules: Clone + Send + Sync + 'static {
    fn cards(&self) -> &[CardInfo];
    fn card(&self, id: &str) -> Option<i32>;
    fn card_props(&self, id: &str) -> std::collections::BTreeMap<String, i32>;
    fn card_prop(&self, id: &str, key: &str) -> i32;
    /// Tile texts this card's entries speak (`On::texts`), in declaration
    /// order. Empty when the card declares none.
    fn card_texts(&self, id: &str) -> Vec<ManifestTileText>;
    fn declares(&self, kind: crate::TriggerKind) -> bool;
    /// Rule id that settles a board tile of this opaque kind (ABI v66).
    /// Default scans [`Self::cards`] for `board_role == "tile"`.
    fn tile_rule_id(&self, kind: &str) -> String {
        for c in self.cards() {
            if c.board_role != "tile" {
                continue;
            }
            if c.governs.iter().any(|g| g == kind) {
                return c.id.clone();
            }
            if c.governs.is_empty() {
                if let Some(k) = c.id.strip_prefix("tile:") {
                    if k == kind {
                        return c.id.clone();
                    }
                }
            }
        }
        String::new()
    }
    /// Standing mark-owner rule ids (`board_role == "mark"`).
    fn mark_rule_ids(&self) -> Vec<String> {
        self.cards()
            .iter()
            .filter(|c| c.board_role == "mark")
            .map(|c| c.id.clone())
            .collect()
    }
    /// Standing status-host rule ids (`board_role == "status"`).
    fn status_rule_ids(&self) -> Vec<String> {
        self.cards()
            .iter()
            .filter(|c| c.board_role == "status")
            .map(|c| c.id.clone())
            .collect()
    }
    /// Cached counteraction index (BOT-RESEARCH.md #1): does this card declare
    /// a counteraction at `kind`? Default scans the entry table; [`Ruleset`] uses the
    /// per-card kind bitmask.
    fn counteracts_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        self.cards()
            .get(card as usize)
            .is_some_and(|c| c.counteracts_to(kind))
    }
    /// The trigger's card declares recipients (`DESIGNATES`) -- those plays are
    /// one counteraction timing (CROSS-TESTS C13): their `Effect` counters also answer
    /// the play's own `card` raise.
    fn trigger_designates(&self, t: &crate::Trigger) -> bool {
        t.kind == crate::TriggerKind::Card
            && t.card
                .as_deref()
                .and_then(|c| self.card(c))
                .is_some_and(|idx| {
                    self.cards().get(idx as usize).is_some_and(|info| {
                        info.props
                            .get(game_core::state::prop::DESIGNATES)
                            .copied()
                            .unwrap_or(0)
                            != 0
                    })
                })
    }
    /// [`Self::counteracts_to`], plus the Effect-on-DESIGNATES-play rule.
    fn counteracts_to_play(&self, card: i32, t: &crate::Trigger) -> bool {
        self.counteracts_to(card, t.kind)
            || (self.trigger_designates(t) && self.counteracts_to(card, crate::TriggerKind::Effect))
    }
    /// The [`On::Counteract`] entry answering `t` (see
    /// [`Self::counteracts_to_play`]).
    fn counteract_entry(&self, card: i32, t: &crate::Trigger) -> Option<i32> {
        let info = self.cards().get(card as usize)?;
        info.entry(OnKind::Counteract, Some(t.kind)).or_else(|| {
            if self.trigger_designates(t) {
                info.entry(OnKind::Counteract, Some(crate::TriggerKind::Effect))
            } else {
                None
            }
        })
    }
    /// Hook/gate twin of [`Self::counteracts_to`] (fix C's buy-hook index).
    /// Default scans the entry table (`CardInfo::hooks`); [`Ruleset`] overrides
    /// with the per-card `hook_mask` shift.
    fn hooks_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        self.cards()
            .get(card as usize)
            .is_some_and(|c| c.hooks(kind))
    }
    /// Does the set declare any buy hook at all (fix C)? Default ORs
    /// [`Self::declares`] over [`BUY_HOOK_KINDS`]; [`Ruleset`] overrides with
    /// the whole-set declared bitmask.
    fn declares_buy(&self) -> bool {
        BUY_HOOK_KINDS.iter().any(|&k| self.declares(k))
    }
    /// A CEL-only capability gate (`ActGate` / `PayGate` / `MoveGate`): `true` =
    /// open. Default scans the manifest; [`Ruleset`] overrides with its
    /// load-time list.
    fn gate_open(&self, kind: crate::TriggerKind, win: &rules_cond::WindowCtx) -> bool {
        if !self.declares(kind) {
            return true;
        }
        gate_open_over(
            gate_entries_scan(self.cards(), kind),
            |c, e| self.pre(c, e),
            win,
        )
    }
    /// Condition pre-filter (BOT-RESEARCH.md #1): `Some(entry)` when the
    /// card's counteraction at this window's kind exists and its condition admits.
    /// `None` skips the `Run` / guard entirely. Default builds the candidate
    /// from the world and evaluates the condition; [`Ruleset`] overrides with
    /// the same shape.
    fn counteract_pre_allows(
        &self,
        world: &crate::Run,
        card: i32,
        player_id: i32,
        scope: &rules_cond::WindowScope,
    ) -> Option<i32> {
        let t = world.trigger();
        if !self.counteracts_to_play(card, &t) {
            return None;
        }
        let info = self.cards().get(card as usize)?;
        let entry = self.counteract_entry(card, &t)?;
        let pre = self.pre(card, entry);
        let cand = crate::cond_pre::fill_candidate(world, player_id, &info.id, false);
        if crate::cond_pre::condition_allows(pre, Some(scope), &cand) {
            Some(entry)
        } else {
            None
        }
    }
    /// The compiled condition of one guarded entry, if any (G0).
    fn pre(&self, card: i32, entry: i32) -> Option<&crate::cond_pre::CompiledPre>;
    /// G4: is the residual wasm guard deleted, leaving the condition alone to
    /// decide? `true` skips the guard instantiation entirely.
    /// Default `false` (conservative: assume a guard and run it). [`Ruleset`]
    /// overrides from its manifest.
    fn entry_guard_is_none(&self, _card: i32, _entry: i32) -> bool {
        false
    }
    /// Fix A cheap pre-filter: does this card's play gate vanish entirely?
    /// `true` when the card has no `On::Play` entry, or G4 deleted the gate
    /// and it declares no condition -- the verdict is always "playable" and
    /// the probe can skip the uid lookup, the `Run` and the CEL scope.
    /// Default `false` (conservative: run the check). [`Ruleset`] overrides
    /// from its manifest.
    fn play_gate_vanishes(&self, card: i32) -> bool {
        let _ = card;
        false
    }
    /// Does this rule declare an activatable `On::Play` entry (a skill button /
    /// card play)? Unlike [`Self::play_gate_vanishes`] this is only about the
    /// entry existing -- a gated entry whose gate currently refuses still has
    /// one. Passive / hook-only rules answer `false`.
    fn has_play_entry(&self, card: i32) -> bool {
        self.cards()
            .get(card as usize)
            .is_some_and(|c| c.has_play())
    }
    /// Content hash of the loaded set; `None` when there is no module image
    /// (the native build -- the cards are compiled in).
    fn sha256(&self) -> Option<&str>;
    fn module_sha256(&self, card: i32) -> Option<&str>;
    fn run(
        &self,
        world: &crate::Run,
        call: Call,
        answers: &[i32],
        on_body: Option<&mut dyn FnMut(&mut crate::Run, i32)>,
    ) -> Result<Outcome<crate::Run>, RuleError>;
    fn run_hook(
        &self,
        world: &crate::Run,
        call: Call,
        answers: &[i32],
        on_body: Option<&mut dyn FnMut(&mut crate::Run, i32)>,
    ) -> Result<Option<HookRun<crate::Run>>, RuleError>;
    fn can_counteract(
        &self,
        world: &crate::Run,
        card: i32,
        player_id: i32,
    ) -> Result<bool, RuleError>;
    /// [`Self::can_counteract`] with a window scope built once per trigger /
    /// chain window and reused across every candidate probe (docs/GUARDS.md
    /// §4.4 item 1). Default falls back to the unscoped form.
    fn can_counteract_scoped(
        &self,
        world: &crate::Run,
        card: i32,
        player_id: i32,
        scope: &rules_cond::WindowScope,
    ) -> Result<bool, RuleError> {
        let _ = scope;
        self.can_counteract(world, card, player_id)
    }
    /// The `Play` gate (`Card.WhyNot`): `Some(reason)` when the card is blocked.
    fn cant_play(
        &self,
        world: &crate::Run,
        card: i32,
        player_id: i32,
    ) -> Result<Option<crate::Msg>, RuleError>;

    /// Run just the `OP_DECLARE` collector of one entry, before the activation
    /// commits (docs/ACTIVATION.md §2). `entry` is the manifest index the
    /// caller resolved. See [`DeclareOut`].
    fn declare_entry(
        &self,
        world: &crate::Run,
        card: i32,
        entry: i32,
        player_id: i32,
        answers: &[i32],
    ) -> Result<DeclareOut, RuleError>;
}

#[cfg(feature = "sandbox")]
impl CardModules for Ruleset {
    fn cards(&self) -> &[CardInfo] {
        Ruleset::cards(self)
    }
    fn card(&self, id: &str) -> Option<i32> {
        Ruleset::card(self, id)
    }
    fn card_props(&self, id: &str) -> std::collections::BTreeMap<String, i32> {
        Ruleset::card_props(self, id)
    }
    fn card_prop(&self, id: &str, key: &str) -> i32 {
        Ruleset::card_prop(self, id, key)
    }
    fn card_texts(&self, id: &str) -> Vec<ManifestTileText> {
        Ruleset::card_texts(self, id)
    }
    fn declares(&self, kind: crate::TriggerKind) -> bool {
        Ruleset::declares(self, kind)
    }
    fn tile_rule_id(&self, kind: &str) -> String {
        Ruleset::tile_rule_id(self, kind)
    }
    fn mark_rule_ids(&self) -> Vec<String> {
        Ruleset::mark_rule_ids(self)
    }
    fn status_rule_ids(&self) -> Vec<String> {
        Ruleset::status_rule_ids(self)
    }
    fn counteracts_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        Ruleset::counteracts_to(self, card, kind)
    }
    fn hooks_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        Ruleset::hooks_to(self, card, kind)
    }
    fn declares_buy(&self) -> bool {
        Ruleset::declares_buy(self)
    }
    fn counteract_pre_allows(
        &self,
        world: &crate::Run,
        card: i32,
        player_id: i32,
        scope: &rules_cond::WindowScope,
    ) -> Option<i32> {
        Ruleset::counteract_pre_allows(self, world, card, player_id, scope)
    }
    fn pre(&self, card: i32, entry: i32) -> Option<&crate::cond_pre::CompiledPre> {
        self.inner
            .pre
            .get(card as usize)?
            .get(entry as usize)?
            .as_ref()
    }
    fn entry_guard_is_none(&self, card: i32, entry: i32) -> bool {
        Ruleset::guard_is_none(self, card, entry)
    }
    fn play_gate_vanishes(&self, card: i32) -> bool {
        Ruleset::play_gate_vanishes(self, card)
    }
    fn gate_open(&self, kind: crate::TriggerKind, win: &rules_cond::WindowCtx) -> bool {
        let Some(i) = gate_index(kind) else {
            return true;
        };
        gate_open_over(
            self.inner.gates[i].iter().copied(),
            |c, e| CardModules::pre(self, c, e),
            win,
        )
    }
    fn sha256(&self) -> Option<&str> {
        Some(Ruleset::sha256(self))
    }
    fn module_sha256(&self, card: i32) -> Option<&str> {
        Ruleset::module_sha256(self, card)
    }
    fn run(
        &self,
        world: &crate::Run,
        call: Call,
        answers: &[i32],
        on_body: Option<&mut dyn FnMut(&mut crate::Run, i32)>,
    ) -> Result<Outcome<crate::Run>, RuleError> {
        Ruleset::run::<crate::Run>(self, world, call, answers, on_body)
    }
    fn run_hook(
        &self,
        world: &crate::Run,
        call: Call,
        answers: &[i32],
        on_body: Option<&mut dyn FnMut(&mut crate::Run, i32)>,
    ) -> Result<Option<HookRun<crate::Run>>, RuleError> {
        Ruleset::run_hook::<crate::Run>(self, world, call, answers, on_body)
    }
    fn can_counteract(
        &self,
        world: &crate::Run,
        card: i32,
        player_id: i32,
    ) -> Result<bool, RuleError> {
        Ruleset::can_counteract::<crate::Run>(self, world, card, player_id)
    }
    fn can_counteract_scoped(
        &self,
        world: &crate::Run,
        card: i32,
        player_id: i32,
        scope: &rules_cond::WindowScope,
    ) -> Result<bool, RuleError> {
        Ruleset::can_counteract_scoped::<crate::Run>(self, world, card, player_id, scope)
    }
    fn cant_play(
        &self,
        world: &crate::Run,
        card: i32,
        player_id: i32,
    ) -> Result<Option<crate::Msg>, RuleError> {
        Ruleset::cant_play::<crate::Run>(self, world, card, player_id)
    }
    fn declare_entry(
        &self,
        world: &crate::Run,
        card: i32,
        entry: i32,
        player_id: i32,
        answers: &[i32],
    ) -> Result<DeclareOut, RuleError> {
        Ruleset::declare_entry::<crate::Run>(self, world, card, entry, player_id, answers)
    }
}

// ---------------------------------------------------------------------------
// Backend-neutral host surface (`hostfns` + native). See `hostfns.rs`.
// ---------------------------------------------------------------------------

/// A host import failed. Backend-neutral on purpose: `hostfns` never names a
/// wasm engine's error type, so the native backend can produce the same two
/// outcomes the sandbox does.
#[derive(Debug, Clone)]
pub enum HostErr {
    /// The run stopped at a prompt / host request. The sandbox raises its
    /// `need_input` trap; the native backend unwinds with this and the entry
    /// point folds it into `EXIT_NEED_INPUT`.
    NeedInput,
    /// "this card is out" -- the sandbox trap contract.
    Trap(String),
}

impl HostErr {
    pub fn trap(msg: impl Into<String>) -> Self {
        HostErr::Trap(msg.into())
    }

    pub fn is_need_input(&self) -> bool {
        matches!(self, HostErr::NeedInput)
    }

    #[cfg(feature = "sandbox")]
    /// Convert to the wasm-engine error at the sandbox boundary.
    pub fn into_err(self) -> be::Error {
        match self {
            HostErr::NeedInput => be::need_input(),
            HostErr::Trap(m) => be::err(m),
        }
    }
}

/// What a nested card call returns when the caller asked for the gate's `Msg`
/// (the `Play` / `cant_play` shape) instead of a plain code.
pub enum CallOut {
    Code(i64),
    Msg(Option<crate::Msg>),
}

/// The card table a host run can look cards up in. `Arc<Inner>` on the sandbox
/// side; `rules-native` provides its own over the linked `RULESET` tables.
pub trait RulesHandle: Clone + Send + Sync + 'static {
    fn by_id(&self, id: &str) -> Option<i32>;
    fn card(&self, i: i32) -> Option<&CardInfo>;
    /// One declared static property of the card rule `id`
    /// (`CardInfo::props`); `0` when the card does not declare it (the
    /// defined default). Mirrors [`crate::Ruleset::card_prop`] /
    /// `game_core::engine::CardRules::card_prop`.
    fn card_prop(&self, id: &str, key: &str) -> i32 {
        self.by_id(id)
            .and_then(|i| self.card(i))
            .and_then(|c| c.props.get(key).copied())
            .unwrap_or(0)
    }
    /// The compiled condition of card `i`'s `entry` (docs/GUARDS.md G0), if
    /// any. Default `None` (no condition) so a stub handle need not care.
    fn pre(&self, i: i32, entry: i32) -> Option<&crate::cond_pre::CompiledPre> {
        let _ = (i, entry);
        None
    }
    /// A CEL-only capability gate, `true` = open (see [`CardModules::gate_open`]).
    /// Default scans the card table through [`Self::card`].
    fn gate_open(&self, kind: crate::TriggerKind, win: &rules_cond::WindowCtx) -> bool {
        let mut entries = Vec::new();
        let mut i = 0;
        while let Some(c) = self.card(i) {
            for (ei, o) in c.on.iter().enumerate() {
                if o.kind == OnKind::Filter as i32
                    && o.triggers.iter().any(|s| s == &kind.spelling())
                {
                    entries.push((i, ei as i32));
                }
            }
            i += 1;
        }
        gate_open_over(entries, |c, e| self.pre(c, e), win)
    }
}

#[cfg(feature = "sandbox")]
impl RulesHandle for Arc<Inner> {
    fn by_id(&self, id: &str) -> Option<i32> {
        self.by_id.get(id).copied()
    }
    fn card(&self, i: i32) -> Option<&CardInfo> {
        self.cards.get(i as usize)
    }
    fn pre(&self, i: i32, entry: i32) -> Option<&crate::cond_pre::CompiledPre> {
        self.pre.get(i as usize)?.get(entry as usize)?.as_ref()
    }
    fn gate_open(&self, kind: crate::TriggerKind, win: &rules_cond::WindowCtx) -> bool {
        let Some(i) = gate_index(kind) else {
            return true;
        };
        gate_open_over(
            self.gates[i].iter().copied(),
            |c, e| RulesHandle::pre(self, c, e),
            win,
        )
    }
}

/// The plumbing a host import needs that differs between the sandbox and the
/// native build: guest memory (wasm linear memory vs the native arena) and
/// fuel (store fuel vs a plain counter). Everything else lives on [`HostState`].
pub trait HostCtx {
    type World: CardWorld;
    type Rules: RulesHandle;

    fn st(&self) -> &HostState<Self::World, Self::Rules>;
    fn st_mut(&mut self) -> &mut HostState<Self::World, Self::Rules>;
    fn read_guest(&mut self, ptr: i32, len: i32) -> Result<Vec<u8>, HostErr>;
    fn write_guest(&mut self, ptr: i32, bytes: &[u8]) -> Result<(), HostErr>;
    fn fuel(&mut self) -> Result<u64, HostErr>;
    fn set_fuel(&mut self, v: u64) -> Result<(), HostErr>;
    /// Run `card`'s `entry` against `nested`. Returns the call result, the
    /// nested state (world / answers / prompts) and the fuel left. The sandbox
    /// builds a fresh `Store`; the native backend swaps the thread-local host.
    #[allow(clippy::type_complexity)]
    fn call_entry(
        &mut self,
        nested: HostState<Self::World, Self::Rules>,
        card: i32,
        entry: i32,
        op: i32,
        player_id: i32,
        arg: &[u8],
        want_msg: bool,
    ) -> (
        Result<CallOut, HostErr>,
        HostState<Self::World, Self::Rules>,
        u64,
    );
}

/// Guest memory access. Implemented over wasmtime/wasmi linear memory on the
/// sandbox path and over `card_sdk::native`'s arena on the native path.
pub trait GuestMem {
    fn read(&self, ptr: i32, len: i32) -> Result<Vec<u8>, HostErr>;
    fn write(&mut self, ptr: i32, bytes: &[u8]) -> Result<(), HostErr>;
}

/// Per-run store data. `world` is `None` only while a nested `play_card` has
/// borrowed it (the outer guest is suspended inside the host call at that time).
pub struct HostState<W, R = DefaultRules> {
    pub rules: Option<R>,
    pub world: Option<W>,
    pub answers: Vec<i32>,
    pub next_answer: usize,
    pub options: Vec<PromptOption>,
    /// Per-option prices for the next tile ask (`opt_price`).
    pub prices: Vec<i32>,
    /// The AI's preferred index for the next tile ask (`opt_ai`); -1 = unset.
    pub ai_hint: i32,
    /// How many options the next multi-select ask wants (`opt_count`); 0 = unset.
    pub count: i32,
    pub asked: Option<Prompt>,
    pub host_request: Option<HostRequest>,
    /// The activation argument (`postcard` bytes) the entry's `OP_DECLARE`
    /// collected, served back to the body via `ctx::declared_arg`
    /// (docs/ACTIVATION.md §2). Set before `OP_RUN`; empty = none.
    pub declared: Vec<u8>,
    pub depth: u32,
    // -- message context (`On::Message` / `ctx::send`) -------------------
    /// Sender instance uid for the message being handled.
    pub msg_sender_uid: i32,
    /// Sender seat for the message being handled.
    pub msg_sender_seat: i32,
    /// The message name being handled.
    pub msg_name: String,
    /// The payload.
    pub msg: card_sdk::abi::Message,
    /// The reply the handler set (`ctx::message::reply`).
    pub msg_reply: i32,
    /// Per-instance resource ceiling. Installed on the store by [`new_store`];
    /// lives in the store data because that is where `Store::limiter` wants its
    /// resource limiter to come from. Sandbox only -- the native backend has no
    /// store limiter.
    #[cfg(feature = "sandbox")]
    pub limits: be::StoreLimits,
}

#[cfg(feature = "sandbox")]
/// The per-instance memory ceiling, as a fresh [`be::StoreLimits`]. One per
/// store (each nested `play_card` is its own instance).
fn store_limits() -> be::StoreLimits {
    be::StoreLimitsBuilder::new()
        .memory_size(be::MAX_MEMORY_BYTES)
        .build()
}

/// Build a store with the per-instance memory ceiling installed.
#[cfg(feature = "sandbox")]
fn new_store<W>(engine: &Engine, state: HostState<W>) -> Store<HostState<W>> {
    let mut store = Store::new(engine, state);
    store.limiter(|s| &mut s.limits);
    store
}

impl<W, R: RulesHandle> HostState<W, R> {
    pub fn new(rules: R, world: W, answers: Vec<i32>, depth: u32) -> Self {
        Self {
            rules: Some(rules),
            world: Some(world),
            answers,
            next_answer: 0,
            options: vec![],
            prices: vec![],
            ai_hint: -1,
            count: 0,
            asked: None,
            host_request: None,
            declared: Vec::new(),
            depth,
            msg_sender_uid: -1,
            msg_sender_seat: -1,
            msg_name: String::new(),
            msg: card_sdk::abi::Message::default(),
            msg_reply: 0,
            #[cfg(feature = "sandbox")]
            limits: store_limits(),
        }
    }

    pub fn w(&mut self) -> &mut W {
        self.world
            .as_mut()
            .expect("world borrowed by a nested call")
    }

    pub fn wr(&self) -> &W {
        self.world
            .as_ref()
            .expect("world borrowed by a nested call")
    }
}

#[cfg(feature = "sandbox")]
/// The sandbox's [`HostCtx`]: a wasmi/wasmtime `Caller` over a `HostState`.
impl<'a, W: CardWorld> HostCtx for Caller<'a, HostState<W>> {
    type World = W;
    type Rules = Arc<Inner>;

    fn st(&self) -> &HostState<W> {
        self.data()
    }
    fn st_mut(&mut self) -> &mut HostState<W> {
        self.data_mut()
    }
    fn read_guest(&mut self, ptr: i32, len: i32) -> Result<Vec<u8>, HostErr> {
        be::read_guest(self, ptr, len).map_err(|e| HostErr::trap(be::error_text(&e)))
    }
    fn write_guest(&mut self, ptr: i32, bytes: &[u8]) -> Result<(), HostErr> {
        be::write_guest(self, ptr, bytes).map_err(|e| HostErr::trap(be::error_text(&e)))
    }
    fn fuel(&mut self) -> Result<u64, HostErr> {
        self.get_fuel()
            .map_err(|e| HostErr::trap(be::error_text(&e)))
    }
    fn set_fuel(&mut self, v: u64) -> Result<(), HostErr> {
        Caller::set_fuel(self, v).map_err(|e| HostErr::trap(be::error_text(&e)))
    }
    #[allow(clippy::type_complexity)]
    fn call_entry(
        &mut self,
        nested: HostState<W>,
        card: i32,
        entry: i32,
        op: i32,
        player_id: i32,
        arg: &[u8],
        want_msg: bool,
    ) -> (Result<CallOut, HostErr>, HostState<W>, u64) {
        let rules = match nested.rules.clone() {
            Some(r) => r,
            None => return (Err(HostErr::trap("call_entry without rules")), nested, 0),
        };
        // Share the remaining fuel with the nested run (the same contract
        // `play_card` had when it built the store inline): a runaway nest
        // exhausts one budget, and the leftover is handed back to the caller.
        let fuel = self.fuel().unwrap_or(0);
        let mut store = new_store(&rules.engine, nested);
        if let Err(e) = store
            .set_fuel(fuel)
            .map_err(|e| HostErr::trap(be::error_text(&e)))
        {
            let inner = store.into_data();
            return (Err(e), inner, fuel);
        }
        store.data_mut().declared = arg.to_vec();
        let res = if want_msg {
            match call_card_msg(&rules, &mut store, card, entry, op, player_id) {
                Ok(m) => Ok(CallOut::Msg(m)),
                Err(e) if be::is_need_input(&e) => Err(HostErr::NeedInput),
                Err(e) => Err(HostErr::trap(be::error_text(&e))),
            }
        } else {
            match call_card(&rules, &mut store, card, entry, op, player_id) {
                Ok(v) => Ok(CallOut::Code(v)),
                Err(e) if be::is_need_input(&e) => Err(HostErr::NeedInput),
                Err(e) => Err(HostErr::trap(be::error_text(&e))),
            }
        };
        let left = store.get_fuel().unwrap_or(0);
        let inner = store.into_data();
        (res, inner, left)
    }
}

/// Is `s` a well-formed entry id -- a stable internal **slug**, not display
/// text (docs/ACTIVATION.md §3 "Identity is not display text")?
/// `[A-Za-z0-9_][A-Za-z0-9_.:-]*`. Rulebook wording belongs in
/// `ManifestOn::label` / a locale key; renumbering a clause must not silently
/// break a query, and full-width vs half-width parens are different strings
/// that look identical in a diff.
fn is_entry_slug(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
}

#[cfg(feature = "sandbox")]
/// A card stops for a prompt by **returning** [`abi::EXIT_NEED_INPUT`] from
/// `ruleset_on` -- `rt::on` turns the run's `Err(card_sdk::Prompt)` into that
/// value. Fold it back into the host's own [`need_input`] marker so every
/// caller's `is_need_input` check (`Outcome::NeedInput` / `Outcome::NeedHost` /
/// [`RuleError::GuardPrompted`]) keeps working unchanged.
///
/// The two are one signal at different layers: the sentinel is the guest <-> host
/// wire form, the marker is this module's error. Nothing raises the marker *into*
/// the guest any more for the imports a card reads as `Result` -- see the note on
/// [`need_input`] for which ones still trap and why.
fn fold_exit(v: i64) -> Result<i64, Error> {
    if v == abi::EXIT_NEED_INPUT as i64 {
        Err(need_input())
    } else {
        Ok(v)
    }
}

/// Instantiate the card's module and call one of its entry points. Returns the
/// i32 result for `can_counteract`, 0 otherwise. A guest that stops for a prompt comes
/// back as [`need_input`], never as the raw sentinel.
#[cfg(feature = "sandbox")]
fn call_card<W: CardWorld>(
    rules: &Inner,
    store: &mut Store<HostState<W>>,
    card: i32,
    entry: i32,
    op: i32,
    player_id: i32,
) -> Result<i64, Error> {
    let slot = rules.slots[card as usize];
    #[cfg(feature = "bot-cost")]
    let t0 = std::time::Instant::now();
    let inst = instantiate_cached(rules, &mut *store, slot.module)?;
    #[cfg(feature = "bot-cost")]
    {
        use std::sync::atomic::Ordering::Relaxed;
        bot_cost::INSTANTIATIONS.fetch_add(1, Relaxed);
        bot_cost::INSTANTIATE_NS.fetch_add(t0.elapsed().as_nanos() as u64, Relaxed);
    }
    #[cfg(feature = "bot-cost")]
    let t1 = std::time::Instant::now();
    let out = inst
        .get_typed_func::<(i32, i32, i32, i32), i64>(&mut *store, export::ON)?
        .call(&mut *store, (slot.local, entry, op, player_id))
        .and_then(fold_exit);
    #[cfg(feature = "bot-cost")]
    bot_cost::GUEST_NS.fetch_add(
        t1.elapsed().as_nanos() as u64,
        std::sync::atomic::Ordering::Relaxed,
    );
    out
}

/// Like [`call_card`] for a `Play` gate, which answers with a packed `Msg`
/// buffer: the buffer is read back before the instance drops. `op` is
/// [`export::OP_GUARD`] for the gate; a run that stops for a prompt never gets
/// this far (the sentinel is folded first, so it cannot be misread as a `Msg`
/// pointer).
#[cfg(feature = "sandbox")]
fn call_card_msg<W: CardWorld>(
    rules: &Inner,
    store: &mut Store<HostState<W>>,
    card: i32,
    entry: i32,
    op: i32,
    player_id: i32,
) -> Result<Option<crate::Msg>, Error> {
    let slot = rules.slots[card as usize];
    #[cfg(feature = "bot-cost")]
    let t0 = std::time::Instant::now();
    let inst = instantiate_cached(rules, &mut *store, slot.module)?;
    #[cfg(feature = "bot-cost")]
    {
        use std::sync::atomic::Ordering::Relaxed;
        bot_cost::INSTANTIATIONS.fetch_add(1, Relaxed);
        bot_cost::INSTANTIATE_NS.fetch_add(t0.elapsed().as_nanos() as u64, Relaxed);
    }
    #[cfg(feature = "bot-cost")]
    let t1 = std::time::Instant::now();
    let packed = inst
        .get_typed_func::<(i32, i32, i32, i32), i64>(&mut *store, export::ON)?
        .call(&mut *store, (slot.local, entry, op, player_id))
        .and_then(fold_exit)?;
    #[cfg(feature = "bot-cost")]
    bot_cost::GUEST_NS.fetch_add(
        t1.elapsed().as_nanos() as u64,
        std::sync::atomic::Ordering::Relaxed,
    );
    if packed == 0 {
        return Ok(None);
    }
    let (ptr, len) = abi::unpack(packed);
    let bytes = read_mem(&inst, store, ptr as i32, len as i32)?;
    let m: card_sdk::msg::Msg = postcard::from_bytes(&bytes).map_err(|e| {
        err(format!(
            "cant_play: guest message is not Msg postcard ({e})"
        ))
    })?;
    Ok(Some(engine_msg(m)))
}

/// Run one `OP_DECLARE` collector and read back its `postcard` bytes
/// (docs/ACTIVATION.md §2). The packed return is `(ptr<<32)|len` in guest
/// memory, so a buffer address can never collide with an `EXIT_*` sentinel.
///
/// Returns the raw `i64` plus the decoded bytes (empty when the return was a
/// sentinel or a zero-length buffer). `declare_entry` decides what each
/// sentinel means.
#[cfg(feature = "sandbox")]
fn call_card_declare<W: CardWorld>(
    rules: &Inner,
    store: &mut Store<HostState<W>>,
    card: i32,
    entry: i32,
    player_id: i32,
) -> Result<(i64, Vec<u8>), Error> {
    let slot = rules.slots[card as usize];
    let inst = instantiate_cached(rules, &mut *store, slot.module)?;
    let packed = inst
        .get_typed_func::<(i32, i32, i32, i32), i64>(&mut *store, export::ON)?
        .call(&mut *store, (slot.local, entry, export::OP_DECLARE, player_id))
        .and_then(fold_exit)?;
    if packed == 0
        || packed == abi::EXIT_NEED_INPUT as i64
        || packed == abi::EXIT_DECLINED as i64
    {
        return Ok((packed, Vec::new()));
    }
    let (ptr, len) = abi::unpack(packed);
    let bytes = read_mem(&inst, store, ptr as i32, len as i32)?;
    Ok((packed, bytes))
}

/// Validate one module and read its manifest. Also reads the optional
/// `ruleset_metadata` roster blob (ABI v63) when the module exports it.
#[cfg(feature = "sandbox")]
fn inspect(
    engine: &Engine,
    wasm: &[u8],
) -> Result<(Module, Vec<CardInfo>, card_sdk::meta::MetaDoc), RuleError> {
    let load = |m: String| RuleError::Load(m);
    let module = be::compile(engine, wasm).map_err(load)?;

    // Metadata exports never touch the world; a no-op world is enough.
    let mut store = new_store(engine, HostState::<NullWorld>::without_rules());
    set_fuel(&mut store, DEFAULT_FUEL).map_err(load)?;
    let inst = build_linker::<NullWorld>(engine)
        .map_err(|e| load(error_text(&e)))
        .and_then(|l| instantiate(&l, &mut store, &module).map_err(|e| load(error_text(&e))))?;

    let version = inst
        .get_typed_func::<(), i32>(&mut store, export::ABI_VERSION)
        .and_then(|f| f.call(&mut store, ()))
        .map_err(|e| load(format!("{}: {e}", export::ABI_VERSION)))?;
    if version != ABI_VERSION {
        return Err(load(format!(
            "module built for ABI v{version}, host speaks v{ABI_VERSION}"
        )));
    }

    let packed = inst
        .get_typed_func::<(), i64>(&mut store, export::MANIFEST)
        .and_then(|f| f.call(&mut store, ()))
        .map_err(|e| load(format!("{}: {e}", export::MANIFEST)))?;
    let (ptr, len) = abi::unpack(packed);
    let bytes =
        read_mem(&inst, &mut store, ptr as i32, len as i32).map_err(|e| load(error_text(&e)))?;
    let entries: Vec<card_sdk::abi::ManifestEntry> =
        postcard::from_bytes(&bytes).map_err(|e| load(format!("manifest: {e}")))?;
    let cards: Vec<CardInfo> = entries
        .into_iter()
        .map(|e| CardInfo {
            id: e.id,
            on: e.on,
            props: e.props.into_iter().collect(),
            texts: e.texts,
            board_role: e.board_role,
            governs: e.governs,
            gate_msgs: e.gate_msgs,
            gate_skip_msgs: e.gate_skip_msgs,
            exemption_prop: e.exemption_prop,
        })
        .collect();
    if cards.is_empty() {
        return Err(load("module declares no cards".into()));
    }
    for c in &cards {
        if let Some(bad) = c.on.iter().find(|o| OnKind::from_i32(o.kind).is_none()) {
            return Err(load(format!("{}: unknown entry kind {}", c.id, bad.kind)));
        }
    }
    if !has_func(&inst, &mut store, export::ON) {
        return Err(load(format!("missing export {}", export::ON)));
    }
    // Roster metadata (ABI v63) is optional -- absent on pre-v63 modules and
    // on fixtures that declare no `.meta()`. Built only when this is called.
    let mut meta = card_sdk::meta::MetaDoc::default();
    if has_func(&inst, &mut store, export::METADATA) {
        set_fuel(&mut store, METADATA_FUEL).map_err(load)?;
        let packed = inst
            .get_typed_func::<(), i64>(&mut store, export::METADATA)
            .and_then(|f| f.call(&mut store, ()))
            .map_err(|e| load(format!("{}: {e}", export::METADATA)))?;
        let (ptr, len) = abi::unpack(packed);
        let bytes = read_mem(&inst, &mut store, ptr as i32, len as i32)
            .map_err(|e| load(error_text(&e)))?;
        meta = crate::roster::decode_metadata(&bytes).map_err(load)?;
    }
    // Tile-class table (ABI v70) -- optional; empty means only the generic
    // `class*` props answer `tile_in`.
    if has_func(&inst, &mut store, export::CLASS_DEFS) {
        set_fuel(&mut store, METADATA_FUEL).map_err(load)?;
        let packed = inst
            .get_typed_func::<(), i64>(&mut store, export::CLASS_DEFS)
            .and_then(|f| f.call(&mut store, ()))
            .map_err(|e| load(format!("{}: {e}", export::CLASS_DEFS)))?;
        let (ptr, len) = abi::unpack(packed);
        let bytes = read_mem(&inst, &mut store, ptr as i32, len as i32)
            .map_err(|e| load(error_text(&e)))?;
        let defs: Vec<crate::tile_class::ClassDef> =
            postcard::from_bytes(&bytes).unwrap_or_default();
        crate::tile_class::set_class_defs(defs);
    }
    // Ruleset-kind flags (hook-only) -- optional; absent means every ruleset
    // kind opens a counteraction window.
    if has_func(&inst, &mut store, export::KIND_FLAGS) {
        set_fuel(&mut store, METADATA_FUEL).map_err(load)?;
        let packed = inst
            .get_typed_func::<(), i64>(&mut store, export::KIND_FLAGS)
            .and_then(|f| f.call(&mut store, ()))
            .map_err(|e| load(format!("{}: {e}", export::KIND_FLAGS)))?;
        let (ptr, len) = abi::unpack(packed);
        let bytes = read_mem(&inst, &mut store, ptr as i32, len as i32)
            .map_err(|e| load(error_text(&e)))?;
        let flags: Vec<(i32, bool)> = postcard::from_bytes(&bytes).unwrap_or_default();
        crate::kind_flags::set_kind_flags(flags);
    }
    Ok((module, cards, meta))
}

/// The abnormal counteraction gate, from inside a card run: on replay the logged
/// answer says whether the effect went through; otherwise the run is paused
/// with a `Gate` request and the engine adjudicates it.
///
/// Pauses by **trapping** with [`need_input`], unlike `ask`/`pay`/`play_card`.
/// `ctx::gate` reads this as a plain `bool`, so a sentinel here would read as
/// `true` ("gate passed") and the effect would run on an unevaluated gate. Same
/// for `target`, `card_move` and the buy/build routines below -- see
/// [`need_input`] for the whole split.
#[cfg(feature = "sandbox")]
fn gate<W: CardWorld>(
    c: &mut Caller<'_, HostState<W>>,
    player_id: i32,
    kind: AbKind,
) -> Result<bool, Error> {
    let st = c.data_mut();
    if let Some(&allowed) = st.answers.get(st.next_answer) {
        st.next_answer += 1;
        return Ok(allowed != 0);
    }
    st.host_request = Some(HostRequest::Gate { player_id, kind });
    Err(need_input())
}

#[cfg(feature = "sandbox")]
fn guest_str<W>(
    caller: &mut Caller<'_, HostState<W>>,
    ptr: i32,
    len: i32,
) -> Result<String, Error> {
    let bytes = read_guest(caller, ptr, len)?;
    String::from_utf8(bytes).map_err(|_| err("guest string is not UTF-8"))
}

/// A message from the guest (ABI v5: every text parameter is a `postcard`-encoded
/// `Msg`). A card that sends anything else traps instead of showing raw text to
/// players. Decoding uses the same `serde` type the guest encodes with, so the
/// two sides cannot disagree on the layout.
#[cfg(feature = "sandbox")]
fn guest_msg<W: CardWorld>(
    caller: &mut Caller<'_, HostState<W>>,
    ptr: i32,
    len: i32,
) -> Result<crate::Msg, Error> {
    let bytes = read_guest(caller, ptr, len)?;
    let m: card_sdk::msg::Msg = postcard::from_bytes(&bytes)
        .map_err(|e| err(format!("guest message is not Msg postcard ({e})")))?;
    Ok(engine_msg(m))
}

/// Guest `Msg` -> engine `Msg`. An exhaustive match: a new argument kind on either
/// side has to be mapped here on purpose.
pub fn engine_msg(m: card_sdk::msg::Msg) -> crate::Msg {
    use card_sdk::msg::Arg as G;
    use game_core::msg::Arg as E;
    let mut out = crate::Msg::new(m.k.as_str());
    for (name, arg) in m.a {
        let arg = match arg {
            G::PlayerId(v) => E::PlayerId(v),
            G::Tile(v) => E::Tile(v),
            G::Card(v) => E::Card(v),
            G::Persona(v) => E::Persona(v),
            G::N(v) => E::N(v),
            G::I(v) => E::I(v),
            G::Msg(v) => E::Msg(Box::new(engine_msg(*v))),
        };
        out.set_arg(name, arg);
    }
    out
}

#[cfg(feature = "sandbox")]
// Linkers, cached per (engine, world type). The linker is a pure function of
// the world type -- every host function is a fixed registration -- but
// building it walks ~160 `func_wrap` calls and cost ~1.3ms of every ~2ms
// linker init, so it must never be per call. Thread-local because the world type
// is not `Send`-bounded and the cache is pure memoization.
thread_local! {
    static LINKERS: std::cell::RefCell<HashMap<(usize, std::any::TypeId), Box<dyn std::any::Any>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// `Inner` ids, so the caches below cannot hit across two sets of modules that
/// happen to share an address.
#[cfg(feature = "sandbox")]
fn next_inner_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

// Instantiation points, cached per (ruleset, world type, module). With the
// linker cached, resolving a module's imports against it is the rest of a
// linker init; `Prepared` does that once and every call instantiates from it.
// Same thread-local reasoning as [`LINKERS`].
#[cfg(feature = "sandbox")]
thread_local! {
    static PREPARED: std::cell::RefCell<
        HashMap<(u64, std::any::TypeId, usize), Box<dyn std::any::Any>>,
    > = std::cell::RefCell::new(HashMap::new());
}

/// Instantiate `rules.modules[module_ix]` against its cached [`be::Prepared`],
/// building it on the first call. Never clones the linker: the whole lookup,
/// build and instantiate happens under one borrow.
#[cfg(feature = "sandbox")]
fn instantiate_cached<W: CardWorld + 'static>(
    rules: &Inner,
    store: &mut Store<HostState<W>>,
    module_ix: usize,
) -> Result<be::Instance, Error> {
    let key = (rules.id, std::any::TypeId::of::<W>(), module_ix);
    PREPARED.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(hit) = c.get(&key) {
            let p = hit
                .downcast_ref::<be::Prepared<HostState<W>>>()
                .expect("cache is keyed by TypeId");
            return be::instantiate_prepared(p, store);
        }
        let linker = linker::<W>(rules)?;
        let p = be::prepare(&linker, &rules.modules[module_ix].module)?;
        let out = be::instantiate_prepared(&p, store);
        c.insert(key, Box::new(p));
        out
    })
}

#[cfg(feature = "sandbox")]
fn linker<W: CardWorld + 'static>(rules: &Inner) -> Result<Linker<HostState<W>>, Error> {
    let key = (
        &rules.engine as *const Engine as usize,
        std::any::TypeId::of::<W>(),
    );
    let cached = LINKERS.with(|c| {
        c.borrow().get(&key).map(|hit| {
            hit.downcast_ref::<Linker<HostState<W>>>()
                .expect("cache is keyed by TypeId")
                .clone()
        })
    });
    if let Some(hit) = cached {
        return Ok(hit);
    }
    let built = build_linker::<W>(&rules.engine)?;
    LINKERS.with(|c| c.borrow_mut().insert(key, Box::new(built.clone())));
    Ok(built)
}

#[cfg(feature = "sandbox")]
fn build_linker<W: CardWorld>(engine: &Engine) -> Result<Linker<HostState<W>>, Error> {
    let mut l = Linker::<HostState<W>>::new(engine);
    let m = IMPORT_MODULE;
    type C<'a, W> = Caller<'a, HostState<W>>;

    // Thin wrappers over `hostfns` -- the bodies live there so the native
    // backend (`rules-native`) can call the exact same logic.
    l.func_wrap(
        m,
        "roll",
        |mut c: C<W>,
         player_id: i32,
         count: i32,
         sides: i32,
         purpose: i32,
         source: i32|
         -> Result<i32, Error> {
            hostfns::roll(&mut c, player_id, count, sides, purpose, source).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "roll_direct",
        |mut c: C<W>, player_id: i32, count: i32, sides: i32, purpose: i32| -> Result<i32, Error> {
            hostfns::roll_direct(&mut c, player_id, count, sides, purpose).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "log",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::log(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "effect",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::effect(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    // flash (v55) -- once-per-flash identity lives on the drive's Run state.
    l.func_wrap(m, "flash", |mut c: C<W>| -> Result<(), Error> {
        hostfns::flash(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "no_flash", |mut c: C<W>| -> Result<(), Error> {
        hostfns::no_flash(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "flash_card",
        |mut c: C<W>, cptr: i32, cl: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::flash_card(&mut c, cptr, cl, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "flash_card_keyed",
        |mut c: C<W>, kp: i32, kl: i32, cptr: i32, cl: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::flash_card_keyed(&mut c, kp, kl, cptr, cl, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "flash_tile",
        |mut c: C<W>, tile: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::flash_tile(&mut c, tile, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "tile_count", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::tile_count(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "place_mark",
        |mut c: C<W>,
         tile: i32,
         kp: i32,
         kl: i32,
         cptr: i32,
         cl: i32,
         owner: i32,
         src: i32,
         count: i32,
         stack: i32,
         np: i32,
         nl: i32|
         -> Result<i32, Error> {
            hostfns::place_mark(
                &mut c, tile, kp, kl, cptr, cl, owner, src, count, stack, np, nl,
            )
            .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "place_mark_pair",
        |mut c: C<W>,
         a: i32,
         b: i32,
         kp: i32,
         kl: i32,
         cptr: i32,
         cl: i32,
         owner: i32,
         src: i32,
         count: i32,
         stack: i32,
         np: i32,
         nl: i32|
         -> Result<i32, Error> {
            hostfns::place_mark_pair(
                &mut c, a, b, kp, kl, cptr, cl, owner, src, count, stack, np, nl,
            )
            .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "count_marks_f",
        |mut c: C<W>, tile: i32, fp: i32, fl: i32| -> Result<i32, Error> {
            hostfns::count_marks_f(&mut c, tile, fp, fl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "bump_mark_f",
        |mut c: C<W>, tile: i32, fp: i32, fl: i32, delta: i32| -> Result<i32, Error> {
            hostfns::bump_mark_f(&mut c, tile, fp, fl, delta).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "remove_marks_f",
        |mut c: C<W>, tile: i32, fp: i32, fl: i32| -> Result<i32, Error> {
            hostfns::remove_marks_f(&mut c, tile, fp, fl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "mark_src_at",
        |mut c: C<W>, tile: i32, fp: i32, fl: i32| -> Result<i32, Error> {
            hostfns::mark_src_at(&mut c, tile, fp, fl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "mark_instance_at",
        |mut c: C<W>, tile: i32, fp: i32, fl: i32| -> Result<i32, Error> {
            hostfns::mark_instance_at(&mut c, tile, fp, fl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "mark_partner",
        |mut c: C<W>, tile: i32, fp: i32, fl: i32| -> Result<i32, Error> {
            hostfns::mark_partner(&mut c, tile, fp, fl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "mark_tile", |mut c: C<W>, uid: i32| -> Result<i32, Error> {
        hostfns::mark_tile(&mut c, uid).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "remove_mark",
        |mut c: C<W>, uid: i32| -> Result<i32, Error> {
            hostfns::remove_mark(&mut c, uid).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "counter_self",
        |mut c: C<W>, np: i32, nl: i32| -> Result<i32, Error> {
            hostfns::counter_self(&mut c, np, nl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_counter_self",
        |mut c: C<W>, np: i32, nl: i32, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_counter_self(&mut c, np, nl, n, max).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_counter_self",
        |mut c: C<W>, np: i32, nl: i32, n: i32| -> Result<i32, Error> {
            hostfns::set_counter_self(&mut c, np, nl, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "counter_at",
        |mut c: C<W>, uid: i32, np: i32, nl: i32| -> Result<i32, Error> {
            hostfns::counter_at(&mut c, uid, np, nl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_counter_at",
        |mut c: C<W>, uid: i32, np: i32, nl: i32, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_counter_at(&mut c, uid, np, nl, n, max).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_counter",
        |mut c: C<W>, player_id: i32, cptr: i32, cl: i32, np: i32, nl: i32| -> Result<i32, Error> {
            hostfns::card_counter(&mut c, player_id, cptr, cl, np, nl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_card_counter",
        |mut c: C<W>,
         player_id: i32,
         cptr: i32,
         cl: i32,
         np: i32,
         nl: i32,
         n: i32,
         max: i32|
         -> Result<i32, Error> {
            hostfns::add_card_counter(&mut c, player_id, cptr, cl, np, nl, n, max)
                .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "count_held",
        |mut c: C<W>, np: i32, nl: i32, player_id: i32| -> Result<i32, Error> {
            hostfns::count_held(&mut c, np, nl, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_held",
        |mut c: C<W>, np: i32, nl: i32, player_id: i32, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_held(&mut c, np, nl, player_id, n, max).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "count_held_name",
        |mut c: C<W>, np: i32, nl: i32, player_id: i32| -> Result<i32, Error> {
            hostfns::count_held_name(&mut c, np, nl, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_held_name",
        |mut c: C<W>,
         np: i32,
         nl: i32,
         player_id: i32,
         v: i32,
         max: i32,
         instance: i32|
         -> Result<(), Error> {
            hostfns::set_held_name(&mut c, np, nl, player_id, v, max, instance)
                .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tok_max",
        |mut c: C<W>, np: i32, nl: i32, player_id: i32| -> Result<i32, Error> {
            hostfns::tok_max(&mut c, np, nl, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "move_units",
        |mut c: C<W>,
         np: i32,
         nl: i32,
         from_tile: i32,
         from_player: i32,
         to_tile: i32,
         to_player: i32,
         n: i32|
         -> Result<i32, Error> {
            hostfns::move_units(
                &mut c,
                np,
                nl,
                from_tile,
                from_player,
                to_tile,
                to_player,
                n,
            )
            .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "self_uid", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::self_uid(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "send",
        |mut c: C<W>, tp: i32, tl: i32, np: i32, nl: i32, pp: i32, pl: i32| -> Result<i32, Error> {
            hostfns::send(&mut c, tp, tl, np, nl, pp, pl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "msg_sender_uid", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::msg_sender_uid(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "msg_sender_seat", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::msg_sender_seat(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "msg_name",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::msg_name(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "msg_a", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::msg_a(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "msg_b", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::msg_b(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "msg_c", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::msg_c(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "msg_tile", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::msg_tile(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "msg_seat", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::msg_seat(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "msg_text",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::msg_text(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "msg_reply", |mut c: C<W>, v: i32| -> Result<(), Error> {
        hostfns::msg_reply(&mut c, v).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "trig_name",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::trig_name(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "money",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::money(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "gain",
        |mut c: C<W>, player_id: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::gain(&mut c, player_id, amount, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "gain_typed",
        |mut c: C<W>,
         player_id: i32,
         amount: i32,
         tp: i32,
         tl: i32,
         xp: i32,
         xl: i32|
         -> Result<i32, Error> {
            hostfns::gain_typed(&mut c, player_id, amount, tp, tl, xp, xl)
                .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "adjust_money",
        |mut c: C<W>, player_id: i32, delta: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::adjust_money(&mut c, player_id, delta, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "pay",
        |mut c: C<W>, player_id: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::pay(&mut c, player_id, amount, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "pay_to",
        |mut c: C<W>, from: i32, to: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::pay_to(&mut c, from, to, amount, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "pay_total",
        |mut c: C<W>, from: i32, to: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::pay_total(&mut c, from, to, amount, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "pay_leg",
        |mut c: C<W>, from: i32, to: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::pay_leg(&mut c, from, to, amount, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "player_count", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::player_count(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "player_out",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::player_out(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "others_count",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::others_count(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "others_at",
        |mut c: C<W>, player_id: i32, index: i32| -> Result<i32, Error> {
            hostfns::others_at(&mut c, player_id, index).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_named",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::tile_named(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_owner",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::tile_owner(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "player_pos",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::player_pos(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_steps_ahead",
        |mut c: C<W>, player_id: i32, steps: i32| -> Result<i32, Error> {
            hostfns::tile_steps_ahead(&mut c, player_id, steps).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "rent_of",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::rent_of(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "buy_price",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::buy_price(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "build_cost",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::build_cost(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "mortgage_value",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::mortgage_value(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "owned_count",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::owned_count(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "owned_at",
        |mut c: C<W>, player_id: i32, index: i32| -> Result<i32, Error> {
            hostfns::owned_at(&mut c, player_id, index).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_is",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32| -> Result<i32, Error> {
            hostfns::tile_is(&mut c, tile, kp, kl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_attr",
        |mut c: C<W>, tile: i32, ap: i32, al: i32| -> Result<i32, Error> {
            hostfns::tile_attr(&mut c, tile, ap, al).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_in",
        |mut c: C<W>, player_id: i32, tile: i32, cptr: i32, cl: i32| -> Result<i32, Error> {
            hostfns::tile_in(&mut c, player_id, tile, cptr, cl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "paid_in_settle", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::paid_in_settle(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "turn_rolls",
        |mut c: C<W>, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::turn_rolls(&mut c, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "field_instances",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::field_instances(&mut c, player_id, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "unplace_at",
        |mut c: C<W>, uid: i32| -> Result<i32, Error> {
            hostfns::unplace_at(&mut c, uid).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_at",
        |mut c: C<W>, uid: i32| -> Result<i32, Error> {
            hostfns::tile_at(&mut c, uid).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_tile_at",
        |mut c: C<W>, uid: i32, tile: i32| -> Result<i32, Error> {
            hostfns::set_tile_at(&mut c, uid, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "is_face_down_at",
        |mut c: C<W>, uid: i32| -> Result<i32, Error> {
            hostfns::is_face_down_at(&mut c, uid).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_face_down_at",
        |mut c: C<W>, uid: i32, on: i32| -> Result<i32, Error> {
            hostfns::set_face_down_at(&mut c, uid, on).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "is_immune_at",
        |mut c: C<W>, uid: i32| -> Result<i32, Error> {
            hostfns::is_immune_at(&mut c, uid).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_immune_at",
        |mut c: C<W>, uid: i32, on: i32| -> Result<i32, Error> {
            hostfns::set_immune_at(&mut c, uid, on).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_build_discount",
        |mut c: C<W>, n: i32, layers: i32| -> Result<(), Error> {
            hostfns::set_build_discount(&mut c, n, layers).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_build_cost_pct",
        |mut c: C<W>, pct: i32| -> Result<(), Error> {
            hostfns::set_build_cost_pct(&mut c, pct).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "turn_start_pos",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::turn_start_pos(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "turn_snap",
        |mut c: C<W>, player_id: i32, buf: i32| -> Result<i32, Error> {
            hostfns::turn_snap(&mut c, player_id, buf).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_group",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::tile_group(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_price",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::tile_price(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "houses_of",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::houses_of(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "rent_houses_of",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::rent_houses_of(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_houses",
        |mut c: C<W>, tile: i32, n: i32| -> Result<(), Error> {
            hostfns::set_houses(&mut c, tile, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_house",
        |mut c: C<W>, tile: i32, n: i32| -> Result<i32, Error> {
            hostfns::add_house(&mut c, tile, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "mortgaged_of",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::mortgaged_of(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_mortgaged",
        |mut c: C<W>, tile: i32, v: i32| -> Result<(), Error> {
            hostfns::set_mortgaged(&mut c, tile, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_owner",
        |mut c: C<W>, tile: i32, player_id: i32| -> Result<(), Error> {
            hostfns::set_owner(&mut c, tile, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "dist",
        |mut c: C<W>, a: i32, b: i32| -> Result<i32, Error> {
            hostfns::dist(&mut c, a, b).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_forward",
        |mut c: C<W>, a: i32, b: i32| -> Result<i32, Error> {
            hostfns::tile_forward(&mut c, a, b).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "neighbor",
        |mut c: C<W>, player_id: i32, dir: i32| -> Result<i32, Error> {
            hostfns::neighbor(&mut c, player_id, dir).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "players_on_count",
        |mut c: C<W>, tile: i32, except: i32| -> Result<i32, Error> {
            hostfns::players_on_count(&mut c, tile, except).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "players_on_at",
        |mut c: C<W>, tile: i32, except: i32, index: i32| -> Result<i32, Error> {
            hostfns::players_on_at(&mut c, tile, except, index).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "draw",
        |mut c: C<W>, player_id: i32, n: i32| -> Result<i32, Error> {
            hostfns::draw(&mut c, player_id, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "draw_event",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::draw_event(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "pay_rent",
        |mut c: C<W>, player_id: i32, tile: i32, half: i32| -> Result<i32, Error> {
            hostfns::pay_rent(&mut c, player_id, tile, half).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "charge_rent",
        |mut c: C<W>, player_id: i32, tile: i32, milli_scale: i32| -> Result<i32, Error> {
            hostfns::charge_rent(&mut c, player_id, tile, milli_scale).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "offer_buy",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            hostfns::offer_buy(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "offer_force_buy",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            hostfns::offer_force_buy(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "offer_build",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            hostfns::offer_build(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_to_dest",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, dest: i32| -> Result<(), Error> {
            hostfns::add_to_dest(&mut c, player_id, p, n, dest).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "take_card",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, src: i32| -> Result<i32, Error> {
            hostfns::take_card(&mut c, player_id, p, n, src).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "cards_in",
        |mut c: C<W>, player_id: i32, pile: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::cards_in(&mut c, player_id, pile, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "to_discard",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::to_discard(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "hand_count",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::hand_count(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "discard_count",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::discard_count(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "hand_size",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::hand_size(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "deck_count",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::deck_count(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "discard_size",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::discard_size(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "discard_from_hand",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::discard_from_hand(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "shuffle_into_deck",
        |mut c: C<W>, player_id: i32, hand: i32, discard: i32| -> Result<i32, Error> {
            hostfns::shuffle_into_deck(&mut c, player_id, hand, discard).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "unplace_card", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::unplace_card(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "event_expire",
        |mut c: C<W>, ip: i32, il: i32, removed: i32| -> Result<(), Error> {
            hostfns::event_expire(&mut c, ip, il, removed).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "event_is_active",
        |mut c: C<W>, ip: i32, il: i32| -> Result<i32, Error> {
            hostfns::event_is_active(&mut c, ip, il).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "event_deck_push",
        |mut c: C<W>, ip: i32, il: i32, face_down: i32| -> Result<(), Error> {
            hostfns::event_deck_push(&mut c, ip, il, face_down).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "event_banish",
        |mut c: C<W>, ip: i32, il: i32| -> Result<(), Error> {
            hostfns::event_banish(&mut c, ip, il).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "placed_cards",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::placed_cards(&mut c, player_id, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_text_mentions",
        |mut c: C<W>, cptr: i32, cl: i32, np: i32, nl: i32| -> Result<i32, Error> {
            hostfns::card_text_mentions(&mut c, cptr, cl, np, nl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_prop",
        |mut c: C<W>, ip: i32, il: i32, kp: i32, kl: i32| -> Result<i32, Error> {
            hostfns::card_prop(&mut c, ip, il, kp, kl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "is_placed", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::is_placed(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "self_tile", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::self_tile(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_self_tile",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::set_self_tile(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "self_face_down", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::self_face_down(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_self_face_down",
        |mut c: C<W>, on: i32| -> Result<i32, Error> {
            hostfns::set_self_face_down(&mut c, on).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "self_immune", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::self_immune(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_self_immune",
        |mut c: C<W>, on: i32| -> Result<i32, Error> {
            hostfns::set_self_immune(&mut c, on).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "self_prop",
        |mut c: C<W>, kp: i32, kl: i32| -> Result<i32, Error> {
            hostfns::self_prop(&mut c, kp, kl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_self_prop",
        |mut c: C<W>, kp: i32, kl: i32, v: i32| -> Result<i32, Error> {
            hostfns::set_self_prop(&mut c, kp, kl, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tile_prop",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32| -> Result<i32, Error> {
            hostfns::tile_prop(&mut c, tile, kp, kl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_tile_prop",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32, v: i32| -> Result<i32, Error> {
            hostfns::set_tile_prop(&mut c, tile, kp, kl, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "prop_at",
        |mut c: C<W>, uid: i32, kp: i32, kl: i32| -> Result<i32, Error> {
            hostfns::prop_at(&mut c, uid, kp, kl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_prop_at",
        |mut c: C<W>, uid: i32, kp: i32, kl: i32, v: i32| -> Result<i32, Error> {
            hostfns::set_prop_at(&mut c, uid, kp, kl, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "state_get",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, field: i32| -> Result<i32, Error> {
            hostfns::state_get(&mut c, player_id, p, n, field).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "state_set",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, field: i32, v: i32| -> Result<i32, Error> {
            hostfns::state_set(&mut c, player_id, p, n, field, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "state_add",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, delta: i32| -> Result<i32, Error> {
            hostfns::state_add(&mut c, player_id, p, n, delta).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "group_pool",
        |mut c: C<W>, player_id: i32, np: i32, nl: i32| -> Result<i32, Error> {
            hostfns::group_pool(&mut c, player_id, np, nl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_group_pool",
        |mut c: C<W>, player_id: i32, np: i32, nl: i32, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_group_pool(&mut c, player_id, np, nl, n, max).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "group_skill",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::group_skill(&mut c, player_id, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "persona_skill",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::persona_skill(&mut c, player_id, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "group_skills",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::group_skills(&mut c, player_id, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_group_skill",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, extra: i32| -> Result<i32, Error> {
            hostfns::add_group_skill(&mut c, player_id, p, n, extra).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "invoke_skill",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::invoke_skill(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "raise_bought",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            hostfns::raise_bought(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "raise",
        |mut c: C<W>, player_id: i32, kp: i32, kl: i32, value: i32| -> Result<i32, Error> {
            let kind = hostfns::guest_str(&mut c, kp, kl).map_err(HostErr::into_err)?;
            hostfns::raise(&mut c, player_id, &kind, value).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "gain_marker",
        |mut c: C<W>,
         player_id: i32,
         np: i32,
         nl: i32,
         n: i32,
         p: i32,
         len: i32|
         -> Result<i32, Error> {
            let name = hostfns::guest_str(&mut c, np, nl).map_err(HostErr::into_err)?;
            hostfns::gain_marker(&mut c, player_id, &name, n, p, len).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "spend_marker",
        |mut c: C<W>,
         player_id: i32,
         np: i32,
         nl: i32,
         n: i32,
         p: i32,
         len: i32|
         -> Result<i32, Error> {
            let name = hostfns::guest_str(&mut c, np, nl).map_err(HostErr::into_err)?;
            hostfns::spend_marker(&mut c, player_id, &name, n, p, len).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_marker_max",
        |mut c: C<W>, player_id: i32, np: i32, nl: i32, n: i32| -> Result<i32, Error> {
            let name = hostfns::guest_str(&mut c, np, nl).map_err(HostErr::into_err)?;
            hostfns::add_marker_max(&mut c, player_id, &name, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "apply_abnormal",
        |mut c: C<W>, player_id: i32, kind: i32, n: i32, to: i32| -> Result<(), Error> {
            hostfns::apply_abnormal(&mut c, player_id, kind, n, to).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "raise_after",
        |mut c: C<W>, player_id: i32, np: i32, nl: i32, value: i32| -> Result<(), Error> {
            let name = hostfns::guest_str(&mut c, np, nl).map_err(HostErr::into_err)?;
            hostfns::raise_after(&mut c, player_id, &name, value).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "skip_main_move",
        |mut c: C<W>, player_id: i32, locked: i32| -> Result<(), Error> {
            hostfns::skip_main_move(&mut c, player_id, locked).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "clear_move_skip",
        |mut c: C<W>, player_id: i32| -> Result<(), Error> {
            hostfns::clear_move_skip(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "give_extra_turn",
        |mut c: C<W>, player_id: i32| -> Result<(), Error> {
            hostfns::give_extra_turn(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "end_turn_now",
        |mut c: C<W>, player_id: i32| -> Result<(), Error> {
            hostfns::end_turn_now(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "teleport_as",
        |mut c: C<W>, player_id: i32, tile: i32, resolve: i32, as_main: i32| -> Result<(), Error> {
            hostfns::teleport_as(&mut c, player_id, tile, resolve, as_main)
                .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "log_as",
        |mut c: C<W>, player_id: i32, kp: i32, kn: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::log_as(&mut c, player_id, kp, kn, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "can_pay",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::can_pay(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "cant_move",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::cant_move(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "turn_player", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::turn_player(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "round_no", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::round_no(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "turn_key", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::turn_key(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "persona_is",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::persona_is(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_persona",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::set_persona(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "in_group",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::in_group(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tok_names",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::tok_names(&mut c, player_id, p, n, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "unplace_card_named",
        |mut c: C<W>, player_id: i32, cptr: i32, cl: i32| -> Result<i32, Error> {
            hostfns::unplace_card_named(&mut c, player_id, cptr, cl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "can_build_on",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            hostfns::can_build_on(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_face_down",
        |mut c: C<W>, player_id: i32, cptr: i32, cl: i32| -> Result<i32, Error> {
            hostfns::card_face_down(&mut c, player_id, cptr, cl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_extreme",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_extreme(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "play_from_hand", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::play_from_hand(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "gain_fixed",
        |mut c: C<W>, player_id: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::gain_fixed(&mut c, player_id, amount, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_card_face_down",
        |mut c: C<W>, player_id: i32, cptr: i32, cl: i32, down: i32| -> Result<i32, Error> {
            hostfns::set_card_face_down(&mut c, player_id, cptr, cl, down)
                .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "clear_dice", |mut c: C<W>| -> Result<(), Error> {
        hostfns::clear_dice(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_card_immune",
        |mut c: C<W>, player_id: i32, cptr: i32, cl: i32, on: i32| -> Result<i32, Error> {
            hostfns::set_card_immune(&mut c, player_id, cptr, cl, on).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_immune",
        |mut c: C<W>, player_id: i32, cptr: i32, cl: i32| -> Result<i32, Error> {
            hostfns::card_immune(&mut c, player_id, cptr, cl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_card_tile",
        |mut c: C<W>, player_id: i32, cptr: i32, cl: i32, tile: i32| -> Result<i32, Error> {
            hostfns::set_card_tile(&mut c, player_id, cptr, cl, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "place_card_on",
        |mut c: C<W>,
         player_id: i32,
         tile: i32,
         cptr: i32,
         cl: i32,
         p: i32,
         n: i32|
         -> Result<i32, Error> {
            hostfns::place_card_on(&mut c, player_id, tile, cptr, cl, p, n)
                .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "place_card_at",
        |mut c: C<W>, player_id: i32, cptr: i32, cl: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::place_card_at(&mut c, player_id, cptr, cl, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_dest",
        |mut c: C<W>, dest: i32| -> Result<(), Error> {
            hostfns::set_dest(&mut c, dest).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_transfer_to_dest",
        |mut c: C<W>, to: i32, dest: i32| -> Result<(), Error> {
            hostfns::set_transfer_to_dest(&mut c, to, dest).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "send_to_dest",
        |mut c: C<W>, dest: i32| -> Result<i32, Error> {
            hostfns::send_to_dest(&mut c, dest).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "transfer_to_dest",
        |mut c: C<W>, to: i32, dest: i32| -> Result<i32, Error> {
            hostfns::transfer_to_dest(&mut c, to, dest).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "rent_unit_mult", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::rent_unit_mult(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "add_rent_unit_bonus",
        |mut c: C<W>, n: i32| -> Result<i32, Error> {
            hostfns::add_rent_unit_bonus(&mut c, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "teleport_to",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<(), Error> {
            hostfns::teleport_to(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "gate",
        |mut c: C<W>, player_id: i32, kind: i32| -> Result<i32, Error> {
            hostfns::gate(&mut c, player_id, kind).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "abnormal_count",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::abnormal_count(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "targeted_count",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::targeted_count(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "gains_this_turn",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::gains_this_turn(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "designations",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::designations(&mut c, player_id, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "cancel_designation",
        |mut c: C<W>, seat: i32| -> Result<(), Error> {
            hostfns::cancel_designation(&mut c, seat).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "designation_cancelled",
        |mut c: C<W>, seat: i32| -> Result<i32, Error> {
            hostfns::designation_cancelled(&mut c, seat).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "target",
        |mut c: C<W>, player_id: i32, tile: i32, single: i32| -> Result<i32, Error> {
            hostfns::target(&mut c, player_id, tile, single).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_move",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::card_move(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_settle_at",
        |mut c: C<W>, player_id: i32, tile: i32, main: i32| -> Result<i32, Error> {
            hostfns::card_settle_at(&mut c, player_id, tile, main).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_buy",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            hostfns::card_buy(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "buy",
        |mut c: C<W>, player_id: i32, tile: i32, kind: i32| -> Result<i32, Error> {
            hostfns::buy(&mut c, player_id, tile, kind).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "buy_quotes",
        |mut c: C<W>,
         player_id: i32,
         kind: i32,
         buf: i32,
         n: i32,
         out: i32|
         -> Result<i32, Error> {
            hostfns::buy_quotes(&mut c, player_id, kind, buf, n, out).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "ai_agent_choice",
        |mut c: C<W>, player_id: i32, buf: i32, n: i32| -> Result<i32, Error> {
            hostfns::ai_agent_choice(&mut c, player_id, buf, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "acquire",
        |mut c: C<W>, player_id: i32, from: i32, tile: i32, price: i32| -> Result<i32, Error> {
            hostfns::acquire(&mut c, player_id, from, tile, price).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "agent_offer",
        |mut c: C<W>, player_id: i32, agent: i32, tile: i32, kind: i32| -> Result<i32, Error> {
            hostfns::agent_offer(&mut c, player_id, agent, tile, kind).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "linger",
        |mut c: C<W>, player_id: i32, expires: i32| -> Result<i32, Error> {
            hostfns::linger(&mut c, player_id, expires).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_build",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            hostfns::card_build(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_offer_build",
        |mut c: C<W>, player_id: i32, buf: i32, n: i32| -> Result<i32, Error> {
            hostfns::card_offer_build(&mut c, player_id, buf, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_mortgage",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            hostfns::card_mortgage(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "apply_mortgage",
        |mut c: C<W>, player_id: i32, tile: i32, amount: i32| -> Result<i32, Error> {
            hostfns::apply_mortgage(&mut c, player_id, tile, amount).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "apply_redeem",
        |mut c: C<W>, player_id: i32, tile: i32, amount: i32| -> Result<i32, Error> {
            hostfns::apply_redeem(&mut c, player_id, tile, amount).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "pay_buy",
        |mut c: C<W>, player_id: i32, tile: i32, amount: i32, kind: i32| -> Result<i32, Error> {
            hostfns::pay_buy(&mut c, player_id, tile, amount, kind).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "pay_build",
        |mut c: C<W>, player_id: i32, tile: i32, amount: i32| -> Result<i32, Error> {
            hostfns::pay_build(&mut c, player_id, tile, amount).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "assign_deed",
        |mut c: C<W>,
         player_id: i32,
         tile: i32,
         owner: i32,
         houses: i32,
         mortgaged: i32|
         -> Result<i32, Error> {
            hostfns::assign_deed(&mut c, player_id, tile, owner, houses, mortgaged)
                .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "placed_tile",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::placed_tile(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "play_doubled", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::play_doubled(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_play_doubled",
        |mut c: C<W>, n: i32| -> Result<(), Error> {
            hostfns::set_play_doubled(&mut c, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_cards",
        |mut c: C<W>, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::trig_cards(&mut c, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "opt_int", |mut c: C<W>, v: i32| -> Result<(), Error> {
        hostfns::opt_int(&mut c, v).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "opt_str",
        |mut c: C<W>, p: i32, n: i32| -> Result<(), Error> {
            hostfns::opt_str(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "opt_tile",
        |mut c: C<W>, tile: i32, lp: i32, ll: i32| -> Result<(), Error> {
            hostfns::opt_tile(&mut c, tile, lp, ll).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "opt_price",
        |mut c: C<W>, price: i32| -> Result<(), Error> {
            hostfns::opt_price(&mut c, price).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "opt_ai", |mut c: C<W>, ai: i32| -> Result<(), Error> {
        hostfns::opt_ai(&mut c, ai).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "opt_count", |mut c: C<W>, n: i32| -> Result<(), Error> {
        hostfns::opt_count(&mut c, n).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "ask",
        |mut c: C<W>,
         kind: i32,
         player_id: i32,
         tp: i32,
         tl: i32,
         xp: i32,
         xl: i32|
         -> Result<i32, Error> {
            hostfns::ask(&mut c, kind, player_id, tp, tl, xp, xl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_kind",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::trig_kind(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "trig_player", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_player(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_target", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_target(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_tile", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_tile(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_value", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_value(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_step", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_step(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_by_card", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_by_card(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_pay_is_rent", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_pay_is_rent(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_move_kind", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_move_kind(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "trig_move_resolve",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_move_resolve(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_move_tag",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::trig_move_tag(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "trig_move_main", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_move_main(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "trig_settle_redirect",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_settle_redirect(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "trig_move_dir", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_move_dir(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "trig_move_remaining",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_move_remaining(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "trig_move_from", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_move_from(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_move_total", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_move_total(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_move_roll", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_move_roll(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_roll_source", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_roll_source(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_buy_kind", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_buy_kind(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_seller", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_seller(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_price", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_price(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "trig_set_price",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::trig_set_price(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "trig_deal_owner", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_deal_owner(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "trig_set_deal_owner",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::trig_set_deal_owner(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "trig_deal_houses", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_deal_houses(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "trig_set_deal_houses",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::trig_set_deal_houses(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_deal_mortgaged",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_deal_mortgaged(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_deal_mortgaged",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::trig_set_deal_mortgaged(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_reason",
        |mut c: C<W>, p: i32, n: i32| -> Result<(), Error> {
            hostfns::trig_set_reason(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_move_roll",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::trig_set_move_roll(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_pay_amount",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::trig_set_pay_amount(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_pay_target",
        |mut c: C<W>, to: i32| -> Result<(), Error> {
            hostfns::trig_set_pay_target(&mut c, to).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_cancelled",
        |mut c: C<W>| -> Result<(), Error> {
            hostfns::trig_set_cancelled(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_negate_effect",
        |mut c: C<W>| -> Result<(), Error> {
            hostfns::trig_set_negate_effect(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_spare",
        |mut c: C<W>, seat: i32| -> Result<(), Error> {
            hostfns::trig_set_spare(&mut c, seat).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "trig_cancelled", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_cancelled(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_seq", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_seq(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "trig_answers", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::trig_answers(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "trig_effect_count",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_effect_count(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_effect_kind",
        |mut c: C<W>, i: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::trig_effect_kind(&mut c, i, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_effect_target",
        |mut c: C<W>, i: i32| -> Result<i32, Error> {
            hostfns::trig_effect_target(&mut c, i).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_effect_from",
        |mut c: C<W>, i: i32| -> Result<i32, Error> {
            hostfns::trig_effect_from(&mut c, i).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_effect_tile",
        |mut c: C<W>, i: i32| -> Result<i32, Error> {
            hostfns::trig_effect_tile(&mut c, i).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_effect_value",
        |mut c: C<W>, i: i32| -> Result<i32, Error> {
            hostfns::trig_effect_value(&mut c, i).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "declare_effect",
        |mut c: C<W>,
         kp: i32,
         kl: i32,
         target: i32,
         from: i32,
         tile: i32,
         value: i32|
         -> Result<(), Error> {
            let kind = hostfns::guest_str(&mut c, kp, kl).map_err(HostErr::into_err)?;
            hostfns::declare_effect(&mut c, &kind, target, from, tile, value)
                .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_card_is",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::trig_card_is(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "play_card",
        |mut c: C<W>, p: i32, n: i32, player_id: i32| -> Result<i32, Error> {
            hostfns::play_card(&mut c, p, n, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "query_guard",
        |mut c: C<W>,
         player_id: i32,
         p: i32,
         n: i32,
         ep: i32,
         el: i32|
         -> Result<i32, Error> {
            hostfns::query_guard(&mut c, player_id, p, n, ep, el).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "declared_arg",
        |mut c: C<W>, out_ptr: i32, cap: i32| -> Result<i32, Error> {
            hostfns::declared_arg(&mut c, out_ptr, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "schedule_turn_end",
        |mut c: C<W>, player_id: i32, mode: i32, id_ptr: i32, id_len: i32| -> Result<(), Error> {
            hostfns::schedule_turn_end(&mut c, player_id, mode, id_ptr, id_len)
                .map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_no_money_loss",
        |mut c: C<W>, player_id: i32| -> Result<(), Error> {
            hostfns::set_no_money_loss(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_fixed_roll",
        |mut c: C<W>, n: i32| -> Result<(), Error> {
            hostfns::set_fixed_roll(&mut c, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "fixed_roll", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::fixed_roll(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_next_steps",
        |mut c: C<W>, player_id: i32, n: i32| -> Result<(), Error> {
            hostfns::set_next_steps(&mut c, player_id, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "turn_main_steps", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::turn_main_steps(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_roller",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_roller(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "set_steps", |mut c: C<W>, v: i32| -> Result<(), Error> {
        hostfns::set_steps(&mut c, v).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_reverse",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_reverse(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_signed",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_signed(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_stop_at",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_stop_at(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_parity",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_parity(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_resolve",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_resolve(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_no_buy",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_no_buy(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_no_build",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_no_build(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_build_anywhere",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_build_anywhere(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "set_kind", |mut c: C<W>, v: i32| -> Result<(), Error> {
        hostfns::set_kind(&mut c, v).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_teleport_to",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_teleport_to(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_start",
        |mut c: C<W>, t: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::set_start(&mut c, t, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_base_dice",
        |mut c: C<W>, count: i32, sides: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::set_base_dice(&mut c, count, sides, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_base_dice",
        |mut c: C<W>, count: i32, sides: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::add_base_dice(&mut c, count, sides, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_extra_dice",
        |mut c: C<W>, count: i32, sides: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::add_extra_dice(&mut c, count, sides, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_tag",
        |mut c: C<W>, kp: i32, kl: i32, v: i32| -> Result<(), Error> {
            hostfns::set_tag(&mut c, kp, kl, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_tag_id",
        |mut c: C<W>, key_id: i32, v: i32| -> Result<(), Error> {
            hostfns::set_tag_id(&mut c, key_id, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "recorded_roll", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::recorded_roll(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "set_min_roll",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_min_roll(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_extra_steps",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_extra_steps(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_settle_tile",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_settle_tile(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_pay_factor",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_pay_factor(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_rent_factor",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_rent_factor(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_can_build",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_can_build(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_settle_as_agent",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_settle_as_agent(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "plan_add_follower",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::plan_add_follower(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_more_steps",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_more_steps(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "move_stop_at", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::move_stop_at(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "move_stopped", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::move_stopped(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "move_parity", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::move_parity(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "move_resolve", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::move_resolve(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "move_steps", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::move_steps(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "move_remaining", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::move_remaining(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "move_total", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::move_total(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(m, "move_dir", |mut c: C<W>| -> Result<i32, Error> {
        hostfns::move_dir(&mut c).map_err(HostErr::into_err)
    })?;
    l.func_wrap(
        m,
        "join_redirect_target",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::join_redirect_target(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(m, "set_join_used", |mut c: C<W>| -> Result<(), Error> {
        hostfns::set_join_used(&mut c).map_err(HostErr::into_err)
    })?;

    Ok(l)
}

#[cfg(feature = "sandbox")]
impl HostState<NullWorld> {
    fn without_rules() -> Self {
        Self {
            rules: None,
            world: Some(NullWorld),
            answers: vec![],
            next_answer: 0,
            options: vec![],
            prices: vec![],
            ai_hint: -1,
            count: 0,
            asked: None,
            host_request: None,
            declared: Vec::new(),
            depth: 0,
            msg_sender_uid: -1,
            msg_sender_seat: -1,
            msg_name: String::new(),
            msg: card_sdk::abi::Message::default(),
            msg_reply: 0,
            #[cfg(feature = "sandbox")]
            limits: store_limits(),
        }
    }
}

/// World used only while reading module metadata.
#[derive(Clone)]
#[cfg(feature = "sandbox")]
struct NullWorld;

#[cfg(feature = "sandbox")]
impl CardWorld for NullWorld {
    fn place_card_on(
        &mut self,
        _player_id: i32,
        _tile: i32,
        _card: &str,
        _note: crate::Msg,
    ) -> i32 {
        -1
    }
    // keyed state: storage the null world does not have
    fn state_var(&self, _player_id: i32, _key: &str) -> game_core::state::StateVar {
        game_core::state::StateVar::default()
    }
    fn state_get(&self, _player_id: i32, _key: &str) -> i32 {
        0
    }
    fn state_pairs(&self, _player_id: i32) -> Vec<(String, i32)> {
        Vec::new()
    }
    fn state_set_flags(&mut self, _player_id: i32, _key: &str, _flags: u8) {}
    fn raise_after(&mut self, _player_id: i32, _name: &str, _value: i32) {}
    fn state_min(&self, _player_id: i32, _key: &str) -> i32 {
        0
    }
    fn state_max(&self, _player_id: i32, _key: &str) -> i32 {
        0
    }
    fn state_expires(&self, _player_id: i32, _key: &str) -> Option<game_core::state::Tick> {
        None
    }
    fn state_set(&mut self, _player_id: i32, _key: &str, value: i32) -> i32 {
        value
    }
    fn state_add(&mut self, _player_id: i32, _key: &str, delta: i32) -> i32 {
        delta
    }
    fn state_set_bounds(&mut self, _player_id: i32, _key: &str, _min: i32, _max: i32) {}
    fn state_set_expires(
        &mut self,
        _player_id: i32,
        _key: &str,
        _expires: Option<game_core::state::Tick>,
    ) {
    }
    fn tick_state(&mut self, _player_id: i32, _when: game_core::state::Tick) -> Vec<(String, i32)> {
        Vec::new()
    }

    fn roll(&mut self, _: i32, _: i32, _: i32) -> i32 {
        1
    }
    fn log(&mut self, _: i32, _: crate::Msg) {}
    fn tile_count(&self) -> i32 {
        1
    }
    fn tile_named(&self, _: &str) -> i32 {
        -1
    }
    fn tile_owner(&self, _: i32) -> i32 {
        -1
    }
    fn player_pos(&self, _: i32) -> i32 {
        -1
    }
    fn tile_steps_ahead(&self, _: i32, _: i32) -> i32 {
        -1
    }
    fn rent_of(&self, _: i32) -> i32 {
        0
    }
    fn buy_price(&self, _: i32) -> i32 {
        0
    }
    fn build_cost(&self, _: i32) -> i32 {
        0
    }
    fn mortgage_value(&self, _: i32) -> i32 {
        0
    }
    fn owned_count(&self, _: i32) -> i32 {
        0
    }
    fn owned_at(&self, _: i32, _: i32) -> i32 {
        -1
    }
    fn player_count(&self) -> i32 {
        1
    }
    fn player_out(&self, _: i32) -> i32 {
        0
    }
    fn others_count(&self, _: i32) -> i32 {
        0
    }
    fn others_at(&self, _: i32, _: i32) -> i32 {
        -1
    }
    fn money(&self, _: i32) -> i32 {
        0
    }
    fn gain(&mut self, _: i32, _: i32, _: crate::Msg) -> i32 {
        0
    }
    fn pay(&mut self, _: i32, _: i32, _: crate::Msg) -> i32 {
        0
    }
    fn draw(&mut self, _: i32, _: i32) -> i32 {
        0
    }
    fn add_to_dest(&mut self, _: i32, _: &str, _: i32) {}
    fn take_card(&mut self, _: i32, _: &str, _: crate::CardPile) -> bool {
        false
    }
    fn cards_in(&self, _: i32, _: crate::CardPile) -> Vec<String> {
        Vec::new()
    }
    fn to_discard(&mut self, _: i32, _: &str) {}
    fn place_card(&mut self, _: i32, _: &str, _: crate::Msg) -> i32 {
        -1
    }
    fn unplace_card(&mut self) -> i32 {
        -1
    }
    fn is_placed(&self) -> i32 {
        0
    }
    // named counters & bound units (user ruling 2026-10-10): the null world
    // holds none, so every read is 0 and every write is a no-op. The sugar
    // (named counters / `tok` / `group_pool`) defaults over these.
    fn counter(&self, _: &str) -> i32 {
        0
    }
    fn set_counter(&mut self, _: &str, _: i32) -> i32 {
        0
    }
    fn add_counter(&mut self, _: &str, _: i32, _: i32) -> i32 {
        0
    }
    fn counter_at(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn add_counter_at(&mut self, _: i32, _: &str, _: i32, _: i32) -> i32 {
        0
    }
    fn place_mark(
        &mut self,
        _: i32,
        _: &str,
        _: &str,
        _: i32,
        _: i32,
        _: i32,
        _: crate::Msg,
        _: bool,
    ) -> i32 {
        -1
    }
    fn count_marks(&self, _: i32, _: &game_core::state::MarkFilter<'_>) -> i32 {
        0
    }
    fn bump_mark(&mut self, _: i32, _: &game_core::state::MarkFilter<'_>, _: i32) -> i32 {
        0
    }
    fn remove_marks(&mut self, _: i32, _: &game_core::state::MarkFilter<'_>) -> i32 {
        0
    }
    fn mark_src_at(&self, _: i32, _: &game_core::state::MarkFilter<'_>) -> i32 {
        -1
    }
    fn mark_instance_at(&self, _: i32, _: &game_core::state::MarkFilter<'_>) -> i32 {
        -1
    }
    fn count_held(&self, _: &str, _: i32) -> i32 {
        0
    }
    fn add_held(&mut self, _: &str, _: i32, _: i32, _: i32) -> i32 {
        0
    }
    fn set_held_name(&mut self, _: &str, _: i32, _: i32, _: i32, _: i32) {}
    fn move_units(&mut self, _: &str, _: i32, _: i32, _: i32, _: i32, _: i32) -> i32 {
        0
    }
    fn self_uid(&self) -> i32 {
        -1
    }
    fn group_pool(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn add_group_pool(&mut self, _: i32, _: &str, _: i32, _: i32) -> i32 {
        0
    }
    fn gain_marker(&mut self, _: i32, _: &str, _: i32, _: crate::Msg) -> i32 {
        0
    }
    fn apply_abnormal(&mut self, _: i32, _: i32, _: i32, _: i32) {}
    fn skip_main_move(&mut self, _: i32, _: i32) {}
    fn give_extra_turn(&mut self, _: i32) {}
    fn end_turn_now(&mut self, _: i32) {}
    fn teleport_as(&mut self, _: i32, _: i32, _: i32, _: i32) {}
    fn log_as(&mut self, _: i32, _: &str, _: crate::Msg) {}
    fn set_dest(&mut self, _: i32) {}
    fn set_transfer_to_dest(&mut self, _: i32, _: i32) {}
    fn send_to_dest(&mut self, _: i32) -> Option<i32> {
        None
    }
    fn transfer_to_dest(&mut self, _: i32, _: i32) -> Option<i32> {
        None
    }
    fn rent_unit_mult(&self) -> i32 {
        10
    }
    fn add_rent_unit_bonus(&mut self, _: i32) -> i32 {
        0
    }
    fn teleport_to(&mut self, _: i32, _: i32) {}
    fn trigger(&self) -> crate::Trigger {
        crate::Trigger::default()
    }
    fn set_trigger_move_roll(&mut self, _: i32) {}
    fn set_trigger_value(&mut self, _: i32) {}
    fn set_trigger_target(&mut self, _: i32) {}
    fn set_trigger_cancelled(&mut self) {}
    fn set_trigger_negate_effect(&mut self) {}
    fn set_trigger_spare(&mut self, _: i32) {}
    fn declare_trigger_effect(&mut self, _: &str, _: i32, _: i32, _: i32, _: i32) {}
    fn trig_card_is(&self, _: &str) -> i32 {
        0
    }
    fn set_trigger_price(&mut self, _: i32) {}
    fn set_trigger_deal_owner(&mut self, _: Option<game_kinds::Payee>) {}
    fn set_trigger_deal_houses(&mut self, _: Option<i32>) {}
    fn set_trigger_deal_mortgaged(&mut self, _: i32) {}
    fn set_trigger_reason(&mut self, _: &str) {}
    fn tile_group(&self, _: i32) -> i32 {
        -1
    }
    fn tile_price(&self, _: i32) -> i32 {
        0
    }
    fn houses_of(&self, _: i32) -> i32 {
        0
    }
    fn set_houses(&mut self, _: i32, _: i32) {}
    fn add_house(&mut self, _: i32, _: i32) -> i32 {
        0
    }
    fn mortgaged_of(&self, _: i32) -> i32 {
        0
    }
    fn set_mortgaged(&mut self, _: i32, _: i32) {}
    fn set_owner(&mut self, _: i32, _: i32) {}
    fn dist(&self, _: i32, _: i32) -> i32 {
        0
    }
    fn tile_forward(&self, _: i32, _: i32) -> i32 {
        0
    }
    fn neighbor(&self, _: i32, _: i32) -> i32 {
        -1
    }
    fn players_on_count(&self, _: i32, _: i32) -> i32 {
        0
    }
    fn players_on_at(&self, _: i32, _: i32, _: i32) -> i32 {
        -1
    }
    fn hand_count(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn hand_size(&self, _: i32) -> i32 {
        0
    }
    fn discard_count(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn deck_count(&self, _: i32) -> i32 {
        0
    }
    fn discard_size(&self, _: i32) -> i32 {
        0
    }
    fn discard_from_hand(&mut self, _: i32, _: &str) -> i32 {
        0
    }
    fn shuffle_into_deck(&mut self, _: i32, _: bool, _: bool) -> i32 {
        0
    }
    fn can_pay(&self, _: i32) -> i32 {
        0
    }
    fn cant_move(&self, _: i32) -> i32 {
        1
    }
    fn spend_marker(&mut self, _: i32, _: &str, _: i32, _: crate::Msg) -> i32 {
        0
    }
    fn add_marker_max(&mut self, _: i32, _: &str, _: i32) -> i32 {
        0
    }
    fn turn_player(&self) -> i32 {
        -1
    }
    fn round_no(&self) -> i32 {
        0
    }
    fn turn_key(&self) -> i32 {
        0
    }
    fn persona_is(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn set_persona(&mut self, _: i32, _: &str) {}
    fn in_group(&self, _: i32, _: &str) -> i32 {
        0
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    fn card(on: Vec<ManifestOn>) -> CardInfo {
        CardInfo {
            id: "t:gate".into(),
            on,
            props: Default::default(),
            texts: Default::default(),
            board_role: Default::default(),
            governs: Default::default(),
            gate_msgs: Default::default(),
            gate_skip_msgs: Default::default(),
            exemption_prop: Default::default(),
        }
    }

    fn gate(has_guard: bool, pre: Option<&str>) -> ManifestOn {
        ManifestOn {
            kind: OnKind::Filter as i32,
            triggers: vec![crate::TriggerKind::PayGate.spelling().into_owned()],
            pre: pre.map(str::to_string),
            has_guard,
            has_declare: false,
            id: None,
            messages: vec![],
            label: None,
            presentation: Default::default(),
            flash: Default::default(),
        }
    }

    /// A capability gate must be CEL-only: a residual guard or a missing
    /// condition is invisible to a pure read, so the load refuses it.
    #[test]
    fn gates_must_be_cel_only() {
        assert!(validate_cel_only_gates(&[card(vec![gate(false, Some("true"))])]).is_ok());
        assert!(validate_cel_only_gates(&[card(vec![gate(true, Some("true"))])]).is_err());
        assert!(validate_cel_only_gates(&[card(vec![gate(false, None)])]).is_err());
    }
}
