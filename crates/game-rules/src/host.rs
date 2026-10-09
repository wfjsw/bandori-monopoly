//! wasmi host: loading a set of card modules, the `bandori` import table, replay,
//! and nested cross-module card calls.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use card_sdk::abi::{
    self, export, AbKind, ManifestOn, OnKind, PromptKind, ABI_VERSION, IMPORT_MODULE,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
// The engine behind the host: wasmi (an interpreter) in the browser, wasmtime
// (the optimizing compiler) on the server. Same modules and same behaviour.
#[cfg(any(target_arch = "wasm32", feature = "wasmi-native"))]
#[path = "be_wasmi.rs"]
mod be;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "wasmi-native")))]
#[path = "be_wasmtime.rs"]
mod be;

use be::{
    err, error_text, has_func, instantiate, is_need_input, need_input, read_guest, read_mem,
    set_fuel, write_guest,
};
use be::{Caller, Engine, Error, Linker, Module, Store};

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
    /// [反击] window opens: `hand_counteractions` rings that passed the
    /// valid-option-first pre-scan (at least one candidate survived the kind
    /// bitmask + seat eligibility + condition).
    pub static COUNTERACT_WINDOWS: AtomicU64 = AtomicU64::new(0);
    /// [反击] windows skipped whole because no candidate could respond.
    pub static COUNTERACT_WINDOWS_SKIPPED: AtomicU64 = AtomicU64::new(0);
    /// Per-candidate counteraction probes (condition / guard evaluations).
    pub static COUNTERACT_PROBES: AtomicU64 = AtomicU64::new(0);
    /// Probes answered from the per-window processed set (no re-evaluation).
    pub static COUNTERACT_PROBE_MEMO_HITS: AtomicU64 = AtomicU64::new(0);
    /// [反击] cards actually declared (left a hand for a counter link).
    pub static COUNTERACT_DECLARED: AtomicU64 = AtomicU64::new(0);
}

/// Fuel per top-level effect run, shared by any nested `play_card` calls. Plenty
/// for straight-line card logic; stops runaway loops at the same instruction on
/// every machine.
pub const DEFAULT_FUEL: u64 = 5_000_000;

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
    /// serialization stay deterministic.
    #[serde(default)]
    pub props: std::collections::BTreeMap<String, i32>,
}

impl CardInfo {
    /// The first entry of `kind` (answering `trigger`, for trigger-keyed kinds).
    pub fn entry(&self, kind: OnKind, trigger: Option<crate::TriggerKind>) -> Option<i32> {
        self.on
            .iter()
            .position(|o| {
                o.kind == kind as i32 && trigger.is_none_or(|t| o.triggers.contains(&(t as i32)))
            })
            .map(|i| i as i32)
    }

    pub fn has_play(&self) -> bool {
        self.entry(OnKind::Play, None).is_some()
    }

    /// Declares a [反击] at this trigger kind.
    pub fn counteracts_to(&self, trigger: crate::TriggerKind) -> bool {
        self.entry(OnKind::Counteract, Some(trigger)).is_some()
    }

    /// Declares a field-card hook at this trigger kind.
    pub fn hooks(&self, trigger: crate::TriggerKind) -> bool {
        self.hook_entry(trigger).is_some()
    }

    /// The field-card entry that answers `trigger`, whether it was declared as a
    /// settlement hook (`On::Hook`) or as a gate (`On::Gate`). Both run
    /// automatically on a placed card and neither opens a [反击] window; they
    /// are separate declarations only so the type can say which is a question.
    pub fn hook_entry(&self, trigger: crate::TriggerKind) -> Option<i32> {
        self.entry(OnKind::Hook, Some(trigger))
            .or_else(|| self.entry(OnKind::Gate, Some(trigger)))
    }

    pub fn has_at_end(&self) -> bool {
        self.entry(OnKind::AtEnd, None).is_some()
    }
}

