//! Generic lane machine, instantiated by every backend.
use crate::constants::C;

/// A 128-bit block on some backend.
pub trait Blk: Copy {
    fn from_bytes(b: &[u8; 16]) -> Self;
    fn bcast(v: u64) -> Self;
    fn round(self, k: Self) -> Self;
    fn xor(self, o: Self) -> Self;
    fn to_u128(self) -> u128;
}

/// A register holding `W` consecutive lanes.
pub trait Lanes: Copy {
    type B: Blk;
    const W: usize;

    /// Load `16 * W` bytes, unaligned; the caller guarantees bounds.
    unsafe fn load(p: *const u8) -> Self;

    /// Lanes `first_lane..first_lane + W` set to `bcast(seed) ^ C[i]`.
    fn init(seed: u64, first_lane: usize) -> Self;

    fn round(self, k: Self) -> Self;
    fn xor(self, o: Self) -> Self;

    /// One 16-byte constant in every lane.
    fn splat(b: &[u8; 16]) -> Self;

    fn zero() -> Self;

    /// Differential step `x = R(x, b ^ prev); prev = b` on lanes `0..count`, `b` from `p`.
    unsafe fn diff_partial(
        x: Self,
        prev: Self,
        p: *const u8,
        count: usize,
    ) -> (Self, Self);

    fn add(self, o: Self) -> Self;

    /// Sum of the `W` lanes.
    fn fold(self) -> Self::B;

    /// `t = R(t ^ b, b)` on lanes `0..count` only, `b` from `p`.
    unsafe fn round_partial(self, p: *const u8, count: usize) -> Self;

    /// Store the `W` lanes, 16 bytes each.
    fn store(self, out: &mut [[u8; 16]]);

    #[inline(always)]
    unsafe fn prefetch(_p: *const u8) {}

    fn from_blocks(inp: &[[u8; 16]]) -> Self;
}

pub const FINAL_ROUNDS: usize = 3;
/// Constant-key rounds per lane after its closing block.
pub const CLOSE_ROUNDS: usize = 3;
/// Software prefetch distance in the bulk loop, bytes.
pub const PREFETCH_DIST: usize = 1024;

/// Above this the EVEX backend switches from differential to sandwich form, which
/// streams better from L3/DRAM. Tuned for a 1 MiB L2.
pub const BULK_SANDWICH_MIN: usize = 1 << 19;
pub const PREFETCH_MIN_LEN: usize = 1 << 16;

#[inline(always)]
pub fn finalize<B: Blk>(mut u: B, len: usize) -> B {
    u = u.round(B::from_bytes(&C[24]).xor(B::bcast(len as u64)));
    let mut r = 1;
    while r < FINAL_ROUNDS {
        u = u.round(B::from_bytes(&C[24 + r]));
        r += 1;
    }
    u
}

/// Lane state: `NR` registers of `W` lanes = `L` lanes.
#[derive(Clone, Copy)]
pub struct State<V: Lanes, const L: usize, const NR: usize> {
    pub t: [V; NR],
}

impl<V: Lanes, const L: usize, const NR: usize> State<V, L, NR> {
    #[inline(always)]
    pub fn new(seed: u64) -> Self {
        debug_assert_eq!(NR * V::W, L);
        let mut t = [V::init(seed, 0); NR];
        for i in 1..NR {
            t[i] = V::init(seed, i * V::W);
        }
        State { t }
    }

    /// `nblocks` regular blocks at `p`, block `j` into lane `j mod L` as data and key:
    /// `t = R(t ^ b, b)`. Every earlier call must have absorbed a multiple of `L` blocks.
    #[inline(always)]
    pub unsafe fn absorb(&mut self, mut p: *const u8, nblocks: usize) {
        let full = nblocks / L;
        let r = nblocks % L;

        for _ in 0..full {
            for i in 0..NR {
                let b = V::load(p.add(16 * V::W * i));
                self.t[i] = self.t[i].xor(b).round(b);
            }
            p = p.add(16 * L);
        }

        let rr = r / V::W;
        let rp = r % V::W;

        for i in 0..NR {
            if i < rr {
                let b = V::load(p.add(16 * V::W * i));
                self.t[i] = self.t[i].xor(b).round(b);
            } else if i == rr && rp != 0 {
                self.t[i] = self.t[i].round_partial(p.add(16 * V::W * i), rp);
            }
        }
    }

