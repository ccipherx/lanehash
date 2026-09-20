//! Runtime backend selection (cached function pointers).
use crate::aes::short::short128_with;
use core::sync::atomic::{AtomicUsize, Ordering};

pub type OneShot = unsafe fn(*const u8, usize, u64) -> u128;
pub type Absorb16 = unsafe fn(&mut [[u8; 16]; 16], *const u8, usize);
pub type Finish16 = unsafe fn(&[[u8; 16]; 16], *const u8, usize) -> u128;
pub type Batch4 = unsafe fn(&[*const u8; 4], &[usize; 4], u64) -> [u128; 4];
pub type Batch2 = unsafe fn(&[*const u8; 2], &[usize; 2], u64) -> [u128; 2];

#[derive(Clone, Copy)]
pub struct Backend {
    pub name: &'static str,
    pub one_shot: OneShot,
    pub absorb16: Absorb16,
    pub finish16: Finish16,
    /// Four inputs of 65..=256 bytes at once.
    pub batch4_l4: Batch4,
    /// Two inputs of 257..=1024 bytes at once.
    pub batch2_l8: Batch2,
}

pub const SPEC: Backend = Backend {
    name: "spec",
    one_shot: crate::aes::spec::hash128_lanes,
    absorb16: crate::aes::stream::absorb16_spec,
    finish16: crate::aes::stream::finish16_spec,
    batch4_l4: crate::aes::spec::batch4_l4_spec,
    batch2_l8: crate::aes::spec::batch2_l8_spec,
};

pub const SOFT: Backend = Backend {
    name: "soft",
    one_shot: crate::aes::soft::hash128_lanes,
    absorb16: crate::aes::soft::absorb16_soft,
    finish16: crate::aes::soft::finish16_soft,
    batch4_l4: crate::aes::soft::batch4_l4_soft,
    batch2_l8: crate::aes::soft::batch2_l8_soft,
};

#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
pub const AESNI: Backend = Backend {
    name: "aesni",
    one_shot: crate::aes::x86::hash128_aesni,
    absorb16: crate::aes::x86::absorb16_aesni,
    finish16: crate::aes::x86::finish16_aesni,
    batch4_l4: crate::aes::x86::batch4_l4_aesni,
    batch2_l8: crate::aes::x86::batch2_l8_aesni,
};
#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
pub const VAES256: Backend = Backend {
    name: "vaes256",
    one_shot: crate::aes::x86::hash128_vaes256,
    absorb16: crate::aes::x86::absorb16_vaes256,
    finish16: crate::aes::x86::finish16_vaes256,
    batch4_l4: crate::aes::x86::batch4_l4_vaes256,
    batch2_l8: crate::aes::x86::batch2_l8_vaes256,
};

#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
pub const VAESVL: Backend = Backend {
    name: "vaesvl",
    one_shot: crate::aes::x86::hash128_vaesvl,
    absorb16: crate::aes::x86::absorb16_vaes256,
    finish16: crate::aes::x86::finish16_vaes256,
    batch4_l4: crate::aes::x86::batch4_l4_vaesvl,
    batch2_l8: crate::aes::x86::batch2_l8_vaesvl,
};

#[cfg(all(target_arch = "aarch64", not(feature = "force-fallback")))]
pub const NEON: Backend = Backend {
    name: "neon",
    one_shot: crate::aes::arm::hash128_neon,
    absorb16: crate::aes::arm::absorb16_neon,
    finish16: crate::aes::arm::finish16_neon,
    batch4_l4: crate::aes::arm::batch4_l4_neon,
    batch2_l8: crate::aes::arm::batch2_l8_neon,
};

fn select() -> &'static Backend {
    #[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
    {
        #[cfg(feature = "std")]
        {
            if std::is_x86_feature_detected!("vaes") && std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512vl") {
                return &VAESVL;
            }
            if std::is_x86_feature_detected!("vaes") && std::is_x86_feature_detected!("avx2") {
                return &VAES256;
            }
            if std::is_x86_feature_detected!("aes") && std::is_x86_feature_detected!("sse2") {
                return &AESNI;
            }
        }
        #[cfg(not(feature = "std"))]
        {
            if cfg!(all(target_feature = "vaes", target_feature = "avx512f", target_feature = "avx512vl")) {
                return &VAESVL;
            }
            if cfg!(all(target_feature = "vaes", target_feature = "avx2")) {
                return &VAES256;
            }
            if cfg!(all(target_feature = "aes", target_feature = "sse2")) {
                return &AESNI;
            }
        }
    }
    #[cfg(all(target_arch = "aarch64", not(feature = "force-fallback")))]
    {
        #[cfg(feature = "std")]
        {
            if std::arch::is_aarch64_feature_detected!("aes") && std::arch::is_aarch64_feature_detected!("neon") {
                return &NEON;
            }
        }
        #[cfg(not(feature = "std"))]
        {
            if cfg!(all(target_feature = "aes", target_feature = "neon")) {
                return &NEON;
            }
        }
    }
    &SOFT
}