/// What to run. `Ruleset` resolves it to the card's matching entry point; a
/// card without one simply does nothing.
#[derive(Debug, Clone, Copy)]
pub enum Call {
    /// `On::Play` -- the effect when played from hand.
    Play { card: i32, player_id: i32 },
    /// `On::Counteract` -- the counteraction effect, for the world's current trigger kind.
    Counteract { card: i32, player_id: i32 },
    /// `On::Hook` -- the field-card hook for `kind`.
    Hook {
        card: i32,
        kind: crate::TriggerKind,
        player_id: i32,
    },
    /// `On::AtEnd` -- the scheduled turn-end callback.
    AtEnd { card: i32, player_id: i32 },
    /// `On::RollPlan` -- the card's movement routine, called while the walk
    /// is being planned.
    RollPlan { card: i32, player_id: i32 },
    /// `On::Settle` -- a rule's **settle body** (`docs/TILES.md`), run for the
    /// tile the rule instance governs. `player_id` is the player settling.
    Settle { card: i32, player_id: i32 },
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
            | Call::RollPlan { player_id, .. }
            | Call::Settle { player_id, .. } => player_id,
        }
    }

    pub fn card(&self) -> i32 {
        match *self {
            Call::Play { card, .. }
            | Call::Counteract { card, .. }
            | Call::Hook { card, .. }
            | Call::AtEnd { card, .. }
            | Call::RollPlan { card, .. }
            | Call::Settle { card, .. } => card,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptOption {
    Int(i32),
    /// A labelled option (`ask_pick`).
    Str(crate::Msg),
}

/// A prompt the effect is blocked on. Maps onto the C# `MatchPrompt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub kind: PromptKind,
    pub player_id: i32,
    pub title: crate::Msg,
    pub text: crate::Msg,
    pub options: Vec<PromptOption>,
    /// Position in the effect's answer log this prompt will fill.
    pub answer_slot: usize,
}

/// Engine work a card effect needs mid-run (it may open [反击] windows or
/// prompt), paused so the engine can do it before the effect continues:
/// trigger (and the [反击] window) before the money moves. The engine answers
/// with the final amount to move -- `0` means the payment was cancelled.
// `Move` carries a `MoveCtx`, which has `f64` factors: no `Eq`.
#[derive(Debug, Clone, PartialEq)]
pub enum HostRequest {
    /// A payment (`t.Pay`): the engine runs the C# `Money` pipeline on it and
    /// answers with the final amount (0 = cancelled). `from < 0` is a print
    /// (game -> `to`), `to < 0` a delete (`from` -> game), both >= 0 a
    /// pay-player. `src` is the reason line the money log carries.
    Pay {
        from: i32,
        to: i32,
        amount: i32,
        src: Option<crate::Msg>,
        /// Run the command-wide **pre-split** stage (`payTotalAdd` /
        /// `payTotalMul` / `payTotalCancel`) on this payment's amount. `true`
        /// for every ordinary payment; a 「[分摊]」 leg sets it `false` because
        /// the card body has already shaped the command total through
        /// [`Self::PayTotal`] before dividing (`PIPELINE-AUDIT` Q2).
        total_stage: bool,
    },
    /// The command-wide **pre-split** stage alone (`PIPELINE-AUDIT` Q2): the
    /// engine runs `payTotalAdd` → `payTotalMul` → `payTotalCancel` on `total`
    /// and answers with the shaped figure, or `-1` when a `payTotalCancel`
    /// hook dropped the whole command. No money moves here -- the card body
    /// divides the answer and runs each share through [`Self::Pay`] with
    /// `total_stage: false`.
    PayTotal {
        from: i32,
        to: i32,
        amount: i32,
        src: Option<crate::Msg>,
    },
    /// An abnormal effect about to hit `player_id`: the engine runs the C#
    /// `AbnormalGate` and answers 1 (it goes through) or 0 (blocked).
    Gate { player_id: i32, kind: AbKind },
    /// A card targeting a player (`tile < 0`) or a tile: the engine runs the C#
    /// `H.Target` / `H.TargetTile` pipeline and answers with the player (or tile)
    /// actually targeted, or -1 when the targeting failed. `single` lets a
    /// field `redirect` hook move the hit (C# `CardDef.SingleTarget`).
    Target {
        player_id: i32,
        tile: i32,
        single: bool,
    },
    /// C# `H.CardMove(c, m)`: the card shaped the move (via the plan ops) and
    /// asked for it to run now. The engine runs `Cx::card_move` -- it may open
    /// prompts -- and the effect then replays past this call. `plan` is the
    /// plan as the card left it.
    Move {
        player_id: i32,
        plan: game_core::engine::MoveCtx,
    },
    /// C# `H.ForceTeleport(..., resolve: false)` / a bare `pos` write: move a
    /// player with no settle and no move bookkeeping. The engine runs the
    /// `AbnormalGate` and applies the write to the **live** world, so a move
    /// that follows in the same effect starts at the destination -- and the
    /// replay skips the call (an answer is logged), so it cannot re-teleport
    /// over a move that already ran. Answers 1 (it went through) or 0 (the
    /// gate blocked it).
    Teleport { player_id: i32, tile: i32 },
    /// C# `H.AgentLanding`: the player lands on `agent` as a 「星光代理」 (the
    /// buy-or-pay-rent routine). Runs engine-side; the effect replays past it.
    AgentLanding { player_id: i32, agent: i32 },
    /// C# `H.SettleAt`: a full [触发结算] of `tile` for this player. The player
    /// does not move.
    SettleAt {
        player_id: i32,
        tile: i32,
        main: bool,
    },
    /// C# `H.BuyRoutine`: the purchase itself.
    Buy {
        player_id: i32,
        tile: i32,
        /// A [`card_sdk::abi::BuyKind`] as `i32` (`0` = land).
        kind: i32,
    },
    /// v40: batched purchase quote (`docs/PURCHASE.md`).
    BuyQuotes {
        player_id: i32,
        kind: i32,
        tiles: Vec<i32>,
        out: i32,
    },
    /// v40: 「收购」 -- take a deed from its owner at a price.
    Acquire {
        player_id: i32,
        from: i32,
        tile: i32,
        price: i32,
    },
    /// v40: the agent offer's chosen branch (buy / build).
    AgentOffer {
        player_id: i32,
        agent: i32,
        tile: i32,
        kind: i32,
    },
    /// v40: bind the running card as a turn-scoped lingering instance.
    Linger { player_id: i32, expires: i32 },
    /// C# `H.BuildRoutine`: pay and raise one house on `tile`.
    Build { player_id: i32, tile: i32 },
    /// C# `H.OfferBuildAmong`: prompt to build on one of `tiles`, then build.
    OfferBuild { player_id: i32, tiles: Vec<i32> },
    /// C# `H.MortgageRoutine`: mortgage one of the player's deeds.
    Mortgage { player_id: i32, tile: i32 },
    /// `H.DrawR`: draw `n` cards. The engine runs the draw itself -- one card at
    /// a time, raising the per-draw points (`drewBefore` / `drawn` / `drew`) on
    /// each, so a `drewBefore` hook may replace a card of it -- and answers with
    /// how many were actually drawn. The effect then replays past this call and
    /// does **not** move the cards again (they are already in the live world).
    Draw { player_id: i32, n: i32 },
    /// `H.DrawEvent` -- draw the top event and resolve it (「抽取一个事件卡」).
    /// The engine runs the whole event pipeline (public reveal, immediate
    /// effect, filing to the event discard, the reshuffle-when-empty half).
    /// `docs/TILES.md`'s `ctx::draw_event`, what `tile:event`'s body calls.
    DrawEvent { player_id: i32 },
    /// `H.PayRent`: 「[支付]拥有格子的玩家格子地契所标记的现等级地租」 -- the rent
    /// pipeline (rent table / RiNG dice / the agent's 「半价收费」 half-flag).
    /// `docs/TILES.md`'s `ctx::pay_rent`, what `tile:property` / `tile:ring` /
    /// `tile:agent` bodies call.
    PayRent {
        player_id: i32,
        tile: i32,
        half: bool,
    },
    /// `H.OfferBuy`: 「可选择[消耗]购买格子地契和建造已有房子的资金总价」 on a
    /// non-main landing on unowned land. `ctx::offer_buy`.
    OfferBuy { player_id: i32, tile: i32 },
    /// `H.OfferForceBuy`: 「可选择[支付]…资金总价的两倍，从该玩家处强行购买」
    /// on a mortgaged deed. `ctx::offer_force_buy`.
    OfferForceBuy { player_id: i32, tile: i32 },
    /// `H.OfferBuild`: 「可选择[消耗]…房屋建筑费进行升级建造」 on a non-main
    /// landing on one's own land. `ctx::offer_build`.
    OfferBuildOne { player_id: i32, tile: i32 },
    /// `H.CircleReward`: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]」
    /// -- the whole reward step (suppression, the choice, the `circleAffected`
    /// window, the payout). `docs/TILES.md`'s `ctx::settle_circle_reward`, the
    /// body of `tile:circle`'s Pass entry. `landing` picks the 「获得」 wording
    /// for a stop on CiRCLE as against a pass over it.
    CircleReward { player_id: i32, landing: bool },
    /// A card- or skill-driven dice roll (`ctx::roll_ask` / `ctx::do_move_roll_ask`).
    /// The engine rolls, raises the `Roll` chain link (the 「掷骰结算前」 [反击]
    /// window -- Y.O.L.O 「你的任意掷骰结算前」, 寄于指尖的执念 「当你使用火罐进行
    /// 掷骰时」) with the roller, the face and `source`, then answers with the
    /// face any counteraction left on the link. `source` is an
    /// [`crate::abi::roll_source`] code (0 = unattributed, 1 = a fire pot, ...).
    Roll {
        player_id: i32,
        count: i32,
        sides: i32,
        source: i32,
    },
    /// C# `f.Bought(i, t)` -- a card handed a deed over (tomoe_savior's
    /// 「从该玩家处收购该地契」) and is announcing the acquisition: the engine
    /// raises the `bought` hook chain over the field (`TriggerKind::Bought`),
    /// with `by_card` = the run's own player. `buy()` raises the same hook
    /// itself after an ordinary purchase; this is the card-driven half.
    RaiseBought { player_id: i32, tile: i32 },
    /// Marker spend / gain (user ruling 2026-10-07): the engine raises
    /// `markerSpend` / `markerGain` -- the marker's own [反击] window --
    /// **before** the markers move. Answers `1` (the move goes through) or `0`
    /// (a counteraction cancelled it, nothing moves). Marker *costs* otherwise
    /// keep today's timing: they are spent as the effect resolves and a
    /// whole-effect negation before the body already prevents them.
    Marker {
        player_id: i32,
        /// Marker name (「火罐」/「奇迹水晶」/「P✽P粉丝」/…). `fire` is the
        /// 火罐 pot shorthand (`key::FIRE`).
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
            } => write!(f, "card {card:?} entry {entry}: bad condition {source:?}: {err}"),
        }
    }
}

