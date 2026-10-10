//! Export runtime used by [`bandori_ruleset!`](crate::bandori_ruleset).

use crate::abi::{export, pack, ManifestEntry, ManifestOn};
#[cfg(target_arch = "wasm32")]
use alloc::{boxed::Box, string::String, vec::Vec};

use crate::{CardDef, On};

/// The card manifest, `postcard`-encoded (ABI v5 -- the guest hand-writes no
/// JSON at all: messages and manifests both travel as serde bytes).
pub fn manifest(bands: &'static [&'static [CardDef]]) -> i64 {
    let entries: Vec<ManifestEntry> = bands
        .iter()
        .flat_map(|band| band.iter())
        .map(|c| ManifestEntry {
            id: String::from(c.id),
            on: c
                .on
                .iter()
                .enumerate()
                .map(|(ei, o)| ManifestOn {
                    kind: o.kind() as i32,
                    triggers: o.triggers(),
                    pre: {
                        let p = o.condition();
                        if p.is_empty() {
                            None
                        } else {
                            Some(String::from(p))
                        }
                    },
                    has_guard: o.has_guard(),
                    has_legacy: c.legacy.iter().any(|(e, _)| *e == ei as i32),
                    messages: match o {
                        On::Message(names, ..) => names.iter().map(|s| String::from(*s)).collect(),
                        _ => Vec::new(),
                    },
                })
                .collect(),
            // Sorted by key so the wire bytes are deterministic regardless of
            // declaration order.
            props: {
                let mut p: Vec<(String, i32)> = c
                    .props
                    .iter()
                    .map(|(k, v)| (String::from(*k), *v))
                    .collect();
                p.sort_by(|a, b| a.0.cmp(&b.0));
                p
            },
        })
        .collect();
    leak(postcard::to_allocvec(&entries).unwrap_or_default())
}

/// Hand a buffer to the host as packed `(ptr << 32) | len`. The bytes live as
/// long as the run (bump allocator; the module is re-instantiated per run).
///
/// On a native build the "pointer" is an arena handle (see [`crate::native`]):
/// `b.as_ptr() as u32` would truncate a 64-bit address and the host's
/// `native::read` would look up a garbage slot. The arena is cleared at the
/// start of the next [`on`], which is long enough for the host to read the
/// buffer it just asked for.
fn leak(bytes: Vec<u8>) -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        let b: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        pack(b.as_ptr() as u32, b.len() as u32)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (h, l) = crate::native::intern(&bytes);
        pack(h as u32, l as u32)
    }
}

/// A card handle is an index into the *flattened* manifest: every band's
/// table in the order they were passed to `bandori_ruleset!`.
fn card(bands: &'static [&'static [CardDef]], idx: i32) -> &'static CardDef {
    let mut i = idx;
    for band in bands {
        if i >= 0 && (i as usize) < band.len() {
            return &band[i as usize];
        }
        i -= band.len() as i32;
    }
    panic!("bad card handle {idx}")
}

/// `bandori_on` -- call one entry point. See [`export::ON`] for the encoding.
pub fn on(
    bands: &'static [&'static [CardDef]],
    idx: i32,
    entry: i32,
    op: i32,
    player_id: i32,
) -> i64 {
    // Call boundary: every guest buffer this entry hands the host is interned
    // in the native arena and nothing outlives the call, so drop the previous
    // call's first. At the *start* rather than the end so a trapping call still
    // resets it.
    #[cfg(not(target_arch = "wasm32"))]
    crate::native::clear();
    let Some(o) = card(bands, idx).on.get(entry.max(0) as usize) else {
        panic!("bad entry {entry} on card {idx}")
    };
    match (*o, op) {
        (On::Counteract(_, _, guard, _), export::OP_GUARD)
        | (On::Hook(_, _, guard, _), export::OP_GUARD)
        | (On::Gate(_, _, guard, _), export::OP_GUARD)
        | (On::RollPlan(_, guard, _), export::OP_GUARD)
        | (On::AtEnd(_, guard, _), export::OP_GUARD)
        | (On::Settle(_, guard, _), export::OP_GUARD)
        | (On::Message(_, _, guard, _), export::OP_GUARD) => match guard {
            // G4-deleted residual: the condition alone decides, and the host
            // normally skips this call (`has_guard == false`). If it does ask,
            // the residual admits.
            None => 1,
            Some(g) => g(player_id) as i64,
        },
        (_, export::OP_LEGACY_GUARD) => {
            // G3 migration audit: the pre-migration guard kept on the card.
            let card = card(bands, idx);
            match card
                .legacy
                .iter()
                .find(|(e, _)| *e == entry.max(0))
                .map(|(_, f)| *f)
            {
                Some(legacy) => legacy(player_id) as i64,
                // No legacy copy: trap-free "no legacy" so the host skips the
                // check for this entry (the same contract as `has_legacy=false`).
                None => -1,
            }
        }
        (On::Play(_, why, _), export::OP_GUARD) => match why {
            Some(why) => match why(player_id) {
                None => 0,
                Some(reason) => leak(postcard::to_allocvec(&reason).unwrap_or_default()),
            },
            None => 0,
        },
        (On::Counteract(_, _, _, run), _)
        | (On::Play(_, _, run), _)
        | (On::Hook(_, _, _, run), _)
        | (On::Gate(_, _, _, run), _)
        | (On::AtEnd(_, _, run), _)
        | (On::Settle(_, _, run), _)
        | (On::RollPlan(_, _, run), _)
        | (On::Message(_, _, _, run), _) => match run(player_id) {
            Ok(()) => 0,
            // Asked: the host reads the published question and re-runs us with
            // the answer. The old `EXIT_NEED_INPUT` trap, as a return value.
            Err(crate::Prompt) => crate::abi::EXIT_NEED_INPUT as i64,
        },
    }
}

