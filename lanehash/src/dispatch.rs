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
/// stack store that Zen 4 does not forward to the finaliser's loads.
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

fn select() -> &'static Backend {
    #[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
    {
        #[cfg(feature = "std")]
        {
            if std::is_x86_feature_detected!("avx512f") {
                return &AVX512;
            }
            if std::is_x86_feature_detected!("avx2") {
                return &AVX2;
            }
        }
        #[cfg(not(feature = "std"))]
        {
            if cfg!(target_feature = "avx512f") {
                return &AVX512;
            }
            if cfg!(target_feature = "avx2") {
                return &AVX2;
            }
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

/// The selected backend (detected once).
#[inline(always)]
pub fn backend() -> &'static Backend {
    let p = CACHE.load(Ordering::Relaxed);
    if p != 0 {
        // SAFETY: only ever stores a `&'static Backend`.
        return unsafe { &*(p as *const Backend) };
    }
    let b = select();
    CACHE.store(b as *const Backend as usize, Ordering::Relaxed);
    b
}

/// 64-bit lanehash of `bytes` under `seed`.
#[inline(always)]
pub fn hash64(bytes: &[u8], seed: u64) -> u64 {
    let n = bytes.len();
    if n <= spec::SHORT_MAX {
        return short::short64(bytes, seed);
    }
    // SAFETY: `bytes` is a valid slice of `n >= 65` bytes; the kernels read within it.
    unsafe { (backend().hash64)(bytes.as_ptr(), n, spec::ks(seed)) }
}
/// 128-bit lanehash of `bytes` under `seed`; its low half is [`hash64`].
#[inline(always)]
pub fn hash128(bytes: &[u8], seed: u64) -> u128 {
    let n = bytes.len();
    if n <= spec::SHORT_MAX {
        return short::short128(bytes, seed);
    }
    // SAFETY: as in `hash64`.
    unsafe { (backend().hash128)(bytes.as_ptr(), n, spec::ks(seed)) }
}
