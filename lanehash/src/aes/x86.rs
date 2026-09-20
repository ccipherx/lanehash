//! x86-64 backends: AES-NI (128-bit lanes) and VAES+AVX2 (256-bit registers, two lanes each).
use crate::constants::C;
use crate::aes::lanes::{Blk, Lanes, State};
use core::arch::x86_64::*;

#[derive(Clone, Copy)]
pub struct Xmm(pub __m128i);
#[derive(Clone, Copy)]
pub struct Ymm(pub __m256i);

impl Blk for Xmm {
    #[inline(always)]
    fn from_bytes(b: &[u8; 16]) -> Self {
        Xmm(unsafe { _mm_loadu_si128(b.as_ptr() as *const __m128i) })
    }
    #[inline(always)]
    fn bcast(v: u64) -> Self {
        Xmm(unsafe { _mm_set1_epi64x(v as i64) })
    }
    #[inline(always)]
    fn round(self, k: Self) -> Self {
        Xmm(unsafe { _mm_aesenc_si128(self.0, k.0) })
    }
    #[inline(always)]
    fn xor(self, o: Self) -> Self {
        Xmm(unsafe { _mm_xor_si128(self.0, o.0) })
    }
    #[inline(always)]
    fn to_u128(self) -> u128 {
        let mut o = [0u8; 16];
        unsafe { _mm_storeu_si128(o.as_mut_ptr() as *mut __m128i, self.0) };
        u128::from_le_bytes(o)
    }
}

impl Lanes for Xmm {
    type B = Xmm;
    const W: usize = 1;
    #[inline(always)]
    unsafe fn load(p: *const u8) -> Self {
        Xmm(_mm_loadu_si128(p as *const __m128i))
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
        Xmm(unsafe { _mm_setzero_si128() })
    }
    #[inline(always)]
    unsafe fn diff_partial(x: Self, prev: Self, _p: *const u8, _count: usize) -> (Self, Self) {
        debug_assert!(false);
        (x, prev)
    }
    #[inline(always)]
    fn add(self, o: Self) -> Self {
        Xmm(unsafe { _mm_add_epi64(self.0, o.0) })
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
        unsafe { _mm_storeu_si128(out[0].as_mut_ptr() as *mut __m128i, self.0) }
    }
    #[inline(always)]
    fn from_blocks(inp: &[[u8; 16]]) -> Self {
        Self::from_bytes(&inp[0])
    }
}

