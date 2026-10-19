//! Native (non-wasm) ABI support: guest buffers cross the `ruleset` boundary as
//! 32-bit arena handles, not raw pointers.
//!
//! The host ABI is `ptr: i32, len: i32` because wasm32 pointers are 32-bit. A
//! 64-bit native build cannot put a real pointer in an `i32` -- `as_ptr() as
//! `i32` would truncate it and the host would read garbage. So on native the
//! "pointer" is an index into this thread-local arena, which the host reads (and
//! for out-buffers, writes) through [`read`] / [`write`].
//!
//! **Nesting.** A host import can re-enter the guest (`call_entry` →
//! [`crate::rt::on`] → another card body). Buffers interned by the *outer*
//! call must still be readable after the nested one returns, so the arena is a
//! **stack of frames**: [`enter`] pushes a fresh frame for a card call, [`exit`]
//! pops it. [`crate::rt::on`] holds a [`Frame`] guard across the body. (A flat
//! arena that was `clear()`ed on every entry emptied the parent's out-buffers
//! mid-call -- `buy_quotes` → nested buy hook → `out_read` returned `[]` →
//! `buf[..n]` panicked. That showed up as native/sandbox drift.)

use std::vec::Vec;

// Kani: `thread_local!` monomorphizes `catch_unwind` (std TLS), which ICEs
// kani-compiler 0.68.0 (intrinsics.rs:243). Verification is single-threaded,
// so a plain static is enough. Guest/native production builds keep TLS.
//
// Storage is a **stack of frames**, each frame a list of buffer slots. Nested
// card calls push a frame; handles are indices into the current frame.

// ---------------------------------------------------------------------------
// Kani facade: fixed-capacity frame stack. No `Vec`, no heap, no drop glue.
// CBMC treats `vec![0u8; len]` / `to_vec` / nested `Vec<Vec<Vec<u8>>>` as
// unbounded grow+drop paths (`extend_with` 31, `drop_glue` 62 in the prompt
// profile). A plain `[[[u8; BUF]; SLOTS]; FRAMES]` is array arithmetic.
// Production (`cfg(not(kani))`) keeps master's TLS `Vec` arena byte-for-byte.
// ---------------------------------------------------------------------------

#[cfg(kani)]
mod arena {
    /// Nested card calls (settle chains) remain well inside this.
    pub const MAX_FRAMES: usize = 8;
    /// Out-buffers per call. `cards_in` / `tiles_named` / mark scans use 1–3.
    pub const MAX_SLOTS: usize = 8;
    /// Bytes per out-buffer. The `cards_in` `[n, id…]` layout is 16; the
    /// largest probe writes 64.
    pub const MAX_BUF: usize = 64;

    #[derive(Clone, Copy)]
    pub struct Slot {
        pub buf: [u8; MAX_BUF],
        pub len: u8,
    }

    impl Slot {
        pub const fn empty() -> Self {
            Slot {
                buf: [0; MAX_BUF],
                len: 0,
            }
        }
    }

    #[derive(Clone, Copy)]
    pub struct Frame {
        pub slots: [Slot; MAX_SLOTS],
        pub n: u8,
    }

    impl Frame {
        pub const fn empty() -> Self {
            Frame {
                slots: [Slot::empty(); MAX_SLOTS],
                n: 0,
            }
        }
    }

    /// Single-threaded verification: `static mut` is enough.
    pub struct Arena;
    unsafe impl Sync for Arena {}
    pub static ARENA: Arena = Arena;
    pub static mut FRAMES: [Frame; MAX_FRAMES] = [Frame::empty(); MAX_FRAMES];
    pub static mut DEPTH: u8 = 0;
    pub static mut LIVE: bool = false;
}

#[cfg(not(kani))]
mod arena {
    use std::cell::RefCell;
    use std::vec::Vec;
    thread_local! {
        pub static ARENA: RefCell<Vec<Vec<Vec<u8>>>> = const { RefCell::new(Vec::new()) };
    }
}

