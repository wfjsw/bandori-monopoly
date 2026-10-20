//! Export runtime used by [`ruleset!`](crate::ruleset).

use crate::abi::{export, pack, ManifestEntry, ManifestOn, ManifestTileText};
#[cfg(target_arch = "wasm32")]
use alloc::{boxed::Box, string::String, vec::Vec};

use crate::meta::{ordinal_of, MetaDoc, MetaRow, RosterSources};
use crate::{CardDef, On};

/// The card manifest, `postcard`-encoded (ABI v5 -- the guest hand-writes no
/// JSON at all: messages and manifests both travel as serde bytes).
///
/// ABI v73: kind/variant agreement is enforced here at ruleset build --
/// a `On::Filter` may only list filter kinds and a `On::Action` only
/// non-filter kinds ([`crate::On::check_kind_variant`]).
pub fn manifest(bands: &'static [&'static [CardDef]]) -> i64 {
    for band in bands {
        for c in band.iter() {
            for (ei, o) in c.on.iter().enumerate() {
                if let Err(kind) = o.check_kind_variant() {
                    panic!(
                        "ruleset build: {} entry {ei} kind {} does not match {} (see TriggerKind::is_filter)",
                        c.id,
                        kind.spelling(),
                        match o.kind() {
                            crate::abi::OnKind::Filter => "On::Filter",
                            _ => "On::Action",
                        }
                    );
                }
            }
        }
    }
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
                    messages: match o {
                        On::Message(names, ..) => names.iter().map(|s| String::from(*s)).collect(),
                        _ => Vec::new(),
                    },
                    label: c
                        .labels
                        .iter()
                        .find(|(e, _)| *e == ei as i32)
                        .map(|(_, l)| String::from(*l)),
                    presentation: c.presentation(ei as i32).header,
                    flash: c.presentation(ei as i32).flash,
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
            // Declaration order -- the view's text precedence uses this as the
            // third component of its order key (docs/ENGINE.md 「Effect
            // ordering」).
            texts: c
                .texts
                .iter()
                .map(|(e, slot, tile, key)| ManifestTileText {
                    entry: *e,
                    slot: String::from(*slot),
                    tile: String::from(*tile),
                    key: String::from(*key),
                })
                .collect(),
            board_role: String::from(c.board_role),
            governs: c.governs.iter().map(|k| String::from(*k)).collect(),
            gate_msgs: c
                .gate_msgs
                .iter()
                .map(|(g, k)| (*g, String::from(*k)))
                .collect(),
            exemption_prop: String::from(c.exemption_prop),
            gate_skip_msgs: c
                .gate_skip_msgs
                .iter()
                .map(|(g, k)| (*g, String::from(*k)))
                .collect(),
        })
        .collect();
    leak(postcard::to_allocvec(&entries).unwrap_or_default())
}

/// `ruleset_metadata` -- the roster metadata blob (ABI v63), built only when
/// the host asks. Card rows come from every `CardDef` that declares
/// [`crate::meta::CardMeta`]; non-card rows ride `sources.lists`. Sorted by
/// `(kind, ordinal)` so the bytes are deterministic regardless of registration
/// order.
pub fn metadata(bands: &'static [&'static [CardDef]], sources: &RosterSources) -> i64 {
    let mut rows: Vec<MetaRow> = Vec::new();
    for band in bands {
        for c in band.iter() {
            if let Some(m) = c.meta {
                rows.push(m.to_row(c.id, ordinal_of(sources.card_ordinals, c.id)));
            }
        }
    }
    for list in sources.lists {
        for r in (list.rows)() {
            let mut row = r;
            row.kind = String::from(list.kind);
            // Canonical data-file order wins over whatever the def left in
            // `ordinal` (the shared table is the one source of truth).
            row.ordinal = ordinal_of(list.ordinals, &row.id);
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(a.ordinal.cmp(&b.ordinal))
            .then(a.id.cmp(&b.id))
    });
    let doc = MetaDoc { rows };
    leak(postcard::to_allocvec(&doc).unwrap_or_default())
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
    // Real pointers only for a pure wasm32 guest module. Native **and**
    // the wasm32-linked monolith (`native-rules`) go through the arena:
    // `NativeHost` always reads `card_sdk::native::read`.
    #[cfg(all(target_arch = "wasm32", not(feature = "native-rules")))]
    {
        let b: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        pack(b.as_ptr() as u32, b.len() as u32)
    }
    #[cfg(any(not(target_arch = "wasm32"), feature = "native-rules"))]
    {
        let (h, l) = crate::native::intern(&bytes);
        pack(h as u32, l as u32)
    }
}

