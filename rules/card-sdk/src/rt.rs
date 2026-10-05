//! Export runtime used by [`bandori_ruleset!`](crate::bandori_ruleset).

use crate::abi::{pack, ManifestEntry};
#[cfg(target_arch = "wasm32")]
use alloc::{boxed::Box, string::String, vec::Vec};

use crate::CardDef;

/// The card manifest, `postcard`-encoded (ABI v5 -- the guest hand-writes no
/// JSON at all: messages and manifests both travel as serde bytes).
pub fn manifest(bands: &'static [&'static [CardDef]]) -> i64 {
    let entries: Vec<ManifestEntry> = bands
        .iter()
        .flat_map(|band| band.iter())
        .map(|c| ManifestEntry {
            id: String::from(c.id),
            play: c.play.is_some(),
            react: c.react.is_some(),
        })
        .collect();
    let bytes = postcard::to_allocvec(&entries).unwrap_or_default();
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

pub fn play(bands: &'static [&'static [CardDef]], idx: i32, seat: i32) {
    if let Some(f) = card(bands, idx).play {
        f(seat)
    }
}

pub fn can_react(bands: &'static [&'static [CardDef]], idx: i32, seat: i32) -> i32 {
    card(bands, idx).can_react.map_or(0, |f| f(seat) as i32)
}

pub fn react(bands: &'static [&'static [CardDef]], idx: i32, seat: i32) {
    if let Some(f) = card(bands, idx).react {
        f(seat)
    }
}

/// `Card.WhyNot` -- packed `(ptr << 32) | len` of a postcard `Msg` reason, or 0
/// when the card is playable. The bytes live as long as the run (bump allocator).
pub fn why_not(bands: &'static [&'static [CardDef]], idx: i32, seat: i32) -> i64 {
    let Some(f) = card(bands, idx).why_not else { return 0 };
    let Some(reason) = f(seat) else { return 0 };
    let bytes = postcard::to_allocvec(&reason).unwrap_or_default();
    let b: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    pack(b.as_ptr() as u32, b.len() as u32)
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
        pub extern "C" fn bandori_play(card: i32, seat: i32) {
            $crate::rt::play(BAND_TABLE, card, seat)
        }
        #[no_mangle]
        pub extern "C" fn bandori_can_react(card: i32, seat: i32) -> i32 {
            $crate::rt::can_react(BAND_TABLE, card, seat)
        }
        #[no_mangle]
        pub extern "C" fn bandori_react(card: i32, seat: i32) {
            $crate::rt::react(BAND_TABLE, card, seat)
        }
        #[no_mangle]
        pub extern "C" fn bandori_why_not(card: i32, seat: i32) -> i64 {
            $crate::rt::why_not(BAND_TABLE, card, seat)
        }
    };
}

/// Exports for a module holding exactly one card (the normal layout).
///
/// ```ignore
/// pub const CARD: CardDef = CardDef { id: "AG:Y.O.L.O", .. };
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
