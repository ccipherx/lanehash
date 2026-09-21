//! `core::hash::Hasher` / `BuildHasher`. Output differs from the one-shot function:
//! writes of up to 16 bytes are one fold with the short-path secrets, 17–64 bytes the
//! short path with the accumulator as seed, longer writes the one-shot hash.
use super::short::{KF, KS};
use super::spec::fold;
use core::hash::{BuildHasher, Hasher};

#[derive(Clone)]
pub struct LaneHasher {
    acc: u64,
    seed: u64,
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
    pub fn new(seed: u64) -> Self {
        LaneHasher { acc: seed ^ KF[0], seed }
    }
    #[inline(never)]
    fn write_long(&mut self, bytes: &[u8]) {
        let len = bytes.len();
        if len <= super::spec::SHORT_MAX {
            // the short path, the accumulator as its seed
            self.acc = if len <= 32 { super::short::short64_le32(bytes, self.acc) } else { super::short::short64_le64(bytes, self.acc) };
        } else {
            self.acc = fold(super::hash64(bytes, self.seed) ^ KS[2] ^ self.acc, KS[3] ^ len as u64);
        }
    }
}

impl Hasher for LaneHasher {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        let len = bytes.len();
        if len > 16 {
            return self.write_long(bytes);
        }
        let (w0, w1) = match len {
            0 => (0, 0),
            1..=3 => ((bytes[0] as u64) | ((bytes[len / 2] as u64) << 8) | ((bytes[len - 1] as u64) << 16), 0),
            4..=8 => (le32(bytes, 0) | (le32(bytes, len - 4) << 32), 0),
            _ => (le64(bytes, 0), le64(bytes, len - 8)),
        };
        self.acc = fold(w0 ^ KS[0] ^ self.acc, w1 ^ KS[1] ^ len as u64);
    }
    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        self.acc = fold(i as u64 ^ KS[0] ^ self.acc, KS[1] ^ 1);
    }
    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        self.acc = fold(i as u64 ^ KS[0] ^ self.acc, KS[1] ^ 2);
    }
    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        self.acc = fold(i as u64 ^ KS[0] ^ self.acc, KS[1] ^ 4);
    }
    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.acc = fold(i ^ KS[0] ^ self.acc, KS[1] ^ 8);
    }
    #[inline(always)]
    fn write_u128(&mut self, i: u128) {
        self.acc = fold(i as u64 ^ KS[0] ^ self.acc, (i >> 64) as u64 ^ KS[1] ^ 16);
    }
    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.acc
    }
}

/// `BuildHasher` with a fixed seed (deterministic maps).
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedState {
    seed: u64,
}
impl FixedState {
    pub const fn new(seed: u64) -> Self {
        FixedState { seed }
    }
}
impl BuildHasher for FixedState {
    type Hasher = LaneHasher;
    #[inline(always)]
    fn build_hasher(&self) -> LaneHasher {
        LaneHasher::new(self.seed)
    }
}

/// `BuildHasher` with a per-process random seed.
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug)]
pub struct RandomState {
    seed: u64,
}
#[cfg(feature = "std")]
impl RandomState {
    pub fn new() -> Self {
        use std::hash::BuildHasher as _;
        use std::sync::OnceLock;
        static SEED: OnceLock<u64> = OnceLock::new();
        RandomState { seed: *SEED.get_or_init(|| std::collections::hash_map::RandomState::new().build_hasher().finish()) }
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
    #[inline(always)]
    fn build_hasher(&self) -> LaneHasher {
        LaneHasher::new(self.seed)
    }
}