impl Lanes for Ymm {
    type B = Xmm;
    const W: usize = 2;
    #[inline(always)]
    unsafe fn prefetch(p: *const u8) {
        _mm_prefetch(p as *const i8, _MM_HINT_T0);
    }
    #[inline(always)]
    unsafe fn load(p: *const u8) -> Self {
        Ymm(_mm256_loadu_si256(p as *const __m256i))
    }
    #[inline(always)]
    fn init(seed: u64, first_lane: usize) -> Self {
        let lo = Xmm::init(seed, first_lane).0;
        let hi = Xmm::init(seed, first_lane + 1).0;
        Ymm(unsafe { _mm256_inserti128_si256(_mm256_castsi128_si256(lo), hi, 1) })
    }
    #[inline(always)]
    fn round(self, k: Self) -> Self {
        Ymm(unsafe { _mm256_aesenc_epi128(self.0, k.0) })
    }
    #[inline(always)]
    fn xor(self, o: Self) -> Self {
        Ymm(unsafe { _mm256_xor_si256(self.0, o.0) })
    }
    #[inline(always)]
    fn splat(b: &[u8; 16]) -> Self {
        Ymm(unsafe { _mm256_broadcastsi128_si256(_mm_loadu_si128(b.as_ptr() as *const __m128i)) })
    }
    #[inline(always)]
    fn zero() -> Self {
        Ymm(unsafe { _mm256_setzero_si256() })
    }
    #[inline(always)]
    unsafe fn diff_partial(x: Self, prev: Self, p: *const u8, count: usize) -> (Self, Self) {
        debug_assert_eq!(count, 1);
        let b = _mm_loadu_si128(p as *const __m128i);
        let key = _mm_xor_si128(b, _mm256_castsi256_si128(prev.0));
        let lo = _mm_aesenc_si128(_mm256_castsi256_si128(x.0), key);
        (Ymm(_mm256_inserti128_si256(x.0, lo, 0)), Ymm(_mm256_inserti128_si256(prev.0, b, 0)))
    }
    #[inline(always)]
    fn add(self, o: Self) -> Self {
        Ymm(unsafe { _mm256_add_epi64(self.0, o.0) })
    }
    #[inline(always)]
    fn fold(self) -> Xmm {
        Xmm(unsafe { _mm_add_epi64(_mm256_castsi256_si128(self.0), _mm256_extracti128_si256(self.0, 1)) })
    }
    #[inline(always)]
    unsafe fn round_partial(self, p: *const u8, count: usize) -> Self {
        debug_assert_eq!(count, 1);
        let b = _mm_loadu_si128(p as *const __m128i);
        let lo = _mm_aesenc_si128(_mm_xor_si128(_mm256_castsi256_si128(self.0), b), b);
        Ymm(_mm256_inserti128_si256(self.0, lo, 0))
    }
    #[inline(always)]
    fn store(self, out: &mut [[u8; 16]]) {
        unsafe {
            _mm_storeu_si128(out[0].as_mut_ptr() as *mut __m128i, _mm256_castsi256_si128(self.0));
            _mm_storeu_si128(out[1].as_mut_ptr() as *mut __m128i, _mm256_extracti128_si256(self.0, 1));
        }
    }
    #[inline(always)]
    fn from_blocks(inp: &[[u8; 16]]) -> Self {
        let lo = Xmm::from_bytes(&inp[0]).0;
        let hi = Xmm::from_bytes(&inp[1]).0;
        Ymm(unsafe { _mm256_inserti128_si256(_mm256_castsi128_si256(lo), hi, 1) })
    }
}

/// # Safety: requires AES-NI + SSE2 and `len > 64` bytes readable at `p`.
#[target_feature(enable = "aes,sse2")]
pub unsafe fn hash128_aesni(p: *const u8, len: usize, seed: u64) -> u128 {
    regimes!(Xmm, 1, p, len, seed)
}

/// # Safety: requires VAES + AVX2 and `len > 64` bytes readable at `p`.
#[target_feature(enable = "vaes,avx2")]
pub unsafe fn hash128_vaes256(p: *const u8, len: usize, seed: u64) -> u128 {
    regimes!(Ymm, 2, p, len, seed)
}

/// Differential-form backend using the 32 EVEX ymm registers (AVX-512VL + VAES):
/// same function, XOR off the dependency chain.
/// # Safety: requires VAES + AVX-512F/VL and `len > 64` bytes readable at `p`.
#[target_feature(enable = "vaes,avx2,avx512f,avx512vl")]
pub unsafe fn hash128_vaesvl(p: *const u8, len: usize, seed: u64) -> u128 {
    if len <= crate::aes::L4_MAX {
        crate::aes::lanes::lanes_hash_l4::<Ymm, 2>(p, len, seed)
    } else if len <= crate::aes::L8_MAX {
        crate::aes::lanes::lanes_hash_diff::<Ymm, 8, 4>(p, len, seed)
    } else if len >= crate::aes::lanes::BULK_SANDWICH_MIN {
        crate::aes::lanes::lanes_hash::<Ymm, 16, 8>(p, len, seed)
    } else {
        crate::aes::lanes::lanes_hash_diff::<Ymm, 16, 8>(p, len, seed)
    }
}

// ---- batched entries: 4 inputs in the L = 4 regime, 2 in the L = 8 regime ----