    #[inline(always)]
    pub unsafe fn step(&mut self, p: *const u8) {
        for i in 0..NR {
            let b = V::load(p.add(16 * V::W * i));
            self.t[i] = self.t[i].xor(b).round(b);
        }
    }

    /// The closing region (`16 * L` bytes at `c`, XORed with the broadcast length) as
    /// data and key, `CLOSE_ROUNDS` constant-key rounds per lane, merge, finalise.
    #[inline(always)]
    pub unsafe fn finish(mut self, c: *const u8, len: usize) -> u128 {
        let ln = V::splat(&crate::aes::spec::bcast(len as u64));

        for i in 0..NR {
            let b = V::load(c.add(16 * V::W * i)).xor(ln);
            self.t[i] = self.t[i].xor(b).round(b);
        }

        let mut r = 0;
        while r < CLOSE_ROUNDS {
            let k = V::splat(&C[40 + r]);
            for i in 0..NR {
                self.t[i] = self.t[i].round(k);
            }
            r += 1;
        }

        let mut acc = self.t[0];
        for i in 1..NR {
            acc = acc.add(self.t[i]);
        }
        finalize(acc.fold(), len).to_u128()
    }

    #[inline(always)]
    pub fn store(&self, out: &mut [[u8; 16]; L]) {
        for i in 0..NR {
            self.t[i].store(&mut out[i * V::W..(i + 1) * V::W]);
        }
    }

    #[inline(always)]
    pub fn from_blocks(inp: &[[u8; 16]; L]) -> Self {
        let mut t = [V::from_blocks(&inp[0..V::W]); NR];
        for i in 1..NR {
            t[i] = V::from_blocks(&inp[i * V::W..(i + 1) * V::W]);
        }
        State { t }
    }
}

/// One-shot lane hash for `len >= 16 * L`.
#[inline(always)]
pub unsafe fn lanes_hash<V: Lanes, const L: usize, const NR: usize>(
    p: *const u8,
    len: usize,
    seed: u64,
) -> u128 {
    debug_assert!(len >= 16 * L);
    let n = (len + 15) / 16;
    let m = n - L;
    let mut st = State::<V, L, NR>::new(seed);
    st.absorb(p, m);
    st.finish(p.add(len - 16 * L), len)
}

/// `lanes_hash` for `L = 4`, `len <= 256`: at most three full steps, written out.
#[inline(always)]
pub unsafe fn lanes_hash_l4<V: Lanes, const NR: usize>(
    p: *const u8,
    len: usize,
    seed: u64,
) -> u128 {
    debug_assert!(len > 64 && len <= 256);
    let m = (len + 15) / 16 - 4;
    let full = m / 4;
    let mut st = State::<V, 4, NR>::new(seed);
    let mut q = p;

    if full >= 1 {
        st.step(q);
        q = q.add(64);
    }
    if full >= 2 {
        st.step(q);
        q = q.add(64);
    }
    if full >= 3 {
        st.step(q);
        q = q.add(64);
    }

    st.absorb(q, m % 4);
    st.finish(p.add(len - 64), len)
}

/// Regime selection shared by all backends; `$v` is the `Lanes` type, `$w` its width.
macro_rules! regimes {
    ($v:ty, $w:expr, $p:expr, $len:expr, $seed:expr) => {{
        if $len <= $crate::aes::L4_MAX {
            $crate::aes::lanes::lanes_hash_l4::<$v, { 4 / $w }>($p, $len, $seed)
        } else if $len <= $crate::aes::L8_MAX {
            $crate::aes::lanes::lanes_hash::<$v, 8, { 8 / $w }>($p, $len, $seed)
        } else {
            $crate::aes::lanes::lanes_hash::<$v, 16, { 16 / $w }>($p, $len, $seed)
        }
    }};
}

