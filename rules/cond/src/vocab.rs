//! The condition vocabulary: **one definition per name**.
//!
//! Adding a condition name = one entry in [`VOCAB`] plus a [`CondView`]
//! accessor (and that accessor's implementation in each host view). Everything
//! else -- the schema/lint's name lists, the CEL type info, the binding, and
//! the evaluator's function registration -- is derived from this table.
//!
//! | scope | CEL spelling | binding |
//! |---|---|---|
//! | [`Scope::Window`] | `actor`, `move.main`, … | a flat window variable |
//! | [`Scope::Candidate`] | `owner`, `card.counter('name')`, … | a flat candidate variable |
//! | [`Scope::Func`] | `slot(…)`, `is_circle(…)`, … | a CEL function over the view |
//!
//! Window / candidate names with a dotted spelling (`move.main`) rewrite to a
//! flat `move_main` identifier (see [`crate::schema`]); the table carries both.
//! Extra CEL spellings that flatten to the same identifier live in
//! [`Name::aliases`] (`chain.count` is an alias of `effect.count`).

use crate::view::{CondView, TileKind};

/// Where a name binds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope {
    /// A window-level scalar (trigger + ambient turn state).
    Window,
    /// A candidate-level scalar (the card + seat being probed).
    Candidate,
    /// A lookup function: `name(args...)` over the view. `cand` marks the
    /// ones whose backing table is per-candidate (`slot` / `tok` / `blocked`).
    Func { arity: usize, cand: bool },
}

/// CEL type of a name. The schema is int-only; `Bool` binds a CEL bool and
/// `OptInt` binds `null` when the value is absent (`-1`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ty {
    Int,
    Bool,
    /// `null` when the accessor returns `-1` (e.g. `move.roll` with no face).
    OptInt,
}

/// How a scalar name is fetched from a [`CondView`]. The fn pointer *is* the
/// binding: adding a name never touches `eval`'s body.
pub type Getter = fn(&dyn CondView) -> i64;

/// How a function name is served. Each variant names the CEL call shape and
/// the hidden context variable the (eager) evaluator bakes from the view.
#[derive(Clone, Copy, Debug)]
pub enum Fx {
    /// `f(seat: int) -> int` over a per-seat map baked as `_<var>`.
    SeatInt {
        var: &'static str,
        field: fn(&dyn CondView, i64) -> i64,
    },
    /// `f(seat: int, id: int) -> bool` = `field(view, seat) == id`.
    SeatIntIs {
        var: &'static str,
        field: fn(&dyn CondView, i64) -> i64,
    },
    /// `f(name: string) -> int` over a string map baked as `_<var>`;
    /// unknown names answer `missing` (0 for slots, -1 for `tile_named`).
    StrInt {
        var: &'static str,
        missing: i64,
        fill: fn(&dyn CondView) -> Vec<(String, i64)>,
    },
    /// `f(key: int) -> int` over an int map baked as `_<var>`; missing = 0.
    IntInt {
        var: &'static str,
        fill: fn(&dyn CondView) -> Vec<(i64, i64)>,
    },
    /// `f(x: int) -> bool` = membership in the list baked as `_<var>`.
    IntHas {
        var: &'static str,
        fill: fn(&dyn CondView) -> Vec<i64>,
    },
    /// `f(p: int, delta: int) -> int` = wrap-around neighbour of `p`.
    Neighbor,
}

/// One vocabulary name.
pub struct Name {
    /// CEL spelling (`"actor"`, `"move.main"`, `"is_circle"`).
    pub cel: &'static str,
    /// Other CEL spellings that flatten to the same `flat` identifier
    /// (`chain.count` for `effect.count`, `trigger.kind` for `kind`).
    pub aliases: &'static [&'static str],
    /// Flat identifier after the schema's `root.field` rewrite (`"actor"`,
    /// `"move_main"`, `"is_circle"`). Same as `cel` for bare names.
    pub flat: &'static str,
    pub scope: Scope,
    pub ty: Ty,
    /// Scalar fetch (`Scope::Window` / `Scope::Candidate`); zero for funcs.
    pub get: Getter,
    /// Function shape (`Scope::Func`); `Fx::Neighbor` unused otherwise.
    pub fx: Fx,
    /// One-line doc (kept in `docs/GUARDS.md`'s vocabulary table).
    pub doc: &'static str,
}

