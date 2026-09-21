//! aarch64 NEON kernels of the long path: `uzp1`/`uzp2` pair the even and odd dwords
//! of two registers, `umlal`/`umlal2` multiply-accumulate them onto `x`. Compile-checked
//! only (no hardware available).
use super::spec::{finish_lanes, Merged, Words, BLOCK, CHAINS, CT, Q, STRIPE};
use core::arch::aarch64::*;

/// `x = a ^ w; x + lo32(x) * hi32(x)` on a register pair.
#[inline(always)]
unsafe fn step2(a0: uint64x2_t, a1: uint64x2_t, w0: uint64x2_t, w1: uint64x2_t) -> (uint64x2_t, uint64x2_t) {
    let (x0, x1) = (veorq_u64(a0, w0), veorq_u64(a1, w1));
    let e = vuzp1q_u32(vreinterpretq_u32_u64(x0), vreinterpretq_u32_u64(x1));
    let o = vuzp2q_u32(vreinterpretq_u32_u64(x0), vreinterpretq_u32_u64(x1));
    (vmlal_u32(x0, vget_low_u32(e), vget_low_u32(o)), vmlal_high_u32(x1, e, o))
}
/// One stripe at `src` (words XORed with `nv`) into chain `c` of `st`.
#[inline(always)]
unsafe fn stripe(st: *mut u64, c: usize, src: *const u8, nv: uint64x2_t) {
    let sp = st.add(8 * c);
    for h in 0..2 {
        let w0 = veorq_u64(vld1q_u64(src.add(h * 32) as *const u64), nv);
        let w1 = veorq_u64(vld1q_u64(src.add(h * 32 + 16) as *const u64), nv);
        let (a0, a1) = step2(vld1q_u64(sp.add(4 * h)), vld1q_u64(sp.add(4 * h + 2)), w0, w1);
        vst1q_u64(sp.add(4 * h), a0);
        vst1q_u64(sp.add(4 * h + 2), a1);
    }
}
#[inline(always)]
unsafe fn merge(st: *const u64, used: usize) -> Merged {
    let q: [uint64x2_t; 4] = core::array::from_fn(|r| vld1q_u64(Q.as_ptr().add(2 * r)));
    let mut mv = [vdupq_n_u64(0); 4];
    for c in 0..used {
        for h in 0..2 {
            let (b0, b1) = step2(vld1q_u64(st.add(8 * c + 4 * h)), vld1q_u64(st.add(8 * c + 4 * h + 2)), q[2 * h], q[2 * h + 1]);
            mv[2 * h] = veorq_u64(mv[2 * h], b0);
            mv[2 * h + 1] = veorq_u64(mv[2 * h + 1], b1);
        }
    }
    let mut out = Merged([0u64; 8]);
    for r in 0..4 {
        vst1q_u64(out.0.as_mut_ptr().add(2 * r), mv[r]);
    }
    out
}

/// Merged lanes of `n >= 65` bytes at `p`, inlined into the entry points (see x86.rs).
#[inline(always)]
unsafe fn lanes(p: *const u8, n: usize, ks: u64) -> Merged {
    let m = (n - 1) / STRIPE;
    let ksv = vdupq_n_u64(ks);
    if m < CHAINS {
        // every chain sees one stripe: stripe step and merge step in registers
        let q: [uint64x2_t; 4] = core::array::from_fn(|r| vld1q_u64(Q.as_ptr().add(2 * r)));
        let mut mv = [vdupq_n_u64(0); 4];
        let mut one = |c: usize, src: *const u8, nv: uint64x2_t| {
            for h in 0..2 {
                let w0 = veorq_u64(vld1q_u64(src.add(h * 32) as *const u64), nv);
                let w1 = veorq_u64(vld1q_u64(src.add(h * 32 + 16) as *const u64), nv);
                let a0 = vaddq_u64(vld1q_u64(CT.as_ptr().add(8 * c + 4 * h)), ksv);
                let a1 = vaddq_u64(vld1q_u64(CT.as_ptr().add(8 * c + 4 * h + 2)), ksv);
                let (a0, a1) = step2(a0, a1, w0, w1);
                let (b0, b1) = step2(a0, a1, q[2 * h], q[2 * h + 1]);
                mv[2 * h] = veorq_u64(mv[2 * h], b0);
                mv[2 * h + 1] = veorq_u64(mv[2 * h + 1], b1);
            }
        };
        for s in 0..m {
            one(s, p.add(s * 64), vdupq_n_u64(0));
        }
        one(m, p.add(n - 64), vdupq_n_u64(n as u64));
        let mut out = Merged([0u64; 8]);
        for r in 0..4 {
            vst1q_u64(out.0.as_mut_ptr().add(2 * r), mv[r]);
        }
        return out;
    }
    let mut w = Words([0u64; 64]);
    let st = w.0.as_mut_ptr();
    for j in 0..32 {
        vst1q_u64(st.add(2 * j), vaddq_u64(vld1q_u64(CT.as_ptr().add(2 * j)), ksv));
    }
    let z = vdupq_n_u64(0);
    let mut s = 0;
    while s + CHAINS <= m {
        for c in 0..CHAINS {
            stripe(st, c, p.add((s + c) * 64), z);
        }
        s += CHAINS;
    }
    while s < m {
        stripe(st, s % CHAINS, p.add(s * 64), z);
        s += 1;
    }
    stripe(st, m % CHAINS, p.add(n - 64), vdupq_n_u64(n as u64));
    merge(st, CHAINS)
}

/// Streaming: absorb `nblocks` whole leading blocks at `p` into `st`.
#[target_feature(enable = "neon")]
pub unsafe fn absorb(st: &mut Words, p: *const u8, nblocks: usize) {
    let z = vdupq_n_u64(0);
    for b in 0..nblocks {
        for c in 0..CHAINS {
            stripe(st.0.as_mut_ptr(), c, p.add(b * BLOCK + c * 64), z);
        }
    }
}

/// Streaming: the 128-bit hash from `st` (`spec::finish`).
#[target_feature(enable = "neon")]
pub unsafe fn finish(st: &Words, leading: *const u8, r: usize, closing: *const u8, n: usize) -> Merged {
    let mut w = Words(st.0);
    let z = vdupq_n_u64(0);
    for c in 0..r {
        stripe(w.0.as_mut_ptr(), c, leading.add(c * 64), z);
    }
    stripe(w.0.as_mut_ptr(), r % CHAINS, closing, vdupq_n_u64(n as u64));
    finish_lanes::<true>(&merge(w.0.as_ptr(), ((n - 1) / STRIPE + 1).min(CHAINS)), ks, n)
}