#[cfg(kani)]
fn ensure_frame() {
    unsafe {
        if !arena::LIVE {
            arena::LIVE = true;
            arena::DEPTH = 1;
            arena::FRAMES[0] = arena::Frame::empty();
        } else if arena::DEPTH == 0 {
            arena::DEPTH = 1;
            arena::FRAMES[0] = arena::Frame::empty();
        }
    }
}

#[cfg(kani)]
fn frame_mut<R>(f: impl FnOnce(&mut arena::Frame) -> R) -> R {
    ensure_frame();
    let d = unsafe { (arena::DEPTH as usize).saturating_sub(1) };
    let d = d.min(arena::MAX_FRAMES - 1);
    f(unsafe { &mut arena::FRAMES[d] })
}

/// Open a frame if this call has none (probes / kani helpers intern without
/// [`enter`]). Returns after the stack is non-empty so `last_mut` is safe.
#[cfg(not(kani))]
fn ensure_frame() {
    use std::vec::Vec;
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        if stack.is_empty() {
            stack.push(Vec::new());
        }
    });
}

/// One card-call frame. `rt::on` holds this across the body so nested entries
/// push/pop around the parent's buffers.
pub struct Frame;

/// Push a fresh frame for a card call. Pair with [`exit`] (or hold [`Frame`]).
#[cfg(kani)]
pub fn enter() -> Frame {
    unsafe {
        if !arena::LIVE {
            arena::LIVE = true;
            arena::DEPTH = 1;
            arena::FRAMES[0] = arena::Frame::empty();
        } else {
            let d = arena::DEPTH as usize;
            if d < arena::MAX_FRAMES {
                arena::FRAMES[d] = arena::Frame::empty();
                arena::DEPTH = (d + 1) as u8;
            } else {
                // Safety valve: reset rather than alias handles.
                arena::DEPTH = 1;
                arena::FRAMES[0] = arena::Frame::empty();
            }
        }
    }
    Frame
}

#[cfg(not(kani))]
pub fn enter() -> Frame {
    use std::vec::Vec;
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        // Safety valve: a trap that skipped `Drop` would otherwise leak frames
        // until handles aliased. Depth 64 is far beyond any legal settle nest.
        if stack.len() > 64 {
            stack.clear();
        }
        stack.push(Vec::new());
    });
    Frame
}

/// Pop the current frame.
#[cfg(kani)]
pub fn exit() {
    unsafe {
        if arena::DEPTH > 1 {
            arena::DEPTH -= 1;
        }
    }
}

#[cfg(not(kani))]
pub fn exit() {
    with_stack(|stack| {
        stack.borrow_mut().pop();
    });
}

impl Drop for Frame {
    fn drop(&mut self) {
        exit();
    }
}