const NO_GET: Getter = |_| 0;
const NO_FX: Fx = Fx::Neighbor;

/// The vocabulary. **The** definition of every condition name.
pub const VOCAB: &[Name] = &[
    // -- window / trigger ---------------------------------------------------
    Name {
        cel: "kind",
        aliases: &["trigger.kind"],
        flat: "kind",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.kind(),
        fx: NO_FX,
        doc: "trigger kind wire value (`trigger.kind` is an alias)",
    },
    Name {
        cel: "actor",
        aliases: &[],
        flat: "actor",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.actor(),
        fx: NO_FX,
        doc: "trigger actor seat, -1 = none",
    },
    Name {
        cel: "target",
        aliases: &[],
        flat: "target",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.target(),
        fx: NO_FX,
        doc: "trigger target seat, -1 = none",
    },
    Name {
        cel: "value",
        aliases: &[],
        flat: "value",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.value(),
        fx: NO_FX,
        doc: "the amount involved",
    },
    Name {
        cel: "step",
        aliases: &[],
        flat: "step",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.step(),
        fx: NO_FX,
        doc: "turn stage (0 = no turn)",
    },
    Name {
        cel: "by",
        aliases: &[],
        flat: "by",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.by(),
        fx: NO_FX,
        doc: "`by_card` owner seat, -1 = none",
    },
    Name {
        cel: "pay_is_rent",
        aliases: &[],
        flat: "pay_is_rent",
        scope: Scope::Window,
        ty: Ty::Bool,
        get: |v| v.pay_is_rent() as i64,
        fx: NO_FX,
        doc: "is this pay rent",
    },
    Name {
        cel: "roll_source",
        aliases: &[],
        flat: "roll_source",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.roll_source(),
        fx: NO_FX,
        doc: "where a roll came from, -1 = none",
    },
    Name {
        cel: "abnormal",
        aliases: &[],
        flat: "abnormal",
        scope: Scope::Window,
        ty: Ty::Bool,
        get: |v| v.abnormal() as i64,
        fx: NO_FX,
        doc: "the AbnormalGate window",
    },
    Name {
        cel: "turn_player",
        aliases: &[],
        flat: "turn_player",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.turn_player(),
        fx: NO_FX,
        doc: "whose turn it is",
    },
    Name {
        cel: "turn_key",
        aliases: &[],
        flat: "turn_key",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.turn_key(),
        fx: NO_FX,
        doc: "once-per-turn key; compared against `slot(…)`",
    },
    Name {
        cel: "trigger_card",
        aliases: &[],
        flat: "trigger_card",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.trigger_card(),
        fx: NO_FX,
        doc: "the trigger's card id hash; comparable to `card.id`",
    },
    Name {
        cel: "counter_name",
        aliases: &[],
        flat: "counter_name",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.counter_name(),
        fx: NO_FX,
        doc: "the trigger's counter / message name hash (`Trigger.name`); `counter_is('…')` is the string spelling",
    },
    Name {
        cel: "tile.id",
        aliases: &[],
        flat: "tile_id",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.tile_id(),
        fx: NO_FX,
        doc: "the trigger's tile, -1 = none",
    },
    Name {
        cel: "tile.owner",
        aliases: &[],
        flat: "tile_owner",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.tile_owner(),
        fx: NO_FX,
        doc: "owning seat, -1 = unowned",
    },
    Name {
        cel: "tile.houses",
        aliases: &[],
        flat: "tile_houses",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.tile_houses(),
        fx: NO_FX,
        doc: "house count",
    },
    Name {
        cel: "tile.mortgaged",
        aliases: &[],
        flat: "tile_mortgaged",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.tile_mortgaged(),
        fx: NO_FX,
        doc: "1 = mortgaged",
    },
    Name {
        cel: "tile.price",
        aliases: &[],
        flat: "tile_price",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.tile_price(),
        fx: NO_FX,
        doc: "price",
    },
    Name {
        cel: "move.roll",
        aliases: &[],
        flat: "move_roll",
        scope: Scope::Window,
        ty: Ty::OptInt,
        get: |v| v.move_roll(),
        fx: NO_FX,
        doc: "face shown; `null` when none",
    },
    Name {
        cel: "move.kind",
        aliases: &[],
        flat: "move_kind",
        scope: Scope::Window,
        ty: Ty::OptInt,
        get: |v| v.move_kind(),
        fx: NO_FX,
        doc: "move kind; `null` when no move",
    },
    Name {
        cel: "move.remaining",
        aliases: &[],
        flat: "move_remaining",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.move_remaining(),
        fx: NO_FX,
        doc: "steps left",
    },
    Name {
        cel: "move.main",
        aliases: &[],
        flat: "move_main",
        scope: Scope::Window,
        ty: Ty::Bool,
        get: |v| v.move_main() as i64,
        fx: NO_FX,
        doc: "was this the turn's main move",
    },
    Name {
        cel: "move.dir",
        aliases: &[],
        flat: "move_dir",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.move_dir(),
        fx: NO_FX,
        doc: "1 forward, -1 backward; 1 when no move",
    },
    Name {
        cel: "effect.count",
        aliases: &["chain.count"],
        flat: "effect_count",
        scope: Scope::Window,
        ty: Ty::Int,
        get: |v| v.chain_count(),
        fx: NO_FX,
        doc: "effect-chain length (`chain.count` is an alias)",
    },
    // -- candidate ----------------------------------------------------------
    Name {
        cel: "owner",
        aliases: &[],
        flat: "owner",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner(),
        fx: NO_FX,
        doc: "the candidate's owner seat",
    },
    Name {
        cel: "owner.id",
        aliases: &[],
        flat: "owner_id",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner(),
        fx: NO_FX,
        doc: "same as `owner`",
    },
    Name {
        cel: "owner.money",
        aliases: &[],
        flat: "owner_money",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_money(),
        fx: NO_FX,
        doc: "the owner's money",
    },
    Name {
        cel: "owner.fire",
        aliases: &[],
        flat: "owner_fire",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_fire(),
        fx: NO_FX,
        doc: "the owner's fire pot",
    },
    Name {
        cel: "owner.crystals",
        aliases: &[],
        flat: "owner_crystals",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_crystals(),
        fx: NO_FX,
        doc: "the owner's band crystals (the band-skill pool, not on-card)",
    },
    Name {
        cel: "owner.hand",
        aliases: &[],
        flat: "owner_hand",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_hand(),
        fx: NO_FX,
        doc: "the owner's hand size",
    },
    Name {
        cel: "owner.pos",
        aliases: &[],
        flat: "owner_pos",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_pos(),
        fx: NO_FX,
        doc: "the owner's board position",
    },
    Name {
        cel: "owner.out",
        aliases: &[],
        flat: "owner_out",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_out(),
        fx: NO_FX,
        doc: "1 = the owner is out of the game",
    },
    Name {
        cel: "owner.stay",
        aliases: &[],
        flat: "owner_stay",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_stay(),
        fx: NO_FX,
        doc: "the owner's stay count",
    },
    Name {
        cel: "owner.stun",
        aliases: &[],
        flat: "owner_stun",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_stun(),
        fx: NO_FX,
        doc: "the owner's stun count",
    },
    Name {
        cel: "owner.exile",
        aliases: &[],
        flat: "owner_exile",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_exile(),
        fx: NO_FX,
        doc: "the owner's exile flag",
    },
    Name {
        cel: "owner.no_hand",
        aliases: &[],
        flat: "owner_no_hand",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_no_hand(),
        fx: NO_FX,
        doc: "1 = 「不可持有手牌」",
    },
    Name {
        cel: "owner.character",
        aliases: &[],
        flat: "owner_character",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_character(),
        fx: NO_FX,
        doc: "the owner's character id hash",
    },
    Name {
        cel: "owner.band",
        aliases: &[],
        flat: "owner_band",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_band(),
        fx: NO_FX,
        doc: "the owner's band id hash",
    },
    Name {
        cel: "owner.tiles",
        aliases: &[],
        flat: "owner_tiles",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.owner_tiles(),
        fx: NO_FX,
        doc: "the owner's owned-tile count",
    },
    Name {
        cel: "card.id",
        aliases: &[],
        flat: "card_id",
        scope: Scope::Candidate,
        ty: Ty::Int,
        get: |v| v.card_id(),
        fx: NO_FX,
        doc: "candidate card id hash",
    },
    Name {
        cel: "card.placed",
        aliases: &[],
        flat: "card_placed",
        scope: Scope::Candidate,
        ty: Ty::Bool,
        get: |v| v.card_placed() as i64,
        fx: NO_FX,
        doc: "is the running instance in play",
    },
    // -- functions ----------------------------------------------------------
    Name {
        cel: "slot",
        aliases: &[],
        flat: "slot",
        scope: Scope::Func { arity: 1, cand: true },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::StrInt {
            var: "_slots",
            missing: 0,
            fill: |v| v.slot_table(),
        },
        doc: "slot(name) -> the candidate's latch; missing = 0",
    },
    Name {
        cel: "tok",
        aliases: &[],
        flat: "tok",
        scope: Scope::Func { arity: 1, cand: true },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::StrInt {
            var: "_toks",
            missing: 0,
            fill: |v| v.tok_named_table(),
        },
        doc: "tok('name') -> the owner's token counter; missing = 0",
    },
    Name {
        cel: "move.tag",
        aliases: &[],
        flat: "move_tag",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::StrInt {
            var: "_move_tags",
            missing: 0,
            fill: |v| v.move_tag_table(),
        },
        doc: "move.tag('name') -> the move's tag counter; missing = 0",
    },
    Name {
        cel: "card_counter",
        aliases: &["card.counter"],
        flat: "card_counter",
        scope: Scope::Func { arity: 1, cand: true },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::StrInt {
            var: "_card_counters",
            missing: 0,
            fill: |v| v.card_counter_table(),
        },
        doc: "card.counter('name') -> the candidate instance's named counter ('cp' / 'crystals' / any); missing = 0",
    },
    Name {
        cel: "counter_is",
        aliases: &[],
        flat: "counter_is",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::IntHas {
            var: "_counter_name",
            fill: |v| {
                let h = v.counter_name();
                if h == 0 {
                    Vec::new()
                } else {
                    vec![h]
                }
            },
        },
        doc: "counter_is('name') -> this window's `Trigger.name` is that counter / message",
    },
    Name {
        cel: "tile_named",
        aliases: &[],
        flat: "tile_named",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::StrInt {
            var: "_tile_ids",
            missing: -1,
            fill: |v| v.tile_id_table(),
        },
        doc: "tile_named(name) -> board id; -1 when unknown",
    },
    Name {
        cel: "is_circle",
        aliases: &[],
        flat: "is_circle",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::IntHas {
            var: "_circle_tiles",
            fill: |v| v.tile_kind_list(TileKind::Circle),
        },
        doc: "is_circle(t) -> tile kind is circle",
    },
    Name {
        cel: "is_ring",
        aliases: &[],
        flat: "is_ring",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::IntHas {
            var: "_ring_tiles",
            fill: |v| v.tile_kind_list(TileKind::Ring),
        },
        doc: "is_ring(t) -> tile kind is ring",
    },
    Name {
        cel: "is_live_house",
        aliases: &[],
        flat: "is_live_house",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::IntHas {
            var: "_live_house_tiles",
            fill: |v| v.tile_kind_list(TileKind::LiveHouse),
        },
        doc: "is_live_house(t) -> tile kind is live house",
    },
    Name {
        cel: "is_buyable",
        aliases: &[],
        flat: "is_buyable",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::IntHas {
            var: "_buyable_tiles",
            fill: |v| v.tile_kind_list(TileKind::Buyable),
        },
        doc: "is_buyable(t) -> tile kind is buyable",
    },
    Name {
        cel: "money",
        aliases: &[],
        flat: "money",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::SeatInt {
            var: "_money",
            field: |v, s| v.money(s),
        },
        doc: "money(p) -> seat's money",
    },
    Name {
        cel: "fire",
        aliases: &[],
        flat: "fire",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::SeatInt {
            var: "_fire",
            field: |v, s| v.fire(s),
        },
        doc: "fire(p) -> seat's fire pot",
    },
    Name {
        cel: "crystals",
        aliases: &[],
        flat: "crystals",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::SeatInt {
            var: "_crystals",
            field: |v, s| v.crystals(s),
        },
        doc: "crystals(p) -> seat's band crystals (the band-skill pool)",
    },
    Name {
        cel: "hand",
        aliases: &[],
        flat: "hand",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::SeatInt {
            var: "_hand",
            field: |v, s| v.hand(s),
        },
        doc: "hand(p) -> seat's hand size",
    },
    Name {
        cel: "pos",
        aliases: &[],
        flat: "pos",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::SeatInt {
            var: "_pos",
            field: |v, s| v.pos(s),
        },
        doc: "pos(p) -> seat's board position",
    },
    Name {
        cel: "character",
        aliases: &[],
        flat: "character",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::SeatInt {
            var: "_character",
            field: |v, s| v.character(s),
        },
        doc: "character(p) -> seat's character id hash",
    },
    Name {
        cel: "band",
        aliases: &[],
        flat: "band",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::SeatInt {
            var: "_band",
            field: |v, s| v.band(s),
        },
        doc: "band(p) -> seat's band id hash",
    },
    Name {
        cel: "tiles",
        aliases: &[],
        flat: "tiles",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::SeatInt {
            var: "_tiles",
            field: |v, s| v.tiles(s),
        },
        doc: "tiles(p) -> seat's owned-tile count",
    },
    Name {
        cel: "character_is",
        aliases: &[],
        flat: "character_is",
        scope: Scope::Func { arity: 2, cand: false },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::SeatIntIs {
            var: "_character",
            field: |v, s| v.character(s),
        },
        doc: "character_is(p, '名') / character_is(p, id) -> seat plays that character",
    },
    Name {
        cel: "band_is",
        aliases: &[],
        flat: "band_is",
        scope: Scope::Func { arity: 2, cand: false },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::SeatIntIs {
            var: "_band",
            field: |v, s| v.band(s),
        },
        doc: "band_is(p, 'Band') / band_is(p, id) -> seat is in that band",
    },
    Name {
        cel: "blocked",
        aliases: &[],
        flat: "blocked",
        scope: Scope::Func { arity: 1, cand: true },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::IntHas {
            var: "_blocked",
            fill: |v| v.blocked_bands(),
        },
        doc: "blocked(band) -> skill blocked for this candidate",
    },
    Name {
        cel: "neighbor",
        aliases: &[],
        flat: "neighbor",
        scope: Scope::Func { arity: 2, cand: false },
        ty: Ty::Int,
        get: NO_GET,
        fx: Fx::Neighbor,
        doc: "neighbor(p, delta) -> seat id, wrapping",
    },
    Name {
        cel: "chain_has",
        aliases: &[],
        flat: "chain_has",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::IntHas {
            var: "_chain_kinds",
            fill: |v| v.chain_kinds(),
        },
        doc: "effect.has(kind) / chain.has(kind)",
    },
    Name {
        cel: "chain_hits",
        aliases: &[],
        flat: "chain_hits",
        scope: Scope::Func { arity: 1, cand: false },
        ty: Ty::Bool,
        get: NO_GET,
        fx: Fx::IntHas {
            var: "_hit_seats",
            fill: |v| v.chain_hits(),
        },
        doc: "effect.hits(seat) / chain.hits(seat)",
    },
];

