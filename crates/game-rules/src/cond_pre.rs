//! G0/G2 of `docs/GUARDS.md`: the serialized **condition** per guarded entry,
//! compiled once at ruleset build and evaluated natively before the wasm guard
//! is instantiated.
//!
//! Distinct layers (user ruling 2026-10-07): **category / condition / guard**
//! each reject what only they can; nothing is rejected twice. The condition is
//! part of the decision, so every path that asks a guard goes through one
//! [`admits`] / [`admits_bool`] / [`admits_gate`] entry point.
//!
//! Wire form: [`CompiledPre::blob`] is `rules_cond::Cond::to_bytes(false)` (the
//! lean postcard AST). The host compiles from source (`compile`, the `cel`
//! parser); the browser glue loads the blob with `rules-cond`'s `runtime-only`
//! feature and never ships the parser.

use rules_cond::{CandidateCtx, ChainLink, Cond, MoveSnap, PlayerSnap, TileSnap, WindowCtx, WindowScope};

use crate::world::CardWorld;

/// One guarded entry's compiled condition plus its lean wire bytes.
#[derive(Clone, Debug)]
pub struct CompiledPre {
    /// Authoring source (tooling / `precheck`); `None` when loaded from bytes.
    pub src: Option<String>,
    /// `Cond::to_bytes(false)` -- what the runtime-only path loads.
    pub blob: Vec<u8>,
    pub cond: Cond,
}

impl CompiledPre {
    /// Host path: parse + lint + rewrite once (docs/GUARDS.md §4.3). Fail-closed:
    /// a parse / unknown-var / float error is a build error, never "treat as
    /// true" -- the clauses the condition carries are gone from the guard.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn compile(src: &str) -> Result<Self, rules_cond::CondError> {
        let cond = rules_cond::compile(src)?;
        let blob = cond.to_bytes(false);
        Ok(Self {
            src: Some(src.to_string()),
            blob,
            cond,
        })
    }

    /// Runtime-only path (browser glue): decode a lean blob and evaluate. No
    /// parser, no `compile` feature.
    pub fn from_bytes(blob: &[u8]) -> Result<Self, rules_cond::CondError> {
        let cond = Cond::from_bytes(blob)?;
        Ok(Self {
            src: None,
            blob: blob.to_vec(),
            cond,
        })
    }

    pub fn eval(&self, win: &WindowCtx, cand: &CandidateCtx) -> bool {
        self.cond.eval(win, cand)
    }
}

/// Measurement counters (same shape as [`crate::bot_cost`]). Compiled out
/// unless the `bot-cost` feature is on; G3/G4 use these to measure how much the
/// conditions skip. Always-on twin counters are cheap `AtomicU64` bumps and
/// live in [`guard_cost`] so a test can read them without the feature.
pub mod guard_cost {
    use std::sync::atomic::AtomicU64;

    /// Admits calls that evaluated a condition.
    pub static COND_EVALS: AtomicU64 = AtomicU64::new(0);
    /// Condition rejected: the wasm guard was skipped (a `can_counteract`
    /// probe / hook guard / play gate never instantiated).
    pub static SKIPPED_BY_CONDITION: AtomicU64 = AtomicU64::new(0);
    /// Condition passed (or absent) and the guard was asked.
    pub static GUARD_ASKED: AtomicU64 = AtomicU64::new(0);
    /// ns spent in `Cond::eval`.
    pub static COND_EVAL_NS: AtomicU64 = AtomicU64::new(0);

    pub fn reset() {
        use std::sync::atomic::Ordering::Relaxed;
        COND_EVALS.store(0, Relaxed);
        SKIPPED_BY_CONDITION.store(0, Relaxed);
        GUARD_ASKED.store(0, Relaxed);
        COND_EVAL_NS.store(0, Relaxed);
    }
}

/// G3 (docs/GUARDS.md §5.1): the migration audit. On every probe the host
/// evaluates the card's pre-migration `legacy_*` guard and
/// `kind ∧ pre(ctx) ∧ guard(ctx)` and **panics** with card id + trigger dump on
/// any mismatch (equivalence, both directions). Compiled out unless the
/// `guard-audit` feature is on; once a card is clean its `legacy_*` copy is
/// deleted and the check is a no-op for it.
///
/// `legacy == None` means the entry has no legacy copy (`has_legacy == false`)
/// and the check is skipped. `trigger` is a one-line dump of the window.
#[cfg(feature = "guard-audit")]
pub fn legacy_audit(
    card: &str,
    entry: i32,
    legacy: Option<bool>,
    migrated: bool,
    trigger: &str,
) {
    let Some(legacy) = legacy else {
        return;
    };
    if legacy != migrated {
        panic!(
            "GUARDS G3 audit mismatch: card {card:?} entry {entry}\n\
             trigger: {trigger}\n\
             legacy(ctx) = {legacy}, category ∧ pre(ctx) ∧ guard(ctx) = {migrated}\n\
             The two must be equivalent (docs/GUARDS.md §5.1). Either the \
             condition drops a clause the legacy guard kept, or the residual \
             guard / condition rejects what the legacy guard admitted."
        );
    }
}

