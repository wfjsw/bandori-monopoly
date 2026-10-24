//! Per-kind filter payloads. One typed struct per payload family; no
//! `FilterValue` bag, no `UNCHANGED` sentinel, no intern-id tables.
//!
//! Bodies are pure: copy, tweak, return. [`FilterPayload::read`] snapshots the
//! host's current answer; [`FilterPayload::write`] applies only the fields the
//! body actually rewrote (`None` / unchanged = leave the host's answer alone).
//!
//! Flattening to `i32` happens only at the wasm import edge (`ctx::*` setters
//! and the host's `hostfns.rs`); these types are the vocabulary on both sides.

use game_kinds::{Dice, MoveKind, MsgKey, Payee, Seat, Tile};

/// Gate fold -- BuyGate, ActGate, PayGate, MoveGate, PlayGate, OnBoardGate,
/// TargetableGate, ExclusiveUse, AbnormalGuard, ImmuneAll, Untargetable.
/// `allowed: None` = leave the host's answer; `reason: None` = no message.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GateValue {
    pub allowed: Option<bool>,
    pub reason: Option<MsgKey>,
}

/// DiceRollPlan -- move-plan shaping. `None` = leave that field alone.
///
/// Dropped from the old `FilterValue` (never written by any body): `steps`,
/// `pay_factor`, `rent_factor`, `stop_at`, `settle_tile`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanValue {
    /// Who rolls; `None` = the mover.
    pub roller: Option<Seat>,
    /// Replace the base dice table.
    pub base: Option<Dice>,
    /// Append to the extra dice table.
    pub extra: Option<Dice>,
    /// Append to the base dice table.
    pub add_base: Option<Dice>,
    pub reverse: Option<bool>,
    /// Walk | Teleport.
    pub move_kind: Option<MoveKind>,
    pub teleport_to: Option<Tile>,
    pub resolve: Option<bool>,
    /// One-shot: `true` clears the extra dice now.
    pub clear_dice: bool,
    pub min_roll: Option<i32>,
    /// `plan::set_tag(key, value)`.
    pub tag: Option<(MsgKey, i32)>,
    /// Host-supplied recorded roll for this raise (`None` = none).
    pub recorded_roll: Option<i32>,
}

/// DiceRollValue -- the face plus host context.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RollValue {
    /// The face; read + written.
    pub value: i32,
    /// Host-supplied recorded roll (`None` = none for this raise).
    pub recorded_roll: Option<i32>,
    pub resolve: Option<bool>,
}

/// BuyAssign -- deal rewrite before the ownership write.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DealValue {
    pub owner: Option<Payee>,
    pub houses: Option<i32>,
    pub mortgaged: Option<bool>,
}

/// PaySplitAdd -- amount modifier that may also redirect the payee (`no_road`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PayAddValue {
    /// The running figure.
    pub value: i32,
    pub payee: Option<Payee>,
}

/// Redirect -- payee rewrite on a single-target effect (`exist::redirect`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RedirectValue {
    pub payee: Option<Payee>,
}

/// A typed filter payload: snapshot in, rewrite, apply the delta.
///
/// Always available so `drive_payload::<P>` can be packed into a `const`
/// registration on non-guest builds (manifest / pin tests). The `read` /
/// `write` bodies do their host IO only under `guest`; outside it they are
/// declaration-only.
pub trait FilterPayload: Copy + Default {
    /// Snapshot the fields this payload speaks. `Default` is "leave everything
    /// alone"; readable scalars (the running figure, the roll face) are filled
    /// from the host so a body can fold them.
    fn read() -> Self;
    /// Apply every field `after` rewrote relative to `before`. Called outside
    /// the pure region so the `ctx` setters are legal.
    fn write(before: Self, after: Self);
}

// ---------------------------------------------------------------------------
// Drive functions -- the **only** place `FilterEntry::body` is cast back to a
// typed `fn`. One per shape. Never call `body` from anywhere else.
// ---------------------------------------------------------------------------

/// `fn(i32, i32) -> i32` scalar fold. Reads `trigger::value()`, folds, writes
/// the running figure back through the existing `set_*` imports.
pub fn drive_value(player_id: i32, body: *const ()) {
    // SAFETY: `body` was packed by `kinds::filter_value` from a
    // `fn(i32, i32) -> i32` and is never reinterpreted as another shape.
    let body: fn(i32, i32) -> i32 = unsafe { core::mem::transmute(body) };
    #[cfg(any(target_arch = "wasm32", feature = "guest"))]
    {
        let v_in = crate::ctx::trigger::value();
        let v_out = {
            let _p = crate::pure::enter();
            body(player_id, v_in)
        };
        if v_out != v_in {
            crate::ctx::trigger::set_pay_amount(v_out);
            crate::ctx::trigger::set_price(v_out);
            crate::ctx::trigger::set_move_roll(v_out);
        }
    }
    #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
    {
        let _ = (player_id, body);
    }
}

