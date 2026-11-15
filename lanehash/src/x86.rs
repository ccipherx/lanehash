//! x86-64 kernels of the long path, one module per vector width, all equal to `spec`.
//! AVX2 and AVX-512 keep the 64-word state in registers, in the one-shot functions and
//! in `absorb`, which serves `Stream` over caller-owned state; SSE2 keeps it in a 64-byte
//! aligned array.
use super::spec::{fold, Words, BLOCK, CHAINS, CT, QT, S, S2L, STRIPE};
use core::arch::x86_64::*;

macro_rules! width {
    ($m:ident, $feat:literal, $reg:expr, $smallm:ident, $small_max:expr, $mid_max:expr, $v:ty, $W:expr, $loadu:ident, $storeu:ident, $storem:ident, $xor:ident, $add:ident, $mul:ident, $srl32:ident, $set1:ident) => {
        pub mod $m {
            use super::*;
            /// Registers per 64-byte stripe.
            const R: usize = 64 / $W;
            /// 64-bit words per register.
            const WPR: usize = $W / 8;
            /// Up to this many leading stripes the one-shot functions use `small`.
            const SMALL_MAX: usize = $small_max;
            /// Up to this many they use the `chained` of the module that provides `small`.
            const MID_MAX: usize = $mid_max;
            /// Without the state in registers: up to this many whole blocks run two chains
            /// at a time from start to merge.
            #[allow(dead_code)]
            const FUSE_MAX: usize = 4;

            /// `x = a ^ w; x + lo32(x) * hi32(x)` on one register.
            #[inline(always)]
            unsafe fn step(a: $v, w: $v) -> $v {
                let x = $xor(a, w);
                $add(x, $mul(x, $srl32(x)))
            }
            #[inline(always)]
            unsafe fn ld(p: *const u64, r: usize) -> $v {
                $loadu(p.add(r * WPR) as *const $v)
            }
            #[inline(always)]
            unsafe fn ldb(p: *const u8, r: usize) -> $v {
                $loadu(p.add(r * $W) as *const $v)
            }
            /// Chain `c`'s start state `CT + ks`, register `r`.
            #[inline(always)]
            unsafe fn start(c: usize, r: usize, ksv: $v) -> $v {
                $add(ld(CT.as_ptr().add(8 * c), r), ksv)
            }
            /// Chain `c`'s merge constants, register `r`. Loops, never
            /// `core::array::from_fn`, in this module: a closure does not inherit the
            /// function's target features, so without AVX in the build its intrinsics go
            /// through memory (+360 cycles per call).
            #[inline(always)]
            unsafe fn q(c: usize, r: usize) -> $v {
                ld(QT.as_ptr().add(8 * c), r)
            }

            /// The folds of `spec::finish_lanes` from the merged lanes in registers. The
            /// XOR with the secrets is done on the vectors, in lane order, so the eight
            /// words leave through plain extractions: with the XOR on the scalar side the
            /// compiler vectorized it in multiply order and paid in shuffles (16 cycles
            /// on the path at 128 B). No memory in between either: a 512-bit store is not
            /// forwarded to the 64-bit loads that follow (+10 cycles).
            #[inline(always)]
            unsafe fn fin<const HI: bool>(mv: &[$v; R], ks: u64, n: usize) -> u128 {
                let mut x = [0u64; 8];
                for r in 0..R {
                    $storem(&mut x[r * WPR..r * WPR + WPR], $xor(mv[r], ld(S.as_ptr(), r)));
                }
                let h = fold(x[0], x[1]) ^ fold(x[2], x[3]) ^ fold(x[4], x[5]) ^ fold(x[6], x[7]);
                let lo = fold(h ^ n as u64, ks ^ S[8]);
                if !HI {
                    return lo as u128;
                }
                let mut y = [0u64; 8];
                for r in 0..R {
                    $storem(&mut y[r * WPR..r * WPR + WPR], $xor(mv[r], ld(S2L.as_ptr(), r)));
                }
                let h2 = fold(y[0], y[2]) ^ fold(y[1], y[3]) ^ fold(y[4], y[6]) ^ fold(y[5], y[7]);
                let hi = fold(h2 ^ n as u64, ks ^ S[18]);
                lo as u128 | (hi as u128) << 64
            }

            /// `m <= SMALL_MAX` leading stripes: stripe step and merge step per chain,
            /// no state. Latency-bound, so the AVX-512 backend uses the AVX2 module's
            /// copy: two ymm chains in flight beat one double-pumped zmm on the test CPU.
            #[inline(always)]
            #[allow(dead_code)] // unused in the avx512 module itself
            pub(super) unsafe fn small<const HI: bool>(p: *const u8, n: usize, m: usize, ks: u64) -> u128 {
                let (ksv, nv) = ($set1(ks as i64), $set1(n as i64));
                let mut mv = [$set1(0); R];
                for s in 0..m {
                    for r in 0..R {
                        mv[r] = $xor(mv[r], step(step(start(s, r, ksv), ldb(p.add(s * 64), r)), q(s, r)));
                    }
                }
                for r in 0..R {
                    mv[r] = $xor(mv[r], step(step(start(m, r, ksv), $xor(ldb(p.add(n - 64), r), nv)), q(m, r)));
                }
                fin::<HI>(&mv, ks, n)
            }

            /// With the chain states in registers: the `rem < 8` leading stripes at
            /// `lead`, the closing stripe at `closing` into chain `m mod 8`, the merge
            /// over the chains that received data.
            #[inline(always)]
            #[allow(dead_code)] // the SSE2 module keeps its state in memory
            unsafe fn close(a: &mut [[$v; R]; CHAINS], lead: *const u8, rem: usize, closing: *const u8, m: usize, n: usize) -> [$v; R] {
                for c in 0..CHAINS {
                    if c < rem {
                        for r in 0..R {
                            a[c][r] = step(a[c][r], ldb(lead.add(c * 64), r));
                        }
                    }
                }
                let nv = $set1(n as i64);
                for c in 0..CHAINS {
                    if c == m % CHAINS {
                        for r in 0..R {
                            a[c][r] = step(a[c][r], $xor(ldb(closing, r), nv));
                        }
                    }
                }
                let used = (m + 1).min(CHAINS);
                let mut mv = [$set1(0); R];
                for c in 0..CHAINS {
                    if c < used {
                        for r in 0..R {
                            mv[r] = $xor(mv[r], step(a[c][r], q(c, r)));
                        }
                    }
                }
                mv
            }
            /// The memory form of `close` over a 64-word state at `st`.
            #[inline(always)]
            unsafe fn close_mem(st: *mut u64, lead: *const u8, rem: usize, closing: *const u8, m: usize, n: usize) -> [$v; R] {
                for c in 0..rem {
                    let sp = st.add(8 * c);
                    for r in 0..R {
                        $storeu(sp.add(r * WPR) as *mut $v, step(ld(sp, r), ldb(lead.add(c * 64), r)));
                    }
                }
                let nv = $set1(n as i64);
                let sp = st.add(8 * (m % CHAINS));
                for r in 0..R {
                    $storeu(sp.add(r * WPR) as *mut $v, step(ld(sp, r), $xor(ldb(closing, r), nv)));
                }
                let used = (m + 1).min(CHAINS);
                let mut mv = [$set1(0); R];
                for c in 0..used {
                    for r in 0..R {
                        mv[r] = $xor(mv[r], step(ld(st.add(8 * c), r), q(c, r)));
                    }
                }
                mv
            }

            /// The hash of `n >= 65` bytes at `p`, inlined into `hash64` / `hash128` so
            /// the merged lanes reach the folds in registers. Three regimes by the number
            /// of leading stripes: `small`, then the chained form of the narrower module
            /// (on the test CPU ymm chains give the throughput of zmm and 11 cycles less latency
            /// up to 448 B), then this module's own chained form.
            #[inline(always)]
            unsafe fn hash<const HI: bool>(p: *const u8, n: usize, ks: u64) -> u128 {
                debug_assert!(n >= 65);
                let m = (n - 1) / STRIPE;
                if m <= SMALL_MAX {
                    return super::$smallm::small::<HI>(p, n, m, ks);
                }
                if m <= MID_MAX {
                    return super::$smallm::chained::<HI>(p, n, m, ks);
                }
                chained::<HI>(p, n, m, ks)
            }

            /// All eight chains started, whole blocks absorbed, then `close`.
            #[inline(always)]
            pub(super) unsafe fn chained<const HI: bool>(p: *const u8, n: usize, m: usize, ks: u64) -> u128 {
                let ksv = $set1(ks as i64);
                let mv = if $reg {
                    let mut a = [[$set1(0); R]; CHAINS];
                    for c in 0..CHAINS {
                        for r in 0..R {
                            a[c][r] = start(c, r, ksv);
                        }
                    }
                    let mut s = 0;
                    while s + CHAINS <= m {
                        for c in 0..CHAINS {
                            for r in 0..R {
                                a[c][r] = step(a[c][r], ldb(p.add((s + c) * 64), r));
                            }
                        }
                        s += CHAINS;
                    }
                    close(&mut a, p.add(s * 64), m - s, p.add(n - 64), m, n)
                } else {
                    // The state does not fit in registers.
                    let nb = m / CHAINS;
                    let rem = m - nb * CHAINS;
                    if nb <= FUSE_MAX {
                        // A few blocks: chains are independent until the merge (chain c
                        // sees only stripes c mod 8), so two chains at a time run from
                        // start to merge with their lanes in registers and no state
                        // array. One chain at a time would be latency bound (four
                        // registers in flight), and the state array costs 64 words of
                        // stores and loads at each end: 513 B took 167 cycles against
                        // 111 for 512.
                        let nv = $set1(n as i64);
                        let mut mv = [$set1(0); R];
                        let mut c = 0;
                        while c < CHAINS {
                            let mut a = [[$set1(0); R]; 2];
                            for k in 0..2 {
                                for r in 0..R {
                                    a[k][r] = start(c + k, r, ksv);
                                }
                            }
                            for b in 0..nb {
                                for k in 0..2 {
                                    for r in 0..R {
                                        a[k][r] = step(a[k][r], ldb(p.add((b * CHAINS + c + k) * 64), r));
                                    }
                                }
                            }
                            for k in 0..2 {
                                if c + k < rem {
                                    for r in 0..R {
                                        a[k][r] = step(a[k][r], ldb(p.add((nb * CHAINS + c + k) * 64), r));
                                    }
                                }
                                if c + k == rem {
                                    for r in 0..R {
                                        a[k][r] = step(a[k][r], $xor(ldb(p.add(n - 64), r), nv));
                                    }
                                }
                                for r in 0..R {
                                    mv[r] = $xor(mv[r], step(a[k][r], q(c + k, r)));
                                }
                            }
                            c += 2;
                        }
                        mv
                    } else {
                        let mut st = Words([0u64; 64]);
                        let st = st.0.as_mut_ptr();
                        for c in 0..CHAINS {
                            for r in 0..R {
                                $storeu(st.add(8 * c + r * WPR) as *mut $v, start(c, r, ksv));
                            }
                        }
                        let mut s = 0;
                        while s + CHAINS <= m {
                            for c in 0..CHAINS {
                                let sp = st.add(8 * c);
                                for r in 0..R {
                                    $storeu(sp.add(r * WPR) as *mut $v, step(ld(sp, r), ldb(p.add((s + c) * 64), r)));
                                }
                            }
                            s += CHAINS;
                        }
                        close_mem(st, p.add(s * 64), m - s, p.add(n - 64), m, n)
                    }
                };
                fin::<HI>(&mv, ks, n)
            }

            /// 64-bit hash of `n >= 65` bytes at `p` (`ks` from `spec::ks`).
            #[target_feature(enable = $feat)]
            pub unsafe fn hash64(p: *const u8, n: usize, ks: u64) -> u64 {
                hash::<false>(p, n, ks) as u64
            }
            /// 128-bit hash of `n >= 65` bytes at `p`.
            #[target_feature(enable = $feat)]
            pub unsafe fn hash128(p: *const u8, n: usize, ks: u64) -> u128 {
                hash::<true>(p, n, ks)
            }

            /// Streaming: absorb `nblocks` whole leading blocks at `p` into `st`.
            #[target_feature(enable = $feat)]
            pub unsafe fn absorb(st: &mut Words, p: *const u8, nblocks: usize) {
                let st = st.0.as_mut_ptr();
                if $reg {
                    // state in registers for the whole call: with a load and a store per
                    // stripe, store forwarding sits on every chain's dependency path
                    let mut a = [[$set1(0); R]; CHAINS];
                    for c in 0..CHAINS {
                        for r in 0..R {
                            a[c][r] = ld(st.add(8 * c), r);
                        }
                    }
                    for b in 0..nblocks {
                        for c in 0..CHAINS {
                            for r in 0..R {
                                a[c][r] = step(a[c][r], ldb(p.add(b * BLOCK + c * 64), r));
                            }
                        }
                    }
                    for c in 0..CHAINS {
                        for r in 0..R {
                            $storeu(st.add(8 * c + r * WPR) as *mut $v, a[c][r]);
                        }
                    }
                } else {
                    for b in 0..nblocks {
                        for c in 0..CHAINS {
                            let sp = st.add(8 * c);
                            for r in 0..R {
                                $storeu(sp.add(r * WPR) as *mut $v, step(ld(sp, r), ldb(p.add(b * BLOCK + c * 64), r)));
                            }
                        }
                    }
                }
            }

            /// Streaming: the 128-bit hash from `st`, the `r < 8` remaining leading stripes
            /// at `leading` and the closing stripe at `closing` (`spec::finish`).
            #[target_feature(enable = $feat)]
            pub unsafe fn finish(st: &Words, leading: *const u8, r: usize, closing: *const u8, n: usize, ks: u64) -> u128 {
                let m = (n - 1) / STRIPE;
                let mut w = Words(st.0);
                let mv = close_mem(w.0.as_mut_ptr(), leading, r, closing, m, n);
                fin::<true>(&mv, ks, n)
            }
        }
    };
}

