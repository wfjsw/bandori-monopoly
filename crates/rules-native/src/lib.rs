//! The card ruleset as plain native Rust, for the bot's simulations.
//!
//! `docs/BOT.md` §3.1 (B1). The same `rules/*` crates the sandboxed
//! `WasmRules` loads as wasm modules are **linked** into this binary; the
//! `bandori` host imports become `#[no_mangle]` shims over a thread-local
//! [`NativeHost`], and the guest entry point is a direct call to
//! `card_sdk::rt::on` -- no wasmtime/wasmi `Store`, no instantiation.
//!
//! **Advisory only.** A real match always runs the sandboxed `WasmRules`.
//! A drift between the two makes the bot weaker, never corrupts a match
//! (BOT.md §1). The drift check (`tests/drift.rs`, `examples/drift.rs`)
//! compares `save()` checkpoints of seeded games across both backends.
//!
//! Panics: a card that panics (or a host import that traps) unwinds to the
//! entry point and becomes `RuleError::Trap` -- "this card is out", the same
//! contract the sandbox trap has. That is why this crate is built with
//! `panic = "unwind"` (the root workspace's profile), not the `rules/`
//! workspace's `panic = "abort"`.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use card_sdk::abi::{export, ManifestOn, OnKind, ABI_VERSION};
use card_sdk::On;
use game_rules::{
    Call, CallOut, CardInfo, CardModules, HookRun, HostCtx, HostErr, HostState, Outcome,
    RuleError, RulesHandle, Run, TriggerKind, DEFAULT_FUEL, MAX_NESTING,
};
use game_rules::hostfns;
use game_rules::{CardWorld, Msg, RulesBridge};

/// The linked ruleset table, as `bandori_ruleset!` publishes it on a native
/// build (one band table per rule crate, in `card-all`'s aggregation order).
pub static RULESET: &[&[card_sdk::CardDef]] = &[
    card_ag::CARDS,
    card_crychic::CARDS,
    card_general::CARDS,
    card_hhw::CARDS,
    card_morfonica::CARDS,
    card_mujica::CARDS,
    card_mygo::CARDS,
    card_pp::CARDS,
    card_ppp::CARDS,
    card_ras::CARDS,
    card_roselia::CARDS,
    card_sumimi::CARDS,
    skill_bands::CARDS,
    skill_characters::CARDS,
    rules_tiles::CARDS,
    rules_tile_marks::CARDS,
    rules_events::CARDS,
];

// ---------------------------------------------------------------------------
// Rules index over the linked table
// ---------------------------------------------------------------------------

/// [`RulesHandle`] over [`RULESET`]: the native counterpart of the sandbox's
/// module index. Also carries the flattened `CardInfo` list the bridge reads.
#[derive(Clone)]
pub struct NativeRulesHandle {
    inner: Arc<NativeIndex>,
}

struct NativeIndex {
    cards: Vec<CardInfo>,
    /// Compiled guard conditions (docs/GUARDS.md G0), parallel to
    /// `cards[i].on[j]`. Read from the same `On::condition()` strings the
    /// sandbox manifest carries.
    pre: Vec<Vec<Option<game_rules::CompiledPre>>>,
    by_id: HashMap<String, i32>,
    declared: u128,
}

impl NativeIndex {
    fn pre(&self, card: i32, entry: i32) -> Option<&game_rules::CompiledPre> {
        self.pre.get(card as usize)?.get(entry as usize)?.as_ref()
    }
}

