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
                .map(|o| ManifestOn {
                    kind: o.kind() as i32,
                    triggers: o.triggers(),
                })
                .collect(),
        })
        .collect();
    leak(postcard::to_allocvec(&entries).unwrap_or_default())
}

/// Hand a buffer to the host as packed `(ptr << 32) | len`. The bytes live as
/// long as the run (bump allocator; the module is re-instantiated per run).
fn leak(bytes: Vec<u8>) -> i64 {
    let b: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    pack(b.as_ptr() as u32, b.len() as u32)
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
        (On::CounterAct(_, guard, _), export::OP_GUARD)
        | (On::Hook(_, guard, _), export::OP_GUARD) => guard(player_id) as i64,
        (On::Play(why, _), export::OP_GUARD) => match why {
            Some(why) => match why(player_id) {
                None => 0,
                Some(reason) => leak(postcard::to_allocvec(&reason).unwrap_or_default()),
            },
            None => 0,
        },
        (On::CounterAct(_, _, run), _)
        | (On::Play(_, run), _)
        | (On::Hook(_, _, run), _)
        | (On::Gate(_, run), _)
        | (On::AtEnd(run), _)
        | (On::RollPlan(run), _) => match run(player_id) {
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
#[macro_export]
macro_rules! bandori_ruleset {
    ($($cards:expr),+ $(,)?) => {
        /// Every band's table, in the order given. `static` so the export
        /// functions can hand out `'static` references into it.
        static BAND_TABLE: &[&[$crate::CardDef]] = &[$($cards),+];

        #[no_mangle]
        pub extern "C" fn bandori_abi_version() -> i32 {
            $crate::abi::ABI_VERSION
        }
        #[no_mangle]
        pub extern "C" fn bandori_manifest() -> i64 {
            $crate::rt::manifest(BAND_TABLE)
        }
        #[no_mangle]
        pub extern "C" fn bandori_on(card: i32, entry: i32, op: i32, player_id: i32) -> i64 {
            $crate::rt::on(BAND_TABLE, card, entry, op, player_id)
        }
    };
}

/// Exports for a module holding exactly one card (the normal layout).
///
/// ```ignore
/// pub const CARD: CardDef = CardDef::new("AG:Y.O.L.O", &[On::React(&[ChainKind::MoveRoll], can_react, react)]);
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
/// The heap is zeroed BSS: it costs nothing in the .wasm file.
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
struct Bump;

#[cfg(all(target_arch = "wasm32", feature = "guest"))]
#[global_allocator]
static ALLOC: Bump = Bump;

#[cfg(all(target_arch = "wasm32", feature = "guest"))]
static mut HEAP: [u8; 256 * 1024] = [0; 256 * 1024];

#[cfg(all(target_arch = "wasm32", feature = "guest"))]
static mut CURSOR: usize = 0;

#[cfg(all(target_arch = "wasm32", feature = "guest"))]
unsafe impl core::alloc::GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let align = layout.align().max(1);
        let start = (CURSOR + align - 1) & !(align - 1);
        let end = match start.checked_add(layout.size()) {
            Some(e) if e <= HEAP.len() => e,
            // Out of memory: return null so the allocator aborts into our
            // panic handler, which traps and takes the card out cleanly.
            _ => return core::ptr::null_mut(),
        };
        CURSOR = end;
        HEAP.as_mut_ptr().add(start)
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // never freed -- see the note above
    }
}