/// Look a name up by flat identifier (the post-rewrite spelling).
pub fn by_flat(flat: &str) -> Option<&'static Name> {
    VOCAB.iter().find(|n| n.flat == flat)
}

/// Look a name up by CEL spelling or any of its aliases.
pub fn by_cel(cel: &str) -> Option<&'static Name> {
    VOCAB.iter().find(|n| n.cel == cel || n.aliases.contains(&cel))
}

/// Flat identifiers the lint accepts (window + candidate scalars).
pub fn flat_idents() -> Vec<&'static str> {
    VOCAB
        .iter()
        .filter(|n| !matches!(n.scope, Scope::Func { .. }))
        .map(|n| n.flat)
        .collect()
}

/// CEL function names the lint accepts (plus CEL operators/stdlib).
pub fn func_names() -> Vec<&'static str> {
    VOCAB
        .iter()
        .filter(|n| matches!(n.scope, Scope::Func { .. }))
        .flat_map(|n| {
            // Both the CEL spelling (`move.tag`) and the flat id (`move_tag`)
            // are callable: the rewrite lowers dotted calls to the flat, and
            // a bare flat spelling is accepted as-is.
            if n.cel == n.flat {
                vec![n.cel]
            } else {
                vec![n.cel, n.flat]
            }
        })
        .collect()
}

