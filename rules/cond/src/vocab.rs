//! The condition vocabulary: **one definition per name**.
//!
//! Adding a condition name = one entry in [`VOCAB`] (plus an accessor on
//! [`crate::view::CondView`] if it reads something new). Everything else --
//! the schema/lint's name lists, the CEL type info, the binding, and the
//! evaluator's function registration -- is derived from this table.
//!
//! | scope | CEL spelling | binding |
//! |---|---|---|
//! | [`Scope::Window`] | `actor`, `move.main`, … | a flat window variable |
//! | [`Scope::Candidate`] | `owner`, `card.cp`, … | a flat candidate variable |
//! | [`Scope::Func`] | `slot(…)`, `is_circle(…)`, … | a CEL function over the view |
//!
//! Window / candidate names with a dotted spelling (`move.main`) rewrite to a
//! flat `move_main` identifier (see [`crate::schema`]); the table carries both.

use crate::view::CondView;

/// Where a name binds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope {
    /// A window-level scalar (trigger + ambient turn state).
    Window,
    /// A candidate-level scalar (the card + seat being probed).
    Candidate,
    /// A lookup function: `name(args...)` over the view.
    Func { arity: usize },
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

/// One vocabulary name.
pub struct Name {
    /// CEL spelling (`"actor"`, `"move.main"`, `"is_circle"`).
    pub cel: &'static str,
    /// Flat identifier after the schema's `root.field` rewrite (`"actor"`,
    /// `"move_main"`, `"is_circle"`). Same as `cel` for bare names.
    pub flat: &'static str,
    pub scope: Scope,
    pub ty: Ty,
    /// One-line doc (kept in `docs/GUARDS.md`'s vocabulary table).
    pub doc: &'static str,
}

/// The vocabulary. **The** definition of every condition name.
pub const VOCAB: &[Name] = &[
    // -- window / trigger ---------------------------------------------------
    Name { cel: "kind", flat: "kind", scope: Scope::Window, ty: Ty::Int, doc: "trigger kind wire value (`trigger.kind` is an alias)" },
    Name { cel: "actor", flat: "actor", scope: Scope::Window, ty: Ty::Int, doc: "trigger actor seat, -1 = none" },
    Name { cel: "target", flat: "target", scope: Scope::Window, ty: Ty::Int, doc: "trigger target seat, -1 = none" },
    Name { cel: "value", flat: "value", scope: Scope::Window, ty: Ty::Int, doc: "the amount involved" },
    Name { cel: "step", flat: "step", scope: Scope::Window, ty: Ty::Int, doc: "turn stage (0 = no turn)" },
    Name { cel: "by", flat: "by", scope: Scope::Window, ty: Ty::Int, doc: "`by_card` owner seat, -1 = none" },
    Name { cel: "pay_is_rent", flat: "pay_is_rent", scope: Scope::Window, ty: Ty::Bool, doc: "is this pay rent" },
    Name { cel: "roll_source", flat: "roll_source", scope: Scope::Window, ty: Ty::Int, doc: "where a roll came from, -1 = none" },
    Name { cel: "abnormal", flat: "abnormal", scope: Scope::Window, ty: Ty::Bool, doc: "the AbnormalGate window" },
    Name { cel: "turn_player", flat: "turn_player", scope: Scope::Window, ty: Ty::Int, doc: "whose turn it is" },
    Name { cel: "turn_key", flat: "turn_key", scope: Scope::Window, ty: Ty::Int, doc: "once-per-turn key; compared against `slot(…)`" },
    Name { cel: "tile.id", flat: "tile_id", scope: Scope::Window, ty: Ty::Int, doc: "the trigger's tile, -1 = none" },
    Name { cel: "tile.owner", flat: "tile_owner", scope: Scope::Window, ty: Ty::Int, doc: "owning seat, -1 = unowned" },
    Name { cel: "tile.houses", flat: "tile_houses", scope: Scope::Window, ty: Ty::Int, doc: "house count" },
    Name { cel: "tile.mortgaged", flat: "tile_mortgaged", scope: Scope::Window, ty: Ty::Int, doc: "1 = mortgaged" },
    Name { cel: "tile.price", flat: "tile_price", scope: Scope::Window, ty: Ty::Int, doc: "price" },
    Name { cel: "move.roll", flat: "move_roll", scope: Scope::Window, ty: Ty::OptInt, doc: "face shown; `null` when none" },
    Name { cel: "move.kind", flat: "move_kind", scope: Scope::Window, ty: Ty::OptInt, doc: "move kind; `null` when no move" },
    Name { cel: "move.remaining", flat: "move_remaining", scope: Scope::Window, ty: Ty::Int, doc: "steps left" },
    Name { cel: "move.main", flat: "move_main", scope: Scope::Window, ty: Ty::Bool, doc: "was this the turn's main move" },
    Name { cel: "chain.count", flat: "effect_count", scope: Scope::Window, ty: Ty::Int, doc: "effect-chain length" },
    // -- candidate ----------------------------------------------------------
    Name { cel: "owner", flat: "owner", scope: Scope::Candidate, ty: Ty::Int, doc: "the candidate's owner seat" },
    Name { cel: "card.id", flat: "card_id", scope: Scope::Candidate, ty: Ty::Int, doc: "candidate card id hash" },
    Name { cel: "card.placed", flat: "card_placed", scope: Scope::Candidate, ty: Ty::Bool, doc: "is the running instance in play" },
    Name { cel: "card.cp", flat: "card_cp", scope: Scope::Candidate, ty: Ty::Int, doc: "crystals on the candidate instance" },
    // -- functions ----------------------------------------------------------
    Name { cel: "slot", flat: "slot", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "slot(name) -> the candidate's latch; missing = 0" },
    Name { cel: "tok", flat: "tok", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "tok(kind) -> the candidate's token; missing = 0" },
    Name { cel: "tile_named", flat: "tile_named", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "tile_named(name) -> board id; -1 when unknown" },
    Name { cel: "is_circle", flat: "is_circle", scope: Scope::Func { arity: 1 }, ty: Ty::Bool, doc: "is_circle(t) -> tile kind is circle" },
    Name { cel: "is_ring", flat: "is_ring", scope: Scope::Func { arity: 1 }, ty: Ty::Bool, doc: "is_ring(t) -> tile kind is ring" },
    Name { cel: "is_live_house", flat: "is_live_house", scope: Scope::Func { arity: 1 }, ty: Ty::Bool, doc: "is_live_house(t) -> tile kind is live house" },
    Name { cel: "is_buyable", flat: "is_buyable", scope: Scope::Func { arity: 1 }, ty: Ty::Bool, doc: "is_buyable(t) -> tile kind is buyable" },
    Name { cel: "money", flat: "money", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "money(p) -> seat's money" },
    Name { cel: "fire", flat: "fire", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "fire(p) -> seat's fire pot" },
    Name { cel: "crystals", flat: "crystals", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "crystals(p) -> seat's band crystals" },
    Name { cel: "hand", flat: "hand", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "hand(p) -> seat's hand size" },
    Name { cel: "pos", flat: "pos", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "pos(p) -> seat's board position" },
    Name { cel: "character", flat: "character", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "character(p) -> seat's character id hash" },
    Name { cel: "band", flat: "band", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "band(p) -> seat's band id hash" },
    Name { cel: "tiles", flat: "tiles", scope: Scope::Func { arity: 1 }, ty: Ty::Int, doc: "tiles(p) -> seat's owned-tile count" },
    Name { cel: "character_is", flat: "character_is", scope: Scope::Func { arity: 2 }, ty: Ty::Bool, doc: "character_is(p, id) -> seat plays that character" },
    Name { cel: "band_is", flat: "band_is", scope: Scope::Func { arity: 2 }, ty: Ty::Bool, doc: "band_is(p, id) -> seat is in that band" },
    Name { cel: "blocked", flat: "blocked", scope: Scope::Func { arity: 1 }, ty: Ty::Bool, doc: "blocked(band) -> skill blocked for this candidate" },
    Name { cel: "neighbor", flat: "neighbor", scope: Scope::Func { arity: 2 }, ty: Ty::Int, doc: "neighbor(p, delta) -> seat id, wrapping" },
    Name { cel: "chain_has", flat: "chain_has", scope: Scope::Func { arity: 1 }, ty: Ty::Bool, doc: "effect.has(kind) / chain.has(kind)" },
    Name { cel: "chain_hits", flat: "chain_hits", scope: Scope::Func { arity: 1 }, ty: Ty::Bool, doc: "effect.hits(seat) / chain.hits(seat)" },
];

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
        .map(|n| n.cel)
        .collect()
}

