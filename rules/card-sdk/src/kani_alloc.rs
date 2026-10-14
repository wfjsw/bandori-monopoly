//! `cfg(kani)` bump allocator: heap-free `String` / `Vec` for card bodies.
//!
//! CBMC models `malloc`/`free` as unbounded heap objects, so every
//! `String::from` / `Vec::push` in an unmodified body pays a symbolic-heap
//! tax (`extend_with` 31, `drop_glue<[String]>` 62 on the prompt profile).
//! A static bump arena turns those into array-index arithmetic: `alloc` is a
//! cursor, `dealloc` is a no-op (like the wasm guest bump), `realloc` grows
//! the most-recent block in place.
//!
//! Production (`cfg(not(kani))`) never sees this module — the system (or the
//! wasm guest `Bump` in `rt.rs`) allocator is unchanged.

/// Backing store. 16 KiB is far above the pilot bodies' handful of 1-byte
/// facade strings + small `Vec`s; exhaustion aborts (loud, like OOM).
#[cfg(kani)]
const HEAP: usize = 16 * 1024;

#[cfg(kani)]
static mut HEAP_BYTES: [u8; HEAP] = [0; HEAP];

/// Next free offset. `0` is a valid offset; use `STARTED` to mean "no alloc yet".
#[cfg(kani)]
static mut CURSOR: usize = 0;

#[cfg(kani)]
static mut STARTED: bool = false;

#[cfg(kani)]
pub struct Bump;

#[cfg(kani)]
unsafe impl core::alloc::GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let align = layout.align().max(1);
        if !STARTED {
            STARTED = true;
            CURSOR = 0;
        }
        let start = (CURSOR + align - 1) & !(align - 1);
        let end = match start.checked_add(layout.size()) {
            Some(e) if e <= HEAP => e,
            _ => return core::ptr::null_mut(),
        };
        CURSOR = end;
        HEAP_BYTES.as_mut_ptr().add(start)
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // never freed — same contract as the wasm guest bump
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: core::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        let old_size = layout.size();
        let base = HEAP_BYTES.as_mut_ptr();
        let addr = (ptr as usize).wrapping_sub(base as usize);
        // In place when the block is the most recent allocation.
        if addr.wrapping_add(old_size) == CURSOR {
            let end = match addr.checked_add(new_size) {
                Some(e) if e <= HEAP => e,
                _ => return core::ptr::null_mut(),
            };
            CURSOR = end;
            return ptr;
        }
        let new_layout =
            match core::alloc::Layout::from_size_align(new_size, layout.align().max(1)) {
                Ok(l) => l,
                Err(_) => return core::ptr::null_mut(),
            };
        let new_ptr = self.alloc(new_layout);
        if !new_ptr.is_null() {
            let n = old_size.min(new_size).min(256);
            // Zero-loop byte copy (`.add` on a constant bound; no Vec walk).
            macro_rules! step {
                ($i:expr) => {
                    if $i < n {
                        *new_ptr.add($i) = *ptr.add($i);
                    }
                };
            }
            step!(0); step!(1); step!(2); step!(3); step!(4); step!(5); step!(6); step!(7);
            step!(8); step!(9); step!(10); step!(11); step!(12); step!(13); step!(14); step!(15);
            step!(16); step!(17); step!(18); step!(19); step!(20); step!(21); step!(22); step!(23);
            step!(24); step!(25); step!(26); step!(27); step!(28); step!(29); step!(30); step!(31);
            step!(32); step!(33); step!(34); step!(35); step!(36); step!(37); step!(38); step!(39);
            step!(40); step!(41); step!(42); step!(43); step!(44); step!(45); step!(46); step!(47);
            step!(48); step!(49); step!(50); step!(51); step!(52); step!(53); step!(54); step!(55);
            step!(56); step!(57); step!(58); step!(59); step!(60); step!(61); step!(62); step!(63);
        }
        new_ptr
    }
}

#[cfg(kani)]
#[global_allocator]
static ALLOC: Bump = Bump;