impl std::error::Error for RuleError {}

/// Result of one effect run.
#[derive(Debug)]
pub enum Outcome<W> {
    /// Finished: commit this world.
    Done(W),
    /// Blocked on a prompt with no answer yet. Nothing was committed.
    NeedInput(Prompt),
    /// Blocked on a host routine (buy/build/mortgage/move/pay/...) with no
    /// answer yet. The run's world rides along so the host can read the
    /// turn-ctx policy the card set up (`build_discount`, `lingering`, ...)
    /// before the routine runs against the live world -- the routine is not
    /// replayed, so those writes have to cross now.
    NeedHost(HostRequest, W),
}

/// One card's answer to a hook point, from [`Ruleset::run_hook`].
pub struct HookRun<W> {
    /// A guard existed and passed -- the one moment the card shows itself.
    /// `false` for a card with no guard at this kind (a gate, or an entry that
    /// simply runs), which runs without announcing.
    pub announced: bool,
    /// What the body did. The body ran on the same instantiation as the guard.
    pub outcome: Outcome<W>,
}

struct LoadedModule {
    module: Module,
    sha256: String,
}

/// Where a card lives.
#[derive(Clone, Copy)]
struct Slot {
    module: usize,
    /// Handle inside that module's manifest.
    local: i32,
}

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
    /// whole bridge when nothing in the set listens. `TriggerKind` values fit in
    /// 0..128, so this is one word and costs no allocation.
    declared: u128,
    /// Cached counteraction index (BOT-RESEARCH.md #1 / GUARDS.md G3): per-card
    /// bitmask of trigger kinds its `On::Counteract` entries answer. The offer
    /// loop tests one shift instead of scanning the entry table per probe.
    counteract_mask: Vec<u128>,
    /// Per-card bitmask of trigger kinds its hook/gate entries answer. Same
    /// shape as `counteract_mask`; the hook dispatch uses it to skip cards that
    /// can never fire at a kind.
    hook_mask: Vec<u128>,
}

/// Builds a [`Ruleset`] from any number of card modules (normally one per card).
pub struct RulesetBuilder {
    engine: Engine,
    modules: Vec<LoadedModule>,
    cards: Vec<CardInfo>,
    slots: Vec<Slot>,
    /// Lean compiled condition blobs (`Cond::to_bytes(false)`), keyed by
    /// `(card id, entry index)`. The runtime-only (browser) path fills these
    /// from the ruleset index instead of compiling -- see
    /// [`RulesetBuilder::precompiled`].
    precompiled: HashMap<(String, i32), Vec<u8>>,
}

impl RulesetBuilder {
    /// Compile and validate one card module (ABI version, exports, manifest) and add
    /// its cards. Returns the module's SHA-256.
    pub fn add(&mut self, wasm: &[u8]) -> Result<String, RuleError> {
        let (module, cards) = inspect(&self.engine, wasm)?;
        let sha256 = hex_sha256(wasm);
        let m = self.modules.len();
        for (local, c) in cards.into_iter().enumerate() {
            self.slots.push(Slot {
                module: m,
                local: local as i32,
            });
            self.cards.push(c);
        }
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
        let mut by_id = HashMap::new();
        for (i, c) in self.cards.iter().enumerate() {
            if by_id.insert(c.id.clone(), i as i32).is_some() {
                return Err(RuleError::DuplicateCard(c.id.clone()));
            }
        }
        let mut declared = 0u128;
        let mut counteract_mask = Vec::with_capacity(self.cards.len());
        let mut hook_mask = Vec::with_capacity(self.cards.len());
        for c in &self.cards {
            let mut cm = 0u128;
            let mut hm = 0u128;
            for o in &c.on {
                let ok = OnKind::from_i32(o.kind);
                for &k in &o.triggers {
                    if (0..128).contains(&k) {
                        declared |= 1u128 << k;
                        match ok {
                            Some(OnKind::Counteract) => cm |= 1u128 << k,
                            Some(OnKind::Hook) | Some(OnKind::Gate) => hm |= 1u128 << k,
                            _ => {}
                        }
                    }
                }
            }
            counteract_mask.push(cm);
            hook_mask.push(hm);
        }
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
        let mut parts: Vec<String> = self
            .modules
            .iter()
            .map(|m| m.sha256.clone())
            .collect();
        parts.sort_unstable();
        let conds = crate::cond_pre::PrecompiledConds::collect(&self.cards, &pre);
        let cond_lines = conds.identity_lines();
        if !cond_lines.is_empty() {
            parts.push("conds".to_string());
            parts.extend(cond_lines);
        }
        let sha256 = hex_sha256(parts.join("\n").as_bytes());
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
            }),
            fuel: DEFAULT_FUEL,
        })
    }
}

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
        let compiled = crate::cond_pre::CompiledPre::compile(src).map_err(|e| bad(e.to_string()))?;
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

/// A loaded, validated set of card modules.
#[derive(Clone)]
pub struct Ruleset {
    inner: Arc<Inner>,
    fuel: u64,
}

impl Ruleset {
    pub fn builder() -> RulesetBuilder {
        RulesetBuilder {
            engine: be::new_engine(),
            modules: vec![],
            cards: vec![],
            slots: vec![],
            precompiled: HashMap::new(),
        }
    }

    /// Convenience: a ruleset made of exactly one module.
    pub fn load(wasm: &[u8]) -> Result<Self, RuleError> {
        let mut b = Self::builder();
        b.add(wasm)?;
        b.build()
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

    /// Does any card in the set declare an entry at this kind? The cheap
    /// whole-set question that lets `counteract` skip building the bridge trigger at
    /// all when the answer is no -- with `StubRules`, or at a kind nothing
    /// listens to, that is every raise.
    pub fn declares(&self, kind: crate::TriggerKind) -> bool {
        let v = kind as i32;
        (0..128).contains(&v) && (self.inner.declared & (1u128 << v)) != 0
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

    /// Cached counteraction index (BOT-RESEARCH.md #1): does this card declare
    /// a [反击] at `kind`? One shift of the per-card bitmask -- the offer loop
    /// never scans the entry table and never builds a `Run` for a card that
    /// cannot answer.
    pub fn counteracts_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        let v = kind as i32;
        (0..128).contains(&v)
            && self
                .inner
                .counteract_mask
                .get(card as usize)
                .is_some_and(|m| (m >> v) & 1 != 0)
    }

    /// Hook/gate twin of [`Self::counteracts_to`].
    pub fn hooks_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        let v = kind as i32;
        (0..128).contains(&v)
            && self
                .inner
                .hook_mask
                .get(card as usize)
                .is_some_and(|m| (m >> v) & 1 != 0)
    }

    /// Handle for a card id from `cards.json`, e.g. `"AG:Y.O.L.O"`.
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

    /// One declared property; `0` when the card does not declare it (the
    /// defined default for every key the engine reads).
    pub fn card_prop(&self, id: &str, key: &str) -> i32 {
        self.card(id)
            .and_then(|i| self.inner.cards.get(i as usize))
            .and_then(|c| c.props.get(key).copied())
            .unwrap_or(0)
    }

