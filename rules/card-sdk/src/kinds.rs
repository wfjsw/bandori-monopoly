//! Per-kind registration constructors for the `On` DSL.
//!
//! [`filter_kinds!`] / [`action_kinds!`] emit a namespace unit struct with one
//! PascalCase `const fn` per kind, plus a `KINDS: &'static [KindInfo]` registry
//! for pin tests and tooling. Engine kind *data* (ids, wire spellings,
//! `is_filter`) stays in `crates/game-kinds`; these macros generate the
//! registration constructors only -- they do not author kind ids.
//!
//! Call sites (no macro, no `.entry()`):
//!
//! ```ignore
//! On::Filter(Filter::DiceRollPlan("", None, on_plan)),
//! On::Filter(Filters::MyRewrite("", None, on_x)),   // ruleset custom kind
//! On::Action(Action::TurnStart("", None, on_turn_start).counter_reason("turn")),
//! ```
//!
//! # The one unsafe corner
//!
//! [`FilterEntry::body`] is a `*const ()` so one entry type can hold any
//! payload body (`fn(i32, PlanValue) -> PlanValue`, `fn(i32, i32) -> i32`, …).
//! It is **only** cast back to a typed `fn` pointer inside the matching
//! [`crate::payload`] `drive_*` free function -- one per shape. Never call
//! `body` directly. Constructors pack plain fn pointers (no closures, no trait
//! method calls) so `CardDef::new` stays `const fn` and `static CARDS` stays
//! const-built.

#[cfg(target_arch = "wasm32")]
use alloc::borrow::Cow;
#[cfg(not(target_arch = "wasm32"))]
use std::borrow::Cow;

use crate::abi::{ActionKind, FilterKind, TriggerKind};
use crate::payload::{drive_flag, drive_payload, drive_value, FilterPayload};
use crate::Asked;

/// Which body shape a filter kind speaks. Host / kani use this to pick the
/// right trampoline; the matching `drive_*` is what casts [`FilterEntry::body`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterShape {
    /// `fn(i32, i32) -> i32` -- pure scalar fold (PaySplitMul, BuyAdd, …).
    Value,
    /// `fn(i32, i32) -> bool` -- pure decision (PaySplitChoose, PayCombinedCancel).
    Flag,
    /// `fn(i32, GateValue) -> GateValue` -- gate fold.
    Gate,
    /// `fn(i32, P) -> P` for a [`crate::payload::FilterPayload`] family
    /// (`PlanValue`, `RollValue`, `DealValue`, `PayAddValue`, `RedirectValue`).
    Payload,
}

/// One registry row: the declaration side of a kind, for pin tests / tooling.
#[derive(Clone, Copy, Debug)]
pub struct KindInfo {
    /// PascalCase constructor name, e.g. `"DiceRollPlan"`.
    pub name: &'static str,
    /// Engine wire spelling (`"diceRollPlan"`). Empty for ruleset rows --
    /// those spell as `"r:{key}:{id}"` via [`Self::spelling`] /
    /// [`TriggerKind::spelling`] (a `const` table cannot `format!`).
    pub wire: &'static str,
    /// The namespaced wire id.
    pub kind: TriggerKind,
    /// Filter-pipeline kind (`On::Filter`) vs action / hook.
    pub is_filter: bool,
    /// Body shape (filters only; actions are `fn(i32) -> Asked`).
    pub shape: FilterShape,
}

impl KindInfo {
    /// The wire spelling: the engine table's camelCase, or
    /// `"r:{key}:{id}"` for a ruleset row.
    pub fn spelling(&self) -> Cow<'static, str> {
        if self.wire.is_empty() {
            self.kind.spelling()
        } else {
            Cow::Borrowed(self.wire)
        }
    }
}

