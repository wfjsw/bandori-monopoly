//! Interned-name helpers for the symbolic host.
//!
//! Re-exports [`card_sdk::intern`] so game code can use `crate::intern::…`
//! without depending on the SDK path. The table itself is generic (no game
//! vocabulary) and lives in card-sdk so the ctx boundary can intern too.

pub use card_sdk::intern::{
    id_eq_bytes, id_eq_str, intern, intern_bytes, len, reset, resolve, resolve_str, NameId,
    MAX_INTERN, MAX_NAME,
};

/// Intern a name, panicking under Kani when the table is exhausted (an
/// unmodeled name is a loud failure, never a silent zero).
pub fn intern_or_fail(s: &str) -> NameId {
    let id = intern(s);
    #[cfg(kani)]
    kani::assert(id.is_valid(), "intern table exhausted or name too long");
    id
}