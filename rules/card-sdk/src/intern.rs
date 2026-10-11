//! Generic interned-string table (no game vocabulary).
//!
//! Under `cfg(kani)` every `&str` that crosses the guest/host boundary is
//! interned to a `u16` so the symbolic host never runs `memcmp` / `String::eq`
//! unwinding. Production wasm / native paths are unchanged: they keep the
//! real `ptr, len` ABI in [`crate::ctx::sys`].
//!
//! The table is a fixed-capacity array of byte slices. Lookup is a bounded
//! byte compare (not `str::eq`) so CBMC unwinds it under `--no-unwinding-checks`
//! with a small bound.
//!
//! Game-specific meaning of a name (which id is 「P✽P粉丝」) lives in
//! `rules-kani`, not here.

#![allow(dead_code)]

/// Maximum interned names in one verification run. Bounded so every loop
/// unwinds; cards under proof only need a handful of card / token / tile names.
pub const MAX_INTERN: usize = 256;

/// Longest name we store. Longer names are truncated for identity purposes
/// only when comparing; the first 64 bytes decide identity (same bound as the
/// pilot's `guest_eq`).
pub const MAX_NAME: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameId(pub u16);

impl NameId {
    pub const INVALID: NameId = NameId(u16::MAX);
    pub fn is_valid(self) -> bool {
        self.0 != u16::MAX
    }
}

struct Table {
    n: u16,
    /// Packed names: `n` entries of `MAX_NAME` bytes + length in `lens`.
    bytes: [[u8; MAX_NAME]; MAX_INTERN],
    lens: [u8; MAX_INTERN],
}

impl Table {
    const fn new() -> Self {
        Self {
            n: 0,
            bytes: [[0; MAX_NAME]; MAX_INTERN],
            lens: [0; MAX_INTERN],
        }
    }
}

// Single-threaded verification (and single-threaded native conformance).
// `static mut` avoids `thread_local!`, which kani-compiler 0.68.0 cannot
// monomorphize (catch_unwind ICE).
static mut TABLE: Table = Table::new();

fn table() -> &'static mut Table {
    unsafe { &mut TABLE }
}

/// Fixed-bound byte compare. `==` on slices lowers to `memcmp`.
fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() || a.len() > MAX_NAME {
        return false;
    }
    let mut i = 0usize;
    let mut ok = true;
    while i < MAX_NAME {
        if i < a.len() && a[i] != b[i] {
            ok = false;
        }
        i += 1;
    }
    ok
}

/// Intern raw bytes (no UTF-8 validation — that is a CBMC unwinding cliff).
///
/// Returns [`NameId::INVALID`] when the table is full or the name is longer
/// than [`MAX_NAME`].
pub fn intern_bytes(s: &[u8]) -> NameId {
    if s.len() > MAX_NAME {
        return NameId::INVALID;
    }
    let n = s.len();
    let t = table();
    let mut i = 0u16;
    while i < t.n {
        let idx = i as usize;
        if t.lens[idx] as usize == n && bytes_eq(&t.bytes[idx][..n], s) {
            return NameId(i);
        }
        i += 1;
    }
    if t.n as usize >= MAX_INTERN {
        return NameId::INVALID;
    }
    let idx = t.n as usize;
    let mut j = 0usize;
    while j < n {
        t.bytes[idx][j] = s[j];
        j += 1;
    }
    t.lens[idx] = n as u8;
    t.n += 1;
    NameId(t.n - 1)
}

/// Intern a `&str` (uses its bytes; does not re-validate UTF-8).
pub fn intern(s: &str) -> NameId {
    intern_bytes(s.as_bytes())
}

/// The bytes previously interned as `id`. Empty when `id` is stale.
pub fn resolve(id: NameId) -> &'static [u8] {
    if !id.is_valid() {
        return &[];
    }
    let t = table();
    let idx = id.0 as usize;
    if idx >= t.n as usize {
        return &[];
    }
    // SAFETY: table entries are never removed; we hand out a slice of the
    // static. Cast through raw pointer to extend the lifetime to `'static`
    // (the table is never resized or cleared during a proof).
    let p = t.bytes[idx].as_ptr();
    let n = t.lens[idx] as usize;
    unsafe { core::slice::from_raw_parts(p, n) }
}

/// Resolve as `&str` only when the stored bytes are valid UTF-8.
/// Avoid this under `cfg(kani)` — it runs the UTF-8 validation loop.
pub fn resolve_str(id: NameId) -> &'static str {
    core::str::from_utf8(resolve(id)).unwrap_or("")
}

/// True when `id`'s stored bytes equal `s` (byte compare, no UTF-8).
pub fn id_eq_bytes(id: NameId, s: &[u8]) -> bool {
    if !id.is_valid() {
        return false;
    }
    bytes_eq(resolve(id), s)
}

/// Number of interned names.
pub fn len() -> u16 {
    table().n
}

/// Clear the table (native conformance / unit tests only; proofs start fresh
/// per harness via process isolation).
pub fn reset() {
    let t = table();
    t.n = 0;
}

/// True when `id`'s stored bytes equal `s`.
pub fn id_eq_str(id: NameId, s: &str) -> bool {
    id_eq_bytes(id, s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_roundtrip() {
        reset();
        let a = intern("PP:[丸山彩]憧憬的前方");
        let b = intern("PP:[丸山彩]憧憬的前方");
        assert_eq!(a, b);
        assert_eq!(resolve_str(a), "PP:[丸山彩]憧憬的前方");
        let c = intern("P✽P粉丝(正)");
        assert_ne!(a, c);
        assert_eq!(resolve_str(c), "P✽P粉丝(正)");
    }
}