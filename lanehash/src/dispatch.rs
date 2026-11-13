//! Backend selection for the long path (one detection, cached pointer) and the public
//! one-shot functions.
use super::short;
use super::spec::{self, finish_lanes, Words};
use core::sync::atomic::{AtomicUsize, Ordering};

/// One-shot 64-bit hash of `n >= 65` bytes with `ks` from `spec::ks`.
pub type Hash64 = unsafe fn(*const u8, usize, u64) -> u64;
pub type Hash128 = unsafe fn(*const u8, usize, u64) -> u128;
pub type Absorb = unsafe fn(&mut Words, *const u8, usize);
pub type Finish = unsafe fn(&Words, *const u8, usize, *const u8, usize, u64) -> u128;

/// Kernels return values in registers: an array returned through memory costs a 512-bit
/// stack store that the test CPU does not forward to the finaliser's loads.
#[derive(Clone, Copy)]
pub struct Backend {
    pub name: &'static str,
    pub hash64: Hash64,
    pub hash128: Hash128,
    /// Streaming: absorb whole leading blocks.
    pub absorb: Absorb,
    /// Streaming: the 128-bit hash after the remaining stripes and the closing stripe.
    pub finish: Finish,
}

unsafe fn hash64_spec(p: *const u8, n: usize, ks: u64) -> u64 {
    finish_lanes::<false>(&spec::lanes(core::slice::from_raw_parts(p, n), ks), ks, n) as u64
}
unsafe fn hash128_spec(p: *const u8, n: usize, ks: u64) -> u128 {
    finish_lanes::<true>(&spec::lanes(core::slice::from_raw_parts(p, n), ks), ks, n)
}
unsafe fn absorb_spec(w: &mut Words, p: *const u8, nblocks: usize) {
    let mut a = w.to_state();
    spec::absorb(&mut a, core::slice::from_raw_parts(p, nblocks * spec::BLOCK));
    *w = Words::from_state(&a);
}
unsafe fn finish_spec(w: &Words, leading: *const u8, r: usize, closing: *const u8, n: usize, ks: u64) -> u128 {
    let m = spec::finish(&w.to_state(), core::slice::from_raw_parts(leading, r * spec::STRIPE), &*(closing as *const [u8; 64]), n);
    finish_lanes::<true>(&m, ks, n)
}
pub const SPEC: Backend = Backend { name: "spec", hash64: hash64_spec, hash128: hash128_spec, absorb: absorb_spec, finish: finish_spec };

#[allow(unused_macros)] // unused on targets with only the reference backend
macro_rules! backend {
    ($name:ident, $s:literal, $m:path) => {
        pub const $name: Backend = {
            use $m as k;
            Backend { name: $s, hash64: k::hash64, hash128: k::hash128, absorb: k::absorb, finish: k::finish }
        };
    };
}
#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
backend!(SSE2, "sse2", super::x86::sse2);
#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
backend!(AVX2, "avx2", super::x86::avx2);
#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
backend!(AVX512, "avx512", super::x86::avx512);
#[cfg(all(target_arch = "aarch64", not(feature = "force-fallback")))]
backend!(NEON, "neon", super::arm);
#[cfg(all(target_arch = "wasm32", target_feature = "simd128", not(feature = "force-fallback")))]
backend!(SIMD128, "simd128", super::wasm);

/// The AVX-512 kernels need AVX-512F and VL, AVX2 and BMI2; the AVX2 kernels AVX2 and
/// BMI2. `true` when the build enables them or, with `std`, when the CPU reports them.
#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
#[inline(always)]
pub fn has_avx512() -> bool {
    if cfg!(all(target_feature = "avx512f", target_feature = "avx512vl", target_feature = "avx2", target_feature = "bmi2")) {
        return true;
    }
    #[cfg(feature = "std")]
    {
        return std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512vl") && std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("bmi2");
    }
    #[allow(unreachable_code)]
    false
}
#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
#[inline(always)]
pub fn has_avx2() -> bool {
    if cfg!(all(target_feature = "avx2", target_feature = "bmi2")) {
        return true;
    }
    #[cfg(feature = "std")]
    {
        return std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("bmi2");
    }
    #[allow(unreachable_code)]
    false
}

fn select() -> &'static Backend {
    #[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
    {
        if has_avx512() {
            return &AVX512;
        }
        if has_avx2() {
            return &AVX2;
        }
        return &SSE2;
    }
    #[cfg(all(target_arch = "aarch64", not(feature = "force-fallback")))]
    {
        return &NEON;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128", not(feature = "force-fallback")))]
    {
        return &SIMD128;
    }
    #[allow(unreachable_code)]
    &SPEC
}

static CACHE: AtomicUsize = AtomicUsize::new(0);