    /// Run an effect from `world` with the answers collected so far.
    ///
    /// `world` is never modified. On [`Outcome::Done`] the caller commits the returned
    /// world; on [`Outcome::NeedInput`] it publishes the prompt, and once the answer
    /// arrives calls again with the **same** `world` and `answers` + the new answer.
    pub fn run<W: CardWorld>(
        &self,
        world: &W,
        call: Call,
        answers: &[i32],
    ) -> Result<Outcome<W>, RuleError> {
        let (card, player_id) = (call.card(), call.player_id());
        self.check(card)?;
        let info = &self.inner.cards[card as usize];
        let entry = match call {
            Call::Play { .. } => info.entry(OnKind::Play, None),
            Call::Counteract { .. } => info.entry(OnKind::Counteract, Some(world.trigger().kind)),
            Call::Hook { kind, .. } => info.hook_entry(kind),
            Call::AtEnd { .. } => info.entry(OnKind::AtEnd, None),
            Call::RollPlan { .. } => info.entry(OnKind::RollPlan, None),
            Call::Settle { .. } => info.entry(OnKind::Settle, None),
        };
        let Some(entry) = entry else {
            return Ok(Outcome::Done(world.clone()));
        };
        let mut store = self.store(world.clone(), answers)?;
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

    /// Ask one card's hook guard and, when it passes, run its body -- on a
    /// **single** instantiation, so a fired hook costs one fire-up and one world
    /// copy rather than the two a separate guard query + [`Self::run`] cost.
    ///
    /// Returns `Ok(None)` when the card is not activated at all (its condition
    /// rejected, its guard refused, or it has no entry at this kind).
    /// [`HookRun::announced`] is the "a guard existed and passed" moment -- the
    /// one time the card shows itself; a gate, which has no guard, runs without
    /// announcing. The guard step goes through [`crate::cond_pre::admits`]
    /// (docs/GUARDS.md §4.4 item 2).
    pub fn run_hook<W: CardWorld>(
        &self,
        world: &W,
        call: Call,
        answers: &[i32],
    ) -> Result<Option<HookRun<W>>, RuleError> {
        let (card, player_id) = (call.card(), call.player_id());
        self.check(card)?;
        let kind = world.trigger().kind;
        let info = &self.inner.cards[card as usize];
        let Some(entry) = info.hook_entry(kind) else {
            return Ok(None);
        };
        let mut store = self.store(world.clone(), answers)?;
        let mut announced = false;
        // `On::Hook` entries carry a guard; `On::Gate` entries are questions and
        // have none, so they run unasked.
        if let Some(guard) = info.entry(OnKind::Hook, Some(kind)) {
            let pre = self
                .inner
                .pre
                .get(card as usize)
                .and_then(|r| r.get(guard as usize))
                .and_then(|p| p.as_ref());
            let scope = crate::cond_pre::window_scope(&crate::cond_pre::fill_window(world));
            let cand = crate::cond_pre::fill_candidate(world, player_id, &info.id, true);
            #[cfg(feature = "guard-audit")]
            let audit = self.legacy_probe(world, card, guard, player_id);
            let admitted = if self.guard_is_none(card, guard) {
                // G4 deleted the residual: the condition alone decides.
                crate::cond_pre::admits_pre(pre, Some(&scope), &cand)
            } else {
                // A guard is a pure query: refuse inline answers (a nested drive's
                // host must not answer a question nobody asked -- see
                // `crate::inline::Deny`). A prompting guard fails closed, as below.
                let asked: Result<bool, ()> = crate::cond_pre::admits(pre, Some(&scope), &cand, || {
                    match crate::inline::with_no_inline(|| {
                        call_card(&self.inner, &mut store, card, guard, export::OP_GUARD, player_id)
                    }) {
                        Ok(0) => Ok(false),
                        Ok(_) => Ok(true),
                        // A trap or a prompting guard: fail closed -- the contract
                        // `can_hook` + `hook_guard` had.
                        Err(_) => Ok(false),
                    }
                });
                matches!(asked, Ok(true))
            };
            #[cfg(feature = "guard-audit")]
            if let Some(legacy) = audit {
                crate::cond_pre::legacy_audit(
                    &info.id,
                    guard,
                    Some(legacy),
                    admitted,
                    &trigger_dump(&world.trigger()),
                );
            }
            if admitted {
                announced = true;
            } else {
                // Condition rejected, guard refused, or a trap: the card does
                // not fire.
                return Ok(None);
            }
        }
        let res = call_card(
            &self.inner,
            &mut store,
            card,
            entry,
            export::OP_RUN,
            player_id,
        );
        Ok(Some(HookRun {
            announced,
            outcome: finish(store, res)?,
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
        self.inner.pre.get(card as usize)?.get(entry as usize)?.as_ref()
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
        let scope = crate::cond_pre::window_scope(&crate::cond_pre::fill_window(world));
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
        // Only a card that declared a [反击] at this kind is ever asked.
        let Some(entry) =
            self.inner.cards[card as usize].entry(OnKind::Counteract, Some(world.trigger().kind))
        else {
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
        #[cfg(feature = "guard-audit")]
        let audit = self.legacy_probe(world, card, entry, player_id);
        let ok = if self.guard_is_none(card, entry) {
            // G4 deleted the residual: the condition alone decides.
            crate::cond_pre::admits_pre(pre, Some(scope), &cand)
        } else {
            crate::cond_pre::admits(pre, Some(scope), &cand, || {
                let mut store = self.store(world.clone(), &[])?;
                match crate::inline::with_no_inline(|| {
                    call_card(&self.inner, &mut store, card, entry, export::OP_GUARD, player_id)
                }) {
                    Ok(v) => Ok(v != 0),
                    Err(e) if is_need_input(&e) => Err(RuleError::GuardPrompted),
                    Err(e) => Err(trap(e)),
                }
            })?
        };
        #[cfg(feature = "guard-audit")]
        if let Some(legacy) = audit {
            crate::cond_pre::legacy_audit(
                &self.inner.cards[card as usize].id,
                entry,
                Some(legacy),
                ok,
                &trigger_dump(&world.trigger()),
            );
        }
        Ok(ok)
    }

    /// Condition-only pre-filter (BOT-RESEARCH.md #1): evaluate the entry's
    /// condition for `(card, seat)` against a shared window scope **without**
    /// building a `Run` or instantiating the guard. `Some(entry)` when the
    /// condition admits (or the entry has none) and the residual guard must be
    /// asked; `None` when the card has no [反击] at this kind or the condition
    /// rejects it. The offer loop calls this first and only builds the `Run`
    /// for the survivors.
    pub fn counteract_pre_allows<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        player_id: i32,
        scope: &rules_cond::WindowScope,
    ) -> Option<i32> {
        let kind = world.trigger().kind;
        if !self.counteracts_to(card, kind) {
            return None;
        }
        let entry = self.inner.cards[card as usize].entry(OnKind::Counteract, Some(kind))?;
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

    /// G3 migration audit: ask the guest for the entry's `legacy_*` guard
    /// (`export::OP_LEGACY_GUARD`). `None` when the entry has no legacy copy.
    #[cfg(feature = "guard-audit")]
    fn legacy_probe<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        entry: i32,
        player_id: i32,
    ) -> Option<bool> {
        let info = self.inner.cards.get(card as usize)?;
        if !info.on.get(entry as usize)?.has_legacy {
            return None;
        }
        let mut store = self.store(world.clone(), &[]).ok()?;
        match crate::inline::with_no_inline(|| {
            call_card(
                &self.inner,
                &mut store,
                card,
                entry,
                export::OP_LEGACY_GUARD,
                player_id,
            )
        }) {
            // rt.rs returns -1 for "no legacy copy".
            Ok(v) if v >= 0 => Some(v != 0),
            _ => None,
        }
    }

    /// `Card.WhyNot` -- the reason the card cannot be played now, or `None` when
    /// it can. Also a pure query on a throwaway copy (a prompting guard reports
    /// [`RuleError::GuardPrompted`]). This is the `Play` **gate**
    /// (`Option<fn(i32) -> Option<Msg>>`), so it runs under
    /// [`export::OP_GUARD`] -- the effect body does not run here.
    ///
    /// Goes through [`crate::cond_pre::admits_gate`] (docs/GUARDS.md §4.4 item
    /// 3): a rejecting condition **is** a block (`err.play_pre`) and the gate
    /// is skipped. The play-gate window has no `Trigger` (`kind` absent).
    pub fn cant_play<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        player_id: i32,
    ) -> Result<Option<crate::Msg>, RuleError> {
        self.check(card)?;
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
            scope = crate::cond_pre::window_scope(&crate::cond_pre::fill_window_ambient(
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
        #[cfg(feature = "guard-audit")]
        let audit = self.legacy_probe(world, card, entry, player_id);
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
                    let mut store = self.store(world.clone(), &[])?;
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
        #[cfg(feature = "guard-audit")]
        if let Some(legacy) = audit {
            crate::cond_pre::legacy_audit(
                &self.inner.cards[card as usize].id,
                entry,
                Some(legacy),
                why.is_none(),
                &format!("cant_play player={player_id}"),
            );
        }
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
    ) -> Result<Store<HostState<W>>, RuleError> {
        #[cfg(feature = "bot-cost")]
        let t0 = std::time::Instant::now();
        #[cfg(feature = "bot-cost")]
        bot_cost::HOST_WORLD_CLONES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let state = HostState::new(self.inner.clone(), world, answers.to_vec(), 0);
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

/// One-line dump of a trigger for the G3 audit panic message.
fn trigger_dump(t: &crate::Trigger) -> String {
    format!(
        "kind={:?} actor={} target={} tile={} value={} step={} by={} pay_is_rent={} move_roll={:?} move_kind={:?} abnormal={}",
        t.kind,
        t.player_id,
        t.target,
        t.tile,
        t.value,
        t.step,
        t.by_card.unwrap_or(-1),
        t.pay_is_rent,
        t.move_roll,
        t.move_kind,
        matches!(t.kind, crate::TriggerKind::Abnormal),
    )
}

fn trap(e: Error) -> RuleError {
    RuleError::Trap(be::error_text(&e))
}

/// Fold one guest call's result into an [`Outcome`] against the store it ran in.
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
                Ok(Outcome::NeedInput(p))
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
    fn declares(&self, kind: crate::TriggerKind) -> bool;
    /// Cached counteraction index (BOT-RESEARCH.md #1): does this card declare
    /// a [反击] at `kind`? Default scans the entry table; [`Ruleset`] uses the
    /// per-card kind bitmask.
    fn counteracts_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        self.cards()
            .get(card as usize)
            .is_some_and(|c| c.counteracts_to(kind))
    }
    /// Hook/gate twin of [`Self::counteracts_to`] (fix C's buy-hook index).
    /// Default scans the entry table (`CardInfo::hooks`); [`Ruleset`] overrides
    /// with the per-card `hook_mask` shift.
    fn hooks_to(&self, card: i32, kind: crate::TriggerKind) -> bool {
        self.cards().get(card as usize).is_some_and(|c| c.hooks(kind))
    }
    /// Does the set declare any buy hook at all (fix C)? Default ORs
    /// [`Self::declares`] over [`BUY_HOOK_KINDS`]; [`Ruleset`] overrides with
    /// the whole-set declared bitmask.
    fn declares_buy(&self) -> bool {
        BUY_HOOK_KINDS.iter().any(|&k| self.declares(k))
    }
    /// Condition pre-filter (BOT-RESEARCH.md #1): `Some(entry)` when the
    /// card's [反击] at this window's kind exists and its condition admits.
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
        let kind = world.trigger().kind;
        if !self.counteracts_to(card, kind) {
            return None;
        }
        let info = self.cards().get(card as usize)?;
        let entry = info.entry(OnKind::Counteract, Some(kind))?;
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
    /// Content hash of the loaded set; `None` when there is no module image
    /// (the native build -- the cards are compiled in).
    fn sha256(&self) -> Option<&str>;
    fn module_sha256(&self, card: i32) -> Option<&str>;
    fn run(
        &self,
        world: &crate::Run,
        call: Call,
        answers: &[i32],
    ) -> Result<Outcome<crate::Run>, RuleError>;
    fn run_hook(
        &self,
        world: &crate::Run,
        call: Call,
        answers: &[i32],
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
}

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
    fn declares(&self, kind: crate::TriggerKind) -> bool {
        Ruleset::declares(self, kind)
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
    ) -> Result<Outcome<crate::Run>, RuleError> {
        Ruleset::run::<crate::Run>(self, world, call, answers)
    }
    fn run_hook(
        &self,
        world: &crate::Run,
        call: Call,
        answers: &[i32],
    ) -> Result<Option<HookRun<crate::Run>>, RuleError> {
        Ruleset::run_hook::<crate::Run>(self, world, call, answers)
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
    /// The compiled condition of card `i`'s `entry` (docs/GUARDS.md G0), if
    /// any. Default `None` (no condition) so a stub handle need not care.
    fn pre(&self, i: i32, entry: i32) -> Option<&crate::cond_pre::CompiledPre> {
        let _ = (i, entry);
        None
    }
}

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
pub struct HostState<W, R = Arc<Inner>> {
    pub rules: Option<R>,
    pub world: Option<W>,
    pub answers: Vec<i32>,
    pub next_answer: usize,
    pub options: Vec<PromptOption>,
    pub asked: Option<Prompt>,
    pub host_request: Option<HostRequest>,
    pub depth: u32,
    /// Per-instance resource ceiling. Installed on the store by [`new_store`];
    /// lives in the store data because that is where `Store::limiter` wants its
    /// resource limiter to come from. Unused on the native backend.
    pub limits: be::StoreLimits,
}

/// The per-instance memory ceiling, as a fresh [`be::StoreLimits`]. One per
/// store (each nested `play_card` is its own instance).
fn store_limits() -> be::StoreLimits {
    be::StoreLimitsBuilder::new()
        .memory_size(be::MAX_MEMORY_BYTES)
        .build()
}

/// Build a store with the per-instance memory ceiling installed.
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
            asked: None,
            host_request: None,
            depth,
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
        self.get_fuel().map_err(|e| HostErr::trap(be::error_text(&e)))
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
        want_msg: bool,
    ) -> (Result<CallOut, HostErr>, HostState<W>, u64) {
        let rules = match nested.rules.clone() {
            Some(r) => r,
            None => {
                return (
                    Err(HostErr::trap("call_entry without rules")),
                    nested,
                    0,
                )
            }
        };
        // Share the remaining fuel with the nested run (the same contract
        // `play_card` had when it built the store inline): a runaway nest
        // exhausts one budget, and the leftover is handed back to the caller.
        let fuel = self.fuel().unwrap_or(0);
        let mut store = new_store(&rules.engine, nested);
        if let Err(e) = store.set_fuel(fuel).map_err(|e| HostErr::trap(be::error_text(&e))) {
            let inner = store.into_data();
            return (Err(e), inner, fuel);
        }
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

/// A card stops for a prompt by **returning** [`abi::EXIT_NEED_INPUT`] from
/// `bandori_on` -- `rt::on` turns the run's `Err(card_sdk::Prompt)` into that
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
    bot_cost::GUEST_NS.fetch_add(t1.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    out
}

/// Like [`call_card`] for a `Play` gate, which answers with a packed `Msg`
/// buffer: the buffer is read back before the instance drops. `op` is
/// [`export::OP_GUARD`] for the gate; a run that stops for a prompt never gets
/// this far (the sentinel is folded first, so it cannot be misread as a `Msg`
/// pointer).
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
    bot_cost::GUEST_NS.fetch_add(t1.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
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

/// Validate one module and read its manifest.
fn inspect(engine: &Engine, wasm: &[u8]) -> Result<(Module, Vec<CardInfo>), RuleError> {
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
    Ok((module, cards))
}

/// The C# `AbnormalGate` from inside a card run: on replay the logged answer
/// says whether the effect went through; otherwise the run is paused with a
/// `Gate` request and the engine adjudicates it.
///
/// Pauses by **trapping** with [`need_input`], unlike `ask`/`pay`/`play_card`.
/// `ctx::gate` reads this as a plain `bool`, so a sentinel here would read as
/// `true` ("gate passed") and the effect would run on an unevaluated gate. Same
/// for `target`, `card_move` and the buy/build routines below -- see
/// [`need_input`] for the whole split.
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
            G::Char(v) => E::Char(v),
            G::N(v) => E::N(v),
            G::I(v) => E::I(v),
            G::Msg(v) => E::Msg(Box::new(engine_msg(*v))),
        };
        out.set_arg(name, arg);
    }
    out
}

// Linkers, cached per (engine, world type). The linker is a pure function of
// the world type -- every host function is a fixed registration -- but
// building it walks ~160 `func_wrap` calls and cost ~1.3ms of every ~2ms
// fire-up, so it must never be per call. Thread-local because the world type
// is not `Send`-bounded and the cache is pure memoization.
thread_local! {
    static LINKERS: std::cell::RefCell<HashMap<(usize, std::any::TypeId), Box<dyn std::any::Any>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// `Inner` ids, so the caches below cannot hit across two sets of modules that
/// happen to share an address.
fn next_inner_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

// Instantiation points, cached per (ruleset, world type, module). With the
// linker cached, resolving a module's imports against it is the rest of a
// fire-up; `Prepared` does that once and every call instantiates from it.
// Same thread-local reasoning as [`LINKERS`].
thread_local! {
    static PREPARED: std::cell::RefCell<
        HashMap<(u64, std::any::TypeId, usize), Box<dyn std::any::Any>>,
    > = std::cell::RefCell::new(HashMap::new());
}

/// Instantiate `rules.modules[module_ix]` against its cached [`be::Prepared`],
/// building it on the first call. Never clones the linker: the whole lookup,
/// build and instantiate happens under one borrow.
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

fn build_linker<W: CardWorld>(engine: &Engine) -> Result<Linker<HostState<W>>, Error> {
    let mut l = Linker::<HostState<W>>::new(engine);
    let m = IMPORT_MODULE;
    type C<'a, W> = Caller<'a, HostState<W>>;

    // Thin wrappers over `hostfns` -- the bodies live there so the native
    // backend (`rules-native`) can call the exact same logic.
    l.func_wrap(
        m,
        "roll",
        |mut c: C<W>, player_id: i32, count: i32, sides: i32| -> Result<i32, Error> {
            hostfns::roll(&mut c, player_id, count, sides).map_err(HostErr::into_err)
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
    l.func_wrap(
        m,
        "tile_count",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::tile_count(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_mark",
        |mut c: C<W>, tile: i32, player_id: i32, kp: i32, kl: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::add_mark(&mut c, tile, player_id, kp, kl, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "place_cp",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::place_cp(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "count_cp",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::count_cp(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "count_cp_from",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::count_cp_from(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "clear_cp",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::clear_cp(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "cp_src_at",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::cp_src_at(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "cp_attached",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::cp_attached(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_cp",
        |mut c: C<W>, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_cp(&mut c, n, max).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "cp_at",
        |mut c: C<W>, uid: i32| -> Result<i32, Error> {
            hostfns::cp_at(&mut c, uid).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_cp_at",
        |mut c: C<W>, uid: i32, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_cp_at(&mut c, uid, n, max).map_err(HostErr::into_err)
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
    l.func_wrap(
        m,
        "player_count",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::player_count(&mut c).map_err(HostErr::into_err)
        },
    )?;
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
        "is_buyable",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::is_buyable(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "is_shop",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::is_shop(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "is_ring",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::is_ring(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "is_circle",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::is_circle(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "is_color",
        |mut c: C<W>, player_id: i32, tile: i32, group: i32| -> Result<i32, Error> {
            hostfns::is_color(&mut c, player_id, tile, group).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "is_live_house_for",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            hostfns::is_live_house_for(&mut c, player_id, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "paid_in_settle",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::paid_in_settle(&mut c).map_err(HostErr::into_err)
        },
    )?;
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
        "crystals_at",
        |mut c: C<W>, uid: i32| -> Result<i32, Error> {
            hostfns::crystals_at(&mut c, uid).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_crystals_at",
        |mut c: C<W>, uid: i32, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_crystals_at(&mut c, uid, n, max).map_err(HostErr::into_err)
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
        "is_agent",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::is_agent(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "is_live_house",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::is_live_house(&mut c, tile).map_err(HostErr::into_err)
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
        "add_to_hand",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
            hostfns::add_to_hand(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_to_deck",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, shuffle: i32| -> Result<(), Error> {
            hostfns::add_to_deck(&mut c, player_id, p, n, shuffle).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_to_deck_at",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, pos: i32| -> Result<(), Error> {
            hostfns::add_to_deck_at(&mut c, player_id, p, n, pos).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "take_card",
        |mut c: C<W>, player_id: i32, pile: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::take_card(&mut c, player_id, pile, p, n).map_err(HostErr::into_err)
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
    l.func_wrap(
        m,
        "unplace_card",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::unplace_card(&mut c).map_err(HostErr::into_err)
        },
    )?;
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
        |mut c: C<W>, cp: i32, cl: i32, np: i32, nl: i32| -> Result<i32, Error> {
            hostfns::card_text_mentions(&mut c, cp, cl, np, nl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_crystals",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32| -> Result<i32, Error> {
            hostfns::card_crystals(&mut c, player_id, cp, cl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_card_crystals",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_card_crystals(&mut c, player_id, cp, cl, n, max).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "is_placed",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::is_placed(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "self_tile",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::self_tile(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_self_tile",
        |mut c: C<W>, tile: i32| -> Result<i32, Error> {
            hostfns::set_self_tile(&mut c, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "self_face_down",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::self_face_down(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_self_face_down",
        |mut c: C<W>, on: i32| -> Result<i32, Error> {
            hostfns::set_self_face_down(&mut c, on).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "self_immune",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::self_immune(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_self_immune",
        |mut c: C<W>, on: i32| -> Result<i32, Error> {
            hostfns::set_self_immune(&mut c, on).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "crystals",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::crystals(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_crystals",
        |mut c: C<W>, n: i32| -> Result<i32, Error> {
            hostfns::set_crystals(&mut c, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_crystals",
        |mut c: C<W>, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_crystals(&mut c, n, max).map_err(HostErr::into_err)
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
        "settle_circle_reward",
        |mut c: C<W>, player_id: i32, landing: i32| -> Result<i32, Error> {
            hostfns::settle_circle_reward(&mut c, player_id, landing).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "count_marks",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32| -> Result<i32, Error> {
            hostfns::count_marks(&mut c, tile, kp, kl, owner).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "remove_marks",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32| -> Result<i32, Error> {
            hostfns::remove_marks(&mut c, tile, kp, kl, owner).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "tok",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::tok(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_tok",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, v: i32| -> Result<(), Error> {
            hostfns::set_tok(&mut c, player_id, p, n, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_tok",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, by: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_tok(&mut c, player_id, p, n, by, max).map_err(HostErr::into_err)
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
        "slot",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::slot(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_slot",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, v: i32| -> Result<(), Error> {
            hostfns::set_slot(&mut c, player_id, p, n, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "inc_slot",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, by: i32| -> Result<i32, Error> {
            hostfns::inc_slot(&mut c, player_id, p, n, by).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "band_crystals",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::band_crystals(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_band_crystals",
        |mut c: C<W>, player_id: i32, n: i32, max: i32| -> Result<i32, Error> {
            hostfns::add_band_crystals(&mut c, player_id, n, max).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "band_skill",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::band_skill(&mut c, player_id, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "character_skill",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::character_skill(&mut c, player_id, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "band_skills",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            hostfns::band_skills(&mut c, player_id, buf, cap).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_band_skill",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, extra: i32| -> Result<i32, Error> {
            hostfns::add_band_skill(&mut c, player_id, p, n, extra).map_err(HostErr::into_err)
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
        "fire",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::fire(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "fire_max",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::fire_max(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "gain_fire",
        |mut c: C<W>, player_id: i32, n: i32, p: i32, l: i32| -> Result<i32, Error> {
            hostfns::gain_fire(&mut c, player_id, n, p, l).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "give_stay",
        |mut c: C<W>, player_id: i32, n: i32| -> Result<(), Error> {
            hostfns::give_stay(&mut c, player_id, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "give_stun",
        |mut c: C<W>, player_id: i32, n: i32| -> Result<(), Error> {
            hostfns::give_stun(&mut c, player_id, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "give_exile",
        |mut c: C<W>, player_id: i32, n: i32, to: i32| -> Result<(), Error> {
            hostfns::give_exile(&mut c, player_id, n, to).map_err(HostErr::into_err)
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
    l.func_wrap(
        m,
        "spend_fire",
        |mut c: C<W>, player_id: i32, n: i32, p: i32, l: i32| -> Result<i32, Error> {
            hostfns::spend_fire(&mut c, player_id, n, p, l).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "stay_of",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::stay_of(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "stun_of",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::stun_of(&mut c, player_id).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "turn_player",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::turn_player(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "round_no",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::round_no(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "turn_key",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::turn_key(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "character_is",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::character_is(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "in_band",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::in_band(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "bump_mark",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32, delta: i32| -> Result<i32, Error> {
            hostfns::bump_mark(&mut c, tile, kp, kl, owner, delta).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "do_move_roll",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            hostfns::do_move_roll(&mut c, player_id).map_err(HostErr::into_err)
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
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32| -> Result<i32, Error> {
            hostfns::unplace_card_named(&mut c, player_id, cp, cl).map_err(HostErr::into_err)
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
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32| -> Result<i32, Error> {
            hostfns::card_face_down(&mut c, player_id, cp, cl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "extreme",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::extreme(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_extreme",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_extreme(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "play_from_hand",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::play_from_hand(&mut c).map_err(HostErr::into_err)
        },
    )?;
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
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, down: i32| -> Result<i32, Error> {
            hostfns::set_card_face_down(&mut c, player_id, cp, cl, down).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "clear_dice",
        |mut c: C<W>| -> Result<(), Error> {
            hostfns::clear_dice(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_card_immune",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, on: i32| -> Result<i32, Error> {
            hostfns::set_card_immune(&mut c, player_id, cp, cl, on).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "card_immune",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32| -> Result<i32, Error> {
            hostfns::card_immune(&mut c, player_id, cp, cl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_card_tile",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, tile: i32| -> Result<i32, Error> {
            hostfns::set_card_tile(&mut c, player_id, cp, cl, tile).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "place_card_on",
        |mut c: C<W>, player_id: i32, tile: i32, cp: i32, cl: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::place_card_on(&mut c, player_id, tile, cp, cl, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "place_card_at",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::place_card_at(&mut c, player_id, cp, cl, p, n).map_err(HostErr::into_err)
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
    l.func_wrap(
        m,
        "ring_multiplier",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::ring_multiplier(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_ring_bonus",
        |mut c: C<W>, n: i32| -> Result<i32, Error> {
            hostfns::add_ring_bonus(&mut c, n).map_err(HostErr::into_err)
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
        "roll_ask",
        |mut c: C<W>, player_id: i32, count: i32, sides: i32, source: i32| -> Result<i32, Error> {
            hostfns::roll_ask(&mut c, player_id, count, sides, source).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "agent_landing",
        |mut c: C<W>, player_id: i32, agent: i32| -> Result<i32, Error> {
            hostfns::agent_landing(&mut c, player_id, agent).map_err(HostErr::into_err)
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
        |mut c: C<W>, player_id: i32, kind: i32, buf: i32, n: i32, out: i32| -> Result<i32, Error> {
            hostfns::buy_quotes(&mut c, player_id, kind, buf, n, out).map_err(HostErr::into_err)
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
        "placed_tile",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::placed_tile(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "play_doubled",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::play_doubled(&mut c).map_err(HostErr::into_err)
        },
    )?;
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
    l.func_wrap(
        m,
        "opt_int",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::opt_int(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "opt_str",
        |mut c: C<W>, p: i32, n: i32| -> Result<(), Error> {
            hostfns::opt_str(&mut c, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "ask",
        |mut c: C<W>, kind: i32, player_id: i32, tp: i32, tl: i32, xp: i32, xl: i32| -> Result<i32, Error> {
            hostfns::ask(&mut c, kind, player_id, tp, tl, xp, xl).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_kind",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_kind(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_player",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_player(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_target",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_target(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_tile",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_tile(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_value",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_value(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_step",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_step(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_by_card",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_by_card(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_pay_is_rent",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_pay_is_rent(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_move_kind",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_move_kind(&mut c).map_err(HostErr::into_err)
        },
    )?;
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
    l.func_wrap(
        m,
        "trig_move_main",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_move_main(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_move_dir",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_move_dir(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_move_remaining",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_move_remaining(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_move_total",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_move_total(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_move_roll",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_move_roll(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_roll_source",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_roll_source(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_buy_kind",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_buy_kind(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_seller",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_seller(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_price",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_price(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_price",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::trig_set_price(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_deal_owner",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_deal_owner(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_set_deal_owner",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::trig_set_deal_owner(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_deal_houses",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_deal_houses(&mut c).map_err(HostErr::into_err)
        },
    )?;
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
    l.func_wrap(
        m,
        "trig_cancelled",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_cancelled(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_seq",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_seq(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "trig_answers",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::trig_answers(&mut c).map_err(HostErr::into_err)
        },
    )?;
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
        |mut c: C<W>, i: i32| -> Result<i32, Error> {
            hostfns::trig_effect_kind(&mut c, i).map_err(HostErr::into_err)
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
        |mut c: C<W>, kind: i32, target: i32, from: i32, tile: i32, value: i32| -> Result<(), Error> {
            hostfns::declare_effect(&mut c, kind, target, from, tile, value).map_err(HostErr::into_err)
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
        "card_replayable",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            hostfns::card_replayable(&mut c, player_id, p, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "schedule_turn_end",
        |mut c: C<W>, player_id: i32, mode: i32| -> Result<(), Error> {
            hostfns::schedule_turn_end(&mut c, player_id, mode).map_err(HostErr::into_err)
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
    l.func_wrap(
        m,
        "fixed_roll",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::fixed_roll(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_next_steps",
        |mut c: C<W>, player_id: i32, n: i32| -> Result<(), Error> {
            hostfns::set_next_steps(&mut c, player_id, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "turn_main_steps",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::turn_main_steps(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "add_fire_max",
        |mut c: C<W>, player_id: i32, n: i32| -> Result<i32, Error> {
            hostfns::add_fire_max(&mut c, player_id, n).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_roller",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_roller(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_steps",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_steps(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
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
        "set_kind",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_kind(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_trigger_target",
        |mut c: C<W>, v: i32| -> Result<(), Error> {
            hostfns::set_trigger_target(&mut c, v).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "set_trigger_cancelled",
        |mut c: C<W>| -> Result<(), Error> {
            hostfns::set_trigger_cancelled(&mut c).map_err(HostErr::into_err)
        },
    )?;
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
        "move_tag",
        |mut c: C<W>, kp: i32, kl: i32| -> Result<i32, Error> {
            hostfns::move_tag(&mut c, kp, kl).map_err(HostErr::into_err)
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
    l.func_wrap(
        m,
        "move_stop_at",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::move_stop_at(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "move_stopped",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::move_stopped(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "move_parity",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::move_parity(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "move_resolve",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::move_resolve(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "move_kind",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::move_kind(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "move_steps",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::move_steps(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "move_remaining",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::move_remaining(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "move_total",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::move_total(&mut c).map_err(HostErr::into_err)
        },
    )?;
    l.func_wrap(
        m,
        "move_dir",
        |mut c: C<W>| -> Result<i32, Error> {
            hostfns::move_dir(&mut c).map_err(HostErr::into_err)
        },
    )?;

    Ok(l)
}

impl HostState<NullWorld> {
    fn without_rules() -> Self {
        Self {
            rules: None,
            world: Some(NullWorld),
            answers: vec![],
            next_answer: 0,
            options: vec![],
            asked: None,
            host_request: None,
            depth: 0,
            limits: store_limits(),
        }
    }
}

/// World used only while reading module metadata.
#[derive(Clone)]
struct NullWorld;

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
    fn add_to_hand(&mut self, _: i32, _: &str) {}
    fn take_card(&mut self, _: i32, _: crate::CardPile, _: &str) -> bool {
        false
    }
    fn cards_in(&self, _: i32, _: crate::CardPile) -> Vec<String> {
        Vec::new()
    }
    fn add_to_deck(&mut self, _: i32, _: &str, _: bool) {}
    fn add_to_deck_at(&mut self, _: i32, _: &str, _: i32) {}
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
    fn crystals(&self) -> i32 {
        0
    }
    fn set_crystals(&mut self, _: i32) -> i32 {
        0
    }
    fn add_crystals(&mut self, _: i32, _: i32) -> i32 {
        0
    }
    fn add_mark(&mut self, _: i32, _: i32, _: &str, _: crate::Msg) {}
    fn count_marks(&self, _: i32, _: &str, _: i32) -> i32 {
        0
    }
    fn remove_marks(&mut self, _: i32, _: &str, _: i32) -> i32 {
        0
    }
    fn tok(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn set_tok(&mut self, _: i32, _: &str, _: i32) {}
    fn add_tok(&mut self, _: i32, _: &str, _: i32, _: i32) -> i32 {
        0
    }
    fn slot(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn set_slot(&mut self, _: i32, _: &str, _: i32) {}
    fn inc_slot(&mut self, _: i32, _: &str, _: i32) -> i32 {
        0
    }
    fn band_crystals(&self, _: i32) -> i32 {
        0
    }
    fn add_band_crystals(&mut self, _: i32, _: i32, _: i32) -> i32 {
        0
    }
    fn fire(&self, _: i32) -> i32 {
        0
    }
    fn fire_max(&self, _: i32) -> i32 {
        0
    }
    fn gain_fire(&mut self, _: i32, _: i32, _: crate::Msg) -> i32 {
        0
    }
    fn give_stay(&mut self, _: i32, _: i32) {}
    fn give_stun(&mut self, _: i32, _: i32) {}
    fn give_exile(&mut self, _: i32, _: i32, _: i32) {}
    fn give_extra_turn(&mut self, _: i32) {}
    fn set_dest(&mut self, _: i32) {}
    fn set_transfer_to_dest(&mut self, _: i32, _: i32) {}
    fn send_to_dest(&mut self, _: i32) -> Option<i32> {
        None
    }
    fn transfer_to_dest(&mut self, _: i32, _: i32) -> Option<i32> {
        None
    }
    fn ring_multiplier(&self) -> i32 {
        10
    }
    fn add_ring_bonus(&mut self, _: i32) -> i32 {
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
    fn declare_trigger_effect(&mut self, _: i32, _: i32, _: i32, _: i32, _: i32) {}
    fn trig_card_is(&self, _: &str) -> i32 {
        0
    }
    fn set_trigger_price(&mut self, _: i32) {}
    fn set_trigger_deal_owner(&mut self, _: i32) {}
    fn set_trigger_deal_houses(&mut self, _: i32) {}
    fn set_trigger_deal_mortgaged(&mut self, _: i32) {}
    fn set_trigger_reason(&mut self, _: &str) {}
    fn is_buyable(&self, _: i32) -> i32 {
        0
    }
    fn is_shop(&self, _: i32) -> i32 {
        0
    }
    fn is_ring(&self, _: i32) -> i32 {
        0
    }
    fn is_circle(&self, _: i32) -> i32 {
        0
    }
    fn is_live_house(&self, _: i32) -> i32 {
        0
    }
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
    fn spend_fire(&mut self, _: i32, _: i32, _: crate::Msg) -> i32 {
        0
    }
    fn stay_of(&self, _: i32) -> i32 {
        0
    }
    fn stun_of(&self, _: i32) -> i32 {
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
    fn character_is(&self, _: i32, _: &str) -> i32 {
        0
    }
    fn in_band(&self, _: i32, _: &str) -> i32 {
        0
    }
}
