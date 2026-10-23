//! `cfg(kani)` bump allocator (TUNING §11 `kani_alloc`).
//!
//! CBMC models the system allocator's OOM / `RawVec::reserve` /
//! `handle_alloc_error` paths for every `String`/`Vec` in an unmodified card
//! body — including `alloc::format!` in a body key helper (`rec_key` and
//! friends). Those paths are pure SAT cost and showed up as the
//! `tsukushi_01_*` TIMEOUT cliff (fmt `PadAdapter` + memchr + alloc-error).
//!
//! This is a static arena: `alloc` is a cursor bump, `dealloc` is a no-op,
//! `realloc` grows in place when the block is the most recent one. The guest
//! wasm bump in [`crate::rt`] is unchanged (production dist hashes remain
//! byte-identical); this module is `cfg(kani)` only.
//!
//! Exhaustion is a **loud** `kani::assert` (never a silent null into
//! `handle_alloc_error`).

/// Bump allocator over a static arena. Unit struct so the `#[global_allocator]`
/// static is `Sync`.
pub struct KaniBump;

#[global_allocator]
static ALLOC: KaniBump = KaniBump;

/// Absolute arena ceiling. 16 KiB covers every unmodified body's `String`/`Vec`
/// traffic in the card corpus (TUNING §11); a wider body would trip the loud
/// exhaust assert rather than wander into the system allocator's OOM model.
const CAP: usize = 16 * 1024;

static mut ARENA: [u8; CAP] = [0; CAP];
static mut CURSOR: usize = 0;

fn bump_alloc(size: usize, align: usize) -> *mut u8 {
    let align = align.max(1);
    unsafe {
        let base = core::ptr::addr_of!(ARENA) as usize;
        if CURSOR == 0 {
            CURSOR = base;
        }
        let start = (CURSOR + align - 1) & !(align - 1);
        let end = match start.checked_add(size) {
            Some(e) if e <= base + CAP => e,
            _ => {
                #[cfg(kani)]
                kani::assert(false, "kani_alloc: arena exhausted");
                return core::ptr::null_mut();
            }
        };
        CURSOR = end;
        start as *mut u8
    }
}

unsafe impl core::alloc::GlobalAlloc for KaniBump {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        bump_alloc(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // never freed -- one verification run, one arena
    }

    unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: core::alloc::Layout,
        new_size: usize,
    ) -> *mut u8 {
        let old_size = layout.size();
        let addr = ptr as usize;
        // In place when this block is the most recent allocation (ends at the
        // cursor). Same recipe as the guest wasm bump in `rt.rs`.
        unsafe {
            if addr.wrapping_add(old_size) == CURSOR {
                let base = core::ptr::addr_of!(ARENA) as usize;
                let end = match addr.checked_add(new_size) {
                    Some(e) if e <= base + CAP => e,
                    _ => {
                        #[cfg(kani)]
                        kani::assert(false, "kani_alloc: arena exhausted");
                        return core::ptr::null_mut();
                    }
                };
                CURSOR = end;
                return ptr;
            }
        }
        // Otherwise alloc-copy-free (free is a no-op, so the old bytes leak
        // into the arena -- fine for one run).
        let new = bump_alloc(new_size, layout.align());
        if !new.is_null() && !ptr.is_null() {
            let n = old_size.min(new_size);
            core::ptr::copy_nonoverlapping(ptr, new, n);
        }
        new
    }
}