/// The selected backend. Known at compile time when the build enables the features of
/// the widest kernel (the call is then direct and can inline), detected once otherwise.
#[inline(always)]
pub fn backend() -> &'static Backend {
    #[cfg(all(target_arch = "x86_64", not(feature = "force-fallback"), target_feature = "avx512f", target_feature = "avx512vl", target_feature = "avx2", target_feature = "bmi2"))]
    {
        return &AVX512;
    }
    #[cfg(all(target_arch = "aarch64", not(feature = "force-fallback")))]
    {
        return &NEON;
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128", not(feature = "force-fallback")))]
    {
        return &SIMD128;
    }
    #[allow(unreachable_code)]
    backend_detected()
}
#[inline(always)]
fn backend_detected() -> &'static Backend {
    let p = CACHE.load(Ordering::Relaxed);
    if p != 0 {
        // SAFETY: only ever stores a `&'static Backend`.
        return unsafe { &*(p as *const Backend) };
    }
    let b = select();
    CACHE.store(b as *const Backend as usize, Ordering::Relaxed);
    b
}

/// `hash64` with the secrets of `spec::keys` already computed.
#[inline(always)]
pub fn hash64_keyed(bytes: &[u8], ka: u64, kb: u64, ks: u64) -> u64 {
    let n = bytes.len();
    if n <= spec::SHORT_MAX {
        return short::short64_keyed(bytes, ka, kb);
    }
    // SAFETY: `bytes` is a valid slice of `n >= 65` bytes; the kernels read within it.
    unsafe { (backend().hash64)(bytes.as_ptr(), n, ks) }
}
/// `hash128` with the secrets of `spec::keys` already computed.
#[inline(always)]
pub fn hash128_keyed(bytes: &[u8], ka: u64, kb: u64, ks: u64) -> u128 {
    let n = bytes.len();
    if n <= spec::SHORT_MAX {
        return short::short128_keyed(bytes, ka, kb);
    }
    // SAFETY: as in `hash64_keyed`.
    unsafe { (backend().hash128)(bytes.as_ptr(), n, ks) }
}
/// 64-bit lanehash of `bytes` under `seed`.
#[inline(always)]
pub fn hash64(bytes: &[u8], seed: u64) -> u64 {
    let (ka, kb, ks) = spec::keys(seed);
    hash64_keyed(bytes, ka, kb, ks)
}
/// 128-bit lanehash of `bytes` under `seed`; its low half is [`hash64`].
#[inline(always)]
pub fn hash128(bytes: &[u8], seed: u64) -> u128 {
    let (ka, kb, ks) = spec::keys(seed);
    hash128_keyed(bytes, ka, kb, ks)
}

/// [`hash64`] of an array: with the length a constant, the short classes inline without
/// a call or a length branch. Same value as `hash64(&key[..], seed)`.
#[inline(always)]
pub fn hash64_fixed<const N: usize>(key: &[u8; N], seed: u64) -> u64 {
    let (ka, kb, ks) = spec::keys(seed);
    if N <= spec::SHORT_MAX {
        short::short128_inline::<false>(key, ka, kb) as u64
    } else {
        hash64_keyed(key, ka, kb, ks)
    }
}
/// [`hash128`] of an array, see [`hash64_fixed`].
#[inline(always)]
pub fn hash128_fixed<const N: usize>(key: &[u8; N], seed: u64) -> u128 {
    let (ka, kb, ks) = spec::keys(seed);
    if N <= spec::SHORT_MAX {
        short::short128_inline::<true>(key, ka, kb)
    } else {
        hash128_keyed(key, ka, kb, ks)
    }
}

/// A seed with its secrets computed once, for hashing many inputs under one seed:
/// `Keyed::new(seed).hash64(b) == hash64(b, seed)` without the seed multiply per call.
#[derive(Clone, Copy, Debug)]
pub struct Keyed {
    ka: u64,
    kb: u64,
    ks: u64,
}
impl Keyed {
    pub const fn new(seed: u64) -> Self {
        let (ka, kb, ks) = spec::keys(seed);
        Keyed { ka, kb, ks }
    }
    /// Equal to [`hash64`] under the seed given to `new`.
    #[inline(always)]
    pub fn hash64(&self, bytes: &[u8]) -> u64 {
        hash64_keyed(bytes, self.ka, self.kb, self.ks)
    }
    /// Equal to [`hash128`] under the seed given to `new`.
    #[inline(always)]
    pub fn hash128(&self, bytes: &[u8]) -> u128 {
        hash128_keyed(bytes, self.ka, self.kb, self.ks)
    }
    /// Equal to [`hash64_fixed`] under the seed given to `new`.
    #[inline(always)]
    pub fn hash64_fixed<const N: usize>(&self, key: &[u8; N]) -> u64 {
        if N <= spec::SHORT_MAX {
            short::short128_inline::<false>(key, self.ka, self.kb) as u64
        } else {
            hash64_keyed(key, self.ka, self.kb, self.ks)
        }
    }
}
