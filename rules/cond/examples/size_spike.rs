//! Throwaway cdylib for the G1 wasm32 size measurement (docs/GUARDS.md §4.1).
//! Not part of the shipping glue; it only exists so the `.wasm` can be weighed
//! against `size_baseline` (same shape, no `cel`).

/// Touch the real crate surface so LTO cannot drop `cel` or the evaluator.
#[no_mangle]
pub extern "C" fn cond_spike(src_ptr: *const u8, src_len: usize) -> i32 {
    let src = unsafe { core::slice::from_raw_parts(src_ptr, src_len) };
    let Ok(src) = core::str::from_utf8(src) else {
        return -1;
    };
    match rules_cond::compile(src) {
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