//! Generic interned-string table (no game vocabulary).
//!
//! Under `cfg(kani)` every `&str` that crosses the guest/host boundary is
//! interned to a `u16` so the symbolic host never runs `memcmp` / `String::eq`
//! unwinding. Production wasm / native paths are unchanged: they keep the
//! real `ptr, len` ABI in [`crate::ctx::sys`].
//!
//! ## Loop bounds (CBMC unwinding assertions)
//!
//! Unwinding assertions require a **static** trip count: `while i < runtime_n`
//! still needs `unwind >= max(runtime_n)` even with early exit. Every loop
//! here is therefore a fixed 8-iteration chunk walk (8×8 = 64 bytes) or a
//! fixed 8-probe open-addressed lookup. A harness with `#[kani::unwind(9)]`
//! covers the whole table. Exceeding the probe limit is a **loud** `INVALID`
//! (the caller's `intern_or_fail` panics under Kani) — never a silent cut.
//!
//! Game-specific meaning of a name (which id is 「P✽P粉丝」) lives in
//! `rules-kani`, not here.

#![allow(dead_code)]

/// Maximum interned names in one verification run. The open-addressed table
/// has `SLOTS = 2 * MAX_INTERN` buckets and answers a lookup in ≤
/// [`MAX_PROBES`] integer compares.
pub const MAX_INTERN: usize = 32;

/// Table buckets (power of two). Load factor ≤ 1/2 at `MAX_INTERN`.
pub const SLOTS: usize = 64;

/// Linear probes per lookup / insert. 8 is enough for a half-full 64-slot
/// table under FNV-1a (and is the unwind bound the proofs use).
pub const MAX_PROBES: usize = 8;

/// Longest name we store. Longer names are rejected (`INVALID`) — a loud
/// cut, never a silent truncate. Guest/host traffic is card / token / tile /
/// key ids; the longest card id is 52 bytes (`PPP:（沙绫）…`) and token keys
/// are ≤ 16. The byte walk is chunked 8×8 so the unwind bound is 8, not 56.
pub const MAX_NAME: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NameId(pub u16);

impl NameId {
    pub const INVALID: NameId = NameId(u16::MAX);
    pub fn is_valid(self) -> bool {
        self.0 != u16::MAX
    }
}

/// One open-addressed bucket. `hash == 0` marks empty (FNV of a non-empty
/// name is never 0; empty string hashes to the offset basis, not 0).
struct Slot {
    hash: u64,
    len: u8,
    bytes: [u8; MAX_NAME],
}

impl Slot {
    const EMPTY: Slot = Slot {
        hash: 0,
        len: 0,
        bytes: [0; MAX_NAME],
    };
}

struct Table {
    n: u16,
    slots: [Slot; SLOTS],
}

impl Table {
    const fn new() -> Self {
        Self {
            n: 0,
            slots: [Slot::EMPTY; SLOTS],
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

/// FNV-1a 64 (same as `rules_cond_ir::ctx::id_of`).
/// Fixed 8×8 chunk walk so the unwind bound is 8, not `MAX_NAME`.
fn fnv1a(s: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut outer = 0usize;
    while outer < 8 {
        let base = outer * 8;
        let mut k = 0usize;
        while k < 8 {
            let i = base + k;
            if i < s.len() {
                h ^= s[i] as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
            k += 1;
        }
        outer += 1;
    }
    h
}

/// Fixed-bound byte compare. Lengths are already checked equal and
/// `<= MAX_NAME`, so `==` (memcmp) is one CBMC unwind — the 8×8 chunk walk
/// was still 500+ unwinds per harness from the intern lookup path.
fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() || a.len() > MAX_NAME {
        return false;
    }
    a == b
}

/// Bucket start index for `hash`.
fn bucket(hash: u64) -> usize {
    (hash as usize) & (SLOTS - 1)
}

/// Intern raw bytes (no UTF-8 validation — that is a CBMC unwinding cliff).
///
/// Returns [`NameId::INVALID`] when the table is full, the name is longer
/// than [`MAX_NAME`], or [`MAX_PROBES`] is exhausted (loud — callers that
/// cannot tolerate `INVALID` go through `intern_or_fail`).
pub fn intern_bytes(s: &[u8]) -> NameId {
    if s.len() > MAX_NAME {
        return NameId::INVALID;
    }
    let n = s.len() as u8;
    let h = fnv1a(s);
    // FNV of any byte string is never 0 (offset basis is nonzero and the
    // multiplier is odd). Still guard: a zero hash would alias EMPTY.
    let h = if h == 0 { 1 } else { h };
    let t = table();
    let start = bucket(h);
    let mut probe = 0usize;
    let mut first_empty = usize::MAX;
    while probe < MAX_PROBES {
        let idx = (start + probe) & (SLOTS - 1);
        let slot = &t.slots[idx];
        if slot.hash == 0 {
            if first_empty == usize::MAX {
                first_empty = idx;
            }
        } else if slot.hash == h && slot.len == n && bytes_eq(&slot.bytes[..n as usize], s) {
            return NameId(idx as u16);
        }
        probe += 1;
    }
    if first_empty == usize::MAX || t.n as usize >= MAX_INTERN {
        return NameId::INVALID;
    }
    let slot = &mut t.slots[first_empty];
    slot.hash = h;
    slot.len = n;
    // memcpy — one CBMC unwind, not an 8×8 chunk walk (the insert path was
    // still the slowest loop after the lookup was hashed).
    let nbytes = s.len().min(MAX_NAME);
    slot.bytes[..nbytes].copy_from_slice(&s[..nbytes]);
    t.n += 1;
    NameId(first_empty as u16)
}

/// Intern a `&str` (uses its bytes; does not re-validate UTF-8).
pub fn intern(s: &str) -> NameId {
    intern_bytes(s.as_bytes())
}

/// The bytes previously interned as `id`. Empty when `id` is stale.
/// (The slot index **is** the id under the open-addressed table.)
pub fn resolve(id: NameId) -> &'static [u8] {
    if !id.is_valid() {
        return &[];
    }
    let t = table();
    let idx = id.0 as usize;
    if idx >= SLOTS || t.slots[idx].hash == 0 {
        return &[];
    }
    // SAFETY: table entries are never removed; we hand out a slice of the
    // static. Cast through raw pointer to extend the lifetime to `'static`
    // (the table is never resized or cleared during a proof).
    let p = t.slots[idx].bytes.as_ptr();
    let n = t.slots[idx].len as usize;
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
    let mut i = 0usize;
    while i < SLOTS {
        t.slots[i] = Slot::EMPTY;
        i += 1;
    }
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

    #[test]
    fn intern_fixed_loop_bounds() {
        reset();
        // Longest card id (52 bytes) must intern; 65-byte name is rejected.
        let long = "x".repeat(52);
        assert!(intern(&long).is_valid());
        let too_long = "y".repeat(MAX_NAME + 1);
        assert!(!intern(&too_long).is_valid());
        // Distinct short names fill the table; overflow is INVALID (loud).
        let mut ids = [NameId::INVALID; MAX_INTERN];
        for i in 0..MAX_INTERN {
            ids[i] = intern(&format!("k{i}"));
            assert!(ids[i].is_valid(), "slot {i}");
        }
        assert!(!intern("overflow").is_valid());
        for i in 0..MAX_INTERN {
            assert_eq!(resolve_str(ids[i]), format!("k{i}"));
        }
    }
}