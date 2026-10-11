//! Seeded property runner: `forall` plus a tiny deterministic RNG.
//!
//! SplitMix64 -- tiny, deterministic, no external crate. Same generator as
//! `crates/game-rules/tests/fuzz/rng.rs`, lifted into the generic kit so a
//! per-card contract and a fuzz soak share one stream shape.

/// SplitMix64. Deterministic across platforms; never allows the zero fixed
/// point.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Never allow the zero fixed point.
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (`n == 0` -> 0).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo + 1) as u64) as i32
    }

    /// True with probability `pct` / 100.
    pub fn chance(&mut self, pct: u32) -> bool {
        self.below(100) < pct as u64
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> Option<&'a T> {
        if xs.is_empty() {
            None
        } else {
            Some(&xs[self.below(xs.len() as u64) as usize])
        }
    }

    pub fn pick_idx(&mut self, n: usize) -> usize {
        self.below(n as u64) as usize
    }

    /// Fisher-Yates, in place.
    pub fn shuffle<T>(&mut self, xs: &mut [T]) {
        for i in (1..xs.len()).rev() {
            let j = self.below((i + 1) as u64) as usize;
            xs.swap(i, j);
        }
    }
}

/// Run `f` for `i in 0..n`, each with a fresh stream derived from `seed` and
/// the case index. A failure reports the case index **and** the seed so it
/// can be replayed:
///
/// ```text
/// forall case 3/20 (seed 0x2a…): <the error>
/// ```
///
/// The `check` body should set up one randomized pre-state, drive one
/// transition, and assert the contract (including [`crate::snap::frame`]).
/// The arranged pre-state is never itself checked -- only the transition.
pub fn forall(
    n: u32,
    seed: u64,
    mut f: impl FnMut(&mut Rng, u32) -> Result<(), String>,
) -> Result<(), String> {
    for i in 0..n {
        // A fresh stream per case: dropping case `i` must not depend on how
        // much randomness case `i - 1` consumed. Mix the index into the seed.
        let case_seed = seed
            .wrapping_add(i as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut rng = Rng::new(case_seed);
        if let Err(e) = f(&mut rng, i) {
            return Err(format!(
                "forall case {i}/{n} (seed {seed:#x}): {e}"
            ));
        }
    }
    Ok(())
}