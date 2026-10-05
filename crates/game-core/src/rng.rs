//! Seeded, deterministic RNG for match logic.
//!
//! Not compatible with .NET `System.Random`, by design: the port reproduces the
//! game's rules, not its exact numbers. What matters is that the same seed and the
//! same inputs give the same game on server and browser, and that the state is a
//! plain value, so it travels inside every replay snapshot.
//!
//! Algorithm: xoshiro256** seeded through SplitMix64.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut x = seed;
        Self { s: [splitmix64(&mut x), splitmix64(&mut x), splitmix64(&mut x), splitmix64(&mut x)] }
    }

    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Uniform in `0..n` (unbiased). `n == 0` returns 0.
    pub fn below(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        let n = n as u64;
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let v = self.next_u64();
            if v < zone {
                return (v % n) as usize;
            }
        }
    }

    /// One die: `1..=sides`.
    pub fn d(&mut self, sides: i32) -> i32 {
        self.below(sides.max(1) as usize) as i32 + 1
    }

    /// Uniform in `[0, 1)`.
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    /// Fisher-Yates.
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }

    pub fn pick<'a, T>(&mut self, v: &'a [T]) -> Option<&'a T> {
        if v.is_empty() {
            None
        } else {
            Some(&v[self.below(v.len())])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn same_seed_same_stream() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
    }

    #[test]
    fn dice_cover_their_range_evenly() {
        let mut r = Rng::new(7);
        let mut hist = [0u32; 20];
        for _ in 0..200_000 {
            let v = r.d(20);
            assert!((1..=20).contains(&v));
            hist[v as usize - 1] += 1;
        }
        for &h in &hist {
            assert!((9_000..11_000).contains(&h), "{hist:?}");
        }
    }

    #[test]
    fn state_survives_a_json_round_trip() {
        let mut r = Rng::new(9);
        r.next_u64();
        let mut back: Rng = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(r.next_u64(), back.next_u64());
    }
}