/// A card handle is an index into the *flattened* manifest: every band's
/// table in the order they were passed to `ruleset!`.
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

/// `ruleset_on` -- call one entry point. See [`export::ON`] for the encoding.
pub fn on(
    bands: &'static [&'static [CardDef]],
    idx: i32,
    entry: i32,
    op: i32,
    player_id: i32,
) -> i64 {
    // Call boundary: every guest buffer this entry hands the host is interned
    // in a **per-call native frame**. Nested `call_entry`s push their own frame
    // so the parent's out-buffers remain valid after the child returns (the old
    // flat `clear()` emptied `buy_quotes` mid-call). The `Frame` guard pops on
    // exit or unwind (`C-unwind` runs `Drop`).
    #[cfg(any(not(target_arch = "wasm32"), feature = "native-rules"))]
    let _native_frame = crate::native::enter();
    let Some(o) = card(bands, idx).on.get(entry.max(0) as usize) else {
        panic!("bad entry {entry} on card {idx}")
    };
    match (*o, op) {
        (On::Counteract(_, _, _, guard, _), export::OP_GUARD)
        | (On::Action(_, _, _, guard, _), export::OP_GUARD)
        | (On::Filter(_, _, guard, _), export::OP_GUARD)
        | (On::FilterFlag(_, _, guard, _), export::OP_GUARD)
        | (On::FilterValue(_, _, guard, _), export::OP_GUARD)
        | (On::Settle(_, guard, _), export::OP_GUARD)
        | (On::Mortgage(_, guard, _), export::OP_GUARD)
        | (On::Redeem(_, guard, _), export::OP_GUARD)
        | (On::Buy(_, guard, _), export::OP_GUARD)
        | (On::Build(_, guard, _), export::OP_GUARD)
        | (On::Message(_, _, guard, _), export::OP_GUARD) => match guard {
            // G4-deleted residual: the condition alone decides, and the host
            // normally skips this call (`has_guard == false`). If it does ask,
            // the residual admits.
            None => 1,
            Some(g) => g(player_id) as i64,
        },
        (On::Play(_, _, why, _), export::OP_GUARD) => match why {
            Some(why) => match why(player_id) {
                None => 0,
                // Kani: the Play `why` reason is a `Msg`; serialising it allocates
                // a `Vec` (and the facade `Msg` is not the wire form). The
                // symbolic host treats a non-zero reason as "has reason" and
                // never reads the bytes. Production keeps the real postcard.
                #[cfg(kani)]
                Some(_reason) => 1,
                #[cfg(not(kani))]
                Some(reason) => leak(postcard::to_allocvec(&reason).unwrap_or_default()),
            },
            None => 0,
        },
        // v74: pure Filter bodies. The host reads `trigger::value()` in,
        // we return the transformed figure (or 0/1 for a flag), and the
        // wrapper writes it back through the existing `set_*` imports so the
        // raise site's `t.value` / `b.price` readback is unchanged.
        (On::Filter(_, _, _, run), _) => {
            let v_in = crate::ctx::trigger::value();
            let v_out = {
                let _p = crate::pure::enter();
                run(player_id, v_in)
            };
            if v_out != v_in {
                crate::ctx::trigger::set_pay_amount(v_out);
                crate::ctx::trigger::set_price(v_out);
                crate::ctx::trigger::set_move_roll(v_out);
            }
            0
        }
        (On::FilterFlag(_, _, _, run), _) => {
            let keep = {
                let _p = crate::pure::enter();
                run(player_id, crate::ctx::trigger::value())
            };
            if !keep {
                crate::ctx::trigger::set_cancelled();
            }
            0
        }
        (On::FilterValue(_, _, _, run), _) => {
            run.run(player_id);
            0
        }
        (On::Counteract(_, _, _, _, run), _)
        | (On::Play(_, _, _, run), _)
        | (On::Action(_, _, _, _, run), _)
        | (On::Settle(_, _, run), _)
        | (On::Mortgage(_, _, run), _)
        | (On::Redeem(_, _, run), _)
        | (On::Buy(_, _, run), _)
        | (On::Build(_, _, run), _)
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
/// card_sdk::ruleset!(CARDS);
/// ```
///
/// On `wasm32` this emits the `ruleset_*` exports the sandbox host calls. On a
/// native build (the `rules-native` bot path) it instead publishes a plain
/// `RULESET` table so every rule crate can link into one binary without
/// `#[no_mangle]` clashes; `rules-native` calls [`crate::rt::on`] over the
/// concatenated tables.
///
/// A trailing `; roster $sources` clause (ABI v63) points `ruleset_metadata`
/// at this game's [`crate::meta::RosterSources`] (card ordinals + the skill
/// crates' character / band tables). Without the clause the export still
/// exists but carries only cards that declare [`crate::meta::CardMeta`].
///
/// ```ignore
/// card_sdk::ruleset!(CARDS; roster: &RosterSources {
///     card_ordinals: bandori_std::roster_order::CARDS,
///     lists: &[...],
/// });
/// ```
#[macro_export]
macro_rules! ruleset {
    ($($cards:expr),+ $(,)?) => {
        $crate::ruleset!(@body [$($cards),+] ; &$crate::meta::EMPTY_ROSTER);
    };
    ($($cards:expr),+ ; roster: $sources:expr $(,)?) => {
        $crate::ruleset!(@body [$($cards),+] ; $sources);
    };
    (@body [$($cards:expr),+] ; $sources:expr) => {
        /// Every band's table, in the order given. `static` so the export
        /// functions can hand out `'static` references into it.
        #[allow(dead_code)]
        static BAND_TABLE: &[&[$crate::CardDef]] = &[$($cards),+];

        /// Linked-guest mode (native, or wasm32 + `native-rules`): the table
        /// `rules-native` aggregates. No `#[no_mangle]` so every rule crate can
        /// link into one binary.
        #[cfg(any(not(target_arch = "wasm32"), feature = "native-rules"))]
        pub static RULESET: &[&[$crate::CardDef]] = &[$($cards),+];

        /// Linked call entry -- same body as the wasm export, no `#[no_mangle]`
        /// so every rule crate can link into one binary.
        #[cfg(any(not(target_arch = "wasm32"), feature = "native-rules"))]
        pub fn ruleset_on(card: i32, entry: i32, op: i32, player_id: i32) -> i64 {
            $crate::rt::on(BAND_TABLE, card, entry, op, player_id)
        }

        /// Linked manifest -- same body as the wasm export.
        #[cfg(any(not(target_arch = "wasm32"), feature = "native-rules"))]
        pub fn ruleset_manifest() -> i64 {
            $crate::rt::manifest(BAND_TABLE)
        }

        /// Linked roster metadata (ABI v63) -- same body as the wasm export.
        #[cfg(any(not(target_arch = "wasm32"), feature = "native-rules"))]
        pub fn ruleset_metadata() -> i64 {
            $crate::rt::metadata(BAND_TABLE, $sources)
        }

        #[cfg(all(target_arch = "wasm32", not(feature = "native-rules")))]
        #[no_mangle]
        pub extern "C" fn ruleset_abi_version() -> i32 {
            $crate::abi::ABI_VERSION
        }
        #[cfg(all(target_arch = "wasm32", not(feature = "native-rules")))]
        #[no_mangle]
        pub extern "C" fn ruleset_manifest() -> i64 {
            $crate::rt::manifest(BAND_TABLE)
        }
        #[cfg(all(target_arch = "wasm32", not(feature = "native-rules")))]
        #[no_mangle]
        pub extern "C" fn ruleset_on(card: i32, entry: i32, op: i32, player_id: i32) -> i64 {
            $crate::rt::on(BAND_TABLE, card, entry, op, player_id)
        }
        #[cfg(all(target_arch = "wasm32", not(feature = "native-rules")))]
        #[no_mangle]
        pub extern "C" fn ruleset_metadata() -> i64 {
            $crate::rt::metadata(BAND_TABLE, $sources)
        }
    };
}

/// Exports for a module holding exactly one card (the normal layout).
///
/// ```ignore
/// pub const CARD: CardDef = CardDef::new("AG:Y.O.L.O", &[On::Counteract(&[ChainKind::DiceRollResult], "", r#"你的一次掷骰小于6"#, Some(can_counteract), counteract)]);
/// card_sdk::ruleset_card!(CARD);
/// ```
#[macro_export]
macro_rules! ruleset_card {
    ($card:expr) => {
        $crate::ruleset!(&[$card]);
    };
}

/// Guest panic handler: trap. The host turns the trap into "this card is out",
/// the same contract as a malformed message -- a card never gets to show a raw
/// panic to players.
#[cfg(all(target_arch = "wasm32", feature = "guest", not(feature = "native-rules")))]
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
#[cfg(all(target_arch = "wasm32", feature = "guest", not(feature = "native-rules")))]
struct Bump;

#[cfg(all(target_arch = "wasm32", feature = "guest", not(feature = "native-rules")))]
#[global_allocator]
static ALLOC: Bump = Bump;

/// Hard ceiling on the absolute addresses the bump allocator will hand out
/// (i.e. the whole linear memory, stack and data included -- `__heap_base` is
/// ~1 MiB into it). Matches the host's per-instance `memory_size` limit in
/// `game-rules` (`be_*::MAX_MEMORY_BYTES`); either side refusing is enough to
/// stop a runaway effect, and the guest hitting its own ceiling first keeps
/// the failure deterministic. Growth past this returns null and the run
/// aborts into the panic handler, which traps and takes the card out cleanly.
#[cfg(all(target_arch = "wasm32", feature = "guest", not(feature = "native-rules")))]
const MAX_MEMORY: usize = 16 * 1024 * 1024;

/// One wasm page of linear memory.
#[cfg(all(target_arch = "wasm32", feature = "guest", not(feature = "native-rules")))]
const PAGE: usize = 64 * 1024;

/// Bump cursor: the absolute address of the next free byte. `0` means "not
/// started yet" -- the first allocation latches [`__heap_base`]. Nothing in
/// the guest reads this for control flow, so it remains deterministic.
#[cfg(all(target_arch = "wasm32", feature = "guest", not(feature = "native-rules")))]
static mut CURSOR: usize = 0;

#[cfg(all(target_arch = "wasm32", feature = "guest", not(feature = "native-rules")))]
extern "C" {
    /// Linker-provided (rust-lld): the first address past data, bss and the
    /// wasm shadow stack. The heap starts here; below it is not ours.
    static __heap_base: u8;
}

/// Make sure linear memory covers the absolute address `end`. Grows by whole
/// pages. Returns `false` if the host refused (its own per-instance limit) --
/// `memory.grow` reports failure with `-1` instead of trapping, so we turn it
/// into the allocator's null contract here.
#[cfg(all(target_arch = "wasm32", feature = "guest", not(feature = "native-rules")))]
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

#[cfg(all(target_arch = "wasm32", feature = "guest", not(feature = "native-rules")))]
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