impl NativeRulesHandle {
    pub fn new() -> Self {
        let mut cards = Vec::new();
        let mut pre: Vec<Vec<Option<game_rules::CompiledPre>>> = Vec::new();
        let mut by_id = HashMap::new();
        let mut declared: u128 = 0;
        for band in RULESET {
            for card in band.iter() {
                let idx = cards.len() as i32;
                by_id.insert(card.id.to_string(), idx);
                let on: Vec<ManifestOn> = card
                    .on
                    .iter()
                    .enumerate()
                    .map(|(ei, o)| ManifestOn {
                        kind: o.kind() as i32,
                        triggers: o.triggers(),
                        pre: {
                            let p = o.condition();
                            if p.is_empty() {
                                None
                            } else {
                                Some(p.to_string())
                            }
                        },
                        has_guard: o.has_guard(),
                        has_legacy: card.legacy.iter().any(|(e, _)| *e == ei as i32),
                        messages: match o {
                            On::Message(names, ..) => {
                                names.iter().map(|s| String::from(*s)).collect()
                            }
                            _ => Vec::new(),
                        },
                        label: card
                            .labels
                            .iter()
                            .find(|(e, _)| *e == ei as i32)
                            .map(|(_, l)| String::from(*l)),
                    })
                    .collect();
                // Same bitset the sandbox builds (): trigger
                // kinds only.  values share the 0..128 numbering, so
                // also setting those bits would make  say yes for
                // kinds nothing listens to and open [反击] windows the sandbox
                // skips -- a drift the check caught.
                for o in &on {
                    for t in &o.triggers {
                        if (0..128).contains(t) {
                            declared |= 1u128 << *t;
                        }
                    }
                }
                let mut props: BTreeMap<String, i32> = card
                    .props
                    .iter()
                    .map(|(k, v)| (k.to_string(), *v))
                    .collect();
                // Deterministic iteration like the manifest's sorted wire form.
                let _ = &mut props;
                // Compile every condition once (docs/GUARDS.md §4.3), from the
                // same strings the sandbox manifest carries. Fail-closed.
                let mut row: Vec<Option<game_rules::CompiledPre>> = Vec::with_capacity(card.on.len());
                for o in card.on.iter() {
                    let src = o.condition();
                    let compiled = if src.is_empty() {
                        None
                    } else {
                        Some(game_rules::CompiledPre::compile(src).unwrap_or_else(|e| {
                            panic!("{}: bad condition {src:?}: {e}", card.id)
                        }))
                    };
                    row.push(compiled);
                }
                pre.push(row);
                cards.push(CardInfo {
                    id: card.id.to_string(),
                    on,
                    props,
                });
            }
        }
        Self {
            inner: Arc::new(NativeIndex {
                cards,
                pre,
                by_id,
                declared,
            }),
        }
    }

    pub fn cards(&self) -> &[CardInfo] {
        &self.inner.cards
    }

    pub fn declares(&self, kind: TriggerKind) -> bool {
        let v = kind as i32;
        (0..128).contains(&v) && (self.inner.declared & (1u128 << v)) != 0
    }

