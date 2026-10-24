//! Shared test fixture: a local `CondExt` covering the three [`LowerAs`]
//! shapes plus the sugar names the kernel tests need. **Not** `bandori-std`:
//! kernel tests must not depend on content.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use rules_cond::ext::{CondExt, LowerAs};
use rules_cond::vocab::{Fx, Getter, Name, Scope, StrGetter, Ty};
use rules_cond::view::CondView;
use rules_cond_ir::ctx::{id_of, FnId, IrCtx};

const NO_GET: Getter = |_| 0;
const NO_GET_STR: StrGetter = |_| std::borrow::Cow::Borrowed("");
const NO_FX: Fx = Fx::Neighbor;

fn owner_get(flat: &str) -> Getter {
    match flat {
        "owner_charge" => |v| v.state(v.owner(), "charge"),
        "owner_group_pool" => |v| v.state(v.owner(), "crystals"),
        "owner_persona" => |v| v.state(v.owner(), "persona"),
        "owner_group" => |v| v.state(v.owner(), "group"),
        "owner_no_hand" => |v| v.state(v.owner(), "noHand"),
        _ => NO_GET,
    }
}

fn seat_charge(v: &dyn CondView, s: i64) -> i64 {
    v.state(s, "charge")
}
fn seat_crystals(v: &dyn CondView, s: i64) -> i64 {
    v.state(s, "crystals")
}
fn seat_persona(v: &dyn CondView, s: i64) -> i64 {
    v.state(s, "persona")
}
fn seat_group(v: &dyn CondView, s: i64) -> i64 {
    v.state(s, "group")
}
fn seat_no_hand(v: &dyn CondView, s: i64) -> i64 {
    v.state(s, "noHand")
}

fn sugar(
    cel: &'static str,
    aliases: &'static [&'static str],
    flat: &'static str,
    is_bool: bool,
    is_func: bool,
    arity: usize,
    cand: bool,
    lower: LowerAs,
    field: fn(&dyn CondView, i64) -> i64,
) -> Name {
    let ty = if is_bool { Ty::Bool } else { Ty::Int };
    let scope = if is_func {
        Scope::Func { arity, cand }
    } else {
        Scope::Candidate
    };
    let get: Getter = if is_func { NO_GET } else { owner_get(flat) };
    let fx = if !is_func {
        NO_FX
    } else if is_bool && arity == 2 {
        Fx::SeatIntIs { var: flat, field }
    } else if is_bool && cand {
        Fx::IntHas {
            var: flat,
            fill: |v| v.cand_set("blocked"),
        }
    } else if is_bool {
        NO_FX
    } else {
        Fx::SeatInt { var: flat, field }
    };
    Name {
        cel,
        aliases,
        flat,
        scope,
        ty,
        get,
        get_str: NO_GET_STR,
        fx,
        lower_as: lower,
        doc: "",
    }
}

