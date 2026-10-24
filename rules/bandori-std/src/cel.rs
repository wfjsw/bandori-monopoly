//! Content-side CEL sugar for this ruleset: plain spelling tables plus the
//! typed [`CondExt`] assembled from them.
//!
//! The kernel (`rules-cond`) keeps a closed generic algebra. BanG Dream
//! spellings survive as deprecated sugar that lowers to that algebra
//! (`charge(p)` → `state(p, 'charge')`). Authoring strings are unchanged; the
//! flat ids match the old kernel rows so precompiled blobs keep binding.
//!
//! [`EXT`] is the single typed package the host installs
//! (`rules_cond::install_ext`). The plain tables below are its source of truth.

/// How one sugar name lowers to the kernel algebra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LowerShape {
    /// `Call(fid, [user_args…, Const(id_of(bake))…])` -- e.g. `charge(p)`.
    CallState { bake: &'static str },
    /// `Eq(Call(fid, [user_args[..n-1]…, Const(id_of(bake))…]), user_args[n-1])`
    /// -- e.g. `persona_is(p, '名')`.
    CallEqState { bake: &'static str },
    /// `Call(CandHas, [user_args…, Const(id_of(bake))…])` -- e.g. `blocked(b)`.
    CallCandHas { bake: &'static str },
    /// `Call(State, [Var(Owner), Const(id_of(key))])` -- e.g. `owner.charge`.
    OwnerState { key: &'static str },
}

/// One content-side sugar name. The host turns this into a kernel `Name`.
pub struct Sugar {
    /// CEL spelling.
    pub cel: &'static str,
    /// Other CEL spellings that flatten to the same `flat`.
    pub aliases: &'static [&'static str],
    /// Flat identifier (must match the old kernel row so precompiled blobs bind).
    pub flat: &'static str,
    /// True for `f(...) -> bool`, false for `f(...) -> int` / scalar int.
    pub is_bool: bool,
    /// `true` = `Scope::Func` (callable), `false` = candidate scalar.
    pub is_func: bool,
    /// Arity (0 for scalars).
    pub arity: usize,
    /// Per-candidate backing table (`slot` / `tok` / `blocked`).
    pub cand: bool,
    /// The `state` / `cand_set` key the host dispatches on (empty for none).
    pub key: &'static str,
    pub lower: LowerShape,
    pub doc: &'static str,
}

/// The 13 sugar names. Flat ids match the old kernel rows.
pub const SUGAR: &[Sugar] = &[
    Sugar {
        cel: "charge",
        aliases: &[],
        flat: "charge",
        is_bool: false,
        is_func: true,
        arity: 1,
        cand: false,
        key: "charge",
        lower: LowerShape::CallState { bake: "charge" },
        doc: "charge(p) -> seat's fire pot (sugar for state(p, 'charge'))",
    },
    Sugar {
        cel: "crystals",
        aliases: &[],
        flat: "crystals",
        is_bool: false,
        is_func: true,
        arity: 1,
        cand: false,
        key: "crystals",
        lower: LowerShape::CallState { bake: "crystals" },
        doc: "crystals(p) -> seat's band crystals (sugar for state(p, 'crystals'))",
    },
    Sugar {
        cel: "persona",
        aliases: &[],
        flat: "persona",
        is_bool: false,
        is_func: true,
        arity: 1,
        cand: false,
        key: "persona",
        lower: LowerShape::CallState { bake: "persona" },
        doc: "persona(p) -> seat's persona id hash (sugar for state(p, 'persona'))",
    },
    Sugar {
        cel: "group",
        aliases: &["band"],
        flat: "group",
        is_bool: false,
        is_func: true,
        arity: 1,
        cand: false,
        key: "group",
        lower: LowerShape::CallState { bake: "group" },
        doc: "group(p) -> seat's group id hash (sugar for state(p, 'group'))",
    },
    Sugar {
        cel: "no_hand",
        aliases: &[],
        flat: "no_hand",
        is_bool: false,
        is_func: true,
        arity: 1,
        cand: false,
        key: "noHand",
        lower: LowerShape::CallState { bake: "noHand" },
        doc: "no_hand(p) -> 1 = 「不可持有手牌」 (sugar for state(p, 'noHand'))",
    },
    Sugar {
        cel: "persona_is",
        aliases: &[],
        flat: "persona_is",
        is_bool: true,
        is_func: true,
        arity: 2,
        cand: false,
        key: "persona",
        lower: LowerShape::CallEqState { bake: "persona" },
        doc: "persona_is(p, '名') -> state(p, 'persona') == id_of('名')",
    },
    Sugar {
        cel: "group_is",
        aliases: &[],
        flat: "group_is",
        is_bool: true,
        is_func: true,
        arity: 2,
        cand: false,
        key: "group",
        lower: LowerShape::CallEqState { bake: "group" },
        doc: "group_is(p, 'Band') -> state(p, 'group') == id_of('Band')",
    },
    Sugar {
        cel: "blocked",
        aliases: &[],
        flat: "blocked",
        is_bool: true,
        is_func: true,
        arity: 1,
        cand: true,
        key: "blocked",
        lower: LowerShape::CallCandHas { bake: "blocked" },
        doc: "blocked(band) -> cand_has('blocked', band)",
    },
    Sugar {
        cel: "owner.charge",
        aliases: &[],
        flat: "owner_charge",
        is_bool: false,
        is_func: false,
        arity: 0,
        cand: false,
        key: "charge",
        lower: LowerShape::OwnerState { key: "charge" },
        doc: "the owner's fire pot (sugar for state(owner, 'charge'))",
    },
    Sugar {
        cel: "owner.crystals",
        aliases: &[],
        flat: "owner_group_pool",
        is_bool: false,
        is_func: false,
        arity: 0,
        cand: false,
        key: "crystals",
        lower: LowerShape::OwnerState { key: "crystals" },
        doc: "the owner's band crystals (sugar for state(owner, 'crystals'))",
    },
    Sugar {
        cel: "owner.persona",
        aliases: &[],
        flat: "owner_persona",
        is_bool: false,
        is_func: false,
        arity: 0,
        cand: false,
        key: "persona",
        lower: LowerShape::OwnerState { key: "persona" },
        doc: "the owner's character id hash (sugar for state(owner, 'persona'))",
    },
    Sugar {
        cel: "owner.group",
        aliases: &[],
        flat: "owner_group",
        is_bool: false,
        is_func: false,
        arity: 0,
        cand: false,
        key: "group",
        lower: LowerShape::OwnerState { key: "group" },
        doc: "the owner's band id hash (sugar for state(owner, 'group'))",
    },
    Sugar {
        cel: "owner.no_hand",
        aliases: &[],
        flat: "owner_no_hand",
        is_bool: false,
        is_func: false,
        arity: 0,
        cand: false,
        key: "noHand",
        lower: LowerShape::OwnerState { key: "noHand" },
        doc: "1 = 「不可持有手牌」 (sugar for state(owner, 'noHand'))",
    },
];

/// Bare-ident CEL kind constants: (`"Exile"`, `"r:bandori:21"`).
/// Wire strings from [`crate::kind`]; ids must not change.
pub const KIND_CONSTS: &[(&str, &str)] = &[
    ("Exile", "r:bandori:21"),
    ("PassRewardAffected", "r:bandori:26"),
    // ABI v69 alias, keep.
    ("CircleAffected", "r:bandori:26"),
    ("FansFlip", "r:bandori:107"),
];

/// Source-scanner expansions for Rust const refs in `pre` position.
/// Literals are [`crate::guards`]; call sites stay on those consts.
pub const GUARD_CONSTS: &[(&str, &str)] = &[
    (
        "bandori_std::guards::OWN_START",
        "actor == owner && tile_is(tile.id, \"circle\")",
    ),
    (
        "bandori_std::guards::OWN_DEED",
        "actor == owner && tile_is(tile.id, \"ring\")",
    ),
];

// -- sugar `state(p, key)` keys ------------------------------------------------
//
// One list, read by the host fill (`fill_window` / `fill_candidate`) and
// `rules/kani`'s `ir_fill`, so the eager `state_table()` and `IrCtx.state`
// rows agree. A unit test pins these against the [`SUGAR`] bake /
// `OwnerState.key` fields.

pub const KEY_CHARGE: &str = "charge";
pub const KEY_CRYSTALS: &str = "crystals";
pub const KEY_PERSONA: &str = "persona";
pub const KEY_GROUP: &str = "group";
pub const KEY_NO_HAND: &str = "noHand";

/// The five sugar state keys. `noHand` is also a `MatchPlayer.state` key; the
/// other four are published from tok / group-pool / name-hash sources.
pub const STATE_KEYS: &[&str] = &[
    KEY_CHARGE,
    KEY_CRYSTALS,
    KEY_PERSONA,
    KEY_GROUP,
    KEY_NO_HAND,
];

// -- typed CondExt -----------------------------------------------------------
//
// Assembled from the plain tables above. Host-side (`cel-ext` feature): the
// no_std guest card build only needs the tables. Leak-once (`AtomicPtr`)
// because `match` on `&str` is not const-stable (the getters / `bake` slots
// dispatch on the sugar flat id).

#[cfg(feature = "cel-ext")]
mod typed {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    use core::sync::atomic::{AtomicPtr, Ordering};

    use rules_cond::ext::{CondExt, LowerAs};
    use rules_cond::view::CondView;
    use rules_cond::vocab::{Fx, Getter, Name, Scope, StrGetter, Ty};
    use rules_cond_ir::ctx::FnId;

    use super::{LowerShape, Sugar, GUARD_CONSTS, KIND_CONSTS, SUGAR};

    const NO_GET: Getter = |_| 0;
    const NO_GET_STR: StrGetter = |_| Cow::Borrowed("");
    const NO_FX: Fx = Fx::Neighbor;

    /// Single-element `LowerAs::{Call,CallEq}.bake` slot.
    fn bake1(key: &'static str) -> &'static [&'static str] {
        Box::leak(Box::new([key]))
    }

    /// Scalar getter for an `owner.*` sugar row (non-capturing).
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

    /// Hidden CEL table variable for a sugar function. Historical names so a
    /// debug dump of the CEL context still reads familiar.
    fn intern_var(flat: &'static str) -> &'static str {
        match flat {
            "charge" => "_fire",
            "crystals" => "_crystals",
            "persona" => "_persona",
            "group" => "_group",
            "no_hand" => "_no_hand",
            "persona_is" => "_character",
            "group_is" => "_band",
            "blocked" => "_blocked",
            other => other,
        }
    }

    /// `f(seat)` sugar: `charge` / `crystals` / `persona` / `group` / `no_hand`.
    fn seat_int_field(flat: &str) -> fn(&dyn CondView, i64) -> i64 {
        match flat {
            "charge" => |v, s| v.state(s, "charge"),
            "crystals" => |v, s| v.state(s, "crystals"),
            "persona" => |v, s| v.state(s, "persona"),
            "group" => |v, s| v.state(s, "group"),
            "no_hand" => |v, s| v.state(s, "noHand"),
            _ => |_, _| 0,
        }
    }

    /// `f(seat, id)` sugar: `persona_is` / `group_is`.
    fn seat_int_is_field(flat: &str) -> fn(&dyn CondView, i64) -> i64 {
        match flat {
            "persona_is" => |v, s| v.state(s, "persona"),
            "group_is" => |v, s| v.state(s, "group"),
            _ => |_, _| 0,
        }
    }

    fn sugar_name(s: &Sugar) -> Name {
        let ty = if s.is_bool { Ty::Bool } else { Ty::Int };
        let scope = if s.is_func {
            Scope::Func {
                arity: s.arity,
                cand: s.cand,
            }
        } else {
            Scope::Candidate
        };
        let lower_as = match s.lower {
            LowerShape::CallState { bake } => LowerAs::Call {
                fid: FnId::State,
                bake: bake1(bake),
            },
            LowerShape::CallEqState { bake } => LowerAs::CallEq {
                fid: FnId::State,
                bake: bake1(bake),
            },
            LowerShape::CallCandHas { bake } => LowerAs::Call {
                fid: FnId::CandHas,
                bake: bake1(bake),
            },
            LowerShape::OwnerState { key } => LowerAs::OwnerState {
                fid: FnId::State,
                key,
            },
        };
        let get: Getter = if s.is_func {
            NO_GET
        } else {
            owner_get(s.flat)
        };
        let fx = if !s.is_func {
            NO_FX
        } else if s.is_bool && s.arity == 2 {
            Fx::SeatIntIs {
                var: intern_var(s.flat),
                field: seat_int_is_field(s.flat),
            }
        } else if s.is_bool && s.cand {
            Fx::IntHas {
                var: intern_var(s.flat),
                fill: |v: &dyn CondView| v.cand_set("blocked"),
            }
        } else if s.is_bool {
            NO_FX
        } else {
            Fx::SeatInt {
                var: intern_var(s.flat),
                field: seat_int_field(s.flat),
            }
        };
        Name {
            cel: s.cel,
            aliases: s.aliases,
            flat: s.flat,
            scope,
            ty,
            get,
            get_str: NO_GET_STR,
            fx,
            lower_as,
            doc: s.doc,
        }
    }

    /// The typed extension, built once from [`SUGAR`] / [`KIND_CONSTS`] /
    /// [`GUARD_CONSTS`]. Leak-once: `Name` rows carry `Box::leak` bake slots.
    /// `AtomicPtr` rather than `OnceLock` -- the crate is `no_std` on wasm32
    /// and this toolchain's `OnceLock` lives in `std` only. A racing second
    /// build leaks one extra table and loses the CAS; both are `'static`.
    pub fn ext() -> &'static CondExt {
        static EXT: AtomicPtr<CondExt> = AtomicPtr::new(core::ptr::null_mut());
        let p = EXT.load(Ordering::Acquire);
        if !p.is_null() {
            return unsafe { &*p };
        }
        let names: &'static [Name] = Box::leak(
            SUGAR
                .iter()
                .map(sugar_name)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        let built = Box::leak(Box::new(CondExt {
            names,
            kind_consts: KIND_CONSTS,
            guard_consts: GUARD_CONSTS,
        }));
        match EXT.compare_exchange(
            core::ptr::null_mut(),
            built as *mut CondExt,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => built,
            Err(existing) => unsafe { &*existing },
        }
    }
}

#[cfg(feature = "cel-ext")]
pub use typed::ext;

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the [`STATE_KEYS`] strings against the [`SUGAR`] bake /
    /// `OwnerState.key` fields. A rename on one side and not the other is a
    /// silent zero (brief 9.1).
    #[test]
    fn sugar_keys_match_state_keys() {
        let mut from_sugar: Vec<&str> = Vec::new();
        for s in SUGAR {
            match s.lower {
                LowerShape::CallState { bake } | LowerShape::CallEqState { bake } => {
                    from_sugar.push(bake)
                }
                LowerShape::OwnerState { key } => from_sugar.push(key),
                LowerShape::CallCandHas { .. } => {}
            }
        }
        from_sugar.sort_unstable();
        from_sugar.dedup();
        let mut expect: Vec<&str> = STATE_KEYS.to_vec();
        expect.sort_unstable();
        assert_eq!(
            from_sugar, expect,
            "SUGAR bake/OwnerState keys must be exactly STATE_KEYS"
        );
    }

    /// The `key` field on every state-backed row must agree with its
    /// lower-shape bake / OwnerState key.
    #[test]
    fn sugar_key_field_agrees_with_lower_shape() {
        for s in SUGAR {
            let lower_key = match s.lower {
                LowerShape::CallState { bake } | LowerShape::CallEqState { bake } => bake,
                LowerShape::OwnerState { key } => key,
                LowerShape::CallCandHas { bake } => bake,
            };
            assert_eq!(
                s.key, lower_key,
                "SUGAR[{}] key/bake mismatch",
                s.cel
            );
        }
    }
}
