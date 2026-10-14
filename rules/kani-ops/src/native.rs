//! Guest name arena: `ptr: i32, len: i32` wire handles.
//!
//! Single-threaded static-mut arena (the Kani model and conf tests are
//! single-threaded; wasm guest uses the same). Portable: no `thread_local!`.

#![allow(missing_docs)]

use alloc::vec::Vec;

/// Arena of out-buffers. `u16::MAX` slot = empty.
const MAX_FRAMES: usize = 8;
const MAX_BUFS: usize = 32;

struct Frame {
    bufs: [[u8; 0]; 0], // placeholder — see Arena
    n: usize,
}

/// Simple append-only byte arena (handles = slot index, len).
struct Arena {
    data: Vec<Vec<u8>>,
}

static mut ARENA: Option<Arena> = None;

fn with_arena<R>(f: impl FnOnce(&mut Arena) -> R) -> R {
    unsafe {
        if ARENA.is_none() {
            ARENA = Some(Arena { data: Vec::new() });
        }
        f(ARENA.as_mut().unwrap_unchecked())
    }
}

/// Intern `bytes` into the arena. Returns `(handle, len)`.
pub fn intern(bytes: &[u8]) -> (i32, i32) {
    with_arena(|a| {
        a.data.push(bytes.to_vec());
        ((a.data.len() - 1) as i32, bytes.len() as i32)
    })
}

/// Reserve `len` zero bytes; returns `(handle, len)`.
pub fn alloc(len: usize) -> (i32, i32) {
    with_arena(|a| {
        a.data.push(alloc::vec![0u8; len]);
        ((a.data.len() - 1) as i32, len as i32)
    })
}

/// The bytes behind a handle, up to `len`. Empty when the handle is stale.
///
/// Under `cfg(kani)`, a handle tagged [`crate::intern::INTERN_TAG`] is a
/// `NameId` (from `ctx::s`), not an arena index — resolve it from the intern
/// table (static bytes, no heap).
pub fn read(handle: i32, len: usize) -> Vec<u8> {
    #[cfg(kani)]
    {
        if let Some(id) = crate::intern::decode_handle(handle) {
            let b = crate::intern::resolve(id);
            return b[..len.min(b.len())].to_vec();
        }
    }
    with_arena(|a| {
        a.data
            .get(handle.max(0) as usize)
            .map(|v| {
                let n = len.min(v.len());
                let mut out = v[..n].to_vec();
                if out.len() < len {
                    out.resize(len, 0);
                }
                out
            })
            .unwrap_or_else(|| alloc::vec![0u8; len])
    })
}

/// Copy `bytes` into the arena slot `handle`.
pub fn write(handle: i32, bytes: &[u8]) {
    with_arena(|a| {
        if let Some(slot) = a.data.get_mut(handle.max(0) as usize) {
            let n = bytes.len().min(slot.len());
            slot[..n].copy_from_slice(&bytes[..n]);
        }
    });
}

/// Read and interpret as a name (intern-tagged handles only).
pub fn read_name_bytes(handle: i32, len: usize) -> Vec<u8> {
    read(handle, len)
}


/// Reserve `cap` zero bytes for a guest out-buffer. Returns `(handle, len)`.
pub fn reserve(cap: usize) -> (i32, i32) {
    alloc(cap)
}

/// Frame guard (no-op on the flat arena; keeps call-site shape).
pub struct FrameGuard;

/// Enter a guest frame. Returns a guard that pops on drop (no-op).
pub fn enter() -> FrameGuard {
    FrameGuard
}


/// Clear the arena (conf tests between cases).
pub fn clear() {
    with_arena(|a| a.data.clear());
}
