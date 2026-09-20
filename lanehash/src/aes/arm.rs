//! aarch64 backend: NEON + Armv8 AES, 128-bit lanes. `vaeseq_u8(x, k)` is
//! `SR(SB(x ^ k))` and `vaesmcq_u8` is `MC`, so the spec round (x86 `AESENC`) is
//! `vaesmcq_u8(vaeseq_u8(x, 0)) ^ k`; LLVM folds `vaeseq_u8(a ^ b, 0)` into
//! `vaeseq_u8(a, b)`, so absorb is `AESE t, b; AESMC; EOR t, b` and the differential
//! chain `AESE x, d; AESMC` with `d = b_prev ^ b` off the chain.

use crate::constants::C;
use crate::aes::lanes::{Blk, Lanes, State};
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
        Neon(unsafe {
            vreinterpretq_u8_u64(vaddq_u64(
                vreinterpretq_u64_u8(self.0),
                vreinterpretq_u64_u8(o.0),
            ))
        })
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

#[inline(always)]
unsafe fn step_shifted<const L: usize>(
    u: &mut [uint8x16_t; L],
    kp: &mut [uint8x16_t; L],
    p: *const u8,
) {
    for i in 0..L {
        let b = vld1q_u8(p.add(16 * i));
        u[i] = vaesmcq_u8(vaeseq_u8(u[i], veorq_u8(kp[i], b)));
        kp[i] = b;
    }
}

#[inline(always)]
unsafe fn absorb_shifted<const L: usize>(
    t: &mut [Neon; L],
    mut p: *const u8,
    nblocks: usize,
) {
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

/// L = 8 uses the shifted chain; L = 16 uses the regular form to avoid register pressure.
///
/// # Safety
/// Requires NEON + AES and `len > 64` readable bytes at `p`.
#[target_feature(enable = "aes,neon")]
pub unsafe fn hash128_neon(p: *const u8, len: usize, seed: u64) -> u128 {
    if len <= crate::aes::L4_MAX {
        crate::aes::lanes::lanes_hash_l4::<Neon, 4>(p, len, seed)
    } else if len <= crate::aes::L8_MAX {
        let mut st = State::<Neon, 8, 8>::new(seed);
        absorb_shifted(&mut st.t, p, (len + 15) / 16 - 8);
        st.finish(p.add(len - 128), len)
    } else {
        crate::aes::lanes::lanes_hash::<Neon, 16, 16>(p, len, seed)
    }
}

/// # Safety
/// Requires NEON + AES; every input is `65..=256` bytes.
#[target_feature(enable = "aes,neon")]
pub unsafe fn batch4_l4_neon(ps: &[*const u8; 4], lens: &[usize; 4], seed: u64) -> [u128; 4] {
    crate::aes::lanes::lanes_hash_batch4::<Neon, 4, 4>(ps, lens, seed)
}

/// # Safety
/// Requires NEON + AES; every input is `257..=1024` bytes.
#[target_feature(enable = "aes,neon")]
pub unsafe fn batch2_l8_neon(ps: &[*const u8; 2], lens: &[usize; 2], seed: u64) -> [u128; 2] {
    crate::aes::lanes::lanes_hash_batch2::<Neon, 8, 8>(ps, lens, seed)
}

/// # Safety
/// Requires NEON + AES; `nblocks * 16` bytes readable at `p`.
#[target_feature(enable = "aes,neon")]
pub unsafe fn absorb16_neon(state: &mut [[u8; 16]; 16], p: *const u8, nblocks: usize) {
    let mut st = State::<Neon, 16, 16>::from_blocks(state);
    st.absorb(p, nblocks);
    st.store(state);
}

#[target_feature(enable = "aes,neon")]
pub unsafe fn finish16_neon(
    state: &[[u8; 16]; 16],
    c: *const u8,
    len: usize,
) -> u128 {
    State::<Neon, 16, 16>::from_blocks(state).finish(c, len)
}