/// Intern bytes for the host to read. Returns `(handle, len)`.
#[cfg(kani)]
pub fn intern(bytes: &[u8]) -> (i32, i32) {
    frame_mut(|frame| {
        let i = frame.n as usize;
        if i >= arena::MAX_SLOTS {
            return (0, 0);
        }
        let n = bytes.len().min(arena::MAX_BUF);
        let mut slot = arena::Slot::empty();
        // Zero-loop byte copy (`.get`, not `[i]`).
        macro_rules! step {
            ($k:expr) => {
                if let Some(&b) = bytes.get($k) {
                    if $k < arena::MAX_BUF {
                        slot.buf[$k] = b;
                    }
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
        slot.len = n as u8;
        frame.slots[i] = slot;
        frame.n = (i + 1) as u8;
        (i as i32, n as i32)
    })
}

#[cfg(not(kani))]
pub fn intern(bytes: &[u8]) -> (i32, i32) {
    use std::vec::Vec;
    ensure_frame();
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        // Stable arena: intern into the bottom frame so handles survive nested
        // `enter`/`exit` (a per-frame index went stale and `read` padded NULs).
        let frame = &mut stack[0];
        frame.push(bytes.to_vec());
        // 1-based handle: 0 remains the wasm-style null / length-only probe.
        (frame.len() as i32, bytes.len() as i32)
    })
}

/// Reserve `len` zeroed bytes for the host to write into. Returns `(handle, len)`.
#[cfg(kani)]
pub fn reserve(len: usize) -> (i32, i32) {
    let n = len.min(arena::MAX_BUF);
    frame_mut(|frame| {
        let i = frame.n as usize;
        if i >= arena::MAX_SLOTS {
            return (0, n as i32);
        }
        frame.slots[i] = arena::Slot::empty();
        frame.slots[i].len = n as u8;
        frame.n = (i + 1) as u8;
        (i as i32, n as i32)
    })
}

#[cfg(not(kani))]
pub fn reserve(len: usize) -> (i32, i32) {
    use std::vec::Vec;
    ensure_frame();
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        let frame = &mut stack[0];
        frame.push(std::vec![0u8; len]);
        // 1-based handle (see `intern`).
        (frame.len() as i32, len as i32)
    })
}

