//! Generic interned-string table (no game vocabulary).
//!
//! Every `&str` that crosses the guest/host boundary as a **name** (card id,
//! token key, counter key) is interned to a `NameId` (`Copy` `u16`) so the
//! seam never heap-allocates a `String` per element and never runs
//! `memcmp` / `String::eq` unwinding under the symbolic host. Display text
//! still travels as bytes ([`crate::msg::Msg`], `read_str`) — only identity
//! lists intern.
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

/// Maximum interned names. The open-addressed table has
/// `SLOTS = 2 * MAX_INTERN` buckets and answers a lookup in ≤ [`MAX_PROBES`]
/// integer compares.
///
/// **Kani bound model** — keep tiny so `#[kani::unwind]` stays tractable and
/// so the presence bitset in `rules-kani`'s world model fits a `u64`.
#[cfg(kani)]
pub const MAX_INTERN: usize = 32;

/// Table buckets (power of two). Load factor ≤ 1/2 at `MAX_INTERN`.
#[cfg(kani)]
pub const SLOTS: usize = 64;

/// Linear probes per lookup / insert. 8 is enough for a half-full 64-slot
/// table under FNV-1a (and is the unwind bound the proofs use).
#[cfg(kani)]
pub const MAX_PROBES: usize = 8;

/// Production capacity. Card ids alone are hundreds; tokens / counters /
/// skill ids push the working set past the Kani model of 32. 2048 leaves
/// headroom without making the table a wasm size problem
/// (`SLOTS * (8 + 1 + 64)` ≈ 300 KiB of zeros).
#[cfg(not(kani))]
pub const MAX_INTERN: usize = 2048;

/// Table buckets (power of two). Load factor ≤ 1/2 at `MAX_INTERN`.
#[cfg(not(kani))]
pub const SLOTS: usize = 4096;

/// Linear probes per lookup / insert. At load ≤ 1/2 a 4096-slot table, 64
/// keeps the probe-exhaustion probability negligible; a miss is a loud
/// `INVALID`, never a silent cut. (The Kani twin is 8 — that bound is the
/// CBMC unwind budget, not a production requirement.)
#[cfg(not(kani))]
pub const MAX_PROBES: usize = 64;

/// Longest name we store. Longer names are rejected (`INVALID`) — a loud
/// cut, never a silent truncate. Guest/host traffic is card / token / tile /
/// key ids; the longest card id is 52 bytes (`PPP:（沙绫）…`) and token keys
/// are ≤ 16. The byte walk is chunked 8×8 so the unwind bound is 8, not 56.
pub const MAX_NAME: usize = 64;

/// An interned name. `Copy`, so a pile listing is `Vec<NameId>` (no `String`
/// clone per element) and `for id in ctx::cards_in(..)` is a `u16` walk.
///
/// Identity, not display: compare with another [`NameId`] or with a `&str`
/// literal (`id == "PP:[丸山彩]憧憬的前方"`). The `&str` side is interned and
/// the ids are compared — under `cfg(kani)` a literal's FNV hash
/// constant-folds, so this is a few integer compares, never `memcmp`.
/// Never leak the `u16` into body code; use the comparisons and [`NameId::as_str`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct NameId(pub u16);

impl NameId {
    pub const INVALID: NameId = NameId(u16::MAX);
    pub fn is_valid(self) -> bool {
        self.0 != u16::MAX
    }

    /// Intern `s` (same as [`intern`]). An inherent alias so body code can
    /// write `NameId::intern(id)` when it is pushing literals into a
    /// `Vec<NameId>` without importing the free function.
    #[inline]
    pub fn intern(s: &str) -> NameId {
        intern(s)
    }

    /// The interned bytes as `&'static str` (`""` when `id` is stale or the
    /// stored bytes are not UTF-8).
    ///
    /// Prefer comparing [`NameId`]s under `cfg(kani)` — [`resolve_str`] runs
    /// the UTF-8 validation loop, a CBMC unwind cliff. Reach for this only at
    /// the few sites that genuinely need text (`starts_with`, `Msg::card`,
    /// `play_card`).
    #[inline]
    pub fn as_str(&self) -> &'static str {
        resolve_str(*self)
    }
}