/// One filter registration: a pure value / decision / gate / payload rewrite
/// at a single kind. Produced by the `filter_kinds!` constructors.
///
/// `drive` monomorphizes the body's payload family and is the **only** place
/// `body` is cast back to a typed `fn`. `rt` calls `(e.drive)(player_id, e.body)`.
#[derive(Clone, Copy)]
pub struct FilterEntry {
    pub kind: FilterKind,
    pub pre: &'static str,
    pub guard: Option<fn(i32) -> bool>,
    /// Monomorphized driver; casts `body` back to `fn(i32, P) -> P`.
    pub drive: fn(i32, *const ()),
    /// Erased body. See the module docs: only `drive` may cast this.
    pub body: *const (),
    pub shape: FilterShape,
}

// Safety: `body` always points at a `fn` item in static code (constructed by
// the `const fn` constructors below from a `fn` pointer). Code is immutable
// and `Sync`; the pointer is never freed.
unsafe impl Send for FilterEntry {}
unsafe impl Sync for FilterEntry {}

/// One action registration: an event reaction at a single kind.
#[derive(Clone, Copy)]
pub struct ActionEntry {
    pub kind: ActionKind,
    pub pre: &'static str,
    pub guard: Option<fn(i32) -> bool>,
    pub body: fn(i32) -> Asked,
    /// The 「…时」 clause the counteraction prompt shows (locale key; `None` =
    /// no clause). Attach with [`ActionEntry::counter_reason`].
    pub counter_reason: Option<crate::MsgKey>,
}

impl ActionEntry {
    /// Attach the 「…时」 clause the counteraction prompt shows (a locale key).
    /// `None` (the default) = no clause.
    ///
    /// ```ignore
    /// On::Action(Action::TurnEnd("", None, tick).counter_reason(key!("why")))
    /// ```
    pub const fn counter_reason(mut self, reason: crate::MsgKey) -> Self {
        self.counter_reason = Some(reason);
        self
    }
}

/// Build a [`FilterEntry`] for a `fn(i32, i32) -> i32` fold.
#[inline]
pub const fn filter_value(
    kind: FilterKind,
    pre: &'static str,
    guard: Option<fn(i32) -> bool>,
    body: fn(i32, i32) -> i32,
) -> FilterEntry {
    FilterEntry {
        kind,
        pre,
        guard,
        drive: drive_value,
        body: body as *const (),
        shape: FilterShape::Value,
    }
}

/// Build a [`FilterEntry`] for a `fn(i32, i32) -> bool` decision.
#[inline]
pub const fn filter_flag(
    kind: FilterKind,
    pre: &'static str,
    guard: Option<fn(i32) -> bool>,
    body: fn(i32, i32) -> bool,
) -> FilterEntry {
    FilterEntry {
        kind,
        pre,
        guard,
        drive: drive_flag,
        body: body as *const (),
        shape: FilterShape::Flag,
    }
}

/// Build a [`FilterEntry`] for a `fn(i32, P) -> P` payload rewrite.
#[inline]
pub const fn filter_payload<P: FilterPayload>(
    kind: FilterKind,
    pre: &'static str,
    guard: Option<fn(i32) -> bool>,
    shape: FilterShape,
    drive: fn(i32, *const ()),
    body: fn(i32, P) -> P,
) -> FilterEntry {
    FilterEntry {
        kind,
        pre,
        guard,
        drive,
        body: body as *const (),
        shape,
    }
}

/// Build an [`ActionEntry`]. Attach the 「…时」 clause with
/// [`ActionEntry::counter_reason`].
#[inline]
pub const fn action(
    kind: ActionKind,
    pre: &'static str,
    guard: Option<fn(i32) -> bool>,
    body: fn(i32) -> Asked,
) -> ActionEntry {
    ActionEntry {
        kind,
        pre,
        guard,
        body,
        counter_reason: None,
    }
}