#[inline(always)]
unsafe fn storem_128(out: &mut [u64], v: __m128i) {
    // SSE2 only: `_mm_extract_epi64` would be SSE4.1
    out[0] = _mm_cvtsi128_si64(v) as u64;
    out[1] = _mm_cvtsi128_si64(_mm_unpackhi_epi64(v, v)) as u64;
}
/// Both words of an xmm register to general registers, as two instructions the compiler
/// cannot regroup: left to itself it gathers even and odd words with `vpunpck` and
/// `vpermq` first, on the critical path of every hash under 513 bytes.
#[inline(always)]
unsafe fn storem_128v(out: &mut [u64], v: __m128i) {
    #[cfg(not(any(miri, kani)))]
    {
        let (a, b): (u64, u64);
        core::arch::asm!("vmovq {a}, {v}", "vpextrq {b}, {v}, 1", v = in(xmm_reg) v, a = out(reg) a, b = out(reg) b, options(pure, nomem, nostack, preserves_flags));
        out[0] = a;
        out[1] = b;
    }
    #[cfg(any(miri, kani))]
    storem_128(out, v);
}
#[inline(always)]
unsafe fn storem_256(out: &mut [u64], v: __m256i) {
    storem_128v(&mut out[..2], _mm256_castsi256_si128(v));
    storem_128v(&mut out[2..], _mm256_extracti128_si256::<1>(v));
}
#[inline(always)]
unsafe fn storem_512(out: &mut [u64], v: __m512i) {
    storem_256(&mut out[..4], _mm512_castsi512_si256(v));
    storem_256(&mut out[4..], _mm512_extracti64x4_epi64::<1>(v));
}
#[inline(always)]
unsafe fn srl32_128(x: __m128i) -> __m128i {
    _mm_srli_epi64::<32>(x)
}
#[inline(always)]
unsafe fn srl32_256(x: __m256i) -> __m256i {
    _mm256_srli_epi64::<32>(x)
}
#[inline(always)]
unsafe fn srl32_512(x: __m512i) -> __m512i {
    _mm512_srli_epi64::<32>(x)
}

// BMI2 with the AVX kernels: the inlined folds become `mulx`, without the register
// shuffling around `mul`. Every CPU with AVX2 has it; the dispatcher checks both.
width!(sse2, "sse2", false, sse2, 7, 0, __m128i, 16, _mm_loadu_si128, _mm_storeu_si128, storem_128, _mm_xor_si128, _mm_add_epi64, _mm_mul_epu32, srl32_128, _mm_set1_epi64x);
width!(avx2, "avx2,bmi2", true, avx2, 3, 0, __m256i, 32, _mm256_loadu_si256, _mm256_storeu_si256, storem_256, _mm256_xor_si256, _mm256_add_epi64, _mm256_mul_epu32, srl32_256, _mm256_set1_epi64x);
width!(avx512, "avx512f,avx512vl,avx2,bmi2", true, avx2, 3, 6, __m512i, 64, _mm512_loadu_si512, _mm512_storeu_si512, storem_512, _mm512_xor_si512, _mm512_add_epi64, _mm512_mul_epu32, srl32_512, _mm512_set1_epi64);