/// Generate the guest exports for a card table.
///
/// ```ignore
/// pub static CARDS: &[CardDef] = &[cards::yolo::CARD, cards::hagumi_marks::CARD];
/// card_sdk::bandori_ruleset!(CARDS);
/// ```
///
/// On `wasm32` this emits the `bandori_*` exports the sandbox host calls. On a
/// native build (the `rules-native` bot path) it instead publishes a plain
/// `RULESET` table so every rule crate can link into one binary without
/// `#[no_mangle]` clashes; `rules-native` calls [`crate::rt::on`] over the
/// concatenated tables.
#[macro_export]
macro_rules! bandori_ruleset {
    ($($cards:expr),+ $(,)?) => {
        /// Every band's table, in the order given. `static` so the export
        /// functions can hand out `'static` references into it.
        #[allow(dead_code)]
        static BAND_TABLE: &[&[$crate::CardDef]] = &[$($cards),+];

        /// Native (non-wasm) mode: the table `rules-native` aggregates.
        #[cfg(not(target_arch = "wasm32"))]
        pub static RULESET: &[&[$crate::CardDef]] = &[$($cards),+];

        /// Native call entry -- same body as the wasm export, no `#[no_mangle]`
        /// so every rule crate can link into one binary.
        #[cfg(not(target_arch = "wasm32"))]
        pub fn bandori_on(card: i32, entry: i32, op: i32, player_id: i32) -> i64 {
            $crate::rt::on(BAND_TABLE, card, entry, op, player_id)
        }

        /// Native manifest -- same body as the wasm export.
        #[cfg(not(target_arch = "wasm32"))]
        pub fn bandori_manifest() -> i64 {
            $crate::rt::manifest(BAND_TABLE)
        }

        #[cfg(target_arch = "wasm32")]
        #[no_mangle]
        pub extern "C" fn bandori_abi_version() -> i32 {
            $crate::abi::ABI_VERSION
        }
        #[cfg(target_arch = "wasm32")]
        #[no_mangle]
        pub extern "C" fn bandori_manifest() -> i64 {
            $crate::rt::manifest(BAND_TABLE)
        }
        #[cfg(target_arch = "wasm32")]
        #[no_mangle]
        pub extern "C" fn bandori_on(card: i32, entry: i32, op: i32, player_id: i32) -> i64 {
            $crate::rt::on(BAND_TABLE, card, entry, op, player_id)
        }
    };
}

/// Exports for a module holding exactly one card (the normal layout).
///
/// ```ignore
/// pub const CARD: CardDef = CardDef::new("AG:Y.O.L.O", &[On::Counteract(&[ChainKind::MoveRoll], "", Some(can_counteract), counteract)]);
/// card_sdk::bandori_card!(CARD);
/// ```
#[macro_export]
macro_rules! bandori_card {
    ($card:expr) => {
        $crate::bandori_ruleset!(&[$card]);
    };
}

/// Guest panic handler: trap. The host turns the trap into "this card is out",
/// the same contract as a malformed message -- a card never gets to show a raw
/// panic to players.
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
#[panic_handler]
fn on_panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

// --------------------------------------------------------------- allocator