/// One `filter_kinds!` / `action_kinds!` arm's registry row.
///
/// `$ns_ty` is the generated namespace unit struct name.
#[macro_export]
macro_rules! filter_kinds {
    // ---- engine namespace: ids come from `FilterKind::Name` (game-kinds) ----
    (
        on $ns_ty:ident;
        ns KindNs::Engine;
        $($name:ident => $shape:ident $( ( $ty:ty ) )? ;)*
    ) => {
        $crate::filter_kinds!(@ns $ns_ty; KindNs::Engine; $($name => $shape $( ( $ty ) )? ;) *);
        $crate::filter_kinds!(@engine_entries $ns_ty; $($name => $shape $( ( $ty ) )? ;) *);
        impl $ns_ty {
            /// Every declared kind, in table order.
            pub const KINDS: &'static [$crate::kinds::KindInfo] = &[
                $($crate::filter_kinds!(@info_engine $name => $shape $( ( $ty ) )? )),*
            ];
        }
    };
    // ---- ruleset namespace: the ruleset authors its own ids (`Name @ id`) ----
    (
        on $ns_ty:ident;
        ns KindNs::Ruleset ( $key:expr );
        $($name:ident @ $id:expr => $shape:ident $( ( $ty:ty ) )? ;)*
    ) => {
        $crate::filter_kinds!(@ns $ns_ty; KindNs::Ruleset($key); $($name => $shape $( ( $ty ) )? ;) *);
        $crate::filter_kinds!(@ruleset_entries $key; $ns_ty; $($name @ $id => $shape $( ( $ty ) )? ;) *);
        impl $ns_ty {
            /// Every declared kind, in table order.
            pub const KINDS: &'static [$crate::kinds::KindInfo] = &[
                $($crate::filter_kinds!(@info_ruleset $key; $name @ $id => $shape $( ( $ty ) )? )),*
            ];
        }
    };
    // ---- namespace type ----
    (@ns $ns_ty:ident; $($ns:tt)*) => {
        /// Namespace unit struct: PascalCase `const fn` per kind.
        #[derive(Clone, Copy, Debug)]
        pub struct $ns_ty;
    };
    // ---- engine constructor table ----
    (@engine_entries $ns_ty:ident; $($name:ident => $shape:ident $( ( $ty:ty ) )? ;) *) => {
        #[allow(non_upper_case_globals, non_snake_case)]
        impl $ns_ty {
            $(
                $crate::filter_kinds!(@ctor_engine $name => $shape $( ( $ty ) )? );
            )*
        }
    };
    (@ctor_engine $name:ident => value) => {
        /// Registration constructor. Body is `fn(i32, i32) -> i32`.
        pub const fn $name(
            pre: &'static str,
            guard: Option<fn(i32) -> bool>,
            body: fn(i32, i32) -> i32,
        ) -> $crate::kinds::FilterEntry {
            $crate::kinds::filter_value($crate::abi::FilterKind::$name, pre, guard, body)
        }
    };
    (@ctor_engine $name:ident => flag) => {
        /// Registration constructor. Body is `fn(i32, i32) -> bool`.
        pub const fn $name(
            pre: &'static str,
            guard: Option<fn(i32) -> bool>,
            body: fn(i32, i32) -> bool,
        ) -> $crate::kinds::FilterEntry {
            $crate::kinds::filter_flag($crate::abi::FilterKind::$name, pre, guard, body)
        }
    };
    (@ctor_engine $name:ident => gate) => {
        /// Registration constructor. Body is `fn(i32, GateValue) -> GateValue`.
        pub const fn $name(
            pre: &'static str,
            guard: Option<fn(i32) -> bool>,
            body: fn(i32, $crate::payload::GateValue) -> $crate::payload::GateValue,
        ) -> $crate::kinds::FilterEntry {
            $crate::kinds::filter_payload::<$crate::payload::GateValue>(
                $crate::abi::FilterKind::$name,
                pre,
                guard,
                $crate::kinds::FilterShape::Gate,
                $crate::payload::drive_payload::<$crate::payload::GateValue>,
                body,
            )
        }
    };
    (@ctor_engine $name:ident => payload ( $ty:ty )) => {
        /// Registration constructor. Body is `fn(i32, $ty) -> $ty`.
        pub const fn $name(
            pre: &'static str,
            guard: Option<fn(i32) -> bool>,
            body: fn(i32, $ty) -> $ty,
        ) -> $crate::kinds::FilterEntry {
            $crate::kinds::filter_payload::<$ty>(
                $crate::abi::FilterKind::$name,
                pre,
                guard,
                $crate::kinds::FilterShape::Payload,
                $crate::payload::drive_payload::<$ty>,
                body,
            )
        }
    };
    // ---- ruleset constructor table ----
    (@ruleset_entries $key:expr; $ns_ty:ident; $($name:ident @ $id:expr => $shape:ident $( ( $ty:ty ) )? ;) *) => {
        #[allow(non_upper_case_globals, non_snake_case)]
        impl $ns_ty {
            $(
                $crate::filter_kinds!(@ctor_ruleset $key; $name @ $id => $shape $( ( $ty ) )? );
            )*
        }
    };
    (@ctor_ruleset $key:expr; $name:ident @ $id:expr => value) => {
        /// Registration constructor. Body is `fn(i32, i32) -> i32`.
        pub const fn $name(
            pre: &'static str,
            guard: Option<fn(i32) -> bool>,
            body: fn(i32, i32) -> i32,
        ) -> $crate::kinds::FilterEntry {
            $crate::kinds::filter_value(
                $crate::abi::FilterKind::ruleset($key, $id),
                pre,
                guard,
                body,
            )
        }
    };
    (@ctor_ruleset $key:expr; $name:ident @ $id:expr => flag) => {
        /// Registration constructor. Body is `fn(i32, i32) -> bool`.
        pub const fn $name(
            pre: &'static str,
            guard: Option<fn(i32) -> bool>,
            body: fn(i32, i32) -> bool,
        ) -> $crate::kinds::FilterEntry {
            $crate::kinds::filter_flag(
                $crate::abi::FilterKind::ruleset($key, $id),
                pre,
                guard,
                body,
            )
        }
    };
    (@ctor_ruleset $key:expr; $name:ident @ $id:expr => gate) => {
        /// Registration constructor. Body is `fn(i32, GateValue) -> GateValue`.
        pub const fn $name(
            pre: &'static str,
            guard: Option<fn(i32) -> bool>,
            body: fn(i32, $crate::payload::GateValue) -> $crate::payload::GateValue,
        ) -> $crate::kinds::FilterEntry {
            $crate::kinds::filter_payload::<$crate::payload::GateValue>(
                $crate::abi::FilterKind::ruleset($key, $id),
                pre,
                guard,
                $crate::kinds::FilterShape::Gate,
                $crate::payload::drive_payload::<$crate::payload::GateValue>,
                body,
            )
        }
    };
    (@ctor_ruleset $key:expr; $name:ident @ $id:expr => payload ( $ty:ty )) => {
        /// Registration constructor. Body is `fn(i32, $ty) -> $ty`.
        pub const fn $name(
            pre: &'static str,
            guard: Option<fn(i32) -> bool>,
            body: fn(i32, $ty) -> $ty,
        ) -> $crate::kinds::FilterEntry {
            $crate::kinds::filter_payload::<$ty>(
                $crate::abi::FilterKind::ruleset($key, $id),
                pre,
                guard,
                $crate::kinds::FilterShape::Payload,
                $crate::payload::drive_payload::<$ty>,
                body,
            )
        }
    };
    // ---- registry rows ----
    (@info_engine $name:ident => value) => {
        $crate::kinds::KindInfo {
            name: stringify!($name),
            wire: game_kinds::wire($crate::abi::TriggerKind::$name.id),
            kind: $crate::abi::TriggerKind::$name,
            is_filter: true,
            shape: $crate::kinds::FilterShape::Value,
        }
    };
    (@info_engine $name:ident => flag) => {
        $crate::kinds::KindInfo {
            name: stringify!($name),
            wire: game_kinds::wire($crate::abi::TriggerKind::$name.id),
            kind: $crate::abi::TriggerKind::$name,
            is_filter: true,
            shape: $crate::kinds::FilterShape::Flag,
        }
    };
    (@info_engine $name:ident => gate) => {
        $crate::kinds::KindInfo {
            name: stringify!($name),
            wire: game_kinds::wire($crate::abi::TriggerKind::$name.id),
            kind: $crate::abi::TriggerKind::$name,
            is_filter: true,
            shape: $crate::kinds::FilterShape::Gate,
        }
    };
    (@info_engine $name:ident => payload ( $ty:ty )) => {
        $crate::kinds::KindInfo {
            name: stringify!($name),
            wire: game_kinds::wire($crate::abi::TriggerKind::$name.id),
            kind: $crate::abi::TriggerKind::$name,
            is_filter: true,
            shape: $crate::kinds::FilterShape::Payload,
        }
    };
    (@info_ruleset $key:expr; $name:ident @ $id:expr => $shape:ident $( ( $ty:ty ) )?) => {{
        const K: $crate::abi::TriggerKind = $crate::abi::TriggerKind::ruleset($key, $id);
        $crate::kinds::KindInfo {
            name: stringify!($name),
            wire: "",
            kind: K,
            is_filter: true,
            shape: $crate::filter_kinds!(@shape_only $shape $( ( $ty ) )?),
        }
    }};
    (@shape_only value) => { $crate::kinds::FilterShape::Value };
    (@shape_only flag) => { $crate::kinds::FilterShape::Flag };
    (@shape_only gate) => { $crate::kinds::FilterShape::Gate };
    (@shape_only payload ( $ty:ty )) => { $crate::kinds::FilterShape::Payload };
}