/// `fn(i32, i32) -> bool` pure decision. `false` cancels the pipeline stage.
pub fn drive_flag(player_id: i32, body: *const ()) {
    // SAFETY: `body` was packed by `kinds::filter_flag` from a
    // `fn(i32, i32) -> bool`.
    let body: fn(i32, i32) -> bool = unsafe { core::mem::transmute(body) };
    #[cfg(any(target_arch = "wasm32", feature = "guest"))]
    {
        let keep = {
            let _p = crate::pure::enter();
            body(player_id, crate::ctx::trigger::value())
        };
        if !keep {
            crate::ctx::trigger::set_cancelled();
        }
    }
    #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
    {
        let _ = (player_id, body);
    }
}

/// `fn(i32, P) -> P` typed payload rewrite: snapshot, call (pure), apply delta.
/// Shared by every `payload(Ty)` / `gate` kind -- including downstream custom
/// payloads that impl [`FilterPayload`].
pub fn drive_payload<P: FilterPayload>(player_id: i32, body: *const ()) {
    // SAFETY: `body` was packed by `kinds::filter_payload::<P>` from a
    // `fn(i32, P) -> P` with the same `P`.
    let body: fn(i32, P) -> P = unsafe { core::mem::transmute(body) };
    #[cfg(any(target_arch = "wasm32", feature = "guest"))]
    {
        let before = P::read();
        let after = {
            let _p = crate::pure::enter();
            body(player_id, before)
        };
        P::write(before, after);
    }
    #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
    {
        let _ = (player_id, body);
    }
}

// ---------------------------------------------------------------------------
// Pure body calls (proofs / tests). Same one-cast-per-shape discipline as the
// `drive_*` fns above, but return the rewritten value and do no host IO.
// `drive_*` is the host read/call/write path; these are the value-in/value-out
// path used by `rules-kani::invoke::filter_*_idx`.
// ---------------------------------------------------------------------------

/// `fn(i32, i32) -> i32` pure fold. Sibling of [`drive_value`].
pub fn pure_value(player_id: i32, body: *const (), value: i32) -> i32 {
    // SAFETY: `body` was packed by `kinds::filter_value` from a
    // `fn(i32, i32) -> i32`.
    let body: fn(i32, i32) -> i32 = unsafe { core::mem::transmute(body) };
    body(player_id, value)
}

/// `fn(i32, i32) -> bool` pure decision. Sibling of [`drive_flag`].
pub fn pure_flag(player_id: i32, body: *const (), value: i32) -> bool {
    // SAFETY: `body` was packed by `kinds::filter_flag` from a
    // `fn(i32, i32) -> bool`.
    let body: fn(i32, i32) -> bool = unsafe { core::mem::transmute(body) };
    body(player_id, value)
}

/// `fn(i32, P) -> P` pure payload rewrite. Sibling of [`drive_payload`].
pub fn pure_payload<P: FilterPayload>(player_id: i32, body: *const (), value: P) -> P {
    // SAFETY: `body` was packed by `kinds::filter_payload::<P>` from a
    // `fn(i32, P) -> P` with the same `P`.
    let body: fn(i32, P) -> P = unsafe { core::mem::transmute(body) };
    body(player_id, value)
}

impl FilterPayload for GateValue {
    fn read() -> Self {
        Self::default()
    }