/// Dotted `root -> [fields]` pairs for the lint's `root.field` rewrite.
/// Includes every CEL spelling and alias of a dotted name, so `chain.count`
/// and `effect.count` both land on the `effect_count` binding.
pub fn struct_roots() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut roots: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    fn push(roots: &mut Vec<(&'static str, Vec<&'static str>)>, dotted: &'static str) {
        if let Some((root, field)) = dotted.split_once('.') {
            match roots.iter_mut().find(|(r, _)| *r == root) {
                Some((_, fields)) => {
                    if !fields.contains(&field) {
                        fields.push(field);
                    }
                }
                None => roots.push((root, vec![field])),
            }
        }
    }
    for n in VOCAB {
        if matches!(n.scope, Scope::Func { .. }) {
            continue;
        }
        push(&mut roots, n.cel);
        for a in n.aliases {
            push(&mut roots, a);
        }
    }
    roots
}

/// The value shape a scalar binding produces.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Int(i64),
    Bool(bool),
    Null,
}

/// The eager function-table shape a [`Fx`] bakes from the view. The evaluator
/// turns this into the hidden CEL variable (`_<var>`) the closure reads.
pub enum Table {
    /// seat -> int (also the `int -> int` case, keyed by the fill list).
    IntMap(Vec<(i64, i64)>),
    /// string -> int (`slot` / `tile_named`).
    StrMap(Vec<(String, i64)>),
    /// membership list (`is_circle`, `blocked`, `chain_has`, …).
    IntList(Vec<i64>),
    /// just the seat count (`neighbor`).
    Seats(i64),
}

