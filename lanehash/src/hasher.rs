//! `core::hash::Hasher` / `BuildHasher`. Output differs from the one-shot function and
//! may change between releases.
//!
//! Small integer writes are packed into a 128-bit sponge and cost one fold per 128 bits
//! (a `u64` key: one multiply in total, spent in `finish`). Byte writes of up to 16 bytes
//! are one fold, 17–64 bytes the short path keyed by the accumulator, longer writes the
//! one-shot hash folded in. Every fold has a secret in both operands: an unkeyed operand
//! can be set to 0 or to all ones by the input, and either value erases the accumulator,
//! seed included. Integer writes and byte writes do not commute with themselves, but a
//! pending integer may be folded after a later byte write, as in foldhash.
use super::short::{short64_mid, words16, KS, ROT};
use super::spec::{fold, keys};
use core::hash::{BuildHasher, Hasher};

#[derive(Clone)]
pub struct LaneHasher {
    acc: u64,
    /// Integer writes not yet folded, packed from bit 0.
    sponge: u128,
    /// Bits of `sponge` in use.
    fill: u32,
    ka: u64,
    kb: u64,
}

impl LaneHasher {
    /// A hasher for `seed`, as `FixedState::new(seed).build_hasher()`.
    #[inline(always)]
    pub fn new(seed: u64) -> Self {
        FixedState::new(seed).build_hasher()
    }
    #[inline(always)]
    const fn with_keys(acc: u64, ka: u64, kb: u64) -> Self {
        LaneHasher { acc, sponge: 0, fill: 0, ka, kb }
    }
    #[inline(always)]
    fn absorb_sponge(&self) -> u64 {
        fold(self.sponge as u64 ^ self.acc, (self.sponge >> 64) as u64 ^ self.ka)
    }
    #[inline(always)]
    fn write_num(&mut self, x: u64, bits: u32) {
        if self.fill + bits > 128 {
            self.acc = self.absorb_sponge();
            self.sponge = x as u128;
            self.fill = bits;
        } else {
            self.sponge |= (x as u128) << self.fill;
            self.fill += bits;
        }
    }
}

/// A write of over 16 bytes: the new accumulator. Out of line so the inlined `write`
/// stays small in a map's probe loop, and by value so the hasher itself stays in
/// registers: behind a `&mut self` call its sponge fill is no longer a compile-time
/// constant and every later write and `finish` goes through memory and branches.
#[inline(never)]
fn write_long(bytes: &[u8], acc: u64, ka: u64, kb: u64) -> u64 {
    let len = bytes.len();
    if len <= super::spec::SHORT_MAX {
        // the short path, its first-operand secret mixed with the accumulator
        short64_mid(bytes, ka ^ acc, kb)
    } else {
        let h = super::dispatch::hash64_keyed(bytes, ka, kb, kb ^ KS[1]);
        fold(h ^ acc, kb ^ len as u64)
    }
}

impl Hasher for LaneHasher {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        let len = bytes.len();
        if len > 16 {
            self.acc = write_long(bytes, self.acc, self.ka, self.kb);
            return;
        }
        let (w0, w1) = words16(bytes);
        self.acc = fold(w0 ^ self.acc, w1 ^ self.kb ^ len as u64);
    }
    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        self.write_num(i as u64, 8);
    }
    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        self.write_num(i as u64, 16);
    }
    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        self.write_num(i as u64, 32);
    }
    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.write_num(i, 64);
    }
    #[inline(always)]
    fn write_u128(&mut self, i: u128) {
        self.write_num(i as u64, 64);
        self.write_num((i >> 64) as u64, 64);
    }
    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        self.write_num(i as u64, usize::BITS);
    }
    #[inline(always)]
    fn finish(&self) -> u64 {
        if self.fill != 0 {
            self.absorb_sponge()
        } else {
            self.acc
        }
    }
}

/// `BuildHasher` with a fixed seed (deterministic maps).
#[derive(Clone, Copy, Debug)]
pub struct FixedState {
    acc: u64,
    ka: u64,
    kb: u64,
}
impl FixedState {
    pub const fn new(seed: u64) -> Self {
        let (ka, kb, _) = keys(seed);
        // a third secret for the accumulator: no public relation to `ka` or `kb`
        FixedState { acc: ka.rotate_left(ROT[2]) ^ kb.rotate_left(ROT[1]), ka, kb }
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
        LaneHasher::with_keys(self.acc, self.ka, self.kb)
    }
}

/// `BuildHasher` with a per-process random seed and a different accumulator start for
/// every instance, so two maps do not share bucket order.
#[cfg(feature = "std")]
#[derive(Clone, Copy, Debug)]
pub struct RandomState(FixedState);
#[cfg(feature = "std")]
impl RandomState {
    pub fn new() -> Self {
        use core::sync::atomic::{AtomicUsize, Ordering};
        use std::hash::BuildHasher as _;
        use std::sync::OnceLock;
        static SEED: OnceLock<FixedState> = OnceLock::new();
        static INSTANCE: AtomicUsize = AtomicUsize::new(0);
        let base = *SEED.get_or_init(|| FixedState::new(std::collections::hash_map::RandomState::new().build_hasher().finish()));
        let n = INSTANCE.fetch_add(1, Ordering::Relaxed) as u64;
        RandomState(FixedState { acc: fold(n ^ base.acc, base.kb.rotate_left(ROT[3]) | 1), ..base })
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
        self.0.build_hasher()
    }
}
