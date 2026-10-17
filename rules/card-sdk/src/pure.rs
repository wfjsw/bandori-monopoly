//! Filter-body purity flag (ABI v74).
//!
//! [`crate::On::Filter`] / [`crate::On::FilterFlag`] / [`crate::On::FilterValue`]
//! bodies must be pure value transforms: no prompts, no `ctx::gain` / place /
//! draw / plan / deal / slot writes. The dispatch wrapper sets this flag for
//! the duration of the body; mutating `ctx` entry points refuse while it is
//! set. Kani has no TLS (see [`crate::native`]), so the check is host-side
//! there.

#[cfg(all(not(kani), not(target_arch = "wasm32")))]
mod imp {
    std::thread_local! {
        static IN_FILTER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// Enter a Filter body. Returns a guard that leaves on drop.
    pub fn enter() -> Guard {
        IN_FILTER.with(|f| f.set(true));
        Guard
    }

    /// Are we inside a Filter body?
    pub fn in_filter() -> bool {
        IN_FILTER.with(|f| f.get())
    }

    pub struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            IN_FILTER.with(|f| f.set(false));
        }
    }
}

/// wasm32 guest: `no_std`, single-threaded per instance -- a plain atomic
/// stands in for TLS. The module is re-instantiated for every run, so the
/// flag cannot leak across runs.
#[cfg(all(not(kani), target_arch = "wasm32"))]
mod imp {
    use core::sync::atomic::{AtomicBool, Ordering};

    static IN_FILTER: AtomicBool = AtomicBool::new(false);

    /// Enter a Filter body. Returns a guard that leaves on drop.
    pub fn enter() -> Guard {
        IN_FILTER.store(true, Ordering::SeqCst);
        Guard
    }

    /// Are we inside a Filter body?
    pub fn in_filter() -> bool {
        IN_FILTER.load(Ordering::SeqCst)
    }

    pub struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            IN_FILTER.store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(kani)]
mod imp {
    pub fn enter() -> Guard {
        Guard
    }
    pub fn in_filter() -> bool {
        false
    }
    pub struct Guard;
}

pub use imp::{enter, in_filter, Guard};

/// Refuse a mutating effect from a Filter body (debug + test builds).
/// Release builds of shipped rules ignore the flag so a stray call cannot
/// brick a live match; `debug_assertions` and the test profile panic.
#[inline]
pub fn refuse_if_filter(what: &str) {
    if in_filter() {
        debug_assert!(
            false,
            "Filter body must be pure: `{what}` is a mutating ctx effect"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_is_clear_outside_a_filter() {
        assert!(!in_filter());
        refuse_if_filter("ctx::gain"); // no-op when clear
    }

    #[test]
    fn flag_is_set_inside_a_filter() {
        {
            let _g = enter();
            assert!(in_filter());
        }
        assert!(!in_filter());
    }
}