/// `lanes_hash` in differential form: with `u_k = s_k ^ b_k` the sandwich chain
/// `s_k = R(s_{k-1} ^ b_k, b_k)` is `u_k = MCSRSB(u_{k-1} ^ b_{k-1} ^ b_k)`, and with
/// `x_k = u_k ^ b_k ^ b_{k+1}` it is `x_k = R(x_{k-1}, b_k ^ b_{k+1})`: the XOR is off the
/// dependency chain. Needs at least `L` regular blocks.
#[inline(always)]
pub unsafe fn lanes_hash_diff<V: Lanes, const L: usize, const NR: usize>(
    p: *const u8,
    len: usize,
    seed: u64,
) -> u128 {
    let n = (len + 15) / 16;
    let m = n - L;
    debug_assert!(m >= L);

    let (mut x, mut prev) = diff_init::<V, L, NR>(p, seed);
    let mut q = p.add(16 * L);
    let full = m / L;
    let r = m % L;

    if len >= PREFETCH_MIN_LEN {
        for _ in 1..full {
            V::prefetch(q.add(PREFETCH_DIST));
            V::prefetch(q.add(PREFETCH_DIST + 64));
            V::prefetch(q.add(PREFETCH_DIST + 128));
            V::prefetch(q.add(PREFETCH_DIST + 192));
            diff_step::<V, L, NR>(&mut x, &mut prev, q);
            q = q.add(16 * L);
        }
    } else {
        for _ in 1..full {
            diff_step::<V, L, NR>(&mut x, &mut prev, q);
            q = q.add(16 * L);
        }
    }

    diff_tail::<V, L, NR>(x, prev, q, r, p, len)
}

/// First differential step: `x_0 = t0 ^ b_1`, `prev = b_1` for every lane.
#[inline(always)]
pub unsafe fn diff_init<V: Lanes, const L: usize, const NR: usize>(
    p: *const u8,
    seed: u64,
) -> ([V; NR], [V; NR]) {
    let mut x = [V::zero(); NR];
    let mut prev = [V::zero(); NR];

    for i in 0..NR {
        let b = V::load(p.add(16 * V::W * i));
        x[i] = V::init(seed, i * V::W).xor(b);
        prev[i] = b;
    }

    (x, prev)
}

#[inline(always)]
pub unsafe fn diff_step<V: Lanes, const L: usize, const NR: usize>(
    x: &mut [V; NR],
    prev: &mut [V; NR],
    q: *const u8,
) {
    for i in 0..NR {
        let b = V::load(q.add(16 * V::W * i));
        x[i] = x[i].round(b.xor(prev[i]));
        prev[i] = b;
    }
}

#[inline(always)]
pub unsafe fn diff_tail<V: Lanes, const L: usize, const NR: usize>(
    mut x: [V; NR],
    mut prev: [V; NR],
    q: *const u8,
    r: usize,
    p: *const u8,
    len: usize,
) -> u128 {
    let rr = r / V::W;
    let rp = r % V::W;

    for i in 0..NR {
        if i < rr {
            let b = V::load(q.add(16 * V::W * i));
            x[i] = x[i].round(b.xor(prev[i]));
            prev[i] = b;
        } else if i == rr && rp != 0 {
            let (nx, np) =
                V::diff_partial(x[i], prev[i], q.add(16 * V::W * i), rp);
            x[i] = nx;
            prev[i] = np;
        }
    }

    let ln = V::splat(&crate::aes::spec::bcast(len as u64));
    let c = p.add(len - 16 * L);
    let z = V::zero();

    for i in 0..NR {
        let e = V::load(c.add(16 * V::W * i)).xor(ln);
        let xn = x[i].round(e.xor(prev[i]));
        x[i] = xn.round(z).xor(e);
    }

    let mut r = 0;
    while r < CLOSE_ROUNDS {
        let k = V::splat(&C[40 + r]);
        for i in 0..NR {
            x[i] = x[i].round(k);
        }
        r += 1;
    }

    let mut acc = x[0];
    for i in 1..NR {
        acc = acc.add(x[i]);
    }

    finalize(acc.fold(), len).to_u128()
}