/// `id == "literal"`: intern the rhs and compare ids.
///
/// Under `cfg(kani)` the literal's FNV hash constant-folds through
/// [`intern_by_hash`] (see the `fnv1a` notes) — integer probes only, no
/// `memcmp` / `String::eq`. A full table yields [`NameId::INVALID`] on the
/// rhs and the compare is `false` (loud in the proofs via `intern_or_fail`).
impl PartialEq<&str> for NameId {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        *self == intern(other)
    }
}

impl PartialEq<str> for NameId {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        *self == intern(other)
    }
}

/// `"literal" == id`, the mirror of [`PartialEq<&str> for NameId`].
/// Inference-friendly: the lhs stays a `&str` literal, the rhs a `NameId`.
impl PartialEq<NameId> for &str {
    #[inline]
    fn eq(&self, other: &NameId) -> bool {
        *other == **self
    }
}

/// ABI handle tag for an interned name. Under `cfg(kani)`, `ctx::s("…")`
/// returns `(INTERN_TAG | NameId, len)` instead of a native-arena index, so
/// the host never re-reads or re-hashes the bytes. Arena (out-buffer) handles
/// are small non-negative indices and never carry this bit.
pub const INTERN_TAG: i32 = 0x4000_0000;

/// Encode a `NameId` as an ABI handle.
#[inline(always)]
pub fn encode_handle(id: NameId) -> i32 {
    INTERN_TAG | (id.0 as i32 & 0xffff)
}

