//! `game-core`'s `CardRules` over the WASM card modules.
//!
//! The engine's routines and a card effect use the same replay model, so the
//! bridge is small: it runs the module against a copy of the world, and when the
//! module blocks on a prompt it raises that prompt as an engine `Ask`. The engine
//! halts the routine, collects the answer, and re-runs it -- which re-runs the
//! module too, rebuilding the answer list line by line as the prompts come up
//! again. Dice, events and prompts remain deterministic because the world copy
//! carries the RNG (see the replay note in `host.rs`).
//!
//! ```text
//! routine -> rules.play -> module run -> NeedInput(p)
//!                ^                        |
//!                |  cx.ask(p) = Halt ------+  (answer arrives, routine re-runs)
//! ```

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{Ask, CardRules, Dest, Flow, Halt, PlayCtx, Trigger as CoreTrigger};
use game_core::msg::Msg;

#[cfg(feature = "sandbox")]
use crate::host::Ruleset;
use crate::host::{
    Call, CardModules, DeclareOut, HostRequest, Outcome, Prompt, PromptOption, RuleError,
};
use crate::inline::InlineHost;
use crate::world::ROLL_ROOT_NONE;
use crate::world::{CardWorld, Trigger, TriggerBuy, TriggerMove, TriggerPay};
use crate::PromptKind;
use crate::{CardPile, TriggerKind};
use game_core::engine::CounteractRecord;
use rules_cond::WindowScope;

// ------------------------------------------------------- surface resolution

/// The card an instance currently runs: its effective card when a stand-in has
/// marked it, else its own. Every dispatch and instance read resolves through
/// this one function (`docs/rulebook/spec/abi/G05-placed-card-press.md` §6.2).
fn surface_card(w: &game_core::engine::World, uid: i32, raw: &str) -> String {
    w.effective_card_at(uid).unwrap_or_else(|| raw.to_string())
}

/// Is the extra instance `uid` suppressed for this raise?
///
/// The tag `tag::SUPPRESS_TILE_EFFECTS` names a tile; both the trigger's
/// raise-time move snapshot and the live plan are read, because a counter
/// writes the plan *after* the snapshot its own raise took -- the plan is what
/// makes a tag armed inside the `tileEffectBefore` round visible to the hook
/// round of the same window. Only an *extra* placed on the very tile is
/// suppressed; the tile's own `tile:` rule always runs.
fn extra_suppressed(
    w: &game_core::engine::World,
    tile: i32,
    mv_tags: Option<&[(String, i32)]>,
    uid: i32,
) -> bool {
    let tagged = |tags: &[(String, i32)]| {
        tags.iter()
            .any(|(k, v)| k == card_sdk::abi::tag::SUPPRESS_TILE_EFFECTS && *v == tile)
    };
    let set = mv_tags.is_some_and(tagged) || tagged(&w.turn.plan.tags);
    if !set {
        return false;
    }
    // Same governing-tile read as `field_hears`: a player placement answers
    // through the tile standing at its slot, not the `tile` it was placed on
    // (a swap moves that set with the tile); a board instance keeps its own
    // tile.
    w.field_by_uid(uid)
        .is_some_and(|f| on_tile(w, f, tile) && !f.card.starts_with("tile:"))
}

/// Kinds that are a happening **of one tile** and that a listener only hears
/// from *that* tile: the tile's own effect window and the house / owner /
/// build writes. [`field_hears`] narrows these and nothing else.
///
/// Everything else (`pass*`, `card`, `pay*`, a settle, a counteraction, …) is
/// a happening of the game, raised with whichever tile it concerns, and a
/// listener anywhere may answer it after gating itself on `trigger.tile` /
/// `trigger.position` -- 练习室里的风暴 (2) charges from a distance, the door
/// surcharge taxes a settle across the board, marina answers an extra on
/// another tile. Narrowing those would drop the listener, whose guard reads
/// the trigger's own tile precisely because its tile is not it.
fn tile_local_kind(kind: TriggerKind) -> bool {
    matches!(
        kind,
        TriggerKind::TileEffectBefore
            | TriggerKind::HouseAdded
            | TriggerKind::HouseRemoved
            | TriggerKind::HouseAdd
            | TriggerKind::HouseRemove
            | TriggerKind::OwnerSet
            | TriggerKind::BuildBefore
            | TriggerKind::BuildAfter
    )
}

/// Does a **placed** field instance hear a trigger for tile `t.tile`? A card
/// lying on a tile only hears its own tile (a swap moves that set with the
/// tile); a card with its owner (`tile < 0`) and a tile-less trigger still
/// reach everything, as before.
fn field_hears(
    w: &game_core::engine::World,
    tile: i32,
    position: i32,
    uid: i32,
    kind: TriggerKind,
) -> bool {
    if tile < 0 {
        return true;
    }
    // `tileResolved` is answered by **slot**: after smile_parade (2) moved
    // itself, the card's governing tile is the swapped-in one, so its own
    // settle no longer matches on the tile.
    if kind == TriggerKind::TileResolved {
        return position < 0
            || w.field_by_uid(uid).is_some_and(|f| {
                f.tile < 0
                    || if f.owner == game_core::state::BOARD_OWNER {
                        f.tile == tile
                    } else {
                        f.position() == position
                    }
            });
    }
    if !tile_local_kind(kind) {
        return true;
    }
    w.field_by_uid(uid)
        .is_some_and(|f| f.tile < 0 || on_tile(w, f, tile))
}

/// Is field instance `f` **on** `tile`? A board instance is anchored to the
/// tile identity it was put on (it keeps `position = -1`, so `f.position()`
/// is `f.tile` and would resolve through the wrong slot after a swap); a
/// player instance answers through the tile standing at its slot.
///
/// A `tile = -1` instance (owner-held / board-wide) is on no tile: callers
/// that want it to reach every tile test `f.tile < 0` themselves.
fn on_tile(w: &game_core::engine::World, f: &game_core::state::FieldCard, tile: i32) -> bool {
    if f.tile < 0 {
        return false;
    }
    if f.owner == game_core::state::BOARD_OWNER {
        return f.tile == tile;
    }
    w.tile_standing_at(f.position()) == tile
}

// ------------------------------------------------- silent-entry debug guard

thread_local! {
    /// Violations of the silent-entry contract: a `On::silent` body that
    /// produced a visible effect (see [`RulesBridge::check_silent_body`]).
    /// Tests read this via [`take_silent_visible`]; debug builds also
    /// `debug_assert!` for shipped (non-`TEST:`) card ids.
    static SILENT_VISIBLE: std::cell::RefCell<Vec<(String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn record_silent_visible(card_id: &str, what: &str) {
    SILENT_VISIBLE.with(|v| v.borrow_mut().push((card_id.to_string(), what.to_string())));
}

/// Fold two declarative flash policies across the sibling entries of one
/// drive: `Never` if any entry says `Never`; else `Always` if any says
/// `Always`; else `Auto`. A multi-entry drive (`run_hook` announces once per
/// matching entry, then `commit_after` takes **one** flash decision for one
/// header) must not let a trailing `Auto` drop an earlier `Always`, nor a
/// later `Always` lift an earlier `Never`. Matches the declarative half of
/// the precedence `commit_after` applies at the flash point
/// (`Never > ctx::no_flash > ctx::flash / Always > Auto`).
///
/// Why `Never` wins here while [`fold_header`] lets `Visible` win: the flash
/// is one shared animation, so the **strongest opinion** (`Never` = "do not
/// cue") must silence the whole drive; the header is one shared clause name,
/// so the **strongest demand** (`Visible` = "show my clause") must keep it.
/// The asymmetry is deliberate -- do not "fix" one fold to match the other.
const fn fold_flash(a: card_sdk::abi::Flash, b: card_sdk::abi::Flash) -> card_sdk::abi::Flash {
    use card_sdk::abi::Flash::{Always, Auto, Never};
    match (a, b) {
        (Never, _) | (_, Never) => Never,
        (Always, _) | (_, Always) => Always,
        _ => Auto,
    }
}

/// Fold two entry header modes across the sibling entries of one drive:
/// `Visible` if any entry says `Visible`; else `QuietWhenEmpty` if any says
/// `QuietWhenEmpty`; else `Silent`. Same one-header-per-drive constraint as
/// [`fold_flash`] (`run_hook` announces once per matching entry, then
/// `commit_after` takes **one** header), but the opposite polarity: a
/// trailing `Silent` / `QuietWhenEmpty` sibling must not retract a leading
/// `Visible` clause, and a trailing `Visible` must not cancel a leading
/// `QuietWhenEmpty` -- in either order. See [`fold_flash`] for why the two
/// folds disagree on which end wins.
const fn fold_header(
    a: card_sdk::abi::Presentation,
    b: card_sdk::abi::Presentation,
) -> card_sdk::abi::Presentation {
    use card_sdk::abi::Presentation::{QuietWhenEmpty, Silent, Visible};
    match (a, b) {
        (Visible, _) | (_, Visible) => Visible,
        (QuietWhenEmpty, _) | (_, QuietWhenEmpty) => QuietWhenEmpty,
        _ => Silent,
    }
}

/// Is this host request a **visible effect** a silent entry must not produce?
/// `Some(what)` for the pay/move/draw/build class; `None` for setup (gates,
/// targeting, linger) and for nested activations (`play_card` / `invoke_skill`
/// are not host requests -- they announce on their own).
/// Does this request touch a field row? The drive shows the body's own rows
/// for these (see the `NeedHost` arm): a body may place a card and then act on
/// it, and the routine must see it.
fn zone_req(req: &HostRequest) -> bool {
    matches!(
        req,
        HostRequest::Leave { .. }
            | HostRequest::Flip { .. }
            | HostRequest::Attach { .. }
            | HostRequest::Detach { .. }
            | HostRequest::Rehome { .. }
            | HostRequest::PlayHand { .. }
    )
}

/// Requests whose host effect writes a **pile** (a card lands in a hand, the
/// draw pile or the discard). The pass top rewinds live's piles to the drive
/// start, so the pile half of such a request is re-supplied from its
/// `PileCheckpoint` rather than re-applied by the replay -- a leave into the
/// deck that is consumed by a re-run would otherwise be lost. Dest wire codes:
/// 0 graveyard, 1 hand, 4 deck top, 5 deck bottom, 6 deck random; 2 (field)
/// keeps the card in play and 3 (banished) is out of the game entirely.
fn writes_piles(req: &HostRequest) -> bool {
    fn pile(dest: i32) -> bool {
        matches!(dest, 0 | 1 | 4 | 5 | 6)
    }
    match req {
        HostRequest::Draw { .. }
        | HostRequest::InspectionBegin { .. }
        | HostRequest::InspectionEnd { .. }
        | HostRequest::PlayHand { .. } => true,
        HostRequest::Leave { destination, .. } | HostRequest::Detach { destination, .. } => {
            pile(*destination)
        }
        _ => false,
    }
}

fn visible_host_request(req: &HostRequest) -> Option<&'static str> {
    match req {
        HostRequest::Pay { .. }
        | HostRequest::PayCommand { .. }
        | HostRequest::PayTotal { .. }
        | HostRequest::PayRent { .. }
        | HostRequest::ChargeRent { .. } => Some("pay/gain/transfer"),
        HostRequest::Move { .. }
        | HostRequest::Teleport { .. }
        | HostRequest::TeleportAs { .. } => Some("move"),
        HostRequest::Draw { .. } | HostRequest::DrawEvent { .. } => Some("draw"),
        HostRequest::DrawEventAs { .. }
        | HostRequest::EventDefer { .. }
        | HostRequest::WorldSnapshot
        | HostRequest::WorldRestore { .. }
        | HostRequest::WorldSnapshotDrop { .. }
        | HostRequest::DiscardNextFx { .. } => None,
        HostRequest::InspectionBegin { .. } | HostRequest::InspectionEnd { .. } => {
            Some("inspection")
        }
        HostRequest::RevealPile { .. } | HostRequest::RevealCard { .. } => Some("reveal"),
        HostRequest::Build { .. }
        | HostRequest::BuildAs { .. }
        | HostRequest::OfferBuild { .. }
        | HostRequest::OfferBuildOne { .. }
        | HostRequest::Mortgage { .. }
        | HostRequest::OfferBuy { .. }
        | HostRequest::OfferForceBuy { .. }
        | HostRequest::Buy { .. }
        | HostRequest::Acquire { .. }
        | HostRequest::Auction { .. }
        | HostRequest::AgentOffer { .. } => Some("buy/build"),
        HostRequest::TileWrite { .. } => Some("house"),
        HostRequest::SettleAt { .. } => Some("settle"),
        HostRequest::Roll { .. }
        | HostRequest::RollRecast { .. }
        | HostRequest::RollMove { .. } => Some("roll"),
        _ => None,
    }
}

/// Take the recorded silent-entry violations (`card id`, `what it produced`)
/// since the last call. Used by tests that pin the debug guard.
pub fn take_silent_visible() -> Vec<(String, String)> {
    SILENT_VISIBLE.with(|v| std::mem::take(&mut *v.borrow_mut()))
}

/// Declared static properties of one card, from a [`Run`]'s snapshot.
fn props_of(
    props: &std::collections::HashMap<String, std::collections::BTreeMap<String, i32>>,
    id: &str,
) -> std::collections::BTreeMap<String, i32> {
    props.get(id).cloned().unwrap_or_default()
}

/// Move `counter_max.<name>` placement props out of a prop map and return
/// `(name, cap)` pairs. They seed [`game_core::state::FieldCard::counter_max`]
/// instead of landing in `props`, so no reader mistakes them for a prop.
fn split_counter_max(props: &mut std::collections::BTreeMap<String, i32>) -> Vec<(String, i32)> {
    let prefix = card_sdk::abi::prop::COUNTER_MAX_PREFIX;
    let keys: Vec<String> = props
        .keys()
        .filter(|k| k.starts_with(prefix))
        .cloned()
        .collect();
    keys.into_iter()
        .filter_map(|k| props.remove(&k).map(|v| (k[prefix.len()..].to_string(), v)))
        .collect()
}

/// A buffered 「效果适用」 header, posted lazily (Ruling 2a) just before the
/// drive's first visible outcome.
#[derive(Clone)]
pub struct PendingHeader {
    kind: String,
    owner: i32,
    msg: Msg,
    card: String,
    target: i32,
    tile: i32,
    /// Field instance (`FieldCard.uid`) for the slot pulse; -1 when the card
    /// is not on a field (hand play, skill, event).
    uid: i32,
}

/// One run of a card module against a copy of the match world.
/// Dest fates the module returns via `set_dest` (see [`DEST_GRAVEYARD`] /
/// [`DEST_FIELD`] / [`DEST_UNSET`]).
const DEST_GRAVEYARD: i32 = 0;
const DEST_FIELD: i32 = 2;
/// The run never named a dest. Distinct from [`DEST_GRAVEYARD`] on purpose: a
/// play with no opinion still lands in the discard (`dest_from`'s wildcard),
/// but a field effect with no opinion must leave its card where it is.
const DEST_UNSET: i32 = -1;

/// A counter-war is bounded by hands shrinking as cards declare; 16 is
/// plenty for a legal exchange. A deeper nest is a runaway net.
const MAX_COUNTERACT_DEPTH: u32 = 16;

/// Hard cap on the offers one seat gets in a single visit of the ask cycle
/// (ruling 2026-10-07: a seat keeps the floor until it passes or runs out of
/// eligible counteractions). The offer set shrinks as cards are declared, so a
/// visit terminates on its own; the cap is a runaway guard, not a design limit.
/// TODO(规则书): the book states no per-visit bound.
const MAX_COUNTERACT_PER_VISIT: u32 = 16;

/// Piles and randomness at a host boundary. Replaying a card starts from the
/// pre-draw piles, then adopts each completed request at its original
/// statement. A discard/redraw effect cannot discard its freshly drawn hand.
///
/// A **snapshot** (deep copy at construction): the checkpoint outlives the
/// mutations that follow the boundary, so it cannot share the live handle.
///
/// The event tail (`recent` / `next_event`) is **not** part of the checkpoint:
/// during a drive every event is allocated from the live world's single
/// counter (the guest copy runs with `skip_log` and posts write-through).
/// Copying the tail here used to reset that counter whenever a host request
/// made the guest's pile writes visible, so the next live log reused an id
/// the guest had already spent (two `text` events with the same id).
#[derive(Clone)]
struct PileCheckpoint {
    world: Arc<game_core::engine::World>,
}
impl PileCheckpoint {
    fn new(world: game_core::engine::World) -> Self {
        Self {
            world: Arc::new(world),
        }
    }
    fn apply(&self, world: &mut game_core::engine::World) {
        // The piles come back; the **RNG stream does not**. Rewinding it here
        // would pin every pass of the drive to the checkpoint's stream, so a
        // reroll pushed later re-drew the face the first one did and a
        // `while roll < N` reroll loop could never leave its first face
        // (`sports_talent` re-rolled a pinned 1 forever). The stream stays
        // monotonic on live; a re-*applied* request still redraws its own face
        // because step 1 restores that request's recorded `condition`, RNG
        // included (§6.3.2).
        world.hidden = self.world.hidden.clone();
    }
}

/// A `Replace` re-runs a request the body recomputed: put the **live** world
/// back on the entry's pre-request `condition` first, so the fresh application
/// is the only one (design G10 §6.3.5). The guest copy was restored already at
/// the pause site; this is the live side of the same undo.
///
/// Two drifts do **not** rewind. A `Move` states its destination absolutely
/// (below). An **identity** drift (the body now makes a different request
/// kind, LOY-TSUGU-01) is the same re-run-after-a-completed-effect shape: the
/// swallowed answer's work already landed -- an accepted buy offer ran the buy
/// body -- so the new request applies on top of it and the tile reads as owned
/// (`rokka_first_deed_contract`).
fn restore_condition(
    cx: &mut PlayCtx,
    log: &game_core::engine::GuestLog,
    start: game_core::engine::JournalStart,
    req: &HostRequest,
) {
    let game_core::engine::JournalStart::Replace(slot) = start else {
        return;
    };
    let entry = log
        .last
        .get(&slot)
        .or_else(|| log.journal.iter().find(|e| e.slot == slot));
    if let Some(e) = entry {
        // A move is not rewound: its plan states the new destination, the
        // re-run overwrites the position, and whatever its old landing opened
        // (a settle, the agent's deal) is what the recomputed read saw.
        if matches!(e.request, game_core::engine::JournalRequest::Move { .. }) {
            return;
        }
        let mirrored = game_core::engine::JournalRequest::from(req);
        if std::mem::discriminant(&e.request) != std::mem::discriminant(&mirrored) {
            return;
        }
        // A `latch:` key is a decision the body froze and the condition
        // predates it; the undo must not unfreeze it, or the recomputed run
        // takes the fresh branch against the very state the latch exists to
        // pin (`Run::restore_from_world` is the copy's half of the same rule).
        let latched = cx.take_latches();
        // The event id counter is monotonic like the RNG stream: the
        // condition predates this pass's write-through lines, and letting it
        // rewind makes the re-posted lines reuse ids a reader has already
        // passed (`keys_since(mark)` loses them).
        let next_event = cx.world().next_event;
        // The posted stream is append-only too: the condition's copy predates
        // this pass's write-through lines, and a restore that took it back
        // removed the lines the next re-run's `relog_event` expects to find.
        let recent = cx.world().recent.clone();
        cx.swap_world((*e.condition).clone());
        cx.world_mut().next_event = next_event;
        cx.world_mut().recent = recent;
        cx.put_latches(latched);
    }
}

#[derive(Clone)]
pub struct Run {
    /// The world this run reads and writes. A [`SharedWorld`] (fix B): the
    /// drive's per-iteration copy and every pure check share the live world
    /// handle, and a body that writes detaches a private copy
    /// (`Arc::make_mut`) -- so a check whose body never writes never clones.
    /// Cloning a `Run` into a rules store is a refcount bump, not a deep copy.
    world: game_core::engine::SharedWorld,
    /// The pass-start world surfaces (player states, field lists) -- the base
    /// the commit's guest-write diff is taken against (design G10 §6.3.3).
    /// Empty on a probe run (its writes are discarded anyway).
    pass_base: game_core::engine::PassBase,
    /// Overlay-stack depth at drive entry. The guest writes this drive crosses
    /// onto live are revertible down to here -- by an outer drive's pass top,
    /// or by the commit pass. Never the same as an enclosing drive's floor.
    overlay_floor: usize,
    /// This drive's overlay id (see [`PlayCtx::guest_overlays`]): only
    /// crossings carrying it are this pass's own to revert.
    drive_id: u32,
    pile_checkpoints: Arc<std::collections::BTreeMap<usize, PileCheckpoint>>,
    data: Arc<GameData>,
    /// Declared static properties (`card_props`) of every card in the loaded
    /// set, snapshotted at run creation -- the one thing a `Run` needs from the
    /// card modules. A snapshot (not a `CardModules` handle) keeps `Run`
    /// concrete so the native backend can hold one in a thread-local.
    props: Arc<std::collections::HashMap<String, std::collections::BTreeMap<String, i32>>>,
    trigger: Trigger,
    /// The execution id factor writes of this body are keyed by (see
    /// [`Run::claim_factor`]): carried by a resume, fresh for a new firing.
    ledger_exec: u32,
    /// The last figure this run wrote through an amount setter -- see
    /// [`Run::fig_write`]. Per run (a body), so two bodies writing the same
    /// number still each fold.
    fig: Option<i32>,
    /// The seat whose activation this is (`Call::player_id`) -- the hand play's
    /// player, the counteraction's declarer. Distinct from
    /// [`Run::trigger`].`player_id`, which is the *answered* link's player on a
    /// counteraction (the payer). [`Run::stamp_owner`] falls back to this, not
    /// to the trigger, so a counter that places itself keeps its declarer as
    /// owner.
    activation_player: i32,
    /// The card whose effect is running -- the one `place_card` /
    /// `unplace_self` act on.
    current_card: String,
    /// The **instance** this run is for. A card's name is not an identity --
    /// one player may hold several copies of the same card in play -- so the
    /// uid is. It is the one the dispatch named, and it follows the card if
    /// this run re-places it (the same object moves with it).
    current_uid: i32,
    /// The owner of the instance this run took off the field (`unplace_self`),
    /// so a card that hops to another field (re-placing itself) keeps its owner.
    carried_owner: Option<i32>,
    /// Where this card goes when the run finishes, set by `set_dest`.
    dest: i32,
    /// Whose pile the dest lands in, set by `set_transfer_to_dest`. `None` is
    /// the owner the instance leaves -- what `set_dest` names.
    dest_to: Option<i32>,
    /// The cause `set_dest` / `set_transfer_to_dest` gave the deferred dest
    /// (`None` = the `Resolve` default).
    dest_cause: Option<i32>,
    /// Uids the drive's loop-top restore put back into the live world so this
    /// pass's early reads see the entry state. A uid still in the pass's world
    /// at commit was not taken out again by the replay -- a nested effect took
    /// the row, so the committed world drops it (see `commit_after`).
    reinserted: Vec<i32>,
    /// Payments this run actually made, `(from, to, amount)` -- the engine
    /// raises `payAfter` / `paid` for each once the run commits.
    paid_log: Vec<(i32, i32, i32)>,
    /// Cards this run put into a discard pile, `(player_id, id)` -- the engine
    /// raises `discarded` for each once the run commits.
    discard_log: Vec<(i32, String)>,
    /// Cards this run drew, `(player_id, id)` -- the engine raises `drawn` /
    /// `drew` for each once the run commits (the per-draw after points).
    draw_log: Vec<(i32, String)>,
    /// One draw **instruction** this run made, `(player_id, batch)` -- the
    /// engine raises `drawAfter` once with the whole batch after the last
    /// `drew` of the drive (design G11 §5.7). A replacement is in this and in
    /// `draw_log`; a cancelled draw is in neither.
    draw_after_log: Vec<(i32, Vec<String>)>,
    /// Frame ids present when the `Run` was built -- the commit net (§5.4)
    /// leaves those alone unless the run adopted them. The drive constructors
    /// fill it; the pure query/probe runs leave it empty, which is safe only
    /// because they never commit (if one ever gains a commit, fill it first).
    frames_at_drive_start: Vec<i32>,
    /// Frames this run adopted instead of creating (a replayed
    /// `inspection_begin`, §5.2). The commit net releases them even though
    /// they are in `frames_at_drive_start`.
    adopted: Vec<i32>,
    /// Hand arrivals this run filed by name, `(player_id, id, counted)` -- the
    /// engine raises `handChanged` for each uncounted one once the run commits.
    hand_log: Vec<(i32, String, bool)>,
    /// Players whose deck this run shuffled -- `reshuffled` once it commits.
    reshuffle_log: Vec<i32>,
    /// Named markers spent during the run, `(player_id, n, name)` -- raised as
    /// kind 73 (`fireSpent`, wire-stable) once the run commits. The name is
    /// payload; `charge` is one caller.
    marker_spent_log: Vec<(i32, i32, String)>,
    /// Bound held-token writes this run made, `(holder, delta, name,
    /// instance)` -- the engine raises `markerChanged` for each once the run
    /// commits (HAG-01). `delta` is the signed change the write actually
    /// applied; every player-token write (`bind_held` / `add_held` /
    /// `set_held` / `spend_marker`, and a transfer's two player legs) is in
    /// here, so a standing 「has N」 count has a post-commit event to hang on.
    marker_changed_log: Vec<(i32, i32, String, i32)>,
    /// Houses added during the run, raised as kind 75 once it commits.
    /// House writes this run applied: `(owner, tile, now, delta)` -- the
    /// owner at write time (design 5.5). `commit_after` raises `houseAdded`
    /// for a rise and `houseRemoved` for a fall.
    house_log: Vec<(i32, i32, i32, i32)>,
    /// Named-counter writes this run made, `(owner, card, name, change)` -- the
    /// engine raises `counterChanged` for each once the run commits. `change`
    /// is the signed delta the write applied (0 = a write that landed on the
    /// same count, e.g. a card placed with none); the count it left is the
    /// instance's own `counter(name)`. One log for every named counter
    /// (ruleset counter keys and any card-declared name).
    counter_log: Vec<(i32, String, i32, String, i32)>,
    named_raise_log: Vec<(i32, String, i32)>,
    /// Skill-state 1↔2 toggles this run made, `(who, from, to)` -- the engine
    /// raises `state` for each once the run commits (Mujica:欢迎来到ave
    /// mujica的世界 clause (2)). The initial 0→1 / 0→2 write is not a toggle.
    state_log: Vec<(i32, i32, i32)>,
    /// Which of the card's numbers this play doubles, or -1. Set by the
    /// doubling group skill, which is not ported yet.
    doubled: i32,
    /// Props the running card wants on the lingering instance `ctx::linger`
    /// binds (`docs/PURCHASE.md` P5). A hand play has no field instance for
    /// `set_prop` to write, so a `set_prop` against no instance parks the value
    /// here and `linger` carries it onto [`crate::world::Lingecycle::props`].
    linger_props: std::collections::BTreeMap<String, i32>,
    /// This run is an already-in-play card applying its effect (a field /
    /// tile / lingering hook, a settle body, a scheduled callback): its log
    /// lines say 「<卡名> 的效果：<what happened>」 rather than standing alone.
    /// A fresh activation (a hand play, a counteraction declaration, a skill press,
    /// a drawn event) logs its own action and leaves its outcome lines bare.
    wrap_effect: bool,
    /// Live-stream sink for write-through logs (`ctx::log`, the activation
    /// header). `None` outside a drive (probes, quotes). Each post appends to
    /// the **live** event tail at the call site, so a body that logs → pays →
    /// logs shows log, pay, log in code order. A multi-pass re-run's k-th log
    /// replaces [`Self::posted`]\[k\] instead of appending (same shape as the
    /// pre-filled `answers` log).
    ///
    /// # Soundness of the raw `PlayCtx` pointer
    ///
    /// Same pattern as [`DriveInline`]'s `cx` (see that struct's docs): the
    /// pointer is installed at `Run` construction from the `PlayCtx` the drive is
    /// running on, and never outlives it.
    ///
    /// * **Lifetime.** `drive_inner_body` holds `cx: &mut PlayCtx` for the whole
    ///   drive. The `Run` (and every copy `Ruleset::store` makes of it) lives
    ///   inside that call. `Run` is not `'static` and is not sent across
    ///   threads; the `'static` bound is the usual launder for a pointer that
    ///   is only dereferenced inside the frame that created it, exactly as
    ///   `DriveInline` does.
    /// * **Aliasing.** While the guest is running, `cx` is only reached through
    ///   this pointer or `DriveInline`'s -- never as a simultaneous `&mut`.
    ///   The drive's own `&mut PlayCtx` is used only between guest calls (NeedHost /
    ///   NeedInput arms), and those arms run after the guest has returned, so
    ///   no `&mut PlayCtx` is live at the same time as a post through this pointer.
    ///   (`announce`'s closure holds a second copy of the same pointer; it runs
    ///   inside `Ruleset::run` / `run_hook`, while the drive is blocked on that
    ///   call, so the same exclusion holds.)
    /// * **Panic / trap.** A wasm trap unwinds out of `call_card` as
    ///   `RuleError::Trap`, which `drive_inner_body` handles **without**
    ///   unwinding through `PlayCtx` -- the pointer is never left dangling. On the
    ///   native backend a `catch_unwind` around the guest call turns a panic
    ///   into the same `RuleError`. A halted drive (`Halt`) drops its posted
    ///   lines (`drop_events`) before returning, so a discarded pass leaves no
    ///   orphan events. There is no `Drop` impl that would touch the pointer.
    live: Option<*mut PlayCtx<'static>>,
    /// Event ids this drive has posted to the live stream, in call order.
    /// Survives across passes (the drive hands it back at each pause).
    posted: Vec<i32>,
    /// How many log calls this pass has consumed (index into [`Self::posted`]).
    log_index: usize,

    /// Uids this drive handed to the body's own placements, in call order.
    /// Survives across passes: a re-run re-uses the same instances instead of
    /// minting new ones (a placement is not repeated), so an engine routine
    /// acting on the row still finds it.
    place_log: Vec<i32>,
    /// How many placements this pass has served (index into
    /// [`Self::place_log`]).
    place_index: usize,
    /// How many narration / outcome lines this run has posted (any
    /// `post_log*` / `post_card`). A silent entry's body must leave this at
    /// 0 -- see [`RulesBridge::check_silent_body`]. Not taken by the
    /// multi-pass replace bookkeeping (unlike [`Self::posted`]).
    logged: u32,
    /// This run's entry is silent (`On::silent`): no activation flash and
    /// no outcome lines for its state / plan writes either (the flag hides
    /// the whole presentation, and the body must be pure bookkeeping).
    silent_body: bool,
    /// This run's entry is `QuietWhenEmpty`: retract the header at commit if
    /// the body produced nothing visible.
    quiet_when_empty: bool,
    /// Ruling 2a: the 「效果适用」 header is **lazy** -- buffered here at body
    /// entry and posted only immediately before the drive's first visible
    /// outcome. A drive that ends with nothing visible (a declined optional
    /// prompt, a bookkeeping write) posts no header at all. `None` once
    /// flushed or when the entry is silent (nothing to post).
    pending_header: Option<PendingHeader>,
    /// The header this run flushed (Ruling 2a), or -1 if none was posted
    /// (silent / nothing visible). `commit_after` completes this one only.
    own_activation: i32,
    /// Every 「效果适用」 header this run actually posted (`flush_header`'s
    /// take-succeeded path), in post order. One drive, one flash decision,
    /// every header it posted -- `commit_after` stamps them all. Sibling
    /// entries each flush their own header; the `__hdr__` once-per-flash
    /// identity only names the first.
    header_ids: Vec<i32>,
    /// What the body produced (for the flash-scope classification).
    /// ...
    /// This run halted for a prompt (the header is already on the stream and
    /// must not be retracted).
    prompted: bool,
    /// The `"card"` activation event id this run groups under (`-1` = none).
    /// Every write-through event carries it, so the log nests the outcome
    /// under the header.
    parent: i32,
    /// This entry's declarative flash policy (`ManifestOn::flash`, v55).
    /// `ctx::flash` / `no_flash` / the once-per-flash identity live on
    /// `world.flash_*` so they survive a prompt's routine re-run the same way
    /// the event tail does.
    flash_policy: card_sdk::abi::Flash,
    /// Did **any** sibling entry cue the player? OR of the imperative
    /// `ctx::flash()` and every `Flash::Always` declaration on this drive.
    /// Distinct from [`Self::flash_policy`] (the folded *decision*, `Never` >
    /// `Always` > `Auto`): "was the player cued" and "should the drive flash"
    /// are different questions, and a `Never` sibling may still lose the
    /// flash while an `Always` sibling already flushed the header. ORs across
    /// entries -- never folded, never reset per entry.
    flash_cued: bool,
    /// Next flash ordinal **this pass** (resets each re-run so the same call
    /// site maps to the same identity). `SharedFlash.seen` holds the posted.
    flash_ordinal: u32,
}

impl Run {
    /// The instance this body already placed for `(card, player)`, when the
    /// call is being reached again. A body is re-run from its top after every
    /// pause (design G10 §6.3.3), so without this every pass would add a
    /// second copy. The row is cleared per play and the uid has to still be on
    /// a field, so a card that unplaces itself and places again (or a second
    /// hand copy of the same card) places afresh.

    fn after_host_inner(
        &mut self,
        answer: usize,
        condition: Option<&game_core::engine::World>,
        after_money: Option<&[i32]>,
        after_pos: Option<(i32, i32)>,
    ) {
        // The consumed answer means the request's effect already happened --
        // on live, not in this copy (a re-run that reached the site from the
        // routine's snapshot starts with the pre-request transactions). The
        // body goes on reading them (the deed it just wrote, a `ctx::roll_pick`
        // on the cast it just threw): reload the copy from live. Otherwise the
        // copy's pre-request values would both feed the body stale reads and be
        // crossed back over the live effects at the commit.
        //
        // The body's own writes travel back onto the copy: they are captured
        // against the pass base and replayed onto the reloaded world. The pass
        // base itself does not move -- the copy's writes stay in the commit's
        // diff, and the host effect just applied is already on live.
        if let Some(live) = self.live {
            let live = unsafe { &mut *live };
            // Put the post-request money back on both sides: live (the
            // re-run skipped the effect) and the copy (which started this
            // pass from the pass base and would otherwise hand its stale
            // figure back in below).
            // Only when the request itself moved money: a move's own settle
            // completes after its `Ok`, so its recorded figure predates the
            // settlement's charge -- re-imposing it would undo a buy the
            // re-run never replays.
            let money_moved = after_money.is_some_and(|m| {
                condition
                    .map(|c| c.st.players.iter().map(|p| p.money).collect::<Vec<_>>())
                    .as_deref()
                    != Some(m)
            });
            if money_moved {
                let money = after_money.expect("money_moved");
                // A seat still at the figure the request started from is the
                // stale one the request outran (live was rewound by the pass
                // top / the copy by the rebuild) and takes the post-request
                // figure; a seat something has already moved past it -- the
                // request's own effect from an earlier consumption, or a
                // nested effect that came after it (the start square's pass
                // reward behind a return-to-start) -- keeps the newer figure
                // instead of being dragged back.
                for (i, v) in money.iter().enumerate() {
                    let pre = condition.and_then(|c| c.st.players.get(i)).map(|p| p.money);
                    if let Some(p) = live.world_mut().st.players.get_mut(i) {
                        if Some(p.money) == pre {
                            p.money = *v;
                        }
                    }
                    if let Some(p) = self.world.st.players.get_mut(i) {
                        if Some(p.money) == pre {
                            p.money = *v;
                        }
                    }
                }
            }
            // A move's landing: the pass top rewound the mover to the request's
            // pre-state (so the re-derived destination reads the same origin),
            // and the effect itself is not re-applied on a plain reuse -- put
            // the landing back before the body reads it.
            if let Some((seat, pos)) = after_pos {
                let s = seat.max(0) as usize;
                if let Some(p) = live.world_mut().st.players.get_mut(s) {
                    p.pos = pos;
                }
                if let Some(p) = self.world.st.players.get_mut(s) {
                    p.pos = pos;
                }
            }
            // A pile write is not replayed: the pass top reset live's piles to
            // the drive start and the answer is consumed without re-applying,
            // so the checkpoint is the only record of what this request did to
            // them. Put it on live (the effect landed there, even though the
            // reset took it off) and on the copy about to be crossed.
            let checkpoint = self.pile_checkpoints.get(&answer);
            if let Some(c) = checkpoint {
                live.world_mut().hidden = c.world.hidden.clone();
            }
            let mut old = std::mem::replace(&mut self.world, live.share_world());
            if let Some(c) = checkpoint {
                c.apply(&mut old);
            }
            // (applied again after the re-share below: that hands a *fresh*
            // live handle, whose turn is the outer play's)

            // The crossing below also applies the body's own writes to live,
            // so the reloaded copy already carries them. The copy was shared
            // with live *before* the crossing, which detaches live; re-share
            // afterwards so the copy is the post-cross world (guest writes,
            // and any effect a nested drive committed meanwhile, included).
            // The diff base is live **minus the drive's own crossings**, not
            // the request's pre-state: a surface the routine superseded (a
            // restore) keeps the routine's value from `live`, and the guest's
            // write is re-recorded as a delta over it, so a pass-top revert
            // puts the routine's value back rather than the pre-request one.
            // It is also the base the pass top will leave: the rows above the
            // floor are undone here (a rewind supersedes the body's writes made
            // so far, exactly like a restart), and the reloaded rows record
            // them again over it.
            let mut base = live.pass_base_for(self.overlay_floor, self.drive_id);
            // A pile is an ordered list, so its crossing is measured against
            // the request's pre-state: the stale copys list must not be laid
            // over what a nested drive (the counter this request raised) took
            // out of it meanwhile. The re-run re-derives the body's own pile
            // writes, so unlike a re-issued position the list is not replaced.
            // Money is an absolute running total and the request's own effect
            // is already on `live`: measured against the pass base, the host
            // effect would read as a guest write and be crossed back (a pay's
            // debit undone by its own replay). The copy is the pre-request
            // state plus the body's writes, so the request's pre-state is the
            // base that isolates them.
            //
            // The base the *commit* measures against stays the pass start:
            // the money row the commit records must be the whole pass delta,
            // or a write made before the last request would be reverted by
            // the pass-top rewind and never re-applied.
            let commit_money = base.money.clone();
            // A pile is an ordered list: its edit is replayed as a diff, so
            // the base must be the list `live` holds now, not the pass base --
            // a card the body added (already crossed by an earlier pause) or
            // the request moved would otherwise be inserted again. The
            // reloaded copy starts from this same list, so a write the body
            // did not re-make shows as no delta.
            base.piles = live
                .world()
                .hidden
                .iter()
                .map(|h| (h.hand.clone(), h.deck.clone(), h.graveyard.clone()))
                .collect();
            if let Some(c) = condition {
                let pre = PlayCtx::pass_base_of(c);
                base.next_steps = pre.next_steps;
            }
            if money_moved {
                // Base on the money the copy **now** carries, not the row's
                // recorded after-figure: the loop above has already re-imposed
                // the request's own effect on a seat that still stood at the
                // pre-value, while a seat something moved past keeps that newer
                // figure (a nested pay behind this request). Either way the
                // copy is the post-request world, so diffing against it makes
                // the request's own debit invisible to the crossing -- an
                // after-figure higher than a nested seat would otherwise be
                // recorded as a guest write and reverted at the next pass top.
                base.money = self.world.st.players.iter().map(|p| p.money).collect();
            }
            let crossed = live.overlay_guest_state(&base, &old, self.drive_id);
            // The staged move plan (`ctx::plan::set_*`) is a guest write too,
            // and the body reads it back when it issues the move.
            live.overlay_guest_plan_tags(&old);
            self.world = live.share_world();
            // A status increase paused by this request (心の雨's stun before its
            // own gain) was recorded but kept off live while the request ran;
            // the rebuilt copy takes it back so the body and the commit see it.
            crossed.apply_state(&mut self.world);
            self.world.turn.plan = old.turn.plan.clone();
            self.world.turn.plan_stack = old.turn.plan_stack.clone();
            // World-level latches the body may have written before this
            // request (a `give_extra_turn` then a draw): the reload above
            // drops the copy's own values, and they ride the commit like any
            // other guest write.
            self.world.extra_turns = old.extra_turns.clone();
            self.world.end_turn_request = old.end_turn_request;
            self.world.blocked_settle = old.blocked_settle.clone();
            self.world.st.skip_move = old.st.skip_move;
            self.carry_play_scope(&old);
            self.world.skip_log = true;
            self.reshuffle_log.clear();
            // The reloaded copy is live: a later pause's diff must not
            // re-record the host effect as a guest write (a pass-top revert
            // would then undo it). The body's own writes are on live too
            // (crossed at their pause) and the replay re-makes them, so the
            // base is live *minus* them -- the commit's revert undoes `own`
            // and its diff hands the writes back.
            self.pass_base = base.clone();
            self.pass_base.money = commit_money;
        }
        if let Some(checkpoint) = self.pile_checkpoints.get(&answer) {
            checkpoint.apply(&mut self.world);
            // These maintenance hooks already ran before this host request.
            self.reshuffle_log.clear();
        }
    }

    /// Stamp the shared turn plan's skill-source tag when the running rule is
    /// a skill (`card_id` starts with `skill:`). Cond reads it as `move.by_skill`.
    /// Called from the plan-write host handlers so a skill's plan rewrite
    /// carries the tag onto the move it shapes. Cleared after each move.
    fn stamp_skill_source(&mut self) {
        if self.current_card.starts_with("skill:") {
            self.world.plan_mut().by_skill = true;
        }
    }

    /// An already-in-play card's log lines name their source: 「<卡名> 的效果：
    /// <what happened>」. A fresh activation (hand play, counteraction, skill press,
    /// drawn event) leaves its outcome lines bare -- the play already said
    /// which card they belong to.
    fn attribute(&self, msg: Msg) -> Msg {
        if self.wrap_effect {
            Msg::new("log.card_effect")
                .card("card", self.current_card.clone())
                .msg("what", msg)
        } else {
            msg
        }
    }

    /// Post one log line **write-through** to the live event stream (the
    /// activation header, `ctx::log`, `ctx::effect`). Walk flushes happen on
    /// the live world first (`World::log`), so an approach always precedes the
    /// line that interrupted it.
    ///
    /// A multi-pass re-run's k-th call replaces the k-th already-posted line
    /// (same shape as the pre-filled `answers` log) instead of appending a
    /// duplicate. Without a live sink (probe / quote) the line lands on the
    /// guest copy only, as before.
    fn post_log(&mut self, kind: &str, player_id: i32, msg: Msg, card: &str) {
        self.post_log_value(kind, player_id, msg, card, 0)
    }

    /// Post the buffered 「效果适用」 header (Ruling 2a) just before the
    /// drive's first visible outcome. Returns the header's event id, or the
    /// current parent when there is nothing pending (silent / already
    /// flushed).
    ///
    /// Once-per-flash (v55): the header IS the activation's flash event. A
    /// prompt's routine re-run must not post it a second time -- the identity
    /// lives in `SharedFlash` across `World::clone`, and the re-run adopts the
    /// live event tail so the first pass's header remains. Reuse it as the
    /// parent for the re-run's body lines.
    fn flush_header(&mut self) -> i32 {
        let hkey = self.flash_key("__hdr__");
        let already = self.world.flash.lock().seen.contains_key(&hkey);
        let Some(h) = self.pending_header.take() else {
            return self.parent;
        };
        let id = self.post_card(
            &h.kind, h.owner, h.msg, &h.card, h.target, h.tile, false, h.uid,
        );
        self.parent = id;
        self.own_activation = id;
        self.header_ids.push(id);
        if already {
            // Default the second-and-later headers' flash off. This is **not**
            // the once-per-flash guarantee (that lives in the `"flash"` cue
            // event's `flash_key("ord:N")` identity) and not the drive's flash
            // decision either: sibling entries of the same drive also land
            // here, and `commit_after` overrides this default with the folded
            // decision for every id in `header_ids`. Kept as the safe default
            // for any id the vec somehow misses.
            if let Some(live) = self.live {
                let cx = unsafe { &mut *live };
                cx.set_event_flash(id, false);
            }
        } else {
            self.world.flash.lock().seen.insert(hkey, id);
        }
        id
    }

    fn post_log_value(&mut self, kind: &str, player_id: i32, msg: Msg, card: &str, value: i32) {
        self.logged = self.logged.saturating_add(1);
        // Ruling 2a: the header goes just before the drive's first visible
        // outcome (this line).
        self.flush_header();
        if let Some(live) = self.live {
            let cx = unsafe { &mut *live };
            let parent = self.parent;
            if self.log_index < self.posted.len() {
                let id = self.posted[self.log_index];
                self.log_index += 1;
                let ok = cx.relog_event(id, msg.clone());
                if !ok {
                    // Truncated tail -- append a fresh one and retarget the slot.
                    let id = cx.log_event(kind, player_id, msg, parent, card, value);
                    self.posted[self.log_index - 1] = id;
                }
                return;
            }
            self.log_index += 1;
            let id = cx.log_event(kind, player_id, msg.clone(), parent, card, value);
            if msg.key.contains("will_settle") {
                let ids: Vec<i32> = cx.world().recent.iter().map(|e| e.id).collect();
            }
            self.posted.push(id);
            return;
        }
        // No live sink: guest-copy only (probe / quote).
        let e = self.world.log(kind, player_id, msg);
        if !card.is_empty() {
            e.card = card.to_string();
        }
        if self.parent >= 0 {
            e.parent = self.parent;
        }
    }

    /// Publish a reveal (design G11 §5.6). The overlay is applied at once --
    /// a reveal is a view, not a pile write -- and the line rides this
    /// drive's `posted` slots so a prompt re-run replays it rather than
    /// posting a second one.
    #[allow(clippy::too_many_arguments)]
    fn post_reveal(
        &mut self,
        player: i32,
        reason: Msg,
        pile: game_core::state::RevealPile,
        ids: Vec<String>,
        card: &str,
        to: i32,
    ) {
        // A reveal is a visible outcome: post the lazy 「效果适用」 header
        // first (same rule as `post_log_value`), so the line and any state it
        // shows group under the body that produced them.
        self.flush_header();
        let count = ids.len() as i32;
        // A reveal-only body is not 「无事发生」 and a `silent` entry that
        // reveals is a bug.
        self.logged = self.logged.saturating_add(1);
        // The overlay is live view state: write it straight to the live world
        // when there is one (a pass's pile checkpoint does not carry
        // `reveals`), else to the guest copy.
        if let Some(live) = self.live {
            let cx = unsafe { &mut *live };
            cx.world_mut()
                .push_reveal(player, reason.clone(), pile, ids);
        } else {
            self.world.push_reveal(player, reason.clone(), pile, ids);
        }
        let parent = self.parent;
        if let Some(live) = self.live {
            let cx = unsafe { &mut *live };
            let id = if self.log_index < self.posted.len() {
                let id = self.posted[self.log_index];
                self.log_index += 1;
                if cx.relog_event(id, reason.clone()) {
                    id
                } else {
                    let id = cx.log_event("reveal", player, reason, parent, card, count);
                    self.posted[self.log_index - 1] = id;
                    id
                }
            } else {
                self.log_index += 1;
                let id = cx.log_event("reveal", player, reason, parent, card, count);
                self.posted.push(id);
                id
            };
            cx.set_event_to(id, to);
            return;
        }
        // No live sink: guest-copy only (probe / quote).
        let e = self.world.log("reveal", player, reason);
        if !card.is_empty() {
            e.card = card.to_string();
        }
        if count != 0 {
            e.value = count;
        }
        e.to = to;
        if parent >= 0 {
            e.parent = parent;
        }
    }

    /// Scope a flash identity to this drive's card so a later activation of
    /// the same card is a fresh drive (and a nested drive is independent).
    fn flash_key(&self, inner: &str) -> String {
        format!("{}|{}", self.current_card, inner)
    }

    /// Post one proactive `"flash"` event write-through, once-per-flash.
    ///
    /// Identity is `key` (an ordinal or an explicit `ctx::flash_card_keyed`
    /// key) within this drive. If the identity was already posted (a re-run
    /// after a prompt / halt), this is a **no-op** -- the flash already shown
    /// before the prompt must not play again. Otherwise the event is posted
    /// parent-linked under the activation and the identity is recorded.
    ///
    /// The lazy header flushes first (a flash is a visible outcome and the
    /// header names which clause is ficycle). Walk segments flush via
    /// `PlayCtx::log_event` / `World::log`, so an approach always precedes the
    /// flash that interrupted it.
    fn post_flash(&mut self, key: String, card: &str, msg: Msg, tile: i32) {
        if self.flash_policy == card_sdk::abi::Flash::Never {
            // Never suppresses even a proactive flash -- the rule said the
            // player must not be cued.
            return;
        }
        // Once-per-flash: already posted under this identity -> no-op.
        if self.world.flash.lock().seen.contains_key(&key) {
            return;
        }
        // Lazy header first: the flash is a visible outcome.
        self.flush_header();
        self.logged = self.logged.saturating_add(1);
        let player_id = self.trigger.player_id;
        // Carry the tile on the Msg so the client can aim the cue.
        let msg = if tile >= 0 {
            msg.clone().tile("tile", tile)
        } else {
            msg
        };
        let parent = self.parent;
        let id = if let Some(live) = self.live {
            let cx = unsafe { &mut *live };
            // Walk-flush ordecycle: `log_event` flushes pending walk segments
            // first, like any log, so an approach precedes the flash.
            let id = cx.log_event("flash", player_id, msg.clone(), parent, card, 0);
            // Carry across a prompt's re-run (once-per-flash).
            if let Some(e) = cx.world().recent.back() {
                self.world.flash.lock().saved.push(e.clone());
            }
            id
        } else {
            // No live sink (probe / quote): guest-copy only.
            let e = self.world.log("flash", player_id, msg);
            if !card.is_empty() {
                e.card = card.to_string();
            }
            e.parent = parent;
            e.id
        };
        self.world.flash.lock().seen.insert(key, id);
    }

    /// Post a `"card"` activation (the 「效果适用」 header / a skill 「发动」 /
    /// a negated flash) write-through. Returns the event id.
    fn post_card(
        &mut self,
        kind: &str,
        owner: i32,
        msg: Msg,
        card: &str,
        target: i32,
        tile: i32,
        negated: bool,
        uid: i32,
    ) -> i32 {
        // The header is not a body line: `logged` counts narration/outcome
        // lines the body posted, and a QuietWhenEmpty drive whose body was
        // silent must see 0 here so it can retract. It is also **not** part of
        // the `posted`/`log_index` replace-log mechanism (that exists so a
        // multi-pass re-run's *body* lines replace the learn pass's, not
        // duplicate); the header is posted once and never replaced. Sharing
        // the slot shifted every body line by one and double-logged on a
        // re-run.
        if let Some(live) = self.live {
            let cx = unsafe { &mut *live };
            let parent = self.parent;
            let id =
                cx.log_card_activation(kind, owner, card, target, tile, negated, msg, parent, uid);
            return id;
        }
        let e = self
            .world
            .card_activation(kind, owner, card, target, tile, negated, msg);
        e.parent = self.parent;
        e.uid = uid;
        e.id
    }

    /// One outcome line for a silent state write (place / mark / counter /
    /// status), so nothing a body did is invisible under the header. Posted
    /// write-through like every other body line.
    fn outcome(&mut self, who: i32, msg: Msg) {
        self.post_log("text", who, msg, "");
    }

    /// Outcome line for a state-key write. The engine has no key->phrase map
    /// and no auto-narration: messages are built by the ruleset (`log_as` /
    /// `ctx::log` with its own i18n key). A raw `state_set` is silent.
    fn outcome_state(&mut self, _who: i32, _key: &str, _was: i32, _now: i32) {
        // Intentionally empty. See `docs/STATUS-HOSTS.md`.
    }

    /// Record a real skill-state 1↔2 toggle so the commit point can raise
    /// `state` (Mujica:欢迎来到ave mujica的世界 clause (2)) and stamp
    /// `switchedRound = turn_key` for 「所有本回合切换了状态的玩家」.
    /// The 0→1 / 0→2 initial write (band-skill placement) is not a toggle.
    fn note_state_toggle(&mut self, who: i32, key: &str, was: i32, now: i32) {
        if key != card_sdk::abi::state_key::SKILL_STATE {
            return;
        }
        if was == now || !matches!(was, 1 | 2) || !matches!(now, 1 | 2) {
            return;
        }
        // Host stamp: character skills never wrote the flag themselves, so
        // clause (2)'s 「所有本回合切换了状态的玩家」 never lit.
        let turn_key = self.world.st.round * 100 + self.world.st.turn + 1;
        self.world.state_set(who, "switchedRound", turn_key);
        // Visible outcome: `commit_after` raises `state` on this row.
        self.flush_header();
        self.state_log.push((who, was, now));
    }

    /// Outcome line for a plan rewrite the rulebook words as happening (dice,
    /// teleport destination, stop-at, extra steps, no-buy / no-build, start).
    /// Posted under the activation header so the log says what the clause did
    /// to the move.
    fn outcome_plan(&mut self, mk: &str, tile: i32, n: i32, reason: &str) {
        if self.silent_body {
            return;
        }
        let who = self.trigger.player_id;
        self.outcome(
            who,
            Msg::new(mk)
                .player_id("who", who)
                .tile("tile", tile)
                .i("n", n as i64)
                .text("reason", reason),
        );
    }

    /// Record a named counter write on the instance at `uid` so the commit point can
    /// raise `counterChanged` (wire event name; the same shape as
    /// `marker_spent_log` / `house_log`). `was` and `now` bracket the write;
    /// only the delta rides the trigger, so a write that lands on the same
    /// count still raises -- a card placed with no named counters has to hear
    /// about its own count. A write to an instance that is not on a field
    /// raises nothing.
    ///
    /// The header names the clause, so any visible outcome must flush it (the
    /// same contract `post_log_value` already honours). A counter write is
    /// visible: `commit_after` raises `counterChanged` on it and `body_empty`
    /// counts it, so a pure counter body must still post its 「效果适用」.
    fn note_counter_changed(&mut self, uid: i32, was: i32, now: i32, name: &str) {
        let Some(f) = self.world.field_by_uid(uid) else {
            return;
        };
        // The holder hears its card's counter change, not the card's owner.
        let (owner, card) = (self.world.holder_of(uid), f.card.clone());
        self.flush_header();
        self.counter_log
            .push((owner, card, uid, name.to_string(), now - was));
    }

    // On-card [CP点] writes ride `note_counter_changed` as `counter::CP` (v51).
    // `CpChanged` is retired (user ruling 2026-10-07): the 「its on-card [CP点]
    // is empty → graveyard」 rule lives on `Action::CounterChanged` with a
    // `counter::CP` name check in the residual guard
    // (`rules/cards/card-general/src/clear_cp.rs`), not a CP-only trigger.

    /// Move the running instance to `dest` **now** -- unplace-and-send, applied
    /// mid-effect rather than at the run's commit, for a card that must be gone
    /// before the rest of the effect runs. `to` is whose pile it lands in;
    /// `None` is the owner it leaves. Returns that owner, or `None` when it was
    /// not in play. Clears `current_uid`, so a later deferred dest finds no
    /// instance to move.
    fn move_now(&mut self, to: Option<i32>, dest: i32, _cause: i32) -> Option<i32> {
        // The **row's own** card leaves; `current_card` may be a stand-in the
        // instance is marked with (the `effective_card` dies with the row).
        let own = self
            .world
            .field_by_uid(self.current_uid)
            .map(|f| f.card.clone());
        let left = self.unplace_card();
        if left < 0 {
            return None;
        }
        let who = to.unwrap_or(left);
        let card = own.unwrap_or_else(|| self.current_card.clone());
        if dest == DEST_GRAVEYARD {
            self.to_discard(who, &card);
        } else if dest == 1 {
            self.add_to_dest(who, &card, dest, false);
        }
        // Banished (「banish」) is just "gone" -- it left above and goes nowhere.
        Some(left)
    }

    /// Put a row the request's `condition` still holds back on this copy --
    /// the leave/detach replay's half: the live effect removed it and the
    /// re-run's reads (a `card.placed` condition, `ctx::field_instances`)
    /// have to see it again. A row already present is left alone.
    ///
    /// The row goes back at the index it held in the request's pre-state, not
    /// at the end: the body walks the field in order, and a re-run that sees
    /// the rows shuffled issues its positional requests against the wrong
    /// slots of the journal (the sweep's leave of the shout matched the
    /// entry of another card and its pile half was lost).
    fn reinsert_row(&mut self, uid: i32, condition: Option<&game_core::engine::World>) {
        if std::env::var("DBG_NO_INS").is_ok() {
            return;
        }
        let Some(cond) = condition else { return };
        let Some(row) = cond.field_by_uid(uid).cloned() else {
            return;
        };
        let board = cond.st.board_field.iter().any(|r| r.uid == uid);
        let seat = cond
            .st
            .players
            .iter()
            .position(|p| p.field.iter().any(|r| r.uid == uid));
        let index = if board {
            cond.st.board_field.iter().position(|r| r.uid == uid)
        } else {
            seat.and_then(|seat| {
                cond.st
                    .players
                    .get(seat)
                    .and_then(|p| p.field.iter().position(|r| r.uid == uid))
            })
        }
        .unwrap_or(usize::MAX);
        let put = |w: &mut game_core::engine::World| {
            if w.field_by_uid(uid).is_some() {
                return;
            }
            let insert = |v: &mut Vec<game_core::state::FieldCard>| {
                let at = index.min(v.len());
                v.insert(at, row.clone());
            };
            if board {
                insert(&mut w.st.board_field);
            } else if let Some(seat) = seat {
                if let Some(p) = w.st.players.get_mut(seat) {
                    insert(&mut p.field);
                }
            }
        };
        put(&mut self.world);
        // Live too: the pass top rebuilds the copy from live, so a copy-only
        // row is gone before the re-run's next read. The commit's field diff
        // takes it off again (the last pass starts from a base that has it).
        if let Some(live) = self.live {
            if std::env::var("DBG_NO_LIVE").is_err() {
                put(unsafe { &mut *live }.world_mut());
            }
        }
    }

    /// The replay's half of [`Self::move_row_out`]: take the row out and
    /// forget the running instance, but leave the piles alone -- a re-run's
    /// world already carries the host's pile write (`PileCheckpoint`), so
    /// filing the card again would double it.
    ///
    /// `outcome` posts the `log.outcome.unplace` line the original
    /// `unplace_card` posted: a `Leave` replay stands in for `unplace_card`,
    /// a `Detach` one does not (`zone_detach` posts no such line).
    fn move_row_only(&mut self, uid: i32, _to: Option<i32>, dest: i32, outcome: bool) {
        let Some(row) = self.world.field_by_uid(uid).cloned() else {
            return;
        };
        // The re-run's own `zoneLeft` may have rewritten the destination; the
        // live writer read the rewrite back, so this does too.
        let dest = row
            .props
            .get(game_core::state::prop::LEAVE_DEST)
            .copied()
            .unwrap_or(dest);
        let from_field = row.on < 0;
        if uid == self.current_uid {
            self.carried_owner = Some(row.owner);
        }
        let owner = self.world.unplace_at(uid);
        if owner < 0 {
            return;
        }
        if uid == self.current_uid {
            self.current_uid = -1;
            // The re-run's own instance left: post the outcome line the
            // original `unplace_card` posted, so the commit pass's line stream
            // still lines up with the learn pass's.
            if outcome {
                self.outcome(
                    owner,
                    Msg::new("log.outcome.unplace").card("card", row.card),
                );
            }
        }
        // The writer's cascade (`PlayCtx::zone_commit`): attachments first,
        // then created children. A leave rewritten from a field back onto a
        // field keeps its edges -- the subtree just follows the new holder.
        if !(from_field && dest == DEST_FIELD) {
            for a in self.world.attached_cards(uid) {
                self.move_row_only(a, None, DEST_GRAVEYARD, false);
            }
            for c in self.world.created_cards(uid) {
                self.move_row_only(c, None, DEST_GRAVEYARD, false);
            }
        }
    }

    /// [`Self::move_now`] for an explicitly named row (a detach, or the replay
    /// of a logged leave whose instance is not the running one). Returns the
    /// owner the row had, `None` when it was not on the field.
    fn move_row_out(&mut self, uid: i32, to: Option<i32>, dest: i32) -> Option<i32> {
        let own = self.world.field_by_uid(uid).map(|f| f.card.clone());
        // A body that takes its own instance off and places it again (the
        // 「移动」 hops) keeps its owner across the fresh instance; remember it
        // the way `unplace_card` does, before the row is gone.
        if uid == self.current_uid {
            self.carried_owner = self.world.field_by_uid(uid).map(|f| f.owner);
        }
        let left = self.world.unplace_at(uid);
        if left < 0 {
            return None;
        }
        if uid == self.current_uid {
            self.current_uid = -1;
        }
        let who = to.unwrap_or(left);
        let card = own.unwrap_or_else(|| self.current_card.clone());
        if dest == DEST_GRAVEYARD {
            self.to_discard(who, &card);
        } else if dest == 1 {
            self.add_to_dest(who, &card, dest, false);
        }
        Some(left)
    }
}

/// The host-side body of `CardWorld::draw` / `draw_over`: one routine, the
/// `over` flag only changes the trim and the log part. This path has no trim
/// loop (the draw routine on the match side owns it), so the flag's whole job
/// here is the allowance and the log.
fn run_draw(r: &mut Run, player_id: i32, n: i32, over: bool) -> i32 {
    let before = r
        .world
        .hidden
        .get(player_id.max(0) as usize)
        .map(|h| h.hand.len())
        .unwrap_or(0);
    let will_refill = r
        .world
        .hidden
        .get(player_id.max(0) as usize)
        .is_some_and(|h| !h.graveyard.is_empty() && n > 0 && h.deck.len() <= n as usize);
    let got = r.world.draw_cards(player_id, n, over);
    if will_refill && got > 0 {
        // Visible outcome: `commit_after` raises `reshuffled`.
        r.flush_header();
        r.reshuffle_log.push(player_id);
    }
    if got > 0 {
        let drawn: Vec<String> = r
            .world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.hand[before..].to_vec())
            .unwrap_or_default();
        if !drawn.is_empty() {
            // Visible outcome: `commit_after` raises `drawn` / `drew` (and the
            // batch's `drawAfter`).
            r.flush_header();
            r.draw_after_log.push((player_id, drawn.clone()));
            for id in drawn {
                r.draw_log.push((player_id, id));
            }
        }
    }
    got
}

impl CardWorld for Run {
    fn restore_from_world(&mut self, from: &game_core::engine::World) {
        // Same rule as `restore_condition`: the condition predates every
        // `latch:` the body froze after it, and the undo must not unfreeze
        // them (立希's rerun guard reset on every pass and never reached its
        // cap).
        let latched = self.world.latches();
        let play_scope = (
            self.world.turn.play_from_hand,
            self.world.turn.placed_during_play,
        );
        self.world = game_core::engine::SharedWorld::from(from.clone());
        self.world.put_latches(latched);
        // The condition was captured on live, which still names the outer
        // play; the nested body's own play scope has to stay (see
        // `Run::carry_play_scope`).
        self.world.turn.play_from_hand = play_scope.0;
        self.world.turn.placed_during_play = play_scope.1;
    }
    fn snapshot_owners(&self) -> Vec<(i32, Vec<i32>)> {
        self.world
            .world_snapshot_owners
            .iter()
            .map(|(key, owners)| (*key, owners.clone()))
            .collect()
    }
    fn add_snapshot_owner(&mut self, key: i32, uid: i32) {
        let owners = self.world.world_snapshot_owners.entry(key).or_default();
        if !owners.contains(&uid) {
            owners.push(uid);
        }
    }
    fn apply_state_overlay(&mut self, overlay: &game_core::engine::GuestOverlay) {
        overlay.apply(&mut self.world);
    }
    /// Re-apply a logged zone request to this body's own view -- see
    /// [`CardWorld::replay_host`]. The live world already has the result; only
    /// the guest copy (whose reads the re-run makes) is behind, and the pile
    /// logs are rebuilt so the commit's `discarded` / `handChanged` still fire
    /// exactly once.
    fn replay_host(
        &mut self,
        req: &crate::host::HostRequest,
        answer: i32,
        condition: Option<&game_core::engine::World>,
    ) {
        use crate::host::HostRequest;
        let live_has = |w: &game_core::engine::World, uid: i32| w.field_by_uid(uid).is_some();
        match req {
            HostRequest::Leave {
                uid,
                destination,
                to,
                ..
            } => {
                // The row the request took off lived on the request's
                // `condition`; the live world no longer has it and the copy
                // was just adopted from live. Put it back so the re-run's
                // reads see what the body saw, then replay the move.
                self.reinsert_row(*uid, condition);
                // The re-run re-places under the ledger uid, so the logged uid
                // still names the row; a body that moved something else falls
                // back to its own instance.
                let uid = if live_has(&self.world, *uid) {
                    *uid
                } else {
                    self.current_uid
                };
                let to = (*to >= 0).then_some(*to);
                self.move_row_only(uid, to, *destination, true);
            }
            HostRequest::Detach {
                uid, destination, ..
            } => {
                self.reinsert_row(*uid, condition);
                let uid = if live_has(&self.world, *uid) {
                    *uid
                } else {
                    self.current_uid
                };
                self.move_row_only(uid, None, *destination, false);
            }
            HostRequest::Flip { uid, face_down, .. } => {
                self.world.set_face_down_at(*uid, *face_down != 0);
            }
            HostRequest::Attach { .. } => {
                // The shell reports the attached uid (`-1` on refusal). A live
                // instance is re-parented in place -- the row is already in
                // the view; only its placement fields follow. A pile copy is
                // filed as a fresh row, which the re-run's view lacks (the
                // host fn takes the hand copy right after this).
                if answer < 0 {
                    return;
                }
                // The request states the placement itself. Live's row is the
                // apply's result, but the pass restore puts the row back at
                // its pass-start host (`-1` for a fresh attachment) before the
                // body re-reaches this slot, so reading it would undo the
                // attach every pass -- and the next request's cycle guard
                // would walk a flat field.
                let (host, face_down) = match req {
                    HostRequest::Attach {
                        host, face_down, ..
                    } => (*host, *face_down != 0),
                    _ => return,
                };
                let tile = self.world.field_by_uid(host).map(|h| h.tile);
                if let Some(row) = self.world.field_by_uid_mut(answer) {
                    row.on = host;
                    row.face_down = face_down;
                    if let Some(tile) = tile {
                        row.tile = tile;
                    }
                    return;
                }
                let Some(live) = self.live else { return };
                let live = unsafe { (*live).world() };
                let Some(src) = live.field_by_uid(answer) else {
                    return;
                };
                let board = live.st.board_field.iter().any(|r| r.uid == answer);
                let seat = live
                    .st
                    .players
                    .iter()
                    .position(|p| p.field.iter().any(|r| r.uid == answer));
                // Runtime counters are the replay's own writes -- start empty,
                // as `overlay_guest_rows` does for a freshly minted row.
                let mut row = src.clone();
                row.counters.clear();
                if board {
                    self.world.st.board_field.push(row);
                } else if let Some(seat) = seat {
                    if let Some(p) = self.world.st.players.get_mut(seat) {
                        p.field.push(row);
                    }
                }
                self.world.st.next_card_uid = self.world.st.next_card_uid.max(answer + 1);
            }
            _ => {}
        }
    }
    /// A body's placements are keyed by their call order: the `n`-th of this
    /// pass re-uses the uid the `n`-th of the first pass minted.
    fn place_or<F>(&mut self, card: &str, place: F) -> i32
    where
        F: FnOnce(&mut Self) -> i32,
    {
        let n = self.place_index;
        self.place_index += 1;
        if let Some(&uid) = self.place_log.get(n) {
            if self.world.field_by_uid(uid).is_some_and(|f| f.card != card) {
                // A restore put a **different** row back on the ledger uid: the
                // body left it before this placement and the re-run has not
                // taken it out yet. The placement is a new instance, so mint
                // past the occupied uid instead of adopting it.
                let uid = place(&mut *self);
                self.reinserted.retain(|u| *u != uid);
                if card == self.current_card {
                    self.current_uid = uid;
                }
                return uid;
            }
            if self.world.field_by_uid(uid).is_none() {
                // A restore dropped the row at a pass boundary (a host routine
                // ran in between, and the re-run is re-doing this placement).
                // Re-place it under the **ledger uid**: host effects the learn
                // pass already applied name that instance, so minting a fresh
                // one would orphan them. Seed the counter for the duration of
                // the call so the row lands on the ledger uid.
                let resume = self.world.st.next_card_uid;
                self.world.st.next_card_uid = uid;
                let made = place(&mut *self);
                debug_assert_eq!(made, uid, "re-placement minted past the ledger uid");
                self.world.st.next_card_uid = self.world.st.next_card_uid.max(resume).max(uid + 1);
                // The replay re-placed the row the restore put back for it: the
                // commit must keep this one (see `commit_after`).
                self.reinserted.retain(|u| *u != uid);
            } else {
                // The live counter never saw the first pass's mint (that pass
                // had only a copy), so it still names this uid; keep it past it.
                self.world.st.next_card_uid = self.world.st.next_card_uid.max(uid + 1);
            }
            // The first pass's `place` re-bound the run's own instance; carry
            // that across, or a later `self` write (a counter, a prop) on the
            // re-run would address `-1` instead of the row.
            if card == self.current_card {
                self.current_uid = uid;
            }
            return uid;
        }
        let uid = place(&mut *self);
        self.reinserted.retain(|u| *u != uid);
        self.place_log.push(uid);
        uid
    }
    // ---- G08 zone surface -------------------------------------------------
    // The run's world is the guest's transaction view: a body that places a
    // row and then reads it back (Here the world's held card, 灯's probe) must
    // see its own write, and the commit pass carries every write to the live
    // world.
    fn zone_of(&self, uid: i32) -> i32 {
        self.world.zone_of(uid).as_i32()
    }
    fn attachment_host(&self, uid: i32) -> i32 {
        self.world.attachment_host(uid)
    }
    fn attached_cards(&self, host: i32) -> Vec<i32> {
        self.world.attached_cards(host)
    }
    fn created_cards(&self, creator: i32) -> Vec<i32> {
        self.world.created_cards(creator)
    }
    fn earliest_owned_tile(&self, player_id: i32) -> i32 {
        self.world.earliest_owned_tile(player_id)
    }
    fn instance_owner(&self, uid: i32) -> i32 {
        self.world.instance_owner(uid)
    }
    fn set_instance_owner(&mut self, uid: i32, owner: i32) -> bool {
        self.world.set_instance_owner(uid, owner)
    }
    fn set_created_by_at(&mut self, uid: i32, creator: i32) -> bool {
        self.world.set_created_by_at(uid, creator)
    }
    fn set_ineffective_at(&mut self, uid: i32, on: bool) -> bool {
        // A played copy with no instance (`self_uid()` answers `-1`) marks the
        // play itself; its arrival then carries the waste bit.
        if uid < 0 {
            self.world.turn.play_ineffective = on;
            return true;
        }
        self.world.set_ineffective_at(uid, on)
    }
    fn is_ineffective_at(&self, uid: i32) -> bool {
        // Mirror the setter: a played copy with no instance (`-1`) keeps the
        // bit on the play, not on a row (review finding 8).
        if uid < 0 {
            return self.world.turn.play_ineffective;
        }
        self.world.is_ineffective_at(uid)
    }
    // ---- G03 roll surface -------------------------------------------------
    // The run's world is the write-through copy the guest reads (and the one
    // the commit pass hands back to the live world), so the roll queries read
    // their own transaction view and a pick writes into the copy the settle
    // sees.
    fn open_roll_id(&self) -> i32 {
        self.world.open_roll_id()
    }
    fn roll_transaction(&self, id: i32) -> Option<&game_core::engine::roll::RollTransaction> {
        self.world.roll_by_id(id)
    }
    fn roll_pick(&mut self, id: i32, index: i32) {
        self.world.roll_pick(id, index);
    }
    fn roll_unused_face(&self, id: i32) -> i32 {
        self.world.roll_unused_face(id)
    }
    fn roll_rerolled(&self, id: i32) -> bool {
        self.world.roll_rerolled(id)
    }
    fn rolls_by_player(&self, player_id: i32) -> Vec<i32> {
        self.world.rolls_by_player(player_id.max(0) as usize)
    }
    // The settle answers are plain world writes postcard-carried by the
    // commit pass with every other player-state write (the move half rides
    // `state_set("resolveOverride")`; the turn-end half is the world's
    // per-seat `blocked_settle`).
    fn set_move_settle_answer(&mut self, player_id: i32, answer: i32) {
        // An `Unanswered` write leaves the armed slot alone (`Match`'s
        // setter semantics).
        if let Some(slot) = game_core::engine::SettleAnswer::from_i32(answer).slot() {
            self.world.st.players[player_id.max(0) as usize].state_set("resolveOverride", slot);
        }
    }
    fn move_settle_answer(&self, player_id: i32) -> i32 {
        game_core::engine::SettleAnswer::from_i32(
            self.world.st.players[player_id.max(0) as usize].state_get("resolveOverride"),
        )
        .slot()
        .unwrap_or(0)
    }
    fn set_turn_end_settle_answer(&mut self, player_id: i32, answer: i32) {
        self.world.set_blocked_settle_answer(
            player_id.max(0) as usize,
            game_core::engine::SettleAnswer::from_i32(answer),
        );
    }
    fn turn_end_settle_answer(&self, player_id: i32) -> i32 {
        self.world
            .blocked_settle_answer(player_id.max(0) as usize)
            .slot()
            .unwrap_or(0)
    }
    fn after_host(
        &mut self,
        answer: usize,
        condition: Option<&game_core::engine::World>,
        after_money: Option<&[i32]>,
        after_pos: Option<(i32, i32)>,
    ) {
        self.after_host_inner(answer, condition, after_money, after_pos);
    }
    fn drop_pile_checkpoint(&mut self, answer: usize) {
        if self.pile_checkpoints.contains_key(&answer) {
            std::sync::Arc::make_mut(&mut self.pile_checkpoints).remove(&answer);
        }
    }
    fn roll(&mut self, player_id: i32, count: i32, sides: i32) -> i32 {
        // Roll on the guest copy (its RNG), then post the `"dice"` event
        // write-through so it survives `commit_after`'s adoption of the live
        // tail. A re-run replaces the already-posted line (same roll -- the
        // guest RNG restarts from the same share point).
        let total = self.world.roll_silent(player_id, count, sides);
        let msg = Msg::new("log.dice")
            .player_id("who", player_id)
            .i("count", count as i64)
            .i("sides", sides as i64)
            .i("sum", total as i64);
        self.post_log_value("dice", player_id, msg, "", total);
        total
    }

    fn effect(&mut self, player_id: i32, msg: Msg) {
        let msg = self.attribute(msg);
        // Name the running card on the event, so the client can ride the line
        // on that card's flash (正论暴击 and kin) instead of popping the effect
        // modal. The id matches the `"card"` activation's (`event:` instances
        // are announced without the prefix); empty outside a card body keeps
        // today's popup.
        let src = self
            .current_card
            .strip_prefix("event:")
            .unwrap_or(&self.current_card)
            .to_string();
        self.post_log("effect", player_id, msg, &src);
    }

    fn extreme(&self) -> i32 {
        self.world.turn.extreme.as_i32()
    }
    fn set_extreme(&mut self, v: i32) {
        self.world.turn.extreme = game_core::engine::Extreme::from_i32(v.signum());
    }
    fn play_from_hand(&self) -> bool {
        self.world.turn.play_from_hand
    }
    fn set_play_from_hand(&mut self, v: bool) {
        self.world.turn.play_from_hand = v;
    }
    fn placed_during_play(&self) -> bool {
        self.world.turn.placed_during_play
    }
    fn set_placed_during_play(&mut self, v: bool) {
        self.world.turn.placed_during_play = v;
    }
    fn play_card(&self) -> Option<String> {
        self.world.turn.play_card.clone()
    }
    fn gain_fixed(&mut self, player_id: i32, amount: i32, reason: crate::Msg) -> i32 {
        let got = self.world.gain_fixed(player_id, amount, reason);
        // 资金变动 (rulebook 支付阶段 7): a fixed gain/loss is still a money
        // change. Logged as `(-1, player, n)` for money in and `(player, -1, n)`
        // for money out, so the Done handler can raise `payAfter` for both and
        // open the `paid` counteraction window only for the loss.
        if got != 0 {
            // Visible outcome: `commit_after` raises `payAfter` / `paid`.
            self.flush_header();
        }
        if got > 0 {
            self.paid_log.push((-1, player_id, got));
        } else if got < 0 {
            self.paid_log.push((player_id, -1, -got));
        }
        got
    }
    fn set_card_face_down(&mut self, player_id: i32, card: &str, down: bool) -> bool {
        self.world.set_card_face_down(player_id, card, down)
    }

    fn do_move_roll(&mut self, player_id: i32) -> i32 {
        self.world.do_move_roll(player_id)
    }

    fn log(&mut self, player_id: i32, msg: Msg) {
        let msg = self.attribute(msg);
        self.post_log("text", player_id, msg, "");
    }

    // -- flash (v55, once-per-flash identity) ---------------------------------

    /// Request the activation's flash at this point (overrides `Auto`; a
    /// no-op under `Flash::Never`). Flushes the lazy header first -- a flash
    /// is a visible outcome and the header names which clause is ficycle. A
    /// `QuietWhenEmpty` entry keeps its header once a flash was requested.
    ///
    /// Posts a `"flash"` event (once-per-flash) so a prompt's re-run does not
    /// play the same cue again.
    fn flash(&mut self) {
        // Never > ctx::no_flash > ctx::flash / Always > Auto.
        if self.flash_policy == card_sdk::abi::Flash::Never {
            return;
        }
        // Lazy header: post the 「效果适用」 line at the flash point so the
        // player sees why the card flashed (a bare face with no why is noise).
        self.flush_header();
        self.world.flash.lock().requested = true;
        // A `"flash"` cue for THIS activation, once-per-flash.
        let ord = self.flash_ordinal;
        self.flash_ordinal += 1;
        let key = self.flash_key(&format!("ord:{ord}"));
        let card = self
            .current_card
            .strip_prefix("event:")
            .unwrap_or(&self.current_card)
            .to_string();
        self.post_flash(key, &card, Msg::new("log.flash_cue"), -1);
    }

    /// Suppress the activation's flash (runtime decision). Wins over
    /// `ctx::flash` / `Flash::Always`, loses to `Flash::Never`.
    fn no_flash(&mut self) {
        self.world.flash.lock().no_flash = true;
    }

    /// Proactively flash another card as a "look here" cue. Its own `"flash"`
    /// event, parent-linked under the activation, identified by the next
    /// flash ordinal within the drive.
    fn flash_card(&mut self, card_id: &str, msg: Msg) {
        let ord = self.flash_ordinal;
        self.flash_ordinal += 1;
        let key = self.flash_key(&format!("ord:{ord}"));
        self.post_flash(key, card_id, msg, -1);
    }

    /// Like [`Self::flash_card`] with an explicit identity key.
    fn flash_card_keyed(&mut self, key: &str, card_id: &str, msg: Msg) {
        let k = self.flash_key(&format!("key:{key}"));
        self.post_flash(k, card_id, msg, -1);
    }

    /// Proactively flash a tile as a "look here" cue.
    fn flash_tile(&mut self, tile: i32, msg: Msg) {
        let ord = self.flash_ordinal;
        self.flash_ordinal += 1;
        let key = self.flash_key(&format!("ord:{ord}"));
        self.post_flash(key, "", msg, tile);
    }

    // board -----------------------------------------------------------------
    fn tile_count(&self) -> i32 {
        self.data.tiles.len() as i32
    }
    fn tile_named(&self, name: &str) -> i32 {
        self.world.tile_named(&self.data, name)
    }
    fn tile_name(&self, tile: i32) -> String {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.name.replace('\n', ""))
            .unwrap_or_default()
    }
    fn tile_owner(&self, tile: i32) -> i32 {
        self.world.tile_owner(tile)
    }
    fn player_pos(&self, player_id: i32) -> i32 {
        self.world.player_pos(player_id)
    }
    fn tile_steps_ahead(&self, player_id: i32, steps: i32) -> i32 {
        self.world.tile_steps_ahead(&self.data, player_id, steps)
    }
    fn rent_of(&self, tile: i32) -> i32 {
        self.world.rent_of(&self.data, tile)
    }
    fn buy_price(&self, tile: i32) -> i32 {
        self.world.buy_price(&self.data, tile)
    }

    /// HOST-07: the same hook-aware quote `buy` charges (`CardRules::buy_quote`
    /// = `BuyGate` / `BuyAdd` / `BuyMul` / `BuySet`), not the native
    /// land+houses figure. Reached through the live `PlayCtx` the drive installed
    /// (`Run::live`, same raw-pointer contract as the log sink). Falls back to
    /// the native formula outside a drive (probes, quotes).
    fn buy_quotes(&self, player_id: i32, kind: i32, tiles: &[i32]) -> Vec<(i32, bool)> {
        use game_core::engine::purchase::{self, BuyKind};
        let kind = BuyKind::from_i32(kind).unwrap_or(BuyKind::Card);
        if let Some(live) = self.live {
            let cx: &game_core::engine::PlayCtx = unsafe { &*live };
            let w = cx.world();
            let specs: Vec<(usize, i32)> = tiles
                .iter()
                .map(|&t| {
                    let t = t.max(0) as usize;
                    let seller = match kind {
                        BuyKind::Force | BuyKind::Acquire => {
                            w.st.owners.get(t).copied().unwrap_or(-1)
                        }
                        _ => -1,
                    };
                    (t, seller)
                })
                .collect();
            return purchase::quote_for(
                cx.rules(),
                w,
                cx.data(),
                player_id.max(0) as usize,
                kind,
                &specs,
            )
            .into_iter()
            .map(|q| (q.price, q.eligible))
            .collect();
        }
        tiles
            .iter()
            .map(|&t| {
                if self.tile_in(-1, t, "buyable") {
                    (self.buy_price(t), true)
                } else {
                    (-1, false)
                }
            })
            .collect()
    }
    fn build_cost(&self, tile: i32) -> i32 {
        self.world.build_cost(&self.data, tile)
    }
    fn build_price(&self, tile: i32) -> i32 {
        self.world.build_price(&self.data, &self.world.turn, tile)
    }
    fn build_cost_percent(&self) -> i32 {
        self.world.turn.build_cost_pct
    }
    fn build_discount(&self) -> i32 {
        self.world.turn.build_discount
    }
    fn tile_buildable(&self, player_id: i32, tile: i32) -> bool {
        self.world
            .guard_buildable(&self.data, player_id, tile)
            .is_none()
    }
    fn mortgage_value(&self, tile: i32) -> i32 {
        self.world.mortgage_value(&self.data, tile)
    }
    fn owned_count(&self, player_id: i32) -> i32 {
        self.world.owned_tiles(player_id).len() as i32
    }
    fn owned_at(&self, player_id: i32, index: i32) -> i32 {
        self.world
            .owned_tiles(player_id)
            .get(index.max(0) as usize)
            .copied()
            .unwrap_or(-1)
    }

    // players -----------------------------------------------------------------
    fn player_count(&self) -> i32 {
        self.world.st.players.len() as i32
    }
    fn player_out(&self, player_id: i32) -> i32 {
        self.world.player_out(player_id) as i32
    }
    fn others_count(&self, player_id: i32) -> i32 {
        self.world.others(player_id).len() as i32
    }
    fn others_at(&self, player_id: i32, index: i32) -> i32 {
        self.world
            .others(player_id)
            .get(index.max(0) as usize)
            .copied()
            .unwrap_or(-1)
    }
    fn money(&self, player_id: i32) -> i32 {
        self.world.player_money(player_id)
    }
    fn gain(&mut self, player_id: i32, amount: i32, src: Msg) -> i32 {
        let got = self.world.gain_money(player_id, amount, src.clone());
        if got != 0 {
            let msg = Msg::new("log.gain")
                .player_id("who", player_id)
                .n("amount", got as i64)
                .msg("src", Msg::new("log.part.reason").msg("reason", src));
            self.post_log_value("gain", player_id, msg, "", got);
        }
        if got > 0 {
            self.paid_log.push((-1, player_id, got));
        }
        got
    }
    fn pay(&mut self, player_id: i32, amount: i32, src: Msg) -> i32 {
        let paid = self.world.pay_money(player_id, amount, src.clone());
        if paid != 0 {
            let msg = Msg::new("log.lose")
                .player_id("who", player_id)
                .n("amount", paid as i64)
                .msg("src", Msg::new("log.part.reason").msg("reason", src));
            self.post_log_value("lose", player_id, msg, "", -paid);
        }
        if paid > 0 {
            self.paid_log.push((player_id, -1, paid));
        }
        paid
    }
    /// Raw money adjustment -- `money += delta` with a `log.gain` / `log.lose`
    /// line, **no** `paid_log` entry (so no `payAfter`) and **no** money-lock
    /// check. 规则书 「不受其他任何效果影响」.
    fn adjust_money_raw(&mut self, player_id: i32, delta: i32, src: Msg) -> i32 {
        if delta >= 0 {
            let got = self.world.gain_money(player_id, delta, src.clone());
            if got != 0 {
                let msg = Msg::new("log.gain")
                    .player_id("who", player_id)
                    .n("amount", got as i64)
                    .msg("src", Msg::new("log.part.reason").msg("reason", src));
                self.post_log_value("gain", player_id, msg, "", got);
            }
            got
        } else {
            let amount = -delta;
            let Some(s) = self.world.st.players.get_mut(player_id.max(0) as usize) else {
                return 0;
            };
            if s.out() {
                return 0;
            }
            let lost = amount.min(s.money).max(0);
            s.money -= lost;
            if lost != 0 {
                let msg = Msg::new("log.lose")
                    .player_id("who", player_id)
                    .n("amount", lost as i64)
                    .msg("src", Msg::new("log.part.reason").msg("reason", src));
                self.post_log_value("lose", player_id, msg, "", -lost);
            }
            lost
        }
    }

    // hand / deck -----------------------------------------------------------
    fn draw(&mut self, player_id: i32, n: i32) -> i32 {
        run_draw(self, player_id, n, false)
    }
    fn draw_over(&mut self, player_id: i32, n: i32) -> i32 {
        run_draw(self, player_id, n, true)
    }
    fn add_to_dest(&mut self, player_id: i32, card: &str, dest: i32, counted: bool) {
        match dest {
            // 「加入弃卡区」 is a create-only push: unlike `to_discard` it must not
            // first consume a same-named hand copy (PERFECT step 2).
            0 => {
                let idx = player_id.max(0) as usize;
                self.flush_header();
                if let Some(h) = self.world.hidden.get_mut(idx) {
                    h.graveyard.push(card.to_string());
                }
                self.discard_log.push((player_id, card.to_string()));
                if self.world.refill_after_pile_write(idx) {
                    self.reshuffle_log.push(player_id);
                }
            }
            1 => {
                if card.starts_with("tile:") {
                    eprintln!(
                        "ADD_TO_HAND pid={player_id} card={card} hands={:?}",
                        self.world
                            .hidden
                            .iter()
                            .map(|h| h.hand.len())
                            .collect::<Vec<_>>()
                    );
                }
                self.world.add_to_hand(player_id, card);
                // 「…加入手牌时」 -- raised once the run commits unless the
                // caller was the draw routine (`counted`).
                self.hand_log.push((player_id, card.to_string(), counted));
                // 「此次加手视为抽卡动作」 -- a `DrewBefore` replacement's add-to-hand
                // *is* the draw: the after points (`drawn` / `drew`) run for it
                // (ABI v30, [`TriggerKind::DrewBefore`]). Any other add-to-hand (a
                // bounce, a tutor) is not a draw and must not raise them.
                if self.trigger.kind == TriggerKind::DrewBefore {
                    // Visible outcome: `commit_after` raises `drawn` / `drew`.
                    self.flush_header();
                    self.draw_log.push((player_id, card.to_string()));
                }
            }
            // Deck fates: 4 = top = draw.push (the end of the vec is the top),
            // 5 = bottom = Insert(0), 6 = shuffle in.
            4 | 5 | 6 => {
                let idx = player_id.max(0) as usize;
                if self.world.hidden.get(idx).is_none() {
                    return;
                }
                match dest {
                    5 => self.world.hidden[idx].deck.insert(0, card.to_string()),
                    6 => {
                        self.world.hidden[idx].deck.push(card.to_string());
                        let mut draw = std::mem::take(&mut self.world.hidden[idx].deck);
                        self.world.rng.shuffle(&mut draw);
                        self.world.hidden[idx].deck = draw;
                    }
                    _ => self.world.hidden[idx].deck.push(card.to_string()),
                }
            }
            // Field / Banished are not piles: `place_card` / "gone" is the
            // right verb. The hostcall layer traps; this is the non-trapping
            // trait edge.
            _ => {}
        }
    }
    fn take_card(&mut self, player_id: i32, card: &str, src: CardPile) -> bool {
        if src == CardPile::Field {
            return self.world.unplace_card(player_id, card).is_some();
        }
        // An inspection pile is addressed by deck owner; innermost frame
        // first (the look a body is inside is the newest one).
        if src == CardPile::Inspection {
            for frame in self.world.inspection_frames.iter_mut().rev() {
                if frame.deck_owner != player_id {
                    continue;
                }
                if let Some(k) = frame.cards.iter().position(|c| c.id == card) {
                    frame.cards.remove(k);
                    return true;
                }
            }
            return false;
        }
        let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) else {
            return false;
        };
        let list = match src {
            CardPile::Hand => &mut h.hand,
            CardPile::Graveyard => &mut h.graveyard,
            _ => &mut h.deck,
        };
        let took = match list.iter().position(|c| c == card) {
            Some(k) => {
                list.remove(k);
                // A look at the deck holds its refill (design G11 I2).
                if src == CardPile::Deck
                    && self
                        .world
                        .refill_after_pile_write(player_id.max(0) as usize)
                {
                    // Visible outcome: `commit_after` raises `reshuffled`.
                    self.flush_header();
                    self.reshuffle_log.push(player_id);
                }
                true
            }
            None => false,
        };
        if took && src == CardPile::Hand {
            self.world
                .settle_over_hand_allowance(player_id.max(0) as usize);
        }
        took
    }
    fn cards_in(&self, player_id: i32, pile: CardPile) -> Vec<String> {
        if pile == CardPile::Inspection {
            // Revealed entries only, front-first; the frame is top-first and
            // the innermost (newest) frame holds the card nearest the front.
            let idx = player_id;
            return self
                .world
                .inspection_frames
                .iter()
                .rev()
                .filter(|f| f.deck_owner == idx)
                .flat_map(|f| f.cards.iter())
                .filter(|c| c.revealed)
                .map(|c| c.id.clone())
                .collect();
        }
        let Some(h) = self.world.hidden.get(player_id.max(0) as usize) else {
            return Vec::new();
        };
        match pile {
            CardPile::Hand => h.hand.clone(),
            CardPile::Graveyard => h.graveyard.clone(),
            // The end of the vec is the top of the pile; list top first.
            CardPile::Deck => h.deck.iter().rev().cloned().collect(),
            CardPile::Field => self.world.placed_cards(player_id),
            CardPile::Inspection => unreachable!(),
        }
    }
    // inspection end / reveals (design G11 §5.4, §5.6) ----------------------
    /// Close the caller's innermost frame. Applied directly: the SDK wrapper
    /// returns a bare `i32`, so a sentinel here would be swallowed by any
    /// body that does not end on the call.
    fn inspection_end(&mut self, player: i32, shuffle: bool) -> i32 {
        let released = self.world.finish_inspection_of(player, shuffle);
        if released > 0 {
            self.post_log_value(
                "text",
                player,
                Msg::new("log.inspection_returned").n("count", released as i64),
                "",
                0,
            );
        }
        released
    }

    /// Publish a whole pile. Applied directly: a reveal is a view overlay,
    /// not a pause point.
    fn reveal_pile(&mut self, player: i32, pile: CardPile, reason: Msg) {
        let (view, ids) = {
            let world = &self.world;
            match pile {
                CardPile::Hand => (
                    game_core::state::RevealPile::Hand,
                    hidden_pile(world, player, CardPile::Hand).unwrap_or_default(),
                ),
                CardPile::Graveyard => (
                    game_core::state::RevealPile::Discard,
                    hidden_pile(world, player, CardPile::Graveyard).unwrap_or_default(),
                ),
                CardPile::Deck => {
                    let mut ids = hidden_pile(world, player, CardPile::Deck).unwrap_or_default();
                    // The open frames are the true top of the deck.
                    ids.splice(0..0, inspection_held(world, player));
                    (game_core::state::RevealPile::Deck, ids)
                }
                // Field is `placed_cards` (no pile order) and the inspection
                // frame is the look itself; `reveal_card` is the verb there.
                _ => return,
            }
        };
        self.post_reveal(player, reason, view, ids, "", pile as i32);
    }
    /// Publish one card id, appended to the seat's revealed cards. An id that
    /// is already published (a re-run of the same pass, or a card in a public
    /// look) is a no-op: one overlay entry and one line.
    fn reveal_card(&mut self, player: i32, card: &str, reason: Msg) {
        let seen = {
            let world = &self.world;
            world
                .reveals
                .iter()
                .find(|r| r.player == player)
                .is_some_and(|r| r.deck.iter().any(|c| c == card))
                // A public frame's turned cards are part of the same overlay
                // (`public_state` splices them in).
                || world
                    .inspection_frames
                    .iter()
                    .rev()
                    .filter(|f| f.deck_owner == player && f.public)
                    .flat_map(|f| f.cards.iter())
                    .filter(|c| c.revealed)
                    .any(|c| c.id == card)
        };
        // Already published (a replay of the same pass): keep both the
        // overlay entry and the log line single.
        if seen {
            return;
        }
        self.post_reveal(
            player,
            reason,
            game_core::state::RevealPile::Card,
            vec![card.to_string()],
            card,
            CardPile::Inspection as i32,
        );
    }
    fn to_discard(&mut self, player_id: i32, card: &str) {
        // Visible outcome: `commit_after` raises `discarded` (and maybe
        // `reshuffled`). The header names the clause that discarded.
        self.flush_header();
        if self.world.to_discard(player_id, card) {
            self.reshuffle_log.push(player_id);
        }
        self.discard_log.push((player_id, card.to_string()));
    }

    // field cards -----------------------------------------------------------
    fn can_build_on(&self, player_id: i32, tile: i32) -> bool {
        self.world
            .cant_build_on(&self.data, player_id, tile)
            .is_none()
    }
    fn card_face_down(&self, player_id: i32, card: &str) -> bool {
        self.world.card_face_down(player_id, card)
    }
    fn place_card_on(&mut self, player_id: i32, tile: i32, card: &str, note: Msg) -> i32 {
        let mut props = props_of(&self.props, card);
        let caps = split_counter_max(&mut props);
        let uid = self
            .world
            .place_card_on(&self.data, player_id, tile, card, note, props);
        self.stamp_owner(uid, card);
        if uid >= 0 {
            self.outcome(
                player_id,
                Msg::new("log.outcome.place")
                    .player_id("who", player_id)
                    .card("card", card.to_string()),
            );
        }
        if uid >= 0 {
            for (name, cap) in caps {
                self.world.set_counter_max_at(uid, &name, cap);
            }
        }
        if card == self.current_card {
            self.current_uid = uid;
        }
        uid
    }
    fn place_card(&mut self, player_id: i32, card: &str, note: Msg) -> i32 {
        let mut props = props_of(&self.props, card);
        let caps = split_counter_max(&mut props);
        let uid = self
            .world
            .place_card(&self.data, player_id, card, note, props);
        self.stamp_owner(uid, card);
        if uid >= 0 {
            self.outcome(
                player_id,
                Msg::new("log.outcome.place")
                    .player_id("who", player_id)
                    .card("card", card.to_string()),
            );
        }
        if uid >= 0 {
            for (name, cap) in caps {
                self.world.set_counter_max_at(uid, &name, cap);
            }
        }
        if card == self.current_card {
            self.current_uid = uid;
        }
        uid
    }
    fn set_dest(&mut self, dest: i32, cause: i32) {
        self.dest = dest;
        self.dest_to = None;
        self.dest_cause = Some(cause);
    }
    fn set_transfer_to_dest(&mut self, to: i32, dest: i32, cause: i32) {
        self.dest = dest;
        self.dest_to = Some(to);
        self.dest_cause = Some(cause);
    }
    fn send_to_dest(&mut self, dest: i32, cause: i32) -> Option<i32> {
        self.move_now(None, dest, cause)
    }
    fn transfer_to_dest(&mut self, to: i32, dest: i32, cause: i32) -> Option<i32> {
        self.move_now(Some(to), dest, cause)
    }
    fn rent_unit_mult(&self) -> i32 {
        self.world.rent_unit_mult()
    }
    fn add_rent_unit_bonus(&mut self, n: i32) -> i32 {
        self.world.add_rent_unit_bonus(n)
    }
    fn teleport_to(&mut self, player_id: i32, tile: i32) {
        self.world.teleport_to(player_id, tile);
    }
    fn unplace_card_named(&mut self, player_id: i32, card: &str) -> bool {
        self.world.unplace_card(player_id, card).is_some()
    }
    fn unplace_card(&mut self) -> i32 {
        let card = self.current_card.clone();
        self.carried_owner = self.world.field_by_uid(self.current_uid).map(|f| f.owner);
        let owner = self.world.unplace_at(self.current_uid);
        if owner >= 0 {
            self.current_uid = -1;
            self.outcome(owner, Msg::new("log.outcome.unplace").card("card", card));
        }
        owner
    }
    fn event_expire(&mut self, id: &str, removed: bool) {
        self.world.expire_event(id, removed);
    }
    fn event_is_active(&self, id: &str) -> bool {
        self.world.event_is_active(id)
    }
    fn event_deck_push(&mut self, id: &str, face_down: bool) {
        self.world.event_deck_push(id, face_down);
    }
    fn event_banish(&mut self, id: &str) {
        self.world.event_banish(id);
    }
    fn self_tile(&self) -> i32 {
        if self.current_uid < 0 {
            return -2;
        }
        self.world.self_tile_of(self.current_uid)
    }
    fn self_position(&self) -> i32 {
        self.world.self_position_of(self.current_uid)
    }
    fn set_self_tile(&mut self, tile: i32) -> bool {
        self.world.set_tile_at(self.current_uid, tile)
    }
    fn self_face_down(&self) -> bool {
        self.world.is_face_down_at(self.current_uid)
    }
    fn set_self_face_down(&mut self, on: bool, _cause: i32) -> bool {
        self.world.set_face_down_at(self.current_uid, on)
    }
    fn self_immune(&self) -> bool {
        self.world.is_immune_at(self.current_uid)
    }
    fn set_self_immune(&mut self, on: bool) -> bool {
        self.world.set_immune_at(self.current_uid, on)
    }
    fn set_self_immune_except_owner(&mut self, on: bool) -> bool {
        self.world.set_immune_except_owner_at(self.current_uid, on)
    }
    fn placed_cards(&self, player_id: i32) -> Vec<String> {
        self.world.placed_cards(player_id)
    }
    fn card_text_mentions(&self, card: &str, needle: &str) -> bool {
        if let Some(c) = self.data.card(card) {
            return c.text.contains(needle);
        }
        // A `skill:<owner>:<skill>` rule id is not in `cards`; the skill's
        // text is the owner's (persona's or group's) `text` field (a bear-cookie
        // emote targeting 「every player whose skills mention charge pots」).
        if let Some(rest) = card.strip_prefix("skill:") {
            if let Some((owner, _)) = rest.split_once(':') {
                if let Some(c) = self.data.persona(owner) {
                    return c.text.contains(needle);
                }
                if let Some(b) = self.data.groups.iter().find(|b| b.name == owner) {
                    return b.text.contains(needle);
                }
            }
        }
        false
    }
    fn card_counter(&self, player_id: i32, card: &str, name: &str) -> i32 {
        let uid = self
            .world
            .field_instances(player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
        self.world.counter_at(uid, name)
    }
    fn counter_names_at(&self, uid: i32) -> Vec<String> {
        self.world.counter_names_at(uid)
    }
    fn add_card_counter(
        &mut self,
        player_id: i32,
        card: &str,
        name: &str,
        n: i32,
        max: i32,
    ) -> i32 {
        let uid = self
            .world
            .field_instances(player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
        let was = self.world.counter_at(uid, name);
        let now = self.world.add_counter_at(uid, name, n, max);
        self.note_counter_changed(uid, was, now, name);
        now
    }
    /// Is the running **instance** in play?
    ///
    /// This used to ask whether *some* card with the running card's name sat at
    /// `player_id`, which under the uid model answers the wrong question: with
    /// two copies of a card on one field, the copy that is not running would
    /// still pass the gate. It is the instance now, so it takes no locator --
    /// the run already knows which instance it is.
    fn is_placed(&self) -> i32 {
        (self.current_uid >= 0 && self.world.field_by_uid(self.current_uid).is_some()) as i32
    }
    fn counter(&self, name: &str) -> i32 {
        self.world.counter_at(self.current_uid, name)
    }
    fn self_pool(&self, name: &str) -> i32 {
        self.world.pool_at(self.current_uid, name)
    }
    fn self_prop(&self, key: &str) -> i32 {
        self.world.prop_at(self.current_uid, key)
    }
    fn set_self_prop(&mut self, key: &str, value: i32) -> i32 {
        self.world.set_prop_at(self.current_uid, key, value)
    }
    fn set_counter(&mut self, name: &str, n: i32) -> i32 {
        let uid = self.current_uid;
        let was = self.world.counter_at(uid, name);
        let now = self.world.set_counter_at(uid, name, n);
        self.note_counter_changed(uid, was, now, name);
        // A silent entry's counter is bookkeeping (`outcome_plan` already
        // suppresses its lines the same way).
        if now != was && !self.silent_body {
            let who = self.trigger.player_id;
            self.outcome(
                who,
                Msg::new("log.outcome.counter")
                    .card("card", self.current_card.clone())
                    .msg("name", crate::trigger_reason::counter_name(name))
                    .i("value", now - was),
            );
        }
        now
    }
    fn add_counter(&mut self, name: &str, n: i32, max: i32) -> i32 {
        let uid = self.current_uid;
        let was = self.world.counter_at(uid, name);
        let now = self.world.add_counter_at(uid, name, n, max);
        self.note_counter_changed(uid, was, now, name);
        // A silent entry's counter is bookkeeping (`outcome_plan` already
        // suppresses its lines the same way).
        if now != was && !self.silent_body {
            let who = self.trigger.player_id;
            self.outcome(
                who,
                Msg::new("log.outcome.counter")
                    .card("card", self.current_card.clone())
                    .msg("name", crate::trigger_reason::counter_name(name))
                    .i("value", now - was),
            );
        }
        now
    }
    fn counter_at(&self, uid: i32, name: &str) -> i32 {
        self.world.counter_at(uid, name)
    }
    fn add_counter_at(&mut self, uid: i32, name: &str, n: i32, max: i32) -> i32 {
        let was = self.world.counter_at(uid, name);
        let now = self.world.add_counter_at(uid, name, n, max);
        self.note_counter_changed(uid, was, now, name);
        now
    }
    fn counter_max_at(&self, uid: i32, name: &str) -> i32 {
        self.world.counter_max_at(uid, name)
    }
    fn set_counter_max_at(&mut self, uid: i32, name: &str, max: i32) -> i32 {
        self.world.set_counter_max_at(uid, name, max)
    }
    fn field_instances(&self, player_id: i32) -> Vec<(i32, String)> {
        let v = self.world.field_instances(player_id);
        v
    }
    fn pool_at(&self, uid: i32, name: &str) -> i32 {
        self.world.pool_at(uid, name)
    }
    fn self_uid(&self) -> i32 {
        self.current_uid
    }
    fn current_card(&self) -> &str {
        &self.current_card
    }
    fn set_self_uid(&mut self, uid: i32) {
        self.current_uid = uid;
    }
    fn prop_at(&self, uid: i32, key: &str) -> i32 {
        self.world.prop_at(uid, key)
    }
    fn set_prop_at(&mut self, uid: i32, key: &str, value: i32) -> i32 {
        if uid < 0 {
            // No field instance to carry the prop (a hand play, or the run is
            // binding a lingering instance). Park it for `ctx::linger`, which
            // puts it on the turn-scoped instance (`docs/PURCHASE.md` P5).
            self.linger_props.insert(key.to_string(), value);
            return value;
        }
        self.world.set_prop_at(uid, key, value)
    }
    fn tile_prop(&self, tile: i32, key: &str) -> i32 {
        self.world.tile_prop(tile, key)
    }
    fn set_tile_prop(&mut self, tile: i32, key: &str, value: i32) -> i32 {
        self.world.set_tile_prop(tile, key, value)
    }
    fn unplace_at(&mut self, uid: i32, destination: i32, _cause: i32) -> i32 {
        let row = self.world.field_by_uid(uid).map(|f| {
            (
                f.card.clone(),
                if destination < 0 {
                    f.props
                        .get(game_core::state::prop::LEAVE_DEST)
                        .copied()
                        .unwrap_or(0)
                } else {
                    destination
                },
            )
        });
        let owner = self.world.unplace_at(uid);
        if owner < 0 {
            return -1;
        }
        // The pile half of a direct (non-request) leave: the `unplace_at`
        // import itself pauses, so this is only the trait fallback.
        if let Some((card, dest)) = row {
            if dest != 2 {
                self.add_to_dest(owner, &card, dest, false);
            }
        }
        owner
    }
    fn tile_at(&self, uid: i32) -> i32 {
        self.world.tile_at(uid)
    }
    fn tile_standing_at(&self, position: i32) -> i32 {
        self.world.tile_standing_at(position)
    }
    fn has_swaps(&self) -> bool {
        self.world.has_swaps()
    }
    fn position_of_tile(&self, tile: i32) -> i32 {
        self.world.position_of_tile(tile)
    }
    fn tile_extra_count(&self, tile: i32) -> i32 {
        self.world.tile_extra_count(tile)
    }
    fn instance_position(&self, uid: i32) -> i32 {
        self.world.position_of_instance(uid)
    }
    fn set_tile_at(&mut self, uid: i32, tile: i32) -> bool {
        self.world.set_tile_at(uid, tile)
    }
    fn is_face_down_at(&self, uid: i32) -> bool {
        self.world.is_face_down_at(uid)
    }
    fn is_own_placement(&self, uid: i32) -> bool {
        self.place_log.contains(&uid)
    }
    fn set_face_down_at(&mut self, uid: i32, on: bool, _cause: i32) -> bool {
        self.world.set_face_down_at(uid, on)
    }
    fn is_immune_at(&self, uid: i32) -> bool {
        self.world.is_immune_at(uid)
    }
    fn set_immune_at(&mut self, uid: i32, on: bool) -> bool {
        self.world.set_immune_at(uid, on)
    }
    fn is_immune_from_at(&self, uid: i32, by: i32) -> bool {
        self.world.is_immune_from_at(uid, by)
    }
    fn set_immune_except_owner_at(&mut self, uid: i32, on: bool) -> bool {
        self.world.set_immune_except_owner_at(uid, on)
    }

    // marks & bound units (user ruling 2026-10-10) ---------------------------
    /// Bind `count` units of the running instance's counter `kind` to `tile`.
    /// Marker ownership (user ruling 2026-10-07): the rule that creates a
    /// mark owns it -- `skill:要乐奈` owns every 抹茶芭菲 copy, on any tile
    /// or player counter. `instance` is stamped as the current uid and `src`
    /// (provenance, -1 → current uid). Returns the mark row's uid (the new
    /// row's, or the merged row's), or `-1` when `count <= 0`.
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
        let who = self.current_card.clone();
        self.world.note_marker_owner(kind, &who);
        let me = self.current_uid;
        let src = if src < 0 { me } else { src };
        let uid = self
            .world
            .place_mark(me, kind, category, tile, owner, src, count, note, fresh);
        if uid >= 0 && count > 0 {
            let who = self.trigger.player_id;
            // A designate never prints its wire kind (`mark:designate`) -- the
            // player sees the readable outcome 「指定了 {{tile}}」. Other kinds
            // keep the generic placement line (the view resolves their i18n
            // key; the tile-mark owner is labelled from its category).
            if category == game_core::state::mark_category::DESIGNATE {
                self.outcome(
                    who,
                    Msg::new("log.outcome.designate")
                        .tile("tile", tile)
                        .i("n", count as i64),
                );
            } else {
                // `kind` is a display name or an i18n key (`cards:…`).
                // Nested Msg: the view resolves keys and falls back to the
                // literal, so a raw key never reaches the player.
                self.outcome(
                    who,
                    Msg::new("log.outcome.mark_add")
                        .tile("tile", tile)
                        .msg("kind", Msg::new(kind))
                        .i("n", count as i64),
                );
            }
        }
        uid
    }
    /// Place **one** paired mark spanning tiles `a` and `b` (one row per tile,
    /// each row's `partner` naming the other). Atomic place-or-refuse: returns
    /// `false` and places nothing when either end already carries a matching
    /// mark for that owner. Stamps `instance` / `src` like [`Self::place_mark`].
    fn place_mark_pair(
        &mut self,
        a: i32,
        b: i32,
        kind: &str,
        category: &str,
        owner: i32,
        src: i32,
        count: i32,
        note: Msg,
        fresh: bool,
    ) -> bool {
        let _ = fresh; // pair rows are always one-per-tile; the refusal is the cap
        let who = self.current_card.clone();
        self.world.note_marker_owner(kind, &who);
        let me = self.current_uid;
        let src = if src < 0 { me } else { src };
        let ok = self
            .world
            .place_mark_pair(me, kind, category, a, b, owner, src, count, note);
        if ok {
            let who = self.trigger.player_id;
            for tile in [a, b] {
                self.outcome(
                    who,
                    Msg::new("log.outcome.mark_add")
                        .tile("tile", tile)
                        .msg("kind", Msg::new(kind))
                        .i("n", count as i64),
                );
            }
        }
        ok
    }
    fn count_marks(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        self.world.count_marks(tile, filter)
    }
    fn bump_mark(
        &mut self,
        tile: i32,
        filter: &game_core::state::MarkFilter<'_>,
        delta: i32,
    ) -> i32 {
        let got = self.world.bump_mark(tile, filter, delta);
        if got != 0 {
            let who = self.trigger.player_id;
            let key = if got > 0 {
                "log.outcome.mark_add"
            } else {
                "log.outcome.mark_remove"
            };
            self.outcome(
                who,
                Msg::new(key)
                    .tile("tile", tile)
                    .msg("kind", Msg::new("log.mark.generic"))
                    .i("n", got.abs() as i64),
            );
        }
        got
    }
    fn remove_marks(&mut self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        // `tile < 0`: every tile, narrated as one line instead of one per tile.
        if tile < 0 {
            let mut total = 0;
            for t in 0..self.data.tiles.len() as i32 {
                total += self.world.remove_marks(t, filter);
            }
            if total != 0 {
                let who = self.trigger.player_id;
                self.outcome(
                    who,
                    Msg::new("log.outcome.mark_remove_all")
                        .msg("kind", crate::trigger_reason::mark_kind_name(filter.kind))
                        .i("n", total as i64),
                );
            }
            return total;
        }
        let got = self.world.remove_marks(tile, filter);
        if got != 0 {
            let who = self.trigger.player_id;
            self.outcome(
                who,
                Msg::new("log.outcome.mark_remove")
                    .tile("tile", tile)
                    .msg("kind", crate::trigger_reason::mark_kind_name(filter.kind))
                    .i("n", got.abs() as i64),
            );
        }
        got
    }
    fn mark_src_at(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        self.world.mark_src_at(tile, filter)
    }
    fn mark_instance_at(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        self.world.mark_instance_at(tile, filter)
    }
    fn mark_partner(&self, tile: i32, filter: &game_core::state::MarkFilter<'_>) -> i32 {
        self.world.mark_partner(tile, filter)
    }
    fn mark_tile_by_uid(&self, uid: i32) -> i32 {
        self.world.mark_tile_by_uid(uid)
    }
    fn remove_mark_by_uid(&mut self, uid: i32) -> bool {
        let tile = self.world.mark_tile_by_uid(uid);
        let got = self.world.remove_mark_by_uid(uid);
        if got {
            let who = self.trigger.player_id;
            self.outcome(
                who,
                Msg::new("log.outcome.mark_remove")
                    .tile("tile", tile)
                    .msg("kind", Msg::new("log.mark.generic"))
                    .i("n", 1),
            );
        }
        got
    }
    /// Units of the running instance's counter `name` held by `player_id`.
    fn count_held(&self, name: &str, player_id: i32) -> i32 {
        self.world.count_held(self.current_uid, name, player_id)
    }
    fn add_held(&mut self, name: &str, player_id: i32, n: i32, max: i32) -> i32 {
        // Marker ownership: the rule that creates a marker owns it.
        let who = self.current_card.clone();
        self.world.note_marker_owner(name, &who);
        let got = self
            .world
            .bind_held(self.current_uid, name, player_id, n, max);
        // Post-commit `markerChanged` (HAG-01): the holder is the raise seat.
        // `instance` is the running card -- the rule that owns the token.
        self.marker_changed_log
            .push((player_id, got, name.to_string(), self.current_uid));
        got
    }
    fn count_held_name(&self, name: &str, player_id: i32) -> i32 {
        // Name-keyed (any owner / instance): the legacy `held` lookup.
        self.world.held_count(player_id, name)
    }
    fn set_held_name(&mut self, name: &str, player_id: i32, v: i32, max: i32, instance: i32) {
        let who = self.current_card.clone();
        self.world.note_marker_owner(name, &who);
        // Stamp the owning instance on a new row (user ruling 2026-10-10).
        // `instance < 0` keeps an existing row's owner and falls back to the
        // running card for a fresh row.
        let inst = if instance >= 0 {
            instance
        } else {
            self.current_uid
        };
        let was = self.world.held_count(player_id, name);
        self.world.set_held(player_id, name, v, inst, max);
        // Post-commit `markerChanged` (HAG-01), even at delta 0: a raw set is
        // a write the holder's count hears about (the fire kernel re-declares
        // its row this way).
        self.marker_changed_log
            .push((player_id, v - was, name.to_string(), inst));
    }
    fn held_max(&self, player_id: i32, name: &str) -> i32 {
        self.world.held_max(player_id, name)
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
        let got = self.world.move_units(
            self.current_uid,
            name,
            from_tile,
            from_player,
            to_tile,
            to_player,
            n,
        );
        // The player-side legs are bound held writes: raise `markerChanged`
        // (HAG-01) after the run commits, one per leg the move actually made.
        if got > 0 && from_player >= 0 {
            self.marker_changed_log
                .push((from_player, -got, name.to_string(), self.current_uid));
        }
        if got > 0 && to_player >= 0 {
            self.marker_changed_log
                .push((to_player, got, name.to_string(), self.current_uid));
        }
        got
    }
    fn mark_rule_instances(&self) -> Vec<(i32, String)> {
        self.world.mark_rule_instances()
    }
    fn status_rule_instances(&self) -> Vec<(i32, String)> {
        self.world.status_rule_instances()
    }
    fn held_names(&self, player_id: i32, prefix: &str) -> Vec<String> {
        self.world.held_names(player_id, prefix)
    }

    // keyed state ------------------------------------------------------------
    fn state_var(&self, player_id: i32, key: &str) -> game_core::state::StateVar {
        self.world.state_var(player_id, key)
    }
    fn state_get(&self, player_id: i32, key: &str) -> i32 {
        self.world.state_get(player_id, key)
    }
    fn state_pairs(&self, player_id: i32) -> Vec<(String, i32)> {
        self.world.state_pairs(player_id)
    }
    fn state_set_flags(&mut self, player_id: i32, key: &str, flags: u8) {
        self.world.state_set_flags(player_id, key, flags);
    }
    fn state_min(&self, player_id: i32, key: &str) -> i32 {
        self.world.state_min(player_id, key)
    }
    fn state_max(&self, player_id: i32, key: &str) -> i32 {
        self.world.state_max(player_id, key)
    }
    fn state_expires(&self, player_id: i32, key: &str) -> Option<game_core::state::Tick> {
        self.world.state_expires(player_id, key)
    }
    fn state_set(&mut self, player_id: i32, key: &str, value: i32) -> i32 {
        let was = self.world.state_get(player_id, key);
        let now = self.world.state_set(player_id, key, value);
        self.outcome_state(player_id, key, was, now);
        self.note_state_toggle(player_id, key, was, now);
        now
    }
    fn state_add(&mut self, player_id: i32, key: &str, delta: i32) -> i32 {
        let was = self.world.state_get(player_id, key);
        let now = self.world.state_add(player_id, key, delta);
        self.outcome_state(player_id, key, was, now);
        self.note_state_toggle(player_id, key, was, now);
        now
    }
    fn state_set_bounds(&mut self, player_id: i32, key: &str, min: i32, max: i32) {
        self.world.state_set_bounds(player_id, key, min, max);
    }
    fn state_set_expires(
        &mut self,
        player_id: i32,
        key: &str,
        expires: Option<game_core::state::Tick>,
    ) {
        self.world.state_set_expires(player_id, key, expires);
    }
    fn tick_state(&mut self, player_id: i32, when: game_core::state::Tick) -> Vec<(String, i32)> {
        self.world.tick_state(player_id, when)
    }

    // status pots -----------------------------------------------------------
    fn group_skill_uid(&self, player_id: i32) -> i32 {
        self.world.group_skill_uid(player_id)
    }
    fn persona_skill_uid(&self, player_id: i32) -> i32 {
        self.world.persona_skill_uid(player_id)
    }
    fn group_skill_id(&self, player_id: i32) -> Option<String> {
        self.world.group_skill_id(player_id)
    }
    fn persona_skill_id(&self, player_id: i32) -> Option<String> {
        self.world.persona_skill_id(player_id)
    }
    fn persona_name(&self, player_id: i32) -> Option<String> {
        // The `persona_is` spelling: `MatchPlayer::persona`.
        self.world
            .st
            .players
            .get(player_id.max(0) as usize)
            .map(|s| s.persona.clone())
            .filter(|c| !c.is_empty())
    }
    fn group_name(&self, player_id: i32) -> Option<String> {
        // The `in_group` spelling: the persona's group from the data table.
        let s = self.world.st.players.get(player_id.max(0) as usize)?;
        self.data
            .personas
            .iter()
            .find(|c| c.name == s.persona)
            .map(|c| c.group.clone())
    }
    fn group_skills(&self, player_id: i32) -> Vec<(i32, String, i32)> {
        self.world.group_skills(player_id)
    }
    fn add_group_skill(&mut self, player_id: i32, id: &str, extra: bool) -> i32 {
        let props = props_of(&self.props, id);
        self.world
            .add_group_skill(&self.data, player_id, id, extra, props)
    }
    fn gain_marker(&mut self, player_id: i32, name: &str, n: i32, reason: Msg) -> i32 {
        let got = self.world.gain_marker(player_id, name, n);
        // Minimal world event-log line (wire kind is the marker name). The
        // 效果适用 narration / cap policy is the rules-layer kernel's
        // (`bandori_std::fire`); this is only the engine's fact of the write.
        if got != 0 {
            let msg = Msg::new("log.gain_charge")
                .player_id("who", player_id)
                .i("n", got as i64);
            self.post_log_value(name, player_id, msg, "", got);
        } else if n > 0 {
            // 「不能超过持有上限」: the effect applied and the storage bound
            // refused it. Narrate so a non-kernel caller sees why nothing landed
            // (the kernel logs its own `log.gain_charge_capped` when it refuses
            // before the write).
            if let Some(p) = self.world.st.players.get(player_id.max(0) as usize) {
                let (cap, have) = (
                    self.world.held_max(player_id, name),
                    self.world.held_count(player_id, name),
                );
                if cap > 0 && have >= cap && !p.out() {
                    let msg = Msg::new("log.gain_charge_capped")
                        .player_id("who", player_id)
                        .msg("reason", reason)
                        .i("charge", have as i64)
                        .i("max", cap as i64);
                    self.post_log_value(name, player_id, msg, "", 0);
                }
            }
        }
        got
    }
    fn apply_abnormal(&mut self, _player_id: i32, _kind: i32, _n: i32, _to: i32) {
        // Split verb: `ctx::gate` + `state::add` + `raise_after` with a
        // caller-supplied name. Kept only so old guests link; new guests use
        // the split form.
    }
    fn raise_after(&mut self, player_id: i32, name: &str, value: i32) {
        // The header names the clause, so any visible outcome must flush it
        // (the same contract `post_log_value` already honours). A named raise
        // is visible: `commit_after` fires its hooks and `body_empty` counts
        // the row, so a raise-only body must still post its 「效果适用」.
        self.flush_header();
        self.named_raise_log
            .push((player_id, name.to_string(), value));
    }
    fn skip_main_move(&mut self, player_id: i32, locked: i32) {
        self.world.skip_main_move(player_id, locked);
    }
    fn give_extra_turn(&mut self, player_id: i32) {
        self.world.give_extra_turn(player_id as usize);
    }
    fn end_turn_now(&mut self, player_id: i32) {
        self.world.end_turn_now(player_id);
    }
    fn teleport_as(&mut self, player_id: i32, tile: i32, settles: i32, is_main_move: i32) {
        // Nested / stub shape: a bare position write plus the `main` flag. The
        // full move tail is `HostRequest::TeleportAs` (hostfns) ->
        // `PlayCtx::teleport_as`.
        self.world.teleport_to(player_id, tile);
        if is_main_move != 0 {
            self.world.turn.main_moved = true;
        }
        let _ = settles;
    }
    fn log_as(&mut self, player_id: i32, kind: &str, msg: Msg) {
        let msg = self.attribute(msg);
        self.post_log(kind, player_id, msg, "");
    }

    // board extensions ------------------------------------------------------
    /// ABI v56: data kind by wire spelling (ruleset tile-kind binding).
    fn tile_kind_of(&self, tile: i32) -> String {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.kind.as_str().to_string())
            .unwrap_or_default()
    }
    fn tile_is(&self, tile: i32, kind: &str) -> bool {
        crate::tile_class::kind_is(self.data.tiles.get(tile.max(0) as usize), kind)
    }
    /// ABI v56: generic field read. Replaces the attr family of `tile_group`/`tile_price`.
    fn tile_attr(&self, tile: i32, attr: &str) -> i32 {
        crate::tile_class::attr_of(self.data.tiles.get(tile.max(0) as usize), attr)
    }
    /// ABI v56: class membership (ruleset class-name bindings). Props first,
    /// then [`crate::tile_class`].
    fn tile_in(&self, player_id: i32, tile: i32, class: &str) -> bool {
        crate::tile_class::tile_in(
            self.data.tiles.get(tile.max(0) as usize),
            player_id,
            class,
            |k| self.world.tile_prop(tile, k),
        )
    }
    fn tile_class_rows(
        &self,
        tile: i32,
        classes: &[String],
        out: &mut Vec<(i64, i64, String)>,
    ) -> bool {
        crate::tile_class::class_rows(
            &self.world,
            self.data.tiles.get(tile.max(0) as usize),
            tile,
            classes,
            out,
        )
    }
    fn paid_in_settle(&self) -> i32 {
        self.world.turn.paid_in_settle
    }
    fn turn_start_pos(&self, player_id: i32) -> i32 {
        self.world
            .turn
            .turn_start_pos
            .get(player_id.max(0) as usize)
            .copied()
            .unwrap_or(-1)
    }
    fn turn_rolls(&self) -> Vec<i32> {
        self.world.turn.turn_rolls.clone()
    }
    fn set_build_discount(&mut self, n: i32, layers: i32) {
        self.world.turn.build_discount = n.max(0);
        self.world.turn.build_discount_layers = layers.max(0);
    }
    fn set_build_cost_pct(&mut self, pct: i32) {
        self.world.turn.build_cost_pct = pct.clamp(0, 100);
    }
    fn turn_snapshot(&self, player_id: i32) -> game_core::engine::TurnSnapshot {
        self.world
            .turn
            .turn_snapshot
            .get(player_id.max(0) as usize)
            .cloned()
            .unwrap_or(game_core::engine::TurnSnapshot {
                pos: -1,
                ..Default::default()
            })
    }
    fn tile_group(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.group)
            .unwrap_or(-1)
    }
    fn tile_price(&self, tile: i32) -> i32 {
        // Land price alone (`buy_price` adds houses).
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.price)
            .unwrap_or(0)
    }
    fn houses_of(&self, tile: i32) -> i32 {
        let v = self
            .world
            .st
            .houses
            .get(tile.max(0) as usize)
            .copied()
            .unwrap_or(0);
        v
    }
    fn rent_houses_of(&self, tile: i32) -> i32 {
        self.world.rent_houses(tile)
    }
    fn set_houses(&mut self, tile: i32, n: i32) {
        let cap = self
            .data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.rent.len().saturating_sub(1))
            .unwrap_or(0) as i32;
        if let Some(h) = self.world.st.houses.get_mut(tile.max(0) as usize) {
            *h = n.clamp(0, cap);
        }
    }
    fn add_house(&mut self, tile: i32, n: i32) -> i32 {
        let cap = self
            .data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.rent.len().saturating_sub(1))
            .unwrap_or(0) as i32;
        if let Some(h) = self.world.st.houses.get_mut(tile.max(0) as usize) {
            let before = *h;
            *h = (before + n).clamp(0, cap);
            let now = *h;
            let delta = now - before;
            if delta != 0 {
                let owner = self
                    .world
                    .st
                    .owners
                    .get(tile.max(0) as usize)
                    .copied()
                    .unwrap_or(-1);
                if owner >= 0 {
                    // Visible outcome: `commit_after` raises `houseAdded` /
                    // `houseRemoved` (design 5.5).
                    self.flush_header();
                    self.house_log.push((owner, tile, now, delta));
                }
            }
            return now;
        }
        0
    }
    fn mortgaged_of(&self, tile: i32) -> i32 {
        self.world
            .st
            .mortgaged
            .get(tile.max(0) as usize)
            .copied()
            .unwrap_or(false) as i32
    }
    fn set_mortgaged(&mut self, tile: i32, v: i32) {
        if let Some(m) = self.world.st.mortgaged.get_mut(tile.max(0) as usize) {
            *m = v != 0;
        }
    }
    fn set_owner(&mut self, tile: i32, player_id: i32) {
        // The acquisition ledger rides with the owner write (`rehome_card`
        // shares the writer): 灯's `firstTile` and the sequence read from it.
        self.world.set_tile_owner(tile, player_id);
    }
    fn dist(&self, a: i32, b: i32) -> i32 {
        // The shorter way around the board loop.
        let n = self.data.tiles.len() as i32;
        if n <= 0 {
            return 0;
        }
        let d = ((b - a) % n + n) % n;
        d.min(n - d)
    }
    fn tile_forward(&self, a: i32, b: i32) -> i32 {
        // Steps forward from a to b around the board loop.
        let n = self.data.tiles.len() as i32;
        if n <= 0 {
            return 0;
        }
        ((b - a) % n + n) % n
    }
    fn nearest(&self, by: i32, kind: i32) -> Vec<i32> {
        self.nearest_with_prop(by, kind, None)
    }
    fn nearest_with_prop(&self, by: i32, kind: i32, key: Option<&str>) -> Vec<i32> {
        // An unknown code falls back to `Forward` (the v107 unknown-value rule).
        let shortest = kind == card_sdk::abi::DistanceKind::Shortest as i32;
        let measure = |a: i32, b: i32| {
            if shortest {
                self.dist(a, b)
            } else {
                self.tile_forward(a, b)
            }
        };
        let at = self.world.player_pos(by);
        if at < 0 || self.data.tiles.is_empty() {
            return Vec::new();
        }
        let mut best = i32::MAX;
        let mut raw: Vec<i32> = Vec::new();
        for p in self.world.others(by) {
            let there = self.world.player_pos(p);
            if there < 0 {
                continue; // a seat with no board position cannot be measured
            }
            let d = measure(at, there);
            if !shortest && d == 0 {
                continue; // co-located is not ahead
            }
            if d < best {
                best = d;
                raw.clear();
                raw.push(p);
            } else if d == best {
                raw.push(p);
            }
        }
        let Some(key) = key else { return raw };
        // The matching seats, distance-then-seat ordered, appended after the
        // raw set. The match is on the *presence* of the value, so seat 0 is a
        // value (the key is written by `bandori_std::nearest::grant`; the
        // engine knows nothing of it).
        let mut extra: Vec<i32> = self
            .world
            .others(by)
            .into_iter()
            .filter(|&p| {
                self.world.player_pos(p) >= 0
                    && !raw.contains(&p)
                    && self
                        .world
                        .st
                        .players
                        .get(p as usize)
                        .is_some_and(|s| s.field.iter().any(|i| i.props.get(key) == Some(&by)))
            })
            .collect();
        extra.sort_by_key(|&p| (measure(at, self.world.player_pos(p)), p));
        raw.extend(extra);
        raw
    }
    fn neighbor(&self, player_id: i32, dir: i32) -> i32 {
        // The next present player in turn order, wrapping.
        let n = self.world.st.players.len();
        if n == 0 {
            return -1;
        }
        let dir = if dir < 0 { -1 } else { 1 };
        for j in 1..n {
            let k = ((player_id + dir * j as i32) % n as i32 + n as i32) % n as i32;
            if self.present(k) {
                return k;
            }
        }
        -1
    }
    fn players_on_count(&self, tile: i32, except: i32) -> i32 {
        self.players_on_list(tile, except).len() as i32
    }
    fn players_on_at(&self, tile: i32, except: i32, index: i32) -> i32 {
        self.players_on_list(tile, except)
            .get(index.max(0) as usize)
            .copied()
            .unwrap_or(-1)
    }

    // hand & deck extensions -----------------------------------------------
    fn hand_count(&self, player_id: i32, card: &str) -> i32 {
        self.hands(player_id)
            .map(|h| h.iter().filter(|c| c.as_str() == card).count() as i32)
            .unwrap_or(0)
    }
    fn hand_size(&self, player_id: i32) -> i32 {
        self.hands(player_id).map(|h| h.len() as i32).unwrap_or(0)
    }
    fn discard_count(&self, player_id: i32, card: &str) -> i32 {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.graveyard.iter().filter(|c| c.as_str() == card).count() as i32)
            .unwrap_or(0)
    }
    fn deck_count(&self, player_id: i32) -> i32 {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.deck.len() as i32)
            .unwrap_or(0)
    }
    fn discard_size(&self, player_id: i32) -> i32 {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.graveyard.len() as i32)
            .unwrap_or(0)
    }
    fn discard_from_hand(&mut self, player_id: i32, card: &str) -> i32 {
        // One copy, hand -> discard pile.
        let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) else {
            return 0;
        };
        let Some(p) = h.hand.iter().position(|c| c == card) else {
            return 0;
        };
        h.hand.remove(p);
        h.graveyard.push(card.to_string());
        self.world
            .settle_over_hand_allowance(player_id.max(0) as usize);
        // Visible outcome: `commit_after` raises `discarded` (and maybe
        // `reshuffled`). The header names the clause that discarded.
        self.flush_header();
        self.discard_log.push((player_id, card.to_string()));
        if self
            .world
            .refill_after_pile_write(player_id.max(0) as usize)
        {
            self.reshuffle_log.push(player_id);
        }
        1
    }
    fn take_from_hand(&mut self, player_id: i32, card: &str) -> bool {
        let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) else {
            return false;
        };
        match h.hand.iter().position(|c| c == card) {
            Some(k) => {
                h.hand.remove(k);
                true
            }
            None => false,
        }
    }
    fn live_instance_of(&self, player_id: i32, card: &str, except: i32) -> Option<i32> {
        self.world.live_instance_of(player_id, card, except)
    }
    fn shuffle_into_deck(&mut self, player_id: i32, hand: bool, discard: bool) -> i32 {
        // Shuffle the requested piles into the draw pile.
        let idx = player_id.max(0) as usize;
        if self.world.hidden.get(idx).is_none() {
            return 0;
        }
        let mut moved: Vec<String> = Vec::new();
        if hand {
            moved.append(&mut self.world.hidden[idx].hand);
            self.world.settle_over_hand_allowance(idx);
        }
        if discard {
            moved.append(&mut self.world.hidden[idx].graveyard);
        }
        let n = moved.len() as i32;
        self.world.hidden[idx].deck.append(&mut moved);
        let mut draw = std::mem::take(&mut self.world.hidden[idx].deck);
        self.world.rng.shuffle(&mut draw);
        self.world.hidden[idx].deck = draw;
        // Visible outcome: `commit_after` raises `reshuffled`.
        self.flush_header();
        self.reshuffle_log.push(player_id);
        n
    }
    fn abnormal_count(&self, player_id: i32) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.world.turn.abnormal.get(s).copied())
            .unwrap_or(0)
    }
    fn targeted_count(&self, player_id: i32) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.world.targeted.get(s).copied())
            .unwrap_or(0)
    }
    fn gains_this_turn(&self, player_id: i32) -> i32 {
        self.world.gains_this_turn(player_id)
    }
    fn designations(&self, player_id: i32) -> Vec<i32> {
        // A play whose card declares recipients (`DESIGNATES`) names the other
        // living players of its user (「designate[使用者]以外的所有玩家」 /
        // 「其他玩家[分摊]」). The play's card is the one the trigger names.
        let card = self.trigger.card.as_deref().unwrap_or("");
        let designates = props_of(&self.props, card)
            .get(game_core::state::prop::DESIGNATES)
            .copied()
            .unwrap_or(0)
            != 0;
        if !designates {
            return Vec::new();
        }
        self.world.others(player_id)
    }
    fn cancel_designation(&mut self, seat: i32) {
        if !self.world.turn.cancelled_designations.contains(&seat) {
            self.world.turn.cancelled_designations.push(seat);
        }
        // A joiner's counter names the designation it cancels for the opener's
        // report (design G06 §6.4; the running seat is `player_id`).
        self.set_trigger_join_designation(seat);
    }
    fn designation_cancelled(&self, seat: i32) -> bool {
        self.world.turn.cancelled_designations.contains(&seat)
    }
    fn placed_tile(&self, player_id: i32, id: &str) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.world.st.players.get(s))
            .and_then(|s| s.field.iter().find(|f| f.card == id))
            .map_or(-2, |f| f.tile.max(-1))
    }
    fn play_doubled(&self) -> i32 {
        self.world.turn.play_doubled.unwrap_or(-1)
    }

    fn set_play_doubled(&mut self, n: i32) {
        self.world.turn.play_doubled = (n >= 0).then_some(n);
    }

    // status extensions -----------------------------------------------------
    fn can_pay(&self, player_id: i32) -> i32 {
        // Fallback when the ruleset has no pay-gate opinion. The real check
        // is `hostfns::pay_gate_open` (the ruleset's CEL).
        let Some(s) = self.world.st.players.get(player_id.max(0) as usize) else {
            return 0;
        };
        (!s.out()) as i32
    }
    fn cant_move_from(&self, player_id: i32, activation_ordinal: i32) -> i32 {
        // Why the main move is still unavailable (0 = free). Turn → skip →
        // spent: a [停留] turn answers 3 even after this activation's own
        // teleport, and a move an earlier activation drove answers 2 however
        // the move gate stands.
        if self.world.st.turn != player_id {
            return 1;
        }
        if self.world.st.skip_move {
            return 3;
        }
        if self.world.turn.main_moved
            && self.world.turn.move_activation_ordinal != activation_ordinal
        {
            return 2;
        }
        0
    }
    fn spend_marker(&mut self, player_id: i32, name: &str, n: i32, reason: Msg) -> i32 {
        // Spend from the named counter; logs the spend with its reason (the
        // caller's message is the log line, like `gain`/`pay`).
        if n <= 0 {
            return 1;
        }
        // A consumer of the counter: all-or-nothing, and it logs the spend. It
        // does not consult any cap -- spending is not the capped direction.
        // The row's creating instance, for the `markerChanged` payload.
        let instance = self
            .world
            .st
            .players
            .get(player_id.max(0) as usize)
            .and_then(|s| s.tokens.iter().find(|t| t.name == name))
            .and_then(|t| t.instance)
            .unwrap_or(-1);
        if self.world.spend_marker(player_id, name, n) == 0 {
            return 0;
        }
        self.post_log_value(name, player_id, reason.clone(), "", -n);
        if self.live.is_none() {
            self.world.log(name, player_id, reason).value = -n;
        }
        self.marker_spent_log.push((player_id, n, name.to_string()));
        // Post-commit `markerChanged` (HAG-01): a spend is a held write too,
        // with the negative delta. `markerSpent` stays spend-only.
        self.marker_changed_log
            .push((player_id, -n, name.to_string(), instance));
        1
    }
    fn turn_player(&self) -> i32 {
        self.world.st.turn
    }
    fn round_no(&self) -> i32 {
        self.world.st.round
    }
    fn turn_key(&self) -> i32 {
        // `round * 100 + turn + 1` -- once-per-turn latches.
        self.world.st.round * 100 + self.world.st.turn + 1
    }
    fn persona_is(&self, player_id: i32, name: &str) -> i32 {
        self.world
            .st
            .players
            .get(player_id.max(0) as usize)
            .is_some_and(|s| s.persona == name) as i32
    }
    fn set_persona(&mut self, player_id: i32, name: &str) {
        // The `persona_is` spelling: `MatchPlayer::persona`.
        if let Some(s) = self.world.st.players.get_mut(player_id.max(0) as usize) {
            s.persona = name.into();
        }
        // The name brings its skill rules, like the `Match::set_persona` seam
        // (`World::bind_skills`, idempotent): a body that only writes the name
        // would otherwise leave `persona_skill` on the old face.
        self.bind_persona_skills(player_id);
    }
    fn persona_skins(&self, player_id: i32) -> Vec<String> {
        Run::persona_skins(self, player_id)
    }
    fn in_group(&self, player_id: i32, name: &str) -> i32 {
        // The persona's group.
        let Some(s) = self.world.st.players.get(player_id.max(0) as usize) else {
            return 0;
        };
        self.data
            .personas
            .iter()
            .find(|c| c.name == s.persona)
            .is_some_and(|c| c.group == name) as i32
    }

    // trigger ---------------------------------------------------------------
    fn trigger(&self) -> Trigger {
        self.trigger.clone()
    }
    /// The resolution stack rides a thread-local, not the world: a raise
    /// nested inside a body reads it, and the engine opens a **fresh `PlayCtx`**
    /// per nested drive while every body commit swaps its world copy -- so
    /// neither the `PlayCtx` nor the world can hold it. One host thread per match.
    fn resolving(&self) -> Vec<(String, i32)> {
        resolving_names()
    }
    fn set_trigger_move_roll(&mut self, roll: i32) {
        self.fig_write(roll);
    }
    fn set_trigger_value(&mut self, value: i32) {
        self.fig_write(value);
    }
    fn set_trigger_target(&mut self, to: i32) {
        self.trigger.target = to;
    }
    fn set_trigger_pay_kind(&mut self, kind: i32) {
        let kind = game_core::engine::PayKind::from_i32(kind);
        if let Some(p) = &mut self.trigger.pay {
            p.kind = kind;
            // `Loss` keeps the declared rent flag (a redirected rent still
            // scales as rent); the other kinds set / clear it.
            p.is_rent = match kind {
                game_core::engine::PayKind::Rent => true,
                game_core::engine::PayKind::Loss => p.is_rent,
                _ => false,
            };
        }
    }
    fn set_trigger_pay_shape(&mut self, shape: i32) -> bool {
        use crate::TriggerKind as K;
        // A command's shape decides whether the split stages run, so a later
        // switch cannot un-skip them.
        let before_split = matches!(
            self.trigger.kind,
            K::PayCalcBefore
                | K::Effect
                | K::PayCombinedAddBefore
                | K::PayCombinedAdd
                | K::PayCombinedMul
                | K::PayCombinedCancel
        );
        if !before_split {
            self.world.log(
                "text",
                self.trigger.player_id,
                crate::Msg::new("log.pay_shape_too_late"),
            );
            return false;
        }
        if let Some(p) = &mut self.trigger.pay {
            p.shape = game_core::engine::PayShape::from_i32(shape);
        }
        true
    }
    fn settle_tile(&self) -> i32 {
        self.trigger.settle_tile
    }
    fn settle_outcome(&self) -> i32 {
        self.trigger.outcome.map_or(-1, |o| o.as_i32())
    }
    fn paid_in_settle_by(&self, seat: i32) -> i32 {
        self.world.paid_in_settle_by(seat)
    }
    fn rent_quote(&mut self, payer: i32, tile: i32) -> i32 {
        if tile < 0 {
            return -1;
        }
        self.world
            .rent_quote(&self.data, payer.max(0) as usize, tile as usize)
    }
    fn pay_factor(&self) -> i32 {
        self.world.pay_factor()
    }
    fn multiply_pay_factor(&mut self, milli: i32, scope: i32) {
        self.stamp_skill_source();
        if !self.claim_factor() {
            return;
        }
        let scope = game_core::engine::PayFactorScope::from_i32(scope);
        self.world.multiply_pay_factor(milli, scope);
    }
    fn set_trigger_cancelled(&mut self) {
        self.trigger.negate_activation();
    }
    fn set_trigger_negate_effect(&mut self) {
        self.trigger.negate_effect();
    }
    fn set_trigger_spare(&mut self, seat: i32) {
        self.trigger.spare(seat);
    }
    fn declare_trigger_effect(
        &mut self,
        kind: &str,
        target: i32,
        from: i32,
        tile: i32,
        value: i32,
    ) {
        self.trigger.declare(game_core::engine::rules::Effect {
            kind: kind.to_string(),
            target,
            from,
            tile,
            value,
        });
    }
    fn trig_card_is(&self, id: &str) -> i32 {
        (self.trigger.card.as_deref() == Some(id)) as i32
    }
    fn set_trigger_price(&mut self, v: i32) {
        self.fig_write(v);
    }
    fn set_trigger_deal_owner(&mut self, v: Option<game_kinds::Payee>) {
        if let Some(b) = &mut self.trigger.buy {
            b.deal_owner = v;
        }
    }
    fn set_trigger_deal_houses(&mut self, v: Option<i32>) {
        if let Some(b) = &mut self.trigger.buy {
            b.deal_houses = v;
        }
    }
    fn set_trigger_deal_mortgaged(&mut self, v: i32) {
        if let Some(b) = &mut self.trigger.buy {
            b.deal_mortgaged = v != 0;
        }
    }
    fn set_trigger_reason(&mut self, reason: &str) {
        self.trigger.reason = Some(reason.to_string());
    }
    fn set_trigger_join_used(&mut self) {
        self.trigger.join_used = true;
        // Keep the per-seat report in step with the legacy alias. The running
        // seat is the declarer ([`Run::activation_player`]): the trigger is
        // the *answered* link, whose `player_id` is its payer.
        let who = self.activation_player;
        if let Some(j) = &mut self.trigger.join {
            if let Some(k) = j.seats.iter().position(|&s| s == who) {
                j.used[k] = true;
            }
        }
    }
    fn set_trigger_join_designation(&mut self, seat: i32) {
        let who = self.activation_player;
        if let Some(j) = &mut self.trigger.join {
            if let Some(k) = j.seats.iter().position(|&s| s == who) {
                j.designated[k] = seat;
            }
        }
    }
    fn trigger_join_redirect(&self) -> Option<i32> {
        self.trigger.join_redirect
    }
    fn counteracted_records(&self) -> Vec<game_core::engine::CounteractRecord> {
        self.world.countered_this_turn.clone()
    }
    fn copy_source_card(&self, seat: i32) -> Option<String> {
        self.world
            .played_cards
            .iter()
            .rev()
            .find(|p| p.player != seat && !p.placed)
            .map(|p| p.card.clone())
    }

    // turn plan & scheduling -------------------------------------------------
    fn schedule_turn_end(
        &mut self,
        player_id: i32,
        next_of_player: bool,
        early: bool,
        entry: &str,
    ) {
        let now = self.world.turn.player_id;
        let (target, skip) = if next_of_player {
            let s = player_id.max(0) as usize;
            // Scheduled during that player's own turn: let this turn end pass.
            (s, s == now)
        } else {
            (now, false)
        };
        self.world.scheduled.push(game_core::engine::Scheduled {
            card: self.current_card.clone(),
            owner: player_id,
            target,
            skip,
            early,
            // The instance asking for the callback, captured now. The
            // identity belongs to the entry rather than being resolved from the
            // field at trigger time (which copy would it be?). `-1` when the card
            // is not in play: `On::Schedule` may run for a card in a hand or pile.
            uid: self.current_uid,
            entry: String::from(entry),
        });
    }
    fn set_no_money_loss(&mut self, player_id: i32) {
        if let Ok(s) = usize::try_from(player_id) {
            if !self.world.turn.no_money_loss.contains(&s) {
                self.world.turn.no_money_loss.push(s);
            }
        }
    }
    fn set_fixed_roll(&mut self, n: i32, source: i32) {
        self.world.turn.fixed_roll = Some(game_core::engine::FixedFace {
            value: n.max(0),
            source: game_core::engine::rules::RollSource::from_i32(source),
        });
    }
    fn fixed_roll(&self) -> i32 {
        self.world.turn.fixed_roll.map(|f| f.value).unwrap_or(-1)
    }
    fn set_next_steps(&mut self, player_id: i32, n: i32) {
        if let Some(h) = self.world.hidden.get_mut(player_id.max(0) as usize) {
            h.next_steps = Some(n.max(0));
        }
    }
    fn turn_main_steps(&self) -> i32 {
        self.world.turn.main_steps
    }
    fn turn_move_abnormal(&self) -> bool {
        self.world.turn.move_abnormal
    }
    fn turn_dice_immune(&self) -> bool {
        self.world.turn.dice_immune
    }
    fn set_turn_dice_immune(&mut self, on: bool) -> bool {
        std::mem::replace(&mut self.world.turn.dice_immune, on)
    }
    fn add_marker_max(&mut self, player_id: i32, name: &str, n: i32) -> i32 {
        self.world.add_marker_max(player_id, name, n)
    }
    fn enter_card(&mut self, id: &str) -> (String, i32, Option<i32>, i32) {
        let card = std::mem::replace(&mut self.current_card, id.to_string());
        let dest = std::mem::replace(&mut self.dest, DEST_UNSET);
        let dest_to = std::mem::replace(&mut self.dest_to, None);
        // Fresh instance: the nested run is not the outer card's field card,
        // so it starts with no uid of its own until it places one.
        let uid = std::mem::replace(&mut self.current_uid, -1);
        (card, dest, dest_to, uid)
    }
    fn leave_card(&mut self, saved: (String, i32, Option<i32>, i32)) -> i32 {
        self.current_card = saved.0;
        self.current_uid = saved.3;
        self.dest_to = saved.2;
        std::mem::replace(&mut self.dest, saved.1)
    }
    fn enter_invoke(&mut self, id: &str, instance: i32) -> crate::world::InvokeSaved {
        let (card, dest, dest_to, caller_uid) = self.enter_card(id);
        // 「该角色」 / 「该技能」 inside the invoked body is the *invoked* card:
        // the frame replaces the trigger payload instead of raising a second
        // counteraction category (G13 I4).
        let trigger_card = std::mem::replace(&mut self.trigger.card, Some(id.to_string()));
        let trigger_cards = std::mem::replace(&mut self.trigger.cards, vec![id.to_string()]);
        // `enter_card` voided the uid; the target's own instance is what the
        // body's self-addressed reads must answer for (I2).
        self.current_uid = instance;
        crate::world::InvokeSaved {
            card,
            dest,
            dest_to,
            instance: caller_uid,
            trigger_card,
            trigger_cards,
        }
    }
    fn leave_invoke(&mut self, saved: crate::world::InvokeSaved) -> i32 {
        self.current_card = saved.card;
        self.current_uid = saved.instance;
        self.dest_to = saved.dest_to;
        self.trigger.card = saved.trigger_card;
        self.trigger.cards = saved.trigger_cards;
        std::mem::replace(&mut self.dest, saved.dest)
    }
    // movement shaping ---------------------------------------------------------
    fn set_steps(&mut self, v: i32) {
        self.stamp_skill_source();
        let was = self.world.move_total();
        self.world.set_steps(v);
        // Written for the move this body shapes, not for a later one.
        // Keep the shared MoveBefore link coherent for later counters and
        // hooks. Its base distance is already final; a plan rewrite extends
        // this move rather than rewriting a roll that has already resolved.
        if self.trigger.kind == TriggerKind::MoveBefore {
            let delta = v.max(0) - self.trigger.value.max(0);
            self.trigger.value = v.max(0);
            if let Some(m) = &mut self.trigger.mv {
                m.total = (m.total + delta).max(0);
                m.remaining = (m.remaining + delta).max(0);
            }
        }
        if self.world.move_total() != was {
            self.outcome_plan("log.outcome.plan_steps", -1, v, "");
        }
    }
    fn set_plan_claimed(&mut self) {
        self.world.plan_mut().claimed = true;
    }
    fn plan_claimed(&self) -> bool {
        self.world.plan().claimed
    }
    fn effective_card_at(&self, uid: i32) -> Option<String> {
        self.world.effective_card_at(uid)
    }
    fn set_effective_card_at(&mut self, uid: i32, card: &str) -> bool {
        self.world.set_effective_card_at(uid, card)
    }
    fn set_roller(&mut self, v: Option<game_kinds::Seat>) {
        self.stamp_skill_source();
        // `plan::set_roller` -- the stand-in roller for 「上一名玩家代替进行此次
        // 投掷」. `MoveCtx::roller` rides `TurnCtx.plan`, so `main_move`'s
        // post-`rollPlan` re-clone picks it up.
        if v.is_some() {
            self.world.plan_mut().roller = v;
        }
    }
    fn set_reverse(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_reverse(v);
    }
    fn set_signed(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_signed(v);
    }
    fn set_stop_at(&mut self, v: Option<game_kinds::Tile>) {
        self.stamp_skill_source();
        let was = self.world.move_stop_at();
        self.world.set_stop_at(v);
        if v != was {
            self.outcome_plan("log.outcome.plan_stop", v.map(|t| t.0).unwrap_or(-1), 0, "");
        }
    }
    fn set_parity(&mut self, v: i32) {
        self.stamp_skill_source();
        self.world.set_parity(v);
    }
    fn set_settles(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_settles(v);
    }
    fn set_no_buy(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_no_buy(v);
        if v {
            self.outcome_plan("log.outcome.plan_no_buy", -1, 0, "");
        }
    }
    fn set_kind(&mut self, v: i32) {
        self.stamp_skill_source();
        self.world.set_kind(v);
    }
    fn set_tag(&mut self, key: &str, v: i32) {
        self.stamp_skill_source();
        self.world.set_tag(key, v);
    }
    fn recorded_roll(&self) -> Option<i32> {
        self.world.recorded_roll
    }
    fn move_tag(&self, key: &str) -> i32 {
        self.world.move_tag(key)
    }
    fn set_min_roll(&mut self, v: Option<i32>) {
        self.stamp_skill_source();
        self.world.set_min_roll(v);
    }
    fn set_extra_steps(&mut self, v: i32) {
        self.stamp_skill_source();
        self.world.set_extra_steps(v);
        if v != 0 {
            self.outcome_plan("log.outcome.plan_steps", -1, v, "");
        }
    }
    fn add_extra_steps(&mut self, v: i32) {
        self.stamp_skill_source();
        self.world.add_extra_steps(v);
        self.outcome_plan("log.outcome.plan_steps", -1, v, "");
    }
    fn clear_steps(&mut self) {
        self.stamp_skill_source();
        self.world.clear_steps();
    }
    fn set_forced(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_forced(v);
    }
    fn set_main_override(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_main_override(v);
    }
    fn void_walk(&mut self) {
        self.stamp_skill_source();
        self.world.void_walk();
    }
    fn clear_void_walk(&mut self) {
        self.world.plan_mut().void_walk = false;
    }
    fn set_settle_tile(&mut self, v: i32) {
        self.stamp_skill_source();
        self.world.set_settle_tile(v);
    }
    fn set_pay_factor(&mut self, v: i32) {
        self.stamp_skill_source();
        if !self.claim_factor() {
            return;
        }
        self.world.set_pay_factor(v);
    }
    fn set_rent_factor(&mut self, v: i32) {
        self.stamp_skill_source();
        if !self.claim_factor() {
            return;
        }
        self.world.set_rent_factor(v);
    }
    fn set_can_build(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_can_build(v);
    }
    fn set_no_build(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_no_build(v);
        if v {
            self.outcome_plan("log.outcome.plan_no_build", -1, 0, "");
        }
    }
    fn set_build_anywhere(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_build_anywhere(v);
    }
    fn set_settle_as_agent(&mut self, v: bool) {
        self.stamp_skill_source();
        self.world.set_settle_as_agent(v);
    }
    fn plan_add_follower(&mut self, player_id: i32) {
        self.stamp_skill_source();
        self.world.plan_add_follower(player_id);
    }
    fn set_more_steps(&mut self, v: i32) {
        self.stamp_skill_source();
        self.world.set_more_steps(v);
    }

    fn set_teleport_to(&mut self, v: Option<game_kinds::Tile>) {
        self.stamp_skill_source();
        self.world.set_teleport_to(v);
        self.outcome_plan(
            "log.outcome.plan_teleport",
            v.map(|t| t.0).unwrap_or(-1),
            0,
            "",
        );
    }
    fn set_start(&mut self, v: Option<game_kinds::Tile>, reason: &str) {
        self.stamp_skill_source();
        self.world.set_start(v, reason);
        self.outcome_plan(
            "log.outcome.plan_start",
            v.map(|t| t.0).unwrap_or(-1),
            0,
            reason,
        );
    }
    fn clear_dice(&mut self) {
        self.stamp_skill_source();
        self.world.clear_dice();
    }
    fn set_base_dice(&mut self, count: i32, sides: i32, reason: &str) {
        self.stamp_skill_source();
        self.world.set_base_dice(count, sides, reason);
        self.outcome_plan("log.outcome.plan_dice", -1, count, reason);
    }
    fn add_base_dice(&mut self, count: i32, sides: i32, reason: &str) {
        self.stamp_skill_source();
        self.world.add_base_dice(count, sides, reason);
    }
    fn add_extra_dice(&mut self, count: i32, sides: i32, reason: &str) {
        self.stamp_skill_source();
        self.world.add_extra_dice(count, sides, reason);
    }
    fn move_stopped(&self) -> bool {
        self.world.move_stopped()
    }
    fn move_stop_at(&self) -> Option<game_kinds::Tile> {
        self.world.move_stop_at()
    }
    fn move_parity(&self) -> i32 {
        self.world.move_parity()
    }
    fn move_settles(&self) -> bool {
        self.world.move_settles()
    }
    fn move_plan(&self) -> game_core::engine::MoveCtx {
        self.world.plan().clone()
    }
    fn plan_mut(&mut self) -> &mut game_core::engine::MoveCtx {
        self.world.plan_mut()
    }
    fn move_kind(&self) -> i32 {
        self.world.move_kind()
    }
    fn move_steps(&self) -> i32 {
        self.world.move_steps()
    }
    fn move_remaining(&self) -> i32 {
        self.world.move_remaining()
    }
    fn move_total(&self) -> i32 {
        self.world.move_total()
    }
    fn move_dir(&self) -> i32 {
        self.world.move_dir()
    }
    fn move_planned_teleport(&self) -> i32 {
        self.world.planned_teleport()
    }
    fn move_pay_factor(&self) -> i32 {
        self.world.move_pay_factor()
    }
    fn move_rent_factor(&self) -> i32 {
        self.world.rent_factor()
    }
    fn move_effective_steps(&self) -> i32 {
        self.world.effective_steps()
    }
    fn move_forced(&self) -> bool {
        self.world.move_forced()
    }
    fn trig_move_rolled(&self) -> bool {
        self.trigger.mv.as_ref().is_some_and(|m| m.rolled)
    }
    fn trig_move_effective_steps(&self) -> i32 {
        self.trigger.mv.as_ref().map_or(0, |m| m.effective)
    }
    fn nearest_tile_on_axis(&self, tile: i32, axis: i32) -> i32 {
        self.world.nearest_tile_on_axis(&self.data, tile, axis)
    }
    fn farthest_tile_on_axis(&self, tile: i32, axis: i32) -> i32 {
        self.world.farthest_tile_on_axis(&self.data, tile, axis)
    }
}

impl Run {
    /// Place the skills of the seat's current face that are not on the field
    /// yet -- `World::bind_skills` over the run's `props` snapshot. Silent, as
    /// the match-start binding is; an explicit `ctx::place_card` of the same
    /// rule afterwards finds it already placed.
    fn bind_persona_skills(&mut self, player_id: i32) {
        let Some(s) = self.world.st.players.get(player_id.max(0) as usize) else {
            return;
        };
        let persona = s.persona.clone();
        let placed = self.world.placed_cards(player_id);
        for id in self.data.skill_rules_of(&persona) {
            if placed.contains(&id) {
                continue;
            }
            let mut props = props_of(&self.props, &id);
            let caps = split_counter_max(&mut props);
            let uid = self
                .world
                .place_card(&self.data, player_id, &id, Msg::default(), props);
            for (name, cap) in caps {
                self.world.set_counter_max_at(uid, &name, cap);
            }
        }
    }

    /// The one figure setter behind `set_trigger_value` / `_move_roll` /
    /// `_price`.
    ///
    /// The guest's [`card_sdk::payload::drive_value`] writes every value-shaped
    /// Filter answer through all three imports, because the figure lands in a
    /// different place per kind (the move payload's face, the buy payload's
    /// price, plain `value`). A plain assignment is idempotent, but a fold
    /// stage (MONEY-01) is not, so this absorbs the sibling writes of the same
    /// number and folds each distinct figure once.
    fn fig_write(&mut self, v: i32) {
        if self.fig == Some(v) {
            return;
        }
        self.fig = Some(v);
        // The face rides the move payload when one exists; a card-driven
        // `roll` raise has no `t.Move` and the writeback reads `value`
        // instead (Y.O.L.O on 热气球's 4x3d20).
        if let Some(m) = &mut self.trigger.mv {
            m.roll = Some(v);
        }
        // A price stage commits `buy.price`.
        if let Some(b) = &mut self.trigger.buy {
            b.price = v;
        }
        // `value`, composing on an amount fold.
        self.trigger.fold_write(v);
    }

    /// Carry the running body's play-origin scope across a reload of the world
    /// copy.
    ///
    /// A reload adopts live, whose turn still names the **outer** play: the
    /// scoped `play_from_hand(false)` (and `placed_during_play(false)`) a
    /// nested `play_card` / `invoke_entry` writes live on the *copy* only, and
    /// live is not written until a request site adopts it -- but a consumed
    /// answer reloads the copy *before* that adopt. Without this the nested
    /// body reads the outer hand play's origin, so a draw-pile auto-play looks
    /// like a hand play at its first nested request (ABI-08's `FromDeck`
    /// question was answered `true` that way).
    fn carry_play_scope(&mut self, old: &game_core::engine::World) {
        self.world.turn.play_from_hand = old.turn.play_from_hand;
        self.world.turn.placed_during_play = old.turn.placed_during_play;
    }

    /// The cosmetic skins of `player_id`'s character (ABI-09, `ctx::persona_skins`):
    /// every roster persona sharing their base character (`variant_of`), the
    /// current one excluded. Cosmetic only -- a skin keeps the bound skills
    /// (「技能相同」), so a swap is a `persona` name write.
    pub fn persona_skins(&self, player_id: i32) -> Vec<String> {
        let Some(p) = self.world.st.players.get(player_id.max(0) as usize) else {
            return Vec::new();
        };
        let cur = p.persona.clone();
        let base = self.data.base_persona(&cur).to_string();
        let mut out: Vec<String> = Vec::new();
        for persona in &self.data.personas {
            if persona.name == cur {
                continue;
            }
            if self.data.base_persona(&persona.name) == base {
                out.push(persona.name.clone());
            }
        }
        out
    }

    /// The `(writer, ordinal)` key of the next factor write: the running
    /// instance when there is one, else the seat plus the card id (a hand play
    /// or a persona skill has no instance), plus this pass's counter. Stable
    /// across the passes of one drive, so a re-run's write lands on its own
    /// earlier entry instead of stacking a second halving.
    fn factor_write_key(&mut self) -> (String, u32) {
        let n = self.world.turn.factor_ordinal;
        self.world.turn.factor_ordinal = n.wrapping_add(1);
        let who = if self.current_uid >= 0 {
            self.current_uid.to_string()
        } else {
            format!("{}#{}", self.activation_player, self.current_card)
        };
        (who, n)
    }

    /// Debug: the live `PlayCtx` this run writes through (`0` for a probe run).
    pub fn live_ptr(&self) -> usize {
        self.live.map_or(0, |p| p as usize)
    }

    /// [`Run::factor_write_key`] plus the replay guard: `false` when this exact
    /// contribution is the re-execution of one that already landed, so the
    /// caller skips it. The key is consumed either way, keeping the ordinals
    /// of a pass lined up with the pass before it.
    fn claim_factor(&mut self) -> bool {
        let (who, n) = self.factor_write_key();
        let exec = self.ledger_exec;
        let ok = self.world.note_factor(exec, &who, n);
        if game_core::engine::stage_dbg() {
            eprintln!("CLAIM exec={exec} who={who} n={n} ok={ok}");
        }
        ok
    }
    /// A card placing *itself* keeps its owner whichever field it lands on:
    /// the owner it carried off the field (a hop), else the player whose
    /// activation this is (a hand play). Placement alone stamps the field
    /// holder, which loses the owner of a card parked on another player.
    ///
    /// The hand-play fallback is [`Run::activation_player`] (`Call::player_id`),
    /// not `trigger.player_id`: a [反击] body runs against the *answered* link,
    /// whose `player_id` is the payer, while the card belongs to its declarer.
    fn stamp_owner(&mut self, uid: i32, card: &str) {
        if uid < 0 || card != self.current_card {
            return;
        }
        let held = self.world.field_by_uid(self.current_uid).map(|f| f.owner);
        let owner = self
            .carried_owner
            .take()
            .or(held)
            .unwrap_or(self.activation_player);
        if owner >= 0 {
            self.world.set_field_owner(uid, owner);
        }
    }

    /// In the game. Off-board is the ruleset's OnBoard gate (the CEL
    /// `players_on` also checks it).
    fn present(&self, s: i32) -> bool {
        self.world
            .st
            .players
            .get(s.max(0) as usize)
            .is_some_and(|x| !x.out())
    }

    /// Present players standing on the tile, minus `except`.
    fn players_on_list(&self, tile: i32, except: i32) -> Vec<i32> {
        (0..self.world.st.players.len() as i32)
            .filter(|&p| {
                p != except && self.present(p) && self.world.st.players[p as usize].pos == tile
            })
            .collect()
    }

    /// The player's private hand, in order.
    fn hands(&self, player_id: i32) -> Option<&Vec<String>> {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| &h.hand)
    }
}

/// The card modules, ready to serve the match shell.
pub struct RulesBridge<M: CardModules> {
    ruleset: M,
    data: Arc<GameData>,
    /// Every card's declared props, snapshotted once: a [`Run`] needs the whole
    /// table, and the ruleset is immutable for the bridge's life.
    props: std::sync::OnceLock<Arc<CardPropsTable>>,
    /// `card_texts` per card, converted and sorted once (the view asks per
    /// board field on every `state()`).
    texts: std::sync::RwLock<
        std::collections::HashMap<String, Arc<[game_core::engine::TileTextDecl]>>,
    >,
}

/// Card id -> declared static props (the per-[`Run`] snapshot).
type CardPropsTable = std::collections::HashMap<String, std::collections::BTreeMap<String, i32>>;

/// The sandboxed bridge: `CardRules` over wasmi/wasmtime card modules.
#[cfg(feature = "sandbox")]
pub type WasmRules = RulesBridge<Ruleset>;

#[cfg(feature = "sandbox")]
impl RulesBridge<Ruleset> {
    /// Load every module listed in `<dir>/index.json` (tools/build-ruleset.sh)
    /// and bind the tables. `Ok(None)` when the directory or index is missing --
    /// the caller falls back to no card effects.
    ///
    /// Hosts that need the roster *before* the tables (the tables' roster comes
    /// from the ruleset) should call [`Ruleset::load_dir`] first, take
    /// [`Ruleset::roster`], then [`Self::new`].
    pub fn load_dir(data: Arc<GameData>, dir: &std::path::Path) -> Result<Option<Self>, RuleError> {
        Ok(Ruleset::load_dir(dir)?.map(|ruleset| Self::new(ruleset, data)))
    }
}

/// The inline host one drive installs around its guest call in simulation
/// mode (`docs/BOT.md` §3.2). Holds raw pointers to the drive's locals (the
/// bridge, the engine `PlayCtx`, the card id, the drive's answer log) so it can be
/// `'static` on the thread-local stack; all of them outlive the
/// `with_inline_host` frame the drive wraps the guest call in.
///
/// **Prompts inline, host requests through the loop.** A player prompt is
/// answered as it happens ([`PlayCtx::ask`] against the provider), so it no longer
/// costs a body re-run -- the big win B2 is after, since a routine with *p*
/// prompts and *h* host requests used to run *p*+*h*+1 times. A
/// [`HostRequest`] still pauses and is applied by the drive's `NeedHost` arm
/// against the live world before the body re-runs: that is what makes the
/// final state `guest_writes(live + host_effects)` -- the guest's writes *on
/// top* of the host effects -- exactly as the replay model derives it. A
/// single forward pass that applied the requests to the guest's own world
/// interleaves the two and diverges (the dice stream ordering changes too).
struct DriveInline<M: CardModules> {
    bridge: *const RulesBridge<M>,
    cx: *mut PlayCtx<'static>,
    call: Call,
    card_id: *const str,
    halt: Option<Halt>,
}

impl<M: CardModules> DriveInline<M> {
    /// The type-erased world `HostCtx` hands back is always a [`Run`]:
    /// `CardModules` pins it. (Both backends share `hostfns`, which remains
    /// generic over `HostCtx::World`.)
    fn as_run<'r>(any: &'r mut dyn std::any::Any) -> &'r mut Run {
        any.downcast_mut::<Run>()
            .expect("CardModules pins HostCtx::World = Run")
    }
}

impl<M: CardModules> crate::inline::InlineHost for DriveInline<M> {
    /// Answer a player prompt through the engine's [`PlayCtx::ask`] -- the provider
    /// decides inline, or declines and this pauses. Runs against the **live**
    /// world (no swap), so `fill_ai` / `ask_seq` advance exactly where the
    /// replay model's `NeedInput` arm advances them. The answer is appended to
    /// the drive's log so a re-run (caused by a later `HostRequest`) finds it.
    fn answer(&mut self, _any: &mut dyn std::any::Any, p: Prompt) -> Option<i32> {
        let cx = unsafe { &mut *self.cx };
        let kind = p.kind;
        let ask = prompt_to_ask(p);
        let items = ask.view.items.clone();
        match cx.ask(ask) {
            Ok(reply) => {
                let v = reply
                    .answered
                    .answers
                    .first()
                    .copied()
                    .filter(|&x| x >= 0)
                    .unwrap_or(reply.fallback);
                Some(recorded_answer(kind, &items, v))
            }
            Err(h) => {
                self.halt = Some(h);
                None
            }
        }
    }

    /// Apply a [`HostRequest`] against the **live** world, with the same
    /// `adopt_turn_policy` / `overlay_guest_state` preamble the replay's
    /// `NeedHost` arm uses. The body keeps going (one forward pass); the drive
    /// then runs one commit pass with the answers pre-filled so the guest's
    /// writes land on top of the host effects, same as the replay's last pass.
    /// Two guest runs per body, not *k*+1.
    fn apply(
        &mut self,
        any: &mut dyn std::any::Any,
        log: &mut game_core::engine::GuestLog,
        start: game_core::engine::JournalStart,
        req: HostRequest,
    ) -> Option<i32> {
        // Draws mutate piles and run after-hooks immediately. Re-enter the
        // guest from the engine's resulting snapshot, so both execution modes
        // see those changes before subsequent guest reads and prompts.
        let run = Self::as_run(any);
        if matches!(
            req,
            HostRequest::Draw { .. } | HostRequest::InspectionBegin { .. }
        ) || !run.pile_checkpoints.is_empty()
        {
            return None;
        }
        // A join window runs on the work stack (design G06 §6.4): always
        // pause so the drive's NeedHost arm takes it.
        if matches!(req, HostRequest::JoinOpen { .. }) {
            return None;
        }
        // STACK-01: nested settle while pumping must pause (NeedHost suspends).
        if matches!(req, HostRequest::SettleAt { .. }) {
            let cx_probe = unsafe { &mut *self.cx };
            if cx_probe.is_draining_work() {
                return None;
            }
        }
        // Ruling 2a: a host effect is a visible outcome -- post the lazy
        // 「效果适用」 header first, so the engine's own lines (the loss, the
        // gain) group under it instead of leaving it 「（无事发生）」.
        if visible_host_request(&req).is_some() {
            run.flush_header();
        }
        let cx = unsafe { &mut *self.cx };
        // The three inline inspection/reveal routines keep the body on one
        // forward pass; the guest's preceding pile writes must land on the
        // live world first, exactly as the NeedHost arm syncs them (G11 §5.5).
        if matches!(
            req,
            HostRequest::InspectionEnd { .. }
                | HostRequest::RevealPile { .. }
                | HostRequest::RevealCard { .. }
        ) {
            let mut w = cx.world_copy();
            PileCheckpoint::new(run.world.deep_clone()).apply(&mut w);
            cx.swap_world(w);
        }
        let bridge = unsafe { &*self.bridge };
        let card_id: &str = unsafe { &*self.card_id };
        let uid = run.current_uid;
        if matches!(start, game_core::engine::JournalStart::Replace(_)) {
            // This pass's earlier pauses crossed the body's writes onto live,
            // including the first application's. The rewind supersedes them:
            // drop them off live first (the entry's `condition` still has the
            // ones the body keeps), then rewind, then re-base the diff.
            cx.restore_guests_to(run.overlay_floor, run.drive_id);
        }
        restore_condition(cx, log, start, &req);
        if matches!(start, game_core::engine::JournalStart::Replace(_)) {
            // See the replay path: the restore re-bases the guest diff.
            run.pass_base = cx.pass_base();
        }
        cx.adopt_turn_policy(&run.world);

        // The body's own writes, so the apply can hand them to the routine and
        // the copy can be rebuilt on top of the host effect afterwards.
        let mut overlay = cx.overlay_guest_state(&run.pass_base, &run.world, run.drive_id);
        cx.overlay_guest_plan_tags(&run.world);
        let linger = run.linger_props.clone();
        let live_before = cx.live_event_count();
        let done = matches!(req, HostRequest::InspectionEnd { .. });
        let _answer_slot = log.cursor;
        if std::env::var("DBG_H01").is_ok() {
            eprintln!(
                "H01 apply pre req={req:?} owner16={:?} live_owner16={:?}",
                run.world.st.owners.get(16),
                cx.world().st.owners.get(16)
            );
        }
        let applied = bridge.apply_host_request(
            cx,
            run,
            req,
            start,
            log,
            &mut overlay,
            &linger,
            self.call.clone(),
            uid,
            card_id,
        );
        match applied {
            Ok(v) => {
                if std::env::var("DBG_H01").is_ok() {
                    eprintln!(
                        "H01 apply post live_owner16={:?}",
                        cx.world().st.owners.get(16)
                    );
                }
                let play_scope = (
                    run.world.turn.play_from_hand,
                    run.world.turn.placed_during_play,
                );
                if done {
                    // The frame is gone and its cards are back in the deck: the
                    // guest's next read must see that, not its own copy.
                    run.world = cx.share_world();
                }
                cx.stamp_parent_since(live_before, run.parent);
                // The apply completed the entry and wrote the answer into the
                // log; the guest's own store continues from that cursor. The
                // effect landed on live and the copy still predates it, so
                // adopt live -- which the crossing already carries the body's
                // own writes on (same as the replay arm; no second apply, or a
                // routine that restored over them would get them put back).
                let old = std::mem::replace(&mut run.world, cx.share_world());
                // Live names the outer play; `old` is this body's copy (or the
                // inspection reload, which already lost it) -- take the scope
                // this body entered with (see `Run::carry_play_scope`).
                run.world.turn.play_from_hand = play_scope.0;
                run.world.turn.placed_during_play = play_scope.1;
                // The host effect (and a nested settle's writes) landed on live:
                // the guest diff has to re-base on it, or the next pass's
                // crossing puts the pre-effect row back (HOST-01).
                run.pass_base = cx.pass_base_for(run.overlay_floor, run.drive_id);
                run.world.turn.plan = old.turn.plan.clone();
                run.world.turn.plan_stack = old.turn.plan_stack.clone();
                cx.overlay_guest_plan_tags(&old);
                run.world.skip_log = true;
                Some(v)
            }
            Err(h) => {
                self.halt = Some(h);
                None
            }
        }
    }

    fn take_halt(&mut self) -> Option<Halt> {
        self.halt.take()
    }
}

impl<M: CardModules> RulesBridge<M> {
    pub fn new(ruleset: M, data: Arc<GameData>) -> Self {
        // A fresh engine owns no resolving frames (the store is a
        // thread-local; see [`reset_resolving`]).
        reset_resolving();
        Self {
            ruleset,
            data,
            props: std::sync::OnceLock::new(),
            texts: Default::default(),
        }
    }

    /// The ruleset (module list, hashes) for inspection.
    pub fn ruleset(&self) -> &M {
        &self.ruleset
    }

    /// Snapshot every card's declared static properties for a [`Run`].
    fn modules_props(
        &self,
    ) -> Arc<std::collections::HashMap<String, std::collections::BTreeMap<String, i32>>> {
        self.props
            .get_or_init(|| {
                let mut m = std::collections::HashMap::new();
                for c in self.ruleset.cards() {
                    m.insert(c.id.clone(), c.props.clone());
                }
                Arc::new(m)
            })
            .clone()
    }

    /// Does this drive's log lines name their source (「<卡名> 的效果：…」)?
    ///
    /// **No.** The 「效果适用」 header at body entry names the card and the
    /// trigger; the body's own lines sit under that header as its results, so
    /// repeating 「<卡名> 的效果：」 on every line is noise. A fresh activation
    /// (hand play, counteraction, skill press, drawn event) already logged its action
    /// and likewise leaves its outcome lines bare.
    fn wraps_effect(_call: &Call, _card_id: &str) -> bool {
        false
    }

    /// One `"card"` activation event for a body that is about to run: the
    /// client's card flash plus its 「效果适用」 line. `call` / `card_id` /
    /// `uid` are the drive's own context; `trigger` says why it ran; `entry`
    /// names which clause of a multi-entry card (its manifest label, if any).
    ///
    /// Written into the world copy `w` (the guest's), then captured as a
    /// guest-side burst ([`Run::capture_guest`]) so [`Self::commit_after`] can
    /// re-insert it **before** the host-effect events the learn pass already
    /// logged -- the same code-order merge that sequences body logs around
    /// pays. The return value is the event's id, used as the `parent` of
    /// everything this body produces.
    ///
    /// Called exactly at **body entry** (the `on_body` callback
    /// [`Self::drive_inner_body`] hands to `run` / `run_hook`): after the CEL
    /// `condition` and the residual guard admitted, after any play gate, and only
    /// when the entry exists -- so a guard reject, a probe, a quote or a
    /// bodyless drive never reaches the UI. One drive's multi-pass re-runs
    /// announce on the copy that lands (the others are thrown away with their
    /// pass).
    ///
    /// A tile's own settle body (`tile:*`) is a board square, not a card, so it
    /// does not flash -- a *card* placed on the square still does. `On::Message`
    /// never reaches here (internal plumbing, no `Call::Message`).
    ///
    /// A **silent** entry (`ManifestOn::silent`, `On::silent`) is
    /// bookkeeping -- `declare_cap` / `set_bounds` / initial values / latch
    /// clears: a mechanical declaration the player never sees as a clause
    /// ficycle. It runs its body (the category → condition → guard layers are
    /// unchanged) but posts no `"card"` event and no 「效果适用」 header;
    /// returns `-1`. Its body's own visible results (if it wrongly produced
    /// any) parent to the enclosing activation or top-level, never to a
    /// synthetic header. Nested activations from a silent body announce
    /// normally.
    fn announce_drive(
        &self,
        w: &mut crate::Run,
        call: Call,
        card_id: &str,
        uid: i32,
        target: i32,
        tile: i32,
        trigger: &Trigger,
        entry: i32,
    ) -> i32 {
        if matches!(
            call,
            Call::Settle { .. } | Call::Mortgage { .. } | Call::Redeem { .. }
        ) && card_id.starts_with("tile:")
        {
            return -1;
        }
        if matches!(call, Call::Buy { .. } | Call::Build { .. }) && card_id.starts_with("tile:") {
            return -1;
        }
        // A copied entry (`copies_previous`) is the **source** card's entry:
        // its label and its silent / quiet / flash policy are the source's,
        // not the copying card's (design G06 §2.15). The header still names
        // the card the player actually activated (`card_id`).
        let entry_card = trigger.copied.as_deref().unwrap_or(card_id);
        // Silent entry (`On::silent`): bookkeeping. Body runs, no flash
        // and no 「效果适用」 header. `QuietWhenEmpty` still announces here;
        // the header is retracted at commit if the body produced nothing.
        if self.entry_presentation(entry_card, entry).header == card_sdk::abi::Presentation::Silent
        {
            return -1;
        }
        use game_core::state::card_trigger;
        let kind = match call {
            Call::Counteract { .. } => card_trigger::COUNTER,
            Call::Play { .. } => {
                if card_id.starts_with("event:") {
                    card_trigger::EVENT
                } else if card_id.starts_with("skill:") {
                    card_trigger::SKILL
                } else {
                    card_trigger::PLAY
                }
            }
            // `On::Action` / `DiceRollPlan` / `AtEnd` (and a card's settle body):
            // a placed card's effect firing on someone else's turn.
            _ => card_trigger::HOOK,
        };
        // An event instance is bound under `event:<id>`; the draw's own event
        // names it without the prefix, so the flash and the two log lines agree.
        let cid = card_id.strip_prefix("event:").unwrap_or(card_id);
        // The seat the instance sits on (not `FieldCard::owner`, which names
        // who the card belongs to).
        let owner = match if uid >= 0 { w.world.holder_of(uid) } else { -1 } {
            h if h >= 0 => h,
            _ => call.player_id(),
        };
        let who = if owner >= 0 { owner } else { target };
        // Entry label (「（1）」 and kin) from the manifest, when the card
        // declared one for this entry.
        let label = self.entry_label(entry_card, call.clone(), entry);
        // Wording:
        // * a skill press has no declaration line of its own -> 「发动」
        // * a hand play / a counteraction declaration / a drawn event already logged
        //   their action (打出 / 抽到); their flash keeps no line of its own
        //   (the player's action is the reason)
        // * everything else that runs as an already-in-play card or a
        //   resolution -> 「效果适用」 + why + (on commit) the outcome
        let msg = match kind {
            card_trigger::SKILL => Msg::new("log.card_activated")
                .player_id("who", who)
                .card("card", cid),
            card_trigger::HOOK | card_trigger::COUNTER => {
                let reason = if matches!(call, Call::Counteract { .. }) {
                    // Resolution names the answered link's card and what it
                    // was doing (the declaration already logged 打出counteraction).
                    crate::counteract_reason(trigger.card.as_deref().unwrap_or(""), trigger)
                } else {
                    crate::trigger_reason(trigger)
                };
                Msg::new("log.effect_applied")
                    .card("card", cid)
                    .text("label", label)
                    .msg("reason", reason)
            }
            _ => Msg::default(),
        };
        // Ruling 2a: the header is lazy for **every** entry kind. It is
        // buffered (with the field `uid` for the slot pulse) and posted
        // immediately before the drive's first visible outcome; a drive that
        // ends with nothing visible (a declined optional prompt, a
        // bookkeeping write) posts no header at all.
        w.pending_header = Some(PendingHeader {
            kind: kind.to_string(),
            owner,
            msg,
            card: cid.to_string(),
            target,
            tile,
            uid,
        });
        // Parent for body lines is set when the header flushes.
        -1
    }

    /// The manifest label of `card_id`'s `entry` (「（1）」 / 「（2）」), or `""`.
    fn entry_label(&self, card_id: &str, call: Call, entry: i32) -> String {
        let _ = call;
        let Some(idx) = self.ruleset.card(card_id) else {
            return String::new();
        };
        self.ruleset
            .cards()
            .get(idx as usize)
            .and_then(|c| c.on.get(entry.max(0) as usize))
            .and_then(|o| o.label.clone())
            .unwrap_or_default()
    }

    /// How `card_id`'s `entry` presents itself: the header mode plus the
    /// flash policy. Both ride the entry (`On::silent` / `On::quiet_when_empty`
    /// / `On::flash`); the defaults (`Visible` + `Auto`) when the card or entry
    /// is unknown.
    fn entry_presentation(&self, card_id: &str, entry: i32) -> card_sdk::abi::EntryPresentation {
        let Some(idx) = self.ruleset.card(card_id) else {
            return card_sdk::abi::EntryPresentation::default();
        };
        let on = self
            .ruleset
            .cards()
            .get(idx as usize)
            .and_then(|c| c.on.get(entry.max(0) as usize));
        card_sdk::abi::EntryPresentation {
            header: on.map(|o| o.presentation).unwrap_or_default(),
            flash: on.map(|o| o.flash).unwrap_or_default(),
        }
    }

    /// Run one effect to completion, prompting through the engine as needed. Returns
    /// the card's destination (`set_dest`).
    fn drive(
        &self,
        cx: &mut PlayCtx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
    ) -> Flow<i32> {
        let r = self.drive_inner(cx, call, card_id, uid, trigger, false);
        r
    }

    /// As [`drive`], but ask the hook's guard first and skip the body when it
    /// refuses -- on the same instantiation, so a run hook costs one linker init
    /// and one world copy rather than the two a `hook_guard` + `drive` pair
    /// cost. A body that runs announces itself once, on the first pass (a
    /// prompt re-runs the module) -- see [`Self::announce_drive`].
    fn drive_hook(
        &self,
        cx: &mut PlayCtx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
    ) -> Flow<i32> {
        // A hook cannot re-trigger on its own money movement.
        if uid >= 0 && is_money_hook_kind(trigger.kind) && cx.reentrant_hooks.contains(&uid) {
            return Ok(0);
        }
        // v78: pre-roll a recorded die into the world so a DiceRollPlan /
        // DiceRollAdd / DiceRollValue Filter body can read `RollValue.recorded_roll` instead
        // of calling `roll` (pure + replay-deterministic: the same RNG stream).
        if matches!(
            trigger.kind,
            TriggerKind::DiceRollPlan | TriggerKind::DiceRollAdd | TriggerKind::DiceRollValue
        ) {
            // Peek the next face from a *clone* of the stream so the main
            // dice stream is unchanged (existing tests / records keep their
            // faces) while `recorded_roll` is still deterministic per raise.
            let mut w = cx.world_copy();
            let mut peek = w.rng.clone();
            w.recorded_roll = Some(peek.d(6) as i32);
            cx.swap_world(w);
        }
        // The body is resolving from here until the raise behind it is done:
        // push its frame now (the guest cannot -- it returns before the hooks
        // its money moves raised are walked) and let `raise_core` drop it.
        push_resolving_frame(card_id, uid, drive_seat(&call), trigger.uid);
        let r = self.drive_inner(cx, call, card_id, uid, trigger, true);
        r
    }

    fn drive_inner(
        &self,
        cx: &mut PlayCtx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
        guarded: bool,
    ) -> Flow<i32> {
        // STACK-01 resume: this drive already completed before a nested-settle
        // suspend; do not run its body again.
        if cx.take_drive_skip() {
            return Ok(DEST_UNSET);
        }
        // A hook cannot re-trigger on its own movement: while this uid's hook
        // runs, nested money pipelines skip it. This is the termination
        // argument for `MAX_MONEY_DEPTH` (see `play.rs`).
        //
        // `On::Play` is an activation, not a hook: its body may drive a move
        // whose settle pays money, and that card's own `payCombinedCancel` /
        // `paySplitAdd` hooks must still hear it (MyGO 「投币式停车场的猫」 (2)
        // 「免除付款」). Only hook bodies mark the uid busy.
        let marked = uid >= 0 && !matches!(call, Call::Play { .. });
        if marked {
            cx.reentrant_hooks.push(uid);
        }
        let result = self.drive_inner_body(cx, call, card_id, uid, trigger, guarded);
        if marked {
            cx.reentrant_hooks.pop();
        }
        result
    }

    fn drive_inner_body(
        &self,
        cx: &mut PlayCtx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
        guarded: bool,
    ) -> Flow<i32> {
        self.drive_inner_body_from(
            cx,
            call,
            card_id,
            uid,
            trigger,
            guarded,
            Default::default(),
            false,
            Vec::new(),
        )
    }

    /// [`Self::drive_inner_body`] with a pre-filled answer log (STACK-01
    /// resume: continue a drive that paused at a nested `settle_tile_at`).
    ///
    /// Wraps the body in its command scope (`enter_plan_command` /
    /// `leave_plan_command`): the staging snapshot is taken on the way in and,
    /// at the outermost leave, restored once the command has driven a move.
    /// `resumed` is `true` only from the resume arm -- the paused world still
    /// holds the command, so it must not snapshot twice.
    /// Queue a re-run of the body whose host request just suspended, to run
    /// **after** the nested work the suspension parked (a counter body, a join
    /// window, a settle). The drive's host-answer log is carried, so the
    /// requests already applied before the suspension are consumed, not
    /// repeated; the request that suspended runs again and completes.
    fn push_body_resume(
        &self,
        cx: &mut PlayCtx,
        call: Call,
        card_id: &str,
        uid: i32,
        guarded: bool,
        trigger: &Trigger,
        log: game_core::engine::GuestLog,
        adopted: Vec<i32>,
    ) {
        let bridge = self as *const RulesBridge<M>;
        let card_id_own = card_id.to_string();
        let mut trigger_own = trigger.clone();
        // The counter node this body settles, when it is one.
        let node_own: Option<usize> = cx
            .counteract_chain()
            .and_then(|c| c.downcast_ref::<CounteractChainCursor>())
            .map(|c| c.index);
        eprintln!("PUSH_RESUME card={card_id}");
        cx.insert_work(game_core::engine::ResumeFnWork {
            f: Box::new(move |cx| {
                eprintln!("RESUME_FN_RUN card={}", card_id_own);
                if uid >= 0 {
                    cx.reentrant_hooks.push(uid);
                }
                // A counter body's chain is parked in the resume; keep it
                // parked and drive through the stable pointer (a nested
                // suspension must be able to park into the same slot).
                let mut chain_slot = node_own.and(take_chain_box(cx));
                let mut chain_ptr = chain_slot
                    .as_ref()
                    .map(|b| &**b as *const Vec<ChainLink> as *mut Vec<ChainLink>);
                if let Some(chain) = chain_slot.take() {
                    park_chain_box(cx, &trigger_own, chain);
                }
                let before_face = trigger_own
                    .mv
                    .as_ref()
                    .and_then(|m| m.roll)
                    .unwrap_or(trigger_own.value);
                let bridge = unsafe { &*bridge };
                let r = bridge.drive_inner_body_from(
                    cx,
                    call,
                    &card_id_own,
                    uid,
                    &mut trigger_own,
                    guarded,
                    log,
                    true,
                    adopted,
                );
                if uid >= 0 {
                    cx.reentrant_hooks.pop();
                }
                match r {
                    Ok(dest) => {
                        if let (Some(node), Some(ptr)) = (node_own, chain_ptr.take()) {
                            let chain = unsafe { &mut *ptr };
                            bridge.close_drive(cx, chain, node, &mut trigger_own, before_face)?;
                            let seat = chain[node].seat;
                            let id = chain[node].id.clone();
                            let uid = chain[node].uid;
                            let negated = chain[node].link.is_cancelled();
                            bridge.settle_tail(cx, seat, &id, uid, negated, dest)?;
                        }
                        cx.set_drive_dest(dest);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }),
        });
    }

    fn drive_inner_body_from(
        &self,
        cx: &mut PlayCtx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
        guarded: bool,
        log: game_core::engine::GuestLog,
        resumed: bool,
        adopted: Vec<i32>,
    ) -> Flow<i32> {
        cx.world_mut().enter_plan_command(resumed);
        // This is a body run of its own: completed rows inherited from a
        // previous routine run are a dead sequence here, not a prefix to
        // re-wind to (a resume continues the same run and keeps the number).
        let mut log = log;
        if !resumed {
            log.fresh = log.fresh.wrapping_add(1);
        }
        let r = self.drive_inner_body_scoped(
            cx, call, card_id, uid, trigger, guarded, log, resumed, adopted,
        );
        cx.world_mut().leave_plan_command();
        r
    }

    fn drive_inner_body_scoped(
        &self,
        cx: &mut PlayCtx,
        call: Call,
        card_id: &str,
        uid: i32,
        trigger: &mut Trigger,
        guarded: bool,
        prefill: game_core::engine::GuestLog,
        resumed: bool,
        adopted: Vec<i32>,
    ) -> Flow<i32> {
        // A fresh execution (a resume pre-fills `log`) gets a ledger id of
        // its own, so the previous execution's writes cannot suppress this
        // one's. A resume keeps the id it was started with.
        if prefill.answers.is_empty() {
            cx.reset_factor_ledger();
        }
        // A drive's answers are its own (G10 §6.3.3): the body re-runs from
        // the top on every pass and re-consumes them from 0, so a sibling
        // drive's log -- or the one a torn pass parked on `Pending` -- is not
        // this drive's list. Its prompts are re-asked and the engine answers
        // them from the replay. Only a body resume (a suspension the work item
        // parked, carrying the log the earlier pass consumed) pre-fills one;
        // its journal is re-marked so each in-flight entry is re-applied at
        // its own site.
        let mut log = prefill;
        // A fresh drive starts at the cursor it was handed; a nested drive
        // (`HostState::new` hands it the parent's log) is handed the parent's
        // positional cursor, or both bodies would read the same answers -- the
        // enclosing request's own answer is not written yet, so the nested
        // body's prompts sit after it.
        //
        // A body *resume* is not a new drive: it re-enters the same run after
        // the nested work its suspension parked, and every earlier pass of
        // that run started at `log.start`. Restarting at the pause cursor
        // instead would put the body's entry announcement on the suspended
        // request's own slot, so a reopen on the next pass would consume the
        // slot after it (a join counter drew twice) and the run would shift
        // one slot deeper on every pass.
        let drive_start_cursor = if resumed { log.start } else { log.cursor };
        log.start = drive_start_cursor;
        if !log.journal.is_empty() {
            log.seed_reapply();
        }
        // This pass re-runs from the routine's snapshot, so every logged
        // request has to be applied again the first time the body reaches it
        // (`inline::request_or_pause` step 2); `applied` is cleared at each
        // pass top for that. Within the pass, the loop repeats after each
        // request and must reuse the live effect instead.
        let ledger_exec = cx.ledger_exec();
        // The pile prefix: a pile write is not replayed (a draw's reshuffle
        // moves the RNG, an inspection reshuffles), so the replay rebuilds the
        // drive's pile state from the world it started in. The handle is
        // shared -- free -- and only deep-cloned at the first such request,
        // which keeps pile-free drives cheap. It has to be the **drive start**,
        // not the request site: the body may already have written piles this
        // pass (a `ctx::draw` after a refill), and a request-site base would
        // then already contain them (G10 §6.3.6).
        let pile_start = cx.share_world();
        let mut pile_base: Option<PileCheckpoint> = None;
        let mut pile_checkpoints = std::collections::BTreeMap::new();
        // Inspection frames this drive owns (adopted on an earlier pass or
        // handed in by a resume); the commit net releases exactly these plus
        // everything opened since the pass's own base (design G11 §5.4).
        let mut drive_adopted: Vec<i32> = adopted;
        // Guest-state overlays this drive pushed (one per `NeedHost`). Dropped
        // at the top of the next iteration -- the routine has finished and the
        // replay re-applies the guest writes from the restored world. Nested
        // drives record their own base, so they never pop an outer one.
        let overlay_base = cx.guest_overlay_depth();
        // The activation announcement runs at **body entry** (see
        // [`Self::announce_drive`]): `run` / `run_hook` call this on the world
        // copy the body will write into, only once the entry exists and the
        // condition + guard admitted. Multi-pass re-runs announce only the
        // first time each entry runs (the landing pass inherits that announce
        // through the code-order merge in [`Self::commit_after`]).
        // It doubles as the re-entry admission set: an entry listed here
        // already cleared its condition and residual guard on an earlier pass,
        // and its own body writes may now refuse them (`unplace_self` under a
        // `card.placed` condition), so a re-run replays only its declare.
        let mut announced: std::collections::BTreeSet<i32> = Default::default();
        let mut activation_id = cx.activation.last().copied().unwrap_or(-1);
        // The header **this** drive posted (the 「效果适用」 event it must
        // complete with 「（无事发生）」). Distinct from `activation_id`, which
        // is the parent for body lines and starts as the *outer* activation.
        // `-1` when the drive is silent or posts no header (tile settle).
        let mut own_activation = -1i32;
        // Header mode folded across sibling entries (see [`fold_header`]).
        // `Silent` is the fold identity -- start there and fold each entry in.
        // The bool cells below are derived from the fold so `commit_after`
        // keeps its two-flag signature. `silent_body` is set at body entry
        // when the (folded) mode is silent; the commit pass then checks the
        // run produced no visible effect (the flag must not hide a real one).
        // `Cell` so the `announce` closure can share it (same shape as the
        // raw `cx_ptr` write-through).
        let header_mode = std::cell::Cell::new(card_sdk::abi::Presentation::Silent);
        let silent_body = std::cell::Cell::new(false);
        let quiet_when_empty = std::cell::Cell::new(false);
        // Declarative flash policy (v55) rides across passes like silent_body:
        // the commit pass's announce is a no-op and must not lose it.
        let flash_policy_cell: std::cell::Cell<card_sdk::abi::Flash> =
            std::cell::Cell::new(card_sdk::abi::Flash::Auto);
        // "Was the player cued?" -- OR of `ctx::flash()` and any
        // `Flash::Always` sibling. Never folded, never reset per entry (see
        // [`Run::flash_cued`]). Separate from `flash_policy_cell` because
        // `Never` can win the flash decision while `Always` already cued.
        let flash_cued_cell = std::cell::Cell::new(false);
        let mut prompted = false;
        // Ruling 2a: the lazy header survives across passes (learn + commit
        // run share one buffer; a pause must not drop it).
        let mut saved_header: Option<PendingHeader> = None;
        let mut flushed_header: i32 = -1;
        // Every header the drive has posted across passes (learn + re-run).
        // A re-run's fresh `Run` starts from this so `commit_after` can stamp
        // the folded flash decision on headers the learn pass already posted.
        let mut drive_header_ids: Vec<i32> = Vec::new();
        let mut posted_ids: Vec<i32> = Vec::new();
        let mut place_log: Vec<i32> = Vec::new();
        // A re-run starts from the world the body **entered** with: its own
        // requests are re-applied at the same point they were issued
        // (`CardWorld::replay_host`), so the area between here and there must
        // be the entry state, not whatever the last pass left. The row
        // overlays carry the pre-overlay row of every routine-delivered
        // `Leave` / `Detach`, and the loop top puts them back (see
        // `PlayCtx::restore_guests_to`); the answered request's pile half is
        // re-supplied by its `PileCheckpoint`.
        let (target, tile) = (trigger.player_id, trigger.tile);
        let drive_id = cx.next_drive_seq();
        // A `Move` paused on the previous pass: the body derives its
        // destination from the mover's position, so the pass that re-derives
        // it has to start where the request did. The landing is put back at
        // the request site (`after_host`), or by the drift's re-application.
        let mut pending_move: Option<(i32, i32)> = None;
        // Rows the restores put back for the re-run's reads, for the whole
        // drive: a row the replay never takes out again must not survive the
        // commit (see `commit_after`).
        let mut reinserted: Vec<i32> = Vec::new();
        // The single commit pass (simulation mode): the next `Done` commits
        // instead of starting one more pass.
        let mut in_commit = false;
        loop {
            {
                let ids: Vec<(i32, String)> = cx
                    .world()
                    .recent
                    .iter()
                    .map(|e| (e.id, e.r#type.clone()))
                    .collect();
            }
            log.cursor = drive_start_cursor;
            log.applied.clear();
            cx.restore_guests_to(overlay_base, drive_id);
            reinserted.extend(cx.take_reinserted());
            // The body derives a move's destination from the mover's position,
            // so a re-run has to start where the move did: give the landing of
            // every move this run already applied back (its `condition`,
            // earliest first per mover). The re-run reaches those slots again
            // and either reuses the answer -- `after_host` re-imposes the
            // landing -- or re-applies the move from this position.
            let mut rewound: Vec<i32> = Vec::new();
            for (&slot, e) in log.last.range(drive_start_cursor..) {
                if e.generation != log.fresh
                    || !matches!(e.request, game_core::engine::JournalRequest::Move { .. })
                    || rewound.contains(&e.player)
                {
                    continue;
                }
                rewound.push(e.player);
                let seat = e.player.max(0) as usize;
                if let Some(pos) = e.condition.st.players.get(seat).map(|p| p.pos) {
                    if let Some(p) = cx.world_mut().st.players.get_mut(seat) {
                        p.pos = pos;
                    }
                }
            }
            if let Some((seat, pos)) = pending_move.take() {
                if !rewound.contains(&seat) {
                    if let Some(p) = cx.world_mut().st.players.get_mut(seat.max(0) as usize) {
                        p.pos = pos;
                    }
                }
            }
            // Share the live world (fix B): the body's writes detach a private
            // copy; a body that never writes and has no pile base never clones.
            let pass_base = cx.pass_base();
            let mut world = cx.share_world();
            if std::env::var("DBG_H01").is_ok() {
                eprintln!(
                    "H01 PASS TOP card={card_id} owner16={:?} answers={:?} cursor={}",
                    world.st.owners.get(16),
                    log.answers,
                    log.cursor
                );
            }
            if let Some(base) = &pile_base {
                base.apply(&mut world);
                cx.restore_pile_base(&base.world);
            }
            let frames_at_drive_start: Vec<i32> =
                world.inspection_frames.iter().map(|f| f.id).collect();
            let mut run = Run {
                world,
                pass_base,
                overlay_floor: overlay_base,
                drive_id,
                pile_checkpoints: Arc::new(pile_checkpoints.clone()),
                data: self.data.clone(),
                props: self.modules_props(),
                trigger: trigger.clone(),
                fig: None,
                ledger_exec,
                activation_player: call.player_id(),
                current_card: card_id.to_string(),
                current_uid: uid,
                carried_owner: None,
                dest: DEST_UNSET,
                dest_to: None,
                dest_cause: None,
                reinserted: reinserted.clone(),
                paid_log: vec![],
                discard_log: vec![],
                draw_log: vec![],
                draw_after_log: vec![],
                frames_at_drive_start,
                adopted: drive_adopted.clone(),
                hand_log: vec![],
                reshuffle_log: vec![],
                marker_spent_log: vec![],
                marker_changed_log: vec![],
                house_log: vec![],
                counter_log: vec![],
                named_raise_log: vec![],
                state_log: vec![],
                doubled: -1,
                linger_props: Default::default(),
                wrap_effect: Self::wraps_effect(&call, card_id),
                live: Some(cx as *mut PlayCtx<'_> as *mut PlayCtx<'static>),
                posted: std::mem::take(&mut posted_ids),
                log_index: 0,
                place_log: place_log.clone(),
                place_index: 0,
                logged: 0,
                silent_body: false,
                quiet_when_empty: false,
                pending_header: None,
                own_activation: -1,
                header_ids: drive_header_ids.clone(),
                prompted: false,
                parent: activation_id,
                flash_policy: card_sdk::abi::Flash::Auto,
                flash_cued: false,
                flash_ordinal: 0,
            };
            run.world.skip_log = true;
            // Simulation mode (`docs/BOT.md` §3.2): with a provider installed
            // the body must run **once** -- every pause is answered inline
            // (see `crate::inline` / [`DriveInline`]), so the loop below runs
            // a single pass. The default (no provider) path is unchanged.
            //
            // The guest call is the only thing the inline host wraps: it must
            // not hold a borrow of `cx` across it (the host reaches `cx`
            // through a raw pointer from inside the guest's host imports).
            let mut inline = cx.has_provider().then(|| DriveInline {
                bridge: self as *const _,
                cx: cx as *mut PlayCtx<'_> as *mut PlayCtx<'static>,
                call: call.clone(),
                card_id: card_id as *const str,
                halt: None,
            });
            let cx_ptr = cx as *mut PlayCtx<'_>;
            // Taken before the closure borrows `announced`; a fresh `Run` per
            // pass starts from the entry state, so the set must be too.
            let already_admitted: Vec<i32> = announced.iter().copied().collect();
            let mut announce = |w: &mut crate::Run, entry: i32| {
                if !announced.insert(entry) {
                    // A multi-pass re-run. The learn pass either flushed the
                    // lazy header (own_activation >= 0 -- adopt it as the
                    // parent so body lines group under it) or never got a
                    // visible outcome (restore the buffer for this pass).
                    w.parent = if flushed_header >= 0 {
                        flushed_header
                    } else {
                        activation_id
                    };
                    w.own_activation = flushed_header;
                    w.live = Some(cx_ptr as *mut PlayCtx<'static>);
                    if flushed_header < 0 && w.pending_header.is_none() {
                        w.pending_header = saved_header.clone();
                    }
                    // Restore the drive's already-folded flash policy (do
                    // **not** fold this entry again) so the body's `Never`
                    // check still sees the sibling's opinion on a re-run.
                    w.flash_policy = flash_policy_cell.get();
                    // Same for the header mode and the cue flag: a fresh `Run`
                    // starts `silent_body = false`, and a silent entry's
                    // bookkeeping body must not post an outcome line via
                    // `outcome_plan` on the re-run just because the cell was
                    // not copied across.
                    let folded_hdr = header_mode.get();
                    w.silent_body = folded_hdr == card_sdk::abi::Presentation::Silent;
                    w.quiet_when_empty = folded_hdr == card_sdk::abi::Presentation::QuietWhenEmpty;
                    w.flash_cued = flash_cued_cell.get();
                    return;
                }
                w.parent = activation_id;
                w.live = Some(cx_ptr as *mut PlayCtx<'static>);
                let pres = self.entry_presentation(card_id, entry);
                // Header mode folds across siblings (see [`fold_header`]):
                // one decision per drive, so `Visible` wins over
                // `QuietWhenEmpty` wins over `Silent` -- a trailing silent
                // sibling must not poison a leading visible one, and a
                // trailing `QuietWhenEmpty` must not retract a leading
                // `Visible`'s header on an empty body. Do not overwrite per
                // entry. The `announced` early-return above fires before any
                // fold, so a re-run does not fold the same entry twice.
                let folded_hdr = fold_header(header_mode.get(), pres.header);
                header_mode.set(folded_hdr);
                let is_silent = folded_hdr == card_sdk::abi::Presentation::Silent;
                let is_quiet = folded_hdr == card_sdk::abi::Presentation::QuietWhenEmpty;
                silent_body.set(is_silent);
                w.silent_body = is_silent;
                quiet_when_empty.set(is_quiet);
                w.quiet_when_empty = is_quiet;
                // Declarative flash policy (v55) rides on the Run so the
                // commit's precedence check can see `Never` / `Always`.
                // One decision per drive, so the sibling entries **fold**
                // (`Never` if any says `Never`; else `Always` if any says
                // `Always`; else `Auto`) rather than overwrite -- a trailing
                // `Auto` must not drop an earlier `Always`, and a trailing
                // `Always` must not lift an earlier `Never`. Both stores
                // carry the same folded value so they cannot diverge. The
                // `announced` early-return above fires before any fold, so a
                // re-run does not fold the same entry twice.
                let folded = fold_flash(flash_policy_cell.get(), pres.flash);
                flash_policy_cell.set(folded);
                w.flash_policy = folded;
                // "Was the player cued?" ORs across entries and is **not**
                // the folded decision: an `Always` sibling cues (and flushes
                // its header) even when a `Never` sibling wins the flash.
                if pres.flash == card_sdk::abi::Flash::Always {
                    flash_cued_cell.set(true);
                }
                w.flash_cued = flash_cued_cell.get();
                let id = self.announce_drive(
                    w,
                    call.clone(),
                    card_id,
                    uid,
                    target,
                    tile,
                    trigger,
                    entry,
                );
                if w.pending_header.is_some() {
                    saved_header = w.pending_header.clone();
                }
                if pres.flash == card_sdk::abi::Flash::Always {
                    // Declarative `Always` (`On::flash`) is the same promise
                    // as the imperative `ctx::flash()`: cue the player. That
                    // promise has to include the lazy 「效果适用」 header -- a
                    // flash with no clause name is noise, and leaving the
                    // header lazy means a no-op body (a capped gain, 「已满」)
                    // never posts one at all, so `Always` would show nothing.
                    // Same move `Run::flash` makes at the flash point; only
                    // now the header exists (`announce_drive` buffered it).
                    w.flush_header();
                }
                if id >= 0 {
                    // Body lines group under **this** drive's header.
                    w.parent = id;
                    if own_activation < 0 {
                        own_activation = id;
                    }
                    activation_id = id;
                    unsafe { (*cx_ptr).activation.push(id) };
                }
            };
            cx.guest_log = std::mem::take(&mut log);
            // A trapping guard / declare declines its entry. That must not be
            // silent (DECLARE-TRAP-LOG): one `log.card_trap` line naming the
            // card and the failure, posted into the world `run_hook` returns.
            // The guard / declare store world is a throwaway clone, so the
            // line goes here, never into the store.
            let mut on_trap = |w: &mut crate::Run, _entry: i32, detail: &str| {
                w.log(
                    call.player_id(),
                    Msg::new("log.card_trap")
                        .card("card", card_id)
                        .text("detail", detail),
                );
            };
            let mut guest = || -> Result<Option<Result<Outcome<Run>, RuleError>>, RuleError> {
                if guarded {
                    match self.ruleset.run_hook(
                        &run,
                        call.clone(),
                        &cx.guest_log,
                        &already_admitted,
                        Some(&mut announce),
                        Some(&mut on_trap),
                    ) {
                        Err(e) => Ok(Some(Err(e))),
                        // Not activated -- the guard refused. Nothing ran, and
                        // nothing about the card reaches the UI.
                        Ok(None) => Ok(None),
                        Ok(Some(hr)) => Ok(Some(Ok(hr.outcome))),
                    }
                } else {
                    Ok(Some(self.ruleset.run(
                        &run,
                        call.clone(),
                        &cx.guest_log,
                        Some(&mut announce),
                    )))
                }
            };
            // `run_hook` cannot actually fail here (its `Err` arm above is
            // unreachable in practice); the double `Result` is so the closure
            // remains a plain `FnOnce` over the borrows it needs.
            let wrapped = match inline.as_mut() {
                Some(h) => crate::inline::with_inline_host(h, guest),
                None => guest(),
            };
            // Harvest the log the guest pass left in the store. It also stays
            // on the context: the answer log is routine-wide (one routine, one
            // positional answer list), and the next drive of the same routine
            // continues where this one left off. An ordinary prompt halt drops
            // it with the context; a body resume carries it in the work item.
            log = std::mem::take(&mut cx.guest_log);
            cx.guest_log = log.clone();
            // The run holds the pass's trigger copy; the engine's is the one a
            // resume (and the commit) carries on. A body that rewrites the
            // trigger and then pauses (`ctx::set_move_roll` in a reroll loop,
            // `set_pay_amount`) must find its write on the next pass, so the
            // guest's copy goes back -- the commit path already does this.
            // The activation flags the body wrote (cancel, negate, spare, pay
            // target) are facts of the activation, not of one pass: keep them
            // on the drive so a re-run -- the next answer's pass or the commit
            // pass -- reads the same ones, whichever way this pass ended.
            *trigger = run.trigger.clone();
            // A nested prompt the provider declined parks a [`Halt`] on the
            // inline host; propagate it and drop the body (the engine routine
            // re-runs from its snapshot, as today).
            if let Some(h) = inline.as_mut().and_then(|h| h.take_halt()) {
                // A halted drive's write-through lines must not survive: the
                // engine routine re-runs from its snapshot, and a second post
                // would duplicate them.
                cx.drop_events(&posted_ids);
                posted_ids.clear();
                // The pass is discarded and the routine re-runs from its
                // snapshot, so the guest-state/row overlays it pushed go back
                // too -- exactly as the loop top does for a re-run. A
                // suspended halt resumes in a **fresh** scope (its own
                // `overlay_base` is the current depth), so an overlay left
                // here would never be popped and the re-run would read its
                // own first attempt back.
                cx.restore_guests_to(overlay_base, drive_id);
                if activation_id >= 0 {
                    cx.activation.retain(|&id| id != activation_id);
                }
                if h.is_suspended() {
                    // A counter walk inside the request suspended: re-drive
                    // this body after the nested work, carrying the log
                    // the inline pass already consumed.
                    let log_own = log.clone();
                    self.push_body_resume(
                        cx,
                        call.clone(),
                        card_id,
                        uid,
                        guarded,
                        trigger,
                        log_own,
                        run.adopted.clone(),
                    );
                }
                cx.guest_log = log;
                return Err(h);
            }
            let outcome = match wrapped {
                Ok(Some(outcome)) => outcome,
                // Guard refused.
                Ok(None) => return Ok(DEST_UNSET),
                Err(e) => Err(e),
            };
            match outcome {
                Ok(Outcome::Done(mut after, done_log)) => {
                    // `run_hook` answers a completed hook with a default log --
                    // the entries' answers are already in the one the call
                    // started from, so keep it then.
                    if !done_log.answers.is_empty() || log.answers.is_empty() {
                        log = done_log;
                    }
                    // The activation flags the body wrote (cancel, spare, pay
                    // target) are facts of the activation, not of one pass:
                    // hand them to the drive now, so the commit pass -- which
                    // re-runs the body from the top -- reads the same ones.

                    // Simulation mode (`docs/BOT.md` §3.2): the learn pass
                    // above ran the body once with every pause answered
                    // inline against the **live** world, so the host effects
                    // are already on `cx.w` and this `after` carries only the
                    // guest's writes. The replay model commits
                    // `guest_writes(live + host_effects)` -- the guest's writes
                    // *on top* of the host effects -- so when any pause was
                    // answered we run one commit pass with the collected
                    // log pre-filled and commit that instead. Two guest
                    // runs per body, not *k*+1, and the same final world.
                    if std::env::var("DBG_H01").is_ok() {
                        eprintln!("H01 pass done card={card_id} answers={:?}", log.answers);
                    }
                    if log.answers.is_empty() || in_commit {
                        let mut after = after;
                        posted_ids = std::mem::take(&mut after.posted);
                        place_log = std::mem::take(&mut after.place_log);
                        if after.own_activation >= 0 {
                            flushed_header = after.own_activation;
                        }
                        after.prompted = prompted;
                        {
                            let mut after = after;
                            after.flash_policy = flash_policy_cell.get();
                            after.flash_cued = flash_cued_cell.get();
                            if after.own_activation < 0 {
                                after.own_activation = flushed_header;
                            }
                            let hdr = if flushed_header >= 0 {
                                flushed_header
                            } else {
                                own_activation
                            };
                            return self.commit_after(
                                cx,
                                after,
                                call,
                                card_id,
                                trigger,
                                silent_body.get(),
                                quiet_when_empty.get(),
                                hdr,
                                overlay_base,
                                pile_base.as_ref(),
                            );
                        }
                    }
                    // A pause was answered (simulation mode, docs/BOT.md §3.2):
                    // run one commit pass with the collected log prefilled, so
                    // the guest's writes land on top of the host effects. The
                    // loop top puts this drive's guest writes back off live
                    // first (the commit pass re-runs them) and `in_commit`
                    // routes the next `Done` to `commit_after`.
                    posted_ids = std::mem::take(&mut after.posted);
                    place_log = std::mem::take(&mut after.place_log);
                    if after.own_activation >= 0 {
                        flushed_header = after.own_activation;
                    }
                    if !after.header_ids.is_empty() {
                        drive_header_ids = after.header_ids.clone();
                    }
                    in_commit = true;
                    continue;
                }
                Ok(Outcome::NeedInput(p, mut paused, input_log)) => {
                    log = input_log;
                    let player_id = p.player_id;
                    let kind = p.kind;
                    let ask = prompt_to_ask(p);
                    let items = ask.view.items.clone();
                    // A prompt that suspends the routine (`Halt`) discards this
                    // pass: drop its write-through lines, including the
                    // activation header, so the landing pass is the only
                    // announcement. Without this the first pass leaves an
                    // orphan 「效果适用」 with no children and no
                    // 「（无事发生）」 -- the EMPTY bucket in the acceptance dump.
                    // A prompt means the header is already on the stream --
                    // a QuietWhenEmpty entry keeps it (retraction is only for
                    // drives that never showed anything).
                    prompted = true;
                    let reply = match cx.ask(ask) {
                        Ok(r) => r,
                        Err(e) => {
                            cx.drop_events(&posted_ids);
                            posted_ids.clear();
                            // The prompt shows the world, but nothing of this
                            // pass is committed: the re-run re-derives every
                            // guest write. Rewind the ones this drive crossed
                            // so a write that *creates* (a placement) is not
                            // reached twice by the next pass on top of the
                            // first (an absolute write would just be redone).
                            cx.restore_guests_to(overlay_base, drive_id);
                            return Err(e);
                        }
                    };
                    // The prompt was answered (replay / provider), so the body
                    // runs again with this answer pre-filled. Keep what the
                    // paused pass already posted -- its header and body lines
                    // -- so the re-run replaces them in place; without this
                    // every line logged before the prompt appears twice.
                    if let Some(run) = paused.as_mut() {
                        posted_ids = std::mem::take(&mut run.posted);
                        place_log = std::mem::take(&mut run.place_log);
                        if run.own_activation >= 0 {
                            flushed_header = run.own_activation;
                        }
                        if !run.header_ids.is_empty() {
                            drive_header_ids = run.header_ids.clone();
                        }
                    }
                    // A card prompt has one player: the module's i-th answer is that
                    // player's answer to its i-th prompt.
                    let v = reply
                        .answered
                        .answers
                        .first()
                        .copied()
                        .filter(|&x| x >= 0)
                        .unwrap_or(reply.fallback);
                    log.answers.push(recorded_answer(kind, &items, v));
                    let _ = player_id;
                }
                // A card-driven payment: the same money pipeline as `money()`
                // -- PaySplitAdd -> PaySplitMul -> PaySplitChoose -> PaySplitAt -> the `pay` counteraction
                // window -- then replay the effect with the adjudicated amount
                // (0 = cancelled, and PayAfter runs with 0).
                Ok(Outcome::NeedHost(req, mut run, host_log, journal_start)) => {
                    log = host_log;
                    // A zone request writes piles too (grave/hand), and a
                    // re-run replays its row half; the checkpoint makes the
                    // pile half present in the re-run's world instead, so the
                    // replay must not file it again. It must be the world
                    // **before** the body's own pile writes are overlaid just
                    // below: a body that discarded before asking (the [共鸣]
                    // fuel) would otherwise have the re-run's early reads see
                    // the post-discard piles and skip its own branch.
                    if (matches!(req, HostRequest::Draw { .. }) || zone_req(&req))
                        && pile_base.is_none()
                    {
                        pile_base = Some(PileCheckpoint::new(cx.world_copy()));
                    }
                    // Keep the write-through log across the re-run.
                    posted_ids = std::mem::take(&mut run.posted);
                    place_log = std::mem::take(&mut run.place_log);
                    if run.own_activation >= 0 {
                        flushed_header = run.own_activation;
                    }
                    if !run.header_ids.is_empty() {
                        drive_header_ids = run.header_ids.clone();
                    }
                    if matches!(
                        journal_start,
                        Some(game_core::engine::JournalStart::Replace(_))
                    ) {
                        // A kind drift supersedes the replaced request: its
                        // effect is not replayed (the body now makes a
                        // different call), so the recorded pile half must not
                        // be laid back into the re-run (`Draw` superseded by
                        // the gate's `Pay`). The pass top has already rewound
                        // the piles to the drive start, which is the state the
                        // re-run's own writes replay from.
                        if let Some(game_core::engine::JournalStart::Replace(slot)) = journal_start
                        {
                            let superseded = log.last.get(&slot).is_some_and(|e| {
                                std::mem::discriminant(&e.request)
                                    != std::mem::discriminant(
                                        &game_core::engine::JournalRequest::from(&req),
                                    )
                            });
                            if superseded {
                                pile_checkpoints.remove(&slot);
                            }
                        }
                        // The pass's earlier pauses crossed the body's writes
                        // onto live (the first application's included). The
                        // rewind supersedes them; see the inline arm.
                        cx.restore_guests_to(run.overlay_floor, run.drive_id);
                    }
                    restore_condition(
                        cx,
                        &log,
                        journal_start.unwrap_or(game_core::engine::JournalStart::Push),
                        &req,
                    );
                    if matches!(
                        journal_start,
                        Some(game_core::engine::JournalStart::Replace(_))
                    ) {
                        // The rewind moved the world the guest inherits as
                        // well (the pause site restored the guest copy to the
                        // same condition). Re-base the guest diff on it, or
                        // the commit would cross that restored copy back in
                        // over effects this very pass re-applies after here.
                        run.pass_base = cx.pass_base();
                    }
                    // The host routine runs against the **live** world and is
                    // not replayed, so the turn-ctx policy the card just set up
                    // (build/buy discounts, free buy, ...) has to cross now.
                    // Progress counters remain on the copy: the replay redoes them.
                    cx.adopt_turn_policy(&run.world);
                    // The guest's pending player state (a status clear, e.g.
                    // 壱雫空 zeroing a layer and then paying) is overlaid for the
                    // routine's duration -- `can_pay` must see it -- and dropped
                    // at the next iteration's top, so the replay re-applies it
                    // rather than double-counting. See `PlayCtx::overlay_guest_state`.
                    // The captured overlay also goes on the journal entry.

                    let mut overlay =
                        cx.overlay_guest_state(&run.pass_base, &run.world, run.drive_id);
                    // A `plan::set_tag` made right before a `settleAt` (e.g.
                    // 微笑游行's redirect) must be on the live frame the nested
                    // settle reads (`spec/abi/G04-move-plan.md` §4.3).
                    cx.overlay_guest_plan_tags(&run.world);
                    // Any request that moves cards in or out of a pile has
                    // to be rebuildable by the replay: the pass top rewinds
                    // live's piles to the drive start (a draw's reshuffle
                    // advances the RNG, an inspection shuffles), so the
                    // request's pile write is re-supplied from its checkpoint
                    // rather than re-applied. A leave/detach *to* a pile is
                    // such a write too -- the card lands in the deck or hand
                    // and must survive the rewind.
                    let writes_piles = writes_piles(&req);
                    if writes_piles && pile_base.is_none() {
                        pile_base = Some(PileCheckpoint::new((*pile_start).clone()));
                    }
                    if pile_base.is_some() {
                        // Make preceding pile writes and their refill visible now.
                        // Replay reconstructs the prefix from pile_base, then skips
                        // already-applied writes by adopting these checkpoints.
                        let mut w = cx.world_copy();
                        PileCheckpoint::new(run.world.deep_clone()).apply(&mut w);
                        cx.swap_world(w);
                        for &player in &run.reshuffle_log {
                            self.raise_core(cx, "reshuffled", player, |_| {})?;
                        }
                    }
                    // STACK-01: a nested [触发结算] while the work-stack pump
                    // is running must suspend (the pump runs the settle, then
                    // this drive resumes). The outermost settle is unchanged:
                    // `apply_host_request` → `settle_tile_at` → pump.
                    if let HostRequest::SettleAt {
                        player_id,
                        tile,
                        is_main_move,
                    } = req
                    {
                        if cx.is_draining_work() {
                            let player_id = player_id.max(0) as usize;
                            let tile = tile.max(0) as usize;
                            let mut log2 = log.clone();
                            log2.answers.push(1);
                            let frame_m = cx.card_settle_move(player_id, tile, is_main_move);
                            cx.begin_suspend_group();
                            let bridge = self as *const RulesBridge<M>;
                            let card_id_own = card_id.to_string();
                            let mut trigger_own = trigger.clone();
                            let call_own = call;
                            let uid_own = uid;
                            let guarded_own = guarded;
                            let run_adopted_own = run.adopted.clone();
                            let who = trigger.player_id;
                            // The counter node this body settles, when the
                            // drive is one (design G06 §6.2).
                            let node_own: Option<usize> = cx
                                .counteract_chain()
                                .and_then(|c| c.downcast_ref::<CounteractChainCursor>())
                                .map(|c| c.index);
                            // Remaining counteract work: the caller (drive's
                            // parent) parks a resume; this frame re-enters the
                            // drive itself.
                            cx.push_work(game_core::engine::ResumeFnWork {
                                f: Box::new(move |cx| {
                                    if uid_own >= 0 {
                                        cx.reentrant_hooks.push(uid_own);
                                    }
                                    // A counter body's chain is parked in the
                                    // resume; keep it parked and drive through
                                    // the stable pointer (a nested settle must
                                    // be able to park into the same slot).
                                    let mut chain_slot = node_own.and(take_chain_box(cx));
                                    let mut chain_ptr = chain_slot.as_ref().map(|b| {
                                        &**b as *const Vec<ChainLink> as *mut Vec<ChainLink>
                                    });
                                    if let Some(chain) = chain_slot.take() {
                                        park_chain_box(cx, &trigger_own, chain);
                                    }
                                    let before_face = trigger_own
                                        .mv
                                        .as_ref()
                                        .and_then(|m| m.roll)
                                        .unwrap_or(trigger_own.value);
                                    let bridge = unsafe { &*bridge };
                                    let r = bridge.drive_inner_body_from(
                                        cx,
                                        call_own,
                                        &card_id_own,
                                        uid_own,
                                        &mut trigger_own,
                                        guarded_own,
                                        log2,
                                        true,
                                        run_adopted_own.clone(),
                                    );
                                    if uid_own >= 0 {
                                        cx.reentrant_hooks.pop();
                                    }
                                    match r {
                                        Ok(dest) => {
                                            if let (Some(node), Some(ptr)) =
                                                (node_own, chain_ptr.take())
                                            {
                                                let chain = unsafe { &mut *ptr };
                                                bridge.close_drive(
                                                    cx,
                                                    chain,
                                                    node,
                                                    &mut trigger_own,
                                                    before_face,
                                                )?;
                                                let seat = chain[node].seat;
                                                let id = chain[node].id.clone();
                                                let uid = chain[node].uid;
                                                let negated = chain[node].link.is_cancelled();
                                                bridge.settle_tail(
                                                    cx, seat, &id, uid, negated, dest,
                                                )?;
                                            }
                                            cx.set_drive_dest(dest);
                                            Ok(())
                                        }
                                        Err(e) => Err(e),
                                    }
                                }),
                            });
                            // Same stage order as the outermost settle: the
                            // frame starts at `RaiseTileEffect`, so the pump
                            // opens the tile's effect window before `settle`
                            // (设计 5.2 / 6.1) -- forcing `RaiseSettle` here
                            // would skip it for every nested settle.
                            let fr = game_core::engine::SettleFrame::new(
                                player_id,
                                cx.position_of_tile(tile as i32) as usize,
                                tile as i32,
                                frame_m,
                            );
                            cx.push_work(game_core::engine::SettleWork { frame: fr });
                            let _ = who;
                            // A STACK-01 suspend **resumes** this drive; its
                            // write-through lines must survive. The overlays
                            // go back first: the resume re-enters in a fresh
                            // scope, so a leaked one would never be popped and
                            // the re-run would read its own first attempt.
                            cx.restore_guests_to(overlay_base, drive_id);
                            return Err(game_core::engine::Halt::suspended());
                        }
                    }
                    // 真奈 join (design G06 §6.4): `join::open()` parks this
                    // body and runs the window on the work stack. The logged
                    // answer keeps a replay aligned; the routine runs on every
                    // pass and resumes the parked round.
                    if let HostRequest::JoinOpen { at } = req {
                        if let Some((chain, node)) = cx
                            .counteract_chain()
                            .and_then(|c| c.downcast_ref::<CounteractChainCursor>())
                            .map(|c| (c.chain, c.index))
                        {
                            let mut log2 = log.clone();
                            log2.answers.push(1);
                            cx.begin_suspend_group();
                            cx.push_work(JoinWork {
                                bridge: self as *const RulesBridge<M>,
                                chain,
                                at: at.max(0) as usize,
                                node,
                                call,
                                card_id: card_id.to_string(),
                                uid,
                                guarded,
                                trigger: trigger.clone(),
                                log: log2,
                                adopted: run.adopted.clone(),
                            });
                            // As above: the resume re-enters in a fresh scope.
                            cx.restore_guests_to(overlay_base, drive_id);
                            return Err(game_core::engine::Halt::suspended());
                        }
                    }
                    let req_is_visible = visible_host_request(&req);
                    // Ruling 2a: a host effect is a visible outcome -- flush
                    // the lazy header before it lands, then carry its id
                    // across the re-run.
                    run.flush_header();
                    if run.own_activation >= 0 {
                        flushed_header = run.own_activation;
                    }
                    if !run.header_ids.is_empty() {
                        drive_header_ids = run.header_ids.clone();
                    }
                    let live_before = cx.live_event_count();
                    if std::env::var("DBG_HOOK").is_ok() {
                        eprintln!(
                            "APPLY_REQ card={card_id} draining={} answers={} req={req:?}",
                            cx.is_draining_work(),
                            log.answers.len()
                        );
                    }
                    let linger = run.linger_props.clone();
                    // A zone request takes its row off live; the copy is
                    // reloaded from live below, so the re-run's reads would no
                    // longer see the instance the body is about. Remember the
                    // uid and the slot, and put the row back from the entry's
                    // pre-state once the effect is applied.
                    let zone_row = match &req {
                        HostRequest::Leave { uid, .. } | HostRequest::Detach { uid, .. } => {
                            Some(*uid)
                        }
                        _ => None,
                    };
                    let zone_slot = match journal_start {
                        Some(game_core::engine::JournalStart::Resume(i)) => log.journal[i].slot,
                        Some(game_core::engine::JournalStart::Replace(s)) => s,
                        _ => log.cursor,
                    };
                    let begun = match &req {
                        HostRequest::InspectionBegin { looker, owner, .. } => {
                            Some((*looker, *owner))
                        }
                        _ => None,
                    };
                    let applied = self.apply_host_request(
                        cx,
                        &mut run,
                        req,
                        journal_start.unwrap_or(game_core::engine::JournalStart::Push),
                        &mut log,
                        &mut overlay,
                        &linger,
                        call.clone(),
                        uid,
                        card_id,
                    );
                    let v = match applied {
                        Ok(v) => {
                            // The effect landed on **live**; the guest copy
                            // still predates it and the body reads the result
                            // back (the deed it just wrote, the money it just
                            // paid). Same boundary as `Run::after_host`: adopt
                            // live (host effects) and lay the body's own
                            // writes back on top of it, so the reads are
                            // current. The pass base is untouched: the copy's
                            // writes stay in the commit's diff.
                            let old = std::mem::replace(&mut run.world, cx.share_world());
                            run.carry_play_scope(&old);
                            overlay.apply(&mut run.world);
                            if let Some(uid) = zone_row {
                                if let Some(cond) = log.last.get(&zone_slot).map(|e| &e.condition) {
                                    run.reinsert_row(uid, Some(cond));
                                }
                            }
                            run.world.turn.plan = old.turn.plan.clone();
                            run.world.turn.plan_stack = old.turn.plan_stack.clone();
                            cx.overlay_guest_plan_tags(&old);
                            run.world.skip_log = true;
                            v
                        }
                        Err(e) => {
                            // A plain halt (an ask, the match ending) leaves
                            // live as the shown world: `execute` re-runs the
                            // routine from its snapshot later, so the overlays
                            // are dropped with the context -- but the writes
                            // already crossed at the pause are what the player
                            // is shown, so they stay.
                            if !e.is_suspended() {
                                cx.guest_log = log;
                                *trigger = run.trigger.clone();
                                return Err(e);
                            }
                            // A counter walk inside the request suspended. The
                            // body has no Rust-stack continuation: re-drive it
                            // once the parked walk (and any join window) has
                            // run. The drive's host-answer log is carried, so
                            // the requests already applied are not repeated.
                            self.push_body_resume(
                                cx,
                                call,
                                card_id,
                                uid,
                                guarded,
                                trigger,
                                log.clone(),
                                run.adopted.clone(),
                            );
                            cx.guest_log = log;
                            // The overlays this iteration pushed go back with
                            // the discarded pass; the resume replays them.
                            cx.restore_guests_to(overlay_base, drive_id);
                            // The rows, though, stay for the resume's reads --
                            // as the commit pass stages them (`_keep`): a body
                            // that paused on its own instance still has to be
                            // able to read that instance's counters and
                            // placement before it re-places it. Whatever the
                            // request itself took away is re-applied by the
                            // replayed answer (`CardWorld::replay_host`).
                            // The activation flags the body wrote before the
                            // request (cancel, spare, pay target) are facts of
                            // this activation: the pass never reaches the
                            // commit that would carry them across, so hand
                            // them over now.
                            *trigger = run.trigger.clone();
                            return Err(e);
                        }
                    };
                    // The begin drove a frame: this drive owns it, so the
                    // commit net must release it even when the pass's own base
                    // still holds it (G11 §5.2/§5.4).
                    if let Some((looker, owner)) = begun {
                        let held = cx
                            .world()
                            .inspection_frames
                            .iter()
                            .rev()
                            .find(|f| f.looker == looker && f.deck_owner == owner)
                            .map(|f| f.id);
                        if let Some(id) = held {
                            run.adopted.push(id);
                            drive_adopted.push(id);
                        }
                    }
                    if run.own_activation >= 0 {
                        cx.stamp_parent_since(live_before, run.parent);
                    }
                    // Debug guard: a silent entry must not produce a visible
                    // effect through the host either (`ctx::gain` and kin are
                    // host requests, not Run-local logs).
                    if silent_body.get() {
                        if let Some(what) = req_is_visible {
                            record_silent_visible(card_id, what);
                            debug_assert!(
                                card_id.starts_with("TEST:"),
                                "silent entry on {card_id} produced a visible host effect ({what}); \
                                 `On::silent` must not hide a real effect"
                            );
                        }
                    }
                    // Group the host-effect events under this activation.
                    cx.stamp_parent_since(live_before, activation_id);
                    // Keyed by the answer **slot**: the replay that consumes
                    // this answer reaches it at that cursor and looks the
                    // checkpoint up there (`after_host`). A push leaves the
                    // cursor at `slot + 1`.
                    let answer_slot = log.cursor.saturating_sub(1);
                    if pile_base.is_some() {
                        pile_checkpoints.insert(answer_slot, PileCheckpoint::new(cx.world_copy()));
                    }
                    // The apply already wrote the answer at the entry's slot
                    // and advanced the cursor; the re-run consumes it there.
                    let _ = v;
                    // The guest keeps running in `run.world`; anything the
                    // routine wrote *there* (a recast's new cast, a folded
                    // face) is only on live. Mirror the transactions back so a
                    // read the body makes right after the request (a
                    // `ctx::roll_pick` on the cast it just threw) sees them
                    // instead of the pre-request copy.
                    run.world.open_rolls = cx.world().open_rolls.clone();
                    // Arm the next pass's pre-request position (see
                    // `pending_move`): the landing stays on live for this
                    // pass, and if the body ends here the next statement
                    // re-imposes it before reading.
                    if let Some(e) = log.last.get(&answer_slot) {
                        if matches!(e.request, game_core::engine::JournalRequest::Move { .. }) {
                            let s = e.player.max(0) as usize;
                            if let Some(pos) = e.condition.st.players.get(s).map(|p| p.pos) {
                                pending_move = Some((e.player, pos));
                            }
                        }
                    }
                    // A nested drive (a hook on the request's outcome) may have
                    // crossed its own writes onto live meanwhile. They are part
                    // of the world now: move the diff base past them, or the
                    // next pause would record them as this body's own writes
                    // and a pass-top revert would drop them (the body does not
                    // re-derive a nested drive's effect).
                    // run.pass_base = cx.pass_base();
                }
                Err(e) => {
                    // A trapping module must not take the match down with it.
                    let detail = crate::host::trap_detail(&e);
                    let player_id = call.player_id();
                    cx.log(
                        player_id,
                        Msg::new("log.card_trap")
                            .card("card", card_id)
                            .text("detail", detail),
                    );
                    return Ok(DEST_GRAVEYARD);
                }
            }
        }
    }

    /// Apply one [`HostRequest`] against `cx` and answer the guest with the
    /// value it wants back (`paid.moved()`, the gate's 0/1, ...).
    ///
    /// Shared by the replay path (the drive loop pauses, applies, and re-runs
    /// the body with the answer appended) and the simulation path (the inline
    /// host applies it mid-body against the guest's own world -- see
    /// [`crate::inline`] and `docs/BOT.md` §3.2). The caller runs
    /// `adopt_turn_policy` / `overlay_guest_state` first in the replay path
    /// only: in a single forward pass the guest's writes are already real on
    /// the world the routine mutates.
    /// The shared body of `pay` / `transfer` / `pay_command`: the immune
    /// check, the one `PlayCtx::money` pipeline, and the answer. `receipt` answers
    /// the packed `PayReceipt` (a command) instead of the bare moved amount.
    #[allow(clippy::too_many_arguments)]
    fn pay_through(
        &self,
        cx: &mut PlayCtx,
        call: &Call,
        from: i32,
        to: i32,
        amount: i32,
        kind: game_core::engine::PayKind,
        shape: game_core::engine::PayShape,
        must: bool,
        allow_mortgage: bool,
        total_stage: bool,
        tile: Option<usize>,
        src: Option<crate::Msg>,
        receipt: bool,
    ) -> Flow<i32> {
        let cancelled = card_sdk::abi::pay_receipt::CANCELLED;
        // A player immune to others' effects (`ImmuneAll`) is neither charged
        // nor paid by another player's card -- the payment does not happen.
        let mut immune = false;
        for x in [from, to] {
            if x >= 0 && x != call.player_id() && self.immune(cx, x, call.player_id())? {
                immune = true;
                break;
            }
        }
        if immune {
            return Ok(if receipt { cancelled } else { 0 });
        }
        let mut p = game_core::engine::Pay::new(amount, kind);
        if from >= 0 {
            p.from = Some(from as usize);
        }
        if to >= 0 {
            p.to = Some(to as usize);
        }
        p.shape = shape;
        p.by_card = Some(call.player_id());
        p.runs_total_stage = total_stage;
        p.tile = tile;
        p.must = must;
        p.allow_mortgage = allow_mortgage;
        p.reason = src;
        let paid = cx.money(p)?;
        if !receipt {
            return Ok(paid.moved());
        }
        let mut answer = paid.moved() & card_sdk::abi::pay_receipt::AMOUNT_MASK;
        if shape == game_core::engine::PayShape::Fixed {
            answer |= card_sdk::abi::pay_receipt::FIXED;
        }
        if paid.paid {
            answer |= card_sdk::abi::pay_receipt::SETTLED;
        } else if paid.shortfall {
            answer |= card_sdk::abi::pay_receipt::SHORTFALL;
        } else {
            answer |= cancelled;
        }
        Ok(answer)
    }

    /// Apply one request and record it in the journal (design G10 §6.3.1).
    ///
    /// Every variant is effectful by construction, and any effectful call can
    /// be followed by a prompt in a nested drive -- so the entry goes down
    /// before the body runs and only comes off on its answer. `start` is the
    /// relation the guest pause point harvested: a new entry, an in-flight one
    /// re-applied, or a completed one whose arguments the body recomputed.
    /// `overlay` is the guest state the boundary just applied (the entry keeps
    /// it for the resume path).
    #[allow(clippy::too_many_arguments)]
    fn apply_host_request(
        &self,
        cx: &mut PlayCtx,
        run: &mut Run,
        req: HostRequest,
        start: game_core::engine::JournalStart,
        log: &mut game_core::engine::GuestLog,
        overlay: &mut game_core::engine::GuestOverlay,
        linger_props: &std::collections::BTreeMap<String, i32>,
        call: Call,
        uid: i32,
        card_id: &str,
    ) -> Flow<i32> {
        let seat = call.player_id();
        let request = game_core::engine::JournalRequest::from(&req);
        let is_move = matches!(request, game_core::engine::JournalRequest::Move { .. });
        // A `Replace` that changed the request **kind** (LOY-TSUGU-01) inherits
        // the replaced row's absolute record: its effect already landed, and
        // the slot's pre-state / money result are what every later pass must
        // re-impose. The new request is applied on top, so its own fresh
        // pre-state would name a world the drift is already past.
        let carried = match start {
            game_core::engine::JournalStart::Replace(slot) => log
                .last
                .get(&slot)
                .or_else(|| log.journal.iter().find(|e| e.slot == slot))
                .filter(|e| std::mem::discriminant(&e.request) != std::mem::discriminant(&request))
                .map(|e| e.clone()),
            _ => None,
        };
        let condition = Box::new(
            carried
                .as_ref()
                .map_or_else(|| cx.world_copy(), |e| (*e.condition).clone()),
        );
        let index = match start {
            game_core::engine::JournalStart::Resume(index) => {
                let e = &mut log.journal[index];
                e.request = request;
                e.card = card_id.to_string();
                e.uid = uid;
                e.player = seat;
                e.condition = condition;
                e.linger = linger_props.clone();
                e.guest = overlay.clone();
                e.generation = log.fresh;
                index
            }
            game_core::engine::JournalStart::Push | game_core::engine::JournalStart::Replace(_) => {
                let slot = match start {
                    game_core::engine::JournalStart::Replace(slot) => slot,
                    _ => log.cursor,
                };
                log.journal.push(game_core::engine::HostJournalEntry {
                    slot,
                    request,
                    card: card_id.to_string(),
                    uid,
                    player: seat,
                    condition,
                    linger: linger_props.clone(),
                    guest: overlay.clone(),
                    generation: log.fresh,
                    after_money: None,
                    after_pos: None,
                });
                log.journal.len() - 1
            }
        };
        let res = self.apply_host_request_body(
            cx,
            run,
            req,
            start,
            linger_props,
            call,
            card_id,
            uid,
            seat,
        );
        match res {
            Ok(v) => {
                // A row the routine took off the field is no longer this
                // body's write to cross back: the overlay is laid on the live
                // world below and re-laid on every re-run, and its `added` row
                // would resurrect the instance the request just removed. Drop
                // every uid live no longer holds (a cascade's children too).
                let vanished: Vec<i32> = log.journal[index]
                    .condition
                    .field_uids()
                    .filter(|u| cx.world().field_by_uid(*u).is_none())
                    .collect();
                if !vanished.is_empty() {
                    fn strip(o: &mut game_core::engine::GuestOverlay, gone: &[i32]) {
                        o.field.added.retain(|(_, _, f)| !gone.contains(&f.uid));
                        o.field.changed.retain(|(u, _, _)| !gone.contains(u));
                        o.owners.retain(|(_, u)| !gone.contains(u));
                    }
                    strip(&mut log.journal[index].guest, &vanished);
                    strip(overlay, &vanished);
                }
                log.journal[index].after_money = carried
                    .as_ref()
                    .and_then(|e| e.after_money.clone())
                    .or_else(|| Some(cx.world().st.players.iter().map(|p| p.money).collect()));
                if is_move {
                    log.journal[index].after_pos = cx
                        .world()
                        .st
                        .players
                        .get(seat.max(0) as usize)
                        .map(|p| p.pos);
                } else if let Some(e) = &carried {
                    log.journal[index].after_pos = e.after_pos;
                }
                // The slot is read *now*: a nested insertion during the body
                // shifted it, and the enclosing answer must land after its
                // nested answers.
                let slot = log.journal[index].slot;
                let entry = log.journal.remove(index);
                let replace = matches!(start, game_core::engine::JournalStart::Replace(_));
                if !replace {
                    // Everything still in flight at or after this slot -- the
                    // enclosing request, whose own answer is not written yet --
                    // moves past the answer (design G10 §6.3.4).
                    log.shift_slots(slot);
                }
                if replace {
                    if slot < log.answers.len() {
                        log.answers[slot] = v;
                    } else {
                        log.answers.push(v);
                    }
                } else {
                    log.answers.insert(slot.min(log.answers.len()), v);
                }
                log.last.insert(slot, entry);
                log.cursor = slot + 1;
                Ok(v)
            }
            Err(e) => {
                if e.is_ask() {
                    *log.reapply.entry(log.journal[index].slot).or_default() += 1;
                }
                Err(e)
            }
        }
    }

    fn apply_host_request_body(
        &self,
        cx: &mut PlayCtx,
        run: &mut Run,
        req: HostRequest,
        start: game_core::engine::JournalStart,
        linger_props: &std::collections::BTreeMap<String, i32>,
        call: Call,
        card_id: &str,
        uid: i32,
        seat: i32,
    ) -> Flow<i32> {
        if std::env::var("DBG_FRAME").is_ok() {
            eprintln!("APPLY_REQ_TOP card={card_id} req={req:?}");
        }
        match req {
            HostRequest::Gate { player_id, kind } => {
                let allowed = self.abnormal_gate(cx, player_id, kind, call.player_id())?;
                return Ok(allowed as i32);
            }
            HostRequest::Target {
                player_id,
                tile,
                single_target,
            } => {
                let got = if tile >= 0 {
                    self.target_tile(cx, tile, call.player_id(), card_id)?
                } else {
                    self.target_player(cx, player_id, call.player_id(), card_id, single_target)?
                };
                return Ok(got);
            }
            // ---- G08 zone lifecycle (§7.4) ----------------------------------
            // Every row move goes through the one writer on `PlayCtx`.
            HostRequest::Leave {
                uid,
                destination,
                to,
                cause,
            } => return cx.zone_leave(uid, destination, to, cause),
            HostRequest::Flip {
                uid,
                face_down,
                cause,
            } => return cx.zone_flip(uid, face_down != 0, cause),
            HostRequest::Attach {
                host,
                card,
                cause,
                face_down,
            } => {
                let r = cx.zone_attach(call.player_id(), host, &card, cause, face_down != 0);
                if card.starts_with("tile:") {
                    eprintln!(
                        "ATTACH pid={} host={host} card={card} card_id={card_id} ans={r:?}",
                        call.player_id()
                    );
                }
                return r;
            }
            HostRequest::Detach {
                uid,
                destination,
                cause,
            } => return cx.zone_detach(uid, destination, cause),
            HostRequest::Rehome { tile, owner } => {
                let mut w = cx.world_copy();
                w.set_tile_owner(tile, owner);
                cx.swap_world(w);
                return Ok(1);
            }
            HostRequest::PlayHand { player_id, card } => {
                let p = player_id.max(0) as usize;
                let has = cx
                    .world_mut()
                    .hidden
                    .get(p)
                    .is_some_and(|h| h.hand.iter().any(|c| *c == card));
                if !has {
                    return Ok(0);
                }
                cx.play_from_hand(p, &card)?;
                return Ok(1);
            }
            // The card shaped the plan and asked for the move to run now.
            // The engine runs it (it may prompt), then the effect replays past
            // this call.
            HostRequest::Move {
                player_id,
                mut plan,
            } => {
                // A re-entered (Resume) or recomputed (Replace) move is the
                // `condition` world's move re-run, not a second one: the first
                // application already set the turn's main-move latch, and
                // `run_planned_move` refuses a second main move (design G10
                // §6.3.3).
                if !matches!(start, game_core::engine::JournalStart::Push) {
                    let mut w = cx.world_copy();
                    w.turn.main_moved = false;
                    cx.swap_world(w);
                }
                // An event-driven move is not a main move (`docs/EVENTS.md`):
                // 「移动X」 / 「移动1d20」 go through even when the turn's
                // main move is already spent. Mark it so `run_planned_move` does
                // not consume (or refuse on) `main_moved`.
                if card_id.starts_with("event:") {
                    plan.forced = true;
                }
                // Skill-source tag (cond `move.by_skill`): the calling rule's id.
                if card_id.starts_with("skill:") {
                    plan.by_skill = true;
                }
                // Forced moves go through the abnormal gate so counteraction cards
                // (安可, 像往常一样) get their window. `AbKind::Forced`
                // covers both other-player and self-applied forced moves.
                let allowed =
                    self.abnormal_gate(cx, player_id, crate::AbKind(5), call.player_id())?;
                if allowed {
                    return Ok(cx.run_planned_move(player_id.max(0) as usize, plan)? as i32);
                }
                // Refused: clear every one-shot this attempt wrote. `plan_mut`
                // is the frame when the call sits inside a move, staging
                // otherwise.
                let mut w = cx.world_copy();
                let p = w.plan_mut();
                p.teleport_to = None;
                p.void_walk = false;
                p.start = None;
                p.start_reason.clear();
                p.by_skill = false;
                p.kind = game_core::engine::MoveKind::Walk;
                cx.swap_world(w);
                return Ok(0);
            }
            // A bare jump with no settle (`settles = false`): runs through the
            // engine teleport so the move tail (`moveBefore` / `moveAfter` /
            // `moveResolved`) raises like every other movement case (B41). The
            // engine sets the position; do not also do the old bare write.
            HostRequest::Teleport { player_id, tile } => {
                let allowed =
                    self.abnormal_gate(cx, player_id, crate::AbKind(4), call.player_id())?;
                if allowed {
                    // Skill-source tag (cond `move.by_skill`) from the calling rule.
                    if card_id.starts_with("skill:") {
                        let mut w = cx.world_copy();
                        w.plan_mut().by_skill = true;
                        cx.swap_world(w);
                    }
                    cx.card_teleport(player_id.max(0) as usize, tile.max(0) as usize)?;
                }
                return Ok(allowed as i32);
            }
            // The status's own return clause: engine `teleport_as` with **no**
            // abnormal window (`HostRequest::TeleportAs`). `is_main_move` marks the
            // move as the turn's [主要移动] and sets `turn.main_moved`.
            HostRequest::TeleportAs {
                player_id,
                tile,
                settles,
                is_main_move,
            } => {
                if is_main_move {
                    let mut w = cx.world_copy();
                    w.turn.main_moved = true;
                    cx.swap_world(w);
                }
                // The request's `settles` is authoritative: `teleport_as_depth`
                // re-reads the shared plan after the pass window, and a nested
                // call (a counteraction inside a settling outer move) would
                // otherwise inherit the outer move's `true`. Force it across the
                // call, then put the plan back unless a pass-window counter
                // rewrote it (e.g. `plan::set_settles(true)` arming the settle,
                // which the re-read then honours).
                {
                    let mut w = cx.world_copy();
                    w.plan_mut().settles = settles;
                    cx.swap_world(w);
                }
                cx.teleport_as(
                    player_id.max(0) as usize,
                    tile.max(0) as usize,
                    settles,
                    None,
                    is_main_move,
                )?;
                return Ok(1);
            }
            // The drive's `JoinOpen` arm normally takes this (it needs the
            // answer tree); without one the body keeps going.
            HostRequest::JoinOpen { .. } => return Ok(1),
            // The rest of the routine family -- see `HostRequest`.
            HostRequest::SettleAt {
                player_id,
                tile,
                is_main_move,
            } => {
                cx.settle_tile_at(
                    player_id.max(0) as usize,
                    tile.max(0) as usize,
                    is_main_move,
                )?;
                return Ok(1);
            }
            HostRequest::Buy {
                player_id,
                tile,
                kind,
            } => {
                // HOST-06: the guest's `-> bool` is the real outcome (0 = the
                // engine no-op'd: `BuyGate` / `buyBefore` cancel, already owned,
                // unpaid quote, unpaid money).
                let did = cx.card_buy(player_id.max(0) as usize, tile.max(0) as usize, kind)?;
                return Ok(did as i32);
            }
            // v40 purchase surface (`docs/PURCHASE.md`).
            HostRequest::Acquire {
                player_id,
                from,
                tile,
                price,
            } => {
                let did = cx.card_acquire(
                    player_id.max(0) as usize,
                    from.max(0) as usize,
                    tile.max(0) as usize,
                    price,
                )?;
                return Ok(did as i32);
            }
            HostRequest::AgentOffer {
                player_id,
                agent,
                tile,
                kind,
            } => {
                // The chosen branch of the agent offer (the post-pick commit):
                // buy the unowned tile, build one level on an own one, or half-rent
                // another player's deed. `kind` names the branch the body saw
                // (0 = buy, 1 = build, 2 = half-rent); the engine re-checks the
                // tile's current state so a commit-pass re-run of the body -- whose
                // options list the just-committed effect already reshaped -- cannot
                // double-act (the answer is in the log, so this arm does not
                // re-execute). HOST-06: answers the branch outcome.
                // HOST-13: `agent` is kept on the audit line -- which agent tile
                // offered this branch -- instead of being dropped.
                let p = player_id.max(0) as usize;
                let t = tile.max(0) as usize;
                cx.log(
                    player_id,
                    Msg::new("log.agent_offer_commit")
                        .player_id("who", player_id)
                        .tile("agent", agent)
                        .tile("tile", tile)
                        .i("kind", kind as i64),
                );
                let did = match kind {
                    1 => cx.card_build(p, t)?,
                    2 => {
                        // The agent's 「half-charge的settle」 on another player's deed.
                        cx.card_pay_rent(p, t, true)?;
                        true
                    }
                    _ => cx.card_buy(p, t, 1)?, // `BuyKind::Agent`
                };
                return Ok(did as i32);
            }
            HostRequest::Linger { player_id, expires } => {
                // The instance binds the **running** card's def, and carries
                // whatever props the body parked via `set_prop` against no
                // field instance (`docs/PURCHASE.md` P5).
                cx.card_linger(
                    player_id.max(0) as usize,
                    expires,
                    card_id,
                    linger_props.clone(),
                );
                return Ok(1);
            }
            HostRequest::Build { player_id, tile } => {
                // HOST-06: 1 = a house was added; 0 = `cant_build_on` /
                // `buildBefore` cancel / unpaid cost / max houses.
                let did = cx.card_build(player_id.max(0) as usize, tile.max(0) as usize)?;
                return Ok(did as i32);
            }
            HostRequest::OfferBuild { player_id, tiles } => {
                let tiles: Vec<usize> = tiles
                    .into_iter()
                    .filter(|&t| t >= 0)
                    .map(|t| t as usize)
                    .collect();
                let did = cx.card_offer_build(player_id.max(0) as usize, &tiles, card_id)?;
                return Ok(did as i32);
            }
            HostRequest::Mortgage { player_id, tile } => {
                let did = cx.card_mortgage(player_id.max(0) as usize, tile.max(0) as usize)?;
                return Ok(did as i32);
            }
            HostRequest::ApplyMortgage {
                player_id,
                tile,
                amount,
            } => {
                let got =
                    cx.commit_mortgage(player_id.max(0) as usize, tile.max(0) as usize, amount)?;
                return Ok(got);
            }
            HostRequest::ApplyRedeem {
                player_id,
                tile,
                amount,
            } => {
                let got = cx.commit_redeem(
                    player_id.max(0) as usize,
                    tile.max(0) as usize,
                    amount,
                    None,
                )?;
                return Ok(got);
            }
            HostRequest::DrawEvent { player_id } => {
                cx.card_draw_event_with_cause(player_id.max(0) as usize, uid)?;
                return Ok(1);
            }
            HostRequest::DrawEventAs { player_id, tile } => {
                cx.card_draw_event_as(player_id.max(0) as usize, tile, uid)?;
                return Ok(1);
            }
            HostRequest::EventDefer {
                event,
                player_id,
                tile,
                ordinal,
            } => {
                let v = cx.card_event_defer(event, player_id, tile, ordinal, uid, seat, card_id);
                return Ok(v);
            }
            HostRequest::WorldSnapshot => {
                return Ok(cx.card_world_snapshot(uid));
            }
            HostRequest::WorldRestore { key } => {
                return Ok(cx.card_world_restore(key, uid) as i32);
            }
            HostRequest::WorldSnapshotDrop { key } => {
                cx.card_world_snapshot_drop(key, uid);
                return Ok(1);
            }
            HostRequest::DiscardNextFx { player_id } => {
                // 「若无手牌则弃掉下一张抽到的牌」 -- armed per seat; the draw
                // loop consumes it (play.rs `draw_cards_with_hooks`).
                if let Some(p) = cx.world_mut().st.players.get_mut(player_id.max(0) as usize) {
                    p.discard_next = true;
                }
                return Ok(1);
            }
            HostRequest::PayRent {
                player_id,
                tile,
                half,
            } => {
                cx.card_pay_rent(player_id.max(0) as usize, tile.max(0) as usize, half)?;
                return Ok(1);
            }
            // A rent-shaped 「收费」 at `milli_scale` (1000 = ×1.0) of the tile's
            // ordinary-settle rent -- `ctx::charge_rent`. Same figure as `pay_rent`
            // (`card_charge_rent` -> `rent_figure`), scaled at `scale_settle_payment`,
            // money pipeline `PayKind::Rent`. No settle hooks / buy-build offer.
            // HOST-06: answers the amount actually paid (0 = refused / cancelled).
            HostRequest::ChargeRent {
                player_id,
                tile,
                milli_scale,
            } => {
                let paid = cx.card_charge_rent(
                    player_id.max(0) as usize,
                    tile.max(0) as usize,
                    milli_scale,
                    Some(call.player_id()),
                )?;
                return Ok(paid);
            }
            HostRequest::OfferBuy { player_id, tile } => {
                cx.card_offer_buy(player_id.max(0) as usize, tile.max(0) as usize)?;
                return Ok(1);
            }
            HostRequest::OfferForceBuy { player_id, tile } => {
                cx.card_offer_force_buy(player_id.max(0) as usize, tile.max(0) as usize)?;
                return Ok(1);
            }
            HostRequest::OfferBuildOne { player_id, tile } => {
                cx.card_offer_build_one(player_id.max(0) as usize, tile.max(0) as usize)?;
                return Ok(1);
            }
            // A card- or skill-driven dice roll (v109: a roll is a
            // transaction, design G03 §4.4).
            //
            // * `window = false` (`roll_direct`): a windowless cast -- the die
            //   is thrown and logged, nothing else.
            // * Move + an open move transaction: a **join** recast on it. The
            //   enclosing commit settles the chosen cast, so the rerolled face
            //   passes the whole ladder again (0.5倍速 &c).
            // * Move without one: a **preview** -- cast and `diceRollBefore`,
            //   committed raw. Karaoke's candidates; the real move settles the
            //   face it pins.
            // * Effect: always a child transaction and the whole Effect
            //   ladder, even inside a move window.
            HostRequest::Roll {
                player_id,
                count,
                sides,
                purpose,
                source,
                window,
            } => {
                use game_core::engine::roll::PURPOSE_MOVE;
                let p = player_id.max(0) as usize;
                let src = game_core::engine::rules::RollSource::from_i32(source);
                if !window {
                    return Ok(cx.run_direct_roll(p, purpose, count, sides, src)?);
                }
                let face = if purpose == PURPOSE_MOVE {
                    let id = cx.open_move_roll_id();
                    if cx.can_join() {
                        let view = cx.roll_move_view(id, p);
                        cx.roll_recast(id, src, Some(&view))?
                    } else {
                        let view = cx.roll_move_view(-1, p);
                        cx.run_preview_roll(p, &view, src)?
                    }
                } else {
                    let f = cx.run_effect_roll(p, count, sides, src)?;
                    f
                };
                return Ok(face);
            }
            HostRequest::RollRecast { roll_id, source } => {
                use game_core::engine::rules::RollSource;
                let view = cx.roll_move_view(roll_id, 0);
                return Ok(cx.roll_recast(roll_id, RollSource::from_i32(source), Some(&view))?);
            }
            HostRequest::RollMove { player_id, source } => {
                use game_core::engine::rules::RollSource;
                let p = player_id.max(0) as usize;
                return Ok(cx.run_standalone_roll(p, RollSource::from_i32(source))?);
            }
            // The card announces an acquisition it performed outside the buy
            // routine (tomoe_savior). The `bought` hook chain runs over the
            // field; `by_card` is the run's player.
            HostRequest::RaiseBought { player_id, tile } => {
                // HOST-06: 1 = the hand-over landed.
                let did = cx.card_raise_bought(
                    player_id.max(0) as usize,
                    tile.max(0) as usize,
                    call.player_id().max(0) as usize,
                )?;
                return Ok(did as i32);
            }
            HostRequest::Auction { player_id, tile } => {
                // The ordinary auction (「竞价」, ABI-09): the bids run on the
                // seat time pools; the answer is the winner seat (-1 = none).
                // The requesting seat (`ctx::auction`'s player) opens the bid
                // order -- not necessarily the turn seat for a nested call.
                let winner = cx.auction_deed(
                    tile.max(0) as usize,
                    player_id.max(0) as usize,
                    true,
                    15.0,
                )?;
                return Ok(winner);
            }
            // Guest-raised trigger points (`ctx::raise`): the pass-reward trigger from
            // the start-square tile rule and kin. The kind is the wire name
            // (`TriggerKind::as_str`); the link settles unless a counteraction
            // cancelled it.
            //
            // v78: filter kinds (`TriggerKind::is_filter`) fold the filter chain
            // instead of opening a counteraction window. A typed-payload kind
            // (BuyAssign) carries the deal in `deal_*` and answers the packed
            // rewritten deal (`hostfns::pack_deal`).
            HostRequest::Raise {
                player_id,
                kind,
                value,
                tile,
                position,
                card,
                deal_owner,
                deal_houses,
                deal_mortgaged,
                charge_skill,
            } => {
                let tk = card_sdk::abi::TriggerKind::from_str(&kind);
                let is_deal = tk == card_sdk::abi::TriggerKind::BuyAssign;
                let t = self.raise_core(cx, kind, player_id, |t| {
                    t.value = value;
                    t.tile = tile;
                    t.position = position;
                    t.card = card;
                    // The acting seat is the raise's cause (`t.ByCard`), the
                    // same read every host-side raise answers. A hook guards
                    // 「你的…」 on it (`by == owner`).
                    t.by_card = Some(player_id);
                    t.charge_skill = charge_skill;
                    if is_deal {
                        let mut b = game_core::engine::rules::TriggerBuy::new(
                            game_core::engine::purchase::BuyKind::Land,
                            -1,
                            value,
                        );
                        if deal_owner != i32::MIN {
                            b.deal_owner = Some(game_kinds::Payee::from_i32(deal_owner));
                        }
                        // Flatten boundary: `i32::MIN` on the wire means "leave the
                        // deal's houses alone". `deal()` would store `Some(MIN)` and
                        // the pack/unpack round-trip would clamp that to a raze.
                        if deal_houses != i32::MIN {
                            b.deal_houses = Some(deal_houses);
                        }
                        b.deal_mortgaged = deal_mortgaged != 0;
                        t.buy = Some(b);
                    }
                })?;
                let cancelled = t.is_cancelled();
                if is_deal {
                    // Flatten boundary: `Option<Payee>` / `Option<i32>` -> wire
                    // (`i32::MIN` = no rewrite) for the packed answer.
                    let (o, h, m) = match t.buy {
                        Some(b) => (
                            b.deal_owner.map(|p| p.as_i32()).unwrap_or(deal_owner),
                            b.deal_houses.unwrap_or(deal_houses),
                            b.deal_mortgaged,
                        ),
                        None => (deal_owner, deal_houses, deal_mortgaged != 0),
                    };
                    return Ok(crate::hostfns::pack_deal(cancelled, o, h, m));
                }
                return Ok((!cancelled) as i32);
            }
            // Pay the buy price through the money pipeline (`PlayCtx::money` with
            // `PayKind::Buy`). `kind` is the [`card_sdk::abi::BuyKind`] wire value.
            HostRequest::PayBuy {
                player_id,
                tile,
                amount,
                kind,
            } => {
                let buy_kind = game_core::engine::purchase::BuyKind::from_i32(kind)
                    .unwrap_or(game_core::engine::purchase::BuyKind::Card);
                let mut p = game_core::engine::Pay::new(amount, game_core::engine::PayKind::Buy);
                p.from = Some(player_id.max(0) as usize);
                p.typ = Some(game_core::engine::PayEvent::Lose);
                p.tile = Some(tile.max(0) as usize);
                p.source = "src.buy";
                p.allow_mortgage = matches!(
                    buy_kind,
                    game_core::engine::purchase::BuyKind::Land
                        | game_core::engine::purchase::BuyKind::Agent
                );
                p.text = Some(
                    crate::Msg::new("log.paid_for")
                        .player_id("who", player_id)
                        .tile("tile", tile),
                );
                let paid = cx.money(p)?;
                return Ok(if paid.paid { paid.loss } else { -1 });
            }
            // Pay the build cost through the money pipeline (`PayKind::Build`).
            // The engine applies the same `build_cost_pct` / `build_discount`
            // figures `PlayCtx::build` does, so the body's `build_cost` base and the
            // charge agree.
            HostRequest::PayBuild {
                player_id,
                tile,
                amount,
            } => {
                let mut cost = amount.max(0);
                let mut refund = 0;
                let pct = cx.state().step; // placeholder -- read turn pct below
                let _ = pct;
                {
                    let w = cx.world_copy();
                    let pct = w.turn.build_cost_pct.clamp(0, 100);
                    if pct != 100 {
                        cost = cost * pct / 100;
                    }
                    // 「可溢出」: no clamp to the cost; the excess is refunded.
                    let cut = w.turn.build_discount;
                    if cut > 0 {
                        cost -= cut;
                    }
                    if cost < 0 {
                        refund = -cost;
                        cost = 0;
                    }
                }
                if refund > 0 {
                    let mut w = cx.world_copy();
                    if let Some(s) = w.st.players.get_mut(player_id.max(0) as usize) {
                        s.money += refund;
                    }
                    cx.swap_world(w);
                }
                // One layer pops whether or not the build commits.
                {
                    let mut w = cx.world_copy();
                    if w.turn.build_discount_layers > 0 {
                        w.turn.build_discount_layers -= 1;
                        if w.turn.build_discount_layers == 0 {
                            w.turn.build_discount = 0;
                        }
                    }
                    cx.swap_world(w);
                }
                if cost <= 0 {
                    return Ok(0);
                }
                let mut p = game_core::engine::Pay::new(cost, game_core::engine::PayKind::Build);
                p.from = Some(player_id.max(0) as usize);
                p.typ = Some(game_core::engine::PayEvent::Lose);
                p.tile = Some(tile.max(0) as usize);
                p.source = "src.build";
                p.allow_mortgage = true;
                p.text = Some(
                    crate::Msg::new("log.paid_for_house")
                        .player_id("who", player_id)
                        .tile("tile", tile),
                );
                let paid = cx.money(p)?;
                return Ok(if paid.paid { paid.loss } else { -1 });
            }
            // The build verb with a card-stated charge; no `On::Build` body
            // re-dispatch (a card path is the body already).
            HostRequest::BuildAs {
                player_id,
                tile,
                charge,
                amount,
                numerator,
                denominator,
                owner_required,
            } => {
                let did = cx.card_build_as(
                    player_id.max(0) as usize,
                    tile.max(0) as usize,
                    charge,
                    amount,
                    numerator,
                    denominator,
                    owner_required != 0,
                )?;
                return Ok(did as i32);
            }
            // A guest tile write (`set_houses` / `add_house` / `set_owner`):
            // the engine declares the effect, then applies unless cancelled.
            // `house_log` keeps the raised `houseAdded`/`houseRemoved` pair
            // (`commit_after`) for a non-zero house delta on an owned tile.
            HostRequest::TileWrite {
                operation,
                tile,
                value,
            } => {
                let t = tile.max(0) as usize;
                let by = call.player_id().max(0) as usize;
                let before = *cx.state().houses.get(t).unwrap_or(&0);
                let answer = match operation {
                    crate::host::TileWriteOperation::SetHouses => cx.write_houses(
                        by,
                        t,
                        game_core::engine::HouseWrite::Set(value),
                        Some(by as i32),
                    )?,
                    crate::host::TileWriteOperation::AddHouses => cx.write_houses(
                        by,
                        t,
                        game_core::engine::HouseWrite::Add(value),
                        Some(by as i32),
                    )?,
                    crate::host::TileWriteOperation::SetOwner => {
                        cx.write_owner(by, t, value, Some(by as i32))?
                    }
                };
                if operation != crate::host::TileWriteOperation::SetOwner {
                    let delta = answer - before;
                    let owner = cx.state().owners.get(t).copied().unwrap_or(-1);
                    if delta != 0 && owner >= 0 {
                        run.flush_header();
                        cx.push_pending_house_log(owner, t as i32, answer, delta);
                    }
                }
                return Ok(answer);
            }
            // Write the deed after a buy (`purchase::assign_deed`).
            HostRequest::AssignDeed {
                player_id: _,
                tile,
                owner,
                houses,
                mortgaged,
            } => {
                // A raze is the previous owner's `houseRemoved` (design 5.5);
                // the write itself stays the direct `assign_deed` below.
                let t = tile.max(0) as usize;
                let before = *cx.state().houses.get(t).unwrap_or(&0);
                let previous = cx.state().owners.get(t).copied().unwrap_or(-1);
                let mut w = cx.world_copy();
                game_core::engine::purchase::assign_deed(
                    &mut w,
                    tile.max(0) as usize,
                    owner,
                    houses,
                    mortgaged != 0,
                );
                cx.swap_world(w);
                let after = *cx.state().houses.get(t).unwrap_or(&0);
                if after < before && previous >= 0 {
                    run.flush_header();
                    cx.push_pending_house_log(previous, t as i32, after, after - before);
                }
                return Ok(1);
            }
            // Which option the player's AI would take among agent-offer tiles.
            HostRequest::AiAgentChoice { player_id, tiles } => {
                let tiles: Vec<usize> = tiles
                    .into_iter()
                    .filter(|&t| t >= 0)
                    .map(|t| t as usize)
                    .collect();
                let pick = cx.ai_agent_choice(player_id.max(0) as usize, &tiles);
                return Ok(pick);
            }
            // `ctx::gain_typed`: a bank print through the money pipeline as
            // `Pay::new(_, "gain")` (so the 「支付」 scalars do not reach a print),
            // carrying the Pay's event `typ` and a replacement log line.
            HostRequest::GainTyped {
                player_id,
                amount,
                typ,
                text,
            } => {
                let mut p = game_core::engine::Pay::new(amount, game_core::engine::PayKind::Gain);
                p.to = Some(player_id.max(0) as usize);
                // `Pay::typ` is a [`game_core::engine::PayEvent`]; the guest's
                // event-type key is the short fixed vocabulary the wire spelling
                // names (`"pass"` / `"lose"` / `"gain"` / `"pay"` / `"rent"`).
                p.typ = Some(match typ.as_str() {
                    "pass" => game_core::engine::PayEvent::Pass,
                    "lose" => game_core::engine::PayEvent::Lose,
                    "pay" => game_core::engine::PayEvent::Pay,
                    "rent" => game_core::engine::PayEvent::Rent,
                    _ => game_core::engine::PayEvent::Gain,
                });
                p.text = text;
                p.by_card = Some(call.player_id());
                let paid = cx.money(p)?;
                return Ok(paid.moved());
            }
            // Raw money adjustment (no payment pipeline). 规则书 「不受其他任何效果
            // 影响」 -- the amount moves as-is.
            HostRequest::AdjustMoney {
                player_id,
                delta,
                src,
            } => {
                let msg = src.unwrap_or_else(|| crate::Msg::new("log.part.reason"));
                let mut w;
                let moved = if delta >= 0 {
                    w = cx.world_copy();
                    w.gain_money(player_id, delta, msg.clone())
                } else {
                    // `spendBefore` -- a raw loss raises the window before its
                    // debit, at the full figure; a reduction keeps the payer
                    // solvent, a cancel leaves the cost alone.
                    let amount = cx.spend_before(
                        player_id.max(0) as usize,
                        -1,
                        -delta,
                        Some(call.player_id()),
                    )?;
                    // The counter body may have written the live world (a
                    // `gain_fixed`, a placement, counters). Copy **after** the
                    // window, or `swap_world` below silently reverts those
                    // writes.
                    w = cx.world_copy();
                    let Some(s) = w.st.players.get_mut(player_id.max(0) as usize) else {
                        return Ok(0);
                    };
                    if s.out() {
                        return Ok(0);
                    }
                    let lost = amount.min(s.money).max(0);
                    s.money -= lost;
                    if lost != 0 {
                        w.log(
                            "lose",
                            player_id,
                            crate::Msg::new("log.lose")
                                .player_id("who", player_id)
                                .n("amount", lost as i64)
                                .msg("src", crate::Msg::new("log.part.reason").msg("reason", msg)),
                        );
                    }
                    lost
                };
                cx.swap_world(w);
                return Ok(moved);
            }
            // Marker spend / gain (user ruling 2026-10-07): the marker's own
            // counteraction window opens **before** the markers move. A counteraction
            // that cancels the link stops the move outright (answer `0`); nothing
            // is spent then. Marker *costs* otherwise keep today's timing --
            // spent as the effect resolves, and a whole-effect negation before
            // the body already prevents them.
            HostRequest::Marker {
                player_id,
                name,
                delta,
            } => {
                let kind = if delta < 0 {
                    "markerSpend"
                } else {
                    "markerGain"
                };
                let t = self.raise_core(cx, kind, player_id, |t| {
                    t.value = delta;
                    t.card = Some(name.clone());
                })?;
                return Ok(if t.is_cancelled() { 0 } else { 1 });
            }
            // Run the entire draw now, including immediate empty-deck maintenance
            // and per-card hooks. A later card/event prompt must see the new deck.
            // The negative answer records an already-applied draw so guest replay
            // reports the count without moving those cards a second time.
            HostRequest::Draw {
                player_id,
                n,
                over_hand_limit,
            } => {
                let Ok(player) = usize::try_from(player_id) else {
                    return Ok(-1);
                };
                // 「回合外受到抽卡效果」 (CRYCHIC:优雅的呐喊): a guest-effect draw
                // that lands outside the drawer's own turn. `HostRequest::Draw` is
                // always a card/event rule's `ctx::draw`, so `by_card` is the
                // causing seat (`call.player_id()`).
                // TODO(规则书): 「受到抽卡效果」 -- does it include self-caused
                // draws on someone else's turn? Implemented as effect-caused only
                // (this arm always is), so a self-caused out-of-turn draw still
                // opens the window.
                if n > 0 && player_id >= 0 && player_id != cx.state().turn {
                    let t = self.raise_core(cx, "drawOut", player_id, |t| {
                        t.target = player_id;
                        t.value = n;
                        t.by_card = Some(call.player_id());
                        t.effects.push(game_core::engine::rules::Effect {
                            kind: "drawOut".into(),
                            target: player_id,
                            from: call.player_id(),
                            tile: -1,
                            value: n,
                        });
                    })?;
                    if t.is_cancelled() {
                        return Ok(0);
                    }
                }
                let got =
                    cx.draw_cards_with_hooks(player, n.max(0) as usize, true, over_hand_limit)?;
                return Ok(-got - 1);
            }
            // Turn deck-top cards face up in a frame for `looker` (design G11
            // §5.2). Idempotent across a replayed pass: an unfinished frame of
            // the same (looker, deck) is adopted, and its `revealed` flags skip
            // what was already raised. A public frame publishes each card as it
            // turns; the reveal is a child of the caller's activation.
            HostRequest::InspectionBegin {
                looker,
                owner,
                count,
                public,
            } => {
                let frame_id = {
                    let w = cx.world_mut();
                    match w
                        .inspection_frames
                        .iter()
                        .rev()
                        .find(|f| f.looker == looker && f.deck_owner == owner && !f.finished)
                    {
                        Some(f) => f.id,
                        None => w.open_inspection(looker, owner, count, public),
                    }
                };
                let parent = cx.activation.last().copied().unwrap_or(-1);
                loop {
                    let next = {
                        let w = cx.world_mut();
                        let Some(frame) = w.inspection_frames.iter_mut().find(|f| f.id == frame_id)
                        else {
                            break;
                        };
                        match frame.cards.iter_mut().position(|c| !c.revealed) {
                            Some(k) => {
                                frame.cards[k].revealed = true;
                                Some((frame.cards[k].id.clone(), k as i32 + 1, frame.public))
                            }
                            None => None,
                        }
                    };
                    let Some((card, position, is_public)) = next else {
                        break;
                    };
                    if is_public {
                        let reason = Msg::new("log.inspection_reveal")
                            .card("card", &card)
                            .player_id("who", looker);
                        // Re-published by replacement: the open frame is the
                        // true top of the deck, so a re-applied begin (or any
                        // later card) restates the same prefix instead of
                        // appending a duplicate set. The frame's own cards are
                        // left out -- `public_state` splices the public frame
                        // in front of this list, so including them here would
                        // show each card twice.
                        {
                            let ids =
                                hidden_pile(cx.world(), owner, CardPile::Deck).unwrap_or_default();
                            cx.world_mut().push_reveal(
                                owner,
                                reason.clone(),
                                game_core::state::RevealPile::Deck,
                                ids,
                            );
                        }
                        let id = cx.log_event("reveal", owner, reason, parent, &card, 1);
                        cx.set_event_to(id, CardPile::Inspection as i32);
                    }
                    self.raise_core(cx, "inspected", owner, |t| {
                        t.target = owner;
                        t.card = Some(card.clone());
                        t.cards = vec![card.clone()];
                        t.value = position;
                        t.by_card = Some(looker);
                    })?;
                }
                let held = {
                    let w = cx.world_mut();
                    match w.inspection_frames.iter_mut().find(|f| f.id == frame_id) {
                        Some(f) => {
                            f.finished = true;
                            f.cards.len() as i32
                        }
                        None => 0,
                    }
                };
                // One `inspection` line per look: the banner names who looked
                // and how many cards turned. The frame itself rides the view.
                let id = cx.log_event(
                    "inspection",
                    owner,
                    Msg::new("log.inspection_open").n("count", held as i64),
                    parent,
                    "",
                    held,
                );
                cx.set_event_to(id, CardPile::Inspection as i32);
                return Ok(held);
            }
            // Release the caller's innermost frame; a body that forgot to
            // answers 0 and leaks it to the commit net (design G11 §5.4).
            //
            // The three arms below are the **pause fallback**: today's
            // `hostfns` apply these through the live `Run` and never build a
            // request (G11 §5.5), so they run only if a caller ever takes the
            // pausing route. Keep them in step with `Run::inspection_end` /
            // `Run::reveal_pile` / `Run::reveal_card`.
            HostRequest::InspectionEnd { player_id, shuffle } => {
                let released = cx.world_mut().finish_inspection_of(player_id, shuffle);
                if released > 0 {
                    // 「观看的牌放回卡组」 -- the leftovers, named by count.
                    cx.log(
                        player_id,
                        Msg::new("log.inspection_returned").n("count", released as i64),
                    );
                }
                return Ok(released);
            }
            // Publish a pile to the table. The reveal is a view overlay, not a
            // pile write: it touches no card (design G11 §5.6).
            HostRequest::RevealPile {
                player_id,
                pile,
                reason,
            } => {
                let (view, ids) = match CardPile::from_i32(pile) {
                    Some(CardPile::Hand) => (
                        game_core::state::RevealPile::Hand,
                        hidden_pile(cx.world(), player_id, CardPile::Hand).unwrap_or_default(),
                    ),
                    Some(CardPile::Graveyard) => (
                        game_core::state::RevealPile::Discard,
                        hidden_pile(cx.world(), player_id, CardPile::Graveyard).unwrap_or_default(),
                    ),
                    Some(CardPile::Deck) => {
                        let mut ids =
                            hidden_pile(cx.world(), player_id, CardPile::Deck).unwrap_or_default();
                        // The open frames are the true top of the deck.
                        ids.splice(0..0, inspection_held(cx.world(), player_id));
                        (game_core::state::RevealPile::Deck, ids)
                    }
                    _ => return Ok(0),
                };
                let count = ids.len() as i32;
                let parent = cx.activation.last().copied().unwrap_or(-1);
                cx.world_mut()
                    .push_reveal(player_id, reason.clone(), view, ids);
                let id = cx.log_event("reveal", player_id, reason, parent, "", count);
                cx.set_event_to(id, pile);
                return Ok(count);
            }
            // Publish one card id; appended to the seat's revealed cards.
            // Same no-op rule as `Run::reveal_card`: an already published id
            // (a re-run, or a public look's own card) adds nothing.
            HostRequest::RevealCard {
                player_id,
                card,
                reason,
            } => {
                let seen = cx
                    .world()
                    .reveals
                    .iter()
                    .find(|r| r.player == player_id)
                    .is_some_and(|r| r.deck.iter().any(|c| *c == card))
                    || cx.world().inspection_frames.iter().rev().any(|f| {
                        f.deck_owner == player_id
                            && f.public
                            && f.cards.iter().any(|c| c.revealed && c.id == card)
                    });
                if seen {
                    return Ok(1);
                }
                let parent = cx.activation.last().copied().unwrap_or(-1);
                cx.world_mut().push_reveal(
                    player_id,
                    reason.clone(),
                    game_core::state::RevealPile::Card,
                    vec![card.clone()],
                );
                let id = cx.log_event("reveal", player_id, reason, parent, &card, 1);
                cx.set_event_to(id, CardPile::Inspection as i32);
                return Ok(1);
            }
            HostRequest::Pay {
                from,
                to,
                amount: asked,
                src,
                runs_total_stage,
            } => {
                // One pipeline: `PlayCtx::money` runs the staged adjustment
                // points (pre-split `effect` counteraction window -> payCombinedAdd /
                // payCombinedMul / payCombinedCancel -> paySplitAdd / paySplitMul /
                // paySplitChoose / paySplitAt -> split -> post-split `pay` /
                // `payAfter` / `paid`) whatever the cause -- print (game ->
                // player), delete (player -> game) or pay-player. The
                // counteraction window opens here the same way it does for rent, so
                // any card-caused payment is answerable. The money move
                // happens inside, so the guest's `pay`/`gain` resume is
                // answered with the amount and does not move it again.
                //
                // Q1 / B1 (`PIPELINE-AUDIT` P11a): a card's forced
                // 「pay/spend」 runs the same raise-funds-then-bankrupt
                // path rent does (规则书 L16 「当玩家无法支付某笔支出时（包括
                // 抵押）」 → 折现 + 破产, L76 the mortgage offer). The card's
                // `why` is a **reason** (「登上武道馆」), not a log line: it
                // rides the standard `log.pay` / `log.lose` / `log.gain` line
                // as the parenthetical `{{src}}`.
                return self.pay_through(
                    cx,
                    &call,
                    from,
                    to,
                    asked,
                    game_core::engine::PayKind::Card,
                    game_core::engine::PayShape::Normal,
                    true,
                    true,
                    runs_total_stage,
                    None,
                    src,
                    false,
                );
            }
            // v107: the context-carrying command. `spec` bits 0-3 shape, 4-7
            // kind, 8-11 flags, 12-31 `tile + 1`; the host validates on decode
            // by falling back (`Normal` / `Card`, unknown flag bits ignored).
            HostRequest::PayCommand {
                from,
                to,
                amount,
                spec,
                src,
            } => {
                let shape = game_core::engine::PayShape::from_i32(spec & 0xF);
                let kind = game_core::engine::PayKind::from_i32((spec >> 4) & 0xF);
                let flags = (spec >> 8) & 0xF;
                let raw_tile = (spec >> 12) & 0xF_FFFF;
                let tile = (raw_tile > 0).then(|| (raw_tile - 1) as usize);
                let must = flags & card_sdk::abi::pay_flag::MUST != 0;
                let allow_mortgage = flags & card_sdk::abi::pay_flag::ALLOW_MORTGAGE != 0;
                let total_stage = flags & card_sdk::abi::pay_flag::TOTAL_STAGE != 0;
                return self.pay_through(
                    cx,
                    &call,
                    from,
                    to,
                    amount,
                    kind,
                    shape,
                    must,
                    allow_mortgage,
                    total_stage,
                    tile,
                    src,
                    true,
                );
            }
            HostRequest::PayTotal {
                from,
                to,
                amount: asked,
                src: _,
            } => {
                // `PIPELINE-AUDIT` Q2: the command-wide pre-split stage on
                // its own. No money moves; the answer is the shaped total
                // (or -1 when a `payCombinedCancel` hook dropped the command).
                let from = (from >= 0).then_some(from as usize);
                let to = (to >= 0).then_some(to as usize);
                let shaped = cx.pay_total(from, to, asked, Some(call.player_id()))?;
                return Ok(shaped.unwrap_or(-1));
            }
        }
    }

    /// Ruling 1: the card **flash** is shown only when the activation left the
    /// owner. Owner-self-only effects (own charge, counters, latches, hand-limit,
    /// own bank money, own draw/discard) keep the log entry but no animation.
    ///
    /// Scope classification (see the report table):
    /// * **Other** -- anything touching another player (money to/from them,
    ///   targeting, designation, their status / hand / state).
    /// * **Board** -- cards on board (place / unplace), tile marks, houses,
    ///   deeds, mortgage, ownership, tile props.
    /// * **Move** -- position change, teleport, stop-at, extra steps, plan
    ///   rewrites of a move.
    /// * **SelfOnly** -- own charge, own counters / latches / skill state, own
    ///   hand-limit / start-hand, own prop / slot writes, own bank money,
    ///   own draw / discard.
    ///
    /// Returns `true` when the activation should flash (some effect is
    /// Other / Board / Move).
    fn drive_should_flash(&self, cx: &PlayCtx, after: &Run, owner: i32, header: i32) -> bool {
        // Money: any leg that is not (owner <-> bank) is Other.
        for &(from, to, _amount) in &after.paid_log {
            let touches_other = (from >= 0 && from != owner) || (to >= 0 && to != owner);
            if touches_other {
                return true;
            }
        }
        // Cards / marks / houses on board.
        if !after.house_log.is_empty() {
            return true;
        }
        // Draw / discard: own = SelfOnly (proposed); another player's = Other.
        for (who, _id) in &after.draw_log {
            if *who != owner {
                return true;
            }
        }
        for (who, _id) in &after.discard_log {
            if *who != owner {
                return true;
            }
        }
        // A look is a card move off another seat's deck.
        for (who, batch) in &after.draw_after_log {
            if *who != owner && !batch.is_empty() {
                return true;
            }
        }
        // Charge spend on self = SelfOnly. Counter writes on self = SelfOnly.
        // (counter_log rows are (owner, card, uid, name, delta) -- the
        // instance's owner; if that is not the drive owner, it touched another
        // player.)
        for (who, _card, _uid, _name, _delta) in &after.counter_log {
            if *who != owner {
                return true;
            }
        }
        // Dest change (place / unplace / graveyard) = Board.
        if after.dest >= 0 {
            return true;
        }
        // Events this drive produced: the header (if any) and everything
        // after it on the stream (body lines + host effects). Use the id
        // tail rather than `parent` -- host-effect events are stamped to the
        // header after the fact, and a nested activation's own events sit in
        // the same window.
        let _ = after.parent;
        for e in cx.world().recent.iter() {
            if header >= 0 && e.id < header {
                continue;
            }
            match e.r#type.as_str() {
                "move" | "diceRollBefore" | "teleport" => return true,
                "build" | "mortgage" | "redeem" | "buy" | "overlap" => return true,
                "pay" | "rent" | "gain" | "lose" => {
                    // Money legs: the two sides ride the Msg args (`who` /
                    // `to` as PlayerId); bank is -1. Own bank money is
                    // SelfOnly (proposed); anything touching another seat
                    // is Other.
                    let sides = ["who", "to", "from"];
                    for k in sides {
                        if let Some(game_core::msg::Arg::PlayerId(p)) = e.msg.args.get(k) {
                            if *p >= 0 && *p != owner {
                                return true;
                            }
                        }
                    }
                    // Fallback: the event's own from/to fields (older paths).
                    if (e.from >= 0 && e.from != owner) || (e.to >= 0 && e.to != owner) {
                        return true;
                    }
                }
                "status" => {
                    if e.player_id != owner && e.player_id >= 0 {
                        return true;
                    }
                    if e.other >= 0 && e.other != owner {
                        return true;
                    }
                }
                "draw" | "discard" | "mulligan" | "inspection" | "reveal" => {
                    if e.player_id != owner && e.player_id >= 0 {
                        return true;
                    }
                }
                _ => {}
            }
            // Outcome lines that name a plan rewrite or a board write.
            if let Some(k) = e.msg.key().strip_prefix("log.outcome.") {
                match k {
                    "plan_steps" | "plan_stop" | "plan_teleport" | "plan_start" | "plan_dice"
                    | "plan_no_buy" | "plan_no_build" => return true,
                    "place" | "unplace" | "mark_add" | "mark_remove" | "mark_move" => return true,
                    _ => {
                        // Player-directed outcomes carry a `target`. Targeted at
                        // someone other than the owner counts as Other.
                        if let Some(game_core::msg::Arg::PlayerId(t)) = e.msg.args.get("target") {
                            if *t != owner {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Debug guard: a **silent** entry (`On::silent`) is bookkeeping --
    /// declarations, resets, latch clears the player never sees as a clause
    /// ficycle. It must not produce a visible effect (money, cards, marks,
    /// houses, charge spend, narration lines); the flag exists to hide the
    /// flash, not the effect. When one does, the flag is lying: record the
    /// violation (tests read [`take_silent_visible`]) and `debug_assert!` for
    /// shipped card ids so debug CI trips on a mis-marked entry. `TEST:*`
    /// fixtures are exempt from the assert so a deliberate-violation fixture
    /// can exercise the recorder without aborting the test binary.
    fn check_silent_body(&self, card_id: &str, after: &Run, borrowed_house: usize) {
        let what = if !after.paid_log.is_empty() {
            Some("pay/gain/transfer")
        } else if !after.draw_log.is_empty() {
            Some("draw")
        } else if !after.discard_log.is_empty() {
            Some("discard")
        } else if !after.reshuffle_log.is_empty() {
            Some("reshuffle")
        } else if after.house_log.len() > borrowed_house {
            Some("house")
        } else if !after.marker_spent_log.is_empty() {
            Some("marker-spend")
        } else if after.trigger.kind != TriggerKind::DeckAtGameStart
            && after
                .marker_changed_log
                .iter()
                .any(|(_, delta, _, _)| *delta != 0)
        {
            // Only a real count move is visible. A `set_held` re-stamp at
            // delta 0 (`fire::declare`'s per-turn cap restatement) raises the
            // event but is bookkeeping -- that is what the silent mark hides.
            // The game-start pot declaration establishes the initial count;
            // setup is not a clause event either.
            Some("marker-change")
        } else if after.logged > 0 {
            Some("log line")
        } else {
            None
        };
        let Some(what) = what else {
            return;
        };
        record_silent_visible(card_id, what);
        debug_assert!(
            card_id.starts_with("TEST:"),
            "silent entry on {card_id} produced a visible effect ({what}); \
             `On::silent` must not hide a real effect -- drop the `.silent()` \
             mark or make the body pure bookkeeping"
        );
    }

    /// The commit half of a drive: adopt the guest's finished [`Run`] as the
    /// live world and raise everything it logged. Shared by the replay path's
    /// `Done` arm and the simulation path's commit pass (`docs/BOT.md` §3.2).
    ///
    /// Write-through logs and host-effect events already sit on the live
    /// stream in code order; the guest copy carries only state writes, so this
    /// adopts the live tail before the swap. Then it completes the activation
    /// entry with its outcome (「无事发生」 when the body changed nothing).
    ///
    /// `silent` marks a bookkeeping body (`On::silent`): it posted no
    /// activation, and it must not have produced a visible effect either --
    /// see [`Self::check_silent_body`].
    fn commit_after(
        &self,
        cx: &mut PlayCtx,
        after: Run,
        call: Call,
        card_id: &str,
        trigger: &mut Trigger,
        silent: bool,
        quiet_when_empty: bool,
        activation_id: i32,
        overlay_base: usize,
        pile_base: Option<&PileCheckpoint>,
    ) -> Flow<i32> {
        let mut after = after;
        if std::env::var("DBG_H01").is_ok() {
            eprintln!(
                "H01 commit card={card_id} copy_owner16={:?} live_owner16={:?}",
                after.world.st.owners.get(16),
                cx.world().st.owners.get(16)
            );
        }
        // Tile writes apply on the live context; their owed raises belong to
        // this commit.
        let owed = cx.take_pending_house_log();
        // Rows drained from the pending queue were written by a *different*
        // drive's request (the build that just committed, an earlier body)
        // and are only owed to this commit for the raise -- they are not this
        // body's own effect. The silent guard and the emptiness test look past
        // them; the body's own rows end up in front (`extend` appends).
        let borrowed_house = owed.len();
        let drive_id = after.drive_id;
        if silent {
            self.check_silent_body(card_id, &after, borrowed_house);
        }
        after.house_log.extend(owed);
        let dest = after.dest;
        *trigger = after.trigger.clone();
        // Keep the live event tail (write-through order) over the guest
        // copy's stale one. The guest copy is the state; the event stream
        // and its `next_event` counter live solely on the live world (the
        // guest runs with `skip_log` and posts write-through). Adopting
        // the live tail **and** the live counter is what keeps ids unique
        // across a drive -- the guest copy may have been detached at any
        // `DerefMut` (its own `skip_log` write included) and must never
        // allocate an id of its own. The pending walk also remains on the
        // live world -- swapping in a stale `WalkFlush` would re-publish
        // the approach from its origin after a mid-walk flash (the
        // walk-before-flash invariant).
        after.world.recent = cx.world().recent.clone();
        after.world.next_event = cx.world().next_event;
        // Event ids must be unique and strictly increasing across the
        // whole tail. A duplicate means two `World`s allocated from the
        // same `next_event` (the draw-with-reshuffle bug).
        {
            let mut prev = i32::MIN;
            for e in after.world.recent.iter() {
                debug_assert!(
                    e.id > prev,
                    "event ids must be strictly increasing: {} follows {}",
                    e.id,
                    prev
                );
                prev = e.id;
            }
        }
        after.world.walk_flush = cx.world().walk_flush.clone();
        // Complete the activation: 「无事发生」 when nothing followed.
        // `activation_id` is the header this drive posted (the local from
        // `drive_inner_body`). Fall back to `after.parent` when the local
        // was not set (a re-entry that adopted an existing header). A
        // silent drive created no header and must not complete one.
        // Complete **this** drive's header. A multi-entry drive that ran
        // one silent and one non-silent entry still has a header (from the
        // non-silent one) and must stamp it; a fully silent drive has no
        // header of its own (`activation_id` remains -1) and must not
        // complete the outer one.
        // Ruling 2a: the header this drive flushed (or -1 if nothing
        // visible was produced -- a declined optional prompt, a
        // bookkeeping write). Never complete the outer activation.
        let header = if after.own_activation >= 0 {
            after.own_activation
        } else {
            activation_id
        };
        if header >= 0 {
            let children = cx.child_event_count(header, header);
            // QuietWhenEmpty + nothing visible + no prompt: retract the
            // header entirely (no flash, no 「效果适用」, no
            // 「（无事发生）」). A prompt already showed it to the client.
            // A **cue** counts as visible -- the player was told something
            // happened, so the header that names the clause remains. Both
            // spellings of "cue me" count: the imperative `ctx::flash()` and
            // the declarative [`card_sdk::abi::Flash::Always`] (`On::flash`),
            // which is the same promise written on the entry instead of in the
            // body. Dropping the header under `Always` would make the policy a
            // lie ("always flashes") the moment the body is a no-op.
            //
            // `flash_cued` is "did any entry cue the player" -- the OR of
            // `ctx::flash()` and any `Flash::Always` sibling -- **not** the
            // folded `after.flash_policy` (which `Never` can win). A drive
            // that was cued keeps its header naming the clause even when
            // `Never` wins the flash decision below ("log entry remains").
            let flash_requested = after.world.flash.lock().requested;
            let flash_cued = flash_requested || after.flash_cued;
            // Body emptiness must see every log that the flush below turns
            // into after-hooks the player can observe: a counter move raises
            // `counterChanged`, a named raise fires its hooks. Missing those
            // would retract the 「效果适用」 header and then fire the hooks on
            // an invisible clause. (`state_log` is likewise a hook raiser.)
            let body_empty = after.logged == 0
                && after.paid_log.is_empty()
                && after.draw_log.is_empty()
                && after.discard_log.is_empty()
                && after.reshuffle_log.is_empty()
                && after.house_log.len() == borrowed_house
                && after.state_log.is_empty()
                && after.marker_spent_log.is_empty()
                && after.marker_changed_log.is_empty()
                && after.counter_log.is_empty()
                && after.named_raise_log.is_empty();
            if quiet_when_empty && body_empty && !after.prompted && !flash_cued {
                cx.drop_events(&[header]);
                after.world.recent = cx.world().recent.clone();
                cx.activation.retain(|&id| id != header);
            } else {
                let results = if children == 0 {
                    vec![Msg::new("log.effect_nothing")]
                } else {
                    Vec::new()
                };
                cx.set_event_results(header, results);
                // Ruling 1 + v55 precedence:
                //   Never > ctx::no_flash > ctx::flash / Always > Auto.
                // `Auto` classifies by scope (owner-self-only => no flash).
                let owner = if call.player_id() >= 0 {
                    call.player_id()
                } else {
                    after.trigger.player_id
                };
                let no_flash = after.world.flash.lock().no_flash;
                let flash = if after.flash_policy == card_sdk::abi::Flash::Never {
                    false
                } else if no_flash {
                    false
                } else if flash_requested || after.flash_policy == card_sdk::abi::Flash::Always {
                    true
                } else {
                    self.drive_should_flash(cx, &after, owner, header)
                };
                cx.set_event_flash(header, flash);
                // One drive, one flash decision, every header it posted.
                // Sibling entries each flush their own 「效果适用」; `flush_header`
                // defaults the second-and-later ones' flash off (its `already`
                // branch), but that is a default, not the decision -- the fold
                // is. Stamp the folded `flash` on every id `header_ids` recorded
                // so a trailing `Auto` sibling's header still rides the leading
                // `Always` (and a `Never` suppresses all of them).
                for &hid in &after.header_ids {
                    cx.set_event_flash(hid, flash);
                }
                // Belt-and-braces: the `__hdr__` once-per-flash identity names
                // the first flush. If some id somehow missed the vec, still
                // stamp that one so `Never` cannot leave a stray flash.
                {
                    let hkey = format!("{card_id}|__hdr__");
                    let first = after.world.flash.lock().seen.get(&hkey).copied();
                    if let Some(first) = first {
                        cx.set_event_flash(first, flash);
                    }
                }
                after.world.recent = cx.world().recent.clone();
                cx.activation.retain(|&id| id != header);
            }
        }
        after.world.skip_log = false;
        after.world.stub = Default::default();
        // later activation of the same card is a fresh drive. (A Halt that
        // never reaches here keeps them -- the flash already shown before
        // the prompt must not play again on the re-run.)
        {
            let mut f = after.world.flash.lock();
            let prefix = format!("{card_id}|");
            f.seen.retain(|k, _| !k.starts_with(&prefix));
            f.ordinal = 0;
            f.requested = false;
            f.no_flash = false;
        }
        let mark_rows = std::mem::take(&mut after.world.mark_log);
        // The rows are raised below; the live log must not keep a second copy
        // of them. It can: a nested drive's mark write lands on live, and the
        // next pass's copy inherits the row from there -- every pass would
        // raise it again, and the raise re-enters the card (`black_suits`'s
        // `resync` attached another circle per pass, unbounded).
        cx.world_mut().mark_log.clear();
        // The completing copy predates every nested application made during
        // this pass: a host effect lands on live only, so swapping the copy
        // in would roll those effects back. Keep the live world as the base
        // and cross what the guest itself wrote -- the same boundary the
        // `NeedHost` arm uses before applying a request (design G10 §6.3.3).
        //
        // The body's own writes were crossed onto live at its pauses and are
        // already in this pass's copy; take them off live first, or the diff
        // below is applied *on top of* itself (a board delta counts twice).
        cx.restore_guests_to(overlay_base, drive_id);
        let state_writes = cx.capture_guest_state_from(&after.pass_base.state, &after.world, false);
        let frame_writes = cx.capture_guest_frames_from(&after.world);
        let field_writes = cx.capture_guest_field_from(&after.pass_base, &after.world);
        let token_writes = cx.capture_guest_tokens_from(&after.pass_base, &after.world);
        let owner_writes = cx.capture_guest_snapshot_owners_from(&after.pass_base, &after.world);
        // The pass top rewound the live piles to the drive start (a replay
        // rebuilds them from there, `pile_base`), so the pile diff must be
        // measured against that same world: `after.pass_base.piles` is the
        // world at the last request site, which live no longer is. The guest
        // copy carries the host effects (each request reloads it), so its
        // lists are authoritative for the piles.
        let pile_writes = match pile_base {
            Some(base) => {
                cx.capture_guest_piles_from(&PlayCtx::pass_base_of(&base.world), &after.world)
            }
            None => cx.capture_guest_piles_from(&after.pass_base, &after.world),
        };
        let rule_bonus_writes = cx.capture_guest_rule_bonus_from(&after.world);
        let roll_pick_writes = cx.capture_guest_roll_picks_from(&after.pass_base, &after.world);
        let scheduled_writes = cx.capture_guest_scheduled_from(&after.pass_base, &after.world);
        let money_writes = cx.capture_guest_money_from(&after.pass_base, &after.world);
        let persona_writes = cx.capture_guest_persona_from(&after.pass_base, &after.world);
        let board_writes = cx.capture_guest_board_from(&after.pass_base, &after.world);
        let event_writes = cx.capture_guest_events_from(&after.pass_base, &after.world);
        let guest_overlay = game_core::engine::GuestOverlay {
            state: state_writes.state,
            field: field_writes,
            tokens: token_writes,
            owners: owner_writes,
            money: money_writes,
            persona: persona_writes,
            board: board_writes,
            piles: pile_writes,
            roll_picks: roll_pick_writes,
            scheduled: scheduled_writes,
            events: event_writes,
            rule_bonus: rule_bonus_writes,
            frames: frame_writes,
        };
        cx.adopt_turn_policy(&after.world);
        cx.overlay_guest_plan_tags(&after.world);
        let mut w = cx.world_copy();
        // `hidden` is indexed by seat: the guest copy's rows are the live
        // ones with the body's pile writes, so only rows past the live length
        // are additions. (Matching by value duplicated a row whose hand the
        // body touched -- a 2-seat table grew a third row and the pass base
        // indexed past its base.)
        if after.world.hidden.len() > w.hidden.len() {
            w.hidden
                .extend(after.world.hidden[w.hidden.len()..].iter().cloned());
        }
        // Plan writes (`ctx::plan::set_*`) are direct guest writes of the
        // staging command -- the roll window folds them against the window's
        // pre-state after the commit, so they must ride the copy. The whole
        // plan (staging buffer and its frames) is the guest's: the pass started
        // from this world's copy and the host's `run_planned_move` reads the
        // request's own plan.
        w.turn.plan = after.world.turn.plan.clone();
        w.turn.plan_stack = after.world.turn.plan_stack.clone();
        // `ctx::cancel_designation` (net_error clause 1) writes the turn's
        // cancelled list, and the request that raised the window reads it back
        // off **live** right after the counter body returns. Nothing replays
        // it there, so it rides the copy like the plan.
        w.turn.cancelled_designations = after.world.turn.cancelled_designations.clone();
        // `ctx::set_play_doubled` is written by a third party (the start
        // square's group skill) in its own nested drive, and the played card's
        // body reads it off **live** afterwards (`ctx::n`). Nothing replays
        // the write in the reader's run, so it rides the copy.
        w.turn.play_doubled = after.world.turn.play_doubled;
        // Top-level world bookkeeping the guest also writes directly:
        // `give_extra_turn` / `end_turn_now` latch onto the world (and the
        // turn end reads them back off live after the hook body), and the
        // turn-end settle answer is a plain per-seat write. The live base
        // holds the host's own writes, so only the guest's lists ride here.
        w.extra_turns = after.world.extra_turns.clone();
        w.end_turn_request = after.world.end_turn_request;
        w.blocked_settle = after.world.blocked_settle.clone();
        // `ctx::skip_main_move` (the `[无法移动]` latch) is a guest write; the
        // turn end reads it off live straight after the body.
        w.st.skip_move = after.world.st.skip_move;
        // `ctx::set_persona` (二人一体的 swap) is a guest write too, and only a
        // body makes one (the host never does): the identity carries from the
        // finished copy. A write made before a pause already crossed to live,
        // but the copy's face is the drive's last word either way -- the commit
        // takes it whole rather than diffing.
        for (to, from) in w.st.players.iter_mut().zip(after.world.st.players.iter()) {
            to.persona = from.persona.clone();
        }
        // A hand play's waste bit is `ctx::set_ineffective_at(-1, ..)`, a turn
        // write with no row to ride a field diff. The play's own discard reads
        // it off live after the body returns (「视为此卡未生效」).
        w.turn.play_ineffective = after.world.turn.play_ineffective;
        // `world_snapshot` / `world_restore` are guest calls; the checkpoint
        // table and its key allocator live on the guest copy. The table is
        // taken whole (a drop is not a per-row write) and the overlay's owner
        // additions land on top of it.
        // The randomness stream: the pass top rewinds live to the pile base
        // so a replayed draw redraws the same cards, and the pass itself only
        // ever peeks/consumes on the throw-away side. The finished copy
        // carries the whole pass's consumption (it inherited live at every
        // request and re-applied them all on the last pass), so it replaces
        // live's stream -- the old whole-world commit did the same (G03).
        w.rng = after.world.rng.clone();
        w.world_snapshots = after.world.world_snapshots.clone();
        w.world_snapshot_owners = after.world.world_snapshot_owners.clone();
        w.next_world_snapshot_key = after.world.next_world_snapshot_key;
        // The inspection frames are guest-writable too: `ctx::take_card` /
        // `ctx::add_to_dest` on `CardPile::Inspection` write the frame list
        // directly (only begin/end are host requests). The pass's list is the
        // live one with the body's takes, so it must replace live's -- else the
        // commit net below releases cards the body already took out and puts
        // them back on the deck.
        w.inspection_frames = after.world.inspection_frames.clone();
        // A look that ended this pass left its deck held (`look_hold`); the
        // play's own filing is a later pass's refill check and must still see
        // the hold (design G11 I2).
        w.look_hold = after.world.look_hold.clone();
        guest_overlay.apply(&mut w);
        // Field placements, removals, counters and in-place rewrites are
        // direct guest writes too (`ctx::place_card_on`, `set_tile_at`,
        // `set_prop`, `set_counter`); they ride the copy and cross through
        // `field_writes` above (design G10 §6.3.3).
        w.skip_log = false;
        w.stub = Default::default();
        cx.swap_shared(game_core::engine::SharedWorld::from(w));
        // The pass's guest writes are in the world now: drop the overlays this
        // drive crossed, so an enclosing pass top's revert does not undo them
        // (a later pass consumes their answers without re-applying, so a
        // reverted effect would not come back).
        cx.keep_guest_overlays_to(overlay_base);
        if std::env::var("DBG_H01").is_ok() {
            eprintln!(
                "H01 commit tail card={card_id} live_owner16={:?} overlays={}",
                cx.world().st.owners.get(16),
                cx.guest_overlay_depth()
            );
        }
        // The pass is final: pop every guest overlay this drive pushed before
        // the swap. Restoring reverts the live state/rows -- the swap then
        // installs the pass's world on top, so the revert costs nothing --
        // and, more importantly, a nested overlay left on the stack would let
        // the parent drive's re-run undo the committed rows.
        // A row an enclosing overlay recorded as taken out ("gone": a `prev`
        // with no `wrote`) is only back in the live world because the restore
        // put it there for the reads before the replayed request. That re-run
        // took the row out of its **own** world too when it re-applied the
        // leave; if this pass's world still carries it, the take came from a
        // nested effect the enclosing routine never re-issues, and the
        // committed world must not keep a dead card.
        let gone = std::mem::take(&mut after.reinserted);
        cx.restore_guests_to(overlay_base, drive_id);
        if !gone.is_empty() {
            // The committed world is the live one with the guest diff crossed
            // on (swapped in above), so the dead rows are filtered there.
            let world = cx.world_mut();
            for p in world.st.players.iter_mut() {
                p.field.retain(|f| !gone.contains(&f.uid));
            }
            world.st.board_field.retain(|f| !gone.contains(&f.uid));
        }
        // Commit net (design G11 §5.4): release every frame this drive opened
        // and did not close -- a body that returned without
        // `inspection_end`, a frame opened inside a cancelled hook. Frames in
        // the pass's base belong to an enclosing drive and stay, unless this
        // drive adopted them. Top-down so the newest look goes back first;
        // leftovers keep their order (a reference reveal does not shuffle).
        {
            let mut index = cx.world().inspection_frames.len();
            while index > 0 {
                index -= 1;
                let frame = &cx.world().inspection_frames[index];
                if after.frames_at_drive_start.contains(&frame.id)
                    && !after.adopted.contains(&frame.id)
                {
                    continue;
                }
                let looker = frame.looker;
                let count = cx.world_mut().release_frame(index, false);
                if count > 0 {
                    cx.log(
                        looker,
                        Msg::new("log.inspection_returned").n("count", count as i64),
                    );
                }
            }
        }
        // Empty-deck maintenance precedes all settlement after-hooks.
        for player_id in after.reshuffle_log {
            self.raise_core(cx, "reshuffled", player_id, |_| {})?;
        }
        // What the effect did, raised now that it has committed --
        // the same points `money()` / `discard()` raise (PayAfter +
        // `paid`, and `discarded`).
        let by_player = Some(call.player_id());
        for (from, to, amount) in after.paid_log {
            // 资金变动 (rulebook 支付阶段 7) runs on **any** money
            // change, merged into `payAfter` per the doc's
            // 「合并到[支付后]?」 -- a print (game -> player) included.
            // The `paid` counteraction window opens only when money left a
            // player, matching 再次牵起手来 / 游击演出.
            let side = if from >= 0 { from } else { to };
            self.raise_core(cx, "payAfter", side, |t| {
                t.player_id = from;
                t.target = to;
                t.value = amount;
                t.by_card = by_player;
            })?;
            if from >= 0 {
                self.raise_core(cx, "paid", from, |t| {
                    t.target = to;
                    t.value = amount;
                    t.by_card = by_player;
                })?;
            }
        }
        for (player_id, id) in after.discard_log {
            // A name-only addition carries no instance and, having never taken
            // effect, is never `wasted`.
            let zone = game_kinds::ZoneChange {
                from: game_kinds::Zone::Nowhere,
                to: game_kinds::Zone::Graveyard,
                cause: game_kinds::ZoneChangeCause::Effect,
                holder: player_id,
                wasted: false,
                counted: false,
            };
            self.raise_core(cx, "discarded", player_id, |t| {
                t.card = Some(id);
                t.zone = Some(zone);
            })?;
        }
        // 「…加入手牌时」 -- one raise per uncounted hand arrival (`counted`
        // arrivals are the draw routine's, which raises `drawn` instead).
        for (player_id, id, counted) in after.hand_log {
            if counted {
                continue;
            }
            let zone = game_kinds::ZoneChange {
                from: game_kinds::Zone::Nowhere,
                to: game_kinds::Zone::Hand,
                cause: game_kinds::ZoneChangeCause::Effect,
                holder: player_id,
                wasted: false,
                counted: false,
            };
            self.raise_core(cx, "handChanged", player_id, |t| {
                t.card = Some(id);
                t.zone = Some(zone);
            })?;
        }
        // The per-draw after points (`drawn` = the drawn card's own
        // hook, `drew` = the field-card per-draw point), one raise
        // per single card.
        for (player_id, id) in after.draw_log {
            self.raise_core(cx, "drawn", player_id, |t| {
                t.card = Some(id.clone());
                t.value = 1;
            })?;
            self.raise_core(cx, "drew", player_id, |t| {
                t.card = Some(id.clone());
                t.value = 1;
                t.cards = vec![id];
            })?;
        }
        // The draw-instruction point (design G11 §5.7): once per call, with
        // the whole batch, after the last per-card `drew` of the drive.
        for (player_id, batch) in after.draw_after_log {
            let count = batch.len() as i32;
            self.raise_core(cx, "drawAfter", player_id, move |t| {
                t.value = count;
                t.cards = batch;
            })?;
        }
        // kind 73 (`fireSpent`, wire-stable; `TODO(ABI) ABI-MARKER-SPENT`
        // rename to `MarkerSpent` when card bodies can follow). Name is payload.
        for (player_id, n, name) in after.marker_spent_log {
            self.raise_core(cx, "markerSpent", player_id, |t| {
                t.value = n;
                t.name = Some(name.clone());
            })?;
        }
        // `markerChanged` (HAG-01) -- one raise per bound held write, after
        // it committed. `player_id` is the token **holder**, `value` the
        // signed delta, `name` the counter name, `instance` the creating
        // instance. A standing 「has N」 clause hangs here instead of at each
        // write site. A handler's own held write commits in a nested drive
        // whose `commit_after` sees the depth -- it re-raises nothing
        // (`PlayCtx::marker_raise_depth`), so an unconditional writer cannot
        // chain forever. The turn-end backstop in hagumi covers writes made
        // under the guard.
        for (holder, delta, name, instance) in after.marker_changed_log {
            if std::env::var("DBG_MC").is_ok() {
                eprintln!(
                    "MC raise holder={holder} delta={delta} name={name} declares={} depth={}",
                    self.ruleset.declares(TriggerKind::MarkerChanged),
                    cx.marker_raise_depth
                );
            }
            if cx.marker_raise_depth > 0 {
                continue;
            }
            cx.marker_raise_depth += 1;
            let raised = self.raise_core(cx, "markerChanged", holder, |t| {
                t.name = Some(name.clone());
                t.value = delta;
                t.instance = instance;
                t.by_card = by_player;
            });
            cx.marker_raise_depth = cx.marker_raise_depth.saturating_sub(1);
            raised?;
        }
        for (player_id, tile, now, delta) in after.house_log {
            let kind = if delta > 0 {
                "houseAdded"
            } else {
                "houseRemoved"
            };
            self.raise_core(cx, kind, player_id, |t| {
                t.tile = tile;
                t.value = now;
            })?;
        }
        // `counterChanged` (wire event name) -- 「when this card no longer
        // has its named counter」 (AG:绯红之魂
        // (3) and kin) live here rather than at each spend site, so a
        // count emptied by *any* path still leaves the field.
        for (owner, card, uid, name, change) in after.counter_log {
            self.raise_core(cx, "counterChanged", owner, |t| {
                t.card = Some(card);
                t.name = Some(name);
                t.value = change;
                t.by_card = by_player;
                t.instance = uid;
            })?;
        }
        // `markChanged` -- one raise per mutated mark row, naming the mark's
        // creating instance, its tile and owner, and the effective signed
        // delta. A row whose creator is already gone still raises (`t.card`
        // carries its provenance for dispatch).
        for row in mark_rows {
            self.raise_core(cx, "markChanged", row.owner, |t| {
                t.tile = row.tile;
                t.name = Some(row.kind);
                t.value = row.delta;
                t.card = row.card;
                t.by_card = by_player;
                t.instance = row.instance;
            })?;
        }
        // The mark-emptying card 「its on-card [CP点] is empty → graveyard」
        // rule (user ruling 2026-10-07) does **not** raise here: `CpChanged`
        // is retired. It lives on `Action::CounterChanged` with a
        // `counter::CP` name check in the residual guard (`clear_cp.rs`) --
        // one event handler on the count, not a check at each spend site.
        // A count emptied by *any* write still leaves the field.

        // Post-commit named raise -- the wire event name is caller-supplied
        // data (the ruleset owns the vocabulary). 「任意玩家获得[除外]…时」
        // (火种燃尽之后会怎么样呢？ 「移除所有此卡的复制品」) listens on
        // that name rather than at each grant site, so a layer granted by
        // *any* path still runs.
        for (player_id, name, value) in after.named_raise_log {
            self.raise_core(cx, name, player_id, |t| {
                t.value = value;
                t.by_card = by_player;
            })?;
        }
        // TODO(规则书): a layer write is a player hit -- Other (it lands on a
        // player). Raise here once the off-board layer concept lands
        // (「任意玩家获得[除外]…时」); nothing pushes a layer log yet.
        // `state` -- 「当有其他玩家切换状态时」 (Mujica:欢迎来到ave
        // mujica的世界 (2)). Raised once per real skill-state 1↔2 toggle
        // this run made, same post-commit shape as a named raise. The counter's
        // own re-toggles must not chain further (`PlayCtx::state_raise_depth`).
        for (who, _from, to) in after.state_log {
            if cx.state_raise_depth > 0 {
                continue;
            }
            cx.state_raise_depth += 1;
            let raised = self.raise_core(cx, "state", who, |t| {
                t.value = to;
                t.by_card = by_player;
                t.effects.push(game_core::engine::rules::Effect {
                    kind: "state".into(),
                    target: who,
                    from: by_player.unwrap_or(-1),
                    tile: -1,
                    value: to,
                });
            });
            cx.state_raise_depth = cx.state_raise_depth.saturating_sub(1);
            raised?;
        }
        // A run that has an instance owns that instance's dest:
        // 「将此卡放入[使用者]弃卡区」 is `set_dest(Graveyard)` whether
        // the card was just placed or has been in play all along.
        // Reporting `DEST_FIELD` back tells the engine there is
        // nothing left for it to move -- the hand card became this
        // instance, and the host just moved that.
        //
        // `DEST_UNSET` is no opinion (a placed card remains put), and a
        // run that already unplaced itself has no instance to move.
        if after.current_uid >= 0 && dest >= 0 && dest != DEST_FIELD {
            self.apply_dest(
                cx,
                after.current_uid,
                card_id,
                dest,
                after.dest_to,
                after.dest_cause,
            )?;
            // The world is live now (the swap above), but an enclosing
            // drive's row overlay was re-anchored against the pre-leave row.
            // Re-anchor again: the row it remembers is gone, and its own
            // restore would otherwise file it back over this commit.
            cx.rebase_guest_overlays(overlay_base);
            return Ok(DEST_FIELD);
        }
        return Ok(dest);
    }

    /// The match-start points (`deckBeforeGame` / `deckAtGameStart`): dispatch
    /// to **every effect source** of `player_id` -- field cards including
    /// skills (per player, in field order) and the card ids in that player's
    /// piles/hands -- each source running its *own* hook with `t.card` naming
    /// it. Up to 5 passes, so sources a hook adds (a card that places itself,
    /// a pile a hook swells) get their turn too.
    ///
    /// `DeckBeforeGame` reaches the draw pile (before the opening deal);
    /// `DeckAtGameStart` reaches the draw pile and the hand (after the mulligan).
    fn counteract_orig(&self, cx: &mut PlayCtx, t: &mut CoreTrigger) -> Flow<()> {
        // Nothing in the set declares an entry at this kind, so no hook, no
        // gate and no counteraction can charge: skip building the bridge trigger at all.
        // This is most raises even with cards in play, and every raise for a
        // kind nobody listens to. Nothing could have rewritten `t`, so the
        // write-back below is a no-op and is safe to skip with it.
        if t.kind.as_str() == "eventResolved" {
            eprintln!(
                "DECL kind={} mapped={:?} declares={}",
                t.kind,
                trigger_kind(t.kind.as_str()),
                self.ruleset.declares(trigger_kind(t.kind.as_str()))
            );
        }
        if !self.ruleset.declares(trigger_kind(t.kind.as_str())) {
            return Ok(());
        }
        // The module's view of the trigger.
        let mut trigger = bridge_trigger(t);
        // Ordering at one trigger (the standard; see `hand_counteractions`):
        // (1) the acting card's own follow-up resolves FIRST, outside the
        //     answer tree -- a card answering its own play is not competing with
        //     the counteractions to it, so it never loses its place to them;
        // (2) then the hand-counteraction counteraction round on the timing, and
        //     counters to counters, settle LIFO -- newest first -- each before
        //     the link it answers;
        // (3) only then the field/tile/event/lingering hooks, and only when
        //     the trigger was not cancelled by (2).
        //
        // `NEGATION-AUDIT` V1: the counteraction round runs **before** hook dispatch
        // (规则书 L32 「counteraction…结算优先于X」 -- the counteraction settles before
        // X, including X's triggered [持续] settlement). A spend inside a hook
        // body must not settle before any seat can counteract the trigger that
        // carries it, and a cancelled trigger runs no hook body at all.
        //
        // (1) The played card's own follow-up: the card named on the trigger
        // runs its `counteract` entry (a card answering its own play). Only at
        // the play itself -- `cardAfter` / `cardPlayed` / `eventAfter` /
        // `drawn` also name a card on `t.card`, and must not re-run it here.
        let own = t.card.clone().unwrap_or_default();
        let own_play = matches!(
            trigger_kind(t.kind.as_str()),
            TriggerKind::Card | TriggerKind::Event
        );
        // A **play** of a `copies_previous` card (mortis) names the card it
        // copies on the link; the body plays it with `ctx::play_card` and
        // `trigger::link_copied` names it (design G06 §2.15). A counter link
        // stamps its own source at build.
        if own_play && !own.is_empty() {
            if let Some(i) = self.ruleset.card(&own) {
                if self.ruleset.copies_previous(i) {
                    trigger.copied = Self::copy_source(cx, t.player_id as usize);
                }
            }
        }
        if own_play && !own.is_empty() {
            // The entry is selected by kind alone -- a play answers no link
            // of its own, so the offer contract (which reads the answered
            // link's subject) must not be applied here: the card's own play
            // is not its own counter subject (`tsugumi`'s `NumberInterval`
            // contract would refuse it). The **condition** still gates the
            // own run: `Mujica:无法将视线移开`'s counter clause is
            // `target == owner && actor != owner`, true only for a counter
            // against the owner, so its counter body has nothing to do on its
            // own play (design G06 §4.9).
            let own_entry = self.ruleset.card(&own).and_then(|i| {
                let entry = self.ruleset.cards()[i as usize]
                    .entry(card_sdk::abi::OnKind::Counteract, Some(trigger.kind))?;
                let condition = self.ruleset.condition(i, entry);
                if condition.is_none() {
                    return Some((i, entry));
                }
                let run = self.probe_run(cx, &trigger, &own);
                let scope =
                    crate::condition::window_scope_owned(crate::condition::fill_window(&run));
                let cand = crate::condition::fill_candidate(&run, t.player_id, &own, false);
                crate::condition::condition_allows(condition, Some(&scope), &cand)
                    .then_some((i, entry))
            });
            if let Some((idx, entry)) = own_entry {
                // The entry is carried on the call: a play answers no link of
                // its own, so the offer's answer parsing (tsugumi's numeric
                // range, vocal's whole payment) must not be applied again here.
                self.drive(
                    cx,
                    Call::Counteract {
                        card: idx,
                        player_id: t.player_id,
                        arg: Vec::new(),
                        entry: Some(entry),
                    },
                    &own,
                    -1,
                    &mut trigger,
                )?;
            }
        }
        // (2) The hand-counteraction window: one round per timing, from the
        // seat after the timing's player around the table; counters settle
        // newest-first before the timing they answer (see
        // `hand_counteractions`). Not at hook-only points.
        if !is_hook_only(t.kind.as_str()) {
            self.hand_counteractions(cx, t, &mut trigger, 0)?;
        }
        // (3) Field-card hooks: at a hook-point kind, every *placed* card
        // runs its `counteract` automatically, in placement order per player. No player
        // declaration -- this is the persistent-effect path, as against the
        // counteraction window below.
        // Any trigger kind can carry a hook; the manifest says which cards
        // declared one, so nothing else is instantiated.
        //
        // V1: a trigger the counteraction round already cancelled runs **no** hook
        // body -- the spend inside one is effect content of the card that
        // wrote it and must not settle past a negation of the trigger.
        let kind = trigger.kind;
        if !trigger.is_cancelled() {
            {
                if is_game_start_kind(kind) {
                    // Match-start points reach **every effect source** of the
                    // player the engine raised them for: field cards including
                    // skills (in field order) and the card ids in that player's
                    // piles/hands (as before). Each source runs its *own* hook,
                    // with `t.card` naming it -- so a card that has no game-start
                    // clause is never instantiated for one.
                    self.game_start_hooks(cx, t.player_id, kind, &mut trigger)?;
                } else {
                    // uids the walks below already reached, so the subject /
                    // resolving passes do not run one twice. The own-card
                    // dispatch runs its body at `-1`, which the frames below
                    // would otherwise repeat for the same card.
                    let mut drove: Vec<i32> = Vec::new();
                    // The subject's card: a body still resolving at `-1` for
                    // that same card (a hand card being played) is the
                    // instance the subject walk just ran.
                    let mut subject_card: Option<String> = None;
                    // Own-card dispatch for `Drawn` (the card is in a hand / pile),
                    // `Inspected` (the turned-up card, held in the frame --
                    // design G11 §5.5), `HandChanged` / `OpeningHandDealt` (a
                    // card named with no instance) and `Discarded` (the card that
                    // just entered the discard -- RAS:游击演出's [特] 「此卡进入弃卡区的
                    // 回合结束时」). A placed field card also hears about discards
                    // (MyGO group (3) 「每次你的卡在未生效的情况下进入弃牌堆时」), so
                    // `Discarded` falls through to the field-card walk as well.
                    if matches!(
                        kind,
                        TriggerKind::Drawn
                            | TriggerKind::Inspected
                            | TriggerKind::Discarded
                            | TriggerKind::HandChanged
                            | TriggerKind::OpeningHandDealt
                    ) {
                        if let Some(card_id) = t.card.as_deref().filter(|s| !s.is_empty()) {
                            if let Some(idx) = self
                                .ruleset
                                .card(card_id)
                                .filter(|&i| self.ruleset.cards()[i as usize].hooks(kind))
                            {
                                self.drive_hook(
                                    cx,
                                    Call::Hook {
                                        card: idx,
                                        kind,
                                        player_id: t.player_id,
                                    },
                                    card_id,
                                    -1,
                                    &mut trigger,
                                )?;
                                drove.push(-1);
                                // The own drive *is* the subject instance
                                // (the row has already left the field, so the
                                // subject pass below must not run it again).
                                subject_card = Some(card_id.to_string());
                            }
                        }
                    }
                    if !matches!(kind, TriggerKind::Drawn | TriggerKind::Inspected) {
                        let world = cx.world_copy();
                        // uids the field walk below already reached, so the
                        // subject / resolving passes do not run one twice.
                        let mut drove: Vec<i32> = Vec::new();
                        for player_id in 0..world.player_count() {
                            // B2 (`PIPELINE-AUDIT` K6/K14) -- 规则书 L81 「所有其正在生效
                            // 的卡，技能效果停止生效」: a bankrupt / left seat's field
                            // hooks never run. (`remove_from_game` clears `s.field`;
                            // this is the dispatch's own guard.) `beforeOut` is the
                            // exception: it runs on the way out, before the clear,
                            // and a card that cares about the leave (ONE OF US) is
                            // still on the leaving seat's field to hear it.
                            if world.out(player_id) && kind != TriggerKind::BeforeOut {
                                continue;
                            }
                            for (uid, raw) in world.field_instances(player_id as i32) {
                                // A hook cannot re-trigger on its own money movement
                                // (the termination argument for nested money).
                                if is_money_hook_kind(kind) && cx.reentrant_hooks.contains(&uid) {
                                    continue;
                                }
                                // Only the tile's own cards hear a tile event
                                // (they travel with the tile), and a suppressed
                                // tile hides its extras.
                                if !field_hears(
                                    &world,
                                    trigger.tile,
                                    trigger.position,
                                    uid,
                                    trigger.kind,
                                ) || extra_suppressed(
                                    &world,
                                    trigger.tile,
                                    trigger.mv.as_ref().map(|m| m.tags.as_slice()),
                                    uid,
                                ) {
                                    continue;
                                }
                                // A marked instance runs its effective card's surface.
                                let id = surface_card(&world, uid, &raw);
                                let Some(idx) = self.ruleset.card(&id) else {
                                    continue;
                                };
                                // 「场上所有背面朝上的卡无法产生效果」 -- a face-down
                                // card on the field is inert, so its hooks must not run.
                                if self.ruleset.cards()[idx as usize].hooks(kind)
                                    && !world.is_face_down_at(uid)
                                {
                                    self.drive_hook(
                                        cx,
                                        Call::Hook {
                                            card: idx,
                                            kind,
                                            player_id: player_id as i32,
                                        },
                                        &id,
                                        uid,
                                        &mut trigger,
                                    )?;
                                    drove.push(uid);
                                }
                            }
                        }
                        // The raise's subject instance, alive or already
                        // unplaced. A destruction removes the row (and clears
                        // the seat, whose walk above is skipped) before the
                        // commit, so the field walk cannot find the very
                        // instance whose own hook must resync or discard its
                        // borrowed row; its id comes from the raise's
                        // provenance (`t.card`, `t.instance`).
                        if t.instance >= 0 {
                            let row = world.field_by_uid(t.instance).map(|f| {
                                (
                                    f.effective_card.clone().unwrap_or_else(|| f.card.clone()),
                                    f.owner,
                                )
                            });
                            let id = row.as_ref().map(|r| r.0.clone()).or_else(|| t.card.clone());
                            // The body's seat is the row's holder, not the
                            // trigger's player: for a gate the trigger player
                            // is the *target*, and `actor == owner` would read
                            // the candidate as owned by whoever is targeted.
                            let seat = row.as_ref().map_or(t.player_id, |r| r.1);
                            if let Some(id) = id {
                                if let Some(idx) = self.ruleset.card(&id) {
                                    if self.ruleset.cards()[idx as usize].hooks(kind) {
                                        // Also when the field walk above
                                        // already ran it: the frames below
                                        // still need to know the subject.
                                        // The own-card drive has already run
                                        // this very instance for a
                                        // `discarded` raise (same card).
                                        let own_ran = subject_card.as_deref() == Some(id.as_str());
                                        subject_card = Some(id.clone());
                                        if !drove.contains(&t.instance) && !own_ran {
                                            self.drive_hook(
                                                cx,
                                                Call::Hook {
                                                    card: idx,
                                                    kind,
                                                    player_id: seat,
                                                },
                                                &id,
                                                t.instance,
                                                &mut trigger,
                                            )?;
                                            drove.push(t.instance);
                                        }
                                    }
                                }
                            }
                        }
                        // Board-owned rule instances (`docs/TILES.md`, `docs/EVENTS.md`)
                        // hear the hooks they declare, the same as a player's field
                        // cards do. They run **after** the player fields so a suppressing
                        // card can arm `prop::NO_REWARD` on the tile instance in its own
                        // hook before the start-square tile rule's Pass entry reads it.
                        //
                        // A tile-carrying trigger (`passTile`, `settle`, a rent `pay`)
                        // reaches only the instances governing *that* tile -- the cheap
                        // path, since `passTile` runs on every step of every walk and
                        // the board holds one instance per tile. **Event** instances
                        // (`tile = -1`, `event:*`) are not tile-governed: they hear
                        // every trigger their rule declares, which is how an active
                        // event listens to a pass / settle / roll anywhere on the board.
                        // A trigger with no tile reaches every board instance, but only
                        // when some `tile:*` or `event:*` rule actually declares the
                        // hook (otherwise the board list is never touched).
                        let mut board: Vec<(i32, String)> = if t.tile >= 0 {
                            world.tile_rule_instances(t.tile)
                        } else if self
                            .ruleset
                            .cards()
                            .iter()
                            .any(|c| c.id.starts_with("tile:") && c.hooks(kind))
                        {
                            world.field_instances(game_core::state::BOARD_OWNER)
                        } else {
                            Vec::new()
                        };
                        if self
                            .ruleset
                            .cards()
                            .iter()
                            .any(|c| c.id.starts_with("event:") && c.hooks(kind))
                        {
                            for ent in world.event_rule_instances() {
                                if !board.iter().any(|(u, _)| *u == ent.0) {
                                    board.push(ent);
                                }
                            }
                        }
                        // **Mark** owners (`mark:*`, the tile-mark owner = the [tile marks] tile-mark
                        // owner) are board-wide the same way: they govern no single
                        // tile, so they hear every trigger their rule declares
                        // wherever it points -- the tile-mark owner's settle clause is
                        // 「在拥有 mark 点的格子上 settle 时」, any tile.
                        if self
                            .ruleset
                            .cards()
                            .iter()
                            .any(|c| c.id.starts_with("mark:") && c.hooks(kind))
                        {
                            for ent in world.mark_rule_instances() {
                                if !board.iter().any(|(u, _)| *u == ent.0) {
                                    board.push(ent);
                                }
                            }
                        }
                        // **Status** hosts (`status:*`, `rules/status`) are board-wide
                        // the same way: one standing instance each on `BOARD_OWNER`,
                        // `tile = -1`. Collected last among the field hooks (after
                        // tile / event / mark) so a suppressing tile rule still gets
                        // its say first and sibling hooks see pre-tick state. The
                        // the off-board layer tick and the no-hand turn-flow fold / clear run
                        // here.
                        if self
                            .ruleset
                            .cards()
                            .iter()
                            .any(|c| c.id.starts_with("status:") && c.hooks(kind))
                        {
                            for ent in world.status_rule_instances() {
                                if !board.iter().any(|(u, _)| *u == ent.0) {
                                    board.push(ent);
                                }
                            }
                        }
                        for (uid, id) in board {
                            if is_money_hook_kind(kind) && cx.reentrant_hooks.contains(&uid) {
                                continue;
                            }
                            if extra_suppressed(
                                &world,
                                trigger.tile,
                                trigger.mv.as_ref().map(|m| m.tags.as_slice()),
                                uid,
                            ) {
                                continue;
                            }
                            if drove.contains(&uid) {
                                continue;
                            }
                            // A marked instance runs its effective card's surface.
                            let id = surface_card(&world, uid, &id);
                            let Some(idx) = self.ruleset.card(&id) else {
                                continue;
                            };
                            if self.ruleset.cards()[idx as usize].hooks(kind)
                                && !world.is_face_down_at(uid)
                            {
                                self.drive_hook(
                                    cx,
                                    Call::Hook {
                                        card: idx,
                                        kind,
                                        player_id: game_core::state::BOARD_OWNER,
                                    },
                                    &id,
                                    uid,
                                    &mut trigger,
                                )?;
                                drove.push(uid);
                            }
                        }
                        // **Lingering** instances (`TurnCtx.lingering`, `docs/PURCHASE.md`
                        // P5) run last: the hand-card home for 「本回合」 effects, which
                        // would otherwise have no instance to carry them. A lingering
                        // `BuyAdd` is how @Tsugu ycm's 「本回合购买格子时spend资金降低1500」
                        // reaches a buy. `uid = -1`: there is no field instance, so
                        // `is_placed` answers false and the money re-entrancy guard does
                        // not apply to it.
                        for l in world.turn.lingering.clone() {
                            let Some(idx) = self.ruleset.card(&l.card) else {
                                continue;
                            };
                            if !self.ruleset.cards()[idx as usize].hooks(kind) {
                                continue;
                            }
                            if subject_card.as_deref() == Some(l.card.as_str()) {
                                continue;
                            }
                            self.drive_hook(
                                cx,
                                Call::Hook {
                                    card: idx,
                                    kind,
                                    player_id: l.owner,
                                },
                                &l.card,
                                -1,
                                &mut trigger,
                            )?;
                        }
                        // Every card body still resolving hears the kind it
                        // declares -- the raise is happening inside that
                        // resolution. **Last**: a card that also sits on the
                        // field or the board has already been driven by the
                        // walks above, and a body that raises must not hear its
                        // own raise twice (a self-driving tile would pay its
                        // reward once per pass).
                        let resolving = resolving_snapshot();
                        for (card, uid, seat) in resolving {
                            if drove.contains(&uid) {
                                continue;
                            }
                            if uid < 0 && subject_card.as_deref() == Some(card.as_str()) {
                                continue;
                            }
                            let Some(idx) = self.ruleset.card(&card) else {
                                continue;
                            };
                            if !self.ruleset.cards()[idx as usize].hooks(kind) {
                                continue;
                            }
                            self.drive_hook(
                                cx,
                                Call::Hook {
                                    card: idx,
                                    kind,
                                    player_id: seat,
                                },
                                &card,
                                uid,
                                &mut trigger,
                            )?;
                            drove.push(uid);
                        }
                    }
                }
                // Scheduled turn-end callbacks (「你的下回合结束时」 and kin): the
                // ones due at this player's turn end run once and are dropped. They
                // are taken out of the world *before* running, so a callback that
                // schedules again lands in the next round.
                let phase = match kind {
                    TriggerKind::TurnEndBefore => Some(true),
                    TriggerKind::TurnEndAfter => Some(false),
                    _ => None,
                };
                if let (Some(early), true) = (phase, t.player_id >= 0) {
                    let ended = t.player_id as usize;
                    let mut w = cx.world_copy();
                    let mut due = Vec::new();
                    w.scheduled.retain_mut(|s| {
                        if s.target != ended || s.early != early {
                            return true;
                        }
                        if s.skip {
                            s.skip = false;
                            return true;
                        }
                        due.push((s.card.clone(), s.owner, s.uid, s.entry.clone()));
                        false
                    });
                    cx.swap_world(w);
                    for (id, owner, uid, entry) in due {
                        let resolved = self.ruleset.card(&id).and_then(|idx| {
                            let info = &self.ruleset.cards()[idx as usize];
                            let e = if entry.is_empty() {
                                // Legacy save: settles to the card's first
                                // Schedule entry (the old AtEnd behaviour).
                                info.entry(card_sdk::abi::OnKind::Schedule, None)
                            } else {
                                info.entry_by_id(&entry)
                            };
                            e.map(|e| (idx, e))
                        });
                        if let Some((idx, entry_index)) = resolved {
                            self.drive(
                                cx,
                                Call::AtEnd {
                                    card: idx,
                                    player_id: owner,
                                    entry: entry_index,
                                },
                                &id,
                                uid,
                                &mut trigger,
                            )?;
                        }
                    }
                }
            }
        } // end if !trigger.is_cancelled() -- V1 hook skip
          // Write back whatever a counteraction rewrote. `set_move_roll` lands
          // in `trigger.move_roll` (the shared `t.Move` the counteractions also
          // read); the pay amount is rewritten in `trigger.value`. The engine
          // reads the result back off `t.value` after `counteract` returns.
          // A fold stage's figure is the composition on `value`; the move-face
          // mirror only ever carries the **last** responder's raw write, so on a
          // fold the composed `value` wins. Every other stage keeps the mirror (a
          // dice face is written through `set_move_roll`).
        t.value = if trigger.fold.is_fold() {
            trigger.value
        } else {
            trigger
                .mv
                .as_ref()
                .and_then(|m| m.roll)
                .unwrap_or(trigger.value)
        };
        // The composed multiplier of a `Fold::Mul` stage (the settle stages
        // read it back).
        t.fold_milli = trigger.fold_milli;
        if trigger.negation != Default::default() {
            t.negation = trigger.negation;
        }
        for s in trigger.spared {
            t.spare(s);
        }
        t.target = trigger.target;
        t.by_card_instance = trigger.by_card_instance;
        t.redirected_from = trigger.redirected_from;
        t.settle_facts = trigger.settle_facts;
        // The subject / join block / pending answers / copy source are link
        // state, not body output: carry whatever the walk left on them.
        t.subject_card = trigger.subject_card.clone();
        t.join = trigger.join.as_ref().map(unbridge_join);
        t.answers_declared = trigger.answers_declared.clone();
        t.copied = trigger.copied.clone();
        t.roll_root = trigger.roll_root;
        t.pending_negation = trigger.pending_negation;
        t.extreme_armed = trigger.extreme_armed;
        t.answered_kinds = trigger
            .answered_kinds
            .iter()
            .map(|k| k.spelling().into_owned())
            .collect();
        // The pay kind / shape rewrite (`set_pay_kind` / `set_pay_shape`) rides
        // the payment trigger; the engine folds it back before the shape is read.
        t.pay = trigger.pay.map(unbridge_pay);
        // The `BuyAssign` deal rewrite (`set_deal_owner` / `set_deal_houses`) rides
        // the buy trigger; the engine reads it back off `t.buy` at the commit.
        t.buy = trigger.buy.map(unbridge_buy);
        // Drop the frames this walk opened. The trigger's uid (not `t.uid`) is
        // the key: a hand counteraction retargets `trigger` to its chain link,
        // and the hooks run under that link.
        truncate_resolving(trigger.uid);
        Ok(())
    }

    /// Snapshot the hook drives `counteract` phase 3 would run (STACK-01).
    /// One world copy; stable job list so a suspension can resume without
    /// re-walking a world the nested settle already changed.
    fn collect_hook_jobs(&self, cx: &PlayCtx, t: &CoreTrigger) -> Vec<HookJob> {
        let kind = trigger_kind(t.kind.as_str());
        let mut jobs = Vec::new();
        if is_game_start_kind(kind) || is_own_card_kind(kind) {
            // These walks are short and do not nest settles in practice;
            // they still go through `drive` / `drive_hook` (skip-aware).
            return jobs;
        }
        let world = cx.world_copy();
        for player_id in 0..world.player_count() {
            if world.out(player_id) {
                continue;
            }
            for (uid, raw) in world.field_instances(player_id as i32) {
                if is_money_hook_kind(kind) && cx.reentrant_hooks.contains(&uid) {
                    if std::env::var("DBG_HOOK").is_ok() {
                        eprintln!("SKIP uid={uid} kind={kind:?} reentrant");
                    }
                    continue;
                }
                if !field_hears(&world, t.tile, t.position, uid, kind)
                    || extra_suppressed(
                        &world,
                        t.tile,
                        t.mv.as_ref().map(|m| m.tags.as_slice()),
                        uid,
                    )
                {
                    continue;
                }
                // A marked instance runs its effective card's surface.
                let id = surface_card(&world, uid, &raw);
                let Some(idx) = self.ruleset.card(&id) else {
                    continue;
                };
                if self.ruleset.cards()[idx as usize].hooks(kind) && !world.is_face_down_at(uid) {
                    jobs.push(HookJob {
                        call: Call::Hook {
                            card: idx,
                            kind,
                            player_id: player_id as i32,
                        },
                        card_id: id.clone(),
                        uid,
                    });
                }
            }
        }
        // The raise's subject instance, alive or already unplaced. A
        // destruction removes the row before the commit, so the field walk
        // above cannot find the very instance whose own hook must resync or
        // discard its borrowed row; its id comes from the raise's provenance.
        let mut push_job = |jobs: &mut Vec<HookJob>, id: &str, uid: i32, seat: i32| {
            // A frame at `-1` is a card played from hand: the same card is
            // already on the field as the raise's subject, so it must not hear
            // the raise a second time.
            let dup = if uid < 0 {
                jobs.iter().any(|j| j.card_id == id)
            } else {
                jobs.iter().any(|j| j.uid == uid && j.card_id == id)
            };
            if dup {
                return;
            }
            let Some(idx) = self.ruleset.card(id) else {
                return;
            };
            if self.ruleset.cards()[idx as usize].hooks(kind) {
                jobs.push(HookJob {
                    call: Call::Hook {
                        card: idx,
                        kind,
                        player_id: seat,
                    },
                    card_id: id.to_string(),
                    uid,
                });
            }
        };
        if t.instance >= 0 {
            let row = world.field_by_uid(t.instance).map(|f| {
                (
                    f.effective_card.clone().unwrap_or_else(|| f.card.clone()),
                    f.owner,
                )
            });
            let id = row.as_ref().map(|r| r.0.clone()).or(t.card.clone());
            // The row's holder, not the trigger's player (see the walk above).
            let seat = row.as_ref().map_or(t.player_id, |r| r.1);
            if let Some(id) = id {
                push_job(&mut jobs, &id, t.instance, seat);
            }
        }
        let mut board: Vec<(i32, String)> = if t.tile >= 0 {
            world.tile_rule_instances(t.tile)
        } else if self
            .ruleset
            .cards()
            .iter()
            .any(|c| c.id.starts_with("tile:") && c.hooks(kind))
        {
            world.field_instances(game_core::state::BOARD_OWNER)
        } else {
            Vec::new()
        };
        if self
            .ruleset
            .cards()
            .iter()
            .any(|c| c.id.starts_with("event:") && c.hooks(kind))
        {
            for ent in world.event_rule_instances() {
                if !board.iter().any(|(u, _)| *u == ent.0) {
                    board.push(ent);
                }
            }
        }
        if self
            .ruleset
            .cards()
            .iter()
            .any(|c| c.id.starts_with("mark:") && c.hooks(kind))
        {
            for ent in world.mark_rule_instances() {
                if !board.iter().any(|(u, _)| *u == ent.0) {
                    board.push(ent);
                }
            }
        }
        // **Status** hosts (`status:*`): board-wide, collected last among the
        // field hooks (see the inline walk above; `counteract_orig`'s mirror).
        if self
            .ruleset
            .cards()
            .iter()
            .any(|c| c.id.starts_with("status:") && c.hooks(kind))
        {
            for ent in world.status_rule_instances() {
                if !board.iter().any(|(u, _)| *u == ent.0) {
                    board.push(ent);
                }
            }
        }
        for (uid, raw) in board {
            if is_money_hook_kind(kind) && cx.reentrant_hooks.contains(&uid) {
                continue;
            }
            if extra_suppressed(
                &world,
                t.tile,
                t.mv.as_ref().map(|m| m.tags.as_slice()),
                uid,
            ) {
                continue;
            }
            let id = surface_card(&world, uid, &raw);
            let Some(idx) = self.ruleset.card(&id) else {
                continue;
            };
            if self.ruleset.cards()[idx as usize].hooks(kind) && !world.is_face_down_at(uid) {
                jobs.push(HookJob {
                    call: Call::Hook {
                        card: idx,
                        kind,
                        player_id: game_core::state::BOARD_OWNER,
                    },
                    card_id: id.clone(),
                    uid,
                });
            }
        }
        for l in world.turn.lingering.clone() {
            let Some(idx) = self.ruleset.card(&l.card) else {
                continue;
            };
            if !self.ruleset.cards()[idx as usize].hooks(kind) {
                continue;
            }
            // A body still resolving for a card already heard as the raise's
            // subject must not hear it again.
            if jobs.iter().any(|j| j.card_id == l.card) {
                continue;
            }
            jobs.push(HookJob {
                call: Call::Hook {
                    card: idx,
                    kind,
                    player_id: l.owner,
                },
                card_id: l.card.clone(),
                uid: -1,
            });
        }
        // Every card body currently resolving hears the kind it declares: the
        // raise is happening inside that resolution. The frame carries its own
        // owner -- a frame at `-1` (a hand card being played) has no row. Last,
        // like the inline walk: a card also on the field or the board already
        // has its job and must not hear its own raise a second time.
        for (card, uid, seat) in resolving_snapshot() {
            push_job(&mut jobs, &card, uid, seat);
        }
        jobs
    }

    /// Run one hook job. On work-stack suspend, park `resume` and propagate.
    fn run_hook_job(
        &self,
        cx: &mut PlayCtx,
        trigger: &mut Trigger,
        job: &HookJob,
        resume: CounteractResume,
    ) -> Flow<()> {
        match self.drive_hook(cx, job.call.clone(), &job.card_id, job.uid, trigger) {
            Err(e) if e.is_suspended() => {
                cx.set_counteract_resume(Box::new(resume));
                Err(e)
            }
            r => r.map(|_| ()),
        }
    }

    fn game_start_hooks(
        &self,
        cx: &mut PlayCtx,
        player_id: i32,
        kind: TriggerKind,
        trigger: &mut Trigger,
    ) -> Flow<()> {
        let Ok(p) = usize::try_from(player_id) else {
            return Ok(());
        };
        let mut seen_uids: Vec<i32> = Vec::new();
        let mut seen_ids: Vec<String> = Vec::new();
        for _ in 0..5 {
            let mut fresh = false;
            let world = cx.world_copy();
            if p >= world.player_count() {
                return Ok(());
            }
            // Field cards including skills, in field order.
            for (uid, raw) in world.field_instances(player_id) {
                // A pile card that placed itself last pass is now a field
                // instance; its id is already in `seen_ids`, so do not
                // dispatch it a second time (two headers for one body).
                if seen_uids.contains(&uid) {
                    continue;
                }
                // A marked instance runs its effective card's surface.
                let id = surface_card(&world, uid, &raw);
                if seen_ids.contains(&id) {
                    continue;
                }
                seen_uids.push(uid);
                seen_ids.push(id.clone());
                fresh = true;
                let Some(idx) = self.ruleset.card(&id) else {
                    continue;
                };
                // 「场上所有背面朝上的卡无法产生效果」.
                if !self.ruleset.cards()[idx as usize].hooks(kind) || world.is_face_down_at(uid) {
                    continue;
                }
                trigger.card = Some(id.clone());
                self.drive_hook(
                    cx,
                    Call::Hook {
                        card: idx,
                        kind,
                        player_id,
                    },
                    &id,
                    uid,
                    trigger,
                )?;
            }
            // Card ids in the piles (and, after the mulligan, the hand).
            let mut ids: Vec<String> = Vec::new();
            for id in &world.hidden[p].deck {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
            if kind == TriggerKind::DeckAtGameStart {
                for id in &world.hidden[p].hand {
                    if !ids.contains(id) {
                        ids.push(id.clone());
                    }
                }
            }
            for id in ids {
                if seen_ids.contains(&id) {
                    continue;
                }
                seen_ids.push(id.clone());
                fresh = true;
                let Some(idx) = self.ruleset.card(&id) else {
                    continue;
                };
                if !self.ruleset.cards()[idx as usize].hooks(kind) {
                    continue;
                }
                trigger.card = Some(id.clone());
                self.drive_hook(
                    cx,
                    Call::Hook {
                        card: idx,
                        kind,
                        player_id,
                    },
                    &id,
                    -1,
                    trigger,
                )?;
            }
            if !fresh {
                break;
            }
        }
        Ok(())
    }

    /// Raise the hook-only refusal and post its visible line (design G06
    /// §6.7). The raise has no header of its own -- there is no body to
    /// announce -- so the line names the refused abnormal and the seat it
    /// would have landed on.
    fn refuse_abnormal(
        &self,
        cx: &mut PlayCtx,
        player_id: i32,
        by_player: i32,
        kind: crate::AbKind,
        on: impl Fn(&mut CoreTrigger),
    ) -> Flow<()> {
        self.raise_core(cx, "abnormalRefused", by_player, on)?;
        cx.log(
            player_id,
            Msg::new("log.abnormalRefused")
                .player_id("who", player_id)
                .i("value", kind.as_i32() as i64),
        );
        Ok(())
    }

    /// The abnormal-move gate, for an effect a card run wants to apply to
    /// `player_id` (`by` = the card's player): an out player is never hit;
    /// field cards guard (`abnormalGuard`, block with `set_cancelled`); an
    /// immune recipient (`ImmuneAll`) is refused before the window; the
    /// ruleset's `abnormalGate` may refuse the landing itself; then the
    /// `abnormal` counteraction window. Every refusal raises the hook-only
    /// `abnormalRefused` first (design G06 §6.7). What gets through bumps
    /// `player_id`'s abnormal counter for this turn and stamps the walk's
    /// abnormal span.
    fn abnormal_gate(
        &self,
        cx: &mut PlayCtx,
        player_id: i32,
        kind: crate::AbKind,
        by_player: i32,
    ) -> Flow<bool> {
        // The payload every raise below carries: the refusal hooks and the
        // window read the same target / effect / source.
        let on = |t: &mut CoreTrigger| {
            t.target = player_id;
            t.value = kind.as_i32();
            t.by_card = Some(by_player);
        };
        let Ok(s) = usize::try_from(player_id) else {
            return Ok(false);
        };
        if s >= cx.state().players.len() || cx.world().out(s) {
            // The refusal comes first: a hook-only report that this abnormal
            // never landed, with no window (design G06 §6.7).
            self.refuse_abnormal(cx, player_id, by_player, kind, on)?;
            return Ok(false);
        }
        // 「不可阻挡」 (glossary 14) is the ruleset's answer, asked through the
        // `abnormalGate` CEL gate: the refusal read below, and the
        // blocked-ness read after it. A self-caused move (`by == player_id`)
        // is *chosen*, not 「受到的」 (「可选择受到的teleport，[强制移动]，
        // [强制停下]效果是否生效」), so the host exempts it -- a card's own
        // `run_planned_move` (里美's 「期间[不可阻挡]」) must still walk.
        // The ruleset's grant verbs also refuse while its 「不可阻挡」 clause holds
        // (STA-01) -- checked before this window opens. The engine does not
        // name that key.
        // `Untargetable` at declaration, same as `target_player`: 「你只会被
        // 自己发动的效果指定」 (夏日合宿) also covers an area status grant that
        // lands on the player without a `ctx::target` pick (那天的雨's 「所有
        // 玩家获得一层[停留]」). A self-caused grant (`by == player_id`) is the
        // owner's own effect and passes the gate's own condition.
        if self
            .raise_core(cx, "untargetable", player_id, |t| {
                t.by_card = Some(by_player)
            })?
            .is_cancelled()
        {
            cx.log(
                player_id,
                Msg::new("log.untargetable").player_id("who", player_id),
            );
            return Ok(false);
        }
        // `AbnormalGuard` is a gate at declaration: a field card may block the
        // effect before anyone answers it.
        if self
            .raise_core(cx, "abnormalGuard", by_player, on)?
            .is_cancelled()
        {
            self.refuse_abnormal(cx, player_id, by_player, kind, on)?;
            return Ok(false);
        }
        // Protection resolves before the window: `ImmuneAll` lets the effect
        // name them and still land as nothing, so an immune recipient is not
        // offered (design G06 §2.13, §6.7).
        if by_player != player_id && self.immune(cx, player_id, by_player)? {
            self.refuse_abnormal(cx, player_id, by_player, kind, on)?;
            return Ok(false);
        }
        // The landing gate: the ruleset refuses the kind before any window
        // opens (「无视已存在的[停留]」 / 「无法获得新的层数」). The layer is
        // never written; existing layers still decay (「在适当时机依旧掉层」).
        let ask = game_core::engine::Gate::Abnormal {
            kind: kind.as_i32(),
            by: by_player,
            exemption: false,
        };
        if !cx.world().gate_open(self, ask, s) {
            self.refuse_abnormal(cx, player_id, by_player, kind, on)?;
            // The refusing rule supplies the skip line as data
            // (`gate_skip_msgs`), same as every other gate.
            cx.log(
                player_id,
                self.gate_skip_msg(ask, s).player_id("who", player_id),
            );
            return Ok(false);
        }
        // Blocked-ness: did this landing hold the mover's move? The engine
        // half is the mover's `MOVE_EXEMPT` (the same expression the clamp
        // uses); the ruleset half is the gate's exemption read, which answers
        // *open* for a kind that does not hold.
        let exempt = cx.world().st.players[s].state_get(game_core::state::key::MOVE_EXEMPT) > 0;
        let ask = game_core::engine::Gate::Abnormal {
            kind: kind.as_i32(),
            by: by_player,
            exemption: true,
        };
        let blocked = !exempt && !cx.world().gate_open(self, ask, s);
        // The span: the walk this landed in, read by the walk and latched at
        // its end (`TurnCtx.move_abnormal`). A landing that *is* the incoming
        // move (a forced move requested from staging) arrives before any
        // frame opens, so the bit is written to staging there and the frame
        // seeds from it (`plan_mut`, the same fallback the refusal path uses).
        let mut w = cx.world_copy();
        w.plan_mut().abnormal = true;
        cx.swap_world(w);
        // Then the chain: the recipient answers the effect declaration.
        // Self-applied abnormals (「因任何原因」/「包括你的技能」) open the
        // window too; cards filter by their guards (`effect::has(Abnormal)`).
        {
            let declared = self.raise_core(cx, "effect", by_player, |t| {
                on(t);
                t.abnormal_blocked = blocked;
                t.effects.push(game_core::engine::rules::Effect {
                    kind: "abnormal".into(),
                    target: player_id,
                    from: -1,
                    tile: -1,
                    value: kind.as_i32(),
                });
            })?;
            if declared.is_cancelled() {
                return Ok(false);
            }
        }
        // Settlement: what actually landed. `abnormal` is a hook now -- it is
        // the outcome, and cannot be counteraction'd. It runs for **every** landing,
        // self-applied included (「场上每有玩家获得」 counts each layer, and
        // 「每次受到…影响」 counts one's own too).
        self.raise_core(cx, "abnormal", by_player, |t| {
            on(t);
            t.abnormal_blocked = blocked;
        })?;
        let mut w = cx.world_copy();
        if w.turn.abnormal.len() <= s {
            w.turn.abnormal.resize(s + 1, 0);
        }
        w.turn.abnormal[s] += 1;
        // Longer-lived latch for 「若受到abnormal move影响，此卡不生效」
        // (是我自己的问题 (2)): survives across turns until the player's next
        // main move completes.
        w.state_set(player_id, game_core::state::key::ABNORMAL_HIT, 1);
        cx.swap_world(w);
        Ok(true)
    }

    /// Does a field card make `player_id` untouchable by `by`'s effects?
    /// (`immuneAll` hook, claimed with `set_cancelled`.) Logged when it holds.
    fn immune(&self, cx: &mut PlayCtx, player_id: i32, by_player: i32) -> Flow<bool> {
        let t = self.raise_core(cx, "immuneAll", player_id, |t| t.by_card = Some(by_player))?;
        if t.is_cancelled() {
            cx.log(
                player_id,
                Msg::new("log.immune_all").player_id("who", player_id),
            );
        }
        Ok(t.is_cancelled())
    }

    /// Count one naming of `named` by `by_player`, while the sheet's window is
    /// open.
    ///
    /// 规则书 (`Mor:离心力，不为所动`): 「当你在两个你的回合之间（回合1结束，
    /// 回合2开始）…成为…目标」. A seat naming itself is the same seat's own
    /// card and never counts; a designation during the named seat's own turn is
    /// before the window opens (the counter is zeroed at that turn's start).
    /// The caller has already passed `Untargetable`, so a refused name does not
    /// reach here (named counts, negated still counts).
    fn note_designation(&self, cx: &mut PlayCtx, named: i32, by_player: i32) {
        let Ok(s) = usize::try_from(named) else {
            return;
        };
        if named == by_player || cx.world().st.turn == named {
            return;
        }
        if s >= cx.state().players.len() {
            return;
        }
        let mut w = cx.world_copy();
        if w.targeted.len() <= s {
            w.targeted.resize(s + 1, 0);
        }
        w.targeted[s] += 1;
        cx.swap_world(w);
    }

    /// `by`'s card `card` tries to target player `p`. Answers the player
    /// actually targeted (a `redirect` hook may move a single-target hit), or
    /// -1. Not ported: the per-play `immune<p>` tags and the
    /// `PendingCounteract` / `TargetsChosen` pass.
    fn target_player(
        &self,
        cx: &mut PlayCtx,
        p: i32,
        by_player: i32,
        card: &str,
        single_target: bool,
    ) -> Flow<i32> {
        let Ok(s) = usize::try_from(p) else {
            return Ok(-1);
        };
        if s >= cx.state().players.len() || cx.world().out(s) {
            return Ok(-1);
        }
        if cx.world().turn.cancelled_designations.contains(&p) {
            return Ok(-1);
        }
        if !cx
            .world()
            .gate_open(cx.rules(), game_core::engine::Gate::OnBoard, s)
        {
            return Ok(-1);
        }
        // ---- declaration ----------------------------------------------
        // `Redirect` is declaration re-naming: the recipient set is settled
        // before the chain opens, so whoever holds the name is who answers.
        // Runs **before** `Untargetable` so a redirect (EXIST 「目标将改为你」)
        // re-names first, and the gate then checks the *final* name -- naming a
        // 夏日合宿-shielded seat and having EXIST move it onto the EXIST holder
        // must still land (C12).
        // The declared seat is kept: when a redirect lands, the designation's
        // cause records where it was moved from (design G10 §6.4).
        let declared = p;
        let mut landed: Option<i32> = None;
        let mut p = p;
        if single_target {
            let r = self.raise_core(cx, "redirect", by_player, |t| {
                t.target = p;
                t.card = Some(card.to_string());
                t.by_card = Some(by_player);
            })?;
            let to = r.target;
            let live = usize::try_from(to)
                .is_ok_and(|x| x < cx.state().players.len() && !cx.world().out(x));
            if to != p && to != by_player && live {
                cx.log(
                    to,
                    Msg::new("log.target_redirected")
                        .card("card", card)
                        .player_id("from", p)
                        .player_id("to", to),
                );
                landed = Some(declared);
                p = to;
            }
        }
        // A self-target (including EXIST's 「包括其他玩家指向自身的卡」) runs
        // the same tail: `Untargetable`, the window and the retraction check
        // all apply to a seat naming itself; only the between-turns count and
        // `ImmuneAll` are skipped (the named seat's own card is not 「其他角色
        // 技能或卡牌」), below.
        // `Untargetable` governs who may be *named*. Block here and the effect
        // cannot name `p` at all, so no chain forms against them. Checked on
        // the post-redirect name.
        if self
            .raise_core(cx, "untargetable", p, |t| t.by_card = Some(by_player))?
            .is_cancelled()
        {
            cx.log(p, Msg::new("log.untargetable").player_id("who", p));
            return Ok(-1);
        }
        // Named. Count it *after* naming, so a player who cannot be named does
        // not accrue 「成为目标」 counts (see `centrifugal`'s 「第二次」).
        self.note_designation(cx, p, by_player);
        // ---- chain ----------------------------------------------------
        // Counters answer the declaration; they may negate or spare it.
        if !self.target_window(cx, p, -1, by_player, card, landed)? {
            return Ok(-1);
        }
        // A counter inside the window may have cancelled this very
        // designation (`cancel_designation`, net_error's clause 1) without
        // cancelling the link; the designation is retracted, so the effect
        // names nobody.
        if cx.world().turn.cancelled_designations.contains(&p) {
            return Ok(-1);
        }
        // ---- resolution -----------------------------------------------
        // `ImmuneAll` is asked here, not at declaration: the effect named its
        // recipient, the chain formed, and it still lands as nothing. A
        // self-designation skips it (the seat's own card, as `abnormal_gate`
        // and the payment gate already do).
        if p != by_player && self.immune(cx, p, by_player)? {
            return Ok(-1);
        }
        Ok(p)
    }

    /// `by`'s card targets `tile`; another player's tile targets its owner
    /// too. Answers the tile, or -1.
    ///
    /// A tile marked [`card_sdk::abi::mark::NO_TARGET`] cannot be named at all --
    /// that is 「有标记时此地块不能被指定」. Not ported: the per-play `immune<p>` tags.
    fn target_tile(&self, cx: &mut PlayCtx, tile: i32, by_player: i32, card: &str) -> Flow<i32> {
        let no_target = game_core::state::MarkFilter {
            kind: card_sdk::abi::mark::NO_TARGET,
            category: "",
            owner: None,
            src: None,
            instance: None,
        };
        if cx.world_copy().count_marks(tile, &no_target) > 0 {
            return Ok(-1);
        }
        let Some(&owner) = usize::try_from(tile)
            .ok()
            .and_then(|t| cx.state().owners.get(t))
        else {
            return Ok(-1);
        };
        let live = usize::try_from(owner).is_ok_and(|o| !cx.world().out(o));
        if owner >= 0 && live {
            // Naming a tile names its owner (the owner answers the declaration
            // below), so `Untargetable` is asked on the owner like
            // `target_player` asks it on the named player. An own tile runs the
            // same gate and window; it does not count and is not stopped by its
            // owner's shield, below.
            if self
                .raise_core(cx, "untargetable", owner, |t| t.by_card = Some(by_player))?
                .is_cancelled()
            {
                cx.log(owner, Msg::new("log.untargetable").player_id("who", owner));
                return Ok(-1);
            }
            self.note_designation(cx, owner, by_player);
            // Chain first: the owner answers the declaration. Then resolution.
            if !self.target_window(cx, owner, tile, by_player, card, None)? {
                return Ok(-1);
            }
            if cx.world().turn.cancelled_designations.contains(&owner) {
                return Ok(-1);
            }
            if owner != by_player && self.immune(cx, owner, by_player)? {
                return Ok(-1);
            }
        }
        Ok(tile)
    }

    /// The effect declaration for a targeting: `targeted` (a field hook), then
    /// the `effect` counteraction chain, then `target` as a settlement hook.
    ///
    /// The chain is keyed on the **effect**, not on the outcome: 「被其他玩家
    /// 的卡效果影响」 listens to `effect` and reads `ctx::effect` for the list.
    /// `target` runs only once the effect actually settles, so it can no
    /// longer be used to reconstruct that clause -- it is what happened, not
    /// what was declared. False when a counter negated the declaration.
    fn target_window(
        &self,
        cx: &mut PlayCtx,
        p: i32,
        tile: i32,
        by: i32,
        card: &str,
        redirected: Option<i32>,
    ) -> Flow<bool> {
        self.raise_core(cx, "targeted", by, |t| {
            t.target = p;
            t.tile = tile;
            t.card = Some(card.to_string());
            t.by_card = Some(by);
            t.redirected_from = redirected;
        })?;
        let declared = self.raise_core(cx, "effect", by, |t| {
            t.target = p;
            t.tile = tile;
            t.card = Some(card.to_string());
            t.by_card = Some(by);
            t.redirected_from = redirected;
            t.effects.push(game_core::engine::rules::Effect {
                kind: "target".into(),
                target: p,
                from: -1,
                tile,
                value: 0,
            });
        })?;
        if declared.is_cancelled() {
            return Ok(false);
        }
        self.raise_core(cx, "target", by, |t| {
            t.target = p;
            t.tile = tile;
            t.card = Some(card.to_string());
            t.by_card = Some(by);
            t.redirected_from = redirected;
        })?;
        Ok(true)
    }

    /// Raise an engine trigger from the bridge itself (what a committed card
    /// effect did), through the same dispatcher the engine's `raise!` reaches.
    /// `player_id` may be -1 (the bank's side of a payment).
    fn raise_core(
        &self,
        cx: &mut PlayCtx,
        kind: impl Into<String>,
        player_id: i32,
        set: impl FnOnce(&mut CoreTrigger),
    ) -> Flow<CoreTrigger> {
        let kind = kind.into();
        let mut t = CoreTrigger::new(kind.clone(), player_id.max(0) as usize);
        t.player_id = player_id;
        t.step = cx.state().step;
        set(&mut t);
        // A direct construction bypasses the `tile =` arm: derive the position
        // from the tile unless the closure wrote one (设计 5.1).
        if t.position < 0 && t.tile >= 0 {
            t.position = cx.position_of_tile(t.tile);
        }
        // A raise inside an open settle inherits the settling move, like
        // `PlayCtx::raise` (设计 5.6).
        if t.mv.is_none() && matches!(kind.as_str(), "effect" | "pay" | "payAfter" | "paid") {
            if let Some(mv) = cx
                .settle_stack()
                .last()
                .and_then(|e| e.move_context.clone())
            {
                t.mv = Some(game_core::engine::TriggerMove {
                    kind: mv.kind,
                    settles: mv.settles,
                    tags: cx.plan_tags(),
                    dir: match mv.dir() {
                        -1 => game_core::engine::rules::Dir::Backward,
                        _ => game_core::engine::rules::Dir::Forward,
                    },
                    from: mv.from,
                    remaining: mv.remaining,
                    total: mv.total,
                    roll: None,
                    by_skill: mv.by_skill,
                    settle_redirect: mv.settle_redirect,
                    rolled: mv.rolled,
                    effective: mv.effective_steps(),
                    path: mv.path,
                    abnormal: mv.abnormal,
                });
            }
        }
        // A body returns from its guest call before what it caused is walked,
        // so its frame outlives the call and dies with the raise that carried
        // the walk. A suspended raise keeps its frames: it has not finished
        // (its re-entry carries the same uid and truncates at the end).
        // A suspension from the raise (a join window, a nested settle) parks
        // the walk in `counteract_resume`. The routine's Rust stack is still
        // live here, so pump the nested work and re-enter the walk instead of
        // unwinding the whole routine: a drain that ends without an ask then
        // continues the very body that raised. An `Ask` reached by the drain
        // still escapes -- the answer replay rebuilds the walk.
        let mut res = self.counteract(cx, &mut t);
        let mut pumps = 0u32;
        while res.as_ref().err().is_some_and(|e| e.is_suspended())
            && !cx.is_draining_work()
            && pumps < 64
        {
            match cx.drain_work() {
                Ok(()) => {}
                // The drain halted (`Ask`): that halt is the routine's now.
                Err(e) => {
                    res = Err(e);
                    break;
                }
            }
            pumps += 1;
            res = self.counteract(cx, &mut t);
        }
        match res {
            Ok(()) => {
                truncate_resolving(t.uid);
                Ok(t)
            }
            Err(e) => {
                if !e.is_suspended() {
                    truncate_resolving(t.uid);
                }
                Err(e)
            }
        }
    }

    /// Apply a field effect's `Dest` to the instance its run was for -- the
    /// unplace-and-send dest `set_dest` names (discard / hand / gone).
    /// `to` is whose pile it lands in; `None` is the owner it leaves, the
    /// target `set_transfer_to_dest` names.
    ///
    fn apply_dest(
        &self,
        cx: &mut PlayCtx,
        uid: i32,
        card: &str,
        dest: i32,
        to: Option<i32>,
        cause: Option<i32>,
    ) -> Flow<()> {
        let mut w = cx.world_copy();
        // `card` is the run's current surface, i.e. the stand-in when the
        // instance is marked. What leaves the field is the **row's own** card
        // (`effective_card` dies with the instance), so file that.
        let filed = cx
            .world_mut()
            .field_by_uid(uid)
            .map(|f| f.card.clone())
            .unwrap_or_else(|| card.to_string());
        // The deferred destination is applied at run commit, which is what the
        // `Resolve` cause says. The writer files the row (and raises the
        // zone-family hooks); only the reshuffle tail is left to run here.
        let left = cx.zone_leave(
            uid,
            dest,
            to.unwrap_or(-1),
            cause.unwrap_or_else(|| game_kinds::ZoneChangeCause::Resolve.as_i32()),
        )?;
        if left < 0 {
            return Ok(());
        }
        let who = to.unwrap_or(left);
        if dest == DEST_GRAVEYARD {
            let mut w = cx.world_copy();
            let refilled = w.refill_draw_pile(who.max(0) as usize);
            cx.swap_world(w);
            if refilled {
                self.raise_core(cx, "reshuffled", who, |_| {})?;
            }
        } else if dest != 1 {
            cx.log(left, Msg::new("log.card_removed").card("card", filed));
        }
        Ok(())
    }

    /// The counteraction hand window: one **round per timing**, per rulebook clauses
    /// 32 and 89.
    ///
    /// **32:** 「counteraction：带有"counteractionX：Y"效果在X发生时打出并触发效果Y，且结算优先于X」.
    /// **89:** 「如果有多名玩家可在同一时间发动counteraction效果则从行动顺序上在触发counteraction时点
    /// 的玩家的**下一位**玩家开始依次决定是否使用counteraction效果。多个效果可counteraction同一个时点，
    /// 所有玩家同意所有对一个时点的counteraction已发动后可对新的时点发动counteraction。」
    ///
    /// # Rulings (what the book leaves open)
    ///
    /// * **Resolution order:** counters to one timing settles **newest first
    ///   (LIFO)**, and every counter still settles before the timing (effect) it
    ///   answers.
    /// * **Ask cycle and per-visit floor** (user ruling 2026-10-07; supersedes
    ///   the 2026-10-06 the earlier game-derived "one activation per
    ///   visit, cycle starts after the trigger's player"): the cycle starts with
    ///   the **initial user** -- the player whose action or effect raised the
    ///   link, `chain_starter`'s seat -- and each seat **exhausts its
    ///   counteractions before priority moves on**. On a visit the seat keeps
    ///   being offered its eligible cards until it explicitly passes
    ///   (voluntarily abandons) or holds none left; every declaration adds its
    ///   link. The pass is one explicit "not playing" option on the offer
    ///   (`ask.counteract.skip`) and ends the visit.
    /// * **Round closing:** laps of the cycle continue until a **full lap
    ///   brings no new declaration** (「所有玩家同意…已发动后」). A lap that
    ///   carried a declaration never closes the round, even if every seat
    ///   passed after it -- the next full lap must be quiet. A later seat's
    ///   declaration can re-open earlier seats on the next lap.
    ///
    /// # Round on timing X
    ///
    /// `t` is **L1**, the effect declaration -- the card effect already aimed at
    /// its named recipients. One round runs on it. The cycle starts at the
    /// initial user (`chain_starter`: the player whose card caused the link; a
    /// board-driven link -- rent, buy, build, turn flow -- is a system / tile
    /// event and starts at the active turn player) and runs forward in turn
    /// order from there. Out / gated / locked players are
    /// skipped (`can_counteract_now`) -- a bot seat is **not** skipped; it is
    /// offered and answers through `PlayCtx::fill_ai` -- and a seat with no eligible card is skipped
    /// without a prompt, and `can_counteract` is re-evaluated before every
    /// offer. Each visit, the responder declares every eligible hand card
    /// answering **X** it wants -- so several players may counter the same
    /// effect, each answering X rather than each other -- or passes to end the
    /// visit; only then does priority advance to the next responder.
    ///
    /// # Counters to counters (「…后可对新的时点发动counteraction」)
    ///
    /// Once the round on X closes, the counters it collected are new timings.
    /// **Design choice (ours):** they are taken **newest first**, each with its
    /// own round (same rules; the "initial user" of that round is *that*
    /// counter's declarer), recursively; answers become timings in turn. Newest
    /// first keeps answering consistent with LIFO resolution -- the newest
    /// counter settles first, so it is the one answered first. Termination is
    /// natural (a declaration removes a card from a hand);
    /// `MAX_COUNTERACT_DEPTH` / `MAX_COUNTERACT_PER_VISIT` are the runaway
    /// guards.
    ///
    /// # Resolution
    ///
    /// LIFO over the resulting answer tree: a post-order walk that visits each
    /// node's answers **newest first**. The newest counter to a timing (and its
    /// whole subtree) settles first, then the next newest, ...; a counter's own
    /// answers settle before the counter; the timing itself settles last of its
    /// subtree. (This shape is ours -- the book only orders same-timing
    /// counters, and the walk is the composition of the two ruling clauses.) A
    /// counter's body runs against the link it answers, so `set_cancelled` /
    /// `negate_effect` / `spare` land on that link and its guard read what that
    /// link declared (`effect::` sees the list). A counter whose own link one of
    /// its answers negated does not run its body; the declaration still stands
    /// and the card is spent to its default dest (the discard). `Dest`
    /// handling, the `discarded` raise and `log.play_counteract` are unchanged.
    ///
    /// Known cases outside this (left as-is, flagged for a later pass):
    ///
    /// * **The acting card's own follow-up** (`CardRules::counteract` step (1)) runs
    ///   before this window, outside the chain.
    fn hand_counteractions(
        &self,
        cx: &mut PlayCtx,
        t: &CoreTrigger,
        trigger: &mut Trigger,
        depth: u32,
    ) -> Flow<()> {
        if depth >= MAX_COUNTERACT_DEPTH || !cx.playing() {
            return Ok(());
        }
        let n = cx.state().players.len();
        if n == 0 {
            return Ok(());
        }

        // Valid-option-first (docs/GUARDS.md §4.5): open a counteraction window only
        // when someone can respond. Cheap static index first (the per-card kind
        // bitmask + seat eligibility), then the compiled conditions against a
        // window context built straight from the live world -- no `Run`, no
        // world copy, and the CEL scope only if a candidate actually declares a
        // condition. Nothing qualifies => skip the whole cycle: no chain clone,
        // no `Priority`, no per-visit offers. That is every raise nobody
        // answers, and one quiet lap today would close the round identically.
        if !counteract_slow_path() && !self.any_counter_candidate(cx, trigger) {
            #[cfg(feature = "bot-cost")]
            crate::host::bot_cost::COUNTERACT_WINDOWS_SKIPPED
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }
        #[cfg(feature = "bot-cost")]
        crate::host::bot_cost::COUNTERACT_WINDOWS
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // The answer tree. Index 0 is L1, the effect declaration; every other
        // node is one declared counter. Boxed so its address survives the
        // resume park (a continuation's chain cursor stays valid).
        let mut chain = Box::new(vec![ChainLink {
            seat: chain_starter(t, cx.state().turn, n),
            idx: -1,
            id: t.card.clone().unwrap_or_default(),
            uid: -1,
            move_extension: 0,
            arg: Vec::new(),
            answered: 0,
            join: None,
            joined_in_window: false,
            saved_join: None,
            link: trigger.clone(),
            answers: Vec::new(),
            settled: false,
        }]);

        // Hard bound: the tree is only as deep and as wide as the hands
        // involved, so this is a runaway guard, not a design limit. One round
        // is at most (declarations + a quiet lap) visits, each visit at most
        // `MAX_COUNTERACT_PER_VISIT` offers, over `MAX_COUNTERACT_DEPTH + 1`
        // rounds.
        let mut budget = (MAX_COUNTERACT_DEPTH + 1) * n as u32 * (MAX_COUNTERACT_PER_VISIT + 1) + 8;
        // 真奈 join (CH-05): the block is created at declaration in
        // `build_round`, so there is no hand pre-scan and no cleanup pass.
        self.build_round(cx, &mut chain, 0, depth, &mut budget, None)?;
        // STACK-01: settles as a job list so a nested settle can suspend the
        // walk and resume without rebuilding the chain.
        let mut order = Vec::new();
        collect_resolve_order(&chain, 0, &mut order);
        self.resolve_jobs(cx, trigger, &mut chain, &order, 0)?;

        // L1 settles next, in the engine; give it the same window readers a
        // counter body gets. Its dest is whatever the counters did to it.
        let mut l1 = chain[0].link.clone();
        self.stamp_window(cx, &chain, 0, &mut l1);
        chain[0].link = l1;
        *trigger = chain[0].link.clone();
        // A join continuation parked inside this walk still indexes the
        // chain through its cursor; keep the box alive with the context.
        let taken: Box<Vec<ChainLink>> = std::mem::take(&mut chain);
        let kept: Box<dyn std::any::Any + Send> = taken;
        cx.keep_chain(kept);
        Ok(())
    }

    /// One round on `chain[timing]`, then -- newest first, the book's 「…后可对
    /// 新的时点发动counteraction」 -- one round per counter it collected, recursively.
    /// See [`Self::hand_counteractions`].
    fn build_round(
        &self,
        cx: &mut PlayCtx,
        chain: &mut Box<Vec<ChainLink>>,
        timing: usize,
        depth: u32,
        budget: &mut u32,
        // A join window offers exactly these seats, in list order, and presents
        // each as the link's target (§6.4). `None` = turn order.
        over: Option<&[i32]>,
    ) -> Flow<()> {
        if depth >= MAX_COUNTERACT_DEPTH {
            return Ok(());
        }
        let n = cx.state().players.len();
        // The cycle starts at the **initial user** (ruling 2026-10-07): the
        // player the timing belongs to -- `chain_starter` at the root, the
        // declarer on a counter's own round.
        let mut priority = match over {
            Some(seats) => Priority::over(seats),
            None => Priority::new(chain[timing].seat, n),
        };
        // Counters this round collected, in declaration order.
        let mut round: Vec<usize> = Vec::new();
        // One processed set per window (this cycle, across its laps): a later
        // lap reuses a probe's verdict when the inputs it reads cannot have
        // changed (docs/GUARDS.md §4.5).
        let mut memo = ProbeMemo::new();
        'visits: while *budget > 0 {
            let cursor = priority.seat();
            // Ruling 2026-10-07: the visit keeps the floor until this seat
            // passes (the offer's `ask.counteract.skip`) or holds no eligible
            // counteraction left -- every declaration re-offers the same seat.
            // Each offer is its own prompt, so the AI answer (chaos re-rolls
            // its counter chance per offer) is recomputed per offer in
            // `PlayCtx::ask` / `fill_ai`.
            let mut declared_any = false;
            if can_counteract_now(cx, cursor)
                || self.has_field_counter(cx, cursor, &chain[timing].link)
            {
                for _ in 0..MAX_COUNTERACT_PER_VISIT {
                    if *budget == 0 {
                        break 'visits;
                    }
                    *budget -= 1;
                    // One source per shared group, reserved at declaration (before
                    // any of the bodies settle). Duplicate cards cannot re-offer it.
                    let groups: Vec<i32> = chain[timing]
                        .answers
                        .iter()
                        .filter(|&&at| chain[at].seat == cursor)
                        .filter_map(|&at| {
                            self.ruleset.cards()[chain[at].idx as usize]
                                .props
                                .get(card_sdk::abi::prop::COUNTERACT_GROUP)
                                .copied()
                        })
                        .collect();
                    // A join round presents the visited seat as the link's
                    // target for its own offer ("as if each were its target",
                    // 「如同自身的对应目标被指定一般」); the real recipient is
                    // put back for the declared link (design G06 §6.4).
                    let offer_target = priority.over.as_ref().map(|_| cursor as i32);
                    let saved_target = chain[timing].link.target;
                    if let Some(t) = offer_target {
                        chain[timing].link.target = t;
                        // The memo's scope was built for a previous seat's
                        // target; the join presents this seat as the target
                        // (design G06 §6.4).
                        memo.invalidate_scope();
                    }
                    // The pending readers evaluate against the chain.
                    let prev_chain =
                        cx.set_counteract_chain(Some(Box::new(CounteractChainCursor {
                            chain: (&mut **chain) as *mut Vec<ChainLink>,
                            index: timing,
                        })));
                    if std::env::var("DBG_OFFER").is_ok() {
                        eprintln!(
                            "VISIT seat={cursor} target={} hand={:?} over={:?}",
                            chain[timing].link.target,
                            hand_of(cx, cursor),
                            priority.over
                        );
                    }
                    let offered = self.declare_one(cx, cursor, chain, timing, &mut memo, &groups);
                    cx.set_counteract_chain(prev_chain);
                    if offer_target.is_some() {
                        chain[timing].link.target = saved_target;
                    }
                    if std::env::var("DBG_OFFER").is_ok() {
                        eprintln!(
                            "OFFER_RESULT seat={cursor} ok={} err={:?}",
                            offered.as_ref().map(|o| o.is_some()).unwrap_or(false),
                            offered.as_ref().err().map(|e| format!("{e:?}"))
                        );
                    }
                    let Some((id, idx, uid, move_extension, arg)) = offered? else {
                        // Explicit pass, or nothing eligible left: the visit
                        // ends and priority advances.
                        break;
                    };
                    // A face-down card played directly is revealed: it is in
                    // play now, its hooks resume, and the client shows the
                    // face (「对帅气的憧憬」(3) -- 「将其直接打出」).
                    if uid >= 0 && cx.world().is_face_down_at(uid) {
                        cx.world_mut().set_face_down_at(uid, false);
                    }
                    // The declaration leaves the hand now.
                    if uid < 0 {
                        let mut w = cx.world_copy();
                        if let Some(pos) = w.hidden[cursor].hand.iter().position(|c| c == &id) {
                            w.hidden[cursor].hand.remove(pos);
                            w.settle_over_hand_allowance(cursor);
                        }
                        cx.swap_world(w);
                    }
                    // A hand changed: hand-sensitive probe verdicts and the
                    // scope's `_hand` table are stale from here.
                    memo.on_declaration();
                    // The counteraction history 「本回合内」 (无法将视线移开): the
                    // declaration, before any body, cleared at `turn_start`.
                    {
                        let mut w = cx.world_copy();
                        w.countered_this_turn.push(CounteractRecord {
                            declarer: cursor as i32,
                            countered: chain[timing].seat as i32,
                            card: id.clone(),
                        });
                        cx.swap_world(w);
                    }
                    // The counter link carries the answered link forward (move /
                    // pay / effects / subject / join / tags) before its own
                    // overrides below: a contract and a body must both be able
                    // to read what they answer.
                    let at = chain.len();
                    let mut link = chain[timing].link.clone();
                    link.kind = if uid >= 0 {
                        crate::TriggerKind::CardUsed
                    } else {
                        crate::TriggerKind::Card
                    };
                    if uid >= 0 {
                        link.cards = vec![id.clone()];
                    }
                    link.card = Some(id.clone());
                    // A counter's own subject is itself, so an entry answering
                    // it (tsugumi on YOLO) reads the counter's properties.
                    link.subject_card = Some(id.clone());
                    // A `copies_previous` entry resolved to the copied card's
                    // own entry: name the source so the body reads it.
                    link.copied = None;
                    if self.ruleset.card(&id) != Some(idx) {
                        link.copied = Some(self.ruleset.cards()[idx as usize].id.clone());
                    }
                    link.step = cx.state().step;
                    link.by_card = Some(cursor as i32);
                    // A counter used against someone is the `counteracted`
                    // timing of 「有玩家对你使用counteraction后」 (无法将视线移开): the
                    // link's `target` is the seat whose effect is being
                    // answered, `actor`/`player_id` the counter's declarer.
                    link.target = chain[timing].seat as i32;
                    link.player_id = cursor as i32;
                    link.seq = (at + 1) as u32;
                    link.answers = timing as u32;
                    // Its own pending set, a fresh negation and spared list.
                    link.answers_declared.clear();
                    link.negation = game_core::engine::rules::Negation::None;
                    link.spared.clear();
                    // The enclosing roll face a body's write propagates to.
                    link.roll_root = nearest_roll_link(chain, timing);
                    round.push(at);
                    chain[timing].answers.push(at);
                    // This answer is now declared and unresolved on the link --
                    // what `trigger::pending_negation` counts (`now_sumimi`).
                    // The remaining offers on the link must see it, so the
                    // counter is kept here beside the `extreme_armed` flag
                    // below (design G06 §6.2).
                    chain[timing].link.answers_declared.push(at as u32);
                    if self
                        .ruleset
                        .entry_effect(idx, &chain[timing].link, None)
                        .contains(card_sdk::abi::AnswerEffect::NEGATES)
                    {
                        chain[timing].link.pending_negation += 1;
                    }
                    // An effect that arms the extreme is declared *now*; the
                    // remaining offers on this link must see it (a second
                    // copy is not offered, design G06 §6.6). The settle-time
                    // stamps refresh the same field from `answers_declared`.
                    if self
                        .ruleset
                        .entry_effect(idx, &chain[timing].link, None)
                        .contains(card_sdk::abi::AnswerEffect::ARMS_EXTREME)
                    {
                        chain[timing].link.extreme_armed = true;
                        // The counter's own link is a later timing: it is
                        // inherited from this one, so it must be armed too or
                        // the next round offers a second copy.
                        link.extreme_armed = true;
                    }
                    // A declaration inside a join window counts for its report
                    // even when the counter is negated later (「打出过」).
                    if let Some(j) = &mut chain[timing].link.join {
                        if let Some(k) = j.seats.iter().position(|&s| s == cursor as i32) {
                            j.declared[k] = true;
                        }
                    }
                    // 真奈 join (CH-05): a `SELF_REDIRECT` counter opens a join
                    // window on the link it answers -- the other seats may answer
                    // the same link as if each were its target, and a landing
                    // counter shields the opener (「改为你」). Declaration-scoped:
                    // the block belongs to this node and is installed for its
                    // body, so neither it nor a nested window can leak.
                    let mut join_open = None;
                    // One window per link: a `SELF_REDIRECT` counter declared
                    // *inside* a join round (`over` is a join seat list) is a
                    // joiner of the window already offering it -- that window
                    // covers this same link, so a second round would overlap
                    // the first's seats and its settle order. The body reads
                    // the outer report instead (`trigger.joined_in_window`,
                    // `join::open` answers `1`; design G06 §6.4).
                    if over.is_none()
                        && self.ruleset.cards()[idx as usize]
                            .props
                            .get(card_sdk::abi::prop::SELF_REDIRECT)
                            .copied()
                            .unwrap_or(0)
                            != 0
                    {
                        let opener = cursor as i32;
                        let seats: Vec<i32> = (0..n as i32)
                            .filter(|&s| s != opener && !cx.world().out(s as usize))
                            .collect();
                        // A same-timing counter already declared on the answered
                        // link *is* a joiner (design G06 §6.4: the window offers
                        // the seats 「如同自身的对应目标被指定一般」; a declaration
                        // made before the opener's body still counts and must not
                        // be offered twice). Mark those seats now so the report
                        // and `join::joiner_count` see them.
                        let declared: Vec<bool> = seats
                            .iter()
                            .map(|&s| {
                                // Index 0 is the timing itself, not a counter.
                                chain
                                    .iter()
                                    .skip(1)
                                    .any(|c| c.answered == timing && c.seat as i32 == s)
                            })
                            .collect();
                        let k = seats.len();
                        join_open = Some(crate::world::TriggerJoin {
                            opener,
                            seats,
                            declared,
                            used: vec![false; k],
                            designated: vec![-1; k],
                            opened: false,
                        });
                        chain[timing].link.join_redirect = Some(opener);
                    }
                    // Master Duel's chain build-up: the first counter opens the
                    // chain, so L1 (the initiating card) is announced first,
                    // then each declaration is a `"chain"` event whose `value`
                    // is its link number. The client flashes the card and
                    // appends it to the chain overlay.
                    if chain.len() == 1 {
                        let (l1_seat, l1_id) = (chain[0].seat as i32, chain[0].id.clone());
                        // A timing with no card of its own (a roll, a payment,
                        // a landing ...) says what it is: the log line names
                        // the trigger itself via [`crate::trigger_reason`] --
                        // never 「（未命名）」 -- and the same message carries
                        // the kind and particulars for the client's origin
                        // tile.
                        let l = &chain[0].link;
                        let src = match l.roll_source {
                            game_core::engine::rules::RollSource::Charge => "charge",
                            game_core::engine::rules::RollSource::Card => "card",
                            game_core::engine::rules::RollSource::Skill => "skill",
                            _ => "",
                        };
                        let mut origin = if l1_id.is_empty() {
                            Msg::new("log.chain_link_trigger")
                                .n("n", 1)
                                .msg("trigger", crate::trigger_reason(l))
                        } else {
                            Msg::new("log.chain_link")
                                .n("n", 1)
                                .card("card", l1_id.clone())
                        }
                        .text("kind", l.kind.spelling())
                        .player_id("who", l.player_id)
                        .i("value", l.value as i64)
                        .text("src", src)
                        .text(
                            "rent",
                            if l.pay.as_ref().is_some_and(|p| p.is_rent) {
                                "1"
                            } else {
                                ""
                            },
                        );
                        if l.tile >= 0 {
                            origin = origin.tile("tile", l.tile);
                        }
                        // Particulars for the origin tile: who it is aimed at,
                        // whose card caused it, the move / purchase behind it
                        // and the effects it declares.
                        if l.target != l.player_id && l.target >= 0 {
                            origin = origin.player_id("target", l.target);
                        }
                        if let Some(by) = l.by_card.filter(|b| *b >= 0 && *b != l.player_id) {
                            origin = origin.player_id("by", by);
                        }
                        if l.is_main_move {
                            origin = origin.text("is_main_move", "1");
                        }
                        if let Some(mv) = &l.mv {
                            origin = origin
                                .tile("mv_from", mv.from)
                                .i("mv_total", mv.total as i64)
                                .i("mv_left", mv.remaining as i64);
                            if let Some(r) = mv.roll {
                                origin = origin.i("mv_roll", r as i64);
                            }
                        }
                        if let Some(b) = &l.buy {
                            origin = origin.i("price", b.price as i64);
                        }
                        let fx: Vec<game_core::msg::Arg> = l
                            .effects
                            .iter()
                            .map(|e| {
                                let mut m = Msg::new("fx")
                                    .text("kind", e.kind.clone())
                                    .i("value", e.value as i64);
                                if e.target >= 0 {
                                    m = m.player_id("target", e.target);
                                }
                                if e.tile >= 0 {
                                    m = m.tile("tile", e.tile);
                                }
                                game_core::msg::Arg::Msg(Box::new(m))
                            })
                            .collect();
                        if !fx.is_empty() {
                            origin = origin.list("effects", fx);
                        }
                        cx.log_event("chain", l1_seat, origin, -1, &l1_id, 1);
                    }
                    cx.log_event(
                        "chain",
                        cursor as i32,
                        Msg::new("log.chain_link")
                            .n("n", at as i64 + 1)
                            .card("card", id.clone()),
                        -1,
                        &id,
                        at as i32 + 1,
                    );
                    chain.push(ChainLink {
                        seat: cursor,
                        idx,
                        id,
                        uid,
                        move_extension,
                        arg,
                        answered: timing,
                        join: join_open,
                        joined_in_window: over.is_some(),
                        saved_join: None,
                        link,
                        answers: Vec::new(),
                        settled: false,
                    });
                    #[cfg(feature = "bot-cost")]
                    crate::host::bot_cost::COUNTERACT_DECLARED
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    declared_any = true;
                }
            }
            let open = priority.answered(declared_any);
            if std::env::var("DBG_OFFER").is_ok() {
                eprintln!(
                    "VISIT_END declared={declared_any} open={open} cursor={} in_lap={} n={}",
                    priority.cursor, priority.in_lap, priority.n
                );
            }
            if !open {
                break;
            }
        }

        // The counters just declared are themselves timings. Answer the newest
        // first (LIFO), each with its own round starting at its declarer (the
        // new round's initial user). A child that opens its own window gets
        // its block on the link while its round builds -- its offers and the
        // declaration marks must land there, not on the enclosing window --
        // and the enclosing block goes back afterwards.
        for &child in round.iter().rev() {
            let saved = chain[child].join.clone().map(|b| {
                chain[timing].link.join_redirect = Some(b.opener);
                chain[timing].link.join.replace(b)
            });
            self.build_round(cx, chain, child, depth + 1, budget, None)?;
            if let Some(saved) = saved {
                chain[timing].link.join = saved;
                chain[timing].link.join_redirect =
                    chain[timing].link.join.as_ref().map(|j| j.opener);
            }
        }
        Ok(())
    }

    /// Resolve the answer tree LIFO: post-order, answers newest-first -- a
    /// counter's own answers settle before it, and sibling counters settle
    /// newest first. Each counter's body runs against the link it answers; one
    /// whose own link an answer negated does not run.
    fn resolve_rounds(&self, cx: &mut PlayCtx, chain: &mut Vec<ChainLink>, idx: usize) -> Flow<()> {
        let answers = chain[idx].answers.clone();
        for &child in answers.iter().rev() {
            self.resolve_rounds(cx, chain, child)?;
        }
        if idx == 0 {
            // L1 settles in the engine; its link is already whatever the
            // counters made of it.
            return Ok(());
        }
        let seat = chain[idx].seat;
        let card = chain[idx].idx;
        let id = chain[idx].id.clone();
        let answered = chain[idx].answered;
        // What the counter's own answers did to its link: a negation there
        // voids the counter's settlement (`set_cancelled` -- the activation
        // never happened -- or `negate_effect`, which settles to nothing).
        let negated = chain[idx].link.is_cancelled();
        let uid = chain[idx].uid;
        if uid < 0 {
            cx.log(
                seat as i32,
                Msg::new("log.play_counteract")
                    .player_id("who", seat as i32)
                    .card("card", id.clone()),
            );
        }
        let dest = if negated {
            // The body does not run, so the card has no dest of its own: it was
            // played (it left the hand at declaration) and is spent. It still
            // flashes -- marked 无效 -- so the negation is visible -- and names
            // the counter that negated it.
            let link = &chain[answered].link;
            let negators: Vec<String> = chain[idx]
                .answers
                .iter()
                .map(|&a| chain[a].id.clone())
                .collect();
            let msg = Msg::new("log.card_negated").card("card", id.clone());
            let parent = cx.activation.last().copied().unwrap_or(-1);
            let ev = cx.log_card_activation(
                game_core::state::card_trigger::COUNTER,
                seat as i32,
                &id,
                link.player_id,
                link.tile,
                true,
                msg,
                parent,
                -1,
            );
            if let Some(n) = negators.first() {
                cx.set_event_results(
                    ev,
                    vec![Msg::new("log.outcome.negated_by").card("card", n.clone())],
                );
            }
            DEST_UNSET
        } else {
            // The counter's body runs against the link it answers, so its
            // `set_cancelled` / `negate_effect` / `spare` land there -- and the
            // effect settles only after every counter has had its say.
            let mut on_link = chain[answered].link.clone();
            // A `copies_previous` entry resolved to the copied card's entry
            // names its source on its own link (stamped at declaration); the
            // copied body reads it with `trigger::link_copied`.
            on_link.copied = chain[idx].link.copied.clone();
            // A joiner reads the window it is answering; its body's
            // `join::open()` must not open a second one (design G06 §6.4).
            on_link.joined_in_window = chain[idx].joined_in_window;
            if chain[idx].move_extension > 0 {
                // The extension is a move-tag rewrite; it needs a move payload.
                if let Some(m) = &mut on_link.mv {
                    m.tags.push((
                        card_sdk::abi::COUNTERACT_MOVE_EXTENSION.to_string(),
                        chain[idx].move_extension,
                    ));
                }
            }
            // The offer-time readers, resolved from the chain and stamped on
            // the link the body runs against (design G06 §6.2).
            self.stamp_window(cx, chain, idx, &mut on_link);
            let before_face = on_link
                .mv
                .as_ref()
                .and_then(|m| m.roll)
                .unwrap_or(on_link.value);
            let prev_chain = cx.set_counteract_chain(Some(Box::new(CounteractChainCursor {
                chain: chain as *mut Vec<ChainLink>,
                index: idx,
            })));
            let driven = self.drive(
                cx,
                Call::Counteract {
                    card,
                    player_id: seat as i32,
                    arg: Vec::new(),
                    entry: None,
                },
                &id,
                uid,
                &mut on_link,
            );
            cx.set_counteract_chain(prev_chain);
            let dest = driven?;
            if let Some(m) = &mut on_link.mv {
                m.tags
                    .retain(|(key, _)| key != card_sdk::abi::COUNTERACT_MOVE_EXTENSION);
            }
            propagate_face(chain, &on_link, before_face);
            chain[answered].link = on_link;
            dest
        };
        self.hand_play_tail(cx, seat, &id, uid, negated, dest)
    }

    /// Run the post-order settles walk as a job list (STACK-01). On suspend,
    /// parks a `HandResume` and returns `Halt::Suspended` so the body that
    /// asked for a nested settle is not re-run.
    fn resolve_jobs(
        &self,
        cx: &mut PlayCtx,
        trigger: &Trigger,
        chain: &mut Box<Vec<ChainLink>>,
        order: &[usize],
        mut next: usize,
    ) -> Flow<()> {
        // The closed chain starts resolving: one `"settles"` banner cue
        // (`value` 0), then one per link as it settles, newest first
        // (`value` = link number), and finally L1 itself (`value` 1).
        if next == 0 && !order.is_empty() {
            cx.log_event("settles", -1, Msg::new("log.chain_resolve"), -1, "", 0);
        }
        while next < order.len() {
            self.settle_node(cx, trigger, chain, order[next], order, next)?;
            next += 1;
        }

        if !order.is_empty() {
            let (l1_seat, l1_id) = (chain[0].seat as i32, chain[0].id.clone());
            // Same wording as the announce: a timing with no card of its own
            // names the trigger, not 「（未命名）」.
            let l1 = if l1_id.is_empty() {
                Msg::new("log.chain_resolve_link_trigger")
                    .n("n", 1)
                    .msg("trigger", crate::trigger_reason(&chain[0].link))
            } else {
                Msg::new("log.chain_resolve_link")
                    .n("n", 1)
                    .card("card", l1_id.clone())
            };
            cx.log_event("settles", l1_seat, l1, -1, &l1_id, 1);
        }
        Ok(())
    }

    /// Settle one chain node -- the job body of [`Self::resolve_jobs`], shared
    /// with the join walk `settle_join`. Logs the link, applies the negation
    /// check and runs the body against the answered link. A nested suspend
    /// parks the given `order` / `next` and returns the halt.
    fn settle_node(
        &self,
        cx: &mut PlayCtx,
        trigger: &Trigger,
        chain: &mut Box<Vec<ChainLink>>,
        idx: usize,
        order: &[usize],
        next: usize,
    ) -> Flow<i32> {
        let seat = chain[idx].seat;
        let card = chain[idx].idx;
        let id = chain[idx].id.clone();
        cx.log_event(
            "settles",
            seat as i32,
            Msg::new("log.chain_resolve_link")
                .n("n", idx as i64 + 1)
                .card("card", id.clone()),
            -1,
            &id,
            idx as i32 + 1,
        );
        let answered = chain[idx].answered;
        let negated = chain[idx].link.is_cancelled();
        let uid = chain[idx].uid;
        if uid < 0 {
            cx.log(
                seat as i32,
                Msg::new("log.play_counteract")
                    .player_id("who", seat as i32)
                    .card("card", id.clone()),
            );
        }
        let dest = if negated {
            let link = &chain[answered].link;
            let negators: Vec<String> = chain[idx]
                .answers
                .iter()
                .map(|&a| chain[a].id.clone())
                .collect();
            let msg = Msg::new("log.card_negated").card("card", id.clone());
            let parent = cx.activation.last().copied().unwrap_or(-1);
            let ev = cx.log_card_activation(
                game_core::state::card_trigger::COUNTER,
                seat as i32,
                &id,
                link.player_id,
                link.tile,
                true,
                msg,
                parent,
                -1,
            );
            if let Some(n) = negators.first() {
                cx.set_event_results(
                    ev,
                    vec![Msg::new("log.outcome.negated_by").card("card", n.clone())],
                );
            }
            DEST_UNSET
        } else {
            // A `SELF_REDIRECT` node installs its window on the answered link
            // for its body; the enclosing window (if any) goes back when the
            // body returns (design G06 §6.4).
            if let Some(block) = chain[idx].join.clone() {
                let opener = block.opener;
                chain[idx].saved_join = chain[answered].link.join.replace(block);
                chain[answered].link.join_redirect = Some(opener);
            }
            let mut on_link = chain[answered].link.clone();
            // A `copies_previous` entry resolved to the copied card's entry
            // names its source on its own link (stamped at declaration); the
            // copied body reads it with `trigger::link_copied`.
            on_link.copied = chain[idx].link.copied.clone();
            // A joiner reads the window it is answering; its body's
            // `join::open()` must not open a second one (design G06 §6.4).
            on_link.joined_in_window = chain[idx].joined_in_window;
            if chain[idx].move_extension > 0 {
                if let Some(m) = &mut on_link.mv {
                    m.tags.push((
                        card_sdk::abi::COUNTERACT_MOVE_EXTENSION.to_string(),
                        chain[idx].move_extension,
                    ));
                }
            }
            // The offer-time readers (`trigger::pending_negation` /
            // `extreme_armed` / `answered_kinds`) from the link's declared
            // answers (design G06 §6.2).
            on_link.answers_declared = chain[answered].link.answers_declared.clone();
            on_link.pending_negation = chain[answered]
                .link
                .answers_declared
                .iter()
                .filter(|&&d| {
                    let n = &chain[d as usize];
                    self.ruleset
                        .entry_effect(n.idx, &n.link, None)
                        .contains(card_sdk::abi::AnswerEffect::NEGATES)
                })
                .count() as i32;
            on_link.extreme_armed = (cx.world().turn.extreme != game_core::engine::Extreme::Plain)
                || chain[answered].link.answers_declared.iter().any(|&d| {
                    let n = &chain[d as usize];
                    self.ruleset
                        .entry_effect(n.idx, &n.link, None)
                        .contains(card_sdk::abi::AnswerEffect::ARMS_EXTREME)
                });
            on_link.answered_kinds.clear();
            {
                let mut walk = vec![idx];
                let mut cur = idx;
                while cur != 0 {
                    cur = chain[cur].link.answers as usize;
                    walk.push(cur);
                }
                on_link.answered_kinds =
                    walk.into_iter().rev().map(|i| chain[i].link.kind).collect();
            }
            let before_face = on_link
                .mv
                .as_ref()
                .and_then(|m| m.roll)
                .unwrap_or(on_link.value);
            let prev_chain = cx.set_counteract_chain(Some(Box::new(CounteractChainCursor {
                chain: (&mut **chain) as *mut Vec<ChainLink>,
                index: idx,
            })));
            let driven = self.drive(
                cx,
                Call::Counteract {
                    card,
                    player_id: seat as i32,
                    // The stored arg is the only source of truth at
                    // settles time (docs/ACTIVATION.md §2): `OP_DECLARE`
                    // ran at the offer pass and is never re-entered here.
                    arg: chain[idx].arg.clone(),
                    entry: None,
                },
                &id,
                uid,
                &mut on_link,
            );
            cx.set_counteract_chain(prev_chain);
            match driven {
                Err(e) if e.is_suspended() => {
                    if let Some(m) = &mut on_link.mv {
                        m.tags
                            .retain(|(key, _)| key != card_sdk::abi::COUNTERACT_MOVE_EXTENSION);
                    }
                    // The node's own window stays installed while its body
                    // resumes; the continuation restores the enclosing one.
                    // `on_link` predates the run: a join round settled inside
                    // it has since marked its block (`declared` / `used` /
                    // `designated`), and the marks live only on the chain's
                    // copy -- take that block back, or the writeback below
                    // drops them (design G06 §6.4).
                    if chain[answered].link.join.is_some() {
                        on_link.join = chain[answered].link.join.clone();
                    }
                    chain[answered].link = on_link;
                    let mut r = cx
                        .take_counteract_resume()
                        .and_then(|s| s.downcast::<CounteractResume>().ok().map(|b| *b))
                        .unwrap_or(CounteractResume {
                            trigger: trigger.clone(),
                            own_done: true,
                            hand: None,
                            hooks: Vec::new(),
                            hook_index: 0,
                            chain: None,
                            join: None,
                        });
                    // Moved, not cloned: the boxed chain keeps its address for
                    // the continuation's cursor.
                    r.chain = Some(std::mem::take(chain));
                    // Inside a join walk the outer hand cursor already points
                    // past the opener -- the join's own progress is in
                    // `JoinResume`, so leave it alone.
                    if r.join.is_none() {
                        r.hand = Some(HandResume {
                            order: order.to_vec(),
                            next: next + 1,
                        });
                    }
                    cx.set_counteract_resume(Box::new(r));
                    return Err(e);
                }
                Ok(dest) => {
                    self.close_drive(cx, chain, idx, &mut on_link, before_face)?;
                    dest
                }
                Err(e) => return Err(e),
            }
        };
        self.settle_tail(cx, seat, &id, uid, negated, dest)?;
        chain[idx].settled = true;
        Ok(dest)
    }

    /// Land a finished drive on `chain[idx]`'s answered link: drop the move
    /// extension tag, restore the enclosing window, compose a face write, and
    /// (when the `SELF_REDIRECT` node opened its own window) put the answered
    /// link's previous block back (design G06 §6.3/§6.4). Shared by the
    /// same-stack settle and a resumed continuation.
    fn close_drive(
        &self,
        cx: &mut PlayCtx,
        chain: &mut Vec<ChainLink>,
        idx: usize,
        on_link: &mut Trigger,
        before_face: i32,
    ) -> Flow<()> {
        if let Some(m) = &mut on_link.mv {
            m.tags
                .retain(|(key, _)| key != card_sdk::abi::COUNTERACT_MOVE_EXTENSION);
        }
        if chain[idx].join.is_some() {
            on_link.join = chain[idx].saved_join.take();
            on_link.join_redirect = on_link.join.as_ref().map(|j| j.opener);
        }
        propagate_face(chain, on_link, before_face);
        let answered = chain[idx].answered;
        chain[answered].link = on_link.clone();
        Ok(())
    }

    /// Apply a settled counter's dest (design G06 keeps the pre-existing
    /// shape: hand counters are spent, field ones stay). `uid >= 0` is a field
    /// skill and files nothing. `negated` rides the `cardPlayed` announcement
    /// (G08 §2.4): a hand counter is a play even when it is counteracted, so
    /// the outcome is what a 「被抵消时」 clause reads.
    fn settle_tail(
        &self,
        cx: &mut PlayCtx,
        seat: usize,
        id: &str,
        uid: i32,
        negated: bool,
        dest: i32,
    ) -> Flow<()> {
        self.hand_play_tail(cx, seat, id, uid, negated, dest)
    }

    /// File a spent hand counter, raise `discarded`, then announce the play
    /// with `cardPlayed` (G08 §2.4, design: a hand counter goes through the
    /// counteraction tree, not `play_from_hand`, but its play is still
    /// announced -- exactly as `play_from_hand` does, on every dest and with
    /// the outcome). A field skill (`uid >= 0`) stays in place, files nothing
    /// and was already announced at its own body.
    fn hand_play_tail(
        &self,
        cx: &mut PlayCtx,
        seat: usize,
        id: &str,
        uid: i32,
        negated: bool,
        dest: i32,
    ) -> Flow<()> {
        if uid >= 0 {
            return Ok(());
        }
        let mut w = cx.world_copy();
        // A spent card is filed even when the seat went out on this play
        // (C-Q5(a): the card is spent regardless).
        let spent = matches!(dest_from(dest), Dest::Graveyard);
        let mut refilled = false;
        match dest_from(dest) {
            Dest::Graveyard => {
                w.hidden[seat].graveyard.push(id.to_string());
                refilled = w.refill_after_pile_write(seat);
            }
            Dest::Hand => w.hidden[seat].hand.push(id.to_string()),
            Dest::Banished => {}
            Dest::Field => {}
        }
        cx.swap_world(w);
        if refilled {
            self.raise_core(cx, "reshuffled", seat as i32, |_| {})?;
        }
        if spent {
            self.raise_core(cx, "discarded", seat as i32, |t| {
                t.card = Some(id.to_string())
            })?;
        }
        // `cardPlayed` (hook) -- the counter's hand effect finished; every dest
        // announces, like `play_from_hand` (design G08 §2.4).
        let outcome = game_kinds::PlayOutcome::from_i32(negated as i32);
        self.raise_core(cx, "cardPlayed", seat as i32, |t| {
            t.card = Some(id.to_string());
            t.cards = vec![id.to_string()];
            t.by_card = Some(seat as i32);
            t.play_outcome = Some(outcome);
        })?;
        Ok(())
    }

    /// Stamp the offer-time readers on the link a body runs against (design
    /// G06 §6.2). `idx` 0 is the root timing: its own declarations are the
    /// window. Called again after a join window so a joined negation / extreme
    /// shows up in the opener's body.
    fn stamp_window(&self, cx: &PlayCtx, chain: &[ChainLink], idx: usize, on_link: &mut Trigger) {
        let answered = if idx == 0 { 0 } else { chain[idx].answered };
        let declared = &chain[answered].link.answers_declared;
        on_link.answers_declared = declared.clone();
        on_link.pending_negation = declared
            .iter()
            .filter(|&&d| {
                let n = &chain[d as usize];
                self.ruleset
                    .entry_effect(n.idx, &n.link, None)
                    .contains(card_sdk::abi::AnswerEffect::NEGATES)
            })
            .count() as i32;
        on_link.extreme_armed = (cx.world().turn.extreme != game_core::engine::Extreme::Plain)
            || declared.iter().any(|&d| {
                let n = &chain[d as usize];
                self.ruleset
                    .entry_effect(n.idx, &n.link, None)
                    .contains(card_sdk::abi::AnswerEffect::ARMS_EXTREME)
            });
        on_link.answered_kinds.clear();
        let mut walk = vec![idx];
        let mut cur = idx;
        while cur != 0 {
            cur = chain[cur].link.answers as usize;
            walk.push(cur);
        }
        on_link.answered_kinds = walk.into_iter().rev().map(|i| chain[i].link.kind).collect();
    }

    /// Park (or refresh) the join round's progress beside the hook list, so a
    /// nested suspension resumes it (design G06 §6.4).
    fn park_join(
        &self,
        cx: &mut PlayCtx,
        trigger: &Trigger,
        join: JoinResume,
        chain: &Box<Vec<ChainLink>>,
    ) {
        let mut r = take_resume_or_fresh(cx, trigger);
        r.join = Some(join);
        // A raise inside the joiner's body adopts this resume and continues
        // `hand`; it needs the tree. The live box stays owned by the join walk
        // (its cursor address must not move), so hand over a copy -- the
        // resume's write-back puts the settled tree back through the cursor.
        if r.chain.is_none() {
            r.chain = Some(Box::new((**chain).clone()));
        }
        cx.set_counteract_resume(Box::new(r));
    }

    /// Build (once) and settle the join round on `chain[at]`, then close the
    /// window: the answered link aims at its real recipient again and `opened`
    /// is set for the opener's report (design G06 §6.4). The round's nodes
    /// settle through [`Self::settle_node`], so logging, the negation check and
    /// the spent-card tail are the ordinary ones.
    fn settle_join(
        &self,
        cx: &mut PlayCtx,
        chain: &mut Box<Vec<ChainLink>>,
        at: usize,
        node: usize,
        mut resume: JoinResume,
        trigger: &mut Trigger,
    ) -> Flow<()> {
        if resume.order.is_empty() && resume.next == 0 {
            let seats = chain[at]
                .link
                .join
                .as_ref()
                .map(|j| j.seats.clone())
                .unwrap_or_default();
            // A declaration on this link made *ahead* of the window is a
            // joiner too (「如同自身的对应目标被指定一般」, design G06 §6.4):
            // it answered before the block existed on the link, so its
            // declaration mark was never written at declaration time. Mark it
            // now, before the window reads the marks for its count / fallback.
            // It may already be settled: the walk resolves a round's counters
            // before the opener's body (`open`) runs, and the report counts a
            // declaration 「打出过」 whether or not the counter still settles
            // here.
            let ahead_nodes: Vec<usize> = (1..chain.len())
                .filter(|&i| i != node && chain[i].answered == at)
                .collect();
            let ahead: Vec<usize> = ahead_nodes
                .iter()
                .filter_map(|&i| {
                    let seat = chain[i].seat as i32;
                    chain[at]
                        .link
                        .join
                        .as_ref()?
                        .seats
                        .iter()
                        .position(|&s| s == seat)
                })
                .collect();
            if let Some(j) = &mut chain[at].link.join {
                for k in ahead {
                    j.declared[k] = true;
                }
            }
            // An ahead-declared counter that is itself a `SELF_REDIRECT` card
            // opened its own block at declaration (no window existed yet). It
            // is a joiner of *this* window now, so drop the block: otherwise
            // settling it would open a second, overlapping window over the
            // same link (design G06 §6.4).
            for &i in &ahead_nodes {
                chain[i].join = None;
                chain[i].joined_in_window = true;
            }
            // Seats that already declared on the link ahead of the window are
            // joiners: do not offer them again (「不重复提供同一座位」).
            let offered: Vec<i32> = {
                let j = chain[at].link.join.as_ref();
                seats
                    .iter()
                    .copied()
                    .filter(|s| {
                        let k = j
                            .and_then(|j| j.seats.iter().position(|x| x == s))
                            .unwrap_or(usize::MAX);
                        !j.is_some_and(|j| j.declared.get(k).copied().unwrap_or(false))
                    })
                    .collect()
            };
            // Nobody left to offer: no block at all, or every seat already
            // declared ahead of the window. A round over an empty seat list
            // would divide by zero in `Priority::answered`, so skip it and go
            // straight to the declared joiners.
            if offered.is_empty() {
                if let Some(j) = &mut chain[at].link.join {
                    j.opened = true;
                }
                trigger.join = chain[at].link.join.clone();
                return Ok(());
            }
            // One hop deeper than the opener (`answered_kinds`' count).
            let mut depth = 1u32;
            let mut cur = node;
            while cur != 0 {
                cur = chain[cur].link.answers as usize;
                depth += 1;
            }
            let n = cx.state().players.len() as u32;
            let mut budget = (MAX_COUNTERACT_DEPTH + 1) * n * (MAX_COUNTERACT_PER_VISIT + 1) + 8;
            let start = chain.len();
            self.build_round(cx, chain, at, depth, &mut budget, Some(&offered))?;
            // The report lives on the opener's own link: take the declared /
            // shielding marks the round wrote.
            trigger.join = chain[at].link.join.clone();
            // Every joiner node -- the seats declared ahead of the window
            // included -- post-order, newest first (LIFO). Only nodes below
            // the opener are still unsettled (the walk settles newest first);
            // a declaration made after it already ran as a plain counter.
            let mut order = Vec::new();
            let joiners: Vec<usize> = (0..node)
                .chain(start..chain.len())
                .filter(|&i| {
                    i > 0
                        && !chain[i].settled
                        && chain[i].answered == at
                        && seats.contains(&(chain[i].seat as i32))
                })
                .collect();
            for &child in joiners.iter().rev() {
                collect_resolve_order(chain, child, &mut order);
            }
            resume.order = order;
        }
        while resume.next < resume.order.len() {
            let idx = resume.order[resume.next];
            self.park_join(
                cx,
                trigger,
                JoinResume {
                    at,
                    saved_target: resume.saved_target,
                    order: resume.order.clone(),
                    next: resume.next + 1,
                },
                chain,
            );
            self.settle_node(cx, trigger, chain, idx, &resume.order, resume.next)?;
            resume.next += 1;
        }
        chain[at].link.target = resume.saved_target;
        if let Some(j) = &mut chain[at].link.join {
            j.opened = true;
        }
        trigger.join = chain[at].link.join.clone();
        Ok(())
    }

    /// The throwaway `Run` a residual wasm guard probe starts up: a shared
    /// handle to the live world (fix B -- the probe is a pure check; its body
    /// copies only if it writes), the answered link as the trigger. Built
    /// **only** for a candidate whose condition already admitted and whose
    /// entry still carries a guard (G4 deleted the rest).
    fn probe_run(&self, cx: &PlayCtx, top: &Trigger, card: &str) -> Run {
        Run {
            world: cx.share_world(),
            pass_base: Default::default(),
            overlay_floor: 0,
            drive_id: 0,
            pile_checkpoints: Default::default(),
            data: self.data.clone(),
            props: self.modules_props(),
            trigger: top.clone(),
            fig: None,
            ledger_exec: 0,
            activation_player: top.player_id,
            current_card: card.to_string(),
            current_uid: -1,
            carried_owner: None,
            dest: DEST_UNSET,
            dest_to: None,
            dest_cause: None,
            reinserted: vec![],
            paid_log: vec![],
            discard_log: vec![],
            draw_log: vec![],
            draw_after_log: vec![],
            frames_at_drive_start: vec![],
            adopted: vec![],
            hand_log: vec![],
            reshuffle_log: vec![],
            marker_spent_log: vec![],
            marker_changed_log: vec![],
            house_log: vec![],
            counter_log: vec![],
            named_raise_log: vec![],
            state_log: vec![],
            doubled: -1,
            linger_props: Default::default(),
            wrap_effect: false,
            live: None,
            posted: vec![],
            log_index: 0,
            place_log: vec![],
            place_index: 0,
            logged: 0,
            silent_body: false,
            quiet_when_empty: false,
            pending_header: None,
            own_activation: -1,
            header_ids: vec![],
            prompted: false,
            parent: -1,
            flash_policy: card_sdk::abi::Flash::Auto,
            flash_cued: false,
            flash_ordinal: 0,
        }
    }

    /// Only sources explicitly opting into the shared declaration menu: a
    /// face-up instance needs `COUNTERACT_FROM_FIELD`, and a face-down one
    /// (normally inert) is offered when the seat controls a facing
    /// `COUNTERACT_FACE_DOWN` source (「对帅气的憧憬」(3) -- 「可在符合条件时
    /// 将其直接打出」).
    fn has_field_counter(&self, cx: &PlayCtx, seat: usize, top: &Trigger) -> bool {
        if cx.world().out(seat) {
            return false;
        }
        let field = cx.world().field_instances(seat as i32);
        let face_down_ok = self.face_down_counter_ok(cx, seat);
        field.iter().any(|(uid, id)| {
            // A marked instance offers its effective card's counteraction.
            let eff = surface_card(cx.world(), *uid, id);
            let Some(idx) = self.ruleset.card(&eff) else {
                return false;
            };
            let offered = if cx.world().is_face_down_at(*uid) {
                face_down_ok
            } else {
                self.field_counter_prop(idx)
            };
            offered && self.counteracts_to_play(idx, top)
        })
    }

    /// Does `seat` control a face-up source granting direct play of face-down
    /// field cards ([`card_sdk::abi::prop::COUNTERACT_FACE_DOWN`])?
    fn face_down_counter_ok(&self, cx: &PlayCtx, seat: usize) -> bool {
        let field = cx.world().field_instances(seat as i32);
        field.iter().any(|(uid, id)| {
            !cx.world().is_face_down_at(*uid)
                && self.face_down_counter_prop(&surface_card(cx.world(), *uid, id))
        })
    }

    /// The [`card_sdk::abi::prop::COUNTERACT_FROM_FIELD`] opt-in of `idx`.
    fn field_counter_prop(&self, idx: i32) -> bool {
        self.ruleset.cards()[idx as usize]
            .props
            .get(card_sdk::abi::prop::COUNTERACT_FROM_FIELD)
            .copied()
            .unwrap_or(0)
            != 0
    }

    /// The [`card_sdk::abi::prop::COUNTERACT_FACE_DOWN`] grant of `id`.
    fn face_down_counter_prop(&self, id: &str) -> bool {
        self.ruleset.card(id).is_some_and(|idx| {
            self.ruleset.cards()[idx as usize]
                .props
                .get(card_sdk::abi::prop::COUNTERACT_FACE_DOWN)
                .copied()
                .unwrap_or(0)
                != 0
        })
    }

    /// Does the play `top` names declare recipients (`DESIGNATES`)? Those are
    /// the 「被…效果影响」 targets of the play as one timing (CROSS-TESTS C13),
    /// visible before the body runs.
    fn counteracts_to_play(&self, idx: i32, top: &Trigger) -> bool {
        self.ruleset.counteracts_to_play(idx, top)
    }

    /// The [`On::Counteract`] entry that answers `top` for `seat` (see
    /// [`Self::counteracts_to_play`] for the Effect-on-DESIGNATES-play rule).
    /// `(card, entry)`: a copy source resolves to its own card.
    fn counteract_entry(
        &self,
        idx: i32,
        top: &Trigger,
        source: Option<&str>,
    ) -> Option<(i32, i32)> {
        self.ruleset.counteract_entry(idx, top, source)
    }

    /// The most recent other-seat, non-placing play (the copy source).
    fn copy_source(cx: &PlayCtx, seat: usize) -> Option<String> {
        cx.world()
            .played_cards
            .iter()
            .rev()
            .find(|p| p.player != seat as i32 && !p.placed)
            .map(|p| p.card.clone())
    }

    /// Valid-option-first pre-scan (docs/GUARDS.md §4.5): could **any** seat
    /// respond to `top` at all? Cheap static index first -- the per-card kind
    /// bitmask and seat eligibility -- then the compiled conditions against a
    /// window context built straight from the live world. The residual wasm
    /// guards are not run here: a condition survivor is enough to open the
    /// cycle, and a card the guard would reject simply contributes no option
    /// (exactly as today's quiet lap).
    ///
    /// When this returns `false`, `hand_counteractions` skips the whole
    /// window -- no chain clone, no `Priority`, no per-visit offers, no world
    /// copies. During the scan nothing mutates the world, so one shared CEL
    /// scope answers every condition (and is only built if some candidate
    /// declares one).
    fn any_counter_candidate(&self, cx: &PlayCtx, top: &Trigger) -> bool {
        let n = cx.state().players.len();
        let live = LiveSnap {
            world: cx.world(),
            data: &self.data,
            trigger: top,
            cand: None,
            pending: None,
            rules: Some(self),
        };
        let mut scope: Option<WindowScope> = None;
        for s in 0..n {
            if self.has_field_counter(cx, s, top) {
                return true;
            }
            if !can_counteract_now(cx, s) {
                continue;
            }
            let mut seen: Vec<&str> = Vec::new();
            for id in hand_of(cx, s) {
                if seen.contains(&id.as_str()) {
                    continue;
                }
                seen.push(id.as_str());
                let Some(idx) = self.ruleset.card(id) else {
                    continue;
                };
                // Cached counteraction index (BOT-RESEARCH.md #1): one shift of
                // the per-card kind bitmask -- no entry-table scan, no `Run`.
                if std::env::var("DBG_OFFER").is_ok() {
                    eprintln!(
                        "CAND seat={s} card={id} kind={:?} tile={} pos={} subj={:?} ctp={} entry={:?} lw={:?} target={} tgt_ow={:?} effs={:?}",
                        top.kind,
                        top.tile,
                        top.position,
                        top.subject_card,
                        self.counteracts_to_play(idx, top),
                        self.counteract_entry(idx, top, Self::copy_source(cx, s).as_deref()),
                        cx.state().players.get(s).map(|p| p.state_get("lastWalk")),
                        top.target,
                        cx.world().st.owners.get(top.tile.max(0) as usize),
                        top.effects.iter().map(|e| (&e.kind, e.target)).collect::<Vec<_>>()
                    );
                }
                if !self.counteracts_to_play(idx, top) {
                    continue;
                }
                let Some((card, entry)) =
                    self.counteract_entry(idx, top, Self::copy_source(cx, s).as_deref())
                else {
                    continue;
                };
                let Some(condition) = self.ruleset.condition(card, entry) else {
                    // No condition: the card always offers (subject to the
                    // guard, which the cycle itself asks).
                    return true;
                };
                let cand = crate::condition::fill_candidate(
                    &live,
                    s as i32,
                    &self.ruleset.cards()[card as usize].id,
                    false,
                );
                let sc = scope.get_or_insert_with(|| {
                    crate::condition::window_scope_owned(crate::condition::fill_window(&live))
                });
                if crate::condition::condition_allows(Some(condition), Some(sc), &cand) {
                    return true;
                }
            }
        }
        false
    }

    /// Offer one player a counteraction window answering `top`. Returns the card they
    /// declared, if any. The prompt describes the **answered** link, so a
    /// player answering a counter is told which counter they are answecycle.
    fn declare_one(
        &self,
        cx: &mut PlayCtx,
        s: usize,
        chain: &[ChainLink],
        timing: usize,
        memo: &mut ProbeMemo,
        groups: &[i32],
    ) -> Flow<Option<(String, i32, i32, i32, Vec<u8>)>> {
        let top = &chain[timing].link;
        if std::env::var("DBG_OFFER").is_ok() {
            eprintln!(
                "DECLARE_ONE seat={s} kind={:?} tile={} pos={} card={:?} target={} designates={} hand={:?}",
                top.kind,
                top.tile,
                top.position,
                top.card,
                top.target,
                self.ruleset.trigger_designates(top),
                hand_of(cx, s)
            );
        }
        // Hand cards that answer this link, deduped to one entry per id.
        // Ordered by `effect_order_key` (Q5): group `Hand`, source = the card's
        // index in the hand `Vec` (the authoritative state list), decl = 0 (one
        // entry per card after the dedupe). The sort is stable and the source
        // component *is* today's hand order, so the offer list is unchanged --
        // the key just makes the guarantee explicit.
        //
        // Stage 1 is the cheap static index (BOT-RESEARCH.md #1): the per-card
        // kind bitmask, no world copy, no scope, no `Run`. A hand with nothing
        // that answers this kind returns before any per-window machinery
        // exists -- the common case for most seats of most raises.
        let mut candidates: Vec<((u8, u32, u32), String, i32, i32)> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for (hand_pos, id) in hand_of(cx, s)
            .iter()
            .enumerate()
            .filter(|_| can_counteract_now(cx, s))
        {
            if seen.contains(id) {
                continue;
            }
            seen.push(id.clone());
            let Some(idx) = self.ruleset.card(id) else {
                continue;
            };
            if !self.counteracts_to_play(idx, top) {
                continue;
            }
            candidates.push((
                effect_order_key(LookupGroup::Hand, hand_pos as u32, 0),
                id.clone(),
                idx,
                -1,
            ));
        }
        if !cx.world().out(s) {
            let face_down_ok = self.face_down_counter_ok(cx, s);
            for (field_pos, (uid, raw)) in
                cx.world().field_instances(s as i32).into_iter().enumerate()
            {
                // A marked instance offers its effective card's counteraction.
                let id = surface_card(cx.world(), uid, &raw);
                let Some(idx) = self.ruleset.card(&id) else {
                    continue;
                };
                // A face-down instance is normally inert; it is offered only
                // for a face-down-granting seat (`COUNTERACT_FACE_DOWN`). The
                // face-up half keeps the `COUNTERACT_FROM_FIELD` opt-in.
                let offered = if cx.world().is_face_down_at(uid) {
                    face_down_ok
                } else {
                    self.field_counter_prop(idx)
                };
                if !offered || !self.counteracts_to_play(idx, top) {
                    continue;
                }
                candidates.push((
                    effect_order_key(source_group(cx.world(), uid), field_pos as u32, 0),
                    id,
                    idx,
                    uid,
                ));
            }
        }
        candidates.retain(|(_, _, idx, _)| {
            let group = self.ruleset.cards()[*idx as usize]
                .props
                .get(card_sdk::abi::prop::COUNTERACT_GROUP)
                .copied()
                .unwrap_or(0);
            group == 0 || !groups.contains(&group)
        });
        if candidates.is_empty() {
            return Ok(None);
        }

        // `BGD_COUNTERACT_SLOW=1` reproduces the pre-optimisation shape
        // exactly (docs/GUARDS.md §4.5): a world copy + `Run` per offer for the
        // window context, and a `Run` per condition survivor for the guard --
        // the `examples/ckpt_equiv.rs` A/B's this against the fast path.
        if counteract_slow_path() {
            let win_run = Run {
                world: cx.share_world(),
                pass_base: Default::default(),
                overlay_floor: 0,
                drive_id: 0,
                pile_checkpoints: Default::default(),
                data: self.data.clone(),
                props: self.modules_props(),
                trigger: top.clone(),
                fig: None,
                ledger_exec: 0,
                activation_player: top.player_id,
                current_card: String::new(),
                current_uid: -1,
                carried_owner: None,
                dest: DEST_UNSET,
                dest_to: None,
                dest_cause: None,
                reinserted: vec![],
                paid_log: vec![],
                discard_log: vec![],
                draw_log: vec![],
                draw_after_log: vec![],
                frames_at_drive_start: vec![],
                adopted: vec![],
                hand_log: vec![],
                reshuffle_log: vec![],
                marker_spent_log: vec![],
                marker_changed_log: vec![],
                house_log: vec![],
                counter_log: vec![],
                named_raise_log: vec![],
                state_log: vec![],
                doubled: -1,
                linger_props: Default::default(),
                wrap_effect: false,
                live: None,
                posted: vec![],
                log_index: 0,
                place_log: vec![],
                place_index: 0,
                logged: 0,
                silent_body: false,
                quiet_when_empty: false,
                pending_header: None,
                own_activation: -1,
                header_ids: vec![],
                prompted: false,
                parent: -1,
                flash_policy: card_sdk::abi::Flash::Auto,
                flash_cued: false,
                flash_ordinal: 0,
            };
            let win_scope =
                crate::condition::window_scope_owned(crate::condition::fill_window(&win_run));
            let mut options: Vec<(String, i32, i32)> = Vec::new();
            let mut order: Vec<(u8, u32, u32)> = Vec::new();
            for (key, id, idx, uid) in candidates {
                if self
                    .ruleset
                    .counteract_pre_allows(&win_run, idx, s as i32, &win_scope)
                    .is_none()
                {
                    continue;
                }
                let Some((idx, _)) =
                    self.counteract_entry(idx, top, Self::copy_source(cx, s).as_deref())
                else {
                    continue;
                };
                let card_id = self.ruleset.cards()[idx as usize].id.clone();
                let mut run = self.probe_run(cx, top, &card_id);
                run.current_uid = uid;
                if self
                    .ruleset
                    .can_counteract_scoped(&run, idx, s as i32, &win_scope)
                    .unwrap_or(false)
                {
                    order.push(key);
                    options.push((id, idx, uid));
                }
            }
            return self.finish_offer(cx, s, top, order, options);
        }

        // Stage 2: the compiled conditions against a window context built
        // straight from the live world (`LiveSnap`, no clone). The CEL scope is
        // built lazily and only if some candidate actually declares a condition
        // (docs/GUARDS.md §4.4 item 1); it is shared across the whole cycle via
        // `memo` and only rebuilt when a probe reads `hand(p)` after a
        // declaration.
        let live = LiveSnap {
            world: cx.world(),
            data: &self.data,
            trigger: top,
            cand: None,
            pending: Some((chain, timing)),
            rules: Some(self),
        };
        let mut options: Vec<(String, i32, i32)> = Vec::new();
        let mut order: Vec<(u8, u32, u32)> = Vec::new();
        for (key, id, idx, uid) in candidates {
            #[cfg(feature = "bot-cost")]
            crate::host::bot_cost::COUNTERACT_PROBES
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let Some((idx, entry)) =
                self.counteract_entry(idx, top, Self::copy_source(cx, s).as_deref())
            else {
                continue;
            };
            // A copied entry runs the source card's module.
            let card_id = self.ruleset.cards()[idx as usize].id.clone();
            let condition = self.ruleset.condition(idx, entry);
            let guard_is_none = self.ruleset.entry_guard_is_none(idx, entry);
            // A verdict is hand-sensitive when the residual wasm guard may read
            // hands (it can read anything) or the condition names a hand field.
            // Only those are stamped with `hand_gen`; the rest are stable for
            // the whole window (a cycle's only world change is a declaration).
            let hand_sensitive = !guard_is_none
                || condition.is_some_and(|p| crate::condition::cond_reads_hand(&p.cond));
            if let Some(&(gen, eligible, hs)) = memo.verdicts.get(&(s, id.clone())) {
                if !counteract_slow_path() && (!hs || gen == memo.hand_gen) {
                    #[cfg(feature = "bot-cost")]
                    crate::host::bot_cost::COUNTERACT_PROBE_MEMO_HITS
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if eligible {
                        order.push(key);
                        options.push((id, idx, uid));
                    }
                    continue;
                }
            }
            let eligible = match condition {
                None => {
                    if guard_is_none {
                        // No condition, no guard: always answers this kind.
                        true
                    } else {
                        // No condition but a residual guard: the only path that
                        // still needs a `Run` (the guard's wasm store).
                        let mut run = self.probe_run(cx, top, &card_id);
                        run.current_uid = uid;
                        let scope = memo.scope_for(&live, true);
                        self.ruleset
                            .can_counteract_scoped(&run, idx, s as i32, scope)
                            .unwrap_or(false)
                    }
                }
                Some(condition) => {
                    let need_fresh = crate::condition::cond_reads_window_hand(&condition.cond);
                    let scope = memo.scope_for(&live, need_fresh);
                    let cand = crate::condition::fill_candidate(&live, s as i32, &card_id, false);
                    let allows =
                        crate::condition::condition_allows(Some(condition), Some(scope), &cand);
                    if std::env::var("DBG_OFFER").is_ok() && id.contains("普通") {
                        eprintln!(
                            "ALLOWS seat={s} kind={:?} allows={} effs={:?}",
                            top.kind,
                            allows,
                            top.effects
                                .iter()
                                .map(|e| (&e.kind, e.target))
                                .collect::<Vec<_>>()
                        );
                    }
                    if !allows {
                        false
                    } else if guard_is_none {
                        // G4: the condition alone decides -- no `Run`, no world
                        // copy, no wasm instantiation.
                        true
                    } else {
                        let mut run = self.probe_run(cx, top, &card_id);
                        run.current_uid = uid;
                        self.ruleset
                            .can_counteract_scoped(&run, idx, s as i32, scope)
                            .unwrap_or(false)
                    }
                }
            };
            memo.verdicts
                .insert((s, id.clone()), (memo.hand_gen, eligible, hand_sensitive));
            if eligible {
                order.push(key);
                options.push((id, idx, uid));
            }
        }
        self.finish_offer(cx, s, top, order, options)
    }

    /// The prompt half of [`Self::declare_one`]: order the offer list by the
    /// Q5 key, show the counteraction prompt (options + 「不打」), and return the card
    /// the seat declared (if any).
    fn finish_offer(
        &self,
        cx: &mut PlayCtx,
        s: usize,
        top: &Trigger,
        order: Vec<(u8, u32, u32)>,
        options: Vec<(String, i32, i32)>,
    ) -> Flow<Option<(String, i32, i32, i32, Vec<u8>)>> {
        if options.is_empty() {
            return Ok(None);
        }
        // Q5: order the offer list by the explicit key. Stable, and the key's
        // source component is the hand position, so this is today's order.
        let mut keyed: Vec<((u8, u32, u32), (String, i32, i32))> =
            order.into_iter().zip(options).collect();
        keyed.sort_by_key(|(k, _)| *k);
        let mut options: Vec<(String, i32, i32)> = keyed.into_iter().map(|(_, o)| o).collect();
        let shared_move = top.kind == TriggerKind::MoveBefore
            && options.iter().any(|(_, idx, _)| {
                self.ruleset.cards()[*idx as usize]
                    .props
                    .get(card_sdk::abi::prop::COUNTERACT_GROUP)
                    .copied()
                    .unwrap_or(0)
                    > 0
            });
        if shared_move {
            // Payment order: dedicated card, marker pool, then back.
            options.sort_by_key(|(_, _, uid)| *uid >= 0);
        }
        // Labels "打出「...」" + "不打"; the hint is the first CounteractHint or
        // the trigger's description. CounteractHint is not in the ABI yet (TODO).
        let mut labels: Vec<Msg> = options
            .iter()
            .map(|(id, idx, uid)| {
                if *uid >= 0 {
                    let cost = self.ruleset.cards()[*idx as usize]
                        .props
                        .get(card_sdk::abi::prop::COUNTERACT_CHARGE_COST)
                        .copied()
                        .unwrap_or(0);
                    Msg::new("ask.counteract.skill")
                        .card("card", id.clone())
                        .i("n", cost as i64)
                } else {
                    Msg::new("ask.counteract.play").card("card", id.clone())
                }
            })
            .collect();
        if shared_move {
            let landing = |extra: i32| {
                let total = top.mv.as_ref().map(|m| m.total).unwrap_or(0);
                let dir = top.mv.as_ref().map(|m| m.dir.as_i32()).unwrap_or(1);
                (top.tile + (total + extra) * dir).rem_euclid(self.data.tiles.len() as i32)
            };
            loop {
                // No declaration or resource change until the payment is confirmed.
                // Keep the source metadata for the existing bot counteraction policy.
                let distances = vec![
                    Msg::new("ask.counteract.extend")
                        .i("n", 2)
                        .tile("tile", landing(2))
                        .card("card", options[0].0.clone()),
                    Msg::new("ask.counteract.extend")
                        .i("n", 1)
                        .tile("tile", landing(1))
                        .card("card", options[0].0.clone()),
                    Msg::new("ask.counteract.skip"),
                ];
                let ask = Ask::choice(
                    vec![s],
                    Msg::new("ask.counteract.title"),
                    Msg::new("ask.counteract.move_extension")
                        .i("n", top.mv.as_ref().map(|m| m.total).unwrap_or(0) as i64)
                        .tile("tile", landing(0)),
                    distances,
                    2,
                    12.0,
                );
                let reply = cx.ask(ask)?;
                let pick = reply
                    .answered
                    .answers
                    .first()
                    .copied()
                    .filter(|&x| x >= 0)
                    .unwrap_or(reply.fallback);
                let extension = match pick {
                    0 => 2,
                    1 => 1,
                    _ => return Ok(None),
                };
                let mut payments = labels.clone();
                payments.push(Msg::new("ask.counteract.back"));
                let ask = Ask::choice(
                    vec![s],
                    Msg::new("ask.counteract.title"),
                    Msg::new("ask.counteract.move_extension_payment")
                        .i("n", extension as i64)
                        .tile("tile", landing(extension)),
                    payments,
                    options.len() as i32,
                    12.0,
                );
                let reply = cx.ask(ask)?;
                let pick = reply
                    .answered
                    .answers
                    .first()
                    .copied()
                    .filter(|&x| x >= 0)
                    .unwrap_or(reply.fallback) as usize;
                if let Some((id, idx, uid)) = options.get(pick) {
                    let Some(arg) = self.declare_for_card(cx, s, id, *idx, top)? else {
                        // Declined at the declare: no counter this visit.
                        return Ok(None);
                    };
                    return Ok(Some((id.clone(), *idx, *uid, extension, arg)));
                }
                // Back (also the payment timeout) reopens the distance choice.
            }
        }
        labels.push(Msg::new("ask.counteract.skip"));
        let fallback = labels.len() as i32 - 1;
        let ask = Ask::choice(
            vec![s],
            Msg::new("ask.counteract.title"),
            Msg::new("ask.counteract.text").msg("detail", describe_trigger(top)),
            labels,
            fallback,
            12.0,
        );
        let reply = cx.ask(ask)?;
        let pick = reply
            .answered
            .answers
            .first()
            .copied()
            .filter(|&x| x >= 0)
            .unwrap_or(reply.fallback) as usize;
        Ok(match options.get(pick) {
            Some((id, idx, uid)) => {
                let Some(arg) = self.declare_for_card(cx, s, id, *idx, top)? else {
                    return Ok(None);
                };
                Some((id.clone(), *idx, *uid, 0, arg))
            }
            None => None,
        })
    }

    /// Run the chosen card's `OP_DECLARE` before the counter declaration
    /// commits (docs/ACTIVATION.md §2). One answer cursor across a multi-step
    /// declare. `Ok(None)` = declined and **no counter is declared this
    /// visit**; `Ok(Some(arg))` = the declaration carries `arg`.
    ///
    /// A **prompted** decline has already consumed its answer-log slot: the
    /// prompt's answer is recorded like any other, so a replay does not
    /// re-ask (docs/ACTIVATION.md "Bots and the answer log").
    fn declare_for_card(
        &self,
        cx: &mut PlayCtx,
        s: usize,
        id: &str,
        idx: i32,
        top: &Trigger,
    ) -> Flow<Option<Vec<u8>>> {
        // The declared entry is the [反击] at this trigger -- not the card's
        // `On::Play`. A declare on the counteract entry is what `finish_offer`
        // is collecting (docs/ACTIVATION.md §2).
        let Some((card, entry)) =
            self.counteract_entry(idx, top, Self::copy_source(cx, s).as_deref())
        else {
            return Ok(Some(Vec::new()));
        };
        let src_id = self.ruleset.cards()[card as usize].id.clone();
        let mut answers: Vec<i32> = Vec::new();
        loop {
            let run = self.declare_run(cx, s, &src_id, card, top);
            match self
                .ruleset
                .declare_entry(&run, card, entry, s as i32, &answers)
            {
                Ok(DeclareOut::Arg(a)) => return Ok(Some(a)),
                Ok(DeclareOut::Declined) => return Ok(None),
                Ok(DeclareOut::Paused(p)) => {
                    let ask = prompt_to_ask(p);
                    let reply = cx.ask(ask)?;
                    let a = reply
                        .answered
                        .answers
                        .first()
                        .copied()
                        .unwrap_or(reply.fallback);
                    answers.push(a);
                }
                // A collector that traps must not run the effect on a made-up
                // arg (`Prompt`'s `#[must_use]` antipattern). Decline, and say
                // reason: the same policy a trapping *body* gets -- log
                // `log.card_trap`, do not take the match down. `HaltKind` has
                // no trap arm, so this cannot be a `Flow::Err`.
                Err(e) => {
                    cx.log(
                        s as i32,
                        Msg::new("log.card_trap")
                            .card("card", id)
                            .text("detail", crate::host::trap_detail(&e)),
                    );
                    return Ok(None);
                }
            }
        }
    }

    /// A `Run` bound to `id` on seat `s`, for a declare probe (no writes are
    /// committed -- see [`Self::cant_play`]). Distinct from [`Self::probe_run`],
    /// which binds to the trigger's player.
    ///
    /// `top` is the trigger the declaration is answering: a [反击]'s collector
    /// reads `trigger::*` (`value` / `target` / `tile` / `move.roll`) against
    /// the real trigger, not a default. The play path passes a default -- a
    /// play answers no trigger.
    fn declare_run(&self, cx: &PlayCtx, s: usize, id: &str, _idx: i32, top: &Trigger) -> Run {
        // A marked instance answers for its effective card too, so the probe
        // binds the instance whose surface is `id`.
        let uid = cx
            .world()
            .field_instances(s as i32)
            .into_iter()
            .find(|(uid, c)| surface_card(cx.world(), *uid, c) == id)
            .map_or(-1, |(uid, _)| uid);
        Run {
            world: cx.share_world(),
            pass_base: Default::default(),
            overlay_floor: 0,
            drive_id: 0,
            pile_checkpoints: Default::default(),
            data: self.data.clone(),
            props: self.modules_props(),
            trigger: top.clone(),
            fig: None,
            ledger_exec: 0,
            activation_player: s as i32,
            current_card: id.to_string(),
            current_uid: uid,
            carried_owner: None,
            dest: DEST_UNSET,
            dest_to: None,
            dest_cause: None,
            reinserted: vec![],
            paid_log: vec![],
            discard_log: vec![],
            draw_log: vec![],
            draw_after_log: vec![],
            frames_at_drive_start: vec![],
            adopted: vec![],
            hand_log: vec![],
            reshuffle_log: vec![],
            marker_spent_log: vec![],
            marker_changed_log: vec![],
            house_log: vec![],
            counter_log: vec![],
            named_raise_log: vec![],
            state_log: vec![],
            doubled: -1,
            linger_props: Default::default(),
            wrap_effect: false,
            live: None,
            posted: vec![],
            log_index: 0,
            place_log: vec![],
            place_index: 0,
            logged: 0,
            silent_body: false,
            quiet_when_empty: false,
            pending_header: None,
            own_activation: -1,
            header_ids: vec![],
            prompted: false,
            parent: -1,
            flash_policy: card_sdk::abi::Flash::Auto,
            flash_cued: false,
            flash_ordinal: 0,
        }
    }
}

fn bridge_trigger(t: &CoreTrigger) -> Trigger {
    let is_roll_kind = matches!(
        trigger_kind(t.kind.as_str()),
        TriggerKind::DiceRollResult
            | TriggerKind::DiceRollBefore
            | TriggerKind::DiceRollAfter
            | TriggerKind::DiceRollValue
            | TriggerKind::DiceRollCast
            | TriggerKind::DiceRollAdd
            | TriggerKind::DiceRollSettled
    );
    // Only a move trigger carries a roll; `paid`/`settle` carry value as an
    // amount, which must not read as a phantom move. `DiceRollAfter` is the
    // post-roll hook and does carry the face (「若移动掷骰出目为16及以上」).
    // A card-driven `roll` raise has no move payload (`mv: None`); its face
    // remains on `value` and the writeback reads it from there.
    let mv = t.mv.as_ref().map(|m| TriggerMove {
        kind: m.kind,
        settles: m.settles,
        tags: m.tags.clone(),
        // Mirror of `Trigger::main`.
        is_main_move: t.is_main_move,
        dir: m.dir,
        from: m.from,
        remaining: m.remaining,
        total: m.total,
        roll: is_roll_kind.then_some(t.value),
        by_skill: m.by_skill,
        settle_redirect: m.settle_redirect,
        rolled: m.rolled,
        effective: m.effective,
        path: m.path.clone(),
        abnormal: m.abnormal,
    });
    // The link's subject: the raised play's card, else the innermost resolving
    // frame (the card body this raise is part of). None from turn flow.
    let subject = t
        .card
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| resolving_snapshot().last().map(|(card, _, _)| card.clone()));
    let buy = t.buy.map(|b| TriggerBuy {
        kind: card_sdk::abi::BuyKind::from_i32(b.kind.as_i32())
            .unwrap_or(card_sdk::abi::BuyKind::Land),
        seller: b.seller,
        price: b.price,
        deal_owner: b.deal_owner,
        deal_houses: b.deal_houses,
        deal_mortgaged: b.deal_mortgaged,
    });
    Trigger {
        kind: trigger_kind(t.kind.as_str()),
        player_id: t.player_id,
        target: t.target,
        tile: t.tile,
        position: t.position,
        ordinal: t.ordinal,
        value: t.value,
        fold: t.fold,
        fold_input: t.fold_input,
        fold_milli: t.fold_milli,
        step: t.step,
        by_card: t.by_card,
        is_main_move: t.is_main_move,
        mv,
        pay: t.pay.map(|p| TriggerPay {
            kind: p.kind,
            shape: p.shape,
            is_rent: p.is_rent,
            total_stage: p.total_stage,
            split_total: p.split_total,
        }),
        buy,
        negation: t.negation,
        spared: t.spared.clone(),
        seq: t.seq,
        answers: t.answers,
        effects: t.effects.clone(),
        cards: t.cards.clone(),
        card: t.card.clone(),
        name: t.name.clone(),
        roll_source: t.roll_source,
        roll: t.roll.as_ref().map(|r| crate::world::TriggerRoll {
            id: r.id,
            purpose: r.purpose,
            source: r.source.as_i32(),
            rerolled: r.rerolled,
            chosen_index: r.chosen_index,
            unused_face: r.unused_face,
            casts: r.casts.clone(),
            final_face: r.final_face,
        }),
        reason: t.reason.clone(),
        uid: t.uid,
        instance: t.instance,
        join_redirect: t.join.as_ref().map(|j| j.opener),
        join_used: t.join.as_ref().is_some_and(|j| j.used.iter().any(|&x| x)),
        settle_tile: t.settle_tile,
        outcome: t.outcome,
        by_card_instance: t.by_card_instance,
        redirected_from: t.redirected_from,
        settle_facts: t.settle_facts,
        zone: t.zone,
        play_outcome: t.play_outcome,
        out_cause: t.out_cause,
        charge_skill: t.charge_skill,
        subject_card: subject,
        join: t.join.as_ref().map(Into::into),
        // The core trigger carries no window seat membership; a link that is a
        // joiner is flagged on its `ChainLink` and copied in at settle.
        joined_in_window: false,
        answers_declared: t.answers_declared.clone(),
        copied: t.copied.clone(),
        roll_root: t.roll_root,
        pending_negation: t.pending_negation,
        extreme_armed: t.extreme_armed,
        activation_ordinal: t.activation_ordinal,
        abnormal_blocked: t.abnormal_blocked,
        answered_kinds: t.answered_kinds.iter().map(|k| trigger_kind(k)).collect(),
    }
}

/// The reverse of [`bridge_trigger`]'s join mapping: the engine mirror of a
/// guest join window.
fn unbridge_join(j: &crate::world::TriggerJoin) -> game_core::engine::rules::JoinState {
    game_core::engine::rules::JoinState {
        opener: j.opener,
        seats: j.seats.clone(),
        declared: j.declared.clone(),
        used: j.used.clone(),
        designated: j.designated.clone(),
        opened: j.opened,
    }
}

/// The reverse of [`bridge_trigger`]'s buy mapping: a guest-side deal rewrite
/// back to the engine payload the commit reads.
fn unbridge_buy(b: TriggerBuy) -> game_core::engine::rules::TriggerBuy {
    let mut out = game_core::engine::rules::TriggerBuy::new(
        game_core::engine::purchase::BuyKind::from_i32(b.kind as i32)
            .unwrap_or(game_core::engine::purchase::BuyKind::Land),
        b.seller.as_i32(),
        b.price,
    );
    out.deal_owner = b.deal_owner;
    out.deal_houses = b.deal_houses;
    out.deal_mortgaged = b.deal_mortgaged;
    out
}

/// The reverse of [`bridge_trigger`]'s pay mapping: a guest-side kind / shape
/// rewrite back to the engine payload the money pipeline folds.
fn unbridge_pay(p: TriggerPay) -> game_core::engine::rules::TriggerPay {
    game_core::engine::rules::TriggerPay {
        kind: p.kind,
        shape: p.shape,
        is_rent: p.is_rent,
        total_stage: p.total_stage,
        split_total: p.split_total,
    }
}

/// Map a raw prompt answer to what the answer log should carry.
/// `PromptKind::TileId` answers with the **tile id** (`Ask::tile`'s
/// `items[i]`), or `-1` for the 「不选」 fallback -- stable across the
/// commit-pass re-run of a body whose options the just-committed host
/// effect reshaped. Every other kind keeps the raw index.
fn recorded_answer(kind: PromptKind, items: &[String], v: i32) -> i32 {
    if kind != PromptKind::TileId {
        return v;
    }
    if v < 0 {
        return -1;
    }
    items
        .get(v as usize)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(-1)
}

/// A module prompt -> the engine's `Ask` (the player sees the same prompt style
/// the engine shows for its own questions).
fn prompt_to_ask(p: Prompt) -> Ask {
    let players = vec![p.player_id as usize];
    let Prompt {
        kind,
        player_id,
        title,
        text,
        options,
        answer_slot: _,
        prices,
        ai_hint,
        count,
    } = p;
    match kind {
        PromptKind::YesNo => Ask::choice(
            players,
            title,
            text,
            vec![Msg::new("ask.yes"), Msg::new("ask.no")],
            1,
            12.0,
        ),
        PromptKind::Choice => {
            let labels = options
                .into_iter()
                .map(|o| match o {
                    PromptOption::Str(m) => m,
                    PromptOption::Int(i) => Msg::new("ask.intOption").i("n", i),
                    PromptOption::Tile { label, .. } => label,
                })
                .collect();
            Ask::choice(players, title, text, labels, 0, 15.0)
        }
        PromptKind::ChoiceMulti => {
            let labels: Vec<Msg> = options
                .into_iter()
                .map(|o| match o {
                    PromptOption::Str(m) => m,
                    PromptOption::Int(i) => Msg::new("ask.intOption").i("n", i),
                    PromptOption::Tile { label, .. } => label,
                })
                .collect();
            let n = labels.len().min(31) as i32;
            // Guest 0 is treated as 1; anything past the option list clamps.
            let want = count.clamp(1, n.max(1));
            let mut ask = Ask::multi(players, title, text, labels, want, 15.0);
            if ai_hint >= 0 {
                ask = ask.with_ai(|_| ai_hint);
            }
            ask
        }
        PromptKind::Money => {
            // The bound rode `count` (`ctx::ask_money` -> `opt_count`); the
            // fallback is the maximum, the answer is a raw amount. `ai_hint`
            // is the card's preferred amount when it set one.
            let max = count.max(0);
            let mut ask = Ask::money(player_id.max(0) as usize, max, title, text, max, 15.0);
            if ai_hint >= 0 {
                ask = ask.with_ai(|_| ai_hint.clamp(0, max));
            }
            ask
        }
        PromptKind::Tile | PromptKind::TileId | PromptKind::TileWalk => {
            // Bare-Int options keep the auto `ask.tileOption` labels so
            // existing `ask_tile` callers are unchanged; `PromptOption::Tile`
            // carries its own label (`ask_tiles` / `opt_tile`).
            let mut tiles: Vec<usize> = Vec::new();
            let mut labels: Vec<Msg> = Vec::new();
            for o in options {
                match o {
                    PromptOption::Int(i) => {
                        tiles.push(i as usize);
                        labels.push(Msg::new("ask.tileOption").tile("tile", i));
                    }
                    PromptOption::Str(_) => {
                        tiles.push(0);
                        labels.push(Msg::new("ask.tileOption").tile("tile", 0));
                    }
                    PromptOption::Tile { tile, label } => {
                        tiles.push(tile as usize);
                        labels.push(label);
                    }
                }
            }
            let mut ask = Ask::tile(player_id as usize, title, text, &tiles, labels);
            ask.view.prices = prices;
            ask.view.walk = kind == PromptKind::TileWalk;
            if ai_hint >= 0 {
                ask = ask.with_ai(|_| ai_hint);
            }
            ask
        }
        PromptKind::Card => {
            // Pick one card out of a list; the options are ready-made labels.
            let labels = options
                .into_iter()
                .map(|o| match o {
                    PromptOption::Str(m) => m,
                    PromptOption::Int(i) => Msg::new("ask.intOption").i("n", i),
                    PromptOption::Tile { label, .. } => label,
                })
                .collect();
            Ask::choice(players, title, text, labels, 0, 15.0)
        }
        PromptKind::Player => {
            let labels = options
                .iter()
                .map(|o| match o {
                    PromptOption::Int(s) => Msg::new("ask.player").player_id("who", *s),
                    PromptOption::Str(_) => Msg::new("ask.player").player_id("who", -1),
                    PromptOption::Tile { tile, .. } => {
                        Msg::new("ask.player").player_id("who", *tile)
                    }
                })
                .collect();
            Ask::choice(players, title, text, labels, 0, 15.0)
        }
    }
}

/// The engine's string trigger kinds -> the module's small enum.

/// The initial user of a counteraction round at the chain root (ruling 2026-10-07):
/// the player whose action or effect raised the link. That is `by_card` when a
/// card caused the trigger -- **not** the trigger's own player, which for a
/// payment is the payer: seat 2's card making seat 0 pay starts at seat 2, not
/// seat 0. A board-driven trigger (rent, buy, build, turn flow -- `by_card` is
/// `None`, docs/CARDS.md) is a system / tile event, so the link has no player
/// and the cycle starts at the active turn player. The cycle starts **at** this
/// seat (superseding clause 89's 「下一位」).
fn chain_starter(t: &CoreTrigger, turn: i32, n: usize) -> usize {
    let seat = match t.by_card {
        Some(by) if by >= 0 => by,
        _ => turn.max(0),
    };
    seat as usize % n
}

/// One node of the counteraction answer tree (see `hand_counteractions`). Index 0 is
/// the timing the chain answers (L1); every other node is one declared counter.
#[derive(Clone)]
struct ChainLink {
    /// Who declared it. For L1, the initial user of the round
    /// (`chain_starter`).
    seat: usize,
    /// The declared card's ruleset handle (-1 for L1).
    idx: i32,
    /// The declared card's id (L1: the trigger's own card).
    id: String,
    /// Field instance being activated; -1 for a hand card or the root timing.
    uid: i32,
    /// Extension chosen before declaring this movement source; zero otherwise.
    move_extension: i32,
    /// Activation argument the declaration's `OP_DECLARE` collected
    /// (docs/ACTIVATION.md §2). Stored here at declaration time; `resolve_jobs`
    /// reads it back and never re-enters `OP_DECLARE`.
    arg: Vec<u8>,
    /// Node index of the timing this answers.
    answered: usize,
    /// The join window this counter opens (`SELF_REDIRECT`), installed on the
    /// answered link for its body only (design G06 §6.4). `None` when the
    /// declaration was already inside another window (`joined_in_window`).
    join: Option<crate::world::TriggerJoin>,
    /// Declared inside an open join window: a joiner, not an opener.
    joined_in_window: bool,
    /// The window block that was on the answered link before this node's own
    /// one went in; put back when the body returns.
    saved_join: Option<crate::world::TriggerJoin>,
    /// This node's own link -- what its answers read and rewrite.
    link: Trigger,
    /// Nodes that answered this one, in declaration order.
    answers: Vec<usize>,
    /// Already settled (a join round must not settle it again).
    settled: bool,
}

/// Post-order walk of the answer tree (newest-first children) -- the order
/// `resolve_rounds` settles nodes. Index 0 (L1) is last.
fn collect_resolve_order(chain: &[ChainLink], idx: usize, out: &mut Vec<usize>) {
    for &child in chain[idx].answers.iter().rev() {
        collect_resolve_order(chain, child, out);
    }
    if idx != 0 {
        out.push(idx);
    }
}

/// The nearest link at or above `idx` whose kind carries a roll face, or
/// [`ROLL_ROOT_NONE`]. A counter body's face write propagates to it (§G06
/// 6.3), so an additive counter composed on a roll root lands on the root.
fn nearest_roll_link(chain: &[ChainLink], mut idx: usize) -> u32 {
    loop {
        if matches!(
            chain[idx].link.kind,
            TriggerKind::DiceRollResult
                | TriggerKind::DiceRollBefore
                | TriggerKind::DiceRollAfter
                | TriggerKind::DiceRollValue
                | TriggerKind::DiceRollCast
                | TriggerKind::DiceRollAdd
                | TriggerKind::DiceRollSettled
        ) {
            return idx as u32;
        }
        if idx == 0 {
            return ROLL_ROOT_NONE;
        }
        idx = chain[idx].link.answers as usize;
    }
}

/// A counter body's face write (`.mv.roll`, else `value`) composes onto the
/// enclosing roll link, when there is one and the face moved (design G06
/// §6.3). Inner answers settle first, so an outer additive body already reads
/// the composed face; the root's final value is what the engine commits.
fn propagate_face(chain: &mut [ChainLink], on_link: &Trigger, before: i32) {
    let after = on_link
        .mv
        .as_ref()
        .and_then(|m| m.roll)
        .unwrap_or(on_link.value);
    let root = on_link.roll_root;
    if after == before || root == crate::world::ROLL_ROOT_NONE {
        return;
    }
    let Some(rl) = chain.get_mut(root as usize) else {
        return;
    };
    if let Some(m) = &mut rl.link.mv {
        m.roll = on_link.mv.as_ref().and_then(|m| m.roll);
    }
    rl.link.value = on_link.value;
}

/// Who is asked next during a round on one timing (see `hand_counteractions`).
///
/// Ruling 2026-10-07 (supersedes clause 89's 「下一位」 start and the
/// 2026-10-06 one-activation-per-visit cap): the cycle starts **at** the initial
/// user and runs forward in turn order; a visit holds the floor until the seat
/// passes or runs out of eligible counteractions (`build_round`). The round
/// closes at the end of a **lap** -- one full cycle of all seats starting from
/// the initial user -- that brings no new declaration (「所有玩家同意…已发动
/// 后」). A lap that carried a declaration never closes the round, even if
/// every seat passed after it; the next full lap must be quiet.
struct Priority {
    n: usize,
    cursor: usize,
    /// Visits completed in the current lap (0..n). A lap is one full cycle of
    /// the cycle from the initial user.
    in_lap: usize,
    /// Did any visit in the current lap declare a counter?
    lap_carried: bool,
    /// When set, only these seats are visited, in list order (a join round).
    over: Option<Vec<i32>>,
}

impl Priority {
    /// A round on `initial`'s timing: starts at the initial user's seat.
    fn new(initial: usize, n: usize) -> Self {
        Self {
            n,
            cursor: initial % n,
            in_lap: 0,
            lap_carried: false,
            over: None,
        }
    }

    /// A round over exactly `seats`, in list order (a join window).
    fn over(seats: &[i32]) -> Self {
        Self {
            n: seats.len(),
            cursor: 0,
            in_lap: 0,
            lap_carried: false,
            over: Some(seats.to_vec()),
        }
    }

    /// The seat to ask now.
    fn seat(&self) -> usize {
        match &self.over {
            Some(list) => list.get(self.cursor).copied().unwrap_or(0).max(0) as usize,
            None => self.cursor,
        }
    }

    /// The current seat's visit ended -- `declared` if it added at least one
    /// counter. Returns whether the round is still open.
    fn answered(&mut self, declared: bool) -> bool {
        self.cursor = (self.cursor + 1) % self.n;
        if declared {
            self.lap_carried = true;
        }
        self.in_lap += 1;
        if self.in_lap < self.n {
            return true;
        }
        // Lap boundary: a lap that brought no declaration closes the round.
        self.in_lap = 0;
        std::mem::take(&mut self.lap_carried)
    }
}

/// Test seam for the counteraction window fast path (docs/GUARDS.md §4.5). When the
/// environment variable `BGD_COUNTERACT_SLOW=1` is set, the valid-option-first
/// pre-scan never skips a window and the per-window processed set is bypassed,
/// reproducing the pre-optimisation control flow. The two paths must produce
/// identical `Match::save()` checkpoints -- `examples/ckpt_equiv.rs` A/B's them.
// ---------------------------------------------------------------------
// STACK-01: counteract resume state. A nested settle suspends the drive;
// the rest of this `counteract` walk is parked here and adopted on re-entry
// so completed hook bodies run exactly once.
// ---------------------------------------------------------------------

/// One field / board / lingering hook drive still to run.
#[derive(Clone)]
struct HookJob {
    call: Call,
    card_id: String,
    uid: i32,
}

/// Remaining work of one `CardRules::counteract` invocation.
struct CounteractResume {
    /// Bridge trigger as completed drives left it.
    trigger: Trigger,
    own_done: bool,
    /// Hand window: `None` = not started; `Some` = built, remaining settles
    /// nodes still to settle.
    hand: Option<HandResume>,
    /// Hook drives left after the suspend (stable snapshot).
    hooks: Vec<HookJob>,
    hook_index: usize,
    /// The answer tree, while a walk (or a body continuation) is parked on it.
    /// Boxed and never moved out by value: continuations point into it.
    chain: Option<Box<Vec<ChainLink>>>,
    /// A join round in progress (`ctx::join::open()`).
    join: Option<JoinResume>,
}

/// Remaining work of one join round (design G06 §6.4).
struct JoinResume {
    /// The answered link the window sits on.
    at: usize,
    /// Its `target` before the round, restored when it finishes.
    saved_target: i32,
    /// Post-order joiner nodes still to settle.
    order: Vec<usize>,
    next: usize,
}

/// Remaining settles work of a hand counteraction window (STACK-01).
struct HandResume {
    /// Post-order node indices still to settle.
    order: Vec<usize>,
    next: usize,
}

/// Take the parked resume, or start one from `trigger`.
fn take_resume_or_fresh(cx: &mut PlayCtx, trigger: &Trigger) -> CounteractResume {
    cx.take_counteract_resume()
        .and_then(|s| s.downcast::<CounteractResume>().ok().map(|b| *b))
        .unwrap_or(CounteractResume {
            trigger: trigger.clone(),
            own_done: true,
            hand: None,
            hooks: Vec::new(),
            hook_index: 0,
            chain: None,
            join: None,
        })
}

/// Take the answer tree out of the parked resume (the rest stays parked).
/// Held boxes stay valid across parks: only the `Box` value moves, never the
/// `Vec` it points at.
fn take_chain_box(cx: &mut PlayCtx) -> Option<Box<Vec<ChainLink>>> {
    let mut r = cx
        .take_counteract_resume()?
        .downcast::<CounteractResume>()
        .ok()
        .map(|b| *b)?;
    let chain = r.chain.take();
    cx.set_counteract_resume(Box::new(r));
    chain
}

/// Put the answer tree back beside the parked walk (see [`take_chain_box`]).
fn park_chain_box(cx: &mut PlayCtx, trigger: &Trigger, chain: Box<Vec<ChainLink>>) {
    let mut r = take_resume_or_fresh(cx, trigger);
    r.chain = Some(chain);
    cx.set_counteract_resume(Box::new(r));
}

/// One run of an opened join window (design G06 §6.4): settle the round, then
/// re-enter the opener past `open()`. Pushed by the drive's `JoinOpen` arm and
/// re-pushed when a nested settle suspends the walk; the round's progress is
/// the parked [`JoinResume`].
struct JoinWork<M: CardModules> {
    bridge: *const RulesBridge<M>,
    /// The answer tree's `Vec`, valid while the walk is parked.
    chain: *mut Vec<ChainLink>,
    /// The answered link the window sits on.
    at: usize,
    /// The opener's own node.
    node: usize,
    call: Call,
    card_id: String,
    uid: i32,
    guarded: bool,
    trigger: Trigger,
    log: game_core::engine::GuestLog,
    /// Frames the drive had already adopted (the commit net, G11 §5.4).
    adopted: Vec<i32>,
}

impl<M: CardModules> game_core::engine::WorkItem for JoinWork<M> {
    fn run(self: Box<Self>, cx: &mut PlayCtx) -> Flow<()> {
        let this = *self;
        if this.uid >= 0 {
            cx.reentrant_hooks.push(this.uid);
        }
        let bridge = unsafe { &*this.bridge };
        let chain_ptr = this.chain;
        let mut trigger = this.trigger;
        // Adopt the parked chain and round; put the resume straight back so a
        // nested suspension parks into it (only the pointer is kept alive).
        let mut chain = take_chain_box(cx);
        let resume = {
            let r = take_resume_or_fresh(cx, &trigger);
            let mut r = r;
            let j = r.join.take();
            cx.set_counteract_resume(Box::new(r));
            j
        };
        let Some(mut chain) = chain.take() else {
            if this.uid >= 0 {
                cx.reentrant_hooks.pop();
            }
            return Ok(());
        };
        let resume = resume.unwrap_or(JoinResume {
            at: this.at,
            saved_target: unsafe { &*chain_ptr }[this.at].link.target,
            order: Vec::new(),
            next: 0,
        });
        let r = bridge.settle_join(cx, &mut chain, this.at, this.node, resume, &mut trigger);
        if this.uid >= 0 {
            cx.reentrant_hooks.pop();
        }
        match r {
            Ok(()) => {
                // Park the live box while the body runs: the walk continues
                // through the installed cursor, whose address is this box, so
                // it must stay parked (and alive).
                park_chain_box(cx, &trigger, chain);
                // The opener's report (marks, `opened`) is on the answered
                // link; hand it to the body's own link before it runs again,
                // with the window readers refreshed for what joined.
                trigger.join = unsafe { (&*chain_ptr)[this.at].link.join.clone() };
                bridge.stamp_window(cx, unsafe { &*chain_ptr }, this.node, &mut trigger);
                let before_face = trigger
                    .mv
                    .as_ref()
                    .and_then(|m| m.roll)
                    .unwrap_or(trigger.value);
                let r2 = bridge.drive_inner_body_from(
                    cx,
                    this.call,
                    &this.card_id,
                    this.uid,
                    &mut trigger,
                    this.guarded,
                    this.log,
                    true,
                    this.adopted,
                );
                match r2 {
                    Ok(dest) => {
                        // A nested adoption may have taken the box out and
                        // handed it back; park it again for the tail.
                        if let Some(b) = take_chain_box(cx) {
                            park_chain_box(cx, &trigger, b);
                        }
                        let chain = unsafe { &mut *chain_ptr };
                        // This run owns the opener's tail (the walk's
                        // settle_node has unwound past it).
                        bridge.close_drive(cx, chain, this.node, &mut trigger, before_face)?;
                        let seat = chain[this.node].seat;
                        let id = chain[this.node].id.clone();
                        let uid = chain[this.node].uid;
                        let negated = chain[this.node].link.is_cancelled();
                        bridge.settle_tail(cx, seat, &id, uid, negated, dest)?;
                        // Keep the box alive across the resume drop below; the
                        // cursor address stays valid.
                        if let Some(a) = take_chain_box(cx) {
                            cx.keep_chain(a);
                        }
                        // A raise in the body or the tail normally picks the
                        // parked walk back up (`counteract` adopts the resume).
                        // A quiet body raises nothing, and the suspension took
                        // the walk off the Rust stack -- continue it here.
                        let mut r = take_resume_or_fresh(cx, &trigger);
                        if let Some(hr) = r.hand.take() {
                            let mut walk_chain = Box::new(unsafe { (*chain_ptr).clone() });
                            let mut walk_trigger = r.trigger.clone();
                            bridge.resolve_jobs(
                                cx,
                                &walk_trigger,
                                &mut walk_chain,
                                &hr.order,
                                hr.next,
                            )?;
                            if let Some(root) = walk_chain.first() {
                                trigger = root.link.clone();
                            }
                            // The outer caller reads the tree through the
                            // cursor: land the continuation's work there.
                            unsafe {
                                *chain_ptr = (*walk_chain).clone();
                            }
                            let kept: Box<dyn std::any::Any + Send> = walk_chain;
                            cx.keep_chain(kept);
                        }
                        cx.set_drive_dest(dest);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            Err(e) => {
                // The round parked itself mid-settle: hand the live tree back
                // to the resume (`take_chain_box` reclaims it on the next run),
                // whether the halt is a suspension or an `Ask` whose replay
                // resumes the parked walk. Dropping it here would leave the
                // continuation with an empty chain.
                park_chain_box(cx, &trigger, chain);
                if e.is_suspended() {
                    if this.uid >= 0 {
                        cx.reentrant_hooks.push(this.uid);
                    }
                    cx.push_work(JoinWork {
                        bridge: this.bridge,
                        chain: chain_ptr,
                        at: this.at,
                        node: this.node,
                        call: this.call,
                        card_id: this.card_id,
                        uid: this.uid,
                        guarded: this.guarded,
                        trigger,
                        log: this.log,
                        adopted: this.adopted,
                    });
                }
                Err(e)
            }
        }
    }
}

fn counteract_slow_path() -> bool {
    static SLOW: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SLOW.get_or_init(|| std::env::var("BGD_COUNTERACT_SLOW").is_ok_and(|v| v == "1"))
}

/// May this seat declare a counteraction right now? Out / off-board players
/// cannot. (`CannotPlay` and a one-turn mute have no engine field yet.)
fn can_counteract_now(cx: &PlayCtx, s: usize) -> bool {
    let Some(player_id) = cx.state().players.get(s) else {
        return false;
    };
    // A bot is still a player, so the window opens for every mentality and the seat
    // answers through `PlayCtx::fill_ai` (standard: `CounterParams` propensity, default
    // never; chaos: `CHAOS_COUNTER_CHANCE`) exactly like a human's offer. Out /
    // off-board remain skipped. `World::out` is `st.players[i].out()`, so the live
    // state answers it -- no world copy (this runs on every visit of every cycle).
    //
    // 骰子已经掷下 「本回合内所有其他玩家无法从手牌中使用counteraction」: another
    // seat's `noCounteractTurn` latch for this turn key locks this seat out of
    // *hand* counteractions. Field counteractions (`counteractFromField`) are
    // offered through `has_field_counter` / the field half of `declare_one`
    // and are not gated here -- the clause names 「从手牌中」. The latch owner
    // (the caster) is exempt: 「其他」.
    let st = cx.state();
    let turn_key = st.round * 100 + st.turn + 1;
    let locked_out = st.players.iter().enumerate().any(|(p, pl)| {
        p != s && pl.state_get(game_core::state::key::NO_COUNTERACT_TURN) == turn_key
    });
    !locked_out
        && !player_id.out()
        && cx
            .world()
            .gate_open(cx.rules(), game_core::engine::Gate::OnBoard, s)
        && cx
            .world()
            .gate_open(cx.rules(), game_core::engine::Gate::Act, s)
        && cx
            .world()
            .gate_open(cx.rules(), game_core::engine::Gate::Play, s)
}

/// The player's private hand (order preserved).
fn hand_of<'a>(cx: &'a PlayCtx, s: usize) -> &'a [String] {
    cx.world()
        .hidden
        .get(s)
        .map(|h| h.hand.as_slice())
        .unwrap_or(&[])
}

/// The live-world snapshot source (docs/GUARDS.md §4.2): reads the engine
/// `World` in place -- **no clone** -- so the counteract pre-filter can build a
/// window / candidate context before any `Run` exists. Only the reads
/// [`crate::condition::fill_window`] / [`crate::condition::fill_candidate`] make.
pub struct LiveSnap<'a> {
    pub world: &'a game_core::engine::World,
    pub data: &'a GameData,
    pub trigger: &'a Trigger,
    /// Candidate being probed, for the [`rules_cond::view::CondView`] impl:
    /// `(owner seat, card id, placed)`. `None` on the window-only form
    /// [`crate::condition::fill_window`] takes.
    pub cand: Option<(i32, &'a str, bool)>,
    /// The chain and the node being offered, when this view is an offer --
    /// the pending readers walk it (design G06 §6.2).
    pub pending: Option<(&'a [ChainLink], usize)>,
    /// The rule set, for the entry-effect / property readers.
    pub rules: Option<&'a dyn EntryEffects>,
}

/// What the pending readers need from the rule set (the `LiveSnap` field).
pub(crate) trait EntryEffects {
    /// The declared answer effect of `card`'s entry answering `t`.
    fn entry_effect_of(
        &self,
        card: i32,
        t: &Trigger,
        source: Option<&str>,
    ) -> card_sdk::abi::AnswerEffect;
    /// Does the card declare `prop::RANGE_CARD`?
    fn declares_range_card(&self, id: &str) -> bool;
}

impl<M: CardModules> EntryEffects for RulesBridge<M> {
    fn entry_effect_of(
        &self,
        card: i32,
        t: &Trigger,
        source: Option<&str>,
    ) -> card_sdk::abi::AnswerEffect {
        self.ruleset.entry_effect(card, t, source)
    }
    fn declares_range_card(&self, id: &str) -> bool {
        self.ruleset.card(id).is_some_and(|i| {
            self.ruleset.cards()[i as usize]
                .props
                .get(card_sdk::abi::prop::RANGE_CARD)
                .copied()
                == Some(1)
        })
    }
}

/// The chain cursor the settle walk installs on the context around every
/// counter body and every offer (design G06 §6.2). Valid only for the duration
/// of that drive / offer.
pub(crate) struct CounteractChainCursor {
    /// The live chain (behind its `Vec`, so appends stay valid).
    pub chain: *mut Vec<ChainLink>,
    /// The node being offered / driven (its `answered` is the link).
    pub index: usize,
}

// SAFETY: the cursor only lives for the synchronous drive / offer that made
// it; the chain it points into does not move while installed.
unsafe impl Send for CounteractChainCursor {}

impl LiveSnap<'_> {
    /// The offer contracts of the declared, unresolved answers on the
    /// answered link (`trigger::pending_negation` / `extreme_armed`).
    fn declared_effects(&self) -> Vec<card_sdk::abi::AnswerEffect> {
        let (Some((chain, index)), Some(rules)) = (self.pending, self.rules) else {
            return Vec::new();
        };
        // `index` is the offered link; on a counter's own round that is the
        // counter, whose `answered` carries the settled-at window declarations
        // (the counter itself included -- an armed extreme must stay armed,
        // §6.6).
        let answered = chain[index].answered;
        chain[answered]
            .link
            .answers_declared
            .iter()
            .filter_map(|&d| chain.get(d as usize))
            .map(|n| rules.entry_effect_of(n.idx, &n.link, None))
            .collect()
    }
}

impl crate::condition::SnapSrc for LiveSnap<'_> {
    #[inline]
    fn trigger(&self) -> Trigger {
        self.trigger.clone()
    }
    #[inline]
    fn tile_standing_at(&self, position: i32) -> i32 {
        self.world.tile_standing_at(position)
    }
    fn has_swaps(&self) -> bool {
        self.world.has_swaps()
    }
    fn subject_range_card(&self) -> bool {
        let (Some(rules), Some(subject)) = (self.rules, self.trigger.subject_card.as_deref())
        else {
            return false;
        };
        rules.declares_range_card(subject)
    }
    fn extreme_armed(&self) -> bool {
        self.world.turn.extreme != game_core::engine::Extreme::Plain
            || self
                .declared_effects()
                .iter()
                .any(|e| e.contains(card_sdk::abi::AnswerEffect::ARMS_EXTREME))
    }
    fn pending_negation(&self) -> i64 {
        self.declared_effects()
            .iter()
            .filter(|e| e.contains(card_sdk::abi::AnswerEffect::NEGATES))
            .count() as i64
    }
    fn chain_depth(&self) -> i64 {
        self.trigger.seq.saturating_sub(1) as i64
    }
    #[inline]
    fn tile_named(&self, name: &str) -> i32 {
        self.world.tile_named(self.data, name)
    }
    #[inline]
    fn tile_count(&self) -> i32 {
        self.data.tiles.len() as i32
    }
    #[inline]
    fn tile_name(&self, tile: i32) -> String {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.name.replace('\n', ""))
            .unwrap_or_default()
    }
    #[inline]
    fn tile_kind_of(&self, tile: i32) -> String {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.kind.as_str().to_string())
            .unwrap_or_default()
    }
    fn tile_is(&self, tile: i32, kind: &str) -> bool {
        crate::tile_class::kind_is(self.data.tiles.get(tile.max(0) as usize), kind)
    }
    #[inline]
    fn tile_attr(&self, tile: i32, attr: &str) -> i32 {
        crate::tile_class::attr_of(self.data.tiles.get(tile.max(0) as usize), attr)
    }
    #[inline]
    fn tile_in(&self, player_id: i32, tile: i32, class: &str) -> bool {
        crate::tile_class::tile_in(
            self.data.tiles.get(tile.max(0) as usize),
            player_id,
            class,
            |k| self.world.tile_prop(tile, k),
        )
    }
    fn tile_class_rows(
        &self,
        tile: i32,
        classes: &[String],
        out: &mut Vec<(i64, i64, String)>,
    ) -> bool {
        crate::tile_class::class_rows(
            &self.world,
            self.data.tiles.get(tile.max(0) as usize),
            tile,
            classes,
            out,
        )
    }
    fn player_count(&self) -> i32 {
        self.world.st.players.len() as i32
    }
    #[inline]
    fn money(&self, player_id: i32) -> i32 {
        self.world.player_money(player_id)
    }
    #[inline]
    fn group_pool(&self, player_id: i32, name: &str) -> i32 {
        self.world.group_pool(player_id, name)
    }
    fn group_pool_total(&self, player_id: i32) -> i32 {
        self.world.group_pool_total(player_id)
    }
    fn counter_names_at(&self, uid: i32) -> Vec<String> {
        self.world
            .field_by_uid(uid)
            .map(|f| f.counters.keys().cloned().collect())
            .unwrap_or_default()
    }
    #[inline]
    fn counter_at(&self, uid: i32, name: &str) -> i32 {
        self.world.counter_at(uid, name)
    }
    #[inline]
    fn tile_at(&self, uid: i32) -> i32 {
        self.world.tile_at(uid)
    }
    #[inline]
    fn hand_size(&self, player_id: i32) -> i32 {
        self.world
            .hidden
            .get(player_id.max(0) as usize)
            .map(|h| h.hand.len() as i32)
            .unwrap_or(0)
    }
    #[inline]
    fn player_pos(&self, player_id: i32) -> i32 {
        self.world.player_pos(player_id)
    }
    #[inline]
    fn player_out(&self, player_id: i32) -> i32 {
        self.world.player_out(player_id) as i32
    }
    #[inline]
    fn state_get(&self, player_id: i32, key: &str) -> i32 {
        let v = self.world.state_get(player_id, key);
        v
    }
    fn state_entries(&self, player_id: i32) -> Vec<(String, i32)> {
        self.world
            .st
            .players
            .get(player_id.max(0) as usize)
            .map(|p| p.state.iter().map(|(k, v)| (k.clone(), v.value)).collect())
            .unwrap_or_default()
    }
    #[inline]
    fn persona_skill_id(&self, player_id: i32) -> Option<String> {
        self.world.persona_skill_id(player_id)
    }
    #[inline]
    fn group_skill_id(&self, player_id: i32) -> Option<String> {
        self.world.group_skill_id(player_id)
    }
    #[inline]
    fn persona_name(&self, player_id: i32) -> Option<String> {
        self.world
            .st
            .players
            .get(player_id.max(0) as usize)
            .map(|s| s.persona.clone())
            .filter(|c| !c.is_empty())
    }
    #[inline]
    fn group_name(&self, player_id: i32) -> Option<String> {
        // Same lookup `CardWorld::in_group` does: the persona's group.
        let s = self.world.st.players.get(player_id.max(0) as usize)?;
        self.data
            .personas
            .iter()
            .find(|c| c.name == s.persona)
            .map(|c| c.group.clone())
    }
    #[inline]
    fn held_count(&self, player_id: i32, name: &str) -> i32 {
        self.world.held_count(player_id, name)
    }
    #[inline]
    fn held_names(&self, player_id: i32, prefix: &str) -> Vec<String> {
        self.world.held_names(player_id, prefix)
    }
    #[inline]
    fn owned_count(&self, player_id: i32) -> i32 {
        let n = self.world.owned_tiles(player_id).len() as i32;
        if std::env::var("DBG_H01").is_ok() && player_id == 0 {
            eprintln!(
                "H01 NATIVE owned_count p0 = {n} owner16={:?}",
                self.world.st.owners.get(16)
            );
        }
        n
    }
    #[inline]
    fn card_pool(&self, player_id: i32, card: &str, name: &str) -> i32 {
        self.world.card_counter(player_id, card, name)
    }
    #[inline]
    fn card_counter(&self, player_id: i32, card: &str, name: &str) -> i32 {
        self.world.card_counter(player_id, card, name)
    }
    #[inline]
    fn card_counter_names(&self, player_id: i32, card: &str) -> Vec<String> {
        self.world
            .field_instances(player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map(|(uid, _)| self.world.counter_names_at(uid))
            .unwrap_or_default()
    }
    #[inline]
    fn tile_owner(&self, tile: i32) -> i32 {
        self.world.tile_owner(tile)
    }
    #[inline]
    fn houses_of(&self, tile: i32) -> i32 {
        let v = self
            .world
            .st
            .houses
            .get(tile.max(0) as usize)
            .copied()
            .unwrap_or(0);
        v
    }
    #[inline]
    fn mortgaged_of(&self, tile: i32) -> i32 {
        self.world
            .st
            .mortgaged
            .get(tile.max(0) as usize)
            .copied()
            .unwrap_or(false) as i32
    }
    #[inline]
    fn tile_price(&self, tile: i32) -> i32 {
        self.data
            .tiles
            .get(tile.max(0) as usize)
            .map(|t| t.price)
            .unwrap_or(0)
    }
    #[inline]
    fn turn_player(&self) -> i32 {
        self.world.st.turn
    }
    #[inline]
    fn turn_key(&self) -> i32 {
        self.world.st.round * 100 + self.world.st.turn + 1
    }
    #[inline]
    fn fixed_roll(&self) -> i32 {
        self.world.turn.fixed_roll.map(|f| f.value).unwrap_or(-1)
    }
    #[inline]
    fn gains_this_turn(&self, player_id: i32) -> i32 {
        self.world.gains_this_turn(player_id)
    }
    #[inline]
    fn targeted_count(&self, player_id: i32) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.world.targeted.get(s).copied())
            .unwrap_or(0)
    }
    #[inline]
    fn card_tile(&self, player_id: i32, card: &str) -> i32 {
        self.world
            .field_instances(player_id)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-2, |(uid, _)| self.world.tile_at(uid))
    }
}

/// `CondView` over the live world (docs/GUARDS.md §4.2b). Every method calls
/// `self.world.*` / `self.data.*` **directly** -- never `self.<SnapSrc::method>`,
/// which would re-enter this trait and overflow the stack.
impl rules_cond::view::CondView for LiveSnap<'_> {
    fn kind(&self) -> std::borrow::Cow<'_, str> {
        self.trigger.kind.spelling()
    }
    fn actor(&self) -> i64 {
        self.trigger.player_id as i64
    }
    fn target(&self) -> i64 {
        self.trigger.target as i64
    }
    fn tile_id(&self) -> i64 {
        self.trigger.tile as i64
    }
    fn tile_owner(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.world.tile_owner(self.trigger.tile) as i64
        } else {
            -1
        }
    }
    fn tile_houses(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.world
                .st
                .houses
                .get(self.trigger.tile.max(0) as usize)
                .copied()
                .unwrap_or(0) as i64
        } else {
            0
        }
    }
    fn tile_mortgaged(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.world
                .st
                .mortgaged
                .get(self.trigger.tile.max(0) as usize)
                .copied()
                .unwrap_or(false) as i64
        } else {
            0
        }
    }
    fn tile_price(&self) -> i64 {
        if self.trigger.tile >= 0 {
            self.data
                .tiles
                .get(self.trigger.tile.max(0) as usize)
                .map(|t| t.price)
                .unwrap_or(0) as i64
        } else {
            0
        }
    }
    fn value(&self) -> i64 {
        self.trigger.shown_value() as i64
    }
    fn step(&self) -> i64 {
        self.trigger.step as i64
    }
    fn by(&self) -> i64 {
        self.trigger.by_card.map(|b| b as i64).unwrap_or(-1)
    }
    fn pay_is_rent(&self) -> bool {
        self.trigger.pay.map(|p| p.is_rent).unwrap_or(false)
    }
    fn pay_kind(&self) -> i64 {
        self.trigger
            .pay
            .map(|p| p.kind.as_i32() as i64)
            .unwrap_or(-1)
    }
    fn pay_is_fixed(&self) -> i64 {
        self.trigger
            .pay
            .map(|p| (p.shape == game_core::engine::PayShape::Fixed) as i64)
            .unwrap_or(0)
    }
    fn pay_total_stage(&self) -> bool {
        self.trigger.pay.map(|p| p.total_stage).unwrap_or(true)
    }
    fn pay_split_total(&self) -> bool {
        self.trigger.pay.map(|p| p.split_total).unwrap_or(false)
    }
    fn charge_is_skill(&self) -> bool {
        self.trigger.charge_skill
    }
    fn subject_range_card(&self) -> bool {
        crate::condition::SnapSrc::subject_range_card(self)
    }
    fn extreme_armed(&self) -> bool {
        crate::condition::SnapSrc::extreme_armed(self)
    }
    fn pending_negation(&self) -> i64 {
        crate::condition::SnapSrc::pending_negation(self)
    }
    fn chain_depth(&self) -> i64 {
        crate::condition::SnapSrc::chain_depth(self)
    }
    fn settle_tile(&self) -> i64 {
        self.trigger.settle_tile as i64
    }
    fn settle_owner(&self) -> i64 {
        if self.trigger.settle_tile >= 0 {
            self.world.tile_owner(self.trigger.settle_tile) as i64
        } else {
            -1
        }
    }
    fn settle_houses(&self) -> i64 {
        if self.trigger.settle_tile >= 0 {
            self.world
                .st
                .houses
                .get(self.trigger.settle_tile.max(0) as usize)
                .copied()
                .unwrap_or(0) as i64
        } else {
            0
        }
    }
    fn settle_mortgaged(&self) -> bool {
        self.trigger.settle_tile >= 0
            && self
                .world
                .st
                .mortgaged
                .get(self.trigger.settle_tile.max(0) as usize)
                .copied()
                .unwrap_or(false)
    }
    fn settle_outcome(&self) -> i64 {
        self.trigger
            .outcome
            .map(|o| o.as_i32() as i64)
            .unwrap_or(-1)
    }
    fn move_roll(&self) -> i64 {
        self.trigger
            .mv
            .as_ref()
            .and_then(|m| m.roll)
            .filter(|&r| r >= 0)
            .map(|r| r as i64)
            .unwrap_or(-1)
    }
    fn move_kind(&self) -> i64 {
        self.trigger
            .mv
            .as_ref()
            .map(|m| m.kind as i64)
            .unwrap_or(-1)
    }
    fn move_remaining(&self) -> i64 {
        self.trigger.mv.as_ref().map(|m| m.remaining).unwrap_or(0) as i64
    }
    fn move_main(&self) -> bool {
        self.trigger.is_main_move
    }
    fn move_by_skill(&self) -> bool {
        self.trigger
            .mv
            .as_ref()
            .map(|m| m.by_skill)
            .unwrap_or(false)
    }
    fn move_rolled(&self) -> bool {
        self.trigger.mv.as_ref().map(|m| m.rolled).unwrap_or(false)
    }
    fn move_effective(&self) -> i64 {
        self.trigger.mv.as_ref().map(|m| m.effective).unwrap_or(0) as i64
    }
    fn move_from(&self) -> i64 {
        self.trigger.mv.as_ref().map(|m| m.from).unwrap_or(-1) as i64
    }
    fn move_dir(&self) -> i64 {
        self.trigger
            .mv
            .as_ref()
            .map(|m| m.dir.as_i32() as i64)
            .unwrap_or(1)
    }
    fn move_tag_named(&self, name: &str) -> i64 {
        self.trigger
            .mv
            .as_ref()
            .and_then(|m| m.tags.iter().find(|(k, _)| k == name))
            .map(|(_, v)| *v as i64)
            .unwrap_or(0)
    }
    fn move_tag_table(&self) -> Vec<(String, i64)> {
        self.trigger
            .mv
            .as_ref()
            .map(|m| m.tags.iter().map(|(k, v)| (k.clone(), *v as i64)).collect())
            .unwrap_or_default()
    }
    fn roll_source(&self) -> i64 {
        self.trigger.roll_source.as_i32() as i64
    }
    fn roll_purpose(&self) -> i64 {
        self.trigger
            .roll
            .as_ref()
            .map(|r| r.purpose as i64)
            .unwrap_or(-1)
    }
    fn roll_rerolled(&self) -> bool {
        self.trigger
            .roll
            .as_ref()
            .map(|r| r.rerolled)
            .unwrap_or(false)
    }
    fn roll_unused_face(&self) -> i64 {
        self.trigger
            .roll
            .as_ref()
            .map(|r| r.unused_face as i64)
            .unwrap_or(-1)
    }
    fn abnormal(&self) -> bool {
        matches!(self.trigger.kind, crate::TriggerKind::Abnormal)
    }
    fn turn_player(&self) -> i64 {
        self.world.st.turn as i64
    }
    fn turn_key(&self) -> i64 {
        (self.world.st.round * 100 + self.world.st.turn + 1) as i64
    }
    fn chain_count(&self) -> i64 {
        self.trigger.effects.len() as i64
    }
    fn chain_kinds(&self) -> Vec<String> {
        self.trigger
            .effects
            .iter()
            .map(|e| e.kind.clone())
            .collect()
    }
    fn chain_hits(&self) -> Vec<i64> {
        self.trigger
            .effects
            .iter()
            .map(|e| e.target as i64)
            .collect()
    }
    fn trigger_card(&self) -> i64 {
        match self.trigger.card.as_deref() {
            Some(card) if !card.is_empty() => crate::condition::id_of(card),
            _ => 0,
        }
    }
    fn counter_name(&self) -> i64 {
        // `t.Name` on a `CounterChanged` hook / `On::Message` entry.
        match self.trigger.name.as_deref() {
            Some(name) if !name.is_empty() => crate::condition::id_of(name),
            _ => 0,
        }
    }
    fn owner(&self) -> i64 {
        self.cand.map(|(o, _, _)| o as i64).unwrap_or(-1)
    }
    fn owner_money(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.player_money(o as i32) as i64
        } else {
            0
        }
    }
    fn owner_hand(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world
                .hidden
                .get(o.max(0) as usize)
                .map(|h| h.hand.len() as i64)
                .unwrap_or(0)
        } else {
            0
        }
    }
    fn owner_pos(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.player_pos(o as i32) as i64
        } else {
            0
        }
    }
    fn owner_out(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.player_out(o as i32) as i64
        } else {
            0
        }
    }
    fn owner_tiles(&self) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.owned_tiles(o as i32).len() as i64
        } else {
            0
        }
    }
    fn card_id(&self) -> i64 {
        self.cand
            .map(|(_, c, _)| crate::condition::id_of(c))
            .unwrap_or(0)
    }
    fn card_placed(&self) -> bool {
        self.cand.map(|(_, _, p)| p).unwrap_or(false)
    }
    fn card_counter(&self, name: &str) -> i64 {
        let (Some((o, c, _)), true) = (self.cand, self.owner() >= 0) else {
            return 0;
        };
        self.world.card_counter(o, c, name) as i64
    }
    fn slot(&self, name: &str) -> i64 {
        let o = self.owner();
        if o >= 0 {
            self.world.state_get(o as i32, name) as i64
        } else {
            0
        }
    }
    fn held_named(&self, name: &str) -> i64 {
        // Same data `ctx::tok(owner, name)` reads.
        let o = self.owner();
        if o >= 0 {
            self.world.held_count(o as i32, name) as i64
        } else {
            0
        }
    }
    fn cand_set(&self, key: &str) -> Vec<i64> {
        // Host fills the blocked list from the skill dispatch, not the raw world.
        let _ = key;
        Vec::new()
    }
    fn money(&self, seat: i64) -> i64 {
        self.world.player_money(seat as i32) as i64
    }
    fn hand(&self, seat: i64) -> i64 {
        self.world
            .hidden
            .get(seat.max(0) as usize)
            .map(|h| h.hand.len() as i64)
            .unwrap_or(0)
    }
    fn pos(&self, seat: i64) -> i64 {
        self.world.player_pos(seat as i32) as i64
    }
    fn out(&self, seat: i64) -> i64 {
        self.world.player_out(seat as i32) as i64
    }
    fn state(&self, seat: i64, key: &str) -> i64 {
        // Sugar keys published through the one `state` door (CondView contract).
        match key {
            bandori_std::cel::KEY_CHARGE => {
                self.world
                    .held_count(seat as i32, bandori_std::cel::KEY_CHARGE) as i64
            }
            bandori_std::cel::KEY_CRYSTALS => {
                self.world
                    .group_pool(seat as i32, bandori_std::cel::KEY_CRYSTALS) as i64
            }
            bandori_std::cel::KEY_PERSONA => {
                let name = self
                    .world
                    .st
                    .players
                    .get(seat.max(0) as usize)
                    .map(|s| s.persona.clone())
                    .unwrap_or_default();
                crate::condition::id_of(&name)
            }
            bandori_std::cel::KEY_GROUP => {
                let group_str = self
                    .world
                    .st
                    .players
                    .get(seat.max(0) as usize)
                    .and_then(|s| {
                        self.data
                            .personas
                            .iter()
                            .find(|c| c.name == s.persona)
                            .map(|c| c.group.clone())
                    })
                    .unwrap_or_default();
                crate::condition::id_of(&group_str)
            }
            _ => self.world.state_get(seat as i32, key) as i64,
        }
    }
    fn state_table(&self) -> Vec<(i64, String, i64)> {
        let mut rows = Vec::new();
        for (s, _p) in self.world.st.players.iter().enumerate() {
            // Sugar keys first so the eager fill and `IrCtx.state` agree.
            let s = s as i64;
            for k in bandori_std::cel::STATE_KEYS {
                rows.push((s, (*k).to_string(), self.state(s, k)));
            }
            for (k, v) in &self.world.st.players[s as usize].state {
                if !bandori_std::cel::STATE_KEYS.contains(&k.as_str()) {
                    rows.push((s, k.clone(), v.value as i64));
                }
            }
        }
        rows
    }
    fn tiles(&self, seat: i64) -> i64 {
        self.world.owned_tiles(seat as i32).len() as i64
    }
    fn seat_count(&self) -> i64 {
        self.world.st.players.len() as i64
    }
    fn tile_named(&self, name: &str) -> i64 {
        self.world.tile_named(self.data, name) as i64
    }
    fn tile_is(&self, tile: i64, kind: &str) -> bool {
        crate::tile_class::kind_is(self.data.tiles.get(tile.max(0) as usize), kind)
    }
    fn tile_attr(&self, tile: i64, attr: &str) -> i64 {
        crate::tile_class::attr_of(self.data.tiles.get(tile.max(0) as usize), attr) as i64
    }
    fn tile_in(&self, player: i64, tile: i64, class: &str) -> bool {
        crate::tile_class::tile_in(
            self.data.tiles.get(tile.max(0) as usize),
            player as i32,
            class,
            |k| self.world.tile_prop(tile as i32, k),
        )
    }
    fn tile_class_names(&self) -> Vec<String> {
        // Ruleset class vocabulary (no engine class table).
        crate::condition::class_names()
    }
    fn slot_table(&self) -> Vec<(String, i64)> {
        let mut rows: Vec<(String, i64)> = Vec::new();
        let o = self.owner();
        for n in crate::condition::SLOT_NAMES {
            let v = crate::condition::slot_value(self, o as i32, n);
            rows.push((n.to_string(), v));
            rows.push((format!("latch:{n}"), v));
        }
        rows
    }
    fn held_named_table(&self) -> Vec<(String, i64)> {
        let o = self.owner();
        if o < 0 {
            return Vec::new();
        }
        self.world
            .held_names(o as i32, "")
            .into_iter()
            .map(|n| {
                let v = self.world.held_count(o as i32, &n) as i64;
                (n, v)
            })
            .collect()
    }
    fn card_counter_table(&self) -> Vec<(String, i64)> {
        // Instance keys (the two on-card wire names live content-side in
        // `card-sdk::abi::counter`; `card_counter` reads them by name).
        let (Some((o, c, _)), true) = (self.cand, self.owner() >= 0) else {
            return Vec::new();
        };
        crate::condition::SnapSrc::card_counter_names(self, o, c)
            .into_iter()
            .map(|n| {
                let v = self.world.card_counter(o, c, &n) as i64;
                (n, v)
            })
            .collect()
    }
    fn tile_id_table(&self) -> Vec<(String, i64)> {
        crate::condition::collect_tile_ids(self)
            .into_iter()
            .collect()
    }
    fn tile_kind_list(&self, kind: &str) -> Vec<i64> {
        let mut out = Vec::new();
        for tile in 0..self.data.tiles.len() as i64 {
            if self.tile_is(tile, kind) {
                out.push(tile);
            }
        }
        out
    }
    fn tile_kind_names(&self) -> Vec<String> {
        self.data
            .tiles
            .iter()
            .map(|t| t.kind.as_str().to_string())
            .filter(|k| !k.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    // -- turn plan / counters ----------------------------------------------
    fn plan_fixed_roll(&self) -> i64 {
        let v = self
            .world
            .turn
            .fixed_roll
            .map(|f| f.value as i64)
            .unwrap_or(-1);
        v
    }
    fn gains_this_turn(&self, seat: i64) -> i64 {
        if seat < 0 {
            return 0;
        }
        self.world.gains_this_turn(seat as i32) as i64
    }
    fn targeted_count(&self, seat: i64) -> i64 {
        if seat < 0 {
            return 0;
        }
        usize::try_from(seat as i32)
            .ok()
            .and_then(|s| self.world.targeted.get(s).copied())
            .unwrap_or(0) as i64
    }

    // -- candidate instance -------------------------------------------------
    fn card_tile(&self) -> i64 {
        let Some((o, c, _)) = self.cand else {
            return -2;
        };
        if o < 0 {
            return -2;
        }
        self.world
            .field_instances(o)
            .into_iter()
            .find(|(_, id)| id == c)
            .map_or(-2i64, |(uid, _)| self.world.tile_at(uid) as i64)
    }

    // -- board geometry ----------------------------------------------------
    fn tile_count(&self) -> i64 {
        self.data.tiles.len() as i64
    }
    fn dist(&self, a: i64, b: i64) -> i64 {
        let n = self.data.tiles.len() as i64;
        if n <= 0 {
            return 0;
        }
        let d = ((b - a) % n + n) % n;
        d.min(n - d)
    }
    fn players_on(&self, tile: i64, except: i64) -> i64 {
        // Matches `players_on_list`: present standers.
        (0..self.world.st.players.len() as i64)
            .filter(|&s| s != except && self.present(s) && self.pos(s) == tile)
            .count() as i64
    }
    fn next_dist(&self, p: i64, dir: i64) -> i64 {
        let n = self.data.tiles.len() as i64;
        let pos = self.pos(p);
        if n <= 0 || pos < 0 {
            return -1;
        }
        let mut best = i64::MAX;
        for o in 0..self.world.st.players.len() as i64 {
            if o == p || self.out(o) != 0 {
                continue;
            }
            let q = self.pos(o);
            if q < 0 {
                continue;
            }
            let fwd = if dir >= 0 {
                ((q - pos) % n + n) % n
            } else {
                ((pos - q) % n + n) % n
            };
            if fwd > 0 && fwd < best {
                best = fwd;
            }
        }
        if best == i64::MAX {
            -1
        } else {
            best
        }
    }
    fn others_within(&self, p: i64, radius: i64) -> i64 {
        let pos = self.pos(p);
        if pos < 0 {
            return 0;
        }
        (0..self.world.st.players.len() as i64)
            .filter(|&o| {
                if o == p || self.out(o) != 0 {
                    return false;
                }
                let d = self.dist(pos, self.pos(o));
                d > 0 && d <= radius
            })
            .count() as i64
    }
    fn owned_within(&self, p: i64, radius: i64) -> i64 {
        let pos = self.pos(p);
        if pos < 0 {
            return 0;
        }
        self.world
            .owned_tiles(p as i32)
            .into_iter()
            .filter(|&t| self.dist(pos, t as i64) <= radius)
            .count() as i64
    }
    fn on_path(&self, me: i64, them: i64) -> i64 {
        let roll = self.move_roll();
        if roll < 0 {
            return 0;
        }
        let steps = roll.abs();
        let n = self.data.tiles.len() as i64;
        let pos = self.pos(them);
        if n <= 0 || pos < 0 {
            return 0;
        }
        (1..=steps)
            .filter(|i| {
                let t = ((pos + i) % n + n) % n;
                self.world.tile_owner(t as i32) as i64 == me
            })
            .count() as i64
    }
    fn between(&self, p: i64) -> i64 {
        let roll = self.move_roll();
        if roll < 0 {
            return 0;
        }
        let roll = roll.abs();
        // G04 item 12: the planned start, when one is stamped.
        let start = if self.move_from() >= 0 {
            self.move_from()
        } else {
            self.pos(p)
        };
        let n = self.data.tiles.len() as i64;
        if n <= 0 || start < 0 {
            return 0;
        }
        let backward = self.move_dir() < 0;
        (0..self.world.st.players.len() as i64)
            .filter(|&o| {
                if o == p || self.out(o) != 0 {
                    return false;
                }
                let q = self.pos(o);
                if q < 0 {
                    return false;
                }
                let fwd = ((q - start) % n + n) % n;
                let d = if backward { (n - fwd) % n } else { fwd };
                (1..=roll).contains(&d)
            })
            .count() as i64
    }
}

/// Per-window processed set (docs/GUARDS.md §4.5): one trigger's cycle, across
/// its laps. Remembers each `(seat, card)` probe's verdict so a later lap does
/// not re-run its condition / guard when the inputs it reads cannot have
/// changed.
///
/// The cycle's only world change is a declaration removing a card from a hand
/// (`build_round`); no resolving link runs until the whole answer tree is
/// built. So a verdict that reads no hand field (`cond_reads_hand`, and any
/// residual wasm guard -- it may read anything) is stable for the window, and
/// only hand-sensitive verdicts are stamped with [`Self::hand_gen`].
struct ProbeMemo {
    /// Bumps whenever a declaration removes a card from a hand.
    hand_gen: u64,
    /// `(seat, card id) -> (hand_gen at probe, eligible, hand-sensitive)`.
    verdicts: std::collections::HashMap<(usize, String), (u64, bool, bool)>,
    /// Lazily built CEL scope, stamped with the `hand_gen` its `_hand` table
    /// reflects. A probe that does not call `hand(p)` accepts a stale scope --
    /// its window inputs are unchanged, and the candidate overlay is rebuilt
    /// per probe from the live world.
    scope: Option<(u64, WindowScope)>,
}

impl ProbeMemo {
    fn new() -> Self {
        Self {
            hand_gen: 0,
            verdicts: std::collections::HashMap::new(),
            scope: None,
        }
    }

    fn on_declaration(&mut self) {
        self.hand_gen += 1;
    }

    /// Drop the shared scope. A join round presents a different `target` to
    /// every visited seat, so the built scope's own window inputs do change
    /// between visits (design G06 §6.4).
    fn invalidate_scope(&mut self) {
        self.scope = None;
    }

    /// The shared window scope, built only when some candidate actually has a
    /// condition (docs/GUARDS.md §4.4 item 1). `need_fresh_hand` forces a
    /// rebuild when the probe reads `hand(p)` and a declaration has moved the
    /// hand table since the scope was built.
    fn scope_for<S: crate::condition::SnapSrc>(
        &mut self,
        world: &S,
        need_fresh_hand: bool,
    ) -> &WindowScope {
        let fresh = self
            .scope
            .as_ref()
            .is_some_and(|(g, _)| *g == self.hand_gen);
        if self.scope.is_none() || (need_fresh_hand && !fresh) {
            let win = crate::condition::fill_window(world);
            self.scope = Some((self.hand_gen, crate::condition::window_scope(&win)));
        }
        &self.scope.as_ref().expect("just built").1
    }
}

/// The one-line description of the answered link the counteraction prompt shows.
///
/// The key is kind-specific so the client can say **what** is being answered
/// (「{{who}} 打出了「{{card}}」」 / 「{{who}} 将支付 {{n}} 给 {{to}}」 /
/// 「{{who}} 将移动 {{n}} 格」) instead of a bare 「{{who}} 的触发」.
/// `ask.counteract.detail` remains the fallback for the long tail of kinds -- and
/// for prompts already on the wire, so old replays keep rendecycle.
fn describe_trigger(t: &Trigger) -> Msg {
    let effect_kind = t.effects.first().map(|e| e.kind.as_str()).unwrap_or("");
    let card = t.card.as_deref().unwrap_or("");
    let mut m = match t.kind {
        // A play (L1) or a declared counter -- both are `card` links.
        TriggerKind::Card if !card.is_empty() => Msg::new("ask.counteract.detail.play"),
        TriggerKind::MoveBefore
        | TriggerKind::MoveAfter
        | TriggerKind::DiceRollBefore
        | TriggerKind::DiceRollResult => Msg::new("ask.counteract.detail.move"),
        // The counteraction key for a payment / a targeted effect is `effect`
        // (`docs` on `TriggerKind::Effect`); the declared effect's own kind
        // says which.
        TriggerKind::Effect if effect_kind == "pay" => {
            if t.pay.map(|p| p.is_rent).unwrap_or(false) && t.target >= 0 {
                Msg::new("ask.counteract.detail.rent")
            } else if t.target >= 0 {
                Msg::new("ask.counteract.detail.pay")
            } else {
                Msg::new("ask.counteract.detail.pay_out")
            }
        }
        TriggerKind::Effect if effect_kind != "" && !card.is_empty() => {
            Msg::new("ask.counteract.detail.effect")
        }
        _ => Msg::new("ask.counteract.detail"),
    };
    m = m.player_id("who", t.player_id);
    if !card.is_empty() {
        m = m.card("card", card);
    }
    if t.tile >= 0 {
        m = m.tile("tile", t.tile);
    }
    if t.target >= 0 {
        m = m.player_id("to", t.target);
    }
    m = m.i("n", t.value as i64);
    m
}

/// Folding unknown kinds to `None` would make `can_counteract` guards silently
/// never match.
fn trigger_kind(kind: &str) -> TriggerKind {
    TriggerKind::from_str(kind)
}

/// The buy price stages, in the order 支付阶段 2 / 4 / 5 run them
/// (`docs/PURCHASE.md`): `BuyAdd` (fixed ±) → `BuyMul` (×) → `BuySet` (free /
/// fixed), each floored at 0.
const BUY_STAGES: [TriggerKind; 3] = [
    TriggerKind::BuyAdd,
    TriggerKind::BuyMul,
    TriggerKind::BuySet,
];

/// The bridge trigger a buy hook sees (`docs/PURCHASE.md`): the kind, who is
/// buying what from whom, and the running price the stage rewrites with
/// `set_price`. `value` mirrors `price` so a generic reader sees the figure.
fn buy_trigger(
    kind: TriggerKind,
    q: &game_core::engine::purchase::BuyQuery,
    t: usize,
    price: i32,
    st: &game_core::state::MatchState,
    activation_ordinal: i32,
) -> Trigger {
    Trigger {
        kind,
        player_id: q.player as i32,
        tile: t as i32,
        value: price,
        step: st.step,
        // The quote runs inside the buy's own activation, so a hook that reads
        // `trigger::activation_ordinal()` sees the same one the commit will.
        activation_ordinal,
        buy: Some(TriggerBuy {
            kind: card_sdk::abi::BuyKind::from_i32(q.kind.as_i32())
                .unwrap_or(card_sdk::abi::BuyKind::Land),
            seller: game_core::engine::rules::Payee::from_i32(q.seller),
            price,
            deal_owner: Some(game_kinds::Payee::Seat(game_kinds::Seat(q.player as i32))),
            deal_houses: Some(st.houses.get(t).copied().unwrap_or(0)),
            deal_mortgaged: st.mortgaged.get(t).copied().unwrap_or(false),
        }),
        ..Trigger::default()
    }
}

impl<M: CardModules> RulesBridge<M> {
    /// Stamp the envelope every body carries: the live step and the running
    /// activation. Bodies are built outside `counteract` (a play, a tile
    /// settle, an event rule), so a body trigger and the raise of the same
    /// activation must agree on the ordinal here.
    fn stamp(&self, cx: &PlayCtx, t: &mut crate::world::Trigger) {
        t.step = cx.state().step;
        t.activation_ordinal = cx.world().turn.activation_ordinal;
    }

    /// The live instances that could carry a buy hook: each player's field
    /// instances in placement order, then the board field (the tile / event /
    /// mark rules), then the turn's **lingering** instances -- the hand-card
    /// home for 「本回合」 buy effects (`docs/PURCHASE.md` P5). The order is the
    /// `counteract` dispatch's, so a quote and the commit see the same chain.
    ///
    /// Only instances whose rule actually declares a buy hook come back, so
    /// the cheap path can ask this once and skip the whole machinery when it is
    /// empty (the `cant_play` shape: nothing is instantiated for a question
    /// nobody answers).
    fn buy_hook_instances(&self, world: &game_core::engine::World) -> Vec<(i32, i32, String)> {
        // Fix C: the manifest's declared kinds first. No card in the set hooks
        // a buy kind -> no live instance can either, so skip the walk.
        if !self.ruleset.declares_buy() {
            return Vec::new();
        }
        let declares = |id: &str| {
            self.ruleset.card(id).is_some_and(|i| {
                crate::host::BUY_HOOK_KINDS
                    .iter()
                    .any(|&k| self.ruleset.hooks_to(i, k))
            })
        };
        let mut keyed: Vec<((u8, u32, u32), (i32, i32, String))> = Vec::new();
        let mut push = |key: (u8, u32, u32), item: (i32, i32, String), out: &mut Vec<_>| {
            if declares(&item.2) {
                out.push((key, item));
            }
        };
        for p in 0..world.player_count() {
            // B2 (`PIPELINE-AUDIT` K6/K14) -- 规则书 L81 「所有其正在生效的卡，技能
            // 效果停止生效」: a bankrupt / left seat's instances never answer.
            // (`remove_from_game` also clears the field; this is the dispatch's
            // own guard for anything a later path re-places.)
            if world.out(p) {
                continue;
            }
            for (n, (uid, id)) in world.field_instances(p as i32).into_iter().enumerate() {
                let group = source_group(world, uid);
                // Q5: (group, source = player-major placement, decl = 0).
                let key = effect_order_key(group, (p as u32) << 16 | n as u32, 0);
                let id = world.effective_card_at(uid).unwrap_or(id);
                push(key, (uid, p as i32, id), &mut keyed);
            }
        }
        for (n, (uid, id)) in world
            .field_instances(game_core::state::BOARD_OWNER)
            .into_iter()
            .enumerate()
        {
            let key = effect_order_key(LookupGroup::Board, n as u32, 0);
            let id = world.effective_card_at(uid).unwrap_or(id);
            push(key, (uid, game_core::state::BOARD_OWNER, id), &mut keyed);
        }
        for (n, l) in world.turn.lingering.iter().enumerate() {
            let key = effect_order_key(LookupGroup::Field, (l.owner as u32) << 16 | n as u32, 0);
            push(key, (-1, l.owner, l.card.clone()), &mut keyed);
        }
        keyed.sort_by_key(|(k, _)| *k);
        keyed.into_iter().map(|(_, item)| item).collect()
    }

    /// One hook, run in **pure guard mode** against `world` -- the `cant_play`
    /// shape: a throwaway copy, so the run cannot change the match and never
    /// prompts. What survives is only the trigger the hook rewrote (the price
    /// stages' `set_price`, a `BuyGate`'s `set_cancelled`); the world the run
    /// produced is dropped.
    ///
    /// A body that would prompt or ask the host contributes nothing here: a
    /// quote is a preview, and the commit re-quotes, so such a hook is answered
    /// (if at all) on the real run and not on a price the UI is merely showing.
    fn pure_buy_hook(
        &self,
        run: &mut Run,
        uid: i32,
        owner: i32,
        card: &str,
        kind: TriggerKind,
        trig: &mut Trigger,
    ) {
        let Some(idx) = self.ruleset.card(card) else {
            return;
        };
        if !self.ruleset.hooks_to(idx, kind) {
            return;
        }
        // The run is a throwaway: only the trigger it rewrote is read back, and
        // the world `run_hook` produces is dropped. `run_hook` clones the run
        // into the store it starts up, so the caller shares one `Run` across the
        // whole quote rather than cloning per hook.
        run.trigger = trig.clone();
        run.current_card = card.to_string();
        run.current_uid = uid;
        // `None` for `on_body`: a quote is a preview, never an activation --
        // the commit's own re-quote is the run that may flash.
        if let Ok(Some(hr)) = self.ruleset.run_hook(
            &*run,
            Call::Hook {
                card: idx,
                kind,
                player_id: owner,
            },
            &Default::default(),
            &[],
            None,
            // A quote's world is dropped, so a trap line has no home here;
            // the commit's re-quote runs the real (logging) drive.
            None,
        ) {
            if let Outcome::Done(after, _) = hr.outcome {
                *trig = after.trigger;
            }
        }
    }

    /// Shared instance-walk skeleton for the four deed ops. Empty-instances
    /// path is `standalone`; otherwise one `Trigger` is built, each bound
    /// instance is driven in order (`call(card_idx)`), cancelled links stop
    /// the walk, and `ok(before, after)` over `snap` decides success.
    fn drive_tile_op<T: Copy>(
        &self,
        cx: &mut PlayCtx,
        player_id: usize,
        tile: usize,
        reason: i32,
        standalone: impl FnOnce(&mut PlayCtx, usize, usize) -> Flow<bool>,
        call: impl Fn(i32) -> Call,
        prepare: impl FnOnce(&mut Trigger, &game_core::state::MatchState),
        snap: impl Fn(&game_core::state::MatchState) -> T,
        ok: impl FnOnce(T, T) -> bool,
    ) -> Flow<bool> {
        let instances = cx.world().tile_rule_instances(tile as i32);
        if instances.is_empty() {
            return standalone(cx, player_id, tile);
        }
        // `position` is the trigger's slot fact; the owner is a **tile** fact
        // (`owners[tile]`), and the two differ under a swap (设计 5.1).
        let position = cx.position_of_tile(tile as i32);
        let owner = cx.state().owners.get(tile).copied().unwrap_or(-1);
        let before = snap(cx.state());
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: owner,
            tile: tile as i32,
            position,
            value: reason,
            step: cx.state().step,
            ..Trigger::default()
        };
        self.stamp(cx, &mut trigger);
        prepare(&mut trigger, cx.state());
        for (uid, id) in instances {
            let Some(idx) = self.ruleset.card(&id) else {
                continue;
            };
            self.drive(cx, call(idx), &id, uid, &mut trigger)?;
            if trigger.is_cancelled() {
                break;
            }
        }
        let after = snap(cx.state());
        Ok(ok(before, after))
    }
}

impl<M: CardModules> CardRules for RulesBridge<M> {
    fn gate_open(
        &self,
        w: &game_core::engine::World,
        gate: game_core::engine::Gate,
        player: usize,
    ) -> Option<bool> {
        let kind = crate::gate::kind_of(gate);
        // No card declares this gate: the ruleset has no opinion.
        if !self.ruleset.declares(kind) {
            return None;
        }
        let win = match gate {
            game_core::engine::Gate::Abnormal {
                kind,
                by,
                exemption,
            } => crate::gate::window_of_abnormal(w, player, kind, by, exemption),
            _ => crate::gate::window_of_world(w, player),
        };
        Some(self.ruleset.gate_open(kind, &win))
    }
    fn exclusive_use(
        &self,
        w: &game_core::engine::World,
        player: usize,
        ask: &game_core::engine::ExclusiveAsk<'_>,
    ) -> Option<bool> {
        let kind = crate::TriggerKind::ExclusiveUse;
        // No card declares this gate: the ruleset has no opinion (the engine's
        // own/base default stands).
        if !self.ruleset.declares(kind) {
            return None;
        }
        let win = crate::gate::window_of_exclusive(w, &self.data, player, ask);
        Some(self.ruleset.gate_open(kind, &win))
    }
    fn gate_refusal_msg(&self, gate: game_core::engine::Gate, _player: usize) -> Msg {
        let kind = crate::gate::kind_of(gate);
        // The gate-declaring rule supplies the i18n key as data.
        for c in self.ruleset.cards() {
            if let Some((_, key)) = c.gate_msgs.iter().find(|(k, _)| *k == kind.id) {
                return Msg::new(key);
            }
        }
        Msg::new("status.move_blocked")
    }
    fn gate_skip_msg(&self, gate: game_core::engine::Gate, _player: usize) -> Msg {
        let kind = crate::gate::kind_of(gate);
        for c in self.ruleset.cards() {
            if let Some((_, key)) = c.gate_skip_msgs.iter().find(|(k, _)| *k == kind.id) {
                return Msg::new(key);
            }
        }
        self.gate_refusal_msg(gate, _player)
    }
    fn card_act_exemption(&self, card: &str) -> bool {
        // A status host may name a card prop that exempts a play from the act
        // gate. The prop name is data from that host's `exemption_prop`.
        let prop = self
            .ruleset
            .cards()
            .iter()
            .find(|c| !c.exemption_prop.is_empty())
            .map(|c| c.exemption_prop.clone());
        match prop {
            Some(prop) => self.ruleset.card_prop(card, &prop) != 0,
            None => false,
        }
    }
    fn ruleset_sha256(&self) -> Option<&str> {
        self.ruleset.sha256()
    }

    /// The card rule's declared static properties (`CardDef::props`), see
    /// [`game_core::engine::CardRules::card_props`].
    fn card_props(&self, card: &str) -> std::collections::BTreeMap<String, i32> {
        self.ruleset.card_props(card)
    }

    /// One prop without cloning the card's whole table (the trait default
    /// goes through [`Self::card_props`]).
    fn card_prop(&self, card: &str, key: &str) -> i32 {
        self.ruleset.card_prop(card, key)
    }

    /// Would a bot offer it? Static card fact: `prop::AI_NEVER`.
    fn ai_play(&self, _cx: &PlayCtx, _player: usize, card: &str) -> bool {
        self.ruleset
            .card_prop(card, game_core::state::prop::AI_NEVER)
            == 0
    }

    /// The card rule's declared **tile texts** (`On::texts`), see
    /// [`game_core::engine::CardRules::card_texts`]. Sorted by entry index
    /// (stable, so declaration order within an entry is kept) -- that order is
    /// the third component of the view's text-precedence key.
    fn card_texts(&self, card: &str) -> Arc<[game_core::engine::TileTextDecl]> {
        if let Some(t) = self.texts.read().ok().and_then(|m| m.get(card).cloned()) {
            return t;
        }
        let mut texts = self.ruleset.card_texts(card);
        texts.sort_by_key(|t| t.entry);
        let built: Arc<[game_core::engine::TileTextDecl]> = texts
            .into_iter()
            .map(|t| game_core::engine::TileTextDecl {
                slot: t.slot,
                tile: t.tile,
                key: t.key,
            })
            .collect();
        if let Ok(mut m) = self.texts.write() {
            m.insert(card.to_string(), built.clone());
        }
        built
    }

    fn has_rule(&self, id: &str) -> bool {
        self.ruleset.card(id).is_some()
    }

    /// [`game_core::engine::CardRules::tile_rule_id`] -- from the ruleset's
    /// `CardDef::governs` / `board_role` table (ABI v66).
    fn tile_rule_id(&self, kind: &str) -> String {
        self.ruleset.tile_rule_id(kind)
    }

    fn rule_ids(&self) -> Vec<String> {
        self.ruleset.rule_ids()
    }

    fn mark_rule_ids(&self) -> Vec<String> {
        self.ruleset.mark_rule_ids()
    }

    fn status_rule_ids(&self) -> Vec<String> {
        self.ruleset.status_rule_ids()
    }

    /// [`game_core::engine::CardRules::normal`] -- a card with a rule but no
    /// `On::Play` entry (a pure counteraction / hook card) is not a normal hand card
    /// and cannot be played from hand. A card without a rule keeps the default.
    fn normal(&self, card: &str) -> bool {
        self.ruleset
            .card(card)
            .is_none_or(|idx| self.ruleset.has_play_entry(idx))
    }

    /// [`game_core::engine::CardRules::has_play`] -- the rule's `On::Play`
    /// entry. Passive / hook-only skills never appear in a skill list.
    fn has_play(&self, card: &str) -> bool {
        self.ruleset
            .card(card)
            .is_some_and(|idx| self.ruleset.has_play_entry(idx))
    }

    /// The settle body (`docs/TILES.md`): run the tile's **rule instances**
    /// (board-owned, placed by `bind_tiles`) in instance order. Falls back to
    /// the engine's built-in `land_at` body for a tile with none -- `StubRules`
    /// binds nothing, and a kind with no rule in this ruleset has no instance.
    fn settle_tile(
        &self,
        cx: &mut PlayCtx,
        player_id: usize,
        position: usize,
        tile: i32,
        m: &game_core::engine::MoveCtx,
        is_main_move: bool,
    ) -> Flow<()> {
        let instances = cx.world().tile_rule_instances(tile);
        if instances.is_empty() {
            return cx.land_at_built_in(player_id, position, tile, is_main_move);
        }
        if !cx.prep_land(player_id, position, is_main_move) {
            return Ok(());
        }
        // `owners` is **tile**-indexed, not slot-indexed: under a swap the
        // settled slot and the governing tile differ, and the body wants the
        // tile's deed owner (设计 5.1).
        let owner = cx
            .state()
            .owners
            .get(tile.max(0) as usize)
            .copied()
            .unwrap_or(-1);
        // `On::Settle` dispatches by entry kind (`Call::Settle`), not by a
        // trigger kind, so the link's kind is `None` -- it is the body of the
        // settle the engine already raised (`settle` / `settleBefore` /
        // `settleAfter` frame it), not a named point of its own. `docs/TILES.md`.
        // No move payload (`mv: None`): the tile body reads `main`, not a move.
        // (The old shape stamped `move_settles: true` with `move_kind: None`;
        // `move_settles` now lives on the move payload and reads false when
        // there is no move.)
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: owner,
            tile,
            position: position as i32,
            step: cx.state().step,
            is_main_move,
            ..Trigger::default()
        };
        let world = cx.world_copy();
        self.stamp(cx, &mut trigger);
        for (uid, id) in instances {
            // The tile's own rule always runs; a suppressed tile hides its
            // (board-owned) extras.
            if !id.starts_with("tile:")
                && extra_suppressed(&world, tile, Some(m.tags.as_slice()), uid)
            {
                continue;
            }
            let Some(idx) = self.ruleset.card(&id) else {
                continue;
            };
            self.drive(
                cx,
                Call::Settle {
                    card: idx,
                    player_id: player_id as i32,
                },
                &id,
                uid,
                &mut trigger,
            )?;
            if trigger.is_cancelled() {
                break;
            }
        }
        Ok(())
    }

    /// [`game_core::engine::CardRules::deed_mortgage`] -- run the tile rule's
    /// `On::Mortgage` body. Falls back to the engine's standing mortgage for a
    /// tile with no rule instance (StubRules).
    fn deed_mortgage(
        &self,
        cx: &mut PlayCtx,
        player_id: usize,
        tile: usize,
        reason: i32,
    ) -> Flow<bool> {
        self.drive_tile_op(
            cx,
            player_id,
            tile,
            reason,
            game_core::engine::purchase::standalone_mortgage,
            |card| Call::Mortgage {
                card,
                player_id: player_id as i32,
                tile: tile as i32,
                reason,
            },
            |_, _| {},
            |st| st.mortgaged.get(tile).copied().unwrap_or(false),
            |before, after| !before && after,
        )
    }

    /// [`game_core::engine::CardRules::deed_redeem`] -- run the tile rule's
    /// `On::Redeem` body. Falls back to the engine's standing redeem.
    fn deed_redeem(&self, cx: &mut PlayCtx, player_id: usize, tile: usize) -> Flow<bool> {
        self.drive_tile_op(
            cx,
            player_id,
            tile,
            0,
            game_core::engine::purchase::standalone_redeem,
            |card| Call::Redeem {
                card,
                player_id: player_id as i32,
                tile: tile as i32,
            },
            |_, _| {},
            |st| st.mortgaged.get(tile).copied().unwrap_or(true),
            |before, after| before && !after,
        )
    }

    /// [`game_core::engine::CardRules::deed_buy`] -- run the tile rule's
    /// `On::Buy` body. Falls back to the engine's standing buy for a tile with
    /// no rule instance (StubRules).
    fn deed_buy(
        &self,
        cx: &mut PlayCtx,
        player_id: usize,
        tile: usize,
        kind: game_core::engine::purchase::BuyKind,
        reason: i32,
    ) -> Flow<bool> {
        self.drive_tile_op(
            cx,
            player_id,
            tile,
            reason,
            |cx, p, t| game_core::engine::purchase::standalone_buy(cx, p, t, kind),
            |card| Call::Buy {
                card,
                player_id: player_id as i32,
                tile: tile as i32,
                kind: kind.as_i32(),
                reason,
            },
            |trigger, st| {
                trigger.buy = Some(crate::world::TriggerBuy {
                    kind: card_sdk::abi::BuyKind::from_i32(kind.as_i32())
                        .unwrap_or(card_sdk::abi::BuyKind::Land),
                    seller: game_core::engine::rules::Payee::from_i32(-1),
                    price: 0,
                    deal_owner: Some(game_kinds::Payee::Seat(game_kinds::Seat(player_id as i32))),
                    deal_houses: Some(st.houses.get(tile).copied().unwrap_or(0)),
                    deal_mortgaged: false,
                });
            },
            |st| st.owners.get(tile).copied().unwrap_or(-1),
            |before, after| before != after && after == player_id as i32,
        )
    }

    /// [`game_core::engine::CardRules::deed_build`] -- run the tile rule's
    /// `On::Build` body. Falls back to the engine's standing build.
    fn deed_build(&self, cx: &mut PlayCtx, player_id: usize, tile: usize) -> Flow<bool> {
        self.drive_tile_op(
            cx,
            player_id,
            tile,
            0,
            game_core::engine::purchase::standalone_build,
            |card| Call::Build {
                card,
                player_id: player_id as i32,
                tile: tile as i32,
                reason: 0,
            },
            |_, _| {},
            |st| st.houses.get(tile).copied().unwrap_or(0),
            |before, after| after > before,
        )
    }

    fn cant_play(&self, cx: &PlayCtx, player_id: usize, card: &str) -> Option<Msg> {
        // A pure query ("why can this not be played?"): a guard that prompts,
        // or a module that fails, never blocks play. Fix B -- the probe shares
        // the live world handle (`share_world`, no copy); the gate body
        // detaches a private copy only if it writes.
        let idx = self.ruleset.card(card)?;
        // Fix A cheap pre-filter: no `On::Play` entry, or G4-deleted gate with
        // no condition -- the verdict is always "playable". Skip the uid
        // lookup, the `Run` and the CEL scope entirely (the condition-only /
        // declares-bitmask shape REPORT.md's A asks for).
        if self.ruleset.play_gate_vanishes(idx) {
            return None;
        }
        // Bind the instance when the card is already on the field (a skill
        // press), so `is_placed` / its named counters answer for it and not for a void
        // `current_uid = -1`. A read of the live world -- no copy. A marked
        // instance answers for its **effective** card too, so playing into it
        // asks that surface's Play gate.
        let uid = cx
            .world()
            .field_instances(player_id as i32)
            .into_iter()
            .find(|(uid, _)| cx.world().effective_card_at(*uid).as_deref() == Some(card))
            .map_or(-1, |(uid, _)| uid);
        let run = Run {
            world: cx.share_world(),
            pass_base: Default::default(),
            overlay_floor: 0,
            drive_id: 0,
            pile_checkpoints: Default::default(),
            data: self.data.clone(),
            props: self.modules_props(),
            trigger: Trigger::default(),
            fig: None,
            ledger_exec: 0,
            activation_player: player_id as i32,
            current_card: card.to_string(),
            current_uid: uid,
            carried_owner: None,
            dest: DEST_UNSET,
            dest_to: None,
            dest_cause: None,
            reinserted: vec![],
            paid_log: vec![],
            discard_log: vec![],
            draw_log: vec![],
            draw_after_log: vec![],
            frames_at_drive_start: vec![],
            adopted: vec![],
            hand_log: vec![],
            reshuffle_log: vec![],
            marker_spent_log: vec![],
            marker_changed_log: vec![],
            house_log: vec![],
            counter_log: vec![],
            named_raise_log: vec![],
            state_log: vec![],
            doubled: -1,
            linger_props: Default::default(),
            wrap_effect: false,
            live: None,
            posted: vec![],
            log_index: 0,
            place_log: vec![],
            place_index: 0,
            logged: 0,
            silent_body: false,
            quiet_when_empty: false,
            pending_header: None,
            own_activation: -1,
            header_ids: vec![],
            prompted: false,
            parent: -1,
            flash_policy: card_sdk::abi::Flash::Auto,
            flash_cued: false,
            flash_ordinal: 0,
        };
        self.ruleset
            .cant_play(&run, idx, player_id as i32)
            .ok()
            .flatten()
    }

    /// Collect the activation argument before the play commits
    /// (docs/ACTIVATION.md §2). Runs the entry's `OP_DECLARE` in a loop over
    /// one answer cursor -- a two-step declare (pick, then confirm) is one
    /// cursor, both prompts advance it. `Ok(None)` = declined and the card
    /// must not leave the hand.
    fn declare_activation(
        &self,
        cx: &mut PlayCtx,
        player_id: usize,
        card: &str,
    ) -> Flow<Option<Vec<u8>>> {
        let Some(idx) = self.ruleset.card(card) else {
            return Ok(Some(Vec::new()));
        };
        let uid = cx
            .world()
            .field_instances(player_id as i32)
            .into_iter()
            .find(|(_, id)| id == card)
            .map_or(-1, |(uid, _)| uid);
        // One cursor across every `OP_DECLARE` step of this activation: the
        // collector re-runs on each pause and reads the shared log in order.
        let mut answers: Vec<i32> = Vec::new();
        loop {
            let run = Run {
                world: cx.share_world(),
                pass_base: Default::default(),
                overlay_floor: 0,
                drive_id: 0,
                pile_checkpoints: Default::default(),
                data: self.data.clone(),
                props: self.modules_props(),
                trigger: Trigger::default(),
                fig: None,
                ledger_exec: 0,
                activation_player: player_id as i32,
                current_card: card.to_string(),
                current_uid: uid,
                carried_owner: None,
                dest: DEST_UNSET,
                dest_to: None,
                dest_cause: None,
                reinserted: vec![],
                paid_log: vec![],
                discard_log: vec![],
                draw_log: vec![],
                draw_after_log: vec![],
                frames_at_drive_start: vec![],
                adopted: vec![],
                hand_log: vec![],
                reshuffle_log: vec![],
                marker_spent_log: vec![],
                marker_changed_log: vec![],
                house_log: vec![],
                counter_log: vec![],
                named_raise_log: vec![],
                state_log: vec![],
                doubled: -1,
                linger_props: Default::default(),
                wrap_effect: false,
                live: None,
                posted: vec![],
                log_index: 0,
                place_log: vec![],
                place_index: 0,
                logged: 0,
                silent_body: false,
                quiet_when_empty: false,
                pending_header: None,
                own_activation: -1,
                header_ids: vec![],
                prompted: false,
                parent: -1,
                flash_policy: card_sdk::abi::Flash::Auto,
                flash_cued: false,
                flash_ordinal: 0,
            };
            // The play path declares the card's `On::Play` entry -- the press
            // surface, `OnKind::Play` (docs/ACTIVATION.md §2).
            let entry = self
                .ruleset
                .cards()
                .get(idx as usize)
                .and_then(|c| c.entry(card_sdk::abi::OnKind::Play, None))
                .unwrap_or(0);
            match self
                .ruleset
                .declare_entry(&run, idx, entry, player_id as i32, &answers)
            {
                Ok(DeclareOut::Arg(a)) => return Ok(Some(a)),
                Ok(DeclareOut::Declined) => return Ok(None),
                Ok(DeclareOut::Paused(p)) => {
                    let ask = prompt_to_ask(p);
                    let reply = cx.ask(ask)?;
                    let a = reply
                        .answered
                        .answers
                        .first()
                        .copied()
                        .unwrap_or(reply.fallback);
                    answers.push(a);
                }
                // Same as `declare_for_card`: never proceed on a made-up arg.
                Err(e) => {
                    cx.log(
                        player_id as i32,
                        Msg::new("log.card_trap")
                            .card("card", card)
                            .text("detail", crate::host::trap_detail(&e)),
                    );
                    return Ok(None);
                }
            }
        }
    }

    /// The purchase quote (`docs/PURCHASE.md`): what would `q.player` be
    /// charged for each tile, and may they buy it at all?
    ///
    /// Runs the `BuyGate` / `BuyAdd` → `BuyMul` → `BuySet` hooks in pure guard
    /// mode against a world copy (like [`Self::cant_play`]), **only** when a
    /// hooking instance exists -- otherwise the plain rulebook formula answers
    /// and nothing is instantiated, which is what `StubRules` and the sim run.
    /// The commit re-quotes ([`game_core::engine`]'s `buy`), so the quoted price
    /// is the price charged.
    fn buy_quote(
        &self,
        w: &game_core::engine::World,
        data: &GameData,
        q: &game_core::engine::purchase::BuyQuery,
    ) -> Vec<game_core::engine::purchase::Quote> {
        use game_core::engine::purchase::{base_quote, Quote};
        let st = &w.st;
        let native = |t: usize| {
            let price = base_quote(data, st, t, q.kind);
            Quote {
                price,
                eligible: price >= 0,
            }
        };
        // Cheap path (fix C): the manifest's declared kinds say no card in the
        // set hooks a buy kind, so no live instance can either -- the quote is
        // the native formula and neither the instance walk nor a module runs.
        if !self.ruleset.declares_buy() {
            return q.tiles.iter().map(|&t| native(t)).collect();
        }
        // Cheap path: no live instance declares a buy hook, so the quote is the
        // native formula and no module is started up.
        let instances = self.buy_hook_instances(w);
        if instances.is_empty() {
            return q.tiles.iter().map(|&t| native(t)).collect();
        }
        // Only the stages somebody actually declares are run -- a `BuySet`
        // nobody hooks is a no-op that would cost a linker init per tile. The
        // per-card `hook_mask` shift (fix C), not an entry-table scan.
        let declares = |kind: TriggerKind| {
            instances.iter().any(|(_, _, card)| {
                self.ruleset
                    .card(card)
                    .is_some_and(|i| self.ruleset.hooks_to(i, kind))
            })
        };
        let gate_decl = declares(TriggerKind::BuyGate);
        // One throwaway run for the whole quote (fix B): the world it carries
        // is a COW handle to `w` (one deep copy per quote batch, not per hook);
        // `run_hook`'s per-hook store clone is a refcount bump, and a hook body
        // that writes detaches its own copy which is then dropped.
        let mut run = Run {
            world: game_core::engine::SharedWorld::new(w.clone()),
            pass_base: Default::default(),
            overlay_floor: 0,
            drive_id: 0,
            pile_checkpoints: Default::default(),
            data: self.data.clone(),
            props: self.modules_props(),
            trigger: Trigger::default(),
            fig: None,
            ledger_exec: 0,
            activation_player: -1,
            current_card: String::new(),
            current_uid: -1,
            carried_owner: None,
            dest: DEST_UNSET,
            dest_to: None,
            dest_cause: None,
            reinserted: vec![],
            paid_log: vec![],
            discard_log: vec![],
            draw_log: vec![],
            draw_after_log: vec![],
            frames_at_drive_start: vec![],
            adopted: vec![],
            hand_log: vec![],
            reshuffle_log: vec![],
            marker_spent_log: vec![],
            marker_changed_log: vec![],
            house_log: vec![],
            counter_log: vec![],
            named_raise_log: vec![],
            state_log: vec![],
            doubled: -1,
            linger_props: Default::default(),
            wrap_effect: false,
            live: None,
            posted: vec![],
            log_index: 0,
            place_log: vec![],
            place_index: 0,
            logged: 0,
            silent_body: false,
            quiet_when_empty: false,
            pending_header: None,
            own_activation: -1,
            header_ids: vec![],
            prompted: false,
            parent: -1,
            flash_policy: card_sdk::abi::Flash::Auto,
            flash_cued: false,
            flash_ordinal: 0,
        };
        q.tiles
            .iter()
            .map(|&t| {
                let base = base_quote(data, st, t, q.kind);
                if base < 0 {
                    return Quote {
                        price: -1,
                        eligible: false,
                    };
                }
                let mut at = |run: &mut Run, kind: TriggerKind, price: i32| {
                    let mut trig = buy_trigger(kind, q, t, price, st, w.turn.activation_ordinal);
                    for &(uid, owner, ref card) in &instances {
                        self.pure_buy_hook(run, uid, owner, card, kind, &mut trig);
                    }
                    trig
                };
                // `BuyGate` -- may this player buy this tile at all? Runs for
                // every [`card_sdk::abi::BuyKind`], Force included (Poppin's
                // hill lock), so a quote is never a price for an impossible buy.
                if gate_decl && at(&mut run, TriggerKind::BuyGate, base).is_cancelled() {
                    return Quote {
                        price: -1,
                        eligible: false,
                    };
                }
                // `BuyAdd` → `BuyMul` → `BuySet`, each floored at 0.
                //
                // 强制购买 is immune (rulebook 其他规则 6: 「抵押，赎回，和强制购买
                // 的资金变动不受任何效果影响，除非效果写明改动抵押，赎回，或强制
                // 购买」). The price is the printed 2×(price+houses) figure; a
                // generic buy-stage hook does not 「写明」 force-buy, so it must
                // not retune this quote. `BuyGate` still runs above (can-you-buy,
                // not a price). TODO(规则书): a hook that *does* write
                // 「强制购买」 as its target has no wire spelling yet -- when one
                // lands, only that entry may rewrite the Force base.
                let mut price = base;
                if q.kind != game_core::engine::purchase::BuyKind::Force {
                    for &kind in &BUY_STAGES {
                        if !declares(kind) {
                            continue;
                        }
                        price = at(&mut run, kind, price)
                            .buy
                            .map(|b| b.price)
                            .unwrap_or(price)
                            .max(0);
                    }
                }
                Quote {
                    price,
                    eligible: true,
                }
            })
            .collect()
    }

    fn play(&self, cx: &mut PlayCtx, player_id: usize, card: &str) -> Flow<Dest> {
        let Some(idx) = self.ruleset.card(card) else {
            cx.log(
                player_id as i32,
                Msg::new("log.card_not_ported").card("card", card),
            );
            return Ok(Dest::Graveyard);
        };
        // A skill press (`use_skill`) runs a *placed* card's play entry: bind
        // the instance so `is_placed` / its named counters read it and not a void
        // `current_uid = -1`. A hand play has no instance yet (uid -1). A marked
        // instance answers for its **effective** card: playing the stand-in on
        // it runs that surface and keeps the row bound.
        let uid = cx
            .world()
            .field_instances(player_id as i32)
            .into_iter()
            .find(|(uid, _)| cx.world().effective_card_at(*uid).as_deref() == Some(card))
            .map_or(-1, |(uid, _)| uid);
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            step: cx.state().step,
            // The card is playing itself, so it is its own cause.
            by_card: Some(player_id as i32),
            // A body's frames live and die with the drive that carried it.
            uid: cx.world_mut().next_trigger_uid(),
            ..Trigger::default()
        };
        self.stamp(cx, &mut trigger);
        // A `copies_previous` play (mortis) names the card it copies on its own
        // play link, so the body can play it with `trigger::link_copied`
        // (design G06 §2.15). The counteract half stamps the same field at the
        // counter declaration; without this the play half always read none.
        if self.ruleset.copies_previous(idx) {
            trigger.copied = Self::copy_source(cx, player_id);
        }
        push_resolving_frame(card, uid, player_id as i32, trigger.uid);
        let dest = match self.drive(
            cx,
            Call::Play {
                card: idx,
                player_id: player_id as i32,
                arg: cx.world().turn.play_arg.clone(),
            },
            card,
            uid,
            &mut trigger,
        ) {
            Ok(d) => {
                truncate_resolving(trigger.uid);
                d
            }
            Err(e) => {
                if !e.is_suspended() {
                    truncate_resolving(trigger.uid);
                }
                return Err(e);
            }
        };
        Ok(dest_from(dest))
    }

    fn event(&self, cx: &mut PlayCtx, player_id: usize, id: &str) -> Flow<bool> {
        // The rule id is `event:<id>` (`docs/EVENTS.md`); the engine binds its
        // instance on the board owner before this runs, so the body's
        // `On::Play` sees a live instance (`is_placed` / named counters / props).
        let rid = game_core::data::event_rule_id(id);
        let Some(idx) = self.ruleset.card(&rid) else {
            cx.log(
                player_id as i32,
                Msg::new("log.event_not_ported").event("event", id),
            );
            return Ok(false);
        };
        let uid = cx
            .world()
            .event_rule_instances()
            .into_iter()
            .find(|(_, c)| c == &rid)
            .map_or(-1, |(uid, _)| uid);
        let mut trigger = Trigger {
            kind: TriggerKind::None,
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            step: cx.state().step,
            uid: cx.world_mut().next_trigger_uid(),
            ..Trigger::default()
        };
        self.stamp(cx, &mut trigger);
        push_resolving_frame(&rid, uid, player_id as i32, trigger.uid);
        let dest = match self.drive(
            cx,
            Call::Play {
                card: idx,
                player_id: player_id as i32,
                arg: Vec::new(),
            },
            &rid,
            uid,
            &mut trigger,
        ) {
            Ok(d) => {
                truncate_resolving(trigger.uid);
                d
            }
            Err(e) => {
                if !e.is_suspended() {
                    truncate_resolving(trigger.uid);
                }
                return Err(e);
            }
        };
        Ok(dest == DEST_FIELD)
    }

    fn counteract(&self, cx: &mut PlayCtx, t: &mut CoreTrigger) -> Flow<()> {
        // One identity per raise, stamped at the choke every raise crosses so
        // a hook elsewhere in the raise can recognise the same occurrence. A
        // resumed half-trigger and a test-built trigger keep what they have.
        if t.uid == 0 {
            t.uid = cx.world_mut().next_trigger_uid();
        }
        // The activation this raise belongs to, stamped at the same choke so
        // a replayed routine reads its own moves back as not-spent. Nested
        // raises inherit the running value.
        t.activation_ordinal = cx.world().turn.activation_ordinal;
        // STACK-01: a nested settle suspends the drive and parks the rest of
        // this walk in `cx.counteract_resume`. Re-entry adopts that state so
        // every completed body runs exactly once.
        let parked = cx.take_counteract_resume();
        let parked_some = parked.is_some();
        let resumed = parked.and_then(|s| s.downcast::<CounteractResume>().ok().map(|b| *b));
        if resumed.is_none() {
            // Non-nested path: master's walk, unchanged.
            return self.counteract_orig(cx, t);
        }
        let from_resume = true;
        if !self.ruleset.declares(trigger_kind(t.kind.as_str())) {
            return Ok(());
        }
        let mut own_done = false;
        let mut hand_resume = None;
        let mut hook_jobs: Vec<HookJob> = Vec::new();
        let mut hook_index = 0usize;
        let mut resumed_trigger = None;
        let mut resumed_chain = None;
        let mut _join_resume = None;
        if let Some(r) = resumed {
            own_done = r.own_done;
            hand_resume = r.hand;
            hook_jobs = r.hooks;
            hook_index = r.hook_index;
            resumed_trigger = Some(r.trigger);
            resumed_chain = r.chain;
            _join_resume = r.join;
        }
        let mut trigger = match resumed_trigger {
            // Continuing the parked walk itself: its half-trigger is the link
            // being answered (`resolve_jobs` only ever parks `hand` there).
            // A **fresh** raise inside a suspended body is a different link and
            // must not borrow the parked one -- it carries its own subject
            // (`t.instance`, `t.card`), and the parked half-trigger would give
            // its hooks a `-1` (a deferred event's `eventResolved` then found
            // no placed instance and its `card.placed` guard never passed).
            // The raise's freshly stamped `uid` tells the two apart.
            Some(tr) if tr.uid == t.uid => tr,
            // A fresh raise inside a suspended body (a draw, a settle ...):
            // the parked trigger is the enclosing link, not this one -- open
            // the new window on the live raise instead, or its chain's
            // bookkeeping (answers / `answers_declared`) would not fit.
            _ => bridge_trigger(t),
        };

        let own = t.card.clone().unwrap_or_default();
        let own_play = matches!(
            trigger_kind(t.kind.as_str()),
            TriggerKind::Card | TriggerKind::Event
        );
        if !own_done && own_play && !own.is_empty() {
            if let Some((idx, entry)) = self.ruleset.card(&own).and_then(|i| {
                self.ruleset.cards()[i as usize]
                    .entry(card_sdk::abi::OnKind::Counteract, Some(trigger.kind))
                    .map(|e| (i, e))
            }) {
                push_resolving_frame(&own, -1, t.player_id, trigger.uid);
                match self.drive(
                    cx,
                    Call::Counteract {
                        card: idx,
                        player_id: t.player_id,
                        arg: Vec::new(),
                        entry: Some(entry),
                    },
                    &own,
                    -1,
                    &mut trigger,
                ) {
                    Err(e) if e.is_suspended() => {
                        // ResumeFnWork completes this drive; own is done.
                        cx.set_counteract_resume(Box::new(CounteractResume {
                            trigger: trigger.clone(),
                            own_done: true,
                            hand: None,
                            hooks: Vec::new(),
                            hook_index: 0,
                            chain: None,
                            join: None,
                        }));
                        return Err(e);
                    }
                    r => {
                        r?;
                    }
                }
            }
        }

        // (2) Hand counteraction window.
        match hand_resume {
            Some(hr) => {
                // Moved: the boxed chain keeps its address, so a continuation
                // parked inside the resumed walk still points into it.
                let mut chain = resumed_chain.take().unwrap_or_default();
                self.resolve_jobs(cx, &trigger, &mut chain, &hr.order, hr.next)?;
                if let Some(root) = chain.first() {
                    trigger = root.link.clone();
                }
                // A synchronous caller (the raise pump) still holds the walk's
                // Box and continues with it; hand the Vec back through the
                // installed cursor so it is not left empty. Without one (an
                // engine-level replay) the chain only has to stay alive.
                if let Some(c) = cx
                    .counteract_chain()
                    .and_then(|c| c.downcast_ref::<CounteractChainCursor>())
                {
                    unsafe {
                        *c.chain = std::mem::take(&mut *chain);
                    }
                } else {
                    cx.keep_chain(chain);
                }
            }
            None => {
                // A new raise inside a suspended body does not continue the
                // parked tree, but a raw chain cursor (a join continuation or
                // a settle drive that parked it) still indexes that tree.
                // Keep its allocation alive for the rest of the context --
                // the parked walk takes it back through the cursor.
                if let Some(c) = resumed_chain.take() {
                    let kept: Box<dyn std::any::Any + Send> = c;
                    cx.keep_chain(kept);
                }
                if !is_hook_only(t.kind.as_str()) {
                    // A suspend inside `hand_counteractions` / `resolve_jobs`
                    // parks a HandResume (see `resolve_jobs`).
                    self.hand_counteractions(cx, t, &mut trigger, 0)?;
                }
            }
        }

        // Snapshot hooks after the hand window (same point the original
        // phase-3 world copy was taken). A resume already carries its list.
        if !from_resume {
            hook_jobs = self.collect_hook_jobs(cx, t);
            hook_index = 0;
        }

        let kind = trigger.kind;
        if !trigger.is_cancelled() {
            if is_game_start_kind(kind) {
                self.game_start_hooks(cx, t.player_id, kind, &mut trigger)?;
            } else if is_own_card_kind(kind) {
                if let Some(card_id) = t.card.as_deref().filter(|s| !s.is_empty()) {
                    if let Some(idx) = self
                        .ruleset
                        .card(card_id)
                        .filter(|&i| self.ruleset.cards()[i as usize].hooks(kind))
                    {
                        match self.drive_hook(
                            cx,
                            Call::Hook {
                                card: idx,
                                kind,
                                player_id: t.player_id,
                            },
                            card_id,
                            -1,
                            &mut trigger,
                        ) {
                            Err(e) if e.is_suspended() => {
                                cx.set_counteract_resume(Box::new(CounteractResume {
                                    trigger: trigger.clone(),
                                    own_done: true,
                                    hand: None,
                                    hooks: hook_jobs,
                                    hook_index: hook_index + 1,
                                    chain: None,
                                    join: None,
                                }));
                                return Err(e);
                            }
                            r => {
                                r?;
                            }
                        }
                    }
                }
            } else {
                while hook_index < hook_jobs.len() {
                    let job = hook_jobs[hook_index].clone();
                    let resume = CounteractResume {
                        trigger: trigger.clone(),
                        own_done: true,
                        hand: None,
                        hooks: hook_jobs.clone(),
                        hook_index: hook_index + 1,
                        chain: None,
                        join: None,
                    };
                    self.run_hook_job(cx, &mut trigger, &job, resume)?;
                    hook_index += 1;
                }
            }
            // Scheduled turn-end callbacks (inside the original `!cancelled`).
            let phase = match trigger.kind {
                TriggerKind::TurnEndBefore => Some(true),
                TriggerKind::TurnEndAfter => Some(false),
                _ => None,
            };
            if let (Some(early), true) = (phase, t.player_id >= 0) {
                let ended = t.player_id as usize;
                let mut w = cx.world_copy();
                let mut due = Vec::new();
                w.scheduled.retain_mut(|s| {
                    if s.target != ended || s.early != early {
                        return true;
                    }
                    if s.skip {
                        s.skip = false;
                        return true;
                    }
                    due.push((s.card.clone(), s.owner, s.uid, s.entry.clone()));
                    false
                });
                cx.swap_world(w);
                for (id, owner, uid, entry) in due {
                    let resolved = self.ruleset.card(&id).and_then(|idx| {
                        let info = &self.ruleset.cards()[idx as usize];
                        let e = if entry.is_empty() {
                            // Legacy save: settles to the card's first
                            // Schedule entry (the old AtEnd behaviour).
                            info.entry(card_sdk::abi::OnKind::Schedule, None)
                        } else {
                            info.entry_by_id(&entry)
                        };
                        e.map(|e| (idx, e))
                    });
                    if let Some((idx, entry_index)) = resolved {
                        match self.drive(
                            cx,
                            Call::AtEnd {
                                card: idx,
                                player_id: owner,
                                entry: entry_index,
                            },
                            &id,
                            uid,
                            &mut trigger,
                        ) {
                            Err(e) if e.is_suspended() => {
                                cx.set_counteract_resume(Box::new(CounteractResume {
                                    trigger: trigger.clone(),
                                    own_done: true,
                                    hand: None,
                                    hooks: hook_jobs,
                                    hook_index,
                                    chain: None,
                                    join: None,
                                }));
                                return Err(e);
                            }
                            r => {
                                r?;
                            }
                        }
                    }
                }
            }
        }

        // A fold stage's figure is the composition on `value`; the move-face
        // mirror only ever carries the **last** responder's raw write, so on a
        // fold the composed `value` wins. Every other stage keeps the mirror (a
        // dice face is written through `set_move_roll`).
        t.value = if trigger.fold.is_fold() {
            trigger.value
        } else {
            trigger
                .mv
                .as_ref()
                .and_then(|m| m.roll)
                .unwrap_or(trigger.value)
        };
        // The composed multiplier of a `Fold::Mul` stage (the settle stages
        // read it back).
        t.fold_milli = trigger.fold_milli;
        if trigger.negation != Default::default() {
            t.negation = trigger.negation;
        }
        for s in trigger.spared {
            t.spare(s);
        }
        t.target = trigger.target;
        t.by_card_instance = trigger.by_card_instance;
        t.redirected_from = trigger.redirected_from;
        t.settle_facts = trigger.settle_facts;
        // The subject / join block / pending answers / copy source are link
        // state, not body output: carry whatever the walk left on them.
        t.subject_card = trigger.subject_card.clone();
        t.join = trigger.join.as_ref().map(unbridge_join);
        t.answers_declared = trigger.answers_declared.clone();
        t.copied = trigger.copied.clone();
        t.roll_root = trigger.roll_root;
        t.pending_negation = trigger.pending_negation;
        t.extreme_armed = trigger.extreme_armed;
        t.answered_kinds = trigger
            .answered_kinds
            .iter()
            .map(|k| k.spelling().into_owned())
            .collect();
        // The pay kind / shape rewrite (`set_pay_kind` / `set_pay_shape`) rides
        // the payment trigger; the engine folds it back before the shape is read.
        t.pay = trigger.pay.map(unbridge_pay);
        // The `BuyAssign` deal rewrite (`set_deal_owner` / `set_deal_houses`) rides
        // the buy trigger; the engine reads it back off `t.buy` at the commit.
        t.buy = trigger.buy.map(unbridge_buy);
        Ok(())
    }
}

/// Money-pipeline hook kinds: a hook of one of these kinds cannot re-trigger
/// on its own money movement (the termination argument for nested money).
fn is_money_hook_kind(kind: TriggerKind) -> bool {
    matches!(
        kind,
        TriggerKind::PaySplitAdd
            | TriggerKind::PaySplitMul
            | TriggerKind::PaySplitChoose
            | TriggerKind::PaySplitAt
            | TriggerKind::PayAfter
            | TriggerKind::PayCombinedAdd
            | TriggerKind::PayCombinedMul
            | TriggerKind::PayCombinedCancel
            | TriggerKind::RentAdd
            | TriggerKind::RentMul
            | TriggerKind::SettleAdd
            | TriggerKind::SettleMul
    )
}

/// `PIPELINE-AUDIT` Q5 -- the **lookup group** half of the deterministic
/// ordering key (EFFECT_ACTIVATION `E6`): sources answering the same window are
/// scanned in this order. `Board` is our addition for the neutral `tile:*` /
/// `event:*` / `mark:*` rules, which are not player sources; it sorts after the
/// player groups, matching the dispatch's "player fields, then the board" walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(crate) enum LookupGroup {
    Status = 0,
    Group = 1,
    Persona = 2,
    Field = 3,
    Hand = 4,
    Discard = 5,
    Deck = 6,
    Removed = 7,
    Board = 8,
}

/// The deterministic order key for two effect sources answering the same
/// window (`PIPELINE-AUDIT` Q5 / EFFECT_ACTIVATION `E6`/`E7`):
/// **(lookup group, source order, declaration order)**.
///
/// * **lookup group** -- [`LookupGroup`], the spec's scan order
///   status → group → persona → field → hand → discard → deck → removed.
/// * **source order** -- the candidate's index in that group's authoritative
///   state list: player-major then placement order for the field-ish groups
///   (`players[p].field`), the hand `Vec` index for `hand`, `board_field` order
///   for the board group.
/// * **declaration order** -- the entry index within the source's manifest
///   `on` list.
///
/// Pure: no `HashMap` iteration, no clock. Two runs of the same authoritative
/// state produce the same order. Field hooks before the hand counteraction
/// cycle is already group `Field`/`Group`/`Persona` < `Hand`, so wiring the
/// key does not move anything today (`docs/ENGINE.md` 「Effect ordering」).
pub(crate) fn effect_order_key(group: LookupGroup, source: u32, decl: u32) -> (u8, u32, u32) {
    (group as u8, source, decl)
}

/// The lookup group of one placed source (`LookupGroup::Group` for a group-skill
/// instance, `Persona` for a persona skill, `Field` for anything else).
/// Reads the instance's `group_skill` stamp (`GameData::is_group_skill`), which
/// is the authoritative classification.
fn source_group(world: &game_core::engine::World, uid: i32) -> LookupGroup {
    match world.field_by_uid(uid) {
        Some(f) if f.group_skill => LookupGroup::Group,
        Some(f) if f.card.starts_with("skill:") => LookupGroup::Persona,
        _ => LookupGroup::Field,
    }
}

/// Field-card (hook) hook points (ABI v17). These are **not** counteraction points:
/// the engine runs every *placed* card's `counteract` against them automatically,
/// with no player declaration. They are distinct `TriggerKind`s from the
/// counteraction kinds so a card can tell a field effect from a hand counteraction by its
/// kind alone.
/// Hook points that are *only* field-card points: no counteraction window opens at
/// them. (`turnStart` / `settleAfter` are both, so they are not in here.)
///
/// `PassTile` / `PassPlayer` are **not** in here: ABI v43 made them
/// [`ChainKind`]s too (「when you pass a persona」 -- `AG:刻入天穹傲岸的烈光`),
/// so the hand counteraction window opens at them. The valid-option-first pre-scan
/// skips the whole cycle when no seat holds a card that answers, which is the
/// common case on a `passTile` (it runs on every tile walked).
///
/// The flag itself is data: engine rows in `game_kinds`, ruleset rows declared
/// by the ruleset and read at load (`kind_flags`). The engine names no id.
fn is_hook_only(kind: &str) -> bool {
    crate::kind_flags::hook_only(trigger_kind(kind))
}

thread_local! {
    /// Every card body running right now, `(card id, instance uid, owner
    /// seat, raise uid)`, innermost last (see `push_resolving_frame`).
    /// Transient: a body pushes at its entry and its frame dies with the raise
    /// that carried it.
    ///
    /// The seat rides the frame because a frame is not necessarily placed
    /// under its uid (a play of a hand card is at `-1` until its own body
    /// places it), so a dispatcher could not look the owner up later. The
    /// raise uid is the frame's lifetime key: a body returns from its guest
    /// call *before* the hooks its money move raised are walked, so the body
    /// cannot pop its own frame on return -- the raise pops the frames it left
    /// behind. One host thread per match.
    static RESOLVING: std::cell::RefCell<Vec<(String, i32, i32, i32)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Drop every resolving frame. A new engine (a fresh match or replay worker)
/// starts with none -- the frames are a thread-local, so a match that was torn
/// down mid-body (a probe that stops on a paused drive) would otherwise hand
/// its frames to the next engine on the same thread.
pub fn reset_resolving() {
    RESOLVING.with(|r| r.borrow_mut().clear());
}

/// Snapshot of [`RESOLVING`] for the walk: `(card, uid, owner seat)`.
pub(crate) fn resolving_snapshot() -> Vec<(String, i32, i32)> {
    RESOLVING.with(|r| {
        r.borrow()
            .iter()
            .map(|(c, u, s, _)| (c.clone(), *u, *s))
            .collect()
    })
}

/// The innermost frame's owner seat, when there is one. A body's seat is the
/// seat its drive was dispatched for (`drive_hook` pushes it): the card's
/// holder for a field hook, the player of the play for a hand card. It is
/// **not** the raise's `player_id` -- on a payment that is the payer, not the
/// card's owner.
pub fn resolving_seat() -> Option<i32> {
    RESOLVING.with(|r| r.borrow().last().map(|f| f.2))
}

/// The frames as `(card, uid)` pairs -- what the `resolving(…)` window reads.
fn resolving_names() -> Vec<(String, i32)> {
    RESOLVING.with(|r| {
        r.borrow()
            .iter()
            .map(|(c, u, _, _)| (c.clone(), *u))
            .collect()
    })
}

/// Drop every frame opened by raise `raise_uid` (see [`RESOLVING`]).
fn truncate_resolving(raise_uid: i32) {
    RESOLVING.with(|r| {
        let mut st = r.borrow_mut();
        let before = st.len();
        st.retain(|(_, _, _, k)| *k != raise_uid);
    });
}

/// Push a body's frame unless it is already the top one: a body that parked a
/// host request re-enters, and its frame stayed up across the pause (the
/// effects it caused are settled on that stack).
pub fn push_resolving_frame(card: &str, uid: i32, seat: i32, raise_uid: i32) {
    RESOLVING.with(|r| {
        let mut st = r.borrow_mut();
        // Already resolving: the same instance cannot be resolving twice, and
        // a second walk re-reaching it must not stack a second frame. A hand
        // card is at `-1` for every copy, so nested plays of one id may share
        // it -- those keep a frame each.
        if uid >= 0 && st.iter().any(|(c, u, _, _)| c == card && *u == uid) {
            return;
        }
        st.push((card.to_string(), uid, seat, raise_uid));
    });
}

/// The seat a body runs for -- the `player_id` its call carries, which is the
/// card's owner for a field hook. `-1` for the calls with no seat.
fn drive_seat(call: &Call) -> i32 {
    match call {
        Call::Play { player_id, .. }
        | Call::Counteract { player_id, .. }
        | Call::Hook { player_id, .. } => *player_id,
        _ => -1,
    }
}

/// The ids of one of a seat's plain piles, top-first for the deck (`None` for
/// a seat with no hidden state). The `Inspection` frame is
/// [`inspection_held`].
fn hidden_pile(
    w: &game_core::engine::World,
    player_id: i32,
    pile: CardPile,
) -> Option<Vec<String>> {
    let h = w.hidden.get(player_id.max(0) as usize)?;
    Some(match pile {
        CardPile::Hand => h.hand.clone(),
        CardPile::Graveyard => h.graveyard.clone(),
        // The end of the vec is the top of the pile; list top first.
        CardPile::Deck => h.deck.iter().rev().cloned().collect(),
        _ => return None,
    })
}

/// The revealed cards a seat's open inspection frames hold, front-first
/// (innermost frame first). These are off the deck but still its top.
fn inspection_held(w: &game_core::engine::World, owner: i32) -> Vec<String> {
    w.inspection_frames
        .iter()
        .rev()
        .filter(|f| f.deck_owner == owner)
        .flat_map(|f| f.cards.iter())
        .filter(|c| c.revealed)
        .map(|c| c.id.clone())
        .collect()
}

/// Kinds that run on a fresh instance of one card (named on `t.card`) rather
/// than on the cards in play.
fn is_own_card_kind(kind: TriggerKind) -> bool {
    // `Drawn` is the drawn card's own hook (it is still in hand); `Inspected`
    // is the turned-up card's own hook (it sits in the frame). So are a hand
    // arrival (`HandChanged`) and the opening deal (`OpeningHandDealt`): both
    // name a card with no instance. `Discarded` is a field-card point
    // instead -- a placed card hears about discards.
    matches!(
        kind,
        TriggerKind::Drawn
            | TriggerKind::Inspected
            | TriggerKind::HandChanged
            | TriggerKind::OpeningHandDealt
    )
}

/// The match-start points. They reach **every effect source** of the player
/// the engine raised them for -- field cards including skills (in field order)
/// and the card ids in that player's piles/hands -- not just the card named on
/// `t.card`. See [`WasmRules::game_start_hooks`].
fn is_game_start_kind(kind: TriggerKind) -> bool {
    matches!(
        kind,
        TriggerKind::DeckBeforeGame | TriggerKind::DeckAtGameStart
    )
}

fn dest_from(v: i32) -> Dest {
    match v {
        1 => Dest::Hand,
        DEST_FIELD => Dest::Field,
        3 => Dest::Banished,
        _ => Dest::Graveyard,
    }
}

#[cfg(test)]
mod tests {
    use super::{chain_starter, trigger_kind, CoreTrigger, Priority};
    use crate::TriggerKind;

    #[test]
    fn a_round_belongs_to_the_player_who_activated_the_effect() {
        // Seat 2's card makes seat 0 pay: the `effect` is the payer's (player 0),
        // but seat 2 activated it, so the round is seat 2's -- and the cycle
        // starts at seat 2 (ruling 2026-10-07).
        let mut pay = CoreTrigger::new("effect", 0);
        pay.by_card = Some(2);
        assert_eq!(chain_starter(&pay, 1, 4), 2);
        // Rent is board-driven (`by_card` is None -- a system / tile event):
        // the link has no player, so the cycle starts at the active turn player
        // (ruling 2026-10-07), not at the payer (player 3).
        let rent = CoreTrigger::new("effect", 3);
        assert_eq!(chain_starter(&rent, 1, 4), 1);
        // A bank-side link with neither: whoever's turn it is.
        let mut bank = CoreTrigger::new("effect", 0);
        bank.player_id = -1;
        assert_eq!(chain_starter(&bank, 1, 4), 1);
    }

    /// Drive one round's visit order. `script` gives, per visit in order, how
    /// many counters that visit declares -- a visit keeps the floor until the
    /// seat passes or runs out (ruling 2026-10-07; `build_round`), so several
    /// declarations in one entry are one visit. Returns every seat visited.
    /// `initial` is the round's initial user; the cycle starts at their seat and
    /// a lap is one full cycle from there.
    fn visited(initial: usize, n: usize, script: &[usize]) -> Vec<usize> {
        let mut p = Priority::new(initial, n);
        let mut out = vec![];
        let mut i = 0;
        loop {
            let s = p.seat();
            out.push(s);
            let declared = script.get(i).copied().unwrap_or(0) > 0;
            i += 1;
            if !p.answered(declared) {
                return out;
            }
        }
    }

    #[test]
    fn a_round_opens_at_the_initial_user() {
        // Seat 2 raised the timing: the cycle asks 2 first, then 3, 0, 1 --
        // superseding clause 89's 「下一位」 start (2026-10-07).
        assert_eq!(visited(2, 4, &[]), [2, 3, 0, 1]);
        // A one-player cycle asks that seat once.
        assert_eq!(visited(0, 1, &[]), [0]);
    }

    #[test]
    fn a_quiet_lap_closes_the_round_and_a_carried_lap_does_not() {
        // No declarations: one lap and done.
        assert_eq!(visited(2, 4, &[]), [2, 3, 0, 1]);
        // A declaration anywhere in lap 1 carries it -- even at its last visit
        // -- so the round runs lap 2 in full and closes only when that lap is
        // quiet (ruling 2026-10-07: "a lap that carried a declaration never
        // closes the round").
        assert_eq!(visited(2, 4, &[0, 0, 0, 1]), [2, 3, 0, 1, 2, 3, 0, 1]);
        // A declaration at the initial user's own visit carries lap 1 the same
        // way; the visits after it in that lap do not close the round.
        assert_eq!(visited(2, 4, &[1]), [2, 3, 0, 1, 2, 3, 0, 1]);
        // Declarations in two consecutive laps need a third quiet one.
        assert_eq!(
            visited(2, 4, &[0, 1, 0, 0, 1]),
            [2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1]
        );
        // A one-player cycle: a declaring visit is carried, the next one closes.
        assert_eq!(visited(0, 1, &[1, 0]), [0, 0]);
    }

    /// The engine raises these kinds; folding any of them to `None` would make
    /// `can_counteract` guards silently never match (34 cards depend on them).
    /// Includes both the original one-shot kinds and the v10 condition/post
    /// halves added to pair every trigger point.
    #[test]
    fn every_engine_trigger_kind_maps() {
        for k in [
            // original one-shot kinds
            "turnStart",
            "pass",
            "settleBefore",
            "settle",
            "mortgage",
            "paid",
            "bankrupt",
            "card",
            "event",
            "diceRollValue",
            // reserved-but-declared kinds (engine does not raise all of these
            // yet, but `can_counteract` guards match on them so they must map)
            "passPlayer",
            "pay",
            "diceRollBefore",
            "counteracted",
            // v10 condition/post halves + new action hooks
            "turnStartBefore",
            "passBefore",
            "settleAfter",
            "mortgageBefore",
            "bankruptBefore",
            "cardAfter",
            "eventAfter",
            "buyBefore",
            "buyAfter",
            "buildBefore",
            "buildAfter",
            "discardBefore",
            "discardAfter",
            "endTurnBefore",
            "endTurnAfter",
            "leaveBefore",
            "leaveAfter",
            // v17 field-card (hook) hook points
            "turnEnd",
            "drawn",
            "passTile",
            "payAfter",
            "diceRollAfter",
            "diceRollValue",
            "cardPlayed",
            "targeted",
            "paySplitChoose",
            // v23 hook points
            "turnEndBefore",
            "turnEndAfter",
            "paySplitAdd",
            "paySplitMul",
            "paySplitAt",
            "discarded",
            "deckBeforeGame",
            "deckAtGameStart",
            "drew",
            "reshuffled",
            "bought",
            // Ruleset spellings, as opaque parse examples (the engine never
            // switches on these ids).
            "r:bandori:26",
            "settleBody",
            "beforeOut",
            "teleported",
            "diceRollPlan",
            "abnormalGuard",
            "immuneAll",
            "untargetable",
            "redirect",
            "actGate",
            "payGate",
            "moveGate",
            "exclusiveUse",
            "turnBonusTime",
            "turnBaseTime",
            // v28 post-commit points (what a committed effect did)
            "markerSpent",
            "cardUsed",
            "houseAdded",
            // v50: named counter writes
            "counterChanged",
            "r:bandori:78",
            // held-token write (HAG-01)
            "markerChanged",
            // v42: the command-wide pre-split payment stage + terminals
            "payCombinedAdd",
            "payCombinedMul",
            "payCombinedCancel",
            "tileResolved",
            "moveResolved",
            "bankruptResolved",
            // v43: the move-head / move-tail pair
            "moveBefore",
            "moveAfter",
            // v78 / v96 interactive payment pre-stages (hook-only Actions)
            "paySplitChooseBefore",
            "payCombinedAddBefore",
            "paySplitMulBefore",
            // v120: 支付阶段 1 「支付计算前」 (hook-only Action)
            "payCalcBefore",
            // v107: the raw-spend window (a redeem / auction / `lose_raw` hook)
            "spendBefore",
        ] {
            assert!(
                !matches!(trigger_kind(k), TriggerKind::None),
                "{k} folded to TriggerKind::None"
            );
        }
    }
}