// Batched kernels: `K` inputs of one regime interleaved so `K * L` chains are in flight;
// each result equals the one-shot function. States are separate variables, not an
// array: an indexed `[State; K]` tail stayed a runtime loop with the state in memory.
/// Sandwich form, four inputs with `len > 64` and the same `L`.
#[inline(always)]
pub unsafe fn lanes_hash_batch4<V: Lanes, const L: usize, const NR: usize>(
    ps: &[*const u8; 4],
    lens: &[usize; 4],
    seed: u64,
) -> [u128; 4] {
    let init = State::<V, L, NR>::new(seed);
    let (mut s0, mut s1, mut s2, mut s3) = (init, init, init, init);

    let (m0, m1, m2, m3) = (
        (lens[0] + 15) / 16 - L,
        (lens[1] + 15) / 16 - L,
        (lens[2] + 15) / 16 - L,
        (lens[3] + 15) / 16 - L,
    );
    let common = (m0 / L).min(m1 / L).min(m2 / L).min(m3 / L);
    let (mut q0, mut q1, mut q2, mut q3) = (ps[0], ps[1], ps[2], ps[3]);

    for _ in 0..common {
        s0.step(q0);
        s1.step(q1);
        s2.step(q2);
        s3.step(q3);
        q0 = q0.add(16 * L);
        q1 = q1.add(16 * L);
        q2 = q2.add(16 * L);
        q3 = q3.add(16 * L);
    }

    s0.absorb(q0, m0 - common * L);
    s1.absorb(q1, m1 - common * L);
    s2.absorb(q2, m2 - common * L);
    s3.absorb(q3, m3 - common * L);

    [
        s0.finish(ps[0].add(lens[0] - 16 * L), lens[0]),
        s1.finish(ps[1].add(lens[1] - 16 * L), lens[1]),
        s2.finish(ps[2].add(lens[2] - 16 * L), lens[2]),
        s3.finish(ps[3].add(lens[3] - 16 * L), lens[3]),
    ]
}

/// Sandwich form, two inputs.
#[inline(always)]
pub unsafe fn lanes_hash_batch2<V: Lanes, const L: usize, const NR: usize>(
    ps: &[*const u8; 2],
    lens: &[usize; 2],
    seed: u64,
) -> [u128; 2] {
    let init = State::<V, L, NR>::new(seed);
    let (mut s0, mut s1) = (init, init);

    let (m0, m1) = (
        (lens[0] + 15) / 16 - L,
        (lens[1] + 15) / 16 - L,
    );
    let common = (m0 / L).min(m1 / L);
    let (mut q0, mut q1) = (ps[0], ps[1]);

    for _ in 0..common {
        s0.step(q0);
        s1.step(q1);
        q0 = q0.add(16 * L);
        q1 = q1.add(16 * L);
    }

    s0.absorb(q0, m0 - common * L);
    s1.absorb(q1, m1 - common * L);

    [
        s0.finish(ps[0].add(lens[0] - 16 * L), lens[0]),
        s1.finish(ps[1].add(lens[1] - 16 * L), lens[1]),
    ]
}

/// Differential form, two inputs with at least `L` regular blocks each.
#[inline(always)]
pub unsafe fn lanes_hash_diff_batch2<V: Lanes, const L: usize, const NR: usize>(
    ps: &[*const u8; 2],
    lens: &[usize; 2],
    seed: u64,
) -> [u128; 2] {
    let (m0, m1) = (
        (lens[0] + 15) / 16 - L,
        (lens[1] + 15) / 16 - L,
    );
    debug_assert!(m0 >= L && m1 >= L);

    let common = (m0 / L).min(m1 / L);
    let (mut x0, mut p0) = diff_init::<V, L, NR>(ps[0], seed);
    let (mut x1, mut p1) = diff_init::<V, L, NR>(ps[1], seed);
    let (mut q0, mut q1) = (ps[0].add(16 * L), ps[1].add(16 * L));

    for _ in 1..common {
        diff_step::<V, L, NR>(&mut x0, &mut p0, q0);
        diff_step::<V, L, NR>(&mut x1, &mut p1, q1);
        q0 = q0.add(16 * L);
        q1 = q1.add(16 * L);
    }

    for _ in common..m0 / L {
        diff_step::<V, L, NR>(&mut x0, &mut p0, q0);
        q0 = q0.add(16 * L);
    }

    for _ in common..m1 / L {
        diff_step::<V, L, NR>(&mut x1, &mut p1, q1);
        q1 = q1.add(16 * L);
    }

    [
        diff_tail::<V, L, NR>(x0, p0, q0, m0 % L, ps[0], lens[0]),
        diff_tail::<V, L, NR>(x1, p1, q1, m1 % L, ps[1], lens[1]),
    ]
}