//! `core::hash::Hasher` / `BuildHasher` for maps. Output differs from the one-shot function.
//!
//! Accumulator design (WP5): one folded 64x64->128 multiply per
//! `write`, the secrets precomputed once per `BuildHasher`; short byte strings enter
//! directly (no one-shot call), longer ones through the one-shot lane hash. `finish`
//! returns the accumulator: every write ends in a fold, so both halves are mixed.
use crate::short::{fold, secrets};
use core::hash::{BuildHasher, Hasher};

/// Map hasher: `acc = fold(acc ^ data ^ secret, data' ^ secret')` per write.
/// Holds only the four secrets it uses (48 bytes), so `build_hasher` is cheap.
#[derive(Clone)]
pub struct LaneHasher {
    acc: u64,
    seed: u64,
    k: [u64; 4],
}

#[inline(always)]
fn le64(p: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(p[i..i + 8].try_into().unwrap())
}
#[inline(always)]
fn le32(p: &[u8], i: usize) -> u64 {
    u32::from_le_bytes(p[i..i + 4].try_into().unwrap()) as u64
}

impl LaneHasher {
    #[inline(always)]
    pub fn with_secrets(seed: u64, k: [u64; 8]) -> Self {
        LaneHasher { acc: k[7], seed, k: [k[0], k[1], k[2], k[3]] }
    }
    /// `> 16` bytes: 16-byte folds up to 64 bytes, the one-shot lane hash above.
    /// Out of line so that the inlined `write` stays small enough for `hash_one`
    /// to inline into the map's probe loop.
    #[inline(never)]
    fn write_long(&mut self, bytes: &[u8]) {
        let len = bytes.len();
        let k = &self.k;
        if len <= crate::SHORT_MAX {
            // 16-byte chunks, then the last 16 bytes (overlapping); the length in the last fold
            let mut acc = self.acc;
            let mut i = 0;
            while i + 16 < len {
                acc = fold(le64(bytes, i) ^ k[0] ^ acc, le64(bytes, i + 8) ^ k[1]);
                i += 16;
            }
            self.acc = fold(le64(bytes, len - 16) ^ k[2] ^ acc, le64(bytes, len - 8) ^ k[3] ^ len as u64);
        } else {
            self.acc = fold(crate::hash64(bytes, self.seed) ^ k[2] ^ self.acc, k[3] ^ len as u64);
        }
    }
    /// Two words carrying all of `p` (`p.len() <= 16`), as in the short path.
    #[inline(always)]
    fn words16(p: &[u8]) -> (u64, u64) {
        let len = p.len();
        match len {
            0 => (0, 0),
            1..=3 => ((p[0] as u64) | ((p[len / 2] as u64) << 8) | ((p[len - 1] as u64) << 16), 0),
            4..=8 => (le32(p, 0) | (le32(p, len - 4) << 32), 0),
            _ => (le64(p, 0), le64(p, len - 8)),
        }
    }
}

impl Hasher for LaneHasher {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        let len = bytes.len();
        let k = &self.k;
        if len <= 16 {
            let (a, b) = Self::words16(bytes);
            self.acc = fold(a ^ k[0] ^ self.acc, b ^ k[1] ^ len as u64);
        } else {
            self.write_long(bytes);
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.write_u64(i as u64)
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.write_u64(i as u64)
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64)
    }
    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.acc = fold(i ^ self.acc, self.k[2]);
    }
    #[inline(always)]
    fn write_u128(&mut self, i: u128) {
        self.acc = fold(i as u64 ^ self.k[0] ^ self.acc, (i >> 64) as u64 ^ self.k[1]);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64)
    }
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.acc
    }
}

/// Deterministic seed (secrets precomputed once).
#[derive(Clone, Copy, Debug)]
pub struct FixedState {
    seed: u64,
    k: [u64; 8],
}
impl FixedState {
    #[inline]
    pub fn new(seed: u64) -> Self {
        FixedState { seed, k: secrets(seed) }
    }
}
impl Default for FixedState {
    fn default() -> Self {
        Self::new(0)
    }
}
impl BuildHasher for FixedState {
    type Hasher = LaneHasher;
    #[inline(always)]
    fn build_hasher(&self) -> LaneHasher {
        LaneHasher::with_secrets(self.seed, self.k)
    }
}

/// Per-process random seed (drawn once from std's RandomState).
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug)]
pub struct RandomState(FixedState);
#[cfg(feature = "std")]
impl RandomState {
    pub fn new() -> Self {
        use std::hash::BuildHasher as _;
        use std::sync::OnceLock;
        static SEED: OnceLock<u64> = OnceLock::new();
        RandomState(FixedState::new(*SEED.get_or_init(|| std::collections::hash_map::RandomState::new().build_hasher().finish())))
    }
}
#[cfg(feature = "std")]
impl Default for RandomState {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(feature = "std")]
impl BuildHasher for RandomState {
    type Hasher = LaneHasher;
    #[inline]
    fn build_hasher(&self) -> LaneHasher {
        self.0.build_hasher()
    }
}
