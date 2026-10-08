//! Baseline cdylib for the G1 wasm32 size measurement: same export shape as
//! `size_spike`, but does not touch `rules_cond` / `cel`, so LTO drops them.
//! The `.wasm` delta is the cost of shipping the condition evaluator.

#[no_mangle]
pub extern "C" fn cond_spike(src_ptr: *const u8, src_len: usize) -> i32 {
    let src = unsafe { core::slice::from_raw_parts(src_ptr, src_len) };
    // Same shape as size_spike: parse a byte slice, return an int.
    if src.is_empty() {
        return -1;
    }
    src.iter().fold(0i32, |a, b| a.wrapping_add(*b as i32))
}