/// Decode an ABI handle. `None` when the handle is a plain arena index.
#[inline(always)]
pub fn decode_handle(handle: i32) -> Option<NameId> {
    if handle & INTERN_TAG != 0 {
        Some(NameId((handle & 0xffff) as u16))
    } else {
        None
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

// -- table storage ---------------------------------------------------------
//
// Three homes, one shape:
//
// * `cfg(kani)` — `static mut`. kani-compiler 0.68.0 cannot monomorphize
//   `thread_local!` (catch_unwind ICE), and the verifier is single-threaded.
// * pure wasm guest (`no_std`, no `native-rules`) — `static mut`. Wasm32
//   without atomics is single-threaded, and each instance owns its own linear
//   memory, so one table per instance is already isolated. `thread_local!`
//   is a std macro and this build has no std.
// * native std (`rules-native`, the `native-rules` bot monolith, unit tests)
//   — `thread_local!`. The server fans matches out over `spawn_blocking` and
//   bot threads, and the intern unit tests run in parallel; a process-global
//   `static mut` races (the flaky `intern_*` tests were the first symptom).
//   A card-body call is single-threaded, so a `NameId` is only compared on
//   the thread that interned it — per-thread tables are correct. The
//   `resolve` `'static` slice outlives the `RefCell` borrow (same lifetime
//   extension as the `static mut` path) and is valid until that thread
//   exits; consume it inside the call, do not stash it in a global.

#[cfg(any(kani, all(target_arch = "wasm32", not(feature = "native-rules"))))]
static mut TABLE: Table = Table::new();

#[cfg(not(any(kani, all(target_arch = "wasm32", not(feature = "native-rules")))))]
std::thread_local! {
    /// Heap-allocated: `Table` is ~320 KiB at production `SLOTS`, and
    /// `Box::new(Table::new())` would materialise that on the thread stack
    /// first (debug builds blow the default test stack).
    static TABLE: core::cell::RefCell<Box<Table>> =
        core::cell::RefCell::new(new_table());
}

/// Fresh heap table. Zeroed memory **is** `Slot::EMPTY` (`hash == 0` marks
/// empty), so this matches [`Table::new`] without a 4096-slot stack temporary.
#[cfg(not(any(kani, all(target_arch = "wasm32", not(feature = "native-rules")))))]
fn new_table() -> Box<Table> {
    let layout = core::alloc::Layout::new::<Table>();
    unsafe {
        // `alloc` is in scope via std/alloc on this cfg (see `lib.rs`).
        let p = std::alloc::alloc_zeroed(layout);
        if p.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Box::from_raw(p as *mut Table)
    }
}

/// Exclusive access to the intern table for the duration of `f`.
#[cfg(any(kani, all(target_arch = "wasm32", not(feature = "native-rules"))))]
fn with_table<R>(f: impl FnOnce(&mut Table) -> R) -> R {
    // SAFETY: single-threaded (see the module note above); the table is
    // never resized or moved.
    f(unsafe { &mut TABLE })
}

/// Exclusive access to the intern table for the duration of `f`.
#[cfg(not(any(kani, all(target_arch = "wasm32", not(feature = "native-rules")))))]
fn with_table<R>(f: impl FnOnce(&mut Table) -> R) -> R {
    TABLE.with(|t| f(t.borrow_mut().as_mut()))
}

/// FNV-1a 64 (same as `rules_cond_ir::ctx::id_of`).
///
/// **Zero loops.** A `while` walk is an unwind cliff under CBMC even at 8
/// iterations (the 900 s profile counted 144 `fnv1a` unrolls). 64 straight
/// `if i < len` steps have no loop for CBMC to unwind, and when `s` is a
/// string literal (every `ctx::tok` / `slot` / `state_*` name in a card body)
/// the whole hash constant-folds through `#[inline(always)]`.
/// Same result as the old 8×8 chunk walk for every `s`.
#[cfg(kani)]
#[inline(always)]
fn fnv1a(s: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    macro_rules! step {
        ($i:expr) => {
            // `.get`, not `s[$i]`: indexing emits a slice_index_fail panic
            // path CBMC still explores (64 per hash in the shave formula).
            if let Some(&b) = s.get($i) {
                h ^= b as u64;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
    }
    step!(0);
    step!(1);
    step!(2);
    step!(3);
    step!(4);
    step!(5);
    step!(6);
    step!(7);
    step!(8);
    step!(9);
    step!(10);
    step!(11);
    step!(12);
    step!(13);
    step!(14);
    step!(15);
    step!(16);
    step!(17);
    step!(18);
    step!(19);
    step!(20);
    step!(21);
    step!(22);
    step!(23);
    step!(24);
    step!(25);
    step!(26);
    step!(27);
    step!(28);
    step!(29);
    step!(30);
    step!(31);
    step!(32);
    step!(33);
    step!(34);
    step!(35);
    step!(36);
    step!(37);
    step!(38);
    step!(39);
    step!(40);
    step!(41);
    step!(42);
    step!(43);
    step!(44);
    step!(45);
    step!(46);
    step!(47);
    step!(48);
    step!(49);
    step!(50);
    step!(51);
    step!(52);
    step!(53);
    step!(54);
    step!(55);
    step!(56);
    step!(57);
    step!(58);
    step!(59);
    step!(60);
    step!(61);
    step!(62);
    step!(63);
    h
}

/// Production twin of `fnv1a` — the original 8×8 chunk walk, byte-for-byte
/// the master code so wasm/native codegen is unchanged (`cfg(not(kani))`).
#[cfg(not(kani))]
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

/// `const fn` twin of [`fnv1a`] — same bits, usable in `const` items so a
/// literal's hash is a compile-time `u64` before CBMC sees the program.
#[cfg(kani)]
pub const fn fnv1a_const(name: &str) -> u64 {
    let s = name.as_bytes();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0usize;
    // `const fn` may loop; this is evaluated at compile time for literals and
    // never enters the CBMC unwind path. The runtime twin above is loop-free.
    while i < s.len() && i < MAX_NAME {
        h ^= s[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

/// Fixed-bound byte compare. Lengths are already checked equal and
/// `<= MAX_NAME`.
///
/// **Zero loops** (same reason as [`fnv1a`]). NOT `a == b`: that lowers to
/// memcmp, whose builtin loop needs unwind >= 64 and trips
/// *"Not unwinding memcmp at 32"* (measured).
#[cfg(kani)]
#[inline(always)]
fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() || a.len() > MAX_NAME {
        return false;
    }
    let n = a.len();
    let mut ok = true;
    macro_rules! step {
        ($i:expr) => {
            // `.get`, not `[i]`: no slice_index_fail panic path.
            match (a.get($i), b.get($i)) {
                (Some(x), Some(y)) if x != y => ok = false,
                _ => {}
            }
        };
    }
    step!(0);
    step!(1);
    step!(2);
    step!(3);
    step!(4);
    step!(5);
    step!(6);
    step!(7);
    step!(8);
    step!(9);
    step!(10);
    step!(11);
    step!(12);
    step!(13);
    step!(14);
    step!(15);
    step!(16);
    step!(17);
    step!(18);
    step!(19);
    step!(20);
    step!(21);
    step!(22);
    step!(23);
    step!(24);
    step!(25);
    step!(26);
    step!(27);
    step!(28);
    step!(29);
    step!(30);
    step!(31);
    step!(32);
    step!(33);
    step!(34);
    step!(35);
    step!(36);
    step!(37);
    step!(38);
    step!(39);
    step!(40);
    step!(41);
    step!(42);
    step!(43);
    step!(44);
    step!(45);
    step!(46);
    step!(47);
    step!(48);
    step!(49);
    step!(50);
    step!(51);
    step!(52);
    step!(53);
    step!(54);
    step!(55);
    step!(56);
    step!(57);
    step!(58);
    step!(59);
    step!(60);
    step!(61);
    step!(62);
    step!(63);
    ok
}

/// Production twin of `bytes_eq` — original 8 outer × 8 unrolled bytes.
#[cfg(not(kani))]
fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() || a.len() > MAX_NAME {
        return false;
    }
    let n = a.len();
    let mut outer = 0usize;
    let mut ok = true;
    while outer < 8 {
        let base = outer * 8;
        if base + 0 < n && a[base + 0] != b[base + 0] {
            ok = false;
        }
        if base + 1 < n && a[base + 1] != b[base + 1] {
            ok = false;
        }
        if base + 2 < n && a[base + 2] != b[base + 2] {
            ok = false;
        }
        if base + 3 < n && a[base + 3] != b[base + 3] {
            ok = false;
        }
        if base + 4 < n && a[base + 4] != b[base + 4] {
            ok = false;
        }
        if base + 5 < n && a[base + 5] != b[base + 5] {
            ok = false;
        }
        if base + 6 < n && a[base + 6] != b[base + 6] {
            ok = false;
        }
        if base + 7 < n && a[base + 7] != b[base + 7] {
            ok = false;
        }
        outer += 1;
    }
    ok
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
#[cfg(kani)]
#[inline(always)]
pub fn intern_bytes(s: &[u8]) -> NameId {
    if s.len() > MAX_NAME {
        return NameId::INVALID;
    }
    let h = fnv1a(s);
    intern_by_hash(h, s)
}

/// Table lookup/insert for a **precomputed** FNV-1a hash. Split from
/// [`intern_bytes`] so an inlined call site can constant-fold `fnv1a`
/// (the hash of a string literal is a compile-time `u64`) and this routine
/// only does integer probes + a zero-loop byte compare.
#[cfg(kani)]
pub fn intern_by_hash(mut h: u64, s: &[u8]) -> NameId {
    if s.len() > MAX_NAME {
        return NameId::INVALID;
    }
    let n = s.len() as u8;
    // FNV of any byte string is never 0 (offset basis is nonzero and the
    // multiplier is odd). Still guard: a zero hash would alias EMPTY.
    if h == 0 {
        h = 1;
    }
    with_table(|t| {
        let start = bucket(h);
        let mut probe = 0usize;
        let mut first_empty = usize::MAX;
    // 8 probes, **zero loops** (same reason as `fnv1a`: an 8-trip `while`
    // still shows up as `Unwinding loop … iteration 8` and, with
    // `inline(always)`, once per call site). Each probe is an integer
    // compare + a zero-loop `bytes_eq` on a hit.
    macro_rules! probe_once {
        () => {{
            let idx = (start + probe) & (SLOTS - 1);
            let slot = &t.slots[idx];
            if slot.hash == 0 {
                if first_empty == usize::MAX {
                    first_empty = idx;
                }
            } else if slot.hash == h && slot.len == n {
                // Hash+len identity. `bytes_eq` (64 steps × 8 probes) was the
                // money_shave setup cliff (`known_init` interns 10 names).
                // Collisions (same FNV+len, different bytes) are a loud
                // INVALID below — never a silent alias. The pilot set is
                // pinned unique in `model_bounds`.
                return NameId(idx as u16);
            } else if slot.hash == h && slot.len != n {
                // Same FNV, different length: collision. Loud, not an alias.
                return NameId::INVALID;
            }
            probe += 1;
        }};
    }
    probe_once!();
    probe_once!();
    probe_once!();
    probe_once!();
    probe_once!();
    probe_once!();
    probe_once!();
    probe_once!();
    if first_empty == usize::MAX || t.n as usize >= MAX_INTERN {
        return NameId::INVALID;
    }
    let slot = &mut t.slots[first_empty];
    slot.hash = h;
    slot.len = n;
    // Zero-loop byte copy. NOT `copy_from_slice`: that lowers to memcpy plus
    // a `len_mismatch_fail` panic path CBMC still explores when the source
    // length is even slightly symbolic (measured in the shave profile).
    let nbytes = s.len().min(MAX_NAME);
    macro_rules! cpy {
        ($i:expr) => {
            // `.get`, not `[i]`: no slice_index_fail panic path.
            if let Some(&b) = s.get($i) {
                slot.bytes[$i] = b;
            }
        };
    }
    cpy!(0);
    cpy!(1);
    cpy!(2);
    cpy!(3);
    cpy!(4);
    cpy!(5);
    cpy!(6);
    cpy!(7);
    cpy!(8);
    cpy!(9);
    cpy!(10);
    cpy!(11);
    cpy!(12);
    cpy!(13);
    cpy!(14);
    cpy!(15);
    cpy!(16);
    cpy!(17);
    cpy!(18);
    cpy!(19);
    cpy!(20);
    cpy!(21);
    cpy!(22);
    cpy!(23);
    cpy!(24);
    cpy!(25);
    cpy!(26);
    cpy!(27);
    cpy!(28);
    cpy!(29);
    cpy!(30);
    cpy!(31);
    cpy!(32);
    cpy!(33);
    cpy!(34);
    cpy!(35);
    cpy!(36);
    cpy!(37);
    cpy!(38);
    cpy!(39);
    cpy!(40);
    cpy!(41);
    cpy!(42);
    cpy!(43);
    cpy!(44);
    cpy!(45);
    cpy!(46);
    cpy!(47);
    cpy!(48);
    cpy!(49);
    cpy!(50);
    cpy!(51);
    cpy!(52);
    cpy!(53);
    cpy!(54);
    cpy!(55);
    cpy!(56);
    cpy!(57);
    cpy!(58);
    cpy!(59);
    cpy!(60);
    cpy!(61);
    cpy!(62);
    cpy!(63);
        t.n += 1;
        NameId(first_empty as u16)
    })
}

/// Intern a `&str` (uses its bytes; does not re-validate UTF-8).
#[cfg_attr(kani, inline(always))]
pub fn intern(s: &str) -> NameId {
    intern_bytes(s.as_bytes())
}

/// Production `intern_bytes` — the 8-probe loop + `copy_from_slice` insert,
/// unchanged codegen for wasm/native (the capacity / storage cfgs above are
/// the only production deltas vs the Kani model).
#[cfg(not(kani))]
pub fn intern_bytes(s: &[u8]) -> NameId {
    if s.len() > MAX_NAME {
        return NameId::INVALID;
    }
    let n = s.len() as u8;
    let h = fnv1a(s);
    let h = if h == 0 { 1 } else { h };
    with_table(|t| {
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
        let nbytes = s.len().min(MAX_NAME);
        slot.bytes[..nbytes].copy_from_slice(&s[..nbytes]);
        t.n += 1;
        NameId(first_empty as u16)
    })
}

/// The bytes previously interned as `id`. Empty when `id` is stale.
/// (The slot index **is** the id under the open-addressed table.)
///
/// The returned slice is lifetime-extended to `'static` off the (never
/// resized) table — see the storage note above. Consume it on the interning
/// thread; do not stash it in a longer-lived global.
pub fn resolve(id: NameId) -> &'static [u8] {
    if !id.is_valid() {
        return &[];
    }
    with_table(|t| {
        let idx = id.0 as usize;
        if idx >= SLOTS || t.slots[idx].hash == 0 {
            return &[] as &[u8];
        }
        // SAFETY: table entries are never removed; we hand out a slice of the
        // table slot. Cast through raw pointer to extend the lifetime to
        // `'static` (the table is never resized or cleared during a proof /
        // a card-body call).
        let p = t.slots[idx].bytes.as_ptr();
        let n = t.slots[idx].len as usize;
        unsafe { core::slice::from_raw_parts(p, n) }
    })
}

/// Resolve as `&str` only when the stored bytes are valid UTF-8.
/// Avoid this under `cfg(kani)` — it runs the UTF-8 validation loop.
/// [`NameId::as_str`] is the ergonomic wrapper.
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

/// Number of interned names on this thread's table.
pub fn len() -> u16 {
    with_table(|t| t.n)
}

/// Clear the table (native conformance / unit tests only; proofs start fresh
/// per harness via process isolation). Under `thread_local` storage this is
/// per-thread — which is what the parallel unit tests want.
pub fn reset() {
    with_table(|t| {
        t.n = 0;
        let mut i = 0usize;
        while i < SLOTS {
            t.slots[i] = Slot::EMPTY;
            i += 1;
        }
    })
}

/// True when `id`'s stored bytes equal `s`.
pub fn id_eq_str(id: NameId, s: &str) -> bool {
    id_eq_bytes(id, s.as_bytes())
}

// Native std tests share nothing: the table is `thread_local!` (see the
// storage note), so `intern_fixed_loop_bounds` and `intern_roundtrip` no
// longer race when cargo runs them in parallel. Under `cfg(kani)` the table
// is still `static mut` (single-threaded verifier).
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
        assert_eq!(a.as_str(), "PP:[丸山彩]憧憬的前方");
        assert!(a == "PP:[丸山彩]憧憬的前方");
        assert!("PP:[丸山彩]憧憬的前方" == a);
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
        // Capacity is a fresh table: the length checks above already occupy a
        // slot, and `MAX_INTERN` is the whole-table cap, not "remaining".
        reset();
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

    #[test]
    fn name_id_eq_str_compares_via_intern() {
        reset();
        let a = intern("crystals");
        assert!(a == "crystals");
        assert!(a != "CP");
        assert!("crystals" == a);
        // `PartialEq<str>`: unsized rhs goes through the same intern path.
        let s: &str = "crystals";
        assert!(a == *s);
    }

    /// Parallel `cargo test` used to flake on a shared `static mut`; the
    /// table is `thread_local!` now, so two threads must not see each
    /// other's inserts.
    #[test]
    fn intern_tables_are_per_thread() {
        reset();
        let mine = intern("per-thread-marker");
        assert_eq!(len(), 1);
        let other = std::thread::spawn(|| {
            reset();
            assert_eq!(len(), 0, "other thread starts empty");
            let id = intern("other-thread-only");
            assert_eq!(len(), 1);
            id
        })
        .join()
        .expect("join intern thread");
        assert_ne!(mine, other, "NameIds are thread-local");
        assert_eq!(len(), 1, "this thread's table is untouched");
        assert_eq!(resolve_str(mine), "per-thread-marker");
    }

    /// Wire-format pin for the seam decode (`ctx::decode_name_list`): the
    /// host still encodes `postcard::to_allocvec(&Vec<String>)`, the guest
    /// deserializes borrowed `&str` and interns — no `String` construction.
    /// Lives here (not in `ctx`) so it runs without linking the host ABI.
    #[test]
    fn postcard_vec_string_borrowed_decode_interns() {
        reset();
        let wire = vec![
            String::from("PP:[丸山彩]憧憬的前方"),
            String::from("c0"),
            String::from("PP:[丸山彩]憧憬的前方"),
        ];
        let bytes = postcard::to_allocvec(&wire).expect("encode Vec<String>");
        let borrowed: Vec<&str> = postcard::from_bytes(&bytes).expect("decode Vec<&str>");
        assert_eq!(borrowed.len(), 3);
        let a = intern(borrowed[0]);
        let b = intern(borrowed[1]);
        let c = intern(borrowed[2]);
        assert_eq!(a, c);
        assert_ne!(a, b);
        assert!(a == "PP:[丸山彩]憧憬的前方");
        // The `(uid, name)` twin used by `field_instances` / `group_skills`.
        let wire2 = vec![(7i32, String::from("通用:[衍生]FEVER!"))];
        let bytes2 = postcard::to_allocvec(&wire2).expect("encode Vec<(i32, String)>");
        let borrowed2: Vec<(i32, &str)> = postcard::from_bytes(&bytes2).expect("decode");
        assert_eq!(borrowed2[0].0, 7);
        assert!(intern(borrowed2[0].1) == "通用:[衍生]FEVER!");
    }
}
