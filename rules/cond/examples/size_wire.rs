//! Throwaway cdylib for the **runtime-only** wasm32 size measurement
//! (docs/GUARDS.md §8). Same export shape as `size_spike`, but loads a
//! serialized condition (`Cond::from_bytes`, postcard) instead of parsing
//! one, so LTO keeps the evaluator and the wire decoder and drops the
//! parser. Build with:
//!
//! ```text
//! cargo build -p rules-cond --example size_wire --no-default-features \
//!     --features runtime-only --target wasm32-unknown-unknown
//! ```

/// Touch the real crate surface so LTO cannot drop `cel` or the evaluator.
#[no_mangle]
pub extern "C" fn cond_spike(src_ptr: *const u8, src_len: usize) -> i32 {
    let bytes = unsafe { core::slice::from_raw_parts(src_ptr, src_len) };
    match rules_cond::Cond::from_bytes(bytes) {
        Ok(cond) => {
            let win = rules_cond::WindowCtx::default();
            let cand = rules_cond::CandidateCtx::default();
            if cond.eval(&win, &cand) {
                1
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}