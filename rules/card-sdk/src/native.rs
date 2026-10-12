//! Native (non-wasm) ABI support: guest buffers cross the `bandori` boundary as
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

#[cfg(kani)]
mod arena {
    use std::vec::Vec;
    /// Single-threaded verification: `static mut` is enough and is `Sync`
    /// by fiat. Production keeps `thread_local!` below.
    pub struct Arena;
    unsafe impl Sync for Arena {}
    pub static ARENA: Arena = Arena;
    pub static mut BUF: Option<Vec<Vec<Vec<u8>>>> = None;
    pub fn with<R>(f: impl FnOnce(&std::cell::RefCell<Vec<Vec<Vec<u8>>>>) -> R) -> R {
        unsafe {
            if BUF.is_none() {
                BUF = Some(Vec::new());
            }
            let cell = std::cell::RefCell::new(std::mem::take(BUF.as_mut().unwrap()));
            let r = f(&cell);
            *BUF.as_mut().unwrap() = cell.into_inner();
            r
        }
    }
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
fn with_stack<R>(f: impl FnOnce(&RefCell<Vec<Vec<Vec<u8>>>>) -> R) -> R {
    arena::with(f)
}

#[cfg(not(kani))]
fn with_stack<R>(f: impl FnOnce(&RefCell<Vec<Vec<Vec<u8>>>>) -> R) -> R {
    arena::ARENA.with(f)
}

#[allow(unused_imports)]
use std::cell::RefCell;

/// Open a frame if this call has none (probes / kani helpers intern without
/// [`enter`]). Returns after the stack is non-empty so `last_mut` is safe.
fn ensure_frame() {
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
pub fn enter() -> Frame {
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
pub fn intern(bytes: &[u8]) -> (i32, i32) {
    ensure_frame();
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        let frame = stack.last_mut().expect("native frame");
        frame.push(bytes.to_vec());
        ((frame.len() - 1) as i32, bytes.len() as i32)
    })
}

/// Reserve `len` zeroed bytes for the host to write into. Returns `(handle, len)`.
pub fn reserve(len: usize) -> (i32, i32) {
    ensure_frame();
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        let frame = stack.last_mut().expect("native frame");
        frame.push(std::vec![0u8; len]);
        ((frame.len() - 1) as i32, len as i32)
    })
}

/// The bytes behind a handle, up to `len`. Empty when the handle is stale.
///
/// Under `cfg(kani)`, a handle tagged [`crate::intern::INTERN_TAG`] is a
/// `NameId` (from `ctx::s`), not an arena index — resolve it from the intern
/// table (static bytes, no heap). Untagged handles are out-buffer arena
/// indices as before.
pub fn read(handle: i32, len: usize) -> Vec<u8> {
    #[cfg(kani)]
    {
        if let Some(id) = crate::intern::decode_handle(handle) {
            let b = crate::intern::resolve(id);
            return b[..len.min(b.len())].to_vec();
        }
    }
    with_stack(|stack| {
        let stack = stack.borrow();
        let frame = match stack.last() {
            Some(f) => f,
            None => return std::vec![0u8; len],
        };
        let mut out = frame
            .get(handle as usize)
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
pub fn write(handle: i32, bytes: &[u8]) {
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        let frame = match stack.last_mut() {
            Some(f) => f,
            None => return,
        };
        if let Some(v) = frame.get_mut(handle as usize) {
            let n = bytes.len().min(v.len());
            v[..n].copy_from_slice(&bytes[..n]);
        }
    })
}

/// Drop everything interned in the **current** frame. `rt::on` uses [`enter`]/
/// [`Frame`] instead of wiping the whole stack (nested calls share the stack).
/// Kept for probes / kani helpers that intern without a frame guard.
pub fn clear() {
    ensure_frame();
    with_stack(|stack| {
        let mut stack = stack.borrow_mut();
        if let Some(frame) = stack.last_mut() {
            frame.clear();
        }
    });
}