/// The bytes behind a handle, up to `len`. Empty when the handle is stale.
///
/// Under `cfg(kani)`, a handle tagged [`crate::intern::INTERN_TAG`] is a
/// `NameId` (from `ctx::s`), not an arena index — resolve it from the intern
/// table (static bytes, no heap). Untagged handles are out-buffer arena
/// indices as before.
#[cfg(kani)]
pub fn read(handle: i32, len: usize) -> Vec<u8> {
    if let Some(id) = crate::intern::decode_handle(handle) {
        let b = crate::intern::resolve(id);
        // Zero-loop copy into a `Vec` (bump-backed under the kani allocator).
        let n = len.min(b.len()).min(arena::MAX_BUF);
        let mut out = Vec::new();
        macro_rules! step {
            ($k:expr) => {
                if $k < n {
                    if let Some(&x) = b.get($k) {
                        out.push(x);
                    }
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
        return out;
    }
    let want = len.min(arena::MAX_BUF);
    frame_mut(|frame| {
        let mut out = Vec::new();
        let slot = frame.slots.get(handle.max(0) as usize);
        let have = slot.map(|s| s.len as usize).unwrap_or(0);
        let n = want.min(have);
        if let Some(s) = slot {
            macro_rules! step {
                ($k:expr) => {
                    if $k < n {
                        out.push(s.buf[$k]);
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
        // Pad to `want` so callers can slice `[..n]` without panicking.
        // Zero-loop: at most MAX_BUF pads, each a one-byte push.
        macro_rules! pad {
            ($k:expr) => {
                if out.len() < want {
                    out.push(0);
                }
            };
        }
        pad!(0); pad!(1); pad!(2); pad!(3); pad!(4); pad!(5); pad!(6); pad!(7);
        pad!(8); pad!(9); pad!(10); pad!(11); pad!(12); pad!(13); pad!(14); pad!(15);
        pad!(16); pad!(17); pad!(18); pad!(19); pad!(20); pad!(21); pad!(22); pad!(23);
        pad!(24); pad!(25); pad!(26); pad!(27); pad!(28); pad!(29); pad!(30); pad!(31);
        pad!(32); pad!(33); pad!(34); pad!(35); pad!(36); pad!(37); pad!(38); pad!(39);
        pad!(40); pad!(41); pad!(42); pad!(43); pad!(44); pad!(45); pad!(46); pad!(47);
        pad!(48); pad!(49); pad!(50); pad!(51); pad!(52); pad!(53); pad!(54); pad!(55);
        pad!(56); pad!(57); pad!(58); pad!(59); pad!(60); pad!(61); pad!(62); pad!(63);
        out
    })
}

#[cfg(not(kani))]
pub fn read(handle: i32, len: usize) -> Vec<u8> {
    use std::vec::Vec;
    with_stack(|stack| {
        let stack = stack.borrow();
        // Handles are indices into the bottom (stable) frame; fall back to the
        // top frame for buffers reserved by `reserve` in the current call.
        let frame = stack.first().or_else(|| stack.last());
        let frame = match frame {
            Some(f) => f,
            None => return std::vec![0u8; len],
        };
        let idx = (handle - 1).max(0) as usize;
        let mut out = frame
            .get(idx)
            .map(|v| v[..len.min(v.len())].to_vec())
            .unwrap_or_default();
        // Pad to `len` so callers can slice `[..n]` without panicking when the
        // host wrote fewer bytes (or the handle was stale). Decoding then
        // fails soft (`unwrap_or_default`) instead of aborting the card.
        if out.len() < len {
            out.resize(len, 0);
        }
        out
    })
}

/// The bytes behind a name handle, as a slice of the intern table when the
/// handle is a `NameId` (no heap). Falls back to [`read`] for arena handles.
/// Host name lookups should prefer [`crate::intern::decode_handle`] +
/// [`crate::intern::resolve`] directly.
#[cfg(kani)]
pub fn read_name_bytes(handle: i32, len: usize) -> Vec<u8> {
    read(handle, len)
}

/// The host writing into a buffer reserved by [`reserve`].
#[cfg(kani)]
pub fn write(handle: i32, bytes: &[u8]) {
    frame_mut(|frame| {
        let idx = handle.max(0) as usize;
        if idx >= arena::MAX_SLOTS {
            return;
        }
        let n = bytes.len().min(arena::MAX_BUF).min(frame.slots[idx].len as usize);
        macro_rules! step {
            ($k:expr) => {
                if $k < n {
                    if let Some(&b) = bytes.get($k) {
                        frame.slots[idx].buf[$k] = b;
                    }
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
    })
}

#[cfg(not(kani))]
pub fn write(handle: i32, bytes: &[u8]) {
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        let frame = match stack.first_mut() {
            Some(f) => f,
            None => return,
        };
        let idx = (handle - 1).max(0) as usize;
        if let Some(v) = frame.get_mut(idx) {
            let n = bytes.len().min(v.len());
            v[..n].copy_from_slice(&bytes[..n]);
        }
    })
}

/// Drop everything interned in the **current** frame. `rt::on` uses [`enter`]/
/// [`Frame`] instead of wiping the whole stack (nested calls share the stack).
/// Kept for probes / kani helpers that intern without a frame guard.
#[cfg(kani)]
pub fn clear() {
    frame_mut(|frame| {
        *frame = arena::Frame::empty();
    });
}

#[cfg(not(kani))]
pub fn clear() {
    ensure_frame();
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        if let Some(frame) = stack.first_mut() {
            frame.clear();
        }
    });
}

// Production-only stack helper (Vec arena).
#[cfg(not(kani))]
fn with_stack<R>(f: impl FnOnce(&std::cell::RefCell<std::vec::Vec<std::vec::Vec<std::vec::Vec<u8>>>>) -> R) -> R {
    arena::ARENA.with(f)
}

#[cfg(all(test, not(kani)))]
mod tests {
    use super::*;

    #[test]
    fn first_reserve_round_trips_crystals() {
        let _f = enter();
        let (h, n) = reserve(8);
        assert_ne!(h, 0, "handle 0 is reserved for the length-only probe");
        assert_eq!(n, 8);
        write(h, b"crystals");
        assert_eq!(read(h, 8), b"crystals");
    }

    #[test]
    fn intern_round_trips_on_the_first_slot() {
        let _f = enter();
        let (h, n) = intern(b"crystals");
        assert_ne!(h, 0);
        assert_eq!(n, 8);
        assert_eq!(read(h, 8), b"crystals");
    }
}