/// Bump allocator for guest builds. A card run allocates a handful of small
/// strings and never frees them -- the module is re-instantiated for every run
/// -- so a pointer that only moves up is enough, and far smaller than dlmalloc.
///
/// The heap is not a fixed static arena: the cursor starts at `__heap_base`
/// (just past data, bss and the stack) and linear memory is grown on demand
/// with `memory.grow`, so an instance boots with the module's small initial
/// memory and the ceiling is soft. The one concession to reuse is
/// `realloc`: when the block being resized is the most recent allocation it is
/// grown (or shrunk) in place, which is what `Vec`/`String` resizing wants.
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
struct Bump;

#[cfg(all(target_arch = "wasm32", feature = "guest"))]
#[global_allocator]
static ALLOC: Bump = Bump;

/// Hard ceiling on the absolute addresses the bump allocator will hand out
/// (i.e. the whole linear memory, stack and data included -- `__heap_base` is
/// ~1 MiB into it). Matches the host's per-instance `memory_size` limit in
/// `game-rules` (`be_*::MAX_MEMORY_BYTES`); either side refusing is enough to
/// stop a runaway effect, and the guest hitting its own ceiling first keeps
/// the failure deterministic. Growth past this returns null and the run
/// aborts into the panic handler, which traps and takes the card out cleanly.
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
const MAX_MEMORY: usize = 16 * 1024 * 1024;

/// One wasm page of linear memory.
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
const PAGE: usize = 64 * 1024;

/// Bump cursor: the absolute address of the next free byte. `0` means "not
/// started yet" -- the first allocation latches [`__heap_base`]. Nothing in
/// the guest reads this for control flow, so it stays deterministic.
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
static mut CURSOR: usize = 0;

#[cfg(all(target_arch = "wasm32", feature = "guest"))]
extern "C" {
    /// Linker-provided (rust-lld): the first address past data, bss and the
    /// wasm shadow stack. The heap starts here; below it is not ours.
    static __heap_base: u8;
}

/// Make sure linear memory covers the absolute address `end`. Grows by whole
/// pages. Returns `false` if the host refused (its own per-instance limit) --
/// `memory.grow` reports failure with `-1` instead of trapping, so we turn it
/// into the allocator's null contract here.
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
fn ensure_mem(end: usize) -> bool {
    let have = core::arch::wasm32::memory_size::<0>() * PAGE;
    if end <= have {
        return true;
    }
    let pages = (end - have).div_ceil(PAGE);
    // Both ceilings are exact multiples of a page, so a grow that covers
    // `end` can never overshoot them by a partial page.
    if have / PAGE + pages > MAX_MEMORY / PAGE {
        return false;
    }
    core::arch::wasm32::memory_grow::<0>(pages) != usize::MAX
}

#[cfg(all(target_arch = "wasm32", feature = "guest"))]
unsafe impl core::alloc::GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let align = layout.align().max(1);
        if CURSOR == 0 {
            CURSOR = &__heap_base as *const u8 as usize;
        }
        let start = (CURSOR + align - 1) & !(align - 1);
        let end = match start.checked_add(layout.size()) {
            Some(e) if e <= MAX_MEMORY && ensure_mem(e) => e,
            // Out of memory: return null so the allocator aborts into our
            // panic handler, which traps and takes the card out cleanly.
            _ => return core::ptr::null_mut(),
        };
        CURSOR = end;
        start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // never freed -- see the note above
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: core::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        let old_size = layout.size();
        let addr = ptr as usize;
        // In place when the block is the most recent allocation: it ends
        // exactly at the cursor, so nothing has been handed out past it. The
        // base -- and therefore the alignment -- is unchanged, and the tail of
        // a shrink is simply handed back to the cursor.
        if addr.wrapping_add(old_size) == CURSOR {
            let end = match addr.checked_add(new_size) {
                Some(e) if e <= MAX_MEMORY => e,
                _ => return core::ptr::null_mut(),
            };
            if new_size > old_size && !ensure_mem(end) {
                return core::ptr::null_mut();
            }
            CURSOR = end;
            return ptr;
        }
        // Otherwise the default strategy: a fresh block and a copy. (Still
        // leaky, like `dealloc` -- the module is re-instantiated per run.)
        let new_layout = match core::alloc::Layout::from_size_align(new_size, layout.align().max(1)) {
            Ok(l) => l,
            Err(_) => return core::ptr::null_mut(),
        };
        let new_ptr = self.alloc(new_layout);
        if new_ptr.is_null() {
            return core::ptr::null_mut();
        }
        core::ptr::copy_nonoverlapping(ptr, new_ptr, old_size.min(new_size));
        new_ptr
    }
}
