//! Native (non-wasm) ABI support: guest buffers cross the `bandori` boundary as
//! 32-bit arena handles, not raw pointers.
//!
//! The host ABI is `ptr: i32, len: i32` because wasm32 pointers are 32-bit. A
//! 64-bit native build cannot put a real pointer in an `i32` -- `as_ptr() as
//! i32` would truncate it and the host would read garbage. So on native the
//! "pointer" is an index into this thread-local arena, which the host reads (and
//! for out-buffers, writes) through [`read`] / [`write`].
//!
//! The worker is single-threaded per request, so a thread-local arena is enough;
//! [`crate::rt::on`] clears it at the start of each card call and nothing
//! outlives one. This also stops the old `mem::forget` leak: on native the bytes
//! are copied here and dropped normally.

use std::vec::Vec;

// Kani: `thread_local!` monomorphizes `catch_unwind` (std TLS), which ICEs
// kani-compiler 0.68.0 (intrinsics.rs:243). Verification is single-threaded,
// so a plain static is enough. Guest/native production builds keep TLS.
#[cfg(kani)]
mod arena {
    use std::vec::Vec;
    /// Single-threaded verification: `static mut` is enough and is `Sync`
    /// by fiat. Production keeps `thread_local!` below.
    pub struct Arena;
    unsafe impl Sync for Arena {}
    pub static ARENA: Arena = Arena;
    pub static mut BUF: Option<Vec<Vec<u8>>> = None;
    pub fn with<R>(f: impl FnOnce(&std::cell::RefCell<Vec<Vec<u8>>>) -> R) -> R {
        // A thread-local-like cell built from a static Vec, wrapped in
        // RefCell only at the use site via a temporary. Simpler: mutate BUF
        // through a RefCell facade.
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
        pub static ARENA: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    }
}

use arena::ARENA;

#[cfg(kani)]
fn with_arena<R>(f: impl FnOnce(&RefCell<Vec<Vec<u8>>>) -> R) -> R {
    arena::with(f)
}

#[cfg(not(kani))]
fn with_arena<R>(f: impl FnOnce(&RefCell<Vec<Vec<u8>>>) -> R) -> R {
    arena::ARENA.with(f)
}

#[allow(unused_imports)]
use std::cell::RefCell;


/// Intern bytes for the host to read. Returns `(handle, len)`.
pub fn intern(bytes: &[u8]) -> (i32, i32) {
    with_arena(|a| {
        let mut a = a.borrow_mut();
        a.push(bytes.to_vec());
        ((a.len() - 1) as i32, bytes.len() as i32)
    })
}

/// Reserve `len` zeroed bytes for the host to write into. Returns `(handle, len)`.
pub fn reserve(len: usize) -> (i32, i32) {
    with_arena(|a| {
        let mut a = a.borrow_mut();
        a.push(std::vec![0u8; len]);
        ((a.len() - 1) as i32, len as i32)
    })
}

/// The bytes behind a handle, up to `len`. Empty when the handle is stale.
pub fn read(handle: i32, len: usize) -> Vec<u8> {
    with_arena(|a| {
        a.borrow()
            .get(handle as usize)
            .map(|v| v[..len.min(v.len())].to_vec())
            .unwrap_or_default()
    })
}

/// The host writing into a buffer reserved by [`reserve`].
pub fn write(handle: i32, bytes: &[u8]) {
    with_arena(|a| {
        if let Some(v) = a.borrow_mut().get_mut(handle as usize) {
            let n = bytes.len().min(v.len());
            v[..n].copy_from_slice(&bytes[..n]);
        }
    })
}

/// Drop everything interned since the last call. Runs at the end of a card call.
pub fn clear() {
    with_arena(|a| a.borrow_mut().clear());
}
