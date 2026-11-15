//! Kani proofs for the AES T-table round and the x86 kernels.

/// The T-table round equals the FIPS-197 round for every state and round key.
#[kani::proof]
fn aes_soft_round_is_spec_round() {
    let x: [u8; 16] = kani::any();
    let k: [u8; 16] = kani::any();
    assert_eq!(crate::aes::soft::aes_round(x, k), crate::aes::spec::aes_round(x, k));
}

#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
mod x86 {
    use crate::spec::{finish_lanes, lanes, step, Words, BLOCK};
    use core::arch::x86_64::*;

    // The lane step of `x86.rs`: `x = a ^ w`, then `x + mul_epu32(x, x >> 32)`. The final
    // `add_epi64` is checked as `wrapping_add`: Kani 0.68 flags every `simd_add` lane
    // overflow, even with `--no-overflow-checks`.
    #[target_feature(enable = "sse2")]
    unsafe fn parts_sse2(a: [u64; 2], w: [u64; 2]) -> ([u64; 2], [u64; 2]) {
        let x = _mm_xor_si128(_mm_loadu_si128(a.as_ptr() as *const __m128i), _mm_loadu_si128(w.as_ptr() as *const __m128i));
        let p = _mm_mul_epu32(x, _mm_srli_epi64::<32>(x));
        let (mut xo, mut po) = ([0u64; 2], [0u64; 2]);
        _mm_storeu_si128(xo.as_mut_ptr() as *mut __m128i, x);
        _mm_storeu_si128(po.as_mut_ptr() as *mut __m128i, p);
        (xo, po)
    }
    #[target_feature(enable = "avx2")]
    unsafe fn parts_avx2(a: [u64; 4], w: [u64; 4]) -> ([u64; 4], [u64; 4]) {
        let x = _mm256_xor_si256(_mm256_loadu_si256(a.as_ptr() as *const __m256i), _mm256_loadu_si256(w.as_ptr() as *const __m256i));
        let p = _mm256_mul_epu32(x, _mm256_srli_epi64::<32>(x));
        let (mut xo, mut po) = ([0u64; 4], [0u64; 4]);
        _mm256_storeu_si256(xo.as_mut_ptr() as *mut __m256i, x);
        _mm256_storeu_si256(po.as_mut_ptr() as *mut __m256i, p);
        (xo, po)
    }
    #[target_feature(enable = "avx512f")]
    unsafe fn parts_avx512(a: [u64; 8], w: [u64; 8]) -> ([u64; 8], [u64; 8]) {
        let x = _mm512_xor_si512(_mm512_loadu_si512(a.as_ptr() as *const __m512i), _mm512_loadu_si512(w.as_ptr() as *const __m512i));
        let p = _mm512_mul_epu32(x, _mm512_srli_epi64::<32>(x));
        let (mut xo, mut po) = ([0u64; 8], [0u64; 8]);
        _mm512_storeu_si512(xo.as_mut_ptr() as *mut __m512i, x);
        _mm512_storeu_si512(po.as_mut_ptr() as *mut __m512i, p);
        (xo, po)
    }

    macro_rules! lane_harness {
        ($name:ident, $parts:ident, $n:literal) => {
            #[kani::proof]
            fn $name() {
                let (a, w): ([u64; $n], [u64; $n]) = (kani::any(), kani::any());
                let (x, p) = unsafe { $parts(a, w) };
                for i in 0..$n {
                    assert_eq!(x[i], a[i] ^ w[i]);
                    assert_eq!(p[i], (x[i] as u32 as u64) * (x[i] >> 32));
                    assert_eq!(x[i].wrapping_add(p[i]), step(a[i], w[i]));
                }
            }
        };
    }
    lane_harness!(lane_step_sse2, parts_sse2, 2);
    lane_harness!(lane_step_avx2, parts_avx2, 4);
    lane_harness!(lane_step_avx512, parts_avx512, 8);

