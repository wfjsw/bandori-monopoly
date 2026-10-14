//! World model — re-exported from [`kani_model::world`].
//!
//! The model lives in `kani-model` so `card_sdk::kani_host::call` can be a
//! direct call (no `BACKEND` fn-ptr). Harness-only helpers stay here.

pub use kani_model::world::*;

pub fn arr4_eq(a: [i32; 4], b: [i32; 4]) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] == b[3]
}

pub fn arr8b_eq(a: [bool; 8], b: [bool; 8]) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] == b[3]
        && a[4] == b[4] && a[5] == b[5] && a[6] == b[6] && a[7] == b[7]
}

pub fn arr4b_eq(a: [bool; 4], b: [bool; 4]) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2] && a[3] == b[3]
}

