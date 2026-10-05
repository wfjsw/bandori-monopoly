//! wasmi host: loading a set of card modules, the `bandori` import table, replay,
//! and nested cross-module card calls.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use card_sdk::abi::{self, export, AbKind, ManifestOn, OnKind, PromptKind, ABI_VERSION, IMPORT_MODULE};
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

use be::{Caller, Engine, Error, Linker, Module, Store};
use be::{err, error_text, has_func, instantiate, is_need_input, need_input, read_guest, read_mem, set_fuel, write_guest};

use crate::world::CardWorld;

/// Fuel per top-level effect run, shared by any nested `play_card` calls. Plenty
/// for straight-line card logic; stops runaway loops at the same instruction on
/// every machine.
const DEFAULT_FUEL: u64 = 5_000_000;

/// Maximum `play_card` nesting (card A plays B plays C ...).
pub const MAX_NESTING: u32 = 8;

/// One card, as declared in its module's manifest: its id and its entry-point
/// table (`card_sdk::On`), each entry with the trigger kinds it answers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInfo {
    pub id: String,
    pub on: Vec<ManifestOn>,
}

impl CardInfo {
    /// The first entry of `kind` (answering `trigger`, for trigger-keyed kinds).
    pub fn entry(&self, kind: OnKind, trigger: Option<crate::TriggerKind>) -> Option<i32> {
        self.on
            .iter()
            .position(|o| o.kind == kind as i32 && trigger.is_none_or(|t| o.triggers.contains(&(t as i32))))
            .map(|i| i as i32)
    }

    pub fn has_play(&self) -> bool {
        self.entry(OnKind::Play, None).is_some()
    }

    /// Declares a [反击] at this trigger kind.
    pub fn reacts_to(&self, trigger: crate::TriggerKind) -> bool {
        self.entry(OnKind::React, Some(trigger)).is_some()
    }

    /// Declares a field-card hook at this trigger kind.
    pub fn hooks(&self, trigger: crate::TriggerKind) -> bool {
        self.entry(OnKind::Hook, Some(trigger)).is_some()
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
    /// `On::React` -- the reaction effect, for the world's current trigger kind.
    React { card: i32, player_id: i32 },
    /// `On::Hook` -- the field-card hook for `kind`.
    Hook { card: i32, kind: crate::TriggerKind, player_id: i32 },
    /// `On::AtEnd` -- the scheduled turn-end callback.
    AtEnd { card: i32, player_id: i32 },
    /// `On::RollPlan` -- the card's movement routine, called while the walk
    /// is being planned.
    RollPlan { card: i32, player_id: i32 },
}

impl Call {
    /// The player whose card this is -- the cause (`t.ByCard`) of anything the
    /// effect does.
    pub fn player_id(&self) -> i32 {
        match *self {
            Call::Play { player_id, .. } | Call::React { player_id, .. } | Call::Hook { player_id, .. } | Call::AtEnd { player_id, .. }
            | Call::RollPlan { player_id, .. } => player_id,
        }
    }

