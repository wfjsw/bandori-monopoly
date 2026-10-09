//! Seeded, deterministic RNG for match logic.
//!
//! Not compatible with .NET `System.Random`, by design: the port reproduces the
//! game's rules, not its exact numbers. What matters is that the same seed and the
//! same inputs give the same game on server and browser, and that the state is a
//! plain value, so it travels inside every replay snapshot.
//!
//! Two algorithms live here, selected by the serialized form:
//!
//! * **ChaCha12** ([`Rng::from_seed256`]) -- every **new** match. Keyed by the
//!   256-bit derived seed of the commit-reveal scheme (`docs/FAIRNESS.md`),
//!   streamed through [`rand_chacha::ChaCha12Rng`]. ChaCha12 over ChaCha20:
//!   the construction is the same quarter-round for 12 rounds instead of 20,
//!   no distinguishing attack is known below 7 rounds, and this is a game
//!   entropy source (fairness = "unpredictable while the seed is sealed"),
//!   not a bulk cipher. The reduced round count is measurably cheaper on
//!   wasm32, where the sim and the browser both draw heavily. The portable
//!   `rand_chacha` implementation is deterministic across platforms.
//! * **xoshiro256\*\*** ([`Rng::new`]) -- the legacy stream, kept for records
//!   and saves written before the switch. `Init::Seed` records that carry only
//!   a `u64` seed and every save blob with `"s": [u64; 4]` continue on this
//!   stream so an old replay / an old solo save resumes byte-identically.
//!   Nothing new is ever seeded through it except throwaway search forks
//!   (`bot-core`'s determinizer) and tests that do not care which stream they
//!   get.
//!
//! The JSON encoding is versioned by shape: a legacy document carries
//! `"s": [u64; 4]`, a ChaCha one carries `"c": { ... }`. Both keep the
//! `loaded` test seam. A document with neither (or both) is corrupt.

use std::collections::VecDeque;

use rand_chacha::ChaCha12Rng;
use rand_core::{RngCore, SeedableRng};
use serde::{Deserialize, Serialize};

/// A 256-bit match seed (the derived commit-reveal seed, `docs/FAIRNESS.md`).
/// Serializes as 64 lower-case hex chars; accepts a bare `[u8; 32]` array too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seed256(pub [u8; 32]);

impl Seed256 {
    pub fn to_hex(self) -> String {
        let mut s = String::with_capacity(64);
        for b in &self.0 {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    pub fn from_hex(s: &str) -> Result<Self, String> {
        crate::fair::unhex32(s).map(Seed256)
    }
}

impl Serialize for Seed256 {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Seed256 {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Seed256;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("32 bytes as a hex string")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Seed256, E> {
                Seed256::from_hex(v).map_err(E::custom)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Seed256, A::Error> {
                let mut out = [0u8; 32];
                for (i, slot) in out.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &"32 bytes"))?;
                }
                Ok(Seed256(out))
            }
        }
        d.deserialize_any(V)
    }
}

/// Serialized form of the ChaCha stream: the 256-bit key plus how many 32-bit
/// words of keystream have been consumed. `set_word_pos` restores exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ChaChaSerde {
    /// Key bytes, lower-case hex (64 chars).
    key: String,
    /// 32-bit words of keystream consumed (`ChaChaXRng::get_word_pos`).
    #[serde(default)]
    word_pos: u64,
}

/// Which stream this [`Rng`] draws from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Algo {
    /// Legacy xoshiro256** state. Old saves / old seeded records only.
    Xoshiro { s: [u64; 4] },
    /// ChaCha12 keyed by a 256-bit seed (every new match).
    ChaCha { rng: ChaCha12Rng, key: [u8; 32] },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    algo: Algo,
    /// Loaded dice (test seam): faces [`Rng::d`] returns before it touches the
    /// stream, oldest first. Lives in the RNG so a replayed routine re-reads
    /// the same faces from its snapshot. Never set outside tests.
    loaded: VecDeque<i32>,
}

/// The wire form: exactly one of `s` / `c`, plus the `loaded` seam.
#[derive(Debug, Serialize, Deserialize)]
struct RngRepr {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    s: Option<[u64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    c: Option<ChaChaSerde>,
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    loaded: VecDeque<i32>,
}

impl From<Rng> for RngRepr {
    fn from(r: Rng) -> Self {
        let (s, c) = match &r.algo {
            Algo::Xoshiro { s } => (Some(*s), None),
            Algo::ChaCha { rng, key } => {
                let mut hex = String::with_capacity(64);
                for b in key {
                    hex.push_str(&format!("{b:02x}"));
                }
                (
                    None,
                    Some(ChaChaSerde {
                        key: hex,
                        word_pos: rng.get_word_pos() as u64,
                    }),
                )
            }
        };
        RngRepr {
            s,
            c,
            loaded: r.loaded,
        }
    }
}

fn hex_key(s: &str) -> Result<[u8; 32], String> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return Err(format!("chacha key needs 64 hex chars, got {}", b.len()));
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let hi = hex1(b[i * 2])?;
        let lo = hex1(b[i * 2 + 1])?;
        out[i] = hi << 4 | lo;
    }
    Ok(out)
}

fn hex1(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("bad hex char {:?}", c as char)),
    }
}