fn opt(v: i64) -> Val {
    if v < 0 {
        Val::Null
    } else {
        Val::Int(v)
    }
}

fn val_of(n: &Name, v: &dyn CondView) -> Val {
    match n.ty {
        Ty::Int => Val::Int((n.get)(v)),
        Ty::Bool => Val::Bool((n.get)(v) != 0),
        Ty::OptInt => opt((n.get)(v)),
    }
}

/// Bind one name against a view into a `name -> Val` sink. Used by the
/// evaluator to fill the CEL context from a [`CondView`] (eagerly for the
/// window half, and for only the names `cond.used_vars()` reads on the
/// candidate half).
pub fn bind_one(emit: &mut dyn FnMut(&str, Val), flat: &str, view: &dyn CondView) {
    let Some(n) = by_flat(flat) else {
        return;
    };
    if matches!(n.scope, Scope::Func { .. }) {
        return;
    }
    emit(n.flat, val_of(n, view));
}

/// Fetch a scalar's value straight from the view (same mapping as [`bind_one`]).
pub fn fetch(flat: &str, view: &dyn CondView) -> Option<Val> {
    let n = by_flat(flat)?;
    fetch_name(n, view)
}

/// Fetch a known [`Name`]'s scalar value (no table lookup).
pub fn fetch_name(n: &Name, view: &dyn CondView) -> Option<Val> {
    if matches!(n.scope, Scope::Func { .. }) {
        return None;
    }
    Some(val_of(n, view))
}

/// Build the hidden table a function's eager closure reads, from the view.
/// The hidden variable name is the one in [`Fx`] (`_slots`, `_money`, …).
pub fn fn_table(n: &Name, view: &dyn CondView) -> Option<(&'static str, Table)> {
    match n.fx {
        Fx::SeatInt { var, field } => {
            let rows = (0..view.seat_count()).map(|s| (s, field(view, s))).collect();
            Some((var, Table::IntMap(rows)))
        }
        Fx::SeatIntIs { var, field } => {
            let rows = (0..view.seat_count()).map(|s| (s, field(view, s))).collect();
            Some((var, Table::IntMap(rows)))
        }
        Fx::StrInt { var, fill, .. } => Some((var, Table::StrMap(fill(view)))),
        Fx::IntInt { var, fill } => Some((var, Table::IntMap(fill(view)))),
        Fx::IntHas { var, fill } => Some((var, Table::IntList(fill(view)))),
        Fx::Neighbor => Some(("_seats", Table::Seats(view.seat_count()))),
    }
}