static CACHE: AtomicUsize = AtomicUsize::new(0);

#[inline]
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

/// 128-bit hash of `bytes` under `seed`; tail calls only, so no prologue on any path.
#[inline(always)]
pub fn hash128(bytes: &[u8], seed: u64) -> u128 {
    let len = bytes.len();
    if len > crate::aes::SHORT_MAX {
        // SAFETY: backend chosen by feature detection; `bytes` is a valid slice of len > 64.
        return unsafe { (backend().one_shot)(bytes.as_ptr(), len, seed) };
    }
    if len <= 16 {
        crate::aes::short::short128_le16(bytes, seed)
    } else if len <= 32 {
        crate::aes::short::short128_le32(bytes, seed)
    } else {
        crate::aes::short::short128_le64(bytes, seed)
    }
}

/// 64-bit hash: the low half of `hash128`.
#[inline(always)]
pub fn hash64(bytes: &[u8], seed: u64) -> u64 {
    let len = bytes.len();
    if len > crate::aes::SHORT_MAX {
        // SAFETY: as in `hash128`.
        return unsafe { (backend().one_shot)(bytes.as_ptr(), len, seed) as u64 };
    }
    if len <= 16 {
        crate::aes::short::short64_le16(bytes, seed)
    } else if len <= 32 {
        crate::aes::short::short64_le32(bytes, seed)
    } else {
        crate::aes::short::short64_le64(bytes, seed)
    }
}

/// `out[i] = hash128(inputs[i], seed)`, with same-regime inputs interleaved so eight AES
/// chains stay busy (four at 65..=256 B, two at 257..=1024 B). Output order is kept.
#[inline(always)]
pub fn hash128_batch(inputs: &[&[u8]], seed: u64, out: &mut [u128]) {
    batch_with(backend(), inputs, seed, |i, v| out[i] = v);
}

/// Batched `hash64`.
#[inline(always)]
pub fn hash64_batch(inputs: &[&[u8]], seed: u64, out: &mut [u64]) {
    batch_with(backend(), inputs, seed, |i, v| out[i] = v as u64);
}

/// The grouping, generic over the backend (tests run it on every one). Short-path
/// secrets are derived only when a short input appears.
#[inline(always)]
pub fn batch_with(b: &Backend, inputs: &[&[u8]], seed: u64, mut store: impl FnMut(usize, u128)) {
    let mut k: Option<[u64; 8]> = None;
    let (mut p4, mut l4, mut i4, mut n4) = ([core::ptr::null(); 4], [0usize; 4], [0usize; 4], 0usize);
    let (mut p8, mut l8, mut i8, mut n8) = ([core::ptr::null(); 2], [0usize; 2], [0usize; 2], 0usize);
    for (i, inp) in inputs.iter().enumerate() {
        let len = inp.len();
        if len <= crate::aes::SHORT_MAX {
            let k = k.get_or_insert_with(|| crate::aes::short::secrets(seed));
            store(i, short128_with(inp, k));
        } else if len <= crate::aes::L4_MAX {
            p4[n4] = inp.as_ptr();
            l4[n4] = len;
            i4[n4] = i;
            n4 += 1;
            if n4 == 4 {
                // SAFETY: four valid slices of 65..=256 bytes.
                let r = unsafe { (b.batch4_l4)(&p4, &l4, seed) };
                store(i4[0], r[0]);
                store(i4[1], r[1]);
                store(i4[2], r[2]);
                store(i4[3], r[3]);
                n4 = 0;
            }
        } else if len <= crate::aes::L8_MAX {
            p8[n8] = inp.as_ptr();
            l8[n8] = len;
            i8[n8] = i;
            n8 += 1;
            if n8 == 2 {
                // SAFETY: two valid slices of 257..=1024 bytes.
                let r = unsafe { (b.batch2_l8)(&p8, &l8, seed) };
                store(i8[0], r[0]);
                store(i8[1], r[1]);
                n8 = 0;
            }
        } else {
            // SAFETY: valid slice of len > 1024.
            store(i, unsafe { (b.one_shot)(inp.as_ptr(), len, seed) });
        }
    }
    for j in 0..n4 {
        // SAFETY: valid slice of len > 64.
        store(i4[j], unsafe { (b.one_shot)(p4[j], l4[j], seed) });
    }
    for j in 0..n8 {
        // SAFETY: valid slice of len > 64.
        store(i8[j], unsafe { (b.one_shot)(p8[j], l8[j], seed) });
    }
}
