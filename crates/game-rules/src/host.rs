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
}

/// Fuel per top-level effect run, shared by any nested `play_card` calls. Plenty
/// for straight-line card logic; stops runaway loops at the same instruction on
/// every machine.
const DEFAULT_FUEL: u64 = 5_000_000;

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
    Buy { player_id: i32, tile: i32 },
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
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(m) => write!(f, "card module load error: {m}"),
            Self::DuplicateCard(id) => write!(f, "card {id:?} is declared by more than one module"),
            Self::NoSuchCard(c) => write!(f, "no card with handle {c}"),
            Self::Trap(m) => write!(f, "card effect failed: {m}"),
            Self::GuardPrompted => write!(f, "can_counteract tried to prompt a player"),
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
    /// turn-ctx policy the card set up (`build_discount`, `buy_discount`, ...)
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

struct Inner {
    /// Unique across the process. Caches keyed on a set of modules must not
    /// survive one `Inner` into another that lands at the same address, so they
    /// key on this rather than on a pointer.
    id: u64,
    engine: Engine,
    modules: Vec<LoadedModule>,
    cards: Vec<CardInfo>,
    slots: Vec<Slot>,
    by_id: HashMap<String, i32>,
    sha256: String,
    /// Bitset of every trigger kind any card declares, so `counteract` can skip the
    /// whole bridge when nothing in the set listens. `TriggerKind` values fit in
    /// 0..128, so this is one word and costs no allocation.
    declared: u128,
}

/// Builds a [`Ruleset`] from any number of card modules (normally one per card).
pub struct RulesetBuilder {
    engine: Engine,
    modules: Vec<LoadedModule>,
    cards: Vec<CardInfo>,
    slots: Vec<Slot>,
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

