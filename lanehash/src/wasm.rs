//! wasm32 simd128 kernels of the long path: `i32x4.shuffle` pairs the even and odd
//! dwords of two registers, `i64x2.extmul_{low,high}_i32x4_u` multiplies them.
use super::spec::{finish_lanes, Merged, Words, BLOCK, CHAINS, CT, Q, STRIPE};
use core::arch::wasm32::*;

#[inline(always)]
fn step2(a0: v128, a1: v128, w0: v128, w1: v128) -> (v128, v128) {
    let (x0, x1) = (v128_xor(a0, w0), v128_xor(a1, w1));
    let e = i32x4_shuffle::<0, 2, 4, 6>(x0, x1);
    let o = i32x4_shuffle::<1, 3, 5, 7>(x0, x1);
    (i64x2_add(x0, i64x2_extmul_low_u32x4(e, o)), i64x2_add(x1, i64x2_extmul_high_u32x4(e, o)))
}
#[inline(always)]
unsafe fn ld(p: *const u8) -> v128 {
    v128_load(p as *const v128)
}
#[inline(always)]
unsafe fn stripe(st: *mut u64, c: usize, src: *const u8, nv: v128) {
    let sp = st.add(8 * c);
    for h in 0..2 {
        let w0 = v128_xor(ld(src.add(h * 32)), nv);
        let w1 = v128_xor(ld(src.add(h * 32 + 16)), nv);
        let (a0, a1) = step2(v128_load(sp.add(4 * h) as *const v128), v128_load(sp.add(4 * h + 2) as *const v128), w0, w1);
        v128_store(sp.add(4 * h) as *mut v128, a0);
        v128_store(sp.add(4 * h + 2) as *mut v128, a1);
    }
}
#[inline(always)]
unsafe fn merge(st: *const u64, used: usize) -> Merged {
    let q: [v128; 4] = core::array::from_fn(|r| ld(Q.as_ptr().add(2 * r) as *const u8));
    let mut mv = [i64x2_splat(0); 4];
    for c in 0..used {
        for h in 0..2 {
            let (b0, b1) = step2(v128_load(st.add(8 * c + 4 * h) as *const v128), v128_load(st.add(8 * c + 4 * h + 2) as *const v128), q[2 * h], q[2 * h + 1]);
            mv[2 * h] = v128_xor(mv[2 * h], b0);
            mv[2 * h + 1] = v128_xor(mv[2 * h + 1], b1);
        }
    }
    let mut out = Merged([0u64; 8]);
    for r in 0..4 {
        v128_store(out.0.as_mut_ptr().add(2 * r) as *mut v128, mv[r]);
    }
    out
}

/// Merged lanes of `n >= 65` bytes at `p`, inlined into the entry points (see x86.rs).
#[inline(always)]
unsafe fn lanes(p: *const u8, n: usize, ks: u64) -> Merged {
    let m = (n - 1) / STRIPE;
    let ksv = i64x2_splat(ks as i64);
    if m < CHAINS {
        let q: [v128; 4] = core::array::from_fn(|r| ld(Q.as_ptr().add(2 * r) as *const u8));
        let mut mv = [i64x2_splat(0); 4];
        let mut one = |c: usize, src: *const u8, nv: v128| {
            for h in 0..2 {
                let w0 = v128_xor(ld(src.add(h * 32)), nv);
                let w1 = v128_xor(ld(src.add(h * 32 + 16)), nv);
                let a0 = i64x2_add(ld(CT.as_ptr().add(8 * c + 4 * h) as *const u8), ksv);
                let a1 = i64x2_add(ld(CT.as_ptr().add(8 * c + 4 * h + 2) as *const u8), ksv);
                let (a0, a1) = step2(a0, a1, w0, w1);
                let (b0, b1) = step2(a0, a1, q[2 * h], q[2 * h + 1]);
                mv[2 * h] = v128_xor(mv[2 * h], b0);
                mv[2 * h + 1] = v128_xor(mv[2 * h + 1], b1);
            }
        };
        for s in 0..m {
            one(s, p.add(s * 64), i64x2_splat(0));
        }
        one(m, p.add(n - 64), i64x2_splat(n as i64));
        let mut out = Merged([0u64; 8]);
        for r in 0..4 {
            v128_store(out.0.as_mut_ptr().add(2 * r) as *mut v128, mv[r]);
        }
        return out;
    }
    let mut w = Words([0u64; 64]);
    let st = w.0.as_mut_ptr();
    for j in 0..32 {
        v128_store(st.add(2 * j) as *mut v128, i64x2_add(ld(CT.as_ptr().add(2 * j) as *const u8), ksv));
    }
    let z = i64x2_splat(0);
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
    stripe(st, m % CHAINS, p.add(n - 64), i64x2_splat(n as i64));
    merge(st, CHAINS)
}

pub unsafe fn hash64(p: *const u8, n: usize, ks: u64) -> u64 {
    finish_lanes::<false>(&lanes(p, n, ks), ks, n) as u64
}
pub unsafe fn hash128(p: *const u8, n: usize, ks: u64) -> u128 {
    finish_lanes::<true>(&lanes(p, n, ks), ks, n)
}

/// Streaming: absorb `nblocks` whole leading blocks at `p` into `st`.
pub unsafe fn absorb(st: &mut Words, p: *const u8, nblocks: usize) {
    let z = i64x2_splat(0);
    for b in 0..nblocks {
        for c in 0..CHAINS {
            stripe(st.0.as_mut_ptr(), c, p.add(b * BLOCK + c * 64), z);
        }
    }
}

/// Streaming: the 128-bit hash from `st` (`spec::finish`).
pub unsafe fn finish(st: &Words, leading: *const u8, r: usize, closing: *const u8, n: usize, ks: u64) -> u128 {
    let mut w = Words(st.0);
    let z = i64x2_splat(0);
    for c in 0..r {
        stripe(w.0.as_mut_ptr(), c, leading.add(c * 64), z);
    }
    stripe(w.0.as_mut_ptr(), r % CHAINS, closing, i64x2_splat(n as i64));
    finish_lanes::<true>(&merge(w.0.as_ptr(), ((n - 1) / STRIPE + 1).min(CHAINS)), ks, n)
}
