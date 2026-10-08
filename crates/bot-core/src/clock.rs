//! Wall clock for the anytime search (`docs/BOT.md` B6).
//!
//! `std::time::Instant` is **unsupported** on `wasm32-unknown-unknown` (std's
//! `sys::time::unsupported` panics at `now()`), and the browser bot worker runs
//! this crate inside wasm32. [`Instant`] is a drop-in stand-in: `Instant`-shaped
//! on every target, backed by `performance.now()`-grade milliseconds once the
//! glue installs a source ([`set_clock`]) or, on wasm32, by `js_sys::Date::now`.
//!
//! Resolution is milliseconds -- fine for 200 ms–3 s budgets. The origin is
//! arbitrary; only differences matter.

use std::sync::OnceLock;
use std::time::Duration;

/// Milliseconds since an arbitrary origin. Monotonic within a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(u64);

type ClockFn = fn() -> u64;

static SOURCE: OnceLock<ClockFn> = OnceLock::new();

/// Install a clock source (milliseconds). The browser glue calls this at init
/// with `performance.now`; the first call wins. No-op when one is already set,
/// so tests can pin a deterministic clock before the search runs.
pub fn set_clock(f: ClockFn) {
    let _ = SOURCE.set(f);
}

/// Milliseconds since an arbitrary origin, from the installed source or the
/// platform default.
pub fn now_ms() -> u64 {
    if let Some(f) = SOURCE.get() {
        return f();
    }
    default_now_ms()
}

#[cfg(target_arch = "wasm32")]
fn default_now_ms() -> u64 {
    // js-sys is a target-specific dependency; `Date.now` is ~1 ms resolution,
    // which is enough for an anytime budget.
    js_sys::Date::now() as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn default_now_ms() -> u64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    let start = START.get_or_init(std::time::Instant::now);
    start.elapsed().as_millis() as u64
}

impl Instant {
    pub fn now() -> Self {
        Self(now_ms())
    }

    /// Time since this mark.
    pub fn elapsed(&self) -> Duration {
        Duration::from_millis(now_ms().saturating_sub(self.0))
    }

    /// Time from `earlier` to this mark (0 when `earlier` is in the future).
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }

    /// Milliseconds since the clock origin (for wire reports).
    pub fn as_ms(&self) -> u64 {
        self.0
    }
}

impl std::ops::Add<Duration> for Instant {
    type Output = Instant;
    fn add(self, d: Duration) -> Instant {
        Instant(self.0.saturating_add(d.as_millis() as u64))
    }
}

impl std::ops::Sub<Duration> for Instant {
    type Output = Instant;
    fn sub(self, d: Duration) -> Instant {
        Instant(self.0.saturating_sub(d.as_millis() as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_and_add_are_consistent() {
        let t0 = Instant::now();
        let t1 = t0 + Duration::from_millis(50);
        // A clock source may not be installed in tests; `t1` is a pure offset.
        assert_eq!(t1.as_ms(), t0.as_ms().saturating_add(50));
        let d = t1.duration_since(t0);
        assert_eq!(d, Duration::from_millis(50));
    }

    #[test]
    fn now_advances() {
        let a = Instant::now();
        let b = Instant::now();
        assert!(b >= a);
    }
}