fn bake(s: &'static str) -> &'static [&'static str] {
    Box::leak(Box::new([s]))
}

fn sugar_rows() -> Vec<Name> {
    vec![
        sugar(
            "charge",
            &[],
            "charge",
            false,
            true,
            1,
            false,
            LowerAs::Call {
                fid: FnId::State,
                bake: bake("charge"),
            },
            seat_charge,
        ),
        sugar(
            "crystals",
            &[],
            "crystals",
            false,
            true,
            1,
            false,
            LowerAs::Call {
                fid: FnId::State,
                bake: bake("crystals"),
            },
            seat_crystals,
        ),
        sugar(
            "persona",
            &[],
            "persona",
            false,
            true,
            1,
            false,
            LowerAs::Call {
                fid: FnId::State,
                bake: bake("persona"),
            },
            seat_persona,
        ),
        sugar(
            "group",
            &["band"],
            "group",
            false,
            true,
            1,
            false,
            LowerAs::Call {
                fid: FnId::State,
                bake: bake("group"),
            },
            seat_group,
        ),
        sugar(
            "no_hand",
            &[],
            "no_hand",
            false,
            true,
            1,
            false,
            LowerAs::Call {
                fid: FnId::State,
                bake: bake("noHand"),
            },
            seat_no_hand,
        ),
        sugar(
            "persona_is",
            &[],
            "persona_is",
            true,
            true,
            2,
            false,
            LowerAs::CallEq {
                fid: FnId::State,
                bake: bake("persona"),
            },
            seat_persona,
        ),
        sugar(
            "group_is",
            &[],
            "group_is",
            true,
            true,
            2,
            false,
            LowerAs::CallEq {
                fid: FnId::State,
                bake: bake("group"),
            },
            seat_group,
        ),
        sugar(
            "blocked",
            &[],
            "blocked",
            true,
            true,
            1,
            true,
            LowerAs::Call {
                fid: FnId::CandHas,
                bake: bake("blocked"),
            },
            seat_charge,
        ),
        sugar(
            "owner.charge",
            &[],
            "owner_charge",
            false,
            false,
            0,
            false,
            LowerAs::OwnerState {
                fid: FnId::State,
                key: "charge",
            },
            seat_charge,
        ),
        sugar(
            "owner.crystals",
            &[],
            "owner_group_pool",
            false,
            false,
            0,
            false,
            LowerAs::OwnerState {
                fid: FnId::State,
                key: "crystals",
            },
            seat_crystals,
        ),
        sugar(
            "owner.persona",
            &[],
            "owner_persona",
            false,
            false,
            0,
            false,
            LowerAs::OwnerState {
                fid: FnId::State,
                key: "persona",
            },
            seat_persona,
        ),
        sugar(
            "owner.group",
            &[],
            "owner_group",
            false,
            false,
            0,
            false,
            LowerAs::OwnerState {
                fid: FnId::State,
                key: "group",
            },
            seat_group,
        ),
        sugar(
            "owner.no_hand",
            &[],
            "owner_no_hand",
            false,
            false,
            0,
            false,
            LowerAs::OwnerState {
                fid: FnId::State,
                key: "noHand",
            },
            seat_no_hand,
        ),
    ]
}

/// The kernel-test extension (built once).
pub fn test_ext() -> &'static CondExt {
    static EXT: OnceLock<CondExt> = OnceLock::new();
    EXT.get_or_init(|| CondExt {
        names: Box::leak(sugar_rows().into_boxed_slice()),
        // Synthetic ids, not this ruleset's `r:bandori:*` namespace -- kernel
        // tests must not name the game. The production gate
        // (`cond_ir_consistency`) installs the real `bandori_std` ext.
        kind_consts: &[
            ("Exile", "r:test:21"),
            ("PassRewardAffected", "r:test:26"),
            ("CircleAffected", "r:test:26"),
            ("FansFlip", "r:test:107"),
        ],
        guard_consts: &[],
    })
}

/// Install the test extension (idempotent). Call before the first `compile`.
pub fn install_test_ext() {
    let _ = rules_cond::install_ext(test_ext());
}

/// Write one `state(p, key)` row on both the CEL snapshot and the IR.
pub fn set_state(
    players: &mut Vec<rules_cond::PlayerSnap>,
    ir: &mut IrCtx,
    seat: usize,
    key: &str,
    v: i64,
) {
    while players.len() <= seat {
        players.push(rules_cond::PlayerSnap::default());
    }
    players[seat].state.insert(key.to_string(), v);
    ir.state_set(seat as i64, id_of(key), v);
}

/// Fill every sugar `state` key on `seat`.
pub fn set_sugar_state(
    players: &mut Vec<rules_cond::PlayerSnap>,
    ir: &mut IrCtx,
    seat: usize,
    charge: i64,
    crystals: i64,
    persona: i64,
    group: i64,
    no_hand: i64,
) {
    set_state(players, ir, seat, "charge", charge);
    set_state(players, ir, seat, "crystals", crystals);
    set_state(players, ir, seat, "persona", persona);
    set_state(players, ir, seat, "group", group);
    set_state(players, ir, seat, "noHand", no_hand);
}

/// Hash helper shared by the sugar cases.
pub fn h(s: &str) -> i64 {
    id_of(s)
}

/// `state` map builder.
pub fn state_map(rows: &[(&str, i64)]) -> BTreeMap<String, i64> {
    rows.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
}