/// Declare an action-kind namespace. One PascalCase `const fn` per kind.
///
/// ```ignore
/// action_kinds! {
///     on Action;
///     ns KindNs::Engine;
///     TurnStart;   // body fn(i32) -> Asked
/// }
/// ```
#[macro_export]
macro_rules! action_kinds {
    // ---- engine namespace ----
    (
        on $ns_ty:ident;
        ns KindNs::Engine;
        $($name:ident ;)*
    ) => {
        $crate::action_kinds!(@ns $ns_ty; $($name ;) *);
        $crate::action_kinds!(@engine_entries $ns_ty; $($name ;) *);
        impl $ns_ty {
            /// Every declared kind, in table order.
            pub const KINDS: &'static [$crate::kinds::KindInfo] = &[
                $($crate::action_kinds!(@info_engine $name)),*
            ];
        }
    };
    // ---- ruleset namespace ----
    (
        on $ns_ty:ident;
        ns KindNs::Ruleset ( $key:expr );
        $($name:ident @ $id:expr ;)*
    ) => {
        $crate::action_kinds!(@ns $ns_ty; $($name ;) *);
        $crate::action_kinds!(@ruleset_entries $key; $ns_ty; $($name @ $id ;) *);
        impl $ns_ty {
            /// Every declared kind, in table order.
            pub const KINDS: &'static [$crate::kinds::KindInfo] = &[
                $($crate::action_kinds!(@info_ruleset $key; $name @ $id)),*
            ];
        }
    };
    (@ns $ns_ty:ident; $($name:ident ;) *) => {
        /// Namespace unit struct: PascalCase `const fn` per kind.
        #[derive(Clone, Copy, Debug)]
        pub struct $ns_ty;
    };
    (@engine_entries $ns_ty:ident; $($name:ident ;) *) => {
        #[allow(non_upper_case_globals, non_snake_case)]
        impl $ns_ty {
            $(
                /// Registration constructor. Body is `fn(i32) -> Asked`.
                /// Attach the 「…时」 clause with
                /// [`$crate::kinds::ActionEntry::counter_reason`].
                pub const fn $name(
                    pre: &'static str,
                    guard: Option<fn(i32) -> bool>,
                    body: fn(i32) -> $crate::Asked,
                ) -> $crate::kinds::ActionEntry {
                    $crate::kinds::action(
                        $crate::abi::ActionKind::$name,
                        pre,
                        guard,
                        body,
                    )
                }
            )*
        }
    };
    (@ruleset_entries $key:expr; $ns_ty:ident; $($name:ident @ $id:expr ;) *) => {
        #[allow(non_upper_case_globals, non_snake_case)]
        impl $ns_ty {
            $(
                /// Registration constructor. Body is `fn(i32) -> Asked`.
                /// Attach the 「…时」 clause with
                /// [`$crate::kinds::ActionEntry::counter_reason`].
                pub const fn $name(
                    pre: &'static str,
                    guard: Option<fn(i32) -> bool>,
                    body: fn(i32) -> $crate::Asked,
                ) -> $crate::kinds::ActionEntry {
                    $crate::kinds::action(
                        $crate::abi::ActionKind::ruleset($key, $id),
                        pre,
                        guard,
                        body,
                    )
                }
            )*
        }
    };
    (@info_engine $name:ident) => {
        $crate::kinds::KindInfo {
            name: stringify!($name),
            wire: game_kinds::wire($crate::abi::TriggerKind::$name.id),
            kind: $crate::abi::TriggerKind::$name,
            is_filter: false,
            shape: $crate::kinds::FilterShape::Value,
        }
    };
    (@info_ruleset $key:expr; $name:ident @ $id:expr) => {{
        const K: $crate::abi::TriggerKind = $crate::abi::TriggerKind::ruleset($key, $id);
        $crate::kinds::KindInfo {
            name: stringify!($name),
            wire: "",
            kind: K,
            is_filter: false,
            shape: $crate::kinds::FilterShape::Value,
        }
    }};
}

