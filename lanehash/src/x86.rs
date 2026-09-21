//! x86-64 kernels of the long path, one module per vector width, all equal to `spec`.
//! AVX2 and AVX-512 keep the 64-word state in registers; SSE2 keeps it in a 64-byte
//! aligned stack array. `absorb` / `finish` serve `Stream` over caller-owned state.
use super::spec::{finish_lanes, Merged, Words, BLOCK, CHAINS, CT, Q, STRIPE};
use core::arch::x86_64::*;

macro_rules! width {
    ($m:ident, $feat:literal, $reg:expr, $smallm:ident, $v:ty, $W:expr, $loadu:ident, $storeu:ident, $storem:ident, $xor:ident, $add:ident, $mul:ident, $srl32:ident, $set1:ident) => {
        pub mod $m {
            use super::*;
            /// Registers per 64-byte stripe.
            const R: usize = 64 / $W;
            /// 64-bit words per register.
            const WPR: usize = $W / 8;

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
            #[inline(always)]
            /// Merged lanes to memory word by word through general registers: a 512-bit
            /// vector store is not forwarded to the 64-bit loads that follow (+10 cycles at
            /// 128 B).
            unsafe fn store(mv: &[$v; R]) -> Merged {
                let mut out = Merged([0u64; 8]);
                for r in 0..R {
                    $storem(&mut out.0[r * WPR..r * WPR + WPR], mv[r]);
                }
                out
            }
            /// Chain `c`'s start state `CT + ks`, register `r`.
            #[inline(always)]
            unsafe fn start(c: usize, r: usize, ksv: $v) -> $v {
                $add(ld(CT.as_ptr().add(8 * c), r), ksv)
            }
            /// `Q` in `R` registers. Loops, never `core::array::from_fn`, in this module: a
            /// closure does not inherit the function's target features, so without AVX in the
            /// build its intrinsics go through memory (+360 cycles per call).
            #[inline(always)]
            unsafe fn qv() -> [$v; R] {
                let mut q = [$set1(0); R];
                for r in 0..R {
                    q[r] = ld(Q.as_ptr(), r);
                }
                q
            }

            /// `m < 8`: stripe step and merge step per chain in registers. Latency-bound,
            /// so the AVX-512 backend uses the AVX2 module's copy: two ymm chains in flight
            /// beat one double-pumped zmm (38 -> 28 cycles at 128 B).
            #[inline(always)]
            #[allow(dead_code)] // unused in the avx512 module itself
            pub(super) unsafe fn small(p: *const u8, n: usize, m: usize, ks: u64) -> Merged {
                let (ksv, nv, q) = ($set1(ks as i64), $set1(n as i64), qv());
                let mut mv = [$set1(0); R];
                for s in 0..m {
                    for r in 0..R {
                        mv[r] = $xor(mv[r], step(step(start(s, r, ksv), ldb(p.add(s * 64), r)), q[r]));
                    }
                }
                for r in 0..R {
                    mv[r] = $xor(mv[r], step(step(start(m, r, ksv), $xor(ldb(p.add(n - 64), r), nv)), q[r]));
                }
                store(&mv)
            }

            /// Merged lanes of `n >= 65` bytes at `p`. Inlined into `hash64` / `hash128` so
            /// the words reach the folds in registers (returned across a call: a 512-bit
            /// stack store Zen 4 does not forward, 100 instead of 35 cycles at 128 B).
            #[inline(always)]
            unsafe fn lanes(p: *const u8, n: usize, ks: u64) -> Merged {
                debug_assert!(n >= 65);
                let m = (n - 1) / STRIPE;
                if m < CHAINS {
                    return super::$smallm::small(p, n, m, ks);
                }
                let (ksv, nv, q) = ($set1(ks as i64), $set1(n as i64), qv());
                let mut mv = [$set1(0); R];
                if $reg {
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
                    let rem = m - s;
                    for c in 0..CHAINS {
                        if c < rem {
                            for r in 0..R {
                                a[c][r] = step(a[c][r], ldb(p.add((s + c) * 64), r));
                            }
                        }
                    }
                    for c in 0..CHAINS {
                        if c == m % CHAINS {
                            for r in 0..R {
                                a[c][r] = step(a[c][r], $xor(ldb(p.add(n - 64), r), nv));
                            }
                        }
                    }
                    for c in 0..CHAINS {
                        for r in 0..R {
                            mv[r] = $xor(mv[r], step(a[c][r], q[r]));
                        }
                    }
                } else {
                    let mut st = Words([0u64; 64]);
                    let st = st.0.as_mut_ptr();
                    for c in 0..CHAINS {
                        for r in 0..R {
                            $storeu(st.add(8 * c + r * WPR) as *mut $v, step(start(c, r, ksv), ldb(p.add(c * 64), r)));
                        }
                    }
                    let mut s = CHAINS;
                    while s + CHAINS <= m {
                        for c in 0..CHAINS {
                            let sp = st.add(8 * c);
                            for r in 0..R {
                                $storeu(sp.add(r * WPR) as *mut $v, step(ld(sp, r), ldb(p.add((s + c) * 64), r)));
                            }
                        }
                        s += CHAINS;
                    }
                    while s < m {
                        let sp = st.add(8 * (s % CHAINS));
                        for r in 0..R {
                            $storeu(sp.add(r * WPR) as *mut $v, step(ld(sp, r), ldb(p.add(s * 64), r)));
                        }
                        s += 1;
                    }
                    let sp = st.add(8 * (m % CHAINS));
                    for r in 0..R {
                        $storeu(sp.add(r * WPR) as *mut $v, step(ld(sp, r), $xor(ldb(p.add(n - 64), r), nv)));
                    }
                    for c in 0..CHAINS {
                        for r in 0..R {
                            mv[r] = $xor(mv[r], step(ld(st.add(8 * c), r), q[r]));
                        }
                    }
                }
                store(&mv)
            }

            /// 64-bit hash of `n >= 65` bytes at `p` (`ks` from `spec::ks`).
            #[target_feature(enable = $feat)]
            pub unsafe fn hash64(p: *const u8, n: usize, ks: u64) -> u64 {
                finish_lanes::<false>(&lanes(p, n, ks), ks, n) as u64
            }
            /// 128-bit hash of `n >= 65` bytes at `p`.
            #[target_feature(enable = $feat)]
            pub unsafe fn hash128(p: *const u8, n: usize, ks: u64) -> u128 {
                finish_lanes::<true>(&lanes(p, n, ks), ks, n)
            }

            /// Streaming: absorb `nblocks` whole leading blocks at `p` into `st`.
            #[target_feature(enable = $feat)]
            pub unsafe fn absorb(st: &mut Words, p: *const u8, nblocks: usize) {
                let st = st.0.as_mut_ptr();
                for b in 0..nblocks {
                    for c in 0..CHAINS {
                        let sp = st.add(8 * c);
                        for r in 0..R {
                            $storeu(sp.add(r * WPR) as *mut $v, step(ld(sp, r), ldb(p.add(b * BLOCK + c * 64), r)));
                        }
                    }
                }
            }

            /// Streaming: the 128-bit hash from `st`, the `r < 8` remaining leading stripes
            /// at `leading` and the closing stripe at `closing` (`spec::finish`).
            #[target_feature(enable = $feat)]
            pub unsafe fn finish(st: &Words, leading: *const u8, r: usize, closing: *const u8, n: usize, ks: u64) -> u128 {
                let mut w = Words(st.0);
                let sp = w.0.as_mut_ptr();
                for c in 0..r {
                    for k in 0..R {
                        $storeu(sp.add(8 * c + k * WPR) as *mut $v, step(ld(sp.add(8 * c), k), ldb(leading.add(c * 64), k)));
                    }
                }
                let (nv, c) = ($set1(n as i64), r % CHAINS);
                for k in 0..R {
                    $storeu(sp.add(8 * c + k * WPR) as *mut $v, step(ld(sp.add(8 * c), k), $xor(ldb(closing, k), nv)));
                }
                let used = ((n - 1) / STRIPE + 1).min(CHAINS);
                let q = qv();
                let mut mv = [$set1(0); R];
                for c in 0..used {
                    for k in 0..R {
                        mv[k] = $xor(mv[k], step(ld(sp.add(8 * c), k), q[k]));
                    }
                }
                finish_lanes::<true>(&store(&mv), ks, n)
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
#[inline(always)]
unsafe fn storem_256(out: &mut [u64], v: __m256i) {
    storem_128(&mut out[..2], _mm256_castsi256_si128(v));
    storem_128(&mut out[2..], _mm256_extracti128_si256::<1>(v));
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

width!(sse2, "sse2", false, sse2, __m128i, 16, _mm_loadu_si128, _mm_storeu_si128, storem_128, _mm_xor_si128, _mm_add_epi64, _mm_mul_epu32, srl32_128, _mm_set1_epi64x);
width!(avx2, "avx2", true, avx2, __m256i, 32, _mm256_loadu_si256, _mm256_storeu_si256, storem_256, _mm256_xor_si256, _mm256_add_epi64, _mm256_mul_epu32, srl32_256, _mm256_set1_epi64x);
width!(avx512, "avx512f", true, avx2, __m512i, 64, _mm512_loadu_si512, _mm512_storeu_si512, storem_512, _mm512_xor_si512, _mm512_add_epi64, _mm512_mul_epu32, srl32_512, _mm512_set1_epi64);