    fn write(before: Self, after: Self) {
        #[cfg(any(target_arch = "wasm32", feature = "guest"))]
        {
            if after.allowed == Some(false) && before.allowed != Some(false) {
                crate::ctx::trigger::set_cancelled();
            }
            if let Some(reason) = after.reason {
                if before.reason != Some(reason) {
                    crate::ctx::trigger::set_reason(reason);
                }
            }
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
        {
            let _ = (before, after);
        }
    }
}

impl FilterPayload for PlanValue {
    fn read() -> Self {
        #[cfg(any(target_arch = "wasm32", feature = "guest"))]
        {
            Self {
                recorded_roll: crate::ctx::plan::recorded_roll(),
                ..Self::default()
            }
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
        {
            Self::default()
        }
    }

    fn write(before: Self, after: Self) {
        #[cfg(any(target_arch = "wasm32", feature = "guest"))]
        {
            use crate::ctx::plan;
            if let Some(seat) = after.roller {
                if before.roller != Some(seat) {
                    plan::set_roller(seat.0);
                }
            }
            if let Some(d) = after.base {
                if before.base != Some(d) {
                    plan::set_base_dice(d.count, d.sides, "filter");
                }
            }
            if let Some(d) = after.extra {
                if before.extra != Some(d) {
                    plan::add_extra_dice(d.count, d.sides, "filter");
                }
            }
            if let Some(d) = after.add_base {
                if before.add_base != Some(d) {
                    plan::add_base_dice(d.count, d.sides, "filter");
                }
            }
            if let Some(on) = after.reverse {
                if before.reverse != Some(on) {
                    plan::set_reverse(on);
                }
            }
            if let Some(k) = after.move_kind {
                if before.move_kind != Some(k) {
                    plan::set_kind(k);
                }
            }
            if let Some(t) = after.teleport_to {
                if before.teleport_to != Some(t) {
                    plan::set_teleport_to(t.0);
                }
            }
            if let Some(on) = after.resolve {
                if before.resolve != Some(on) {
                    plan::set_resolve(on);
                }
            }
            if after.clear_dice {
                plan::clear_dice();
            }
            if let Some(n) = after.min_roll {
                if before.min_roll != Some(n) {
                    plan::set_min_roll(n);
                }
            }
            if let Some((key, value)) = after.tag {
                if before.tag != Some((key, value)) {
                    plan::set_tag(key, value);
                }
            }
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
        {
            let _ = (before, after);
        }
    }
}

impl FilterPayload for RollValue {
    fn read() -> Self {
        #[cfg(any(target_arch = "wasm32", feature = "guest"))]
        {
            Self {
                // Running face, not the raise's original `value`. A move raise
                // seeds `mv.roll = None` and puts the face on `trigger.value`;
                // the first `set_move_roll` parks the rewrite on `mv.roll` and
                // never touches `value`. Reading `value` would make every
                // later DiceRollValue body fold the *original* face and the
                // last writer win (`moca_half` x `no_expectation` composed
                // 7 -> 5, dropping the half). Same idiom as the roll skills
                // (`move_roll().unwrap_or(value())`).
                value: crate::ctx::trigger::move_roll()
                    .unwrap_or_else(|| crate::ctx::trigger::value()),
                recorded_roll: crate::ctx::plan::recorded_roll(),
                resolve: None,
            }
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
        {
            Self::default()
        }
    }

    fn write(before: Self, after: Self) {
        #[cfg(any(target_arch = "wasm32", feature = "guest"))]
        {
            if after.value != before.value {
                crate::ctx::trigger::set_move_roll(after.value);
            }
            if let Some(on) = after.resolve {
                if before.resolve != Some(on) {
                    crate::ctx::plan::set_resolve(on);
                }
            }
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
        {
            let _ = (before, after);
        }
    }
}

impl FilterPayload for DealValue {
    fn read() -> Self {
        // `None` = leave the host's deal alone. A body that needs the standing
        // owner / houses / mortgage reads them through `ctx::trigger`.
        Self::default()
    }

    fn write(before: Self, after: Self) {
        #[cfg(any(target_arch = "wasm32", feature = "guest"))]
        {
            use crate::ctx::trigger;
            if let Some(owner) = after.owner {
                if before.owner != Some(owner) {
                    trigger::set_deal_owner(owner.as_i32());
                }
            }
            if let Some(h) = after.houses {
                if before.houses != Some(h) {
                    trigger::set_deal_houses(h);
                }
            }
            if let Some(m) = after.mortgaged {
                if before.mortgaged != Some(m) {
                    trigger::set_deal_mortgaged(m);
                }
            }
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
        {
            let _ = (before, after);
        }
    }
}

impl FilterPayload for PayAddValue {
    fn read() -> Self {
        #[cfg(any(target_arch = "wasm32", feature = "guest"))]
        {
            Self {
                value: crate::ctx::trigger::value(),
                payee: None,
            }
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
        {
            Self::default()
        }
    }

    fn write(before: Self, after: Self) {
        #[cfg(any(target_arch = "wasm32", feature = "guest"))]
        {
            if after.value != before.value {
                crate::ctx::trigger::set_pay_amount(after.value);
                crate::ctx::trigger::set_price(after.value);
            }
            if let Some(p) = after.payee {
                if before.payee != Some(p) {
                    crate::ctx::trigger::set_pay_target(p.as_i32());
                }
            }
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
        {
            let _ = (before, after);
        }
    }
}

impl FilterPayload for RedirectValue {
    fn read() -> Self {
        Self::default()
    }

    fn write(before: Self, after: Self) {
        #[cfg(any(target_arch = "wasm32", feature = "guest"))]
        {
            if let Some(p) = after.payee {
                if before.payee != Some(p) {
                    crate::ctx::trigger::set_pay_target(p.as_i32());
                }
            }
        }
        #[cfg(not(any(target_arch = "wasm32", feature = "guest")))]
        {
            let _ = (before, after);
        }
    }
}