// ---------------------------------------------------------------------------
// Engine tables (ids referenced, never authored here)
// ---------------------------------------------------------------------------

filter_kinds! {
    on Filter;
    ns KindNs::Engine;
    PaySplitChoose => flag;
    PaySplitAdd => payload(crate::payload::PayAddValue);
    PaySplitMul => value;
    PaySplitAt => value;
    DiceRollPlan => payload(crate::payload::PlanValue);
    AbnormalGuard => gate;
    ImmuneAll => gate;
    Untargetable => gate;
    Redirect => payload(crate::payload::RedirectValue);
    BuyGate => gate;
    BuyAdd => value;
    BuyMul => value;
    BuySet => value;
    BuyAssign => payload(crate::payload::DealValue);
    PayCombinedAdd => value;
    PayCombinedMul => value;
    PayCombinedCancel => flag;
    ActGate => gate;
    PayGate => gate;
    MoveGate => gate;
    ExclusiveUse => gate;
    TurnBonusTime => value;
    TurnBaseTime => value;
    PlayGate => gate;
    DiceRollValue => payload(crate::payload::RollValue);
    OnBoardGate => gate;
    TargetableGate => gate;
}

action_kinds! {
    on Action;
    ns KindNs::Engine;
    None;
    DiceRollBefore;
    DiceRollResult;
    TurnStart;
    Pass;
    PassPlayer;
    SettleBefore;
    Settle;
    Mortgage;
    Pay;
    Paid;
    Bankrupt;
    Card;
    Event;
    Abnormal;
    Target;
    State;
    Counteracted;
    DrawOut;
    TwoCards;
    TurnStartBefore;
    PassBefore;
    MortgageBefore;
    BankruptBefore;
    CardAfter;
    EventAfter;
    SettleAfter;
    BuyBefore;
    BuyAfter;
    BuildBefore;
    BuildAfter;
    DiscardBefore;
    DiscardAfter;
    EndTurnBefore;
    EndTurnAfter;
    LeaveBefore;
    LeaveAfter;
    TurnEnd;
    Drawn;
    PassTile;
    PayAfter;
    DiceRollAfter;
    CardPlayed;
    Targeted;
    TurnEndBefore;
    TurnEndAfter;
    Discarded;
    DeckBeforeGame;
    DeckAtGameStart;
    Drew;
    Reshuffled;
    Bought;
    SettleBody;
    BeforeOut;
    Teleported;
    MarkerSpent;
    CardUsed;
    HouseAdded;
    CounterChanged;
    DrewBefore;
    Effect;
    TileResolved;
    MoveResolved;
    BankruptResolved;
    MoveBefore;
    MoveAfter;
    MarkerSpend;
    MarkerGain;
    AtEnd;
    PaySplitChooseBefore;
    PayCombinedAddBefore;
    PaySplitMulBefore;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::{GateValue, PayAddValue, PlanValue, RollValue};

    fn value_body(_p: i32, v: i32) -> i32 {
        v
    }
    fn flag_body(_p: i32, v: i32) -> bool {
        v != 0
    }
    fn gate_body(_p: i32, v: GateValue) -> GateValue {
        v
    }
    fn plan_body(_p: i32, v: PlanValue) -> PlanValue {
        v
    }

    #[test]
    fn filter_kinds_registry_matches_table() {
        assert_eq!(Filter::KINDS.len(), 27);
        assert!(Filter::KINDS.iter().all(|k| k.is_filter));
        for k in Filter::KINDS {
            assert_eq!(k.kind.is_filter(), true, "{}", k.name);
            assert!(!k.wire.is_empty(), "{}", k.name);
        }
        let add = Filter::KINDS
            .iter()
            .find(|k| k.name == "PaySplitAdd")
            .unwrap();
        assert_eq!(add.wire, "paySplitAdd");
        assert_eq!(add.shape, FilterShape::Payload);
        let choose = Filter::KINDS
            .iter()
            .find(|k| k.name == "PaySplitChoose")
            .unwrap();
        assert_eq!(choose.shape, FilterShape::Flag);
        let gate = Filter::KINDS.iter().find(|k| k.name == "ActGate").unwrap();
        assert_eq!(gate.shape, FilterShape::Gate);
    }

    #[test]
    fn action_kinds_registry_matches_table() {
        assert!(!Action::KINDS.is_empty());
        assert!(Action::KINDS.iter().all(|k| !k.is_filter));
        for k in Action::KINDS {
            assert!(!k.kind.is_filter(), "{}", k.name);
        }
    }

    #[test]
    fn constructors_pack_shapes() {
        let v = Filter::PaySplitMul("", None, value_body);
        assert_eq!(v.shape, FilterShape::Value);
        assert_eq!(v.kind, FilterKind::PaySplitMul);

        let f = Filter::PaySplitChoose("", None, flag_body);
        assert_eq!(f.shape, FilterShape::Flag);

        let g = Filter::ActGate("", None, gate_body);
        assert_eq!(g.shape, FilterShape::Gate);

        let p = Filter::DiceRollPlan("", None, plan_body);
        assert_eq!(p.shape, FilterShape::Payload);
        assert_eq!(p.kind, FilterKind::DiceRollPlan);

        fn act(_p: i32) -> Asked {
            Ok(())
        }
        let a = Action::TurnStart(crate::pre::MINE, None, act).counter_reason("turn");
        assert_eq!(a.kind, ActionKind::TurnStart);
        assert_eq!(a.pre, "actor == owner");
        assert_eq!(a.counter_reason, Some("turn"));
    }

    #[test]
    fn gate_value_drive_is_shared() {
        // `gate` is sugar for `payload(GateValue)` -- same drive.
        let g = Filter::ImmuneAll("", None, gate_body);
        assert_eq!(
            g.drive as usize,
            drive_payload::<GateValue> as *const () as usize
        );
        let p = Filter::PaySplitAdd("", None, |_p, v: PayAddValue| v);
        assert_eq!(
            p.drive as usize,
            drive_payload::<PayAddValue> as *const () as usize
        );
        let r = Filter::DiceRollValue("", None, |_p, v: RollValue| v);
        assert_eq!(
            r.drive as usize,
            drive_payload::<RollValue> as *const () as usize
        );
        let v = Filter::BuyAdd("", None, value_body);
        assert_eq!(v.drive as usize, drive_value as usize);
    }
}
