//! aarch64 backend: NEON + Armv8 AES (128-bit lanes).
//!
//! Round mapping: `vaeseq_u8(x, k)` is `SR(SB(x ^ k))` and
//! `vaesmcq_u8` is `MC`, so the spec round `R(x, k) = MC(SR(SB(x))) ^ k` (x86
//! `AESENC`) is `vaesmcq_u8(vaeseq_u8(x, 0)) ^ k`. LLVM folds `vaeseq_u8(a ^ b, 0)`
//! into `vaeseq_u8(a, b)`, so the sandwich absorb `t = R(t ^ b, b)` becomes
//! `AESE t, b; AESMC; EOR t, b` (one fused pair plus the trailing XOR), and the
//! differential chain `x = R(x, d)` becomes `AESE x, d; AESMC` (one fused pair,
//! the XOR `d = b_prev ^ b` off the chain: the "shifted" form).
use crate::constants::C;
use crate::lanes::{Blk, Lanes, State};
use core::arch::aarch64::*;

#[derive(Clone, Copy)]
pub struct Neon(pub uint8x16_t);

impl Blk for Neon {
    #[inline(always)]
    fn from_bytes(b: &[u8; 16]) -> Self {
        Neon(unsafe { vld1q_u8(b.as_ptr()) })
    }
    #[inline(always)]
    fn bcast(v: u64) -> Self {
        Neon(unsafe { vreinterpretq_u8_u64(vdupq_n_u64(v)) })
    }
    #[inline(always)]
    fn round(self, k: Self) -> Self {
        Neon(unsafe { veorq_u8(vaesmcq_u8(vaeseq_u8(self.0, vdupq_n_u8(0))), k.0) })
    }
    #[inline(always)]
    fn xor(self, o: Self) -> Self {
        Neon(unsafe { veorq_u8(self.0, o.0) })
    }
    #[inline(always)]
    fn to_u128(self) -> u128 {
        let mut o = [0u8; 16];
        unsafe { vst1q_u8(o.as_mut_ptr(), self.0) };
        u128::from_le_bytes(o)
    }
}

impl Lanes for Neon {
    type B = Neon;
    const W: usize = 1;
    #[inline(always)]
    unsafe fn load(p: *const u8) -> Self {
        Neon(vld1q_u8(p))
    }
    #[inline(always)]
    fn init(seed: u64, first_lane: usize) -> Self {
        <Self as Blk>::xor(Self::bcast(seed), Self::from_bytes(&C[first_lane]))
    }
    #[inline(always)]
    fn round(self, k: Self) -> Self {
        Blk::round(self, k)
    }
    #[inline(always)]
    fn xor(self, o: Self) -> Self {
        <Self as Blk>::xor(self, o)
    }
    #[inline(always)]
    fn splat(b: &[u8; 16]) -> Self {
        <Self as Blk>::from_bytes(b)
    }
    #[inline(always)]
    fn zero() -> Self {
        Neon(unsafe { vdupq_n_u8(0) })
    }
    #[inline(always)]
    unsafe fn diff_partial(x: Self, prev: Self, _p: *const u8, _count: usize) -> (Self, Self) {
        debug_assert!(false);
        (x, prev)
    }
    #[inline(always)]
    fn add(self, o: Self) -> Self {
        Neon(unsafe { vreinterpretq_u8_u64(vaddq_u64(vreinterpretq_u64_u8(self.0), vreinterpretq_u64_u8(o.0))) })
    }
    #[inline(always)]
    fn fold(self) -> Self {
        self
    }
    #[inline(always)]
    unsafe fn round_partial(self, _p: *const u8, _count: usize) -> Self {
        debug_assert!(false);
        self
    }
    #[inline(always)]
    fn store(self, out: &mut [[u8; 16]]) {
        unsafe { vst1q_u8(out[0].as_mut_ptr(), self.0) }
    }
    #[inline(always)]
    fn from_blocks(inp: &[[u8; 16]]) -> Self {
        Self::from_bytes(&inp[0])
    }
}

/// One step of `absorb_shifted`: `u = MCSRSB(u ^ kp ^ b); kp = b` for `L` blocks at `p`.
#[inline(always)]
unsafe fn step_shifted<const L: usize>(u: &mut [uint8x16_t; L], kp: &mut [uint8x16_t; L], p: *const u8) {
    for i in 0..L {
        let b = vld1q_u8(p.add(16 * i));
        u[i] = vaesmcq_u8(vaeseq_u8(u[i], veorq_u8(kp[i], b)));
        kp[i] = b;
    }
}