/// No-op when the `guard-audit` feature is off (the production shape).
#[cfg(not(feature = "guard-audit"))]
#[inline]
pub fn legacy_audit(
    _card: &str,
    _entry: i32,
    _legacy: Option<bool>,
    _migrated: bool,
    _trigger: &str,
) {
}

#[inline]
fn bump(stat: &std::sync::atomic::AtomicU64) {
    stat.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Evaluate the condition half of [`admits`]. `true` when the entry has no
/// condition or its condition accepts the (window, candidate) pair.
#[inline]
pub fn condition_allows(pre: Option<&CompiledPre>, scope: &WindowScope, cand: &CandidateCtx) -> bool {
    let Some(pre) = pre else {
        return true;
    };
    bump(&guard_cost::COND_EVALS);
    #[cfg(feature = "bot-cost")]
    let t0 = std::time::Instant::now();
    let ok = scope.eval(&pre.cond, cand);
    #[cfg(feature = "bot-cost")]
    guard_cost::COND_EVAL_NS
        .fetch_add(t0.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    if !ok {
        bump(&guard_cost::SKIPPED_BY_CONDITION);
    }
    ok
}

/// The one shared gate: `admits = cond(window, candidate) && guard(...)`.
/// Every path that asks a guard goes through this -- counteraction offers,
/// hook dispatch, play gates / skill gates, the view's `playable` flags, the
/// bot paths, `rules-native`. No caller may call a guard directly: with
/// clauses removed from the guards (G4), skipping the condition would admit
/// what it should reject.
///
/// `guard` returns `Ok(true)` = the residual guard admits. A rejecting
/// condition short-circuits before the wasm guard is ever instantiated.
#[inline]
pub fn admits<E, F: FnOnce() -> Result<bool, E>>(
    pre: Option<&CompiledPre>,
    scope: &WindowScope,
    cand: &CandidateCtx,
    guard: F,
) -> Result<bool, E> {
    if !condition_allows(pre, scope, cand) {
        return Ok(false);
    }
    bump(&guard_cost::GUARD_ASKED);
    let ok = guard()?;
    Ok(ok)
}

/// [`admits`] for the `On::Play` gate shape (`Ok(None)` = playable,
/// `Ok(Some(why))` = blocked). A rejecting condition **is** a block -- the
/// entry is part of the decision -- and the gate is skipped. `blocked`
/// supplies the reason the caller shows (so the UI/i18n stays in the caller's
/// hands).
#[inline]
pub fn admits_gate<T, E, F: FnOnce() -> Result<Option<T>, E>>(
    pre: Option<&CompiledPre>,
    scope: &WindowScope,
    cand: &CandidateCtx,
    blocked: impl FnOnce() -> T,
    gate: F,
) -> Result<Option<T>, E> {
    if !condition_allows(pre, scope, cand) {
        bump(&guard_cost::GUARD_ASKED);
        return Ok(Some(blocked()));
    }
    bump(&guard_cost::GUARD_ASKED);
    let why = gate()?;
    Ok(why)
}

/// `admits` for a pure "does this entry apply" question with no residual
/// guard at all (`pre` only). Used when the guard was deleted (`None`) in G4.
#[inline]
pub fn admits_pre(pre: Option<&CompiledPre>, scope: &WindowScope, cand: &CandidateCtx) -> bool {
    if !condition_allows(pre, scope, cand) {
        return false;
    }
    bump(&guard_cost::GUARD_ASKED);
    true
}

// ---------------------------------------------------------------------------
// Window / candidate snapshots (docs/GUARDS.md §4.2)
// ---------------------------------------------------------------------------

/// Stable int id for a string identity (character / band / card id). Not the
/// data-table index -- a name-stable hash so a condition's `character_is(p, …)`
/// literal survives a data reshuffle. Collisions are astronomically unlikely
/// at 64 bits and the schema is int-only.
pub fn id_of(name: &str) -> i64 {
    // FNV-1a 64.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h as i64
}

/// Build the window snapshot once per trigger / chain window and reuse it
/// across every candidate probe in that window ([`WindowScope`]). This is
/// where the G1 savings live: ~48 900 counteract probes share ~250 windows.
pub fn fill_window<W: CardWorld>(world: &W) -> WindowCtx {
    let t = world.trigger();
    let mut tile_ids = std::collections::BTreeMap::new();
    // Best-effort: the schema only needs the names a condition actually
    // mentions, and `tile_named` resolves at load. Register the common CiRCLE
    // / shop aliases used by the §2.1 sample conditions; anything else
    // resolves to 0 (the "unknown tile" sentinel the evaluator already uses).
    for name in ["circle", "shop", "ring", "agent", "liveHouse"] {
        let id = world.tile_named(name);
        if id >= 0 {
            tile_ids.insert(name.to_string(), id as i64);
        }
    }
    let players: Vec<PlayerSnap> = (0..world.player_count())
        .map(|p| PlayerSnap {
            money: world.money(p) as i64,
            fire: world.fire(p) as i64,
            crystals: world.band_crystals(p) as i64,
            hand: world.hand_size(p) as i64,
            pos: world.player_pos(p) as i64,
            out: world.player_out(p) as i64,
            stay: world.stay_of(p) as i64,
            stun: world.stun_of(p) as i64,
            exile: world.state_get(p, game_core::state::key::EXILE) as i64,
            no_hand: world.state_get(p, game_core::state::key::NO_HAND) as i64,
            character: id_of(&world.character_skill_id(p).unwrap_or_default()),
            band: id_of(&world.band_skill_id(p).unwrap_or_default()),
            tiles: world.owned_count(p) as i64,
        })
        .collect();

    let chain: Vec<ChainLink> = t
        .effects
        .iter()
        .map(|e| ChainLink {
            kind: crate::TriggerKind::from_str(e.kind) as i64,
            from: e.from as i64,
            hits: e.target as i64,
        })
        .collect();

    let mv = MoveSnap {
        // Match the guest's `trigger::move_roll()` (`card-sdk/ctx.rs`): a
        // negative face is the pre-cast sentinel (`value = -1` on the `roll`
        // pre-trigger) and reads as `null`, so `move.roll != null` means "a
        // real face exists" exactly as the guards wrote it.
        roll: t.move_roll.filter(|&r| r >= 0).map(|r| r as i64),
        kind: t.move_kind.map(|k| k as i64),
        remaining: t.move_remaining as i64,
    };

    let tile = TileSnap {
        id: t.tile as i64,
        owner: if t.tile >= 0 { world.tile_owner(t.tile) as i64 } else { -1 },
        houses: if t.tile >= 0 { world.houses_of(t.tile) as i64 } else { 0 },
        mortgaged: if t.tile >= 0 { world.mortgaged_of(t.tile) as i64 } else { 0 },
        price: if t.tile >= 0 { world.tile_price(t.tile) as i64 } else { 0 },
    };

    WindowCtx {
        kind: t.kind as i64,
        actor: t.player_id as i64,
        target: t.target as i64,
        tile,
        value: t.value as i64,
        step: t.step as i64,
        by: t.by_card.map(|b| b as i64).unwrap_or(-1),
        pay_is_rent: t.pay_is_rent,
        mv,
        roll_source: t.roll_source as i64,
        // `abnormal` is the AbnormalGate window, not "this link was negated".
        // There is no `t.Abnormal` bool on `Trigger`; the closest signal is the
        // trigger kind itself.
        abnormal: matches!(t.kind, crate::TriggerKind::Abnormal),
        chain,
        turn_player: world.turn_player() as i64,
        turn_key: world.turn_key() as i64,
        players,
        tile_ids,
    }
}

/// Play-gate window (docs/GUARDS.md §4.4 item 3): `cant_play` / skill gates have
/// **no** `Trigger` to source a window from -- same schema with `kind` absent
/// (`TriggerKind::None`). Everything else is ambient world state, actor = the
/// player being asked. Built once per ask; reused across the cards of one view.
pub fn fill_window_ambient<W: CardWorld>(world: &W, player_id: i32) -> WindowCtx {
    let mut tile_ids = std::collections::BTreeMap::new();
    for name in ["circle", "shop", "ring", "agent", "liveHouse"] {
        let id = world.tile_named(name);
        if id >= 0 {
            tile_ids.insert(name.to_string(), id as i64);
        }
    }
    let players: Vec<PlayerSnap> = (0..world.player_count())
        .map(|p| PlayerSnap {
            money: world.money(p) as i64,
            fire: world.fire(p) as i64,
            crystals: world.band_crystals(p) as i64,
            hand: world.hand_size(p) as i64,
            pos: world.player_pos(p) as i64,
            out: world.player_out(p) as i64,
            stay: world.stay_of(p) as i64,
            stun: world.stun_of(p) as i64,
            exile: world.state_get(p, game_core::state::key::EXILE) as i64,
            no_hand: world.state_get(p, game_core::state::key::NO_HAND) as i64,
            character: id_of(&world.character_skill_id(p).unwrap_or_default()),
            band: id_of(&world.band_skill_id(p).unwrap_or_default()),
            tiles: world.owned_count(p) as i64,
        })
        .collect();
    WindowCtx {
        kind: 0,
        actor: player_id as i64,
        target: -1,
        tile: TileSnap {
            id: world.player_pos(player_id) as i64,
            owner: {
                let t = world.player_pos(player_id);
                if t >= 0 { world.tile_owner(t) as i64 } else { -1 }
            },
            houses: 0,
            mortgaged: 0,
            price: 0,
        },
        value: 0,
        step: 0,
        by: -1,
        pay_is_rent: false,
        mv: MoveSnap::default(),
        roll_source: 0,
        abnormal: false,
        chain: Vec::new(),
        turn_player: world.turn_player() as i64,
        turn_key: world.turn_key() as i64,
        players,
        tile_ids,
    }
}

/// Per-candidate overlay: the card + seat being probed. Built per (card, seat)
/// inside a window -- much cheaper than a window, so a fresh value is fine.
pub fn fill_candidate<W: CardWorld>(
    world: &W,
    owner: i32,
    card: &str,
    placed: bool,
) -> CandidateCtx {
    let p = owner;
    let mut cand = CandidateCtx {
        owner: owner as i64,
        owner_money: world.money(p) as i64,
        owner_fire: world.fire(p) as i64,
        owner_crystals: world.band_crystals(p) as i64,
        owner_hand: world.hand_size(p) as i64,
        owner_pos: world.player_pos(p) as i64,
        owner_out: world.player_out(p) as i64,
        owner_stay: world.stay_of(p) as i64,
        owner_stun: world.stun_of(p) as i64,
        owner_exile: world.state_get(p, game_core::state::key::EXILE) as i64,
        owner_no_hand: world.state_get(p, game_core::state::key::NO_HAND) as i64,
        owner_character: id_of(&world.character_skill_id(p).unwrap_or_default()),
        owner_band: id_of(&world.band_skill_id(p).unwrap_or_default()),
        owner_tiles: world.owned_count(p) as i64,
        card_id: id_of(card),
        card_placed: placed,
        card_cp: world.card_crystals(p, card) as i64,
        slots: Default::default(),
        toks: Default::default(),
        blocked_bands: Vec::new(),
    };
    fill_candidate_extras(world, owner, &mut cand, true);
    cand
}

/// Slot names any condition reads (`slot('…')`). Each is a bare player-state
/// key -- the same one `ctx::slot(player_id, name)` reads via
/// `CardWorld::slot`, **not** a `slot:`-prefixed alias (that mismatch made
/// `slot('lastWalk') > 0` read 0 and silently close a counteraction window).
/// Extend this list when a new `slot('…')` shows up in a `pre`.
const SLOT_NAMES: &[&str] = &["asUsualTurn", "lastWalk"];

/// Fill the per-candidate slot / token tables a condition may read
/// (`slot('asUsualTurn')`, `tok(kind)`). Only worth the copy when the
/// condition actually names them -- the caller can pass `false` otherwise.
pub fn fill_candidate_extras<W: CardWorld>(
    world: &W,
    owner: i32,
    cand: &mut CandidateCtx,
    with_slots: bool,
) {
    if with_slots {
        // The schema's `slot(name)` is per-card latch state keyed by name.
        // The host stores latches as bare player-state keys -- the same key
        // `ctx::slot(player_id, name)` reads through `CardWorld::slot`.
        for name in SLOT_NAMES {
            let v = world.state_get(owner, name);
            cand.slots.insert(name.to_string(), v as i64);
        }
    }
}

/// A window scope helper: build once, evaluate every candidate against it.
pub fn window_scope(win: &WindowCtx) -> WindowScope {
    WindowScope::new(win)
}