    /// Content hash of the linked table, so a record stamped with it can tell
    /// native and sandbox runs of the same ruleset apart from a stale one.
    pub fn content_sha256(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(ABI_VERSION.to_le_bytes());
        for band in RULESET {
            for card in band.iter() {
                h.update(card.id.as_bytes());
                h.update(0u8.to_le_bytes());
                let mut props: Vec<_> = card.props.iter().collect();
                props.sort_by(|a, b| a.0.cmp(b.0));
                for (k, v) in props {
                    h.update(k.as_bytes());
                    h.update(v.to_le_bytes());
                }
                for o in card.on {
                    h.update((o.kind() as i32).to_le_bytes());
                    for t in o.triggers() {
                        h.update(t.to_le_bytes());
                    }
                }
            }
        }
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

impl Default for NativeRulesHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl RulesHandle for NativeRulesHandle {
    fn by_id(&self, id: &str) -> Option<i32> {
        self.inner.by_id.get(id).copied()
    }
    fn card(&self, i: i32) -> Option<&CardInfo> {
        self.inner.cards.get(i as usize)
    }
    /// The compiled condition of `cards[i].on[entry]` (docs/GUARDS.md G0).
    /// **Must not fall back to the `RulesHandle` default (`None`)** -- that
    /// reads as "no condition" and `admits_pre` / `admits_gate` then admit
    /// every entry, which is how a `pre::MINE` hook fired for the wrong
    /// player on native while the sandbox skipped it (the drift `tests/drift.rs`
    /// caught in round 1).
    fn pre(&self, i: i32, entry: i32) -> Option<&game_rules::CompiledPre> {
        self.inner.pre(i, entry)
    }
}

// ---------------------------------------------------------------------------
// The host a card run executes against
// ---------------------------------------------------------------------------

/// A host import aborted the run with a panic, so the guest never returns.
/// The entry point folds this back into [`HostErr`] / `EXIT_NEED_INPUT`.
pub enum NativeExit {
    NeedInput,
    Trap(String),
}

pub(crate) type NativeHostState = HostState<Run, NativeRulesHandle>;

/// The native [`HostCtx`]: one run's [`HostState`] plus a plain fuel counter
/// (the sandbox has store fuel; the native build has no instruction meter, so
/// the budget is the host-call budget the nested-call plumbing shares).
pub struct NativeHost {
    pub state: NativeHostState,
    pub fuel: u64,
}

impl HostCtx for NativeHost {
    type World = Run;
    type Rules = NativeRulesHandle;

    fn st(&self) -> &NativeHostState {
        &self.state
    }
    fn st_mut(&mut self) -> &mut NativeHostState {
        &mut self.state
    }
    fn read_guest(&mut self, ptr: i32, len: i32) -> Result<Vec<u8>, HostErr> {
        Ok(card_sdk::native::read(ptr, len as usize))
    }
    fn write_guest(&mut self, ptr: i32, bytes: &[u8]) -> Result<(), HostErr> {
        card_sdk::native::write(ptr, bytes);
        Ok(())
    }
    fn fuel(&mut self) -> Result<u64, HostErr> {
        Ok(self.fuel)
    }
    fn set_fuel(&mut self, v: u64) -> Result<(), HostErr> {
        self.fuel = v;
        Ok(())
    }
    #[allow(clippy::type_complexity)]
    fn call_entry(
        &mut self,
        nested: NativeHostState,
        card: i32,
        entry: i32,
        op: i32,
        player_id: i32,
        want_msg: bool,
    ) -> (
        Result<CallOut, HostErr>,
        NativeHostState,
        u64,
    ) {
        let fuel = self.fuel;
        let (res, state, left) = run_on(nested, card, entry, op, player_id, want_msg, fuel);
        (res, state, left)
    }
}

/// The `#[no_mangle] bandori_*` shims live in `game_rules::native_shims`
/// (the crate every `card-sdk/guest` consumer links) and dispatch into this
/// crate's [`NativeHost`] through the [`game_rules::native_shims::HostOps`]
/// blanket impl. See `docs/BOT.md` B1.
fn quiet_control_flow_panics() {
    use std::panic::{set_hook, take_hook, PanicHookInfo};
    use std::sync::Once;
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let prev = take_hook();
        set_hook(Box::new(move |info: &PanicHookInfo<'_>| {
            if info.payload().is::<NativeExit>()
                || info.payload().is::<HostErr>()
            {
                return;
            }
            prev(info);
        }));
    });
}

// ---------------------------------------------------------------------------
// One card call: install the host, run the entry, fold the result
// ---------------------------------------------------------------------------

/// Guard on the guest call chain. The sandbox bounds a runaway effect with
/// store fuel and a wasm stack limit (a trap = "this card is out"); the native
/// build has no instruction meter, so the bound is the call depth. The limit
/// is far above anything the shipped ruleset produces (the deepest chain in
/// the drift suite is ~20) and far below a thread stack.
const MAX_RUN_DEPTH: u32 = 256;

