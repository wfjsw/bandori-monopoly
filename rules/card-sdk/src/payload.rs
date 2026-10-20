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

/// Temporary shape-erasure for `On::FilterValue` bodies until Phase 3's
/// `FilterEntry` drive. One arm per payload; registration picks the arm.
/// The list form of `On::FilterValue(&[FilterKind], pre, guard, body)` is
/// unchanged -- only the body type moves.
#[derive(Clone, Copy, Debug)]
pub enum FilterBody {
    Plan(fn(i32, PlanValue) -> PlanValue),
    Deal(fn(i32, DealValue) -> DealValue),
    Gate(fn(i32, GateValue) -> GateValue),
    Redirect(fn(i32, RedirectValue) -> RedirectValue),
    PayAdd(fn(i32, PayAddValue) -> PayAddValue),
    Roll(fn(i32, RollValue) -> RollValue),
}

/// A typed filter payload: snapshot in, rewrite, apply the delta.
#[cfg(any(target_arch = "wasm32", feature = "guest"))]
pub trait FilterPayload: Copy + Default {
    /// Snapshot the fields this payload speaks. `Default` is "leave everything
    /// alone"; readable scalars (the running figure, the roll face) are filled
    /// from the host so a body can fold them.
    fn read() -> Self;
    /// Apply every field `after` rewrote relative to `before`. Called outside
    /// the pure region so the `ctx` setters are legal.
    fn write(before: Self, after: Self);
}

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
impl FilterBody {
    /// Run the body over its payload: snapshot, call (pure), apply the delta.
    pub fn run(self, player_id: i32) {
        match self {
            FilterBody::Plan(body) => drive(player_id, body),
            FilterBody::Deal(body) => drive(player_id, body),
            FilterBody::Gate(body) => drive(player_id, body),
            FilterBody::Redirect(body) => drive(player_id, body),
            FilterBody::PayAdd(body) => drive(player_id, body),
            FilterBody::Roll(body) => drive(player_id, body),
        }
    }
}

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
fn drive<P: FilterPayload>(player_id: i32, body: fn(i32, P) -> P) {
    let before = P::read();
    let after = {
        let _p = crate::pure::enter();
        body(player_id, before)
    };
    P::write(before, after);
}

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
impl FilterPayload for GateValue {
    fn read() -> Self {
        Self::default()
    }

    fn write(before: Self, after: Self) {
        if after.allowed == Some(false) && before.allowed != Some(false) {
            crate::ctx::trigger::set_cancelled();
        }
        if let Some(reason) = after.reason {
            if before.reason != Some(reason) {
                crate::ctx::trigger::set_reason(reason);
            }
        }
    }
}

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
impl FilterPayload for PlanValue {
    fn read() -> Self {
        Self {
            recorded_roll: crate::ctx::plan::recorded_roll(),
            ..Self::default()
        }
    }

    fn write(before: Self, after: Self) {
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
}

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
impl FilterPayload for RollValue {
    fn read() -> Self {
        Self {
            value: crate::ctx::trigger::value(),
            recorded_roll: crate::ctx::plan::recorded_roll(),
            resolve: None,
        }
    }

    fn write(before: Self, after: Self) {
        if after.value != before.value {
            crate::ctx::trigger::set_move_roll(after.value);
        }
        if let Some(on) = after.resolve {
            if before.resolve != Some(on) {
                crate::ctx::plan::set_resolve(on);
            }
        }
    }
}

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
impl FilterPayload for DealValue {
    fn read() -> Self {
        // `None` = leave the host's deal alone. A body that needs the standing
        // owner / houses / mortgage reads them through `ctx::trigger`.
        Self::default()
    }

    fn write(before: Self, after: Self) {
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
}

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
impl FilterPayload for PayAddValue {
    fn read() -> Self {
        Self {
            value: crate::ctx::trigger::value(),
            payee: None,
        }
    }

    fn write(before: Self, after: Self) {
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
}

#[cfg(any(target_arch = "wasm32", feature = "guest"))]
impl FilterPayload for RedirectValue {
    fn read() -> Self {
        Self::default()
    }

    fn write(before: Self, after: Self) {
        if let Some(p) = after.payee {
            if before.payee != Some(p) {
                crate::ctx::trigger::set_pay_target(p.as_i32());
            }
        }
    }
}