/// `State::absorb` in the shifted state described above: `u` is the
/// lane before its pending key XOR (`t = u ^ kp`), so `t = R(t ^ b, b)` is
/// `u = AESMC(AESE(u, kp ^ b)); kp = b`: one fused pair on the chain, the XOR
/// `kp ^ b = b_{k-1} ^ b_k` off it (the differential chain). The
/// generic `lanes_hash_diff` would not compile to this: LLVM folds
/// `AESE(x ^ k, 0)` into `AESE(x, k)` only within a basic block, not across
/// the loop's phi. Two steps per iteration make `kp = b` a register rename.
#[inline(always)]
unsafe fn absorb_shifted<const L: usize>(t: &mut [Neon; L], mut p: *const u8, nblocks: usize) {
    let mut u = [vdupq_n_u8(0); L];
    let mut kp = [vdupq_n_u8(0); L];
    for i in 0..L {
        u[i] = t[i].0;
    }
    let full = nblocks / L;
    for _ in 0..full / 2 {
        step_shifted(&mut u, &mut kp, p);
        step_shifted(&mut u, &mut kp, p.add(16 * L));
        p = p.add(32 * L);
    }
    if full % 2 == 1 {
        step_shifted(&mut u, &mut kp, p);
        p = p.add(16 * L);
    }
    let r = nblocks % L;
    for i in 0..L {
        if i < r {
            let b = vld1q_u8(p.add(16 * i));
            u[i] = vaesmcq_u8(vaeseq_u8(u[i], veorq_u8(kp[i], b)));
            kp[i] = b;
        }
    }
    for i in 0..L {
        t[i] = Neon(veorq_u8(u[i], kp[i]));
    }
}

/// L = 8 runs the shifted chain (8 states + 2 x 8 blocks in registers). L = 16
/// keeps the sandwich form: its 16 chains already cover the pair + EOR latency,
/// and the shifted form would need 48 registers.
/// # Safety: requires NEON + AES and `len > 64` bytes readable at `p`.
#[target_feature(enable = "aes,neon")]
pub unsafe fn hash128_neon(p: *const u8, len: usize, seed: u64) -> u128 {
    if len <= crate::L4_MAX {
        crate::lanes::lanes_hash_l4::<Neon, 4>(p, len, seed)
    } else if len <= crate::L8_MAX {
        let mut st = State::<Neon, 8, 8>::new(seed);
        absorb_shifted(&mut st.t, p, (len + 15) / 16 - 8);
        st.finish(p.add(len - 128), len)
    } else {
        crate::lanes::lanes_hash::<Neon, 16, 16>(p, len, seed)
    }
}

/// # Safety: NEON + AES; every input `65..=256` bytes readable.
#[target_feature(enable = "aes,neon")]
pub unsafe fn batch4_l4_neon(ps: &[*const u8; 4], lens: &[usize; 4], seed: u64) -> [u128; 4] {
    crate::lanes::lanes_hash_batch4::<Neon, 4, 4>(ps, lens, seed)
}
/// # Safety: NEON + AES; every input `257..=1024` bytes readable.
#[target_feature(enable = "aes,neon")]
pub unsafe fn batch2_l8_neon(ps: &[*const u8; 2], lens: &[usize; 2], seed: u64) -> [u128; 2] {
    crate::lanes::lanes_hash_batch2::<Neon, 8, 8>(ps, lens, seed)
}

/// # Safety: NEON + AES; `nblocks*16` bytes readable at `p`; `nblocks` history as in `State::absorb`.
#[target_feature(enable = "aes,neon")]
pub unsafe fn absorb16_neon(state: &mut [[u8; 16]; 16], p: *const u8, nblocks: usize) {
    let mut st = State::<Neon, 16, 16>::from_blocks(state);
    st.absorb(p, nblocks);
    st.store(state);
}
#[target_feature(enable = "aes,neon")]
pub unsafe fn finish16_neon(state: &[[u8; 16]; 16], c: *const u8, len: usize) -> u128 {
    State::<Neon, 16, 16>::from_blocks(state).finish(c, len)
}