    // The intrinsic lane extraction of `storem_256`, as Kani and Miri compile it.
    #[target_feature(enable = "avx2")]
    unsafe fn extract256(a: [u64; 4]) -> [u64; 4] {
        let v = _mm256_loadu_si256(a.as_ptr() as *const __m256i);
        let (lo, hi) = (_mm256_castsi256_si128(v), _mm256_extracti128_si256::<1>(v));
        [
            _mm_cvtsi128_si64(lo) as u64,
            _mm_cvtsi128_si64(_mm_unpackhi_epi64(lo, lo)) as u64,
            _mm_cvtsi128_si64(hi) as u64,
            _mm_cvtsi128_si64(_mm_unpackhi_epi64(hi, hi)) as u64,
        ]
    }
    #[kani::proof]
    fn lane_extract_avx2() {
        let a: [u64; 4] = kani::any();
        assert_eq!(unsafe { extract256(a) }, a);
    }

    #[kani::proof]
    fn broadcast_avx2() {
        let k: u64 = kani::any();
        let mut o = [0u64; 4];
        unsafe { _mm256_storeu_si256(o.as_mut_ptr() as *mut __m256i, _mm256_set1_epi64x(k as i64)) };
        assert_eq!(o, [k; 4]);
    }

    // Stub for `_mm*_add_epi64`: lane-wise wrapping addition, as Intel defines it.
    fn add_lanes<const N: usize>(a: [u64; N], b: [u64; N]) -> [u64; N] {
        let mut o = [0u64; N];
        for i in 0..N {
            o[i] = a[i].wrapping_add(b[i]);
        }
        o
    }
    #[target_feature(enable = "sse2")]
    fn add128(a: __m128i, b: __m128i) -> __m128i {
        unsafe { core::mem::transmute(add_lanes::<2>(core::mem::transmute(a), core::mem::transmute(b))) }
    }
    #[target_feature(enable = "avx2")]
    fn add256(a: __m256i, b: __m256i) -> __m256i {
        unsafe { core::mem::transmute(add_lanes::<4>(core::mem::transmute(a), core::mem::transmute(b))) }
    }
    #[target_feature(enable = "avx512f")]
    fn add512(a: __m512i, b: __m512i) -> __m512i {
        unsafe { core::mem::transmute(add_lanes::<8>(core::mem::transmute(a), core::mem::transmute(b))) }
    }

    /// One block through `absorb` equals `spec::absorb`, for every state and block.
    macro_rules! absorb_harness {
        ($name:ident, $m:ident, $intr:path, $stub:ident) => {
            #[kani::proof]
            #[kani::stub($intr, $stub)]
            fn $name() {
                let st: [u64; 64] = kani::any();
                let blk: [u8; BLOCK] = kani::any();
                let mut w = Words(st);
                unsafe { crate::x86::$m::absorb(&mut w, blk.as_ptr(), 1) };
                let mut a = Words(st).to_state();
                crate::spec::absorb(&mut a, &blk);
                assert!(w.0 == Words::from_state(&a).0);
            }
        };
    }
    absorb_harness!(absorb_sse2, sse2, core::arch::x86_64::_mm_add_epi64, add128);
    absorb_harness!(absorb_avx2, avx2, core::arch::x86_64::_mm256_add_epi64, add256);
    absorb_harness!(absorb_avx512, avx512, core::arch::x86_64::_mm512_add_epi64, add512);

    // Ackermann's reduction: `step` and `fold` become uninterpreted functions, one table
    // each, shared by kernel and reference. Equal arguments give equal results, anything
    // else is arbitrary. `step` is keyed by `a ^ w`, `fold` by its unordered pair. If the
    // kernel matches the reference for every such function it matches for the real
    // ones, and `lane_step_*` ties the vector step to the scalar one. This removes the
    // multipliers, which CBMC could not get through.
    const TABLE: usize = 80;
    static mut STEPS: [(u64, u64); TABLE] = [(0, 0); TABLE];
    static mut NSTEPS: usize = 0;
    static mut FOLDS: [(u64, u64, u64); TABLE] = [(0, 0, 0); TABLE];
    static mut NFOLDS: usize = 0;

