//! Ruleset-declared trigger-kind flags, read at load.
//!
//! Engine kinds carry their flags in `game_kinds`. Ruleset kinds are opaque
//! tokens to the engine -- their hook-only bit is declared in the ruleset
//! (`bandori-std::kind::kind_hook_only`) and reaches the host through the
//! `ruleset_kind_flags` export (or [`set_kind_flags`] on the native / test
//! path). The engine never names a ruleset id.
//!
//! One ruleset is live at a time (same shape as [`crate::tile_class`]), so the
//! table is keyed by the opaque id inside that ruleset's namespace.

use std::sync::RwLock;

use card_sdk::abi::KindNs;

use crate::TriggerKind;

static RULESET_HOOK_ONLY: RwLock<Vec<(i32, bool)>> = RwLock::new(Vec::new());

/// Seed the ruleset kind-flag table from the ruleset (at load). `flags` is
/// `(ruleset kind id, hook_only)` as the ruleset's `kind` module declared it.
/// The engine never names the ids.
pub fn set_kind_flags(flags: Vec<(i32, bool)>) {
    *RULESET_HOOK_ONLY.write().unwrap() = flags;
}

/// Does this kind skip the hand-counteraction window?
///
/// Engine kinds read `game_kinds`. Ruleset kinds read the load-time table; an
/// unregistered ruleset kind opens a window (`false`), matching the old
/// default for kinds the host had never named.
pub fn hook_only(kind: TriggerKind) -> bool {
    match kind.ns {
        KindNs::Engine => game_kinds::hook_only(kind.id),
        KindNs::Ruleset(_) => RULESET_HOOK_ONLY
            .read()
            .unwrap()
            .iter()
            .find(|(id, _)| *id == kind.id)
            .map(|(_, hook_only)| *hook_only)
            .unwrap_or(false),
    }
}