thread_local! {
    static RUN_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// RAII depth counter -- drops (including on unwind) decrement.
struct DepthGuard;

impl DepthGuard {
    fn enter() -> Result<Self, HostErr> {
        RUN_DEPTH.with(|d| {
            let v = d.get();
            if v >= MAX_RUN_DEPTH {
                return Err(HostErr::trap(format!(
                    "guest call nested deeper than {MAX_RUN_DEPTH}"
                )));
            }
            d.set(v + 1);
            Ok(Self)
        })
    }
}

impl Drop for DepthGuard {
    fn drop(&mut self) {
        RUN_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

fn run_on(
    nested: NativeHostState,
    card: i32,
    entry: i32,
    op: i32,
    player_id: i32,
    want_msg: bool,
    fuel: u64,
) -> (
    Result<CallOut, HostErr>,
    NativeHostState,
    u64,
) {
    let rules = match nested.rules.clone() {
        Some(r) => r,
        None => {
            return (
                Err(HostErr::trap("call_entry without rules")),
                nested,
                fuel,
            )
        }
    };
    let _depth = match DepthGuard::enter() {
        Ok(g) => g,
        Err(e) => return (Err(e), nested, fuel),
    };
    quiet_control_flow_panics();
    let host = NativeHost {
        state: nested,
        fuel,
    };
    // Install for the duration of the guest call. A nested `play_card` swaps
    // the slot again and restores it below, so the outer run continues after.
    let prev = game_rules::native_shims::install_host(Box::new(host));
    let res = catch_unwind(AssertUnwindSafe(|| {
        card_sdk::rt::on(RULESET, card, entry, op, player_id)
    }));
    let host_box = game_rules::native_shims::take_host().expect("host installed");
    if let Some(p) = prev {
        let _ = game_rules::native_shims::install_host(p);
    }
    // Recover the concrete host. This crate is the only installer, and it
    // always installs a `NativeHost`, so the fat-to-thin cast is exact.
    let host: Box<NativeHost> = {
        let raw: *mut dyn game_rules::native_shims::HostOps = Box::into_raw(host_box);
        unsafe { Box::from_raw(raw as *mut NativeHost) }
    };
    let out = match res {
        Ok(v) => {
            if v == card_sdk::abi::EXIT_NEED_INPUT as i64 {
                Err(HostErr::NeedInput)
            } else {
                Ok(CallOut::Code(v))
            }
        }
        Err(p) => {
            if let Some(NativeExit::NeedInput) = p.downcast_ref::<NativeExit>() {
                Err(HostErr::NeedInput)
            } else if let Some(NativeExit::Trap(m)) = p.downcast_ref::<NativeExit>() {
                Err(HostErr::Trap(m.clone()))
            } else if let Some(e) = p.downcast_ref::<HostErr>() {
                Err(e.clone())
            } else {
                Err(HostErr::Trap(
                    p.downcast_ref::<String>()
                        .cloned()
                        .unwrap_or_else(|| "card panicked".into()),
                ))
            }
        }
    };
    let out = if want_msg && matches!(out, Ok(CallOut::Code(0))) {
        Ok(CallOut::Msg(None))
    } else if want_msg {
        // A gate answering with a packed `Msg`: `rt::on` returns the packed
        // buffer as the code. Unpack it through the arena before it drops.
        match out {
            Ok(CallOut::Code(packed)) if packed != 0 => {
                let (ptr, len) = card_sdk::abi::unpack(packed);
                let bytes = card_sdk::native::read(ptr as i32, len as usize);
                match postcard::from_bytes::<card_sdk::msg::Msg>(&bytes) {
                    Ok(m) => Ok(CallOut::Msg(Some(game_rules::engine_msg(m)))),
                    Err(_) => Err(HostErr::trap("cant_play: guest message is not Msg postcard")),
                }
            }
            other => other,
        }
    } else {
        out
    };
    (out, host.state, host.fuel)
}

/// Fold a finished run's state into an [`Outcome`], the native counterpart of
/// `host::finish`.
fn finish(state: NativeHostState, res: Result<CallOut, HostErr>) -> Result<Outcome<Run>, RuleError> {
    match res {
        Ok(_) => {
            if state.asked.is_some() || state.host_request.is_some() {
                return Err(RuleError::Trap(
                    "card published a prompt and kept going".into(),
                ));
            }
            Ok(Outcome::Done(
                state.world.expect("world is restored after nested calls"),
            ))
        }
        Err(HostErr::NeedInput) => {
            if let Some(req) = state.host_request {
                let world = state.world.expect("world is restored after nested calls");
                Ok(Outcome::NeedHost(req, world))
            } else if let Some(p) = state.asked {
                Ok(Outcome::NeedInput(p))
            } else {
                Err(RuleError::Trap("need-input exit without a prompt".into()))
            }
        }
        Err(HostErr::Trap(m)) => Err(RuleError::Trap(m)),
    }
}

fn entry_of(info: &CardInfo, call: &Call, trigger_kind: TriggerKind) -> Option<i32> {
    match call {
        Call::Play { .. } => info.entry(OnKind::Play, None),
        // (Play mode selection happens in `NativeModules::play_entry_for`.)
        Call::Counteract { .. } => info.entry(OnKind::Counteract, Some(trigger_kind)),
        Call::Hook { kind, .. } => info.hook_entry(*kind),
        Call::AtEnd { .. } => info.entry(OnKind::AtEnd, None),
        Call::RollPlan { .. } => info.entry(OnKind::RollPlan, None),
        Call::Settle { .. } => info.entry(OnKind::Settle, None),
    }
}

// ---------------------------------------------------------------------------
// `CardModules` over the linked table
// ---------------------------------------------------------------------------

/// The native `CardModules`: same manifest / declares / prefilters semantics
/// as [`game_rules::Ruleset`], but the card runs are direct Rust calls.
#[derive(Clone)]
pub struct NativeModules {
    index: NativeRulesHandle,
    sha: Arc<str>,
}

impl NativeModules {
    pub fn new() -> Self {
        let index = NativeRulesHandle::new();
        let sha: Arc<str> = index.content_sha256().into();
        Self { index, sha }
    }

    pub fn handle(&self) -> &NativeRulesHandle {
        &self.index
    }

    /// Play-mode selection (mirrors `game_rules::host::Ruleset::play_entry_for`):
    /// the first **gated** entry whose gate admits; failing that, the first
    /// ungated entry. A `Play` gate answers `Option<Msg>`: `None` = playable.
    fn play_entry_for(&self, world: &Run, card: i32, player_id: i32) -> Option<i32> {
        let info = self.index.card(card)?;
        let entries = info.entries(OnKind::Play, None);
        if entries.is_empty() {
            return None;
        }
        let mut first_ungated = None;
        for &entry in &entries {
            if self.entry_guard_is_none(card, entry) && self.pre(card, entry).is_none() {
                first_ungated.get_or_insert(entry);
                continue;
            }
            let state = HostState::new(self.index.clone(), world.clone(), vec![], 0);
            let (res, _st, _fuel) = game_rules::inline::with_no_inline(|| {
                run_on(state, card, entry, export::OP_GUARD, player_id, true, DEFAULT_FUEL)
            });
            let gate_ok = match res {
                Ok(CallOut::Msg(_)) => false,
                Ok(_) => true,
                Err(_) => false,
            };
            if gate_ok {
                return Some(entry);
            }
        }
        first_ungated.or(entries.first().copied())
    }

    /// Category → condition → guard for one entry (v49, mirrors
    /// `game_rules::host::Ruleset::entry_admitted`). A rejecting condition
    /// skips the wasm guard entirely; a residual guard runs on a throwaway
    /// copy.
    fn entry_admitted(
        &self,
        world: &Run,
        card: i32,
        entry: i32,
        player_id: i32,
        scope: &game_rules::cond_pre::WindowScope,
        cand: &game_rules::cond_pre::CandidateCtx,
        answers: &[i32],
    ) -> bool {
        let pre = self.index.pre(card, entry);
        if self.entry_guard_is_none(card, entry) {
            // G4 deleted the residual: the condition alone decides.
            return game_rules::cond_pre::admits_pre(pre, Some(scope), cand);
        }
        let mut state = Some(HostState::new(
            self.index.clone(),
            world.clone(),
            answers.to_vec(),
            0,
        ));
        let asked: Result<bool, ()> = game_rules::cond_pre::admits(pre, Some(scope), cand, || {
            let st = state.take().expect("host state present");
            // A guard is a pure query: refuse inline answers (see
            // `game_rules::inline`).
            let (res, st, _fuel) = game_rules::inline::with_no_inline(|| {
                run_on(st, card, entry, export::OP_GUARD, player_id, false, DEFAULT_FUEL)
            });
            state = Some(st);
            Ok(match res {
                Ok(CallOut::Code(0)) => false,
                Ok(_) => true,
                Err(_) => false,
            })
        });
        matches!(asked, Ok(true))
    }
}

impl Default for NativeModules {
    fn default() -> Self {
        Self::new()
    }
}

impl CardModules for NativeModules {
    fn cards(&self) -> &[CardInfo] {
        self.index.cards()
    }
    fn card(&self, id: &str) -> Option<i32> {
        RulesHandle::by_id(&self.index, id)
    }
    fn card_props(&self, id: &str) -> BTreeMap<String, i32> {
        self.card(id)
            .and_then(|i| self.index.card(i))
            .map_or_else(Default::default, |c| c.props.clone())
    }
    fn card_prop(&self, id: &str, key: &str) -> i32 {
        self.card_props(id).get(key).copied().unwrap_or(0)
    }
    fn declares(&self, kind: TriggerKind) -> bool {
        self.index.declares(kind)
    }
    fn pre(&self, i: i32, entry: i32) -> Option<&game_rules::CompiledPre> {
        self.index.pre(i, entry)
    }
    fn entry_guard_is_none(&self, card: i32, entry: i32) -> bool {
        self.cards()
            .get(card as usize)
            .and_then(|c| c.on.get(entry as usize))
            .is_some_and(|o| !o.has_guard)
    }
    /// Fix A cheap pre-filter, same shape as [`game_rules::host::Ruleset`]:
    /// no `On::Play` entry, or a G4-deleted gate with no condition -- the
    /// verdict is always "playable".
    fn play_gate_vanishes(&self, card: i32) -> bool {
        let Some(info) = self.index.card(card) else {
            return true;
        };
        let Some(entry) = info.entry(OnKind::Play, None) else {
            return true;
        };
        self.entry_guard_is_none(card, entry) && self.pre(card, entry).is_none()
    }
    fn sha256(&self) -> Option<&str> {
        Some(&self.sha)
    }
    fn module_sha256(&self, _card: i32) -> Option<&str> {
        // One linked image: the set hash is the only one there is.
        Some(&self.sha)
    }

    fn run(
        &self,
        world: &Run,
        call: Call,
        answers: &[i32],
        mut on_body: Option<&mut dyn FnMut(&mut Run, i32)>,
    ) -> Result<Outcome<Run>, RuleError> {
        let (card, player_id) = (call.card(), call.player_id());
        let info = self
            .index
            .card(card)
            .ok_or_else(|| RuleError::Trap(format!("bad card handle {card}")))?;
        let Some(entry) = entry_of(info, &call, world.trigger().kind) else {
            return Ok(Outcome::Done(world.clone()));
        };
        // v49: AtEnd / RollPlan / Settle answer "does this entry apply?" here
        // (mirrors `Ruleset::run`). A rejecting entry runs no body and so never
        // flashes.
        if matches!(
            call,
            Call::AtEnd { .. } | Call::RollPlan { .. } | Call::Settle { .. }
        ) {
            let scope =
                game_rules::cond_pre::window_scope(&game_rules::cond_pre::fill_window(world));
            let cand = game_rules::cond_pre::fill_candidate(
                world,
                player_id,
                &info.id,
                world.is_placed() != 0,
            );
            if !self.entry_admitted(world, card, entry, player_id, &scope, &cand, answers) {
                return Ok(Outcome::Done(world.clone()));
            }
        }
        let mut state = HostState::new(
            self.index.clone(),
            world.clone(),
            answers.to_vec(),
            0,
        );
        // Body entry: the activation announcement goes here (mirrors
        // `Ruleset::run`), never for a bodyless drive.
        if let Some(cb) = on_body.as_deref_mut() {
            cb(state.w(), entry);
        }
        let (res, state, _fuel) = run_on(
            state,
            card,
            entry,
            export::OP_RUN,
            player_id,
            false,
            DEFAULT_FUEL,
        );
        finish(state, res)
    }

    fn run_hook(
        &self,
        world: &Run,
        call: Call,
        answers: &[i32],
        mut on_body: Option<&mut dyn FnMut(&mut Run, i32)>,
    ) -> Result<Option<HookRun<Run>>, RuleError> {
        let (card, player_id) = (call.card(), call.player_id());
        let kind = world.trigger().kind;
        let info = self
            .index
            .card(card)
            .ok_or_else(|| RuleError::Trap(format!("bad card handle {card}")))?;
        let entries = info.hook_entries(kind);
        if entries.is_empty() {
            return Ok(None);
        }
        let scope =
            game_rules::cond_pre::window_scope(&game_rules::cond_pre::fill_window(world));
        let cand = game_rules::cond_pre::fill_candidate(
            world,
            player_id,
            &info.id,
            world.is_placed() != 0,
        );
        let mut cur = world.clone();
        let mut announced = false;
        let mut ran_any = false;
        for entry in entries {
        let mut state = Some(HostState::new(
            self.index.clone(),
            cur.clone(),
            answers.to_vec(),
            0,
        ));
        // v49: `On::Hook` and `On::Gate` both carry the condition /
        // residual-guard pair; a rejecting entry is skipped (and so never
        // flashes).
        {
            // docs/GUARDS.md §4.4: every guard call goes through `admits`.
            let pre = self.index.pre(card, entry);
            let admitted = if self.entry_guard_is_none(card, entry) {
                // G4 deleted the residual: the condition alone decides.
                game_rules::cond_pre::admits_pre(pre, Some(&scope), &cand)
            } else {
                let asked: Result<bool, ()> =
                    game_rules::cond_pre::admits(pre, Some(&scope), &cand, || {
                        let st = state.take().expect("host state present");
                        // A guard is a pure query: refuse inline answers (see
                        // `game_rules::inline`).
                        let (res, st, _fuel) = game_rules::inline::with_no_inline(|| {
                            run_on(st, card, entry, export::OP_GUARD, player_id, false, DEFAULT_FUEL)
                        });
                        state = Some(st);
                        Ok(match res {
                            Ok(CallOut::Code(0)) => false,
                            Ok(_) => true,
                            Err(_) => false,
                        })
                    });
                matches!(asked, Ok(true))
            };
            if !admitted {
                continue;
            }
            announced = true;
        }
        let mut state = state.take().expect("host state present");
        if let Some(cb) = on_body.as_deref_mut() {
            cb(state.w(), entry);
        }
        let (res, state, _fuel) = run_on(
            state,
            card,
            entry,
            export::OP_RUN,
            player_id,
            false,
            DEFAULT_FUEL,
        );
        ran_any = true;
        let outcome = finish(state, res)?;
        match outcome {
            Outcome::Done(w) => {
                cur = w;
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

    fn can_counteract(
        &self,
        world: &Run,
        card: i32,
        player_id: i32,
    ) -> Result<bool, RuleError> {
        let scope = game_rules::cond_pre::window_scope(&game_rules::cond_pre::fill_window(world));
        self.can_counteract_scoped(world, card, player_id, &scope)
    }

    /// [`Self::can_counteract`] with the window scope the caller built once per
    /// trigger window (`docs/GUARDS.md` §4.4 item 1 / §4.5) and reused across
    /// every candidate probe. Mirrors [`game_rules::host::Ruleset`].
    fn can_counteract_scoped(
        &self,
        world: &Run,
        card: i32,
        player_id: i32,
        scope: &game_rules::cond_pre::WindowScope,
    ) -> Result<bool, RuleError> {
        let info = self
            .index
            .card(card)
            .ok_or_else(|| RuleError::Trap(format!("bad card handle {card}")))?;
        let Some(entry) = info.entry(OnKind::Counteract, Some(world.trigger().kind)) else {
            return Ok(false);
        };
        // docs/GUARDS.md §4.4: condition first, guard second, one `admits`.
        let pre = self.index.pre(card, entry);
        let cand = game_rules::cond_pre::fill_candidate(world, player_id, &info.id, false);
        if self.entry_guard_is_none(card, entry) {
            // G4 deleted the residual: the condition alone decides.
            return Ok(game_rules::cond_pre::admits_pre(pre, Some(scope), &cand));
        }
        game_rules::cond_pre::admits(pre, Some(scope), &cand, || {
            let state = HostState::new(self.index.clone(), world.clone(), vec![], 0);
            let (res, _state, _fuel) = game_rules::inline::with_no_inline(|| {
                run_on(state, card, entry, export::OP_GUARD, player_id, false, DEFAULT_FUEL)
            });
            // Same verdict / error surface as `Ruleset::can_counteract_scoped`:
            // a prompting guard is `GuardPrompted`, a trap propagates (callers
            // fold it with `unwrap_or(false)`), a code is `!= 0`.
            match res {
                Ok(CallOut::Code(v)) => Ok(v != 0),
                Ok(_) => Ok(true),
                Err(HostErr::NeedInput) => Err(RuleError::GuardPrompted),
                Err(HostErr::Trap(m)) => Err(RuleError::Trap(m)),
            }
        })
    }


    fn cant_play(
        &self,
        world: &Run,
        card: i32,
        player_id: i32,
    ) -> Result<Option<Msg>, RuleError> {
        let info = self
            .index
            .card(card)
            .ok_or_else(|| RuleError::Trap(format!("bad card handle {card}")))?;
        let Some(entry) = info.entry(OnKind::Play, None) else {
            return Ok(None);
        };
        let pre = self.index.pre(card, entry);
        // Scope only when a condition exists (`pre == None` never reads it) --
        // the `cant_play` hot path (ai_step / view extras ask it per hand card).
        let scope;
        let scope: Option<&game_rules::cond_pre::WindowScope> = if pre.is_some() {
            scope = game_rules::cond_pre::window_scope(&game_rules::cond_pre::fill_window_ambient(
                world, player_id,
            ));
            Some(&scope)
        } else {
            None
        };
        let cand = game_rules::cond_pre::fill_candidate(world, player_id, &info.id, false);
        game_rules::cond_pre::admits_gate(
            pre,
            scope,
            &cand,
            || Msg::new("err.play_pre"),
            || {
                let state = HostState::new(self.index.clone(), world.clone(), vec![], 0);
                let (res, _state, _fuel) = game_rules::inline::with_no_inline(|| {
                    run_on(state, card, entry, export::OP_GUARD, player_id, true, DEFAULT_FUEL)
                });
                match res {
                    Ok(CallOut::Msg(m)) => Ok(m),
                    Ok(CallOut::Code(0)) => Ok(None),
                    Ok(CallOut::Code(_)) => Ok(None),
                    Err(HostErr::NeedInput) => Err(RuleError::GuardPrompted),
                    Err(HostErr::Trap(m)) => Err(RuleError::Trap(m)),
                }
            },
        )
    }
}

/// The bot's simulation rules: [`game_rules::RulesBridge`] over the linked
/// table. Drop-in for `WasmRules` (same `CardRules` impl), no sandbox.
pub type NativeRules = RulesBridge<NativeModules>;

/// The native counterpart of `WasmRules::load_dir`: always available, no
/// `dist/cards` needed.
pub fn native_rules(data: Arc<game_core::data::GameData>) -> NativeRules {
    NativeRules::new(NativeModules::new(), data)
}

// `MAX_NESTING` is used by hostfns via the nested call path; re-export so
// tests can assert the same bound.
pub use game_rules::MAX_NESTING as MAX_PLAY_NESTING;