    // Every call appends an entry, so the table length is the call count, a constant for
    // CBMC. Returning early on a hit made it data-dependent.
    fn step_key(key: u64) -> u64 {
        unsafe {
            let mut r: u64 = kani::any();
            let mut found = false;
            let mut i = 0;
            while i < NSTEPS {
                if !found && STEPS[i].0 == key {
                    r = STEPS[i].1;
                    found = true;
                }
                i += 1;
            }
            assert!(NSTEPS < TABLE, "step table full");
            STEPS[NSTEPS] = (key, r);
            NSTEPS += 1;
            r
        }
    }
    fn step_uf(a: u64, w: u64) -> u64 {
        step_key(a ^ w)
    }
    macro_rules! step_uf_vec {
        ($name:ident, $v:ty, $n:literal) => {
            unsafe fn $name(a: $v, w: $v) -> $v {
                let (a, w): ([u64; $n], [u64; $n]) = (core::mem::transmute(a), core::mem::transmute(w));
                let mut o = [0u64; $n];
                for i in 0..$n {
                    o[i] = step_key(a[i] ^ w[i]);
                }
                core::mem::transmute(o)
            }
        };
    }
    step_uf_vec!(step_uf128, __m128i, 2);
    step_uf_vec!(step_uf256, __m256i, 4);
    step_uf_vec!(step_uf512, __m512i, 8);

    fn fold_uf(a: u64, b: u64) -> u64 {
        unsafe {
            let mut r: u64 = kani::any();
            let mut found = false;
            let mut i = 0;
            while i < NFOLDS {
                let (x, y, v) = FOLDS[i];
                if !found && ((x == a && y == b) || (x == b && y == a)) {
                    r = v;
                    found = true;
                }
                i += 1;
            }
            assert!(NFOLDS < TABLE, "fold table full");
            FOLDS[NFOLDS] = (a, b, r);
            NFOLDS += 1;
            r
        }
    }

    /// The one-shot kernel equals `finish_lanes(lanes(..))` at one length, for every
    /// input and key.
    macro_rules! hash_harness {
        ($name:ident, $m:ident, $n:literal, $add:path, $addstub:ident, $vstep:path, $vstub:ident) => {
            #[kani::proof]
            #[kani::unwind(90)]
            #[kani::stub($add, $addstub)]
            #[kani::stub(crate::spec::fold, fold_uf)]
            #[kani::stub(crate::spec::step, step_uf)]
            #[kani::stub($vstep, $vstub)]
            fn $name() {
                let p: [u8; $n] = kani::any();
                let ks: u64 = kani::any();
                let got = unsafe { crate::x86::$m::hash128(p.as_ptr(), $n, ks) };
                assert_eq!(got, finish_lanes::<true>(&lanes(&p, ks), ks, $n));
            }
        };
    }
    hash_harness!(hash_avx2_65, avx2, 65, core::arch::x86_64::_mm256_add_epi64, add256, crate::x86::avx2::step, step_uf256);
    hash_harness!(hash_sse2_65, sse2, 65, core::arch::x86_64::_mm_add_epi64, add128, crate::x86::sse2::step, step_uf128);

    /// Like `hash_harness!`, with both widths stubbed: up to three leading stripes the
    /// AVX-512 kernel runs AVX2 code.
    #[kani::proof]
    #[kani::unwind(90)]
    #[kani::stub(core::arch::x86_64::_mm256_add_epi64, add256)]
    #[kani::stub(core::arch::x86_64::_mm512_add_epi64, add512)]
    #[kani::stub(crate::spec::fold, fold_uf)]
    #[kani::stub(crate::spec::step, step_uf)]
    #[kani::stub(crate::x86::avx2::step, step_uf256)]
    #[kani::stub(crate::x86::avx512::step, step_uf512)]
    fn hash_avx512_65() {
        let p: [u8; 65] = kani::any();
        let ks: u64 = kani::any();
        let got = unsafe { crate::x86::avx512::hash128(p.as_ptr(), 65, ks) };
        assert_eq!(got, finish_lanes::<true>(&lanes(&p, ks), ks, 65));
    }
}
