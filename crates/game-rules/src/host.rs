//! wasmi host: loading a set of card modules, the `bandori` import table, replay,
//! and nested cross-module card calls.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use card_sdk::abi::{self, export, PromptKind, ABI_VERSION, IMPORT_MODULE};
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
use be::{err, error_text, has_func, instantiate, is_need_input, need_input, read_guest, read_mem, set_fuel};

use crate::world::CardWorld;

/// Fuel per top-level effect run, shared by any nested `play_card` calls. Plenty
/// for straight-line card logic; stops runaway loops at the same instruction on
/// every machine.
const DEFAULT_FUEL: u64 = 5_000_000;

/// Maximum `play_card` nesting (card A plays B plays C ...).
pub const MAX_NESTING: u32 = 8;

/// One card, as declared in its module's manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardInfo {
    pub id: String,
    pub play: bool,
    pub react: bool,
}

/// What to run.
#[derive(Debug, Clone, Copy)]
pub enum Call {
    /// `Card.Play` -- effect when played from hand.
    Play { card: i32, seat: i32 },
    /// `Card.React` -- reaction effect at the current trigger.
    React { card: i32, seat: i32 },
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
    pub seat: i32,
    pub title: crate::Msg,
    pub text: crate::Msg,
    pub options: Vec<PromptOption>,
    /// Position in the effect's answer log this prompt will fill.
    pub answer_slot: usize,
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
        let (entry, card, seat) = match call {
            Call::Play { card, seat } => (export::PLAY, card, seat),
            Call::React { card, seat } => (export::REACT, card, seat),
        };
        self.check(card)?;
        let mut store = self.store(world.clone(), answers)?;
        let res = call_card(&self.inner, &mut store, entry, card, seat);
        let state = store.into_data();
        match res {
            Ok(_) => Ok(Outcome::Done(state.world.expect("world is restored after nested calls"))),
            Err(e) if is_need_input(&e) => match state.asked {
                Some(p) => Ok(Outcome::NeedInput(p)),
                None => Err(RuleError::Trap("need-input exit without a prompt".into())),
            },
            Err(e) => Err(trap(e)),
        }
    }

    /// `Card.CanReact` against the current trigger. Runs on a throwaway copy, so it
    /// cannot change the world even if the card calls a mutating function.
    pub fn can_react<W: CardWorld>(&self, world: &W, card: i32, seat: i32) -> Result<bool, RuleError> {
        self.check(card)?;
        let mut store = self.store(world.clone(), &[])?;
        match call_card(&self.inner, &mut store, export::CAN_REACT, card, seat) {
            Ok(v) => Ok(v != 0),
            Err(e) if is_need_input(&e) => Err(RuleError::GuardPrompted),
            Err(e) => Err(trap(e)),
        }
    }

    /// `Card.WhyNot` -- the reason the card cannot be played now, or `None` when
    /// it can. Also a pure query on a throwaway copy (a prompting guard reports
    /// [`RuleError::GuardPrompted`]).
    pub fn why_not<W: CardWorld>(&self, world: &W, card: i32, seat: i32) -> Result<Option<crate::Msg>, RuleError> {
        self.check(card)?;
        let mut store = self.store(world.clone(), &[])?;
        match call_card_msg(&self.inner, &mut store, export::WHY_NOT, card, seat) {
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
    depth: u32,
}

impl<W> HostState<W> {
    fn new(rules: Arc<Inner>, world: W, answers: Vec<i32>, depth: u32) -> Self {
        Self { rules: Some(rules), world: Some(world), answers, next_answer: 0, options: vec![], asked: None, depth }
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
    entry: &str,
    card: i32,
    seat: i32,
) -> Result<i32, Error> {
    let slot = rules.slots[card as usize];
    let linker = linker::<W>(&rules.engine)?;
    let inst = instantiate(&linker, &mut *store, &rules.modules[slot.module].module)?;
    if entry == export::CAN_REACT {
        inst.get_typed_func::<(i32, i32), i32>(&mut *store, entry)?.call(&mut *store, (slot.local, seat))
    } else {
        inst.get_typed_func::<(i32, i32), ()>(&mut *store, entry)?.call(&mut *store, (slot.local, seat))?;
        Ok(0)
    }
}

/// Like [`call_card`] for entries that answer with a packed `Msg` buffer
/// (`bandori_why_not`): the buffer is read back before the instance drops.
fn call_card_msg<W: CardWorld>(
    rules: &Inner,
    store: &mut Store<HostState<W>>,
    entry: &str,
    card: i32,
    seat: i32,
) -> Result<Option<crate::Msg>, Error> {
    let slot = rules.slots[card as usize];
    let linker = linker::<W>(&rules.engine)?;
    let inst = instantiate(&linker, &mut *store, &rules.modules[slot.module].module)?;
    let packed = inst.get_typed_func::<(i32, i32), i64>(&mut *store, entry)?.call(&mut *store, (slot.local, seat))?;
    if packed == 0 {
        return Ok(None);
    }
    let (ptr, len) = abi::unpack(packed);
    let bytes = read_mem(&inst, store, ptr as i32, len as i32)?;
    let m: card_sdk::msg::Msg =
        postcard::from_bytes(&bytes).map_err(|e| err(format!("{entry}: guest message is not Msg postcard ({e})")))?;
    Ok(Some(engine_msg(m)))
}

/// Validate one module and read its manifest.
fn inspect(engine: &Engine, wasm: &[u8]) -> Result<(Module, Vec<CardInfo>), RuleError> {
    let load = |m: String| RuleError::Load(m);
    let module = be::compile(engine, wasm).map_err(load)?;

    // Metadata exports never touch the world; a no-op world is enough.
    let mut store = Store::new(engine, HostState::<NullWorld>::without_rules());
    set_fuel(&mut store, DEFAULT_FUEL).map_err(load)?;
    let inst = linker::<NullWorld>(engine)
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
    let cards: Vec<CardInfo> = entries
        .into_iter()
        .map(|e| CardInfo { id: e.id, play: e.play, react: e.react })
        .collect();
    if cards.is_empty() {
        return Err(load("module declares no cards".into()));
    }
    for name in [export::PLAY, export::CAN_REACT, export::REACT, export::WHY_NOT] {
        if !has_func(&inst, &mut store, name) {
            return Err(load(format!("missing export {name}")));
        }
    }
    Ok((module, cards))
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
            G::Seat(v) => E::Seat(v),
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

fn linker<W: CardWorld>(engine: &Engine) -> Result<Linker<HostState<W>>, Error> {
    let mut l = Linker::<HostState<W>>::new(engine);
    let m = IMPORT_MODULE;
    type C<'a, W> = Caller<'a, HostState<W>>;

    l.func_wrap(m, "roll", |mut c: C<W>, seat: i32, count: i32, sides: i32| -> Result<i32, Error> {
        if !(1..=100).contains(&count) || !(1..=1000).contains(&sides) {
            return Err(err(format!("roll({count}d{sides}) out of range")));
        }
        Ok(c.data_mut().w().roll(seat, count, sides))
    })?;
    l.func_wrap(m, "log", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<(), Error> {
        let msg = guest_msg(&mut c, p, n)?;
        c.data_mut().w().log(seat, msg);
        Ok(())
    })?;
    l.func_wrap(m, "tile_count", |c: C<W>| c.data().wr().tile_count())?;
    l.func_wrap(m, "add_mark", |mut c: C<W>, tile: i32, seat: i32, kp: i32, kl: i32, p: i32, n: i32| -> Result<(), Error> {
        let kind = guest_str(&mut c, kp, kl)?;
        let note = guest_msg(&mut c, p, n)?;
        c.data_mut().w().add_mark(tile, seat, &kind, note);
        Ok(())
    })?;
    l.func_wrap(m, "money", |c: C<W>, seat: i32| c.data().wr().money(seat))?;
    l.func_wrap(m, "gain", |mut c: C<W>, seat: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
        let src = guest_msg(&mut c, p, n)?;
        Ok(c.data_mut().w().gain(seat, amount, src))
    })?;
    l.func_wrap(m, "pay", |mut c: C<W>, seat: i32, amount: i32, p: i32, n: i32| -> Result<i32, Error> {
        let src = guest_msg(&mut c, p, n)?;
        Ok(c.data_mut().w().pay(seat, amount, src))
    })?;
    l.func_wrap(m, "seat_count", |c: C<W>| c.data().wr().seat_count())?;
    l.func_wrap(m, "seat_out", |c: C<W>, seat: i32| c.data().wr().seat_out(seat))?;
    l.func_wrap(m, "others_count", |c: C<W>, seat: i32| c.data().wr().others_count(seat))?;
    l.func_wrap(m, "others_at", |c: C<W>, seat: i32, index: i32| c.data().wr().others_at(seat, index))?;
    l.func_wrap(m, "tile_named", |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().tile_named(&name))
    })?;
    l.func_wrap(m, "tile_owner", |c: C<W>, tile: i32| c.data().wr().tile_owner(tile))?;
    l.func_wrap(m, "seat_pos", |c: C<W>, seat: i32| c.data().wr().seat_pos(seat))?;
    l.func_wrap(m, "tile_steps_ahead", |c: C<W>, seat: i32, steps: i32| c.data().wr().tile_steps_ahead(seat, steps))?;
    l.func_wrap(m, "rent_of", |c: C<W>, tile: i32| c.data().wr().rent_of(tile))?;
    l.func_wrap(m, "buy_price", |c: C<W>, tile: i32| c.data().wr().buy_price(tile))?;
    l.func_wrap(m, "build_cost", |c: C<W>, tile: i32| c.data().wr().build_cost(tile))?;
    l.func_wrap(m, "mortgage_value", |c: C<W>, tile: i32| c.data().wr().mortgage_value(tile))?;
    l.func_wrap(m, "owned_count", |c: C<W>, seat: i32| c.data().wr().owned_count(seat))?;
    l.func_wrap(m, "owned_at", |c: C<W>, seat: i32, index: i32| c.data().wr().owned_at(seat, index))?;
    l.func_wrap(m, "is_buyable", |c: C<W>, tile: i32| c.data().wr().is_buyable(tile))?;
    l.func_wrap(m, "is_shop", |c: C<W>, tile: i32| c.data().wr().is_shop(tile))?;
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
    l.func_wrap(m, "set_owner", |mut c: C<W>, tile: i32, seat: i32| {
        c.data_mut().w().set_owner(tile, seat);
        Ok(())
    })?;
    l.func_wrap(m, "dist", |c: C<W>, a: i32, b: i32| c.data().wr().dist(a, b))?;
    l.func_wrap(m, "tile_forward", |c: C<W>, a: i32, b: i32| c.data().wr().tile_forward(a, b))?;
    l.func_wrap(m, "neighbor", |c: C<W>, seat: i32, dir: i32| c.data().wr().neighbor(seat, dir))?;
    l.func_wrap(m, "seats_on_count", |c: C<W>, tile: i32, except: i32| c.data().wr().seats_on_count(tile, except))?;
    l.func_wrap(m, "seats_on_at", |c: C<W>, tile: i32, except: i32, index: i32| c.data().wr().seats_on_at(tile, except, index))?;
    l.func_wrap(m, "draw", |mut c: C<W>, seat: i32, n: i32| -> Result<i32, Error> {
        Ok(c.data_mut().w().draw(seat, n))
    })?;
    l.func_wrap(m, "add_to_hand", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, p, n)?;
        c.data_mut().w().add_to_hand(seat, &card);
        Ok(())
    })?;
    l.func_wrap(m, "add_to_deck", |mut c: C<W>, seat: i32, p: i32, n: i32, shuffle: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, p, n)?;
        c.data_mut().w().add_to_deck(seat, &card, shuffle != 0);
        Ok(())
    })?;
    l.func_wrap(m, "add_to_deck_at", |mut c: C<W>, seat: i32, p: i32, n: i32, pos: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, p, n)?;
        c.data_mut().w().add_to_deck_at(seat, &card, pos);
        Ok(())
    })?;
    l.func_wrap(m, "to_discard", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, p, n)?;
        c.data_mut().w().to_discard(seat, &card);
        Ok(())
    })?;
    l.func_wrap(m, "hand_count", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<i32, Error> {
        let card = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().hand_count(seat, &card))
    })?;
    l.func_wrap(m, "discard_count", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<i32, Error> {
        let card = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().discard_count(seat, &card))
    })?;
    l.func_wrap(m, "deck_count", |c: C<W>, seat: i32| c.data().wr().deck_count(seat))?;
    l.func_wrap(m, "discard_size", |c: C<W>, seat: i32| c.data().wr().discard_size(seat))?;
    l.func_wrap(m, "discard_from_hand", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<i32, Error> {
        let card = guest_str(&mut c, p, n)?;
        Ok(c.data_mut().w().discard_from_hand(seat, &card))
    })?;
    l.func_wrap(m, "sweep_to_deck", |mut c: C<W>, seat: i32| c.data_mut().w().sweep_to_deck(seat))?;
    l.func_wrap(m, "unplace_card", |mut c: C<W>, seat: i32| c.data_mut().w().unplace_card(seat) as i32)?;
    l.func_wrap(m, "is_placed", |c: C<W>, seat: i32| c.data().wr().is_placed(seat))?;
    l.func_wrap(m, "count_marks", |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32| -> Result<i32, Error> {
        let kind = guest_str(&mut c, kp, kl)?;
        Ok(c.data().wr().count_marks(tile, &kind, owner))
    })?;
    l.func_wrap(m, "remove_marks", |mut c: C<W>, tile: i32, kp: i32, kl: i32, owner: i32| -> Result<i32, Error> {
        let kind = guest_str(&mut c, kp, kl)?;
        Ok(c.data_mut().w().remove_marks(tile, &kind, owner))
    })?;
    l.func_wrap(m, "tok", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().tok(seat, &name))
    })?;
    l.func_wrap(m, "set_tok", |mut c: C<W>, seat: i32, p: i32, n: i32, v: i32| -> Result<(), Error> {
        let name = guest_str(&mut c, p, n)?;
        c.data_mut().w().set_tok(seat, &name, v);
        Ok(())
    })?;
    l.func_wrap(m, "add_tok", |mut c: C<W>, seat: i32, p: i32, n: i32, by: i32, max: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data_mut().w().add_tok(seat, &name, by, max))
    })?;
    l.func_wrap(m, "slot", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<i32, Error> {
        let key = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().slot(seat, &key))
    })?;
    l.func_wrap(m, "set_slot", |mut c: C<W>, seat: i32, p: i32, n: i32, v: i32| -> Result<(), Error> {
        let key = guest_str(&mut c, p, n)?;
        c.data_mut().w().set_slot(seat, &key, v);
        Ok(())
    })?;
    l.func_wrap(m, "inc_slot", |mut c: C<W>, seat: i32, p: i32, n: i32, by: i32| -> Result<i32, Error> {
        let key = guest_str(&mut c, p, n)?;
        Ok(c.data_mut().w().inc_slot(seat, &key, by))
    })?;
    l.func_wrap(m, "band_crystals", |c: C<W>, seat: i32| c.data().wr().band_crystals(seat))?;
    l.func_wrap(m, "add_band_crystals", |mut c: C<W>, seat: i32, n: i32, max: i32| c.data_mut().w().add_band_crystals(seat, n, max))?;
    l.func_wrap(m, "fire", |c: C<W>, seat: i32| c.data().wr().fire(seat))?;
    l.func_wrap(m, "fire_max", |c: C<W>, seat: i32| c.data().wr().fire_max(seat))?;
    l.func_wrap(m, "gain_fire", |mut c: C<W>, seat: i32, n: i32, p: i32, l: i32| -> Result<i32, Error> {
        let why = guest_msg(&mut c, p, l)?;
        Ok(c.data_mut().w().gain_fire(seat, n, why))
    })?;
    l.func_wrap(m, "give_stay", |mut c: C<W>, seat: i32, n: i32| {
        c.data_mut().w().give_stay(seat, n);
        Ok(())
    })?;
    l.func_wrap(m, "give_stun", |mut c: C<W>, seat: i32, n: i32| {
        c.data_mut().w().give_stun(seat, n);
        Ok(())
    })?;
    l.func_wrap(m, "give_exile", |mut c: C<W>, seat: i32, n: i32, to: i32| {
        c.data_mut().w().give_exile(seat, n, to);
        Ok(())
    })?;
    l.func_wrap(m, "give_extra_turn", |mut c: C<W>, seat: i32| {
        c.data_mut().w().give_extra_turn(seat);
        Ok(())
    })?;
    l.func_wrap(m, "can_pay", |c: C<W>, seat: i32| c.data().wr().can_pay(seat))?;
    l.func_wrap(m, "spend_fire", |mut c: C<W>, seat: i32, n: i32, p: i32, l: i32| -> Result<i32, Error> {
        let why = guest_msg(&mut c, p, l)?;
        Ok(c.data_mut().w().spend_fire(seat, n, why))
    })?;
    l.func_wrap(m, "stay_of", |c: C<W>, seat: i32| c.data().wr().stay_of(seat))?;
    l.func_wrap(m, "stun_of", |c: C<W>, seat: i32| c.data().wr().stun_of(seat))?;
    l.func_wrap(m, "turn_seat", |c: C<W>| c.data().wr().turn_seat())?;
    l.func_wrap(m, "round_no", |c: C<W>| c.data().wr().round_no())?;
    l.func_wrap(m, "turn_key", |c: C<W>| c.data().wr().turn_key())?;
    l.func_wrap(m, "character_is", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().character_is(seat, &name))
    })?;
    l.func_wrap(m, "in_band", |mut c: C<W>, seat: i32, p: i32, n: i32| -> Result<i32, Error> {
        let name = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().in_band(seat, &name))
    })?;
    l.func_wrap(m, "place_card_at", |mut c: C<W>, seat: i32, cp: i32, cl: i32, p: i32, n: i32| -> Result<(), Error> {
        let card = guest_str(&mut c, cp, cl)?;
        let note = guest_msg(&mut c, p, n)?;
        c.data_mut().w().place_card(seat, &card, note);
        Ok(())
    })?;
    l.func_wrap(m, "set_dest", |mut c: C<W>, dest: i32| {
        c.data_mut().w().set_dest(dest);
        Ok(())
    })?;
    l.func_wrap(m, "ring_multiplier", |c: C<W>| c.data().wr().ring_multiplier())?;
    l.func_wrap(m, "add_ring_bonus", |mut c: C<W>, n: i32| c.data_mut().w().add_ring_bonus(n))?;
    l.func_wrap(m, "teleport_to", |mut c: C<W>, seat: i32, tile: i32| {
        c.data_mut().w().teleport_to(seat, tile);
        Ok(())
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
        |mut c: C<W>, kind: i32, seat: i32, tp: i32, tl: i32, xp: i32, xl: i32| -> Result<i32, Error> {
            let kind = PromptKind::from_i32(kind).ok_or_else(|| err(format!("bad prompt kind {kind}")))?;
            let title = guest_msg(&mut c, tp, tl)?;
            let text = guest_msg(&mut c, xp, xl)?;
            let st = c.data_mut();
            let options = std::mem::take(&mut st.options);
            if kind != PromptKind::Yes && options.is_empty() {
                return Err(err(format!("{} prompt with no options", kind.as_str())));
            }
            if let Some(&a) = st.answers.get(st.next_answer) {
                st.next_answer += 1;
                return Ok(a);
            }
            st.asked = Some(Prompt { kind, seat, title, text, options, answer_slot: st.next_answer });
            Err(need_input())
        },
    )?;
    l.func_wrap(m, "trig_kind", |c: C<W>| c.data().wr().trigger().kind as i32)?;
    l.func_wrap(m, "trig_seat", |c: C<W>| c.data().wr().trigger().seat)?;
    l.func_wrap(m, "trig_target", |c: C<W>| c.data().wr().trigger().target)?;
    l.func_wrap(m, "trig_tile", |c: C<W>| c.data().wr().trigger().tile)?;
    l.func_wrap(m, "trig_value", |c: C<W>| c.data().wr().trigger().value)?;
    l.func_wrap(m, "trig_move_roll", |c: C<W>| c.data().wr().trigger().move_roll.unwrap_or(-1))?;
    l.func_wrap(m, "trig_set_move_roll", |mut c: C<W>, v: i32| c.data_mut().w().set_trigger_move_roll(v))?;
    l.func_wrap(m, "trig_card_is", |mut c: C<W>, p: i32, n: i32| -> Result<i32, Error> {
        let id = guest_str(&mut c, p, n)?;
        Ok(c.data().wr().trig_card_is(&id))
    })?;

    // Cross-module call: run another card's `play` inside this run. The world, the
    // answer log position and the remaining fuel move into a nested store and back,
    // so a prompt raised by the inner card aborts (and later replays) the outer run.
    l.func_wrap(m, "play_card", |mut c: C<W>, p: i32, n: i32, seat: i32| -> Result<(), Error> {
        let id = guest_str(&mut c, p, n)?;
        let rules = c.data().rules.clone().ok_or_else(|| err("play_card unavailable here"))?;
        let card = *rules.by_id.get(&id).ok_or_else(|| err(format!("play_card: unknown card {id:?}")))?;
        if c.data().depth >= MAX_NESTING {
            return Err(err(format!("play_card nested deeper than {MAX_NESTING}")));
        }
        let fuel = c.get_fuel()?;
        let st = c.data_mut();
        let mut nested = HostState::new(rules.clone(), st.world.take().expect("world present"), std::mem::take(&mut st.answers), st.depth + 1);
        nested.next_answer = st.next_answer;
        let mut store = Store::new(&rules.engine, nested);
        store.set_fuel(fuel)?;
        let res = call_card(&rules, &mut store, export::PLAY, card, seat);
        let left = store.get_fuel().unwrap_or(0);
        let inner = store.into_data();
        let st = c.data_mut();
        st.world = inner.world;
        st.answers = inner.answers;
        st.next_answer = inner.next_answer;
        if inner.asked.is_some() {
            st.asked = inner.asked;
        }
        c.set_fuel(left)?;
        res.map(|_| ())
    })?;

    Ok(l)
}

impl HostState<NullWorld> {
    fn without_rules() -> Self {
        Self { rules: None, world: Some(NullWorld), answers: vec![], next_answer: 0, options: vec![], asked: None, depth: 0 }
    }
}

/// World used only while reading module metadata.
#[derive(Clone)]
struct NullWorld;

impl CardWorld for NullWorld {
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
    fn seat_pos(&self, _: i32) -> i32 {
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
    fn seat_count(&self) -> i32 {
        1
    }
    fn seat_out(&self, _: i32) -> i32 {
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
    fn trig_card_is(&self, _: &str) -> i32 {
        0
    }
    fn is_buyable(&self, _: i32) -> i32 {
        0
    }
    fn is_shop(&self, _: i32) -> i32 {
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
    fn seats_on_count(&self, _: i32, _: i32) -> i32 {
        0
    }
    fn seats_on_at(&self, _: i32, _: i32, _: i32) -> i32 {
        -1
    }
    fn hand_count(&self, _: i32, _: &str) -> i32 {
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
    fn sweep_to_deck(&mut self, _: i32) -> i32 {
        0
    }
    fn can_pay(&self, _: i32) -> i32 {
        0
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
    fn turn_seat(&self) -> i32 {
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