    pub fn build(self) -> Result<Ruleset, RuleError> {
        let mut by_id = HashMap::new();
        for (i, c) in self.cards.iter().enumerate() {
            if by_id.insert(c.id.clone(), i as i32).is_some() {
                return Err(RuleError::DuplicateCard(c.id.clone()));
            }
        }
        // Order-independent identity of the whole set.
        let mut hashes: Vec<&str> = self.modules.iter().map(|m| m.sha256.as_str()).collect();
        hashes.sort_unstable();
        let sha256 = hex_sha256(hashes.join("\n").as_bytes());
        let mut declared = 0u128;
        for c in &self.cards {
            for o in &c.on {
                for &k in &o.triggers {
                    if (0..128).contains(&k) {
                        declared |= 1u128 << k;
                    }
                }
            }
        }
        Ok(Ruleset {
            inner: Arc::new(Inner {
                id: next_inner_id(),
                engine: self.engine,
                modules: self.modules,
                cards: self.cards,
                slots: self.slots,
                by_id,
                sha256,
                declared,
            }),
            fuel: DEFAULT_FUEL,
        })
    }
}

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
    /// Returns `Ok(None)` when the card is not activated at all (its guard
    /// refused, or it has no entry at this kind). [`HookRun::announced`] is the
    /// "a guard existed and passed" moment -- the one time the card shows
    /// itself; a gate, which has no guard, runs without announcing.
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
            match call_card(
                &self.inner,
                &mut store,
                card,
                guard,
                export::OP_GUARD,
                player_id,
            ) {
                Ok(0) => return Ok(None),
                Ok(_) => announced = true,
                // A trap or a prompting guard: fail closed, the card does not
                // fire -- the contract `can_hook` + `hook_guard` had.
                Err(_) => return Ok(None),
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

    /// `Card.CanCounteract` against the current trigger. Runs on a throwaway copy, so it
    /// cannot change the world even if the card calls a mutating function.
    pub fn can_counteract<W: CardWorld>(
        &self,
        world: &W,
        card: i32,
        player_id: i32,
    ) -> Result<bool, RuleError> {
        self.check(card)?;
        // Only a card that declared a [反击] at this kind is ever asked.
        let Some(entry) =
            self.inner.cards[card as usize].entry(OnKind::Counteract, Some(world.trigger().kind))
        else {
            return Ok(false);
        };
        let mut store = self.store(world.clone(), &[])?;
        match call_card(
            &self.inner,
            &mut store,
            card,
            entry,
            export::OP_GUARD,
            player_id,
        ) {
            Ok(v) => Ok(v != 0),
            Err(e) if is_need_input(&e) => Err(RuleError::GuardPrompted),
            Err(e) => Err(trap(e)),
        }
    }

    /// `Card.WhyNot` -- the reason the card cannot be played now, or `None` when
    /// it can. Also a pure query on a throwaway copy (a prompting guard reports
    /// [`RuleError::GuardPrompted`]). This is the `Play` **gate**
    /// (`Option<fn(i32) -> Option<Msg>>`), so it runs under
    /// [`export::OP_GUARD`] -- the effect body does not run here.
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
        let mut store = self.store(world.clone(), &[])?;
        match call_card_msg(
            &self.inner,
            &mut store,
            card,
            entry,
            export::OP_GUARD,
            player_id,
        ) {
            Ok(v) => Ok(v),
            Err(e) if is_need_input(&e) => Err(RuleError::GuardPrompted),
            Err(e) => Err(trap(e)),
        }
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

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Per-run store data. `world` is `None` only while a nested `play_card` has
/// borrowed it (the outer guest is suspended inside the host call at that time).
struct HostState<W> {
    rules: Option<Arc<Inner>>,
    world: Option<W>,
    answers: Vec<i32>,
    next_answer: usize,
    options: Vec<PromptOption>,
    asked: Option<Prompt>,
    host_request: Option<HostRequest>,
    depth: u32,
    /// Per-instance resource ceiling. Installed on the store by [`new_store`];
    /// lives in the store data because that is where `Store::limiter` wants its
    /// resource limiter to come from.
    limits: be::StoreLimits,
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

impl<W> HostState<W> {
    fn new(rules: Arc<Inner>, world: W, answers: Vec<i32>, depth: u32) -> Self {
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

    fn w(&mut self) -> &mut W {
        self.world
            .as_mut()
            .expect("world borrowed by a nested call")
    }

    fn wr(&self) -> &W {
        self.world
            .as_ref()
            .expect("world borrowed by a nested call")
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
fn engine_msg(m: card_sdk::msg::Msg) -> crate::Msg {
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

    l.func_wrap(
        m,
        "roll",
        |mut c: C<W>, player_id: i32, count: i32, sides: i32| -> Result<i32, Error> {
            if !(1..=100).contains(&count) || !(1..=1000).contains(&sides) {
                return Err(err(format!("roll({count}d{sides}) out of range")));
            }
            Ok(c.data_mut().w().roll(player_id, count, sides))
        },
    )?;
    l.func_wrap(
        m,
        "log",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
            let msg = guest_msg(&mut c, p, n)?;
            c.data_mut().w().log(player_id, msg);
            Ok(())
        },
    )?;
    // `H.Effect` -- announce which effect a card just applied, the line the
    // client's effect popup is built from. Same wire shape as `log` (a `Msg`);
    // the world decides how it differs from a plain log line.
    l.func_wrap(
        m,
        "effect",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
            let msg = guest_msg(&mut c, p, n)?;
            c.data_mut().w().effect(player_id, msg);
            Ok(())
        },
    )?;
    l.func_wrap(m, "tile_count", |c: C<W>| c.data().wr().tile_count())?;
    l.func_wrap(
        m,
        "add_mark",
        |mut c: C<W>,
         tile: i32,
         player_id: i32,
         kp: i32,
         kl: i32,
         p: i32,
         n: i32|
         -> Result<(), Error> {
            let kind = guest_str(&mut c, kp, kl)?;
            let note = guest_msg(&mut c, p, n)?;
            c.data_mut().w().add_mark(tile, player_id, &kind, note);
            Ok(())
        },
    )?;
    // [CP点] -- the two kinds (user ruling 2026-10-07). Tile marks are the
    // `mark:cp` owner's small API (`rules/tiles/src/cp.rs`); `place_cp` takes no
    // note: the label comes from the mark's category (「CP点」) and the provenance
    // from `TileMark.src` / `TileMark.card`, so there is no per-call text to
    // carry. `cp_src_at` is that provenance (the instance a tile's mark is
    // attached to). The on-card count (`cp_attached` / `add_cp` / `cp_at` /
    // `add_cp_at`) is `FieldCard::cp` -- the CP points attached to the card.
    l.func_wrap(m, "place_cp", |mut c: C<W>, tile: i32| -> Result<i32, Error> {
        Ok(c.data_mut().w().place_cp(tile, crate::Msg::default()))
    })?;
    l.func_wrap(m, "count_cp", |c: C<W>, tile: i32| c.data().wr().count_cp(tile))?;
    l.func_wrap(m, "count_cp_from", |c: C<W>, tile: i32| {
        c.data().wr().count_cp_from(tile)
    })?;
    l.func_wrap(m, "clear_cp", |mut c: C<W>, tile: i32| c.data_mut().w().clear_cp(tile))?;
    l.func_wrap(m, "cp_src_at", |c: C<W>, tile: i32| c.data().wr().cp_src_at(tile))?;
    l.func_wrap(m, "cp_attached", |c: C<W>| c.data().wr().cp_attached())?;
    l.func_wrap(m, "add_cp", |mut c: C<W>, n: i32, max: i32| {
        c.data_mut().w().add_cp(n, max)
    })?;
    l.func_wrap(m, "cp_at", |c: C<W>, uid: i32| c.data().wr().cp_at(uid))?;
    l.func_wrap(
        m,
        "add_cp_at",
        |mut c: C<W>, uid: i32, n: i32, max: i32| c.data_mut().w().add_cp_at(uid, n, max),
    )?;
    l.func_wrap(m, "money", |c: C<W>, player_id: i32| {
        c.data().wr().money(player_id)
    })?;
    l.func_wrap(
        m,
        "gain",
        |mut c: C<W>, player_id: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            let src = guest_msg(&mut c, p, n)?;
            let st = c.data_mut();
            // A card-driven gain runs the same `Money` pipeline as a payment --
            // print (game -> player) -- so PayAdd / PayChoose / the `effect`
            // [反击] window all see it. The pause/resume is the same as `pay`:
            // the engine answers with the amount that actually moved.
            if let Some(&final_amount) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                // The engine-side money move already ran (the pipeline printed
                // it); this only answers the guest with what moved.
                return Ok(final_amount);
            }
            st.host_request = Some(HostRequest::Pay {
                from: -1,
                to: player_id,
                amount,
                src: Some(src),
            });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(
        m,
        "pay",
        |mut c: C<W>, player_id: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            let src = guest_msg(&mut c, p, n)?;
            let st = c.data_mut();
            // A card-driven payment is paused so the engine can raise a `pay`
            // trigger (and the [反击] window) before the money moves. The answer is
            // the final amount to move -- 0 cancels the payment outright.
            //
            // The pause is the sentinel **as the return value**, not a trap:
            // `ctx::pay` reads it through `asked()` and hands the card
            // `Err(Prompt)`. Trapping here would tear the stack before the card
            // ever saw a `Result`.
            if let Some(&final_amount) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                // The engine-side money move already ran (the pipeline deleted
                // it); this only answers the guest with what moved.
                return Ok(final_amount);
            }
            st.host_request = Some(HostRequest::Pay {
                from: player_id,
                to: -1,
                amount,
                src: Some(src),
            });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    // `H.PayR` to a named payee (player -> player) -- one pipeline entry, so
    // the `effect` declaration carries both the payer and the payee and （小白）
    // sees 「向其他玩家支付」. The pause/resume is the same as `pay`.
    l.func_wrap(
        m,
        "pay_to",
        |mut c: C<W>, from: i32, to: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            let src = guest_msg(&mut c, p, n)?;
            let st = c.data_mut();
            if let Some(&final_amount) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                // The engine-side money move already ran (the pipeline credited
                // the payee); this only answers the guest with what moved.
                return Ok(final_amount);
            }
            st.host_request = Some(HostRequest::Pay {
                from,
                to,
                amount,
                src: Some(src),
            });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(m, "player_count", |c: C<W>| c.data().wr().player_count())?;
    l.func_wrap(m, "player_out", |c: C<W>, player_id: i32| {
        c.data().wr().player_out(player_id)
    })?;
    l.func_wrap(m, "others_count", |c: C<W>, player_id: i32| {
        c.data().wr().others_count(player_id)
    })?;
    l.func_wrap(m, "others_at", |c: C<W>, player_id: i32, index: i32| {
        c.data().wr().others_at(player_id, index)
    })?;
    l.func_wrap(
        m,
        "tile_named",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            let name = guest_str(&mut c, p, n)?;
            Ok(c.data().wr().tile_named(&name))
        },
    )?;
    l.func_wrap(m, "tile_owner", |c: C<W>, tile: i32| {
        c.data().wr().tile_owner(tile)
    })?;
    l.func_wrap(m, "player_pos", |c: C<W>, player_id: i32| {
        c.data().wr().player_pos(player_id)
    })?;
    l.func_wrap(
        m,
        "tile_steps_ahead",
        |c: C<W>, player_id: i32, steps: i32| c.data().wr().tile_steps_ahead(player_id, steps),
    )?;
    l.func_wrap(m, "rent_of", |c: C<W>, tile: i32| {
        c.data().wr().rent_of(tile)
    })?;
    l.func_wrap(m, "buy_price", |c: C<W>, tile: i32| {
        c.data().wr().buy_price(tile)
    })?;
    l.func_wrap(m, "build_cost", |c: C<W>, tile: i32| {
        c.data().wr().build_cost(tile)
    })?;
    l.func_wrap(m, "mortgage_value", |c: C<W>, tile: i32| {
        c.data().wr().mortgage_value(tile)
    })?;
    l.func_wrap(m, "owned_count", |c: C<W>, player_id: i32| {
        c.data().wr().owned_count(player_id)
    })?;
    l.func_wrap(m, "owned_at", |c: C<W>, player_id: i32, index: i32| {
        c.data().wr().owned_at(player_id, index)
    })?;
    l.func_wrap(m, "is_buyable", |c: C<W>, tile: i32| {
        c.data().wr().is_buyable(tile)
    })?;
    l.func_wrap(m, "is_shop", |c: C<W>, tile: i32| {
        c.data().wr().is_shop(tile)
    })?;
    l.func_wrap(m, "is_ring", |c: C<W>, tile: i32| {
        c.data().wr().is_ring(tile)
    })?;
    l.func_wrap(m, "is_circle", |c: C<W>, tile: i32| {
        c.data().wr().is_circle(tile)
    })?;
    l.func_wrap(m, "set_tile_color", |mut c: C<W>, tile: i32, group: i32| {
        c.data_mut().w().set_tile_color(tile, group);
        Ok(())
    })?;
    l.func_wrap(
        m,
        "set_extra_color",
        |mut c: C<W>, player_id: i32, tile: i32, group: i32| {
            c.data_mut().w().set_extra_color(player_id, tile, group);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "is_color",
        |c: C<W>, player_id: i32, tile: i32, group: i32| {
            c.data().wr().is_color(player_id, tile, group) as i32
        },
    )?;
    l.func_wrap(
        m,
        "is_live_house_for",
        |c: C<W>, player_id: i32, tile: i32| c.data().wr().is_live_house_for(player_id, tile),
    )?;
    l.func_wrap(m, "set_buy_discount", |mut c: C<W>, n: i32| {
        c.data_mut().w().set_buy_discount(n);
        Ok(())
    })?;
    l.func_wrap(m, "paid_in_settle", |c: C<W>| {
        c.data().wr().paid_in_settle()
    })?;
    l.func_wrap(m, "set_free_buy", |mut c: C<W>, on: i32| {
        c.data_mut().w().set_free_buy(on != 0);
        Ok(())
    })?;
    l.func_wrap(
        m,
        "turn_rolls",
        |mut c: C<W>, buf: i32, cap: i32| -> Result<i32, Error> {
            let rolls = c.data().wr().turn_rolls();
            let bytes = postcard::to_allocvec(&rolls)
                .map_err(|e| err(format!("turn_rolls encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(
        m,
        "field_instances",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            let inst = c.data().wr().field_instances(player_id);
            let bytes = postcard::to_allocvec(&inst)
                .map_err(|e| err(format!("field_instances encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(m, "crystals_at", |c: C<W>, uid: i32| {
        c.data().wr().crystals_at(uid)
    })?;
    l.func_wrap(
        m,
        "add_crystals_at",
        |mut c: C<W>, uid: i32, n: i32, max: i32| c.data_mut().w().add_crystals_at(uid, n, max),
    )?;
    l.func_wrap(m, "unplace_at", |mut c: C<W>, uid: i32| {
        c.data_mut().w().unplace_at(uid)
    })?;
    l.func_wrap(m, "tile_at", |c: C<W>, uid: i32| c.data().wr().tile_at(uid))?;
    l.func_wrap(m, "set_tile_at", |mut c: C<W>, uid: i32, tile: i32| {
        c.data_mut().w().set_tile_at(uid, tile) as i32
    })?;
    l.func_wrap(m, "is_face_down_at", |c: C<W>, uid: i32| {
        c.data().wr().is_face_down_at(uid) as i32
    })?;
    l.func_wrap(m, "set_face_down_at", |mut c: C<W>, uid: i32, on: i32| {
        c.data_mut().w().set_face_down_at(uid, on != 0) as i32
    })?;
    l.func_wrap(m, "is_immune_at", |c: C<W>, uid: i32| {
        c.data().wr().is_immune_at(uid) as i32
    })?;
    l.func_wrap(m, "set_immune_at", |mut c: C<W>, uid: i32, on: i32| {
        c.data_mut().w().set_immune_at(uid, on != 0) as i32
    })?;
    l.func_wrap(
        m,
        "set_build_discount",
        |mut c: C<W>, n: i32, layers: i32| {
            c.data_mut().w().set_build_discount(n, layers);
            Ok(())
        },
    )?;
    l.func_wrap(m, "set_build_cost_pct", |mut c: C<W>, pct: i32| {
        c.data_mut().w().set_build_cost_pct(pct);
        Ok(())
    })?;
    l.func_wrap(m, "turn_start_pos", |c: C<W>, player_id: i32| {
        c.data().wr().turn_start_pos(player_id)
    })?;
    l.func_wrap(
        m,
        "turn_snap",
        |mut c: C<W>, player_id: i32, buf: i32| -> Result<i32, Error> {
            let (pos, stay, stun, exile) = c.data().wr().turn_snap(player_id);
            let mut out = [0u8; 16];
            for (i, v) in [pos, stay, stun, exile].into_iter().enumerate() {
                out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            write_guest(&mut c, buf, &out)?;
            Ok(16)
        },
    )?;
    l.func_wrap(m, "set_raze_on_buy", |mut c: C<W>, on: i32| {
        c.data_mut().w().set_raze_on_buy(on != 0);
        Ok(())
    })?;
    l.func_wrap(m, "is_agent", |c: C<W>, tile: i32| {
        c.data().wr().is_agent(tile)
    })?;
    l.func_wrap(m, "is_live_house", |c: C<W>, tile: i32| {
        c.data().wr().is_live_house(tile)
    })?;
    l.func_wrap(m, "tile_group", |c: C<W>, tile: i32| {
        c.data().wr().tile_group(tile)
    })?;
    l.func_wrap(m, "tile_price", |c: C<W>, tile: i32| {
        c.data().wr().tile_price(tile)
    })?;
    l.func_wrap(m, "houses_of", |c: C<W>, tile: i32| {
        c.data().wr().houses_of(tile)
    })?;
    l.func_wrap(m, "rent_houses_of", |c: C<W>, tile: i32| {
        c.data().wr().rent_houses_of(tile)
    })?;
    l.func_wrap(m, "set_houses", |mut c: C<W>, tile: i32, n: i32| {
        c.data_mut().w().set_houses(tile, n);
        Ok(())
    })?;
    l.func_wrap(m, "add_house", |mut c: C<W>, tile: i32, n: i32| {
        c.data_mut().w().add_house(tile, n)
    })?;
    l.func_wrap(m, "mortgaged_of", |c: C<W>, tile: i32| {
        c.data().wr().mortgaged_of(tile)
    })?;
    l.func_wrap(m, "set_mortgaged", |mut c: C<W>, tile: i32, v: i32| {
        c.data_mut().w().set_mortgaged(tile, v);
        Ok(())
    })?;
    l.func_wrap(m, "set_owner", |mut c: C<W>, tile: i32, player_id: i32| {
        c.data_mut().w().set_owner(tile, player_id);
        Ok(())
    })?;
    l.func_wrap(m, "dist", |c: C<W>, a: i32, b: i32| {
        c.data().wr().dist(a, b)
    })?;
    l.func_wrap(m, "tile_forward", |c: C<W>, a: i32, b: i32| {
        c.data().wr().tile_forward(a, b)
    })?;
    l.func_wrap(m, "neighbor", |c: C<W>, player_id: i32, dir: i32| {
        c.data().wr().neighbor(player_id, dir)
    })?;
    l.func_wrap(m, "players_on_count", |c: C<W>, tile: i32, except: i32| {
        c.data().wr().players_on_count(tile, except)
    })?;
    l.func_wrap(
        m,
        "players_on_at",
        |c: C<W>, tile: i32, except: i32, index: i32| {
            c.data().wr().players_on_at(tile, except, index)
        },
    )?;
    l.func_wrap(
        m,
        "draw",
        |mut c: C<W>, player_id: i32, n: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            // A card-driven draw is paused so the engine can raise the per-card
            // `drewBefore` point (and so a hook may replace a card of it) before
            // the cards move. The engine adjudicates; **this** run then moves
            // the plain cards on its own world copy -- the same shape as `pay`,
            // where the engine runs the Money pipeline and the effect applies
            // the payment. The answer is how many of the `n` are plain draws.
            if let Some(&plain) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                let plain = plain.clamp(0, n);
                return Ok(st.w().draw(player_id, plain));
            }
            st.host_request = Some(HostRequest::Draw { player_id, n });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(
        m,
        "draw_event",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ans) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ans);
            }
            st.host_request = Some(HostRequest::DrawEvent { player_id });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(
        m,
        "pay_rent",
        |mut c: C<W>, player_id: i32, tile: i32, half: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ans) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ans);
            }
            st.host_request = Some(HostRequest::PayRent {
                player_id,
                tile,
                half: half != 0,
            });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(
        m,
        "offer_buy",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ans) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ans);
            }
            st.host_request = Some(HostRequest::OfferBuy { player_id, tile });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(
        m,
        "offer_force_buy",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ans) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ans);
            }
            st.host_request = Some(HostRequest::OfferForceBuy { player_id, tile });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(
        m,
        "offer_build",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ans) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ans);
            }
            st.host_request = Some(HostRequest::OfferBuildOne { player_id, tile });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(
        m,
        "add_to_hand",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
            let card = guest_str(&mut c, p, n)?;
            c.data_mut().w().add_to_hand(player_id, &card);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "add_to_deck",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, shuffle: i32| -> Result<(), Error> {
            let card = guest_str(&mut c, p, n)?;
            c.data_mut().w().add_to_deck(player_id, &card, shuffle != 0);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "add_to_deck_at",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, pos: i32| -> Result<(), Error> {
            let card = guest_str(&mut c, p, n)?;
            c.data_mut().w().add_to_deck_at(player_id, &card, pos);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "take_card",
        |mut c: C<W>, player_id: i32, pile: i32, p: i32, n: i32| -> Result<i32, Error> {
            let pile = crate::CardPile::from_i32(pile)
                .ok_or_else(|| err(format!("bad card pile {pile}")))?;
            let id = guest_str(&mut c, p, n)?;
            Ok(c.data_mut().w().take_card(player_id, pile, &id) as i32)
        },
    )?;
    // Host->guest list: the guest passes its own buffer; the host writes the
    // postcard bytes only if they fit and always returns the length needed.
    l.func_wrap(
        m,
        "cards_in",
        |mut c: C<W>, player_id: i32, pile: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            let pile = crate::CardPile::from_i32(pile)
                .ok_or_else(|| err(format!("bad card pile {pile}")))?;
            let list = c.data().wr().cards_in(player_id, pile);
            let bytes =
                postcard::to_allocvec(&list).map_err(|e| err(format!("cards_in encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(
        m,
        "to_discard",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
            let card = guest_str(&mut c, p, n)?;
            c.data_mut().w().to_discard(player_id, &card);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "hand_count",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, p, n)?;
            Ok(c.data().wr().hand_count(player_id, &card))
        },
    )?;
    l.func_wrap(
        m,
        "discard_count",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, p, n)?;
            Ok(c.data().wr().discard_count(player_id, &card))
        },
    )?;
    l.func_wrap(m, "hand_size", |c: C<W>, player_id: i32| {
        c.data().wr().hand_size(player_id)
    })?;
    l.func_wrap(m, "deck_count", |c: C<W>, player_id: i32| {
        c.data().wr().deck_count(player_id)
    })?;
    l.func_wrap(m, "discard_size", |c: C<W>, player_id: i32| {
        c.data().wr().discard_size(player_id)
    })?;
    l.func_wrap(
        m,
        "discard_from_hand",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, p, n)?;
            Ok(c.data_mut().w().discard_from_hand(player_id, &card))
        },
    )?;
    l.func_wrap(
        m,
        "shuffle_into_deck",
        |mut c: C<W>, player_id: i32, hand: i32, discard: i32| {
            c.data_mut()
                .w()
                .shuffle_into_deck(player_id, hand != 0, discard != 0)
        },
    )?;
    l.func_wrap(m, "unplace_card", |mut c: C<W>| {
        c.data_mut().w().unplace_card()
    })?;
    // Active events (`docs/EVENTS.md`) -- the rule body's handles on the
    // engine's event deck and active list.
    l.func_wrap(
        m,
        "event_expire",
        |mut c: C<W>, ip: i32, il: i32, removed: i32| -> Result<(), Error> {
            let id = guest_str(&mut c, ip, il)?;
            c.data_mut().w().event_expire(&id, removed != 0);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "event_is_active",
        |mut c: C<W>, ip: i32, il: i32| -> Result<i32, Error> {
            let id = guest_str(&mut c, ip, il)?;
            Ok(c.data().wr().event_is_active(&id) as i32)
        },
    )?;
    l.func_wrap(
        m,
        "event_deck_push",
        |mut c: C<W>, ip: i32, il: i32, face_down: i32| -> Result<(), Error> {
            let id = guest_str(&mut c, ip, il)?;
            c.data_mut().w().event_deck_push(&id, face_down != 0);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "event_banish",
        |mut c: C<W>, ip: i32, il: i32| -> Result<(), Error> {
            let id = guest_str(&mut c, ip, il)?;
            c.data_mut().w().event_banish(&id);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "placed_cards",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            let names = c.data().wr().placed_cards(player_id);
            let bytes = postcard::to_allocvec(&names)
                .map_err(|e| err(format!("placed_cards encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(
        m,
        "card_text_mentions",
        |mut c: C<W>, cp: i32, cl: i32, np: i32, nl: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            let needle = guest_str(&mut c, np, nl)?;
            Ok(c.data().wr().card_text_mentions(&card, &needle) as i32)
        },
    )?;
    l.func_wrap(
        m,
        "card_crystals",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            Ok(c.data().wr().card_crystals(player_id, &card))
        },
    )?;
    l.func_wrap(
        m,
        "add_card_crystals",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, n: i32, max: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            Ok(c.data_mut().w().add_card_crystals(player_id, &card, n, max))
        },
    )?;
    l.func_wrap(m, "is_placed", |c: C<W>| c.data().wr().is_placed())?;
    l.func_wrap(m, "self_tile", |c: C<W>| c.data().wr().self_tile())?;
    l.func_wrap(m, "set_self_tile", |mut c: C<W>, tile: i32| {
        c.data_mut().w().set_self_tile(tile) as i32
    })?;
    l.func_wrap(m, "self_face_down", |c: C<W>| {
        c.data().wr().self_face_down() as i32
    })?;
    l.func_wrap(m, "set_self_face_down", |mut c: C<W>, on: i32| {
        c.data_mut().w().set_self_face_down(on != 0) as i32
    })?;
    l.func_wrap(m, "self_immune", |c: C<W>| {
        c.data().wr().self_immune() as i32
    })?;
    l.func_wrap(m, "set_self_immune", |mut c: C<W>, on: i32| {
        c.data_mut().w().set_self_immune(on != 0) as i32
    })?;
    l.func_wrap(m, "crystals", |c: C<W>| c.data().wr().crystals())?;
    l.func_wrap(m, "set_crystals", |mut c: C<W>, n: i32| {
        c.data_mut().w().set_crystals(n)
    })?;
    l.func_wrap(m, "add_crystals", |mut c: C<W>, n: i32, max: i32| {
        c.data_mut().w().add_crystals(n, max)
    })?;
    l.func_wrap(
        m,
        "self_prop",
        |mut c: C<W>, kp: i32, kl: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, kp, kl)?;
            Ok(c.data().wr().self_prop(&key))
        },
    )?;
    l.func_wrap(
        m,
        "set_self_prop",
        |mut c: C<W>, kp: i32, kl: i32, v: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, kp, kl)?;
            Ok(c.data_mut().w().set_self_prop(&key, v))
        },
    )?;
    // `docs/TILES.md`: a card that bends a tile writes the tile instance's props
    // instead of an engine flag. `prop::NO_REWARD` on the CiRCLE tile's
    // `tile:circle` instance is 「无法获取[CiRCLE奖励]」; `prop::GROUP` is
    // 「该格获得所有颜色」; and so on.
    l.func_wrap(
        m,
        "tile_prop",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, kp, kl)?;
            Ok(c.data().wr().tile_prop(tile, &key))
        },
    )?;
    l.func_wrap(
        m,
        "set_tile_prop",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32, v: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, kp, kl)?;
            Ok(c.data_mut().w().set_tile_prop(tile, &key, v))
        },
    )?;
    // `docs/TILES.md`'s `ctx::settle_circle_reward` -- the [经过] CiRCLE reward,
    // `tile:circle`'s Pass entry. Prompts, so it is a host request.
    l.func_wrap(
        m,
        "settle_circle_reward",
        |mut c: C<W>, player_id: i32, landing: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ans) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ans);
            }
            st.host_request = Some(HostRequest::CircleReward {
                player_id,
                landing: landing != 0,
            });
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(
        m,
        "count_marks",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32| -> Result<i32, Error> {
            let kind = guest_str(&mut c, kp, kl)?;
            Ok(c.data().wr().count_marks(tile, &kind, owner))
        },
    )?;
    l.func_wrap(
        m,
        "remove_marks",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32| -> Result<i32, Error> {
            let kind = guest_str(&mut c, kp, kl)?;
            Ok(c.data_mut().w().remove_marks(tile, &kind, owner))
        },
    )?;
    l.func_wrap(
        m,
        "tok",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let name = guest_str(&mut c, p, n)?;
            Ok(c.data().wr().tok(player_id, &name))
        },
    )?;
    l.func_wrap(
        m,
        "set_tok",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, v: i32| -> Result<(), Error> {
            let name = guest_str(&mut c, p, n)?;
            c.data_mut().w().set_tok(player_id, &name, v);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "add_tok",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, by: i32, max: i32| -> Result<i32, Error> {
            let name = guest_str(&mut c, p, n)?;
            Ok(c.data_mut().w().add_tok(player_id, &name, by, max))
        },
    )?;
    // Keyed state: `{value, min, max, expires}` items. `field` selects the
    // column (0 = value, 1 = min, 2 = max, 3 = expires: 0 none / 1 turn-start /
    // 2 turn-end). The engine holds these and enforces nothing.
    l.func_wrap(
        m,
        "state_get",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, field: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, p, n)?;
            Ok(match field {
                1 => c.data().wr().state_min(player_id, &key),
                2 => c.data().wr().state_max(player_id, &key),
                3 => match c.data().wr().state_expires(player_id, &key) {
                    None => 0,
                    Some(game_core::state::Tick::TurnStart) => 1,
                    Some(game_core::state::Tick::TurnEnd) => 2,
                },
                _ => c.data().wr().state_get(player_id, &key),
            })
        },
    )?;
    l.func_wrap(
        m,
        "state_set",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, field: i32, v: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, p, n)?;
            Ok(match field {
                1 => {
                    let max = c.data().wr().state_max(player_id, &key);
                    c.data_mut().w().state_set_bounds(player_id, &key, v, max);
                    v
                }
                2 => {
                    let min = c.data().wr().state_min(player_id, &key);
                    c.data_mut().w().state_set_bounds(player_id, &key, min, v);
                    v
                }
                3 => {
                    let e = match v {
                        1 => Some(game_core::state::Tick::TurnStart),
                        2 => Some(game_core::state::Tick::TurnEnd),
                        _ => None,
                    };
                    c.data_mut().w().state_set_expires(player_id, &key, e);
                    v
                }
                _ => c.data_mut().w().state_set(player_id, &key, v),
            })
        },
    )?;
    l.func_wrap(
        m,
        "state_add",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, delta: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, p, n)?;
            Ok(c.data_mut().w().state_add(player_id, &key, delta))
        },
    )?;

    l.func_wrap(
        m,
        "slot",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, p, n)?;
            Ok(c.data().wr().slot(player_id, &key))
        },
    )?;
    l.func_wrap(
        m,
        "set_slot",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, v: i32| -> Result<(), Error> {
            let key = guest_str(&mut c, p, n)?;
            c.data_mut().w().set_slot(player_id, &key, v);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "inc_slot",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, by: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, p, n)?;
            Ok(c.data_mut().w().inc_slot(player_id, &key, by))
        },
    )?;
    l.func_wrap(m, "band_crystals", |c: C<W>, player_id: i32| {
        c.data().wr().band_crystals(player_id)
    })?;
    l.func_wrap(
        m,
        "add_band_crystals",
        |mut c: C<W>, player_id: i32, n: i32, max: i32| {
            c.data_mut().w().add_band_crystals(player_id, n, max)
        },
    )?;
    // v35: the skill / band-skill attachment surface (C# `H._fx[i].bands` /
    // `.skill`) and the skill invoke. `band_skill` / `character_skill` answer a
    // postcard `String` (empty = none); `band_skills` answers a postcard
    // `Vec<(uid, id, extra)>`.
    l.func_wrap(
        m,
        "band_skill",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            let s = c.data().wr().band_skill_id(player_id).unwrap_or_default();
            let bytes =
                postcard::to_allocvec(&s).map_err(|e| err(format!("band_skill encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(
        m,
        "character_skill",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            let s = c
                .data()
                .wr()
                .character_skill_id(player_id)
                .unwrap_or_default();
            let bytes = postcard::to_allocvec(&s)
                .map_err(|e| err(format!("character_skill encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(
        m,
        "band_skills",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            let list = c.data().wr().band_skills(player_id);
            let bytes = postcard::to_allocvec(&list)
                .map_err(|e| err(format!("band_skills encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(
        m,
        "add_band_skill",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, extra: i32| -> Result<i32, Error> {
            let id = guest_str(&mut c, p, n)?;
            Ok(c.data_mut().w().add_band_skill(player_id, &id, extra != 0))
        },
    )?;
    // Run a skill rule's press entry (`On::Play`) for a player. Same nested-run
    // shape as `play_card` below; the id is expected to be a `skill:` rule.
    l.func_wrap(
        m,
        "invoke_skill",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let id = guest_str(&mut c, p, n)?;
            let rules = c
                .data()
                .rules
                .clone()
                .ok_or_else(|| err("invoke_skill unavailable here"))?;
            let card = *rules
                .by_id
                .get(&id)
                .ok_or_else(|| err(format!("invoke_skill: unknown skill {id:?}")))?;
            if c.data().depth >= MAX_NESTING {
                return Err(err(format!(
                    "invoke_skill nested deeper than {MAX_NESTING}"
                )));
            }
            let fuel = c.get_fuel()?;
            let st = c.data_mut();
            let saved = st.w().enter_card(&id);
            let was_from_hand = st.w().play_from_hand();
            st.w().set_play_from_hand(false);
            let mut nested = HostState::new(
                rules.clone(),
                st.world.take().expect("world present"),
                std::mem::take(&mut st.answers),
                st.depth + 1,
            );
            nested.next_answer = st.next_answer;
            let mut store = new_store(&rules.engine, nested);
            store.set_fuel(fuel)?;
            let res = match rules.cards[card as usize].entry(OnKind::Play, None) {
                Some(entry) => {
                    call_card(&rules, &mut store, card, entry, export::OP_RUN, player_id)
                        .map(|_| ())
                }
                None => Ok(()),
            };
            let left = store.get_fuel().unwrap_or(0);
            let inner = store.into_data();
            let st = c.data_mut();
            st.world = inner.world;
            if let Some(w) = st.world.as_mut() {
                w.set_play_from_hand(was_from_hand);
            }
            st.answers = inner.answers;
            st.next_answer = inner.next_answer;
            if inner.asked.is_some() {
                st.asked = inner.asked;
            }
            if inner.host_request.is_some() {
                st.host_request = inner.host_request;
            }
            let dest = st.w().leave_card(saved);
            c.set_fuel(left)?;
            match res {
                Ok(()) => Ok(dest),
                Err(e) if is_need_input(&e) => Ok(abi::EXIT_NEED_INPUT),
                Err(e) => Err(e),
            }
        },
    )?;
    // C# `f.Bought(i, t)` -- the engine raises the `bought` hook chain.
    l.func_wrap(
        m,
        "raise_bought",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::RaiseBought { player_id, tile });
            Err(need_input())
        },
    )?;
    l.func_wrap(m, "fire", |c: C<W>, player_id: i32| {
        c.data().wr().fire(player_id)
    })?;
    l.func_wrap(m, "fire_max", |c: C<W>, player_id: i32| {
        c.data().wr().fire_max(player_id)
    })?;
    l.func_wrap(
        m,
        "gain_fire",
        |mut c: C<W>, player_id: i32, n: i32, p: i32, l: i32| -> Result<i32, Error> {
            let why = guest_msg(&mut c, p, l)?;
            Ok(c.data_mut().w().gain_fire(player_id, n, why))
        },
    )?;
    // Abnormal effects pass the C# `AbnormalGate` first (C# `GiveStay` /
    // `GiveStun` / `GiveExile` / `ForceTeleport` all run it): a blocked one
    // does nothing. Stripping layers (`n < 0`, 「清除」) is not applying an
    // abnormal status and bypasses the gate.
    l.func_wrap(
        m,
        "give_stay",
        |mut c: C<W>, player_id: i32, n: i32| -> Result<(), Error> {
            if n < 0 {
                c.data_mut().w().give_stay(player_id, n);
            } else if n > 0 && gate(&mut c, player_id, AbKind::Stay)? {
                c.data_mut().w().give_stay(player_id, n);
            }
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "give_stun",
        |mut c: C<W>, player_id: i32, n: i32| -> Result<(), Error> {
            if n < 0 {
                c.data_mut().w().give_stun(player_id, n);
            } else if n > 0 && gate(&mut c, player_id, AbKind::Stun)? {
                c.data_mut().w().give_stun(player_id, n);
            }
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "give_exile",
        |mut c: C<W>, player_id: i32, n: i32, to: i32| -> Result<(), Error> {
            if n > 0 && gate(&mut c, player_id, AbKind::Exile)? {
                c.data_mut().w().give_exile(player_id, n, to);
            }
            Ok(())
        },
    )?;
    l.func_wrap(m, "give_extra_turn", |mut c: C<W>, player_id: i32| {
        c.data_mut().w().give_extra_turn(player_id);
        Ok(())
    })?;
    l.func_wrap(m, "can_pay", |c: C<W>, player_id: i32| {
        c.data().wr().can_pay(player_id)
    })?;
    l.func_wrap(m, "cant_move", |c: C<W>, player_id: i32| {
        c.data().wr().cant_move(player_id)
    })?;
    l.func_wrap(
        m,
        "spend_fire",
        |mut c: C<W>, player_id: i32, n: i32, p: i32, l: i32| -> Result<i32, Error> {
            let why = guest_msg(&mut c, p, l)?;
            Ok(c.data_mut().w().spend_fire(player_id, n, why))
        },
    )?;
    l.func_wrap(m, "stay_of", |c: C<W>, player_id: i32| {
        c.data().wr().stay_of(player_id)
    })?;
    l.func_wrap(m, "stun_of", |c: C<W>, player_id: i32| {
        c.data().wr().stun_of(player_id)
    })?;
    l.func_wrap(m, "turn_player", |c: C<W>| c.data().wr().turn_player())?;
    l.func_wrap(m, "round_no", |c: C<W>| c.data().wr().round_no())?;
    l.func_wrap(m, "turn_key", |c: C<W>| c.data().wr().turn_key())?;
    l.func_wrap(
        m,
        "character_is",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let name = guest_str(&mut c, p, n)?;
            Ok(c.data().wr().character_is(player_id, &name))
        },
    )?;
    l.func_wrap(
        m,
        "in_band",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let name = guest_str(&mut c, p, n)?;
            Ok(c.data().wr().in_band(player_id, &name))
        },
    )?;
    l.func_wrap(
        m,
        "bump_mark",
        |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32, delta: i32| -> Result<i32, Error> {
            let kind = guest_str(&mut c, kp, kl)?;
            Ok(c.data_mut().w().bump_mark(tile, &kind, owner, delta))
        },
    )?;
    // `H.DoMoveRoll` -- plain, like `roll`: no prompts and no raise points (the
    // caller is usually *inside* `moveRoll`), so there is nothing to pause for.
    l.func_wrap(
        m,
        "do_move_roll",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            Ok(c.data_mut().w().do_move_roll(player_id))
        },
    )?;
    l.func_wrap(
        m,
        "tok_names",
        |mut c: C<W>, player_id: i32, p: i32, n: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            let prefix = guest_str(&mut c, p, n)?;
            let names = c.data().wr().tok_names(player_id, &prefix);
            let bytes =
                postcard::to_allocvec(&names).map_err(|e| err(format!("tok_names encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(
        m,
        "unplace_card_named",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            Ok(c.data_mut().w().unplace_card_named(player_id, &card) as i32)
        },
    )?;
    l.func_wrap(m, "can_build_on", |c: C<W>, player_id: i32, tile: i32| {
        c.data().wr().can_build_on(player_id, tile) as i32
    })?;
    l.func_wrap(
        m,
        "card_face_down",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            Ok(c.data().wr().card_face_down(player_id, &card) as i32)
        },
    )?;
    l.func_wrap(m, "extreme", |c: C<W>| c.data().wr().extreme())?;
    l.func_wrap(m, "set_extreme", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_extreme(v)
    })?;
    l.func_wrap(m, "play_from_hand", |c: C<W>| {
        c.data().wr().play_from_hand() as i32
    })?;
    l.func_wrap(
        m,
        "gain_fixed",
        |mut c: C<W>, player_id: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
            let why = guest_msg(&mut c, p, n)?;
            Ok(c.data_mut().w().gain_fixed(player_id, amount, why))
        },
    )?;
    l.func_wrap(
        m,
        "set_card_face_down",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, down: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            Ok(c.data_mut()
                .w()
                .set_card_face_down(player_id, &card, down != 0) as i32)
        },
    )?;
    l.func_wrap(m, "clear_dice", |mut c: C<W>| -> Result<(), Error> {
        c.data_mut().w().clear_dice();
        Ok(())
    })?;
    l.func_wrap(
        m,
        "set_card_immune",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, on: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            Ok(c.data_mut().w().set_card_immune(player_id, &card, on != 0) as i32)
        },
    )?;
    l.func_wrap(
        m,
        "card_immune",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            Ok(c.data().wr().card_immune(player_id, &card) as i32)
        },
    )?;
    l.func_wrap(
        m,
        "set_card_tile",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, tile: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            Ok(c.data_mut().w().set_card_tile(player_id, &card, tile) as i32)
        },
    )?;
    l.func_wrap(
        m,
        "place_card_on",
        |mut c: C<W>,
         player_id: i32,
         tile: i32,
         cp: i32,
         cl: i32,
         p: i32,
         n: i32|
         -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            let note = guest_msg(&mut c, p, n)?;
            Ok(c.data_mut().w().place_card_on(player_id, tile, &card, note))
        },
    )?;
    l.func_wrap(
        m,
        "place_card_at",
        |mut c: C<W>, player_id: i32, cp: i32, cl: i32, p: i32, n: i32| -> Result<i32, Error> {
            let card = guest_str(&mut c, cp, cl)?;
            let note = guest_msg(&mut c, p, n)?;
            Ok(c.data_mut().w().place_card(player_id, &card, note))
        },
    )?;
    l.func_wrap(m, "set_dest", |mut c: C<W>, dest: i32| {
        c.data_mut().w().set_dest(dest);
        Ok(())
    })?;
    l.func_wrap(
        m,
        "set_transfer_to_dest",
        |mut c: C<W>, to: i32, dest: i32| {
            c.data_mut().w().set_transfer_to_dest(to, dest);
            Ok(())
        },
    )?;
    l.func_wrap(m, "send_to_dest", |mut c: C<W>, dest: i32| {
        Ok(c.data_mut().w().send_to_dest(dest).unwrap_or(-1))
    })?;
    l.func_wrap(m, "transfer_to_dest", |mut c: C<W>, to: i32, dest: i32| {
        Ok(c.data_mut().w().transfer_to_dest(to, dest).unwrap_or(-1))
    })?;
    l.func_wrap(m, "ring_multiplier", |c: C<W>| {
        c.data().wr().ring_multiplier()
    })?;
    l.func_wrap(m, "add_ring_bonus", |mut c: C<W>, n: i32| {
        c.data_mut().w().add_ring_bonus(n)
    })?;
    l.func_wrap(
        m,
        "teleport_to",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<(), Error> {
            if gate(&mut c, player_id, AbKind::Teleport)? {
                c.data_mut().w().teleport_to(player_id, tile);
            }
            Ok(())
        },
    )?;
    // The C# `H.AbnormalGate` -- the helper above has been here since the
    // `Gate` request landed, but never got a linker entry, so no card could
    // open the path. This is the entry.
    l.func_wrap(
        m,
        "gate",
        |mut c: C<W>, player_id: i32, kind: i32| -> Result<i32, Error> {
            let Some(kind) = crate::AbKind::from_i32(kind) else {
                return Err(err(format!("gate: unknown abnormal kind {kind}")));
            };
            Ok(gate(&mut c, player_id, kind)? as i32)
        },
    )?;
    l.func_wrap(m, "abnormal_count", |c: C<W>, player_id: i32| {
        c.data().wr().abnormal_count(player_id)
    })?;
    l.func_wrap(m, "targeted_count", |c: C<W>, player_id: i32| {
        c.data().wr().targeted_count(player_id)
    })?;
    l.func_wrap(m, "gains_this_turn", |c: C<W>, player_id: i32| {
        c.data().wr().gains_this_turn(player_id)
    })?;
    // The static targeting query + per-pair cancel (C# `H.Db.Card(id).Targeting`
    // / `play.Tags["immune"+seat]`).
    l.func_wrap(
        m,
        "designations",
        |mut c: C<W>, player_id: i32, buf: i32, cap: i32| -> Result<i32, Error> {
            let list = c.data().wr().designations(player_id);
            let bytes = postcard::to_allocvec(&list)
                .map_err(|e| err(format!("designations encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(m, "cancel_designation", |mut c: C<W>, seat: i32| -> Result<(), Error> {
        c.data_mut().w().cancel_designation(seat);
        Ok(())
    })?;
    l.func_wrap(m, "designation_cancelled", |c: C<W>, seat: i32| {
        c.data().wr().designation_cancelled(seat) as i32
    })?;
    l.func_wrap(
        m,
        "target",
        |mut c: C<W>, player_id: i32, tile: i32, single: i32| -> Result<i32, Error> {
            // Paused like `gate`: the engine runs the targeting pipeline (it
            // raises hooks and opens the `target` [反击] window) and the replay
            // reads its answer.
            let st = c.data_mut();
            if let Some(&got) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(got);
            }
            st.host_request = Some(HostRequest::Target {
                player_id,
                tile,
                single: single != 0,
            });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "card_move",
        |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
            // C# `H.CardMove(c, m)` -- the card shaped the plan and wants the move
            // to run *now*. Paused like the others: the engine runs `Cx::card_move`
            // (which may prompt) and the replay reads the answer. The plan is
            // captured here because the run's world copy is discarded on pause.
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            let plan = st.w().move_plan();
            st.host_request = Some(HostRequest::Move { player_id, plan });
            Err(need_input())
        },
    )?;
    // `ctx::roll_ask` / `ctx::do_move_roll_ask` -- a card- or skill-driven dice
    // roll. The engine rolls and raises the `Roll` chain link (the 「掷骰结算前」
    // [反击] window) with the roller, the face and `source`; the answer is the
    // face a counteraction left. `sides == 0` is the `do_move_roll` shape.
    l.func_wrap(
        m,
        "roll_ask",
        |mut c: C<W>,
         player_id: i32,
         count: i32,
         sides: i32,
         source: i32|
         -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&face) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(face);
            }
            st.host_request = Some(HostRequest::Roll {
                player_id,
                count,
                sides,
                source,
            });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "agent_landing",
        |mut c: C<W>, player_id: i32, agent: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::AgentLanding { player_id, agent });
            Err(need_input())
        },
    )?;
    // The rest of the routine family, same shape as `card_move`: the card asks,
    // the run pauses, the engine runs the real routine, the effect replays past.
    // These pause by trapping (see `gate` above) -- their guest wrappers return
    // `bool`, so a sentinel would read as "it worked".
    l.func_wrap(
        m,
        "card_settle_at",
        |mut c: C<W>, player_id: i32, tile: i32, main: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::SettleAt {
                player_id,
                tile,
                main: main != 0,
            });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "card_buy",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::Buy { player_id, tile });
            Err(need_input())
        },
    )?;
    // v40 purchase surface (`docs/PURCHASE.md`). P0 registers the ABI; the
    // engine-side dispatch lands with P1 (property buy) / P2 (agent) /
    // P3 (force & acquire) / P5 (linger).
    l.func_wrap(
        m,
        "buy",
        |mut c: C<W>, player_id: i32, tile: i32, _kind: i32| -> Result<i32, Error> {
            // P0: same as `card_buy`; the kind selects the pipeline shape at P1+.
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::Buy { player_id, tile });
            Err(need_input())
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
            let bytes = read_guest(&mut c, buf, n)?;
            let tiles: Vec<i32> = bytes
                .chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::BuyQuotes {
                player_id,
                kind,
                tiles,
                out,
            });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "acquire",
        |mut c: C<W>, player_id: i32, from: i32, tile: i32, price: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::Acquire {
                player_id,
                from,
                tile,
                price,
            });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "agent_offer",
        |mut c: C<W>, player_id: i32, agent: i32, tile: i32, kind: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::AgentOffer {
                player_id,
                agent,
                tile,
                kind,
            });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "linger",
        |mut c: C<W>, player_id: i32, expires: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::Linger {
                player_id,
                expires,
            });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "card_build",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::Build { player_id, tile });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "card_offer_build",
        |mut c: C<W>, player_id: i32, buf: i32, n: i32| -> Result<i32, Error> {
            let bytes = read_guest(&mut c, buf, n)?;
            let tiles: Vec<i32> = bytes
                .chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::OfferBuild { player_id, tiles });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "card_mortgage",
        |mut c: C<W>, player_id: i32, tile: i32| -> Result<i32, Error> {
            let st = c.data_mut();
            if let Some(&ok) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(ok);
            }
            st.host_request = Some(HostRequest::Mortgage { player_id, tile });
            Err(need_input())
        },
    )?;
    l.func_wrap(
        m,
        "placed_tile",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let id = guest_str(&mut c, p, n)?;
            Ok(c.data().wr().placed_tile(player_id, &id))
        },
    )?;
    l.func_wrap(m, "play_doubled", |c: C<W>| c.data().wr().play_doubled())?;
    l.func_wrap(
        m,
        "set_play_doubled",
        |mut c: C<W>, n: i32| -> Result<(), Error> {
            c.data_mut().w().set_play_doubled(n);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "trig_cards",
        |mut c: C<W>, buf: i32, cap: i32| -> Result<i32, Error> {
            let list = c.data().wr().trigger().cards;
            let bytes =
                postcard::to_allocvec(&list).map_err(|e| err(format!("trig_cards encode: {e}")))?;
            if bytes.len() as i32 <= cap {
                write_guest(&mut c, buf, &bytes)?;
            }
            Ok(bytes.len() as i32)
        },
    )?;
    l.func_wrap(m, "opt_int", |mut c: C<W>, v: i32| {
        c.data_mut().options.push(PromptOption::Int(v))
    })?;
    l.func_wrap(
        m,
        "opt_str",
        |mut c: C<W>, p: i32, n: i32| -> Result<(), Error> {
            let s = guest_msg(&mut c, p, n)?;
            c.data_mut().options.push(PromptOption::Str(s));
            Ok(())
        },
    )?;
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
            let kind =
                PromptKind::from_i32(kind).ok_or_else(|| err(format!("bad prompt kind {kind}")))?;
            let title = guest_msg(&mut c, tp, tl)?;
            let text = guest_msg(&mut c, xp, xl)?;
            let st = c.data_mut();
            let options = std::mem::take(&mut st.options);
            if kind != PromptKind::YesNo && options.is_empty() {
                return Err(err(format!("{} prompt with no options", kind.as_str())));
            }
            if let Some(&a) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(a);
            }
            st.asked = Some(Prompt {
                kind,
                player_id,
                title,
                text,
                options,
                answer_slot: st.next_answer,
            });
            // Like `pay`: the pause is the sentinel as the return value, so
            // `ctx::ask_*` can hand the card `Err(Prompt)` instead of unwinding.
            Ok(abi::EXIT_NEED_INPUT)
        },
    )?;
    l.func_wrap(m, "trig_kind", |c: C<W>| {
        c.data().wr().trigger().kind as i32
    })?;
    l.func_wrap(m, "trig_player", |c: C<W>| {
        c.data().wr().trigger().player_id
    })?;
    l.func_wrap(m, "trig_target", |c: C<W>| c.data().wr().trigger().target)?;
    l.func_wrap(m, "trig_tile", |c: C<W>| c.data().wr().trigger().tile)?;
    l.func_wrap(m, "trig_value", |c: C<W>| c.data().wr().trigger().value)?;
    l.func_wrap(m, "trig_step", |c: C<W>| c.data().wr().trigger().step)?;
    l.func_wrap(m, "trig_by_card", |c: C<W>| {
        c.data().wr().trigger().by_card.unwrap_or(-1)
    })?;
    l.func_wrap(m, "trig_pay_is_rent", |c: C<W>| {
        c.data().wr().trigger().pay_is_rent as i32
    })?;
    l.func_wrap(m, "trig_move_kind", |c: C<W>| {
        c.data().wr().trigger().move_kind.map_or(-1, |k| k as i32)
    })?;
    l.func_wrap(m, "trig_move_resolve", |c: C<W>| {
        c.data().wr().trigger().move_resolve as i32
    })?;
    l.func_wrap(
        m,
        "trig_move_tag",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, p, n)?;
            Ok(c.data()
                .wr()
                .trigger()
                .move_tags
                .iter()
                .find(|(k, _)| *k == key)
                .map_or(0, |(_, v)| *v))
        },
    )?;
    l.func_wrap(m, "trig_move_main", |c: C<W>| {
        c.data().wr().trigger().move_main as i32
    })?;
    l.func_wrap(m, "trig_move_dir", |c: C<W>| {
        c.data().wr().trigger().move_dir
    })?;
    l.func_wrap(m, "trig_move_remaining", |c: C<W>| {
        c.data().wr().trigger().move_remaining
    })?;
    l.func_wrap(m, "trig_move_total", |c: C<W>| {
        c.data().wr().trigger().move_total
    })?;
    l.func_wrap(m, "trig_move_roll", |c: C<W>| {
        c.data().wr().trigger().move_roll.unwrap_or(-1)
    })?;
    l.func_wrap(m, "trig_roll_source", |c: C<W>| {
        c.data().wr().trigger().roll_source
    })?;
    // v40 purchase payload (`docs/PURCHASE.md`)
    l.func_wrap(m, "trig_buy_kind", |c: C<W>| {
        c.data().wr().trigger().buy_kind
    })?;
    l.func_wrap(m, "trig_seller", |c: C<W>| c.data().wr().trigger().seller)?;
    l.func_wrap(m, "trig_price", |c: C<W>| c.data().wr().trigger().price)?;
    l.func_wrap(m, "trig_set_price", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_trigger_price(v)
    })?;
    l.func_wrap(m, "trig_deal_owner", |c: C<W>| {
        c.data().wr().trigger().deal_owner
    })?;
    l.func_wrap(m, "trig_set_deal_owner", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_trigger_deal_owner(v)
    })?;
    l.func_wrap(m, "trig_deal_houses", |c: C<W>| {
        c.data().wr().trigger().deal_houses
    })?;
    l.func_wrap(m, "trig_set_deal_houses", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_trigger_deal_houses(v)
    })?;
    l.func_wrap(m, "trig_deal_mortgaged", |c: C<W>| {
        c.data().wr().trigger().deal_mortgaged as i32
    })?;
    l.func_wrap(m, "trig_set_deal_mortgaged", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_trigger_deal_mortgaged(v)
    })?;
    l.func_wrap(m, "trig_set_reason", |mut c: C<W>, p: i32, n: i32| -> Result<(), Error> {
        let reason = guest_str(&mut c, p, n)?;
        c.data_mut().w().set_trigger_reason(&reason);
        Ok(())
    })?;
    l.func_wrap(m, "trig_set_move_roll", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_trigger_move_roll(v)
    })?;
    l.func_wrap(m, "trig_set_pay_amount", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_trigger_value(v)
    })?;
    l.func_wrap(m, "trig_set_pay_target", |mut c: C<W>, to: i32| {
        c.data_mut().w().set_trigger_target(to)
    })?;
    l.func_wrap(m, "trig_set_cancelled", |mut c: C<W>| {
        c.data_mut().w().set_trigger_cancelled()
    })?;
    l.func_wrap(m, "trig_set_negate_effect", |mut c: C<W>| {
        c.data_mut().w().set_trigger_negate_effect()
    })?;
    l.func_wrap(m, "trig_set_spare", |mut c: C<W>, seat: i32| {
        c.data_mut().w().set_trigger_spare(seat)
    })?;
    l.func_wrap(m, "trig_cancelled", |c: C<W>| {
        c.data().wr().trigger().is_cancelled() as i32
    })?;
    l.func_wrap(m, "trig_seq", |c: C<W>| c.data().wr().trigger().seq as i32)?;
    l.func_wrap(m, "trig_answers", |c: C<W>| {
        c.data().wr().trigger().answers as i32
    })?;
    l.func_wrap(m, "trig_effect_count", |c: C<W>| {
        c.data().wr().trigger().effects.len() as i32
    })?;
    l.func_wrap(m, "trig_effect_kind", |c: C<W>, i: i32| -> i32 {
        let t = c.data().wr().trigger();
        t.effects
            .get(i.max(0) as usize)
            .map_or(-1, |e| crate::TriggerKind::from_str(e.kind) as i32)
    })?;
    l.func_wrap(m, "trig_effect_target", |c: C<W>, i: i32| -> i32 {
        c.data()
            .wr()
            .trigger()
            .effects
            .get(i.max(0) as usize)
            .map_or(-1, |e| e.target)
    })?;
    l.func_wrap(m, "trig_effect_from", |c: C<W>, i: i32| -> i32 {
        c.data()
            .wr()
            .trigger()
            .effects
            .get(i.max(0) as usize)
            .map_or(-1, |e| e.from)
    })?;
    l.func_wrap(m, "trig_effect_tile", |c: C<W>, i: i32| -> i32 {
        c.data()
            .wr()
            .trigger()
            .effects
            .get(i.max(0) as usize)
            .map_or(-1, |e| e.tile)
    })?;
    l.func_wrap(m, "trig_effect_value", |c: C<W>, i: i32| -> i32 {
        c.data()
            .wr()
            .trigger()
            .effects
            .get(i.max(0) as usize)
            .map_or(0, |e| e.value)
    })?;
    l.func_wrap(
        m,
        "declare_effect",
        |mut c: C<W>, kind: i32, target: i32, from: i32, tile: i32, value: i32| {
            c.data_mut()
                .w()
                .declare_trigger_effect(kind, target, from, tile, value)
        },
    )?;
    l.func_wrap(
        m,
        "trig_card_is",
        |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
            let id = guest_str(&mut c, p, n)?;
            Ok(c.data().wr().trig_card_is(&id))
        },
    )?;

    // Cross-module call: run another card's `play` inside this run. The world, the
    // answer log position and the remaining fuel move into a nested store and back,
    // so a prompt raised by the inner card aborts (and later replays) the outer run.
    l.func_wrap(
        m,
        "play_card",
        |mut c: C<W>, p: i32, n: i32, player_id: i32| -> Result<i32, Error> {
            let id = guest_str(&mut c, p, n)?;
            let rules = c
                .data()
                .rules
                .clone()
                .ok_or_else(|| err("play_card unavailable here"))?;
            let card = *rules
                .by_id
                .get(&id)
                .ok_or_else(|| err(format!("play_card: unknown card {id:?}")))?;
            if c.data().depth >= MAX_NESTING {
                return Err(err(format!("play_card nested deeper than {MAX_NESTING}")));
            }
            let fuel = c.get_fuel()?;
            let st = c.data_mut();
            // The inner card runs as itself: its own id for place/crystals and its
            // own `Dest`, which must not overwrite the outer card's.
            let saved = st.w().enter_card(&id);
            // `PlayCtx.FromDeck` -- a card run through `play_card` did not come out
            // of a hand (「从手牌以外的地方打出」). Restored after so the outer card
            // still sees its own origin.
            let was_from_hand = st.w().play_from_hand();
            st.w().set_play_from_hand(false);
            let mut nested = HostState::new(
                rules.clone(),
                st.world.take().expect("world present"),
                std::mem::take(&mut st.answers),
                st.depth + 1,
            );
            nested.next_answer = st.next_answer;
            let mut store = new_store(&rules.engine, nested);
            store.set_fuel(fuel)?;
            let res = match rules.cards[card as usize].entry(OnKind::Play, None) {
                Some(entry) => {
                    call_card(&rules, &mut store, card, entry, export::OP_RUN, player_id)
                        .map(|_| ())
                }
                None => Ok(()),
            };
            let left = store.get_fuel().unwrap_or(0);
            let inner = store.into_data();
            let st = c.data_mut();
            st.world = inner.world;
            if let Some(w) = st.world.as_mut() {
                w.set_play_from_hand(was_from_hand);
            }
            st.answers = inner.answers;
            st.next_answer = inner.next_answer;
            if inner.asked.is_some() {
                st.asked = inner.asked;
            }
            if inner.host_request.is_some() {
                st.host_request = inner.host_request;
            }
            let dest = st.w().leave_card(saved);
            c.set_fuel(left)?;
            // A nested card that stopped for a prompt hands the pause to the
            // *outer* card as the sentinel, so `ctx::play_card` returns
            // `Err(Prompt)` and the outer run can `?` it. The nested question
            // (`inner.asked` / `inner.host_request`) is already merged into the
            // outer state above -- that is what the outer `call_card` reads once
            // it folds this sentinel back. `dest` is a `Dest` discriminant
            // (small), so it cannot collide with the sentinel; the guest checks
            // the sentinel before `Dest::from_i32` too.
            match res {
                Ok(()) => Ok(dest),
                Err(e) if is_need_input(&e) => Ok(abi::EXIT_NEED_INPUT),
                Err(e) => Err(e),
            }
        },
    )?;

    // `H.CanReplay` -- could `player_id` play card `id` now? A pure query: the card's
    // `cant_play` runs on a throwaway copy of the world. A card with no `play`
    // effect is never replayable; a guard that prompts or traps counts as "no".
    l.func_wrap(
        m,
        "card_replayable",
        |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
            let id = guest_str(&mut c, p, n)?;
            let rules = c
                .data()
                .rules
                .clone()
                .ok_or_else(|| err("card_replayable unavailable here"))?;
            let Some(&card) = rules.by_id.get(&id) else {
                return Ok(0);
            };
            if !rules.cards[card as usize].has_play() || c.data().depth >= MAX_NESTING {
                return Ok(0);
            }
            let Some(entry) = rules.cards[card as usize].entry(OnKind::Play, None) else {
                return Ok(1);
            };
            let fuel = c.get_fuel()?;
            let mut world = c.data().wr().clone();
            world.enter_card(&id);
            let depth = c.data().depth + 1;
            let mut store = new_store(
                &rules.engine,
                HostState::new(rules.clone(), world, vec![], depth),
            );
            store.set_fuel(fuel)?;
            // The `Play` gate again, on a throwaway copy: `OP_GUARD`, not the
            // effect. A gate that prompts or traps lands in `res` as an `Err`
            // and counts as "no", which is what this query wants.
            let res = call_card_msg(&rules, &mut store, card, entry, export::OP_GUARD, player_id);
            let left = store.get_fuel().unwrap_or(0);
            c.set_fuel(left)?;
            Ok(matches!(res, Ok(None)) as i32)
        },
    )?;
    // `mode`: bit 1 = the end of `player_id`'s next turn, bit 2 = before the wear-off.
    l.func_wrap(
        m,
        "schedule_turn_end",
        |mut c: C<W>, player_id: i32, mode: i32| {
            c.data_mut()
                .w()
                .schedule_turn_end(player_id, mode & 1 != 0, mode & 2 != 0)
        },
    )?;
    l.func_wrap(m, "set_no_money_loss", |mut c: C<W>, player_id: i32| {
        c.data_mut().w().set_no_money_loss(player_id)
    })?;
    l.func_wrap(m, "set_fixed_roll", |mut c: C<W>, n: i32| {
        c.data_mut().w().set_fixed_roll(n)
    })?;
    l.func_wrap(m, "fixed_roll", |c: C<W>| c.data().wr().fixed_roll())?;
    l.func_wrap(
        m,
        "set_next_steps",
        |mut c: C<W>, player_id: i32, n: i32| c.data_mut().w().set_next_steps(player_id, n),
    )?;
    l.func_wrap(m, "turn_main_steps", |c: C<W>| {
        c.data().wr().turn_main_steps()
    })?;
    l.func_wrap(m, "add_fire_max", |mut c: C<W>, player_id: i32, n: i32| {
        c.data_mut().w().add_fire_max(player_id, n)
    })?;
    // movement shaping: the move being planned (TurnCtx.plan)
    l.func_wrap(m, "set_roller", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_roller(v)
    })?;
    l.func_wrap(m, "set_steps", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_steps(v)
    })?;
    l.func_wrap(m, "set_reverse", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_reverse(v != 0)
    })?;
    l.func_wrap(m, "set_signed", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_signed(v != 0)
    })?;
    l.func_wrap(m, "set_stop_at", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_stop_at(v)
    })?;
    l.func_wrap(m, "set_parity", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_parity(v)
    })?;
    l.func_wrap(m, "set_resolve", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_resolve(v != 0)
    })?;
    l.func_wrap(m, "set_no_buy", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_no_buy(v != 0)
    })?;
    l.func_wrap(m, "set_kind", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_kind(v)
    })?;
    l.func_wrap(m, "set_trigger_target", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_trigger_target(v)
    })?;
    l.func_wrap(m, "set_trigger_cancelled", |mut c: C<W>| {
        c.data_mut().w().set_trigger_cancelled()
    })?;
    l.func_wrap(m, "set_teleport_to", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_teleport_to(v)
    })?;
    l.func_wrap(
        m,
        "set_start",
        |mut c: C<W>, t: i32, p: i32, n: i32| -> Result<(), Error> {
            let why = guest_str(&mut c, p, n)?;
            c.data_mut().w().set_start(t, &why);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "set_base_dice",
        |mut c: C<W>, count: i32, sides: i32, p: i32, n: i32| -> Result<(), Error> {
            let why = guest_str(&mut c, p, n)?;
            c.data_mut().w().set_base_dice(count, sides, &why);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "add_base_dice",
        |mut c: C<W>, count: i32, sides: i32, p: i32, n: i32| -> Result<(), Error> {
            let why = guest_str(&mut c, p, n)?;
            c.data_mut().w().add_base_dice(count, sides, &why);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "add_extra_dice",
        |mut c: C<W>, count: i32, sides: i32, p: i32, n: i32| -> Result<(), Error> {
            let why = guest_str(&mut c, p, n)?;
            c.data_mut().w().add_extra_dice(count, sides, &why);
            Ok(())
        },
    )?;
    l.func_wrap(
        m,
        "set_tag",
        |mut c: C<W>, kp: i32, kl: i32, v: i32| -> Result<(), Error> {
            let key = guest_str(&mut c, kp, kl)?;
            c.data_mut().w().set_tag(&key, v);
            Ok(())
        },
    )?;
    l.func_wrap(m, "set_min_roll", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_min_roll(v)
    })?;
    l.func_wrap(m, "set_extra_steps", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_extra_steps(v)
    })?;
    l.func_wrap(
        m,
        "move_tag",
        |mut c: C<W>, kp: i32, kl: i32| -> Result<i32, Error> {
            let key = guest_str(&mut c, kp, kl)?;
            Ok(c.data().wr().move_tag(&key))
        },
    )?;
    l.func_wrap(m, "set_settle_tile", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_settle_tile(v)
    })?;
    l.func_wrap(m, "set_pay_factor", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_pay_factor(v)
    })?;
    l.func_wrap(m, "set_rent_factor", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_rent_factor(v)
    })?;
    l.func_wrap(m, "set_can_build", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_can_build(v != 0)
    })?;
    l.func_wrap(m, "set_settle_as_agent", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_settle_as_agent(v != 0)
    })?;
    // v35: 「使你的下次主要移动结果对那些玩家一起执行」 -- the follower list on
    // the move being planned (C# `LeadFx.Who`).
    l.func_wrap(m, "plan_add_follower", |mut c: C<W>, v: i32| {
        c.data_mut().w().plan_add_follower(v)
    })?;
    l.func_wrap(m, "set_more_steps", |mut c: C<W>, v: i32| {
        c.data_mut().w().set_more_steps(v)
    })?;
    l.func_wrap(m, "move_stop_at", |c: C<W>| c.data().wr().move_stop_at())?;
    l.func_wrap(m, "move_stopped", |c: C<W>| {
        c.data().wr().move_stopped() as i32
    })?;
    l.func_wrap(m, "move_parity", |c: C<W>| c.data().wr().move_parity())?;
    l.func_wrap(m, "move_resolve", |c: C<W>| {
        c.data().wr().move_resolve() as i32
    })?;
    l.func_wrap(m, "move_kind", |c: C<W>| c.data().wr().move_kind())?;
    l.func_wrap(m, "move_steps", |c: C<W>| c.data().wr().move_steps())?;
    l.func_wrap(m, "move_remaining", |c: C<W>| {
        c.data().wr().move_remaining()
    })?;
    l.func_wrap(m, "move_total", |c: C<W>| c.data().wr().move_total())?;
    l.func_wrap(m, "move_dir", |c: C<W>| c.data().wr().move_dir())?;

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