impl TryFrom<RngRepr> for Rng {
    type Error = String;

    fn try_from(r: RngRepr) -> Result<Self, String> {
        let algo = match (r.s, r.c) {
            (Some(s), None) => Algo::Xoshiro { s },
            (None, Some(c)) => {
                let key = hex_key(&c.key)?;
                let mut rng = ChaCha12Rng::from_seed(key);
                rng.set_word_pos(c.word_pos as u128);
                Algo::ChaCha { rng, key }
            }
            (None, None) => return Err("rng: neither s nor c present".into()),
            (Some(_), Some(_)) => return Err("rng: both s and c present".into()),
        };
        Ok(Rng {
            algo,
            loaded: r.loaded,
        })
    }
}

impl Serialize for Rng {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        RngRepr::from(self.clone()).serialize(s)
    }
}

impl<'de> Deserialize<'de> for Rng {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        RngRepr::deserialize(d)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    /// Legacy xoshiro256**, seeded through SplitMix64.
    ///
    /// This is the stream every record written before the ChaCha switch runs
    /// on. Do **not** seed a new match through it -- use
    /// [`Rng::from_seed256`] with the derived commit-reveal seed
    /// (`docs/FAIRNESS.md`).
    pub fn new(seed: u64) -> Self {
        let mut x = seed;
        Self {
            algo: Algo::Xoshiro {
                s: [
                    splitmix64(&mut x),
                    splitmix64(&mut x),
                    splitmix64(&mut x),
                    splitmix64(&mut x),
                ],
            },
            loaded: VecDeque::new(),
        }
    }

    /// ChaCha12 keyed by a full 256-bit seed -- every new match
    /// (`docs/FAIRNESS.md`).
    pub fn from_seed256(seed: [u8; 32]) -> Self {
        Self {
            algo: Algo::ChaCha {
                rng: ChaCha12Rng::from_seed(seed),
                key: seed,
            },
            loaded: VecDeque::new(),
        }
    }

    /// The 256-bit key when this is a ChaCha stream (what a fairness check
    /// compares against the derived seed). `None` for the legacy stream.
    pub fn seed256(&self) -> Option<[u8; 32]> {
        match &self.algo {
            Algo::ChaCha { key, .. } => Some(*key),
            Algo::Xoshiro { .. } => None,
        }
    }

    /// Test seam: the next [`Rng::d`] calls return these faces, in order
    /// (each clamped to `1..=sides`), before the stream resumes.
    #[doc(hidden)]
    pub fn load_dice(&mut self, faces: &[i32]) {
        self.loaded.extend(faces);
    }

    /// Loaded faces not yet rolled.
    #[doc(hidden)]
    pub fn loaded_dice(&self) -> usize {
        self.loaded.len()
    }

    pub fn next_u64(&mut self) -> u64 {
        match &mut self.algo {
            Algo::Xoshiro { s } => {
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
            Algo::ChaCha { rng, .. } => rng.next_u64(),
        }
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
        if let Some(face) = self.loaded.pop_front() {
            return face.clamp(1, sides.max(1));
        }
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
    fn chacha_same_seed_same_stream() {
        let (mut a, mut b) = (Rng::from_seed256([7; 32]), Rng::from_seed256([7; 32]));
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(
            Rng::from_seed256([7; 32]).next_u64(),
            Rng::from_seed256([8; 32]).next_u64()
        );
    }

    #[test]
    fn dice_cover_their_range_evenly() {
        for mut r in [Rng::new(7), Rng::from_seed256([7; 32])] {
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
    }

    #[test]
    fn state_survives_a_json_round_trip() {
        let mut r = Rng::new(9);
        r.next_u64();
        let mut back: Rng = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(r.next_u64(), back.next_u64());
    }

    #[test]
    fn chacha_state_survives_a_json_round_trip() {
        let mut r = Rng::from_seed256([9; 32]);
        for _ in 0..5 {
            r.next_u64();
        }
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"c\""), "{s}");
        let mut back: Rng = serde_json::from_str(&s).unwrap();
        assert_eq!(back.seed256(), Some([9; 32]));
        for _ in 0..20 {
            assert_eq!(r.next_u64(), back.next_u64());
        }
    }

    #[test]
    fn legacy_save_json_still_deserializes() {
        // The exact shape `Match::save` wrote before the ChaCha switch.
        let mut r: Rng = serde_json::from_str(r#"{"s":[1,2,3,4]}"#).expect("legacy rng loads");
        assert!(r.seed256().is_none());
        let v = r.next_u64();
        let v2 = r.next_u64();
        let mut again: Rng = serde_json::from_str(r#"{"s":[1,2,3,4]}"#).unwrap();
        assert_eq!(again.next_u64(), v);
        assert_eq!(again.next_u64(), v2);
        // And it differs from a freshly seeded xoshiro of the same words.
        let mut fresh = Rng::new(0);
        assert_ne!(fresh.next_u64(), v);
    }

    #[test]
    fn loaded_dice_survive_both_streams() {
        for mut r in [Rng::new(1), Rng::from_seed256([1; 32])] {
            r.load_dice(&[3, 5]);
            assert_eq!(r.d(6), 3);
            assert_eq!(r.d(6), 5);
            assert!((1..=6).contains(&r.d(6)));
        }
    }
}