    pub fn card(&self) -> i32 {
        match *self {
            Call::Play { card, .. } | Call::React { card, .. } | Call::Hook { card, .. } | Call::AtEnd { card, .. }
            | Call::RollPlan { card, .. } => card,
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
    /// answers with the final amount (0 = cancelled).
    Pay { from: i32, to: i32, amount: i32 },
    /// An abnormal effect about to hit `player_id`: the engine runs the C#
    /// `AbnormalGate` and answers 1 (it goes through) or 0 (blocked).
    Gate { player_id: i32, kind: AbKind },
    /// A card targeting a player (`tile < 0`) or a tile: the engine runs the C#
    /// `H.Target` / `H.TargetTile` pipeline and answers with the player (or tile)
    /// actually targeted, or -1 when the targeting failed. `single` lets a
    /// field `redirect` hook move the hit (C# `CardDef.SingleTarget`).
    Target { player_id: i32, tile: i32, single: bool },
    /// C# `H.CardMove(c, m)`: the card shaped the move (via the plan ops) and
    /// asked for it to run now. The engine runs `Cx::card_move` -- it may open
    /// prompts -- and the effect then replays past this call. `plan` is the
    /// plan as the card left it.
    Move { player_id: i32, plan: game_core::engine::MoveCtx },
    /// C# `H.AgentLanding`: the player lands on `agent` as a 「星光代理」 (the
    /// buy-or-pay-rent routine). Runs engine-side; the effect replays past it.
    AgentLanding { player_id: i32, agent: i32 },
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
    /// A guard (`can_react`) tried to prompt. Guards must be pure queries.
    GuardPrompted,
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(m) => write!(f, "card module load error: {m}"),
            Self::DuplicateCard(id) => write!(f, "card {id:?} is declared by more than one module"),
            Self::NoSuchCard(c) => write!(f, "no card with handle {c}"),
            Self::Trap(m) => write!(f, "card effect failed: {m}"),
            Self::GuardPrompted => write!(f, "can_react tried to prompt a player"),
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
    /// Blocked on a payment with no answer yet. Nothing was committed; the
    /// engine raises a `pay` trigger for it and answers with the final amount.
    NeedHost(HostRequest),
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
    engine: Engine,
    modules: Vec<LoadedModule>,
    cards: Vec<CardInfo>,
    slots: Vec<Slot>,
    by_id: HashMap<String, i32>,
    sha256: String,
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
            self.slots.push(Slot { module: m, local: local as i32 });
            self.cards.push(c);
        }
        self.modules.push(LoadedModule { module, sha256: sha256.clone() });
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
        Ok(Ruleset {
            inner: Arc::new(Inner {
                engine: self.engine,
                modules: self.modules,
                cards: self.cards,
                slots: self.slots,
                by_id,
                sha256,
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
        RulesetBuilder { engine: be::new_engine(), modules: vec![], cards: vec![], slots: vec![] }
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

    /// Handle for a card id from `cards.json`, e.g. `"AG:Y.O.L.O"`.
    pub fn card(&self, id: &str) -> Option<i32> {
        self.inner.by_id.get(id).copied()
    }

    /// Run an effect from `world` with the answers collected so far.
    ///
    /// `world` is never modified. On [`Outcome::Done`] the caller commits the returned
    /// world; on [`Outcome::NeedInput`] it publishes the prompt, and once the answer
    /// arrives calls again with the **same** `world` and `answers` + the new answer.
    pub fn run<W: CardWorld>(&self, world: &W, call: Call, answers: &[i32]) -> Result<Outcome<W>, RuleError> {
        let (card, player_id) = (call.card(), call.player_id());
        self.check(card)?;
        let info = &self.inner.cards[card as usize];
        let entry = match call {
            Call::Play { .. } => info.entry(OnKind::Play, None),
            Call::React { .. } => info.entry(OnKind::React, Some(world.trigger().kind)),
            Call::Hook { kind, .. } => info.entry(OnKind::Hook, Some(kind)),
            Call::AtEnd { .. } => info.entry(OnKind::AtEnd, None),
            Call::RollPlan { .. } => info.entry(OnKind::RollPlan, None),
        };
        let Some(entry) = entry else { return Ok(Outcome::Done(world.clone())) };
        let mut store = self.store(world.clone(), answers)?;
        let res = call_card(&self.inner, &mut store, card, entry, export::OP_RUN, player_id);
        let state = store.into_data();
        match res {
            Ok(_) => Ok(Outcome::Done(state.world.expect("world is restored after nested calls"))),
            Err(e) if is_need_input(&e) => {
                if let Some(req) = state.host_request {
                    Ok(Outcome::NeedHost(req))
                } else if let Some(p) = state.asked {
                    Ok(Outcome::NeedInput(p))
                } else {
                    Err(RuleError::Trap("need-input exit without a prompt".into()))
                }
            }
            Err(e) => Err(trap(e)),
        }
    }

    /// `Card.CanReact` against the current trigger. Runs on a throwaway copy, so it
    /// cannot change the world even if the card calls a mutating function.
    pub fn can_react<W: CardWorld>(&self, world: &W, card: i32, player_id: i32) -> Result<bool, RuleError> {
        self.check(card)?;
        // Only a card that declared a [反击] at this kind is ever asked.
        let Some(entry) = self.inner.cards[card as usize].entry(OnKind::React, Some(world.trigger().kind)) else {
            return Ok(false);
        };
        let mut store = self.store(world.clone(), &[])?;
        match call_card(&self.inner, &mut store, card, entry, export::OP_GUARD, player_id) {
            Ok(v) => Ok(v != 0),
            Err(e) if is_need_input(&e) => Err(RuleError::GuardPrompted),
            Err(e) => Err(trap(e)),
        }
    }

    /// `Card.WhyNot` -- the reason the card cannot be played now, or `None` when
    /// it can. Also a pure query on a throwaway copy (a prompting guard reports
    /// [`RuleError::GuardPrompted`]).
    pub fn cant_play<W: CardWorld>(&self, world: &W, card: i32, player_id: i32) -> Result<Option<crate::Msg>, RuleError> {
        self.check(card)?;
        let Some(entry) = self.inner.cards[card as usize].entry(OnKind::CantPlay, None) else { return Ok(None) };
        let mut store = self.store(world.clone(), &[])?;
        match call_card_msg(&self.inner, &mut store, card, entry, player_id) {
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

    fn store<W: CardWorld>(&self, world: W, answers: &[i32]) -> Result<Store<HostState<W>>, RuleError> {
        let state = HostState::new(self.inner.clone(), world, answers.to_vec(), 0);
        let mut store = Store::new(&self.inner.engine, state);
        store.set_fuel(self.fuel).map_err(trap)?;
        Ok(store)
    }
}

fn trap(e: Error) -> RuleError {
    RuleError::Trap(be::error_text(&e))
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
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
        }
    }

    fn w(&mut self) -> &mut W {
        self.world.as_mut().expect("world borrowed by a nested call")
    }

    fn wr(&self) -> &W {
        self.world.as_ref().expect("world borrowed by a nested call")
    }
}

/// Instantiate the card's module and call one of its entry points. Returns the
/// i32 result for `can_react`, 0 otherwise.
fn call_card<W: CardWorld>(
    rules: &Inner,
    store: &mut Store<HostState<W>>,
    card: i32,
    entry: i32,
    op: i32,
    player_id: i32,
) -> Result<i64, Error> {
    let slot = rules.slots[card as usize];
    let linker = linker::<W>(rules)?;
    let inst = instantiate(&linker, &mut *store, &rules.modules[slot.module].module)?;
    inst.get_typed_func::<(i32, i32, i32, i32), i64>(&mut *store, export::ON)?.call(&mut *store, (slot.local, entry, op, player_id))
}

/// Like [`call_card`] for a `CantPlay` entry, which answers with a packed `Msg`
/// buffer: the buffer is read back before the instance drops.
fn call_card_msg<W: CardWorld>(
    rules: &Inner,
    store: &mut Store<HostState<W>>,
    card: i32,
    entry: i32,
    player_id: i32,
) -> Result<Option<crate::Msg>, Error> {
    let slot = rules.slots[card as usize];
    let linker = linker::<W>(rules)?;
    let inst = instantiate(&linker, &mut *store, &rules.modules[slot.module].module)?;
    let packed = inst
        .get_typed_func::<(i32, i32, i32, i32), i64>(&mut *store, export::ON)?
        .call(&mut *store, (slot.local, entry, export::OP_RUN, player_id))?;
    if packed == 0 {
        return Ok(None);
    }
    let (ptr, len) = abi::unpack(packed);
    let bytes = read_mem(&inst, store, ptr as i32, len as i32)?;
    let m: card_sdk::msg::Msg =
        postcard::from_bytes(&bytes).map_err(|e| err(format!("cant_play: guest message is not Msg postcard ({e})")))?;
    Ok(Some(engine_msg(m)))
}

/// Validate one module and read its manifest.
fn inspect(engine: &Engine, wasm: &[u8]) -> Result<(Module, Vec<CardInfo>), RuleError> {
    let load = |m: String| RuleError::Load(m);
    let module = be::compile(engine, wasm).map_err(load)?;

    // Metadata exports never touch the world; a no-op world is enough.
    let mut store = Store::new(engine, HostState::<NullWorld>::without_rules());
    set_fuel(&mut store, DEFAULT_FUEL).map_err(load)?;
    let inst = build_linker::<NullWorld>(engine)
        .map_err(|e| load(error_text(&e)))
        .and_then(|l| instantiate(&l, &mut store, &module).map_err(|e| load(error_text(&e))))?;

    let version = inst
        .get_typed_func::<(), i32>(&mut store, export::ABI_VERSION)
        .and_then(|f| f.call(&mut store, ()))
        .map_err(|e| load(format!("{}: {e}", export::ABI_VERSION)))?;
    if version != ABI_VERSION {
        return Err(load(format!("module built for ABI v{version}, host speaks v{ABI_VERSION}")));
    }

    let packed = inst
        .get_typed_func::<(), i64>(&mut store, export::MANIFEST)
        .and_then(|f| f.call(&mut store, ()))
        .map_err(|e| load(format!("{}: {e}", export::MANIFEST)))?;
    let (ptr, len) = abi::unpack(packed);
    let bytes = read_mem(&inst, &mut store, ptr as i32, len as i32).map_err(|e| load(error_text(&e)))?;
    let entries: Vec<card_sdk::abi::ManifestEntry> =
        postcard::from_bytes(&bytes).map_err(|e| load(format!("manifest: {e}")))?;
    let cards: Vec<CardInfo> = entries.into_iter().map(|e| CardInfo { id: e.id, on: e.on }).collect();
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
fn gate<W: CardWorld>(c: &mut Caller<'_, HostState<W>>, player_id: i32, kind: AbKind) -> Result<bool, Error> {
    let st = c.data_mut();
    if let Some(&allowed) = st.answers.get(st.next_answer) {
        st.next_answer += 1;
        return Ok(allowed != 0);
    }
    st.host_request = Some(HostRequest::Gate { player_id, kind });
    Err(need_input())
}

fn guest_str<W>(caller: &mut Caller<'_, HostState<W>>, ptr: i32, len: i32) -> Result<String, Error> {
    let bytes = read_guest(caller, ptr, len)?;
    String::from_utf8(bytes).map_err(|_| err("guest string is not UTF-8"))
}

/// A message from the guest (ABI v5: every text parameter is a `postcard`-encoded
/// `Msg`). A card that sends anything else traps instead of showing raw text to
/// players. Decoding uses the same `serde` type the guest encodes with, so the
/// two sides cannot disagree on the layout.
fn guest_msg<W: CardWorld>(caller: &mut Caller<'_, HostState<W>>, ptr: i32, len: i32) -> Result<crate::Msg, Error> {
    let bytes = read_guest(caller, ptr, len)?;
    let m: card_sdk::msg::Msg =
        postcard::from_bytes(&bytes).map_err(|e| err(format!("guest message is not Msg postcard ({e})")))?;
    Ok(engine_msg(m))
}

/// Guest `Msg` -> engine `Msg`. An exhaustive match: a new argument kind on either
/// side has to be mapped here on purpose.
fn engine_msg(m: card_sdk::msg::Msg) -> crate::Msg {
    use card_sdk::msg::Arg as G;
    use game_core::msg::Arg as E;
    let mut out = crate::Msg::new(&m.k);
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
        out.a.insert(name, arg);
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

fn linker<W: CardWorld + 'static>(rules: &Inner) -> Result<Linker<HostState<W>>, Error> {
    let key = (&rules.engine as *const Engine as usize, std::any::TypeId::of::<W>());
    let cached = LINKERS.with(|c| {
        c.borrow()
            .get(&key)
            .map(|hit| hit.downcast_ref::<Linker<HostState<W>>>().expect("cache is keyed by TypeId").clone())
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

    l.func_wrap(m, "roll", |mut c: C<W>, player_id: i32, count: i32, sides: i32| -> Result<i32, Error> {
        if !(1..=100).contains(&count) || !(1..=1000).contains(&sides) {
            return Err(err(format!("roll({count}d{sides}) out of range")));
        }
        Ok(c.data_mut().w().roll(player_id, count, sides))
    })?;
    l.func_wrap(m, "log", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
        let msg = guest_msg(&mut c, p, n)?;
        c.data_mut().w().log(player_id, msg);
        Ok(())
    })?;
    l.func_wrap(m, "tile_count", |c: C<W>| c.data().wr().tile_count())?;
    l.func_wrap(m, "add_mark", |mut c: C<W>, tile: i32, player_id: i32, kp: i32, kl: i32, p: i32, n: i32| -> Result<(), Error> {
        let kind = guest_str(&mut c, kp, kl)?;
        let note = guest_msg(&mut c, p, n)?;
        c.data_mut().w().add_mark(tile, player_id, &kind, note);
        Ok(())
    })?;
    l.func_wrap(m, "money", |c: C<W>, player_id: i32| c.data().wr().money(player_id))?;
    l.func_wrap(m, "gain", |mut c: C<W>, player_id: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
        let src = guest_msg(&mut c, p, n)?;
        Ok(c.data_mut().w().gain(player_id, amount, src))
    })?;
    l.func_wrap(m, "pay", |mut c: C<W>, player_id: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
        let src = guest_msg(&mut c, p, n)?;
        let st = c.data_mut();
        // A card-driven payment is paused so the engine can raise a `pay`
        // trigger (and the [反击] window) before the money moves. The answer is
        // the final amount to move -- 0 cancels the payment outright.
        if let Some(&final_amount) = st.answers.get(st.next_answer) {
            st.next_answer += 1;
            return Ok(st.w().pay(player_id, final_amount, src));
        }
        st.host_request = Some(HostRequest::Pay { from: player_id, to: -1, amount });
        Err(need_input())
    })?;
    l.func_wrap(m, "player_count", |c: C<W>| c.data().wr().player_count())?;
    l.func_wrap(m, "player_out", |c: C<W>, player_id: i32| c.data().wr().player_out(player_id))?;
    l.func_wrap(m, "others_count", |c: C<W>, player_id: i32| c.data().wr().others_count(player_id))?;
    l.func_wrap(m, "others_at", |c: C<W>, player_id: i32, index: i32| c.data().wr().others_at(player_id, index))?;
    l.func_wrap(m, "tile_named", |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().tile_named(&name))
    })?;
    l.func_wrap(m, "tile_owner", |c: C<W>, tile: i32| c.data().wr().tile_owner(tile))?;
    l.func_wrap(m, "player_pos", |c: C<W>, player_id: i32| c.data().wr().player_pos(player_id))?;
    l.func_wrap(m, "tile_steps_ahead", |c: C<W>, player_id: i32, steps: i32| c.data().wr().tile_steps_ahead(player_id, steps))?;
    l.func_wrap(m, "rent_of", |c: C<W>, tile: i32| c.data().wr().rent_of(tile))?;
    l.func_wrap(m, "buy_price", |c: C<W>, tile: i32| c.data().wr().buy_price(tile))?;
    l.func_wrap(m, "build_cost", |c: C<W>, tile: i32| c.data().wr().build_cost(tile))?;
    l.func_wrap(m, "mortgage_value", |c: C<W>, tile: i32| c.data().wr().mortgage_value(tile))?;
    l.func_wrap(m, "owned_count", |c: C<W>, player_id: i32| c.data().wr().owned_count(player_id))?;
    l.func_wrap(m, "owned_at", |c: C<W>, player_id: i32, index: i32| c.data().wr().owned_at(player_id, index))?;
    l.func_wrap(m, "is_buyable", |c: C<W>, tile: i32| c.data().wr().is_buyable(tile))?;
    l.func_wrap(m, "is_shop", |c: C<W>, tile: i32| c.data().wr().is_shop(tile))?;
    l.func_wrap(m, "is_ring", |c: C<W>, tile: i32| c.data().wr().is_ring(tile))?;
    l.func_wrap(m, "is_circle", |c: C<W>, tile: i32| c.data().wr().is_circle(tile))?;
    l.func_wrap(m, "is_live_house", |c: C<W>, tile: i32| c.data().wr().is_live_house(tile))?;
    l.func_wrap(m, "tile_group", |c: C<W>, tile: i32| c.data().wr().tile_group(tile))?;
    l.func_wrap(m, "tile_price", |c: C<W>, tile: i32| c.data().wr().tile_price(tile))?;
    l.func_wrap(m, "houses_of", |c: C<W>, tile: i32| c.data().wr().houses_of(tile))?;
    l.func_wrap(m, "set_houses", |mut c: C<W>, tile: i32, n: i32| {
        c.data_mut().w().set_houses(tile, n);
        Ok(())
    })?;
    l.func_wrap(m, "add_house", |mut c: C<W>, tile: i32, n: i32| c.data_mut().w().add_house(tile, n))?;
    l.func_wrap(m, "mortgaged_of", |c: C<W>, tile: i32| c.data().wr().mortgaged_of(tile))?;
    l.func_wrap(m, "set_mortgaged", |mut c: C<W>, tile: i32, v: i32| {
        c.data_mut().w().set_mortgaged(tile, v);
        Ok(())
    })?;
    l.func_wrap(m, "set_owner", |mut c: C<W>, tile: i32, player_id: i32| {
        c.data_mut().w().set_owner(tile, player_id);
        Ok(())
    })?;
    l.func_wrap(m, "dist", |c: C<W>, a: i32, b: i32| c.data().wr().dist(a, b))?;
    l.func_wrap(m, "tile_forward", |c: C<W>, a: i32, b: i32| c.data().wr().tile_forward(a, b))?;
    l.func_wrap(m, "neighbor", |c: C<W>, player_id: i32, dir: i32| c.data().wr().neighbor(player_id, dir))?;
    l.func_wrap(m, "players_on_count", |c: C<W>, tile: i32, except: i32| c.data().wr().players_on_count(tile, except))?;
    l.func_wrap(m, "players_on_at", |c: C<W>, tile: i32, except: i32, index: i32| c.data().wr().players_on_at(tile, except, index))?;
    l.func_wrap(m, "draw", |mut c: C<W>, player_id: i32, n: i32| -> Result<i32, Error> {
        Ok(c.data_mut().w().draw(player_id, n))
    })?;
    l.func_wrap(m, "add_to_hand", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, p, n)?;
        c.data_mut().w().add_to_hand(player_id, &card);
        Ok(())
    })?;
    l.func_wrap(m, "add_to_deck", |mut c: C<W>, player_id: i32, p: i32, n: i32, shuffle: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, p, n)?;
        c.data_mut().w().add_to_deck(player_id, &card, shuffle != 0);
        Ok(())
    })?;
    l.func_wrap(m, "add_to_deck_at", |mut c: C<W>, player_id: i32, p: i32, n: i32, pos: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, p, n)?;
        c.data_mut().w().add_to_deck_at(player_id, &card, pos);
        Ok(())
    })?;
    l.func_wrap(m, "take_card", |mut c: C<W>, player_id: i32, pile: i32, p: i32, n: i32| -> Result<i32, Error> {
        let pile = crate::CardPile::from_i32(pile).ok_or_else(|| err(format!("bad card pile {pile}")))?;
        let id = guest_str(&mut c, p, n)?;
        Ok(c.data_mut().w().take_card(player_id, pile, &id) as i32)
    })?;
    // Host->guest list: the guest passes its own buffer; the host writes the
    // postcard bytes only if they fit and always returns the length needed.
    l.func_wrap(m, "cards_in", |mut c: C<W>, player_id: i32, pile: i32, buf: i32, cap: i32| -> Result<i32, Error> {
        let pile = crate::CardPile::from_i32(pile).ok_or_else(|| err(format!("bad card pile {pile}")))?;
        let list = c.data().wr().cards_in(player_id, pile);
        let bytes = postcard::to_allocvec(&list).map_err(|e| err(format!("cards_in encode: {e}")))?;
        if bytes.len() as i32 <= cap {
            write_guest(&mut c, buf, &bytes)?;
        }
        Ok(bytes.len() as i32)
    })?;
    l.func_wrap(m, "to_discard", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, p, n)?;
        c.data_mut().w().to_discard(player_id, &card);
        Ok(())
    })?;
    l.func_wrap(m, "hand_count", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
        let card = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().hand_count(player_id, &card))
    })?;
    l.func_wrap(m, "discard_count", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
        let card = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().discard_count(player_id, &card))
    })?;
    l.func_wrap(m, "hand_size", |c: C<W>, player_id: i32| c.data().wr().hand_size(player_id))?;
    l.func_wrap(m, "deck_count", |c: C<W>, player_id: i32| c.data().wr().deck_count(player_id))?;
    l.func_wrap(m, "discard_size", |c: C<W>, player_id: i32| c.data().wr().discard_size(player_id))?;
    l.func_wrap(m, "discard_from_hand", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
        let card = guest_str(&mut c, p, n)?;
        Ok(c.data_mut().w().discard_from_hand(player_id, &card))
    })?;
    l.func_wrap(m, "shuffle_into_deck", |mut c: C<W>, player_id: i32, hand: i32, discard: i32| {
        c.data_mut().w().shuffle_into_deck(player_id, hand != 0, discard != 0)
    })?;
    l.func_wrap(m, "unplace_card", |mut c: C<W>, player_id: i32| c.data_mut().w().unplace_card(player_id) as i32)?;
    l.func_wrap(m, "is_placed", |c: C<W>, player_id: i32| c.data().wr().is_placed(player_id))?;
    l.func_wrap(m, "crystals", |c: C<W>, player_id: i32| c.data().wr().crystals(player_id))?;
    l.func_wrap(m, "set_crystals", |mut c: C<W>, player_id: i32, n: i32| c.data_mut().w().set_crystals(player_id, n))?;
    l.func_wrap(m, "add_crystals", |mut c: C<W>, player_id: i32, n: i32, max: i32| c.data_mut().w().add_crystals(player_id, n, max))?;
    l.func_wrap(m, "count_marks", |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32| -> Result<i32, Error> {
        let kind = guest_str(&mut c, kp, kl)?;
        Ok(c.data().wr().count_marks(tile, &kind, owner))
    })?;
    l.func_wrap(m, "remove_marks", |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32| -> Result<i32, Error> {
        let kind = guest_str(&mut c, kp, kl)?;
        Ok(c.data_mut().w().remove_marks(tile, &kind, owner))
    })?;
    l.func_wrap(m, "tok", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().tok(player_id, &name))
    })?;
    l.func_wrap(m, "set_tok", |mut c: C<W>, player_id: i32, p: i32, n: i32, v: i32| -> Result<(), Error> {
        let name = guest_str(&mut c, p, n)?;
        c.data_mut().w().set_tok(player_id, &name, v);
        Ok(())
    })?;
    l.func_wrap(m, "add_tok", |mut c: C<W>, player_id: i32, p: i32, n: i32, by: i32, max: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data_mut().w().add_tok(player_id, &name, by, max))
    })?;
    // Keyed state: `{value, min, max, expires}` items. `field` selects the
    // column (0 = value, 1 = min, 2 = max, 3 = expires: 0 none / 1 turn-start /
    // 2 turn-end). The engine holds these and enforces nothing.
    l.func_wrap(m, "state_get", |mut c: C<W>, player_id: i32, p: i32, n: i32, field: i32| -> Result<i32, Error> {
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
    })?;
    l.func_wrap(m, "state_set", |mut c: C<W>, player_id: i32, p: i32, n: i32, field: i32, v: i32| -> Result<i32, Error> {
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
                let e = match v { 1 => Some(game_core::state::Tick::TurnStart), 2 => Some(game_core::state::Tick::TurnEnd), _ => None };
                c.data_mut().w().state_set_expires(player_id, &key, e);
                v
            }
            _ => c.data_mut().w().state_set(player_id, &key, v),
        })
    })?;
    l.func_wrap(m, "state_add", |mut c: C<W>, player_id: i32, p: i32, n: i32, delta: i32| -> Result<i32, Error> {
        let key = guest_str(&mut c, p, n)?;
        Ok(c.data_mut().w().state_add(player_id, &key, delta))
    })?;

    l.func_wrap(m, "slot", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
        let key = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().slot(player_id, &key))
    })?;
    l.func_wrap(m, "set_slot", |mut c: C<W>, player_id: i32, p: i32, n: i32, v: i32| -> Result<(), Error> {
        let key = guest_str(&mut c, p, n)?;
        c.data_mut().w().set_slot(player_id, &key, v);
        Ok(())
    })?;
    l.func_wrap(m, "inc_slot", |mut c: C<W>, player_id: i32, p: i32, n: i32, by: i32| -> Result<i32, Error> {
        let key = guest_str(&mut c, p, n)?;
        Ok(c.data_mut().w().inc_slot(player_id, &key, by))
    })?;
    l.func_wrap(m, "band_crystals", |c: C<W>, player_id: i32| c.data().wr().band_crystals(player_id))?;
    l.func_wrap(m, "add_band_crystals", |mut c: C<W>, player_id: i32, n: i32, max: i32| c.data_mut().w().add_band_crystals(player_id, n, max))?;
    l.func_wrap(m, "fire", |c: C<W>, player_id: i32| c.data().wr().fire(player_id))?;
    l.func_wrap(m, "fire_max", |c: C<W>, player_id: i32| c.data().wr().fire_max(player_id))?;
    l.func_wrap(m, "gain_fire", |mut c: C<W>, player_id: i32, n: i32, p: i32, l: i32| -> Result<i32, Error> {
        let why = guest_msg(&mut c, p, l)?;
        Ok(c.data_mut().w().gain_fire(player_id, n, why))
    })?;
    // Abnormal effects pass the C# `AbnormalGate` first (C# `GiveStay` /
    // `GiveStun` / `GiveExile` / `ForceTeleport` all run it): a blocked one
    // does nothing.
    l.func_wrap(m, "give_stay", |mut c: C<W>, player_id: i32, n: i32| -> Result<(), Error> {
        if n > 0 && gate(&mut c, player_id, AbKind::Stay)? {
            c.data_mut().w().give_stay(player_id, n);
        }
        Ok(())
    })?;
    l.func_wrap(m, "give_stun", |mut c: C<W>, player_id: i32, n: i32| -> Result<(), Error> {
        if n > 0 && gate(&mut c, player_id, AbKind::Stun)? {
            c.data_mut().w().give_stun(player_id, n);
        }
        Ok(())
    })?;
    l.func_wrap(m, "give_exile", |mut c: C<W>, player_id: i32, n: i32, to: i32| -> Result<(), Error> {
        if n > 0 && gate(&mut c, player_id, AbKind::Exile)? {
            c.data_mut().w().give_exile(player_id, n, to);
        }
        Ok(())
    })?;
    l.func_wrap(m, "give_extra_turn", |mut c: C<W>, player_id: i32| {
        c.data_mut().w().give_extra_turn(player_id);
        Ok(())
    })?;
    l.func_wrap(m, "can_pay", |c: C<W>, player_id: i32| c.data().wr().can_pay(player_id))?;
    l.func_wrap(m, "cant_move", |c: C<W>, player_id: i32| c.data().wr().cant_move(player_id))?;
    l.func_wrap(m, "spend_fire", |mut c: C<W>, player_id: i32, n: i32, p: i32, l: i32| -> Result<i32, Error> {
        let why = guest_msg(&mut c, p, l)?;
        Ok(c.data_mut().w().spend_fire(player_id, n, why))
    })?;
    l.func_wrap(m, "stay_of", |c: C<W>, player_id: i32| c.data().wr().stay_of(player_id))?;
    l.func_wrap(m, "stun_of", |c: C<W>, player_id: i32| c.data().wr().stun_of(player_id))?;
    l.func_wrap(m, "turn_player", |c: C<W>| c.data().wr().turn_player())?;
    l.func_wrap(m, "round_no", |c: C<W>| c.data().wr().round_no())?;
    l.func_wrap(m, "turn_key", |c: C<W>| c.data().wr().turn_key())?;
    l.func_wrap(m, "character_is", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().character_is(player_id, &name))
    })?;
    l.func_wrap(m, "in_band", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().in_band(player_id, &name))
    })?;
    l.func_wrap(m, "place_card_at", |mut c: C<W>, player_id: i32, cp: i32, cl: i32, p: i32, n: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, cp, cl)?;
        let note = guest_msg(&mut c, p, n)?;
        c.data_mut().w().place_card(player_id, &card, note);
        Ok(())
    })?;
    l.func_wrap(m, "set_dest", |mut c: C<W>, dest: i32| {
        c.data_mut().w().set_dest(dest);
        Ok(())
    })?;
    l.func_wrap(m, "ring_multiplier", |c: C<W>| c.data().wr().ring_multiplier())?;
    l.func_wrap(m, "add_ring_bonus", |mut c: C<W>, n: i32| c.data_mut().w().add_ring_bonus(n))?;
    l.func_wrap(m, "teleport_to", |mut c: C<W>, player_id: i32, tile: i32| -> Result<(), Error> {
        if gate(&mut c, player_id, AbKind::Teleport)? {
            c.data_mut().w().teleport_to(player_id, tile);
        }
        Ok(())
    })?;
    l.func_wrap(m, "abnormal_count", |c: C<W>, player_id: i32| c.data().wr().abnormal_count(player_id))?;
    l.func_wrap(m, "targeted_count", |c: C<W>, player_id: i32| c.data().wr().targeted_count(player_id))?;
    l.func_wrap(m, "target", |mut c: C<W>, player_id: i32, tile: i32, single: i32| -> Result<i32, Error> {
        // Paused like `gate`: the engine runs the targeting pipeline (it
        // raises hooks and opens the `target` [反击] window) and the replay
        // reads its answer.
        let st = c.data_mut();
        if let Some(&got) = st.answers.get(st.next_answer) {
            st.next_answer += 1;
            return Ok(got);
        }
        st.host_request = Some(HostRequest::Target { player_id, tile, single: single != 0 });
        Err(need_input())
    })?;
    l.func_wrap(m, "card_move", |mut c: C<W>, player_id: i32| -> Result<i32, Error> {
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
    })?;
    l.func_wrap(m, "agent_landing", |mut c: C<W>, player_id: i32, agent: i32| -> Result<i32, Error> {
        let st = c.data_mut();
        if let Some(&ok) = st.answers.get(st.next_answer) {
            st.next_answer += 1;
            return Ok(ok);
        }
        st.host_request = Some(HostRequest::AgentLanding { player_id, agent });
        Err(need_input())
    })?;
    l.func_wrap(m, "placed_tile", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
        let id = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().placed_tile(player_id, &id))
    })?;
    l.func_wrap(m, "play_doubled", |c: C<W>| c.data().wr().play_doubled())?;
    l.func_wrap(m, "trig_cards", |mut c: C<W>, buf: i32, cap: i32| -> Result<i32, Error> {
        let list = c.data().wr().trigger().cards;
        let bytes = postcard::to_allocvec(&list).map_err(|e| err(format!("trig_cards encode: {e}")))?;
        if bytes.len() as i32 <= cap {
            write_guest(&mut c, buf, &bytes)?;
        }
        Ok(bytes.len() as i32)
    })?;
    l.func_wrap(m, "opt_int", |mut c: C<W>, v: i32| c.data_mut().options.push(PromptOption::Int(v)))?;
    l.func_wrap(m, "opt_str", |mut c: C<W>, p: i32, n: i32| -> Result<(), Error> {
        let s = guest_msg(&mut c, p, n)?;
        c.data_mut().options.push(PromptOption::Str(s));
        Ok(())
    })?;
    l.func_wrap(
        m,
        "ask",
        |mut c: C<W>, kind: i32, player_id: i32, tp: i32, tl: i32, xp: i32, xl: i32| -> Result<i32, Error> {
            let kind = PromptKind::from_i32(kind).ok_or_else(|| err(format!("bad prompt kind {kind}")))?;
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
            st.asked = Some(Prompt { kind, player_id, title, text, options, answer_slot: st.next_answer });
            Err(need_input())
        },
    )?;
    l.func_wrap(m, "trig_kind", |c: C<W>| c.data().wr().trigger().kind as i32)?;
    l.func_wrap(m, "trig_player", |c: C<W>| c.data().wr().trigger().player_id)?;
    l.func_wrap(m, "trig_target", |c: C<W>| c.data().wr().trigger().target)?;
    l.func_wrap(m, "trig_tile", |c: C<W>| c.data().wr().trigger().tile)?;
    l.func_wrap(m, "trig_value", |c: C<W>| c.data().wr().trigger().value)?;
    l.func_wrap(m, "trig_step", |c: C<W>| c.data().wr().trigger().step)?;
    l.func_wrap(m, "trig_by_card", |c: C<W>| c.data().wr().trigger().by_card.unwrap_or(-1))?;
    l.func_wrap(m, "trig_pay_is_rent", |c: C<W>| c.data().wr().trigger().pay_is_rent as i32)?;
    l.func_wrap(m, "trig_move_kind", |c: C<W>| c.data().wr().trigger().move_kind.map_or(-1, |k| k as i32))?;
    l.func_wrap(m, "trig_move_resolve", |c: C<W>| c.data().wr().trigger().move_resolve as i32)?;
    l.func_wrap(m, "trig_move_tag", |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
        let key = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().trigger().move_tags.iter().find(|(k, _)| *k == key).map_or(0, |(_, v)| *v))
    })?;
    l.func_wrap(m, "trig_move_main", |c: C<W>| c.data().wr().trigger().move_main as i32)?;
    l.func_wrap(m, "trig_move_dir", |c: C<W>| c.data().wr().trigger().move_dir)?;
    l.func_wrap(m, "trig_move_remaining", |c: C<W>| c.data().wr().trigger().move_remaining)?;
    l.func_wrap(m, "trig_move_total", |c: C<W>| c.data().wr().trigger().move_total)?;
    l.func_wrap(m, "trig_move_roll", |c: C<W>| c.data().wr().trigger().move_roll.unwrap_or(-1))?;
    l.func_wrap(m, "trig_set_move_roll", |mut c: C<W>, v: i32| c.data_mut().w().set_trigger_move_roll(v))?;
    l.func_wrap(m, "trig_set_pay_amount", |mut c: C<W>, v: i32| c.data_mut().w().set_trigger_value(v))?;
    l.func_wrap(m, "trig_set_pay_target", |mut c: C<W>, to: i32| c.data_mut().w().set_trigger_target(to))?;
    l.func_wrap(m, "trig_set_cancelled", |mut c: C<W>| c.data_mut().w().set_trigger_cancelled())?;
    l.func_wrap(m, "trig_cancelled", |c: C<W>| c.data().wr().trigger().cancelled as i32)?;
    l.func_wrap(m, "trig_card_is", |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
        let id = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().trig_card_is(&id))
    })?;

    // Cross-module call: run another card's `play` inside this run. The world, the
    // answer log position and the remaining fuel move into a nested store and back,
    // so a prompt raised by the inner card aborts (and later replays) the outer run.
    l.func_wrap(m, "play_card", |mut c: C<W>, p: i32, n: i32, player_id: i32| -> Result<i32, Error> {
        let id = guest_str(&mut c, p, n)?;
        let rules = c.data().rules.clone().ok_or_else(|| err("play_card unavailable here"))?;
        let card = *rules.by_id.get(&id).ok_or_else(|| err(format!("play_card: unknown card {id:?}")))?;
        if c.data().depth >= MAX_NESTING {
            return Err(err(format!("play_card nested deeper than {MAX_NESTING}")));
        }
        let fuel = c.get_fuel()?;
        let st = c.data_mut();
        // The inner card runs as itself: its own id for place/crystals and its
        // own `Dest`, which must not overwrite the outer card's.
        let saved = st.w().enter_card(&id);
        let mut nested = HostState::new(rules.clone(), st.world.take().expect("world present"), std::mem::take(&mut st.answers), st.depth + 1);
        nested.next_answer = st.next_answer;
        let mut store = Store::new(&rules.engine, nested);
        store.set_fuel(fuel)?;
        let res = match rules.cards[card as usize].entry(OnKind::Play, None) {
            Some(entry) => call_card(&rules, &mut store, card, entry, export::OP_RUN, player_id).map(|_| ()),
            None => Ok(()),
        };
        let left = store.get_fuel().unwrap_or(0);
        let inner = store.into_data();
        let st = c.data_mut();
        st.world = inner.world;
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
        res.map(|()| dest)
    })?;

    // `H.CanReplay` -- could `player_id` play card `id` now? A pure query: the card's
    // `cant_play` runs on a throwaway copy of the world. A card with no `play`
    // effect is never replayable; a guard that prompts or traps counts as "no".
    l.func_wrap(m, "card_replayable", |mut c: C<W>, player_id: i32, p: i32, n: i32| -> Result<i32, Error> {
        let id = guest_str(&mut c, p, n)?;
        let rules = c.data().rules.clone().ok_or_else(|| err("card_replayable unavailable here"))?;
        let Some(&card) = rules.by_id.get(&id) else { return Ok(0) };
        if !rules.cards[card as usize].has_play() || c.data().depth >= MAX_NESTING {
            return Ok(0);
        }
        let Some(entry) = rules.cards[card as usize].entry(OnKind::CantPlay, None) else { return Ok(1) };
        let fuel = c.get_fuel()?;
        let mut world = c.data().wr().clone();
        world.enter_card(&id);
        let depth = c.data().depth + 1;
        let mut store = Store::new(&rules.engine, HostState::new(rules.clone(), world, vec![], depth));
        store.set_fuel(fuel)?;
        let res = call_card_msg(&rules, &mut store, card, entry, player_id);
        let left = store.get_fuel().unwrap_or(0);
        c.set_fuel(left)?;
        Ok(matches!(res, Ok(None)) as i32)
    })?;
    // `mode`: bit 1 = the end of `player_id`'s next turn, bit 2 = before the wear-off.
    l.func_wrap(m, "schedule_turn_end", |mut c: C<W>, player_id: i32, mode: i32| {
        c.data_mut().w().schedule_turn_end(player_id, mode & 1 != 0, mode & 2 != 0)
    })?;
    l.func_wrap(m, "set_no_money_loss", |mut c: C<W>, player_id: i32| c.data_mut().w().set_no_money_loss(player_id))?;
    l.func_wrap(m, "set_fixed_roll", |mut c: C<W>, n: i32| c.data_mut().w().set_fixed_roll(n))?;
    l.func_wrap(m, "fixed_roll", |c: C<W>| c.data().wr().fixed_roll())?;
    l.func_wrap(m, "set_next_steps", |mut c: C<W>, player_id: i32, n: i32| c.data_mut().w().set_next_steps(player_id, n))?;
    l.func_wrap(m, "turn_main_steps", |c: C<W>| c.data().wr().turn_main_steps())?;
    l.func_wrap(m, "add_fire_max", |mut c: C<W>, player_id: i32, n: i32| c.data_mut().w().add_fire_max(player_id, n))?;
    // movement shaping: the move being planned (TurnCtx.plan)
    l.func_wrap(m, "set_steps", |mut c: C<W>, v: i32| c.data_mut().w().set_steps(v))?;
    l.func_wrap(m, "set_reverse", |mut c: C<W>, v: i32| c.data_mut().w().set_reverse(v != 0))?;
    l.func_wrap(m, "set_signed", |mut c: C<W>, v: i32| c.data_mut().w().set_signed(v != 0))?;
    l.func_wrap(m, "set_stop_at", |mut c: C<W>, v: i32| c.data_mut().w().set_stop_at(v))?;
    l.func_wrap(m, "set_parity", |mut c: C<W>, v: i32| c.data_mut().w().set_parity(v))?;
    l.func_wrap(m, "set_resolve", |mut c: C<W>, v: i32| c.data_mut().w().set_resolve(v != 0))?;
    l.func_wrap(m, "set_no_buy", |mut c: C<W>, v: i32| c.data_mut().w().set_no_buy(v != 0))?;
    l.func_wrap(m, "set_no_build", |mut c: C<W>, v: i32| c.data_mut().w().set_no_build(v != 0))?;
    l.func_wrap(m, "set_kind", |mut c: C<W>, v: i32| c.data_mut().w().set_kind(v))?;
    l.func_wrap(m, "set_tag", |mut c: C<W>, kp: i32, kl: i32, v: i32| -> Result<(), Error> {
        let key = guest_str(&mut c, kp, kl)?;
        c.data_mut().w().set_tag(&key, v);
        Ok(())
    })?;
    l.func_wrap(m, "set_min_roll", |mut c: C<W>, v: i32| c.data_mut().w().set_min_roll(v))?;
    l.func_wrap(m, "set_extra_steps", |mut c: C<W>, v: i32| c.data_mut().w().set_extra_steps(v))?;
    l.func_wrap(m, "move_tag", |mut c: C<W>, kp: i32, kl: i32| -> Result<i32, Error> {
        let key = guest_str(&mut c, kp, kl)?;
        Ok(c.data().wr().move_tag(&key))
    })?;
    l.func_wrap(m, "set_settle_tile", |mut c: C<W>, v: i32| c.data_mut().w().set_settle_tile(v))?;
    l.func_wrap(m, "set_pay_factor", |mut c: C<W>, v: i32| c.data_mut().w().set_pay_factor(v))?;
    l.func_wrap(m, "set_rent_factor", |mut c: C<W>, v: i32| c.data_mut().w().set_rent_factor(v))?;
    l.func_wrap(m, "set_build_anywhere", |mut c: C<W>, v: i32| c.data_mut().w().set_build_anywhere(v != 0))?;
    l.func_wrap(m, "set_settle_as_agent", |mut c: C<W>, v: i32| c.data_mut().w().set_settle_as_agent(v != 0))?;
    l.func_wrap(m, "set_more_steps", |mut c: C<W>, v: i32| c.data_mut().w().set_more_steps(v))?;
    l.func_wrap(m, "set_no_circle_reward", |mut c: C<W>, v: i32| c.data_mut().w().set_no_circle_reward(v != 0))?;
    l.func_wrap(m, "move_stop_at", |c: C<W>| c.data().wr().move_stop_at())?;
    l.func_wrap(m, "move_parity", |c: C<W>| c.data().wr().move_parity())?;
    l.func_wrap(m, "move_resolve", |c: C<W>| c.data().wr().move_resolve() as i32)?;
    l.func_wrap(m, "move_kind", |c: C<W>| c.data().wr().move_kind())?;
    l.func_wrap(m, "move_steps", |c: C<W>| c.data().wr().move_steps())?;
    l.func_wrap(m, "move_remaining", |c: C<W>| c.data().wr().move_remaining())?;
    l.func_wrap(m, "move_total", |c: C<W>| c.data().wr().move_total())?;
    l.func_wrap(m, "move_dir", |c: C<W>| c.data().wr().move_dir())?;

    Ok(l)
}

impl HostState<NullWorld> {
    fn without_rules() -> Self {
        Self { rules: None, world: Some(NullWorld), answers: vec![], next_answer: 0, options: vec![], asked: None, host_request: None, depth: 0 }
    }
}

/// World used only while reading module metadata.
#[derive(Clone)]
struct NullWorld;

impl CardWorld for NullWorld {
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
    fn state_set_expires(&mut self, _player_id: i32, _key: &str, _expires: Option<game_core::state::Tick>) {}
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
    fn place_card(&mut self, _: i32, _: &str, _: crate::Msg) {}
    fn unplace_card(&mut self, _: i32) -> bool {
        false
    }
    fn is_placed(&self, _: i32) -> i32 {
        0
    }
    fn crystals(&self, _: i32) -> i32 {
        0
    }
    fn set_crystals(&mut self, _: i32, _: i32) -> i32 {
        0
    }
    fn add_crystals(&mut self, _: i32, _: i32, _: i32) -> i32 {
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
    fn trig_card_is(&self, _: &str) -> i32 {
        0
    }
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