/// Dotted `root -> [fields]` pairs for the lint's `root.field` rewrite.
pub fn struct_roots() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut roots: Vec<(&'static str, Vec<&'static str>)> = Vec::new();
    for n in VOCAB {
        if matches!(n.scope, Scope::Func { .. }) {
            continue;
        }
        if let Some((root, field)) = n.cel.split_once('.') {
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
    roots
}

/// Bind one name against a view into a `name -> Val` sink. Used by the
/// evaluator to fill the CEL context from a [`CondView`] (lazily: only the
/// names `cond.used_vars()` actually reads).
pub fn bind_one(emit: &mut dyn FnMut(&str, Val), flat: &str, view: &dyn CondView) {
    let Some(n) = VOCAB.iter().find(|n| n.flat == flat) else {
        return;
    };
    match n.scope {
        Scope::Window => {
            let v = match n.cel {
                "kind" => Val::Int(view.kind()),
                "actor" => Val::Int(view.actor()),
                "target" => Val::Int(view.target()),
                "value" => Val::Int(view.value()),
                "step" => Val::Int(view.step()),
                "by" => Val::Int(view.by()),
                "pay_is_rent" => Val::Bool(view.pay_is_rent()),
                "roll_source" => Val::Int(view.roll_source()),
                "abnormal" => Val::Bool(view.abnormal()),
                "turn_player" => Val::Int(view.turn_player()),
                "turn_key" => Val::Int(view.turn_key()),
                "tile.id" => Val::Int(view.tile_id()),
                "tile.owner" => Val::Int(view.tile_owner()),
                "tile.houses" => Val::Int(view.tile_houses()),
                "tile.mortgaged" => Val::Int(view.tile_mortgaged()),
                "tile.price" => Val::Int(view.tile_price()),
                "move.roll" => opt(view.move_roll()),
                "move.kind" => opt(view.move_kind()),
                "move.remaining" => Val::Int(view.move_remaining()),
                "move.main" => Val::Bool(view.move_main()),
                "chain.count" => Val::Int(view.chain_count()),
                _ => return,
            };
            emit(n.flat, v);
        }
        Scope::Candidate => {
            let v = match n.cel {
                "owner" => Val::Int(view.owner()),
                "card.id" => Val::Int(view.card_id()),
                "card.placed" => Val::Bool(view.card_placed()),
                "card.cp" => Val::Int(view.card_cp()),
                _ => return,
            };
            emit(n.flat, v);
        }
        Scope::Func { .. } => {}
    }
}

/// The value shape a binding produces.
#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Int(i64),
    Bool(bool),
    Null,
}

fn opt(v: i64) -> Val {
    if v < 0 {
        Val::Null
    } else {
        Val::Int(v)
    }
}