/// # Safety: AES-NI; every input `65..=256` bytes readable.
#[target_feature(enable = "aes,sse2")]
pub unsafe fn batch4_l4_aesni(ps: &[*const u8; 4], lens: &[usize; 4], seed: u64) -> [u128; 4] {
    crate::aes::lanes::lanes_hash_batch4::<Xmm, 4, 4>(ps, lens, seed)
}
/// # Safety: AES-NI; every input `257..=1024` bytes readable.
#[target_feature(enable = "aes,sse2")]
pub unsafe fn batch2_l8_aesni(ps: &[*const u8; 2], lens: &[usize; 2], seed: u64) -> [u128; 2] {
    crate::aes::lanes::lanes_hash_batch2::<Xmm, 8, 8>(ps, lens, seed)
}
/// # Safety: VAES + AVX2; every input `65..=256` bytes readable.
#[target_feature(enable = "vaes,avx2")]
pub unsafe fn batch4_l4_vaes256(ps: &[*const u8; 4], lens: &[usize; 4], seed: u64) -> [u128; 4] {
    crate::aes::lanes::lanes_hash_batch4::<Ymm, 4, 2>(ps, lens, seed)
}
/// # Safety: VAES + AVX2; every input `257..=1024` bytes readable.
#[target_feature(enable = "vaes,avx2")]
pub unsafe fn batch2_l8_vaes256(ps: &[*const u8; 2], lens: &[usize; 2], seed: u64) -> [u128; 2] {
    crate::aes::lanes::lanes_hash_batch2::<Ymm, 8, 4>(ps, lens, seed)
}
/// # Safety: VAES + AVX-512VL; every input `65..=256` bytes readable.
#[target_feature(enable = "vaes,avx2,avx512f,avx512vl")]
pub unsafe fn batch4_l4_vaesvl(ps: &[*const u8; 4], lens: &[usize; 4], seed: u64) -> [u128; 4] {
    crate::aes::lanes::lanes_hash_batch4::<Ymm, 4, 2>(ps, lens, seed)
}
/// # Safety: VAES + AVX-512VL; every input `257..=1024` bytes readable.
#[target_feature(enable = "vaes,avx2,avx512f,avx512vl")]
pub unsafe fn batch2_l8_vaesvl(ps: &[*const u8; 2], lens: &[usize; 2], seed: u64) -> [u128; 2] {
    crate::aes::lanes::lanes_hash_diff_batch2::<Ymm, 8, 4>(ps, lens, seed)
}

// ---- streaming support: 16-lane state in memory, absorb/finish with each backend ----

/// # Safety: AES-NI; `nblocks*16` bytes readable at `p`; `nblocks` history as in `State::absorb`.
#[target_feature(enable = "aes,sse2")]
pub unsafe fn absorb16_aesni(state: &mut [[u8; 16]; 16], p: *const u8, nblocks: usize) {
    let mut st = State::<Xmm, 16, 16>::from_blocks(state);
    st.absorb(p, nblocks);
    st.store(state);
}
#[target_feature(enable = "aes,sse2")]
pub unsafe fn finish16_aesni(state: &[[u8; 16]; 16], c: *const u8, len: usize) -> u128 {
    State::<Xmm, 16, 16>::from_blocks(state).finish(c, len)
}
#[target_feature(enable = "vaes,avx2")]
pub unsafe fn absorb16_vaes256(state: &mut [[u8; 16]; 16], p: *const u8, nblocks: usize) {
    let mut st = State::<Ymm, 16, 8>::from_blocks(state);
    st.absorb(p, nblocks);
    st.store(state);
}
#[target_feature(enable = "vaes,avx2")]
pub unsafe fn finish16_vaes256(state: &[[u8; 16]; 16], c: *const u8, len: usize) -> u128 {
    State::<Ymm, 16, 8>::from_blocks(state).finish(c, len)
}
