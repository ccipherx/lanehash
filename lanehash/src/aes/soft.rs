//! Portable fast path: T-table AES rounds for targets without AES instructions
//! and `force-fallback`.
//!
//! Blocks use two little-endian u64 halves; each round uses 16 T-table loads/XORs.
use crate::constants::{C, SBOX};
use crate::aes::lanes::{Blk, Lanes, State};
use crate::aes::spec::Block;

#[derive(Clone, Copy)]
pub struct Soft {
    pub lo: u64,
    pub hi: u64,
}

const fn xt(a: u8) -> u8 {
    (a << 1) ^ (((a >> 7) & 1) * 0x1b)
}

const fn tables() -> [[u32; 256]; 4] {
    let mut t = [[0u32; 256]; 4];
    let mut v = 0;
    while v < 256 {
        let s = SBOX[v];
        let (s2, s3) = (xt(s), xt(s) ^ s);
        // row contributions of a0: (2, 1, 1, 3); a1: (3, 2, 1, 1); a2: (1, 3, 2, 1); a3: (1, 1, 3, 2)
        t[0][v] = (s2 as u32) | (s as u32) << 8 | (s as u32) << 16 | (s3 as u32) << 24;
        t[1][v] = (s3 as u32) | (s2 as u32) << 8 | (s as u32) << 16 | (s as u32) << 24;
        t[2][v] = (s as u32) | (s3 as u32) << 8 | (s2 as u32) << 16 | (s as u32) << 24;
        t[3][v] = (s as u32) | (s as u32) << 8 | (s3 as u32) << 16 | (s2 as u32) << 24;
        v += 1;
    }
    t
}

pub static TE: [[u32; 256]; 4] = tables();

/// One AES round on the two-halves representation: `R(x, k)`.
#[inline(always)]
fn round(x: Soft, k: Soft) -> Soft {
    let c = [x.lo as u32, (x.lo >> 32) as u32, x.hi as u32, (x.hi >> 32) as u32];
    #[inline(always)]
    fn col(c: &[u32; 4], i: usize) -> u32 {
        TE[0][(c[i] & 0xff) as usize] ^ TE[1][((c[(i + 1) & 3] >> 8) & 0xff) as usize] ^ TE[2][((c[(i + 2) & 3] >> 16) & 0xff) as usize] ^ TE[3][(c[(i + 3) & 3] >> 24) as usize]
    }
    Soft { lo: (col(&c, 0) as u64 | (col(&c, 1) as u64) << 32) ^ k.lo, hi: (col(&c, 2) as u64 | (col(&c, 3) as u64) << 32) ^ k.hi }
}

/// The round on byte arrays, for the tests against `spec::aes_round`.
pub fn aes_round(x: Block, k: Block) -> Block {
    let r = round(Soft::from_bytes(&x), Soft::from_bytes(&k));
    let mut o = [0u8; 16];
    <Soft as Lanes>::store(r, core::slice::from_mut(&mut o));
    o
}

impl Blk for Soft {
    #[inline(always)]
    fn from_bytes(b: &[u8; 16]) -> Self {
        Soft { lo: u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]), hi: u64::from_le_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]) }
    }
    #[inline(always)]
    fn bcast(v: u64) -> Self {
        Soft { lo: v, hi: v }
    }
    #[inline(always)]
    fn round(self, k: Self) -> Self {
        round(self, k)
    }
    #[inline(always)]
    fn xor(self, o: Self) -> Self {
        Soft { lo: self.lo ^ o.lo, hi: self.hi ^ o.hi }
    }
    #[inline(always)]
    fn to_u128(self) -> u128 {
        (self.lo as u128) | ((self.hi as u128) << 64)
    }
}

impl Lanes for Soft {
    type B = Soft;
    const W: usize = 1;
    #[inline(always)]
    unsafe fn load(p: *const u8) -> Self {
        Soft { lo: u64::from_le_bytes(core::ptr::read_unaligned(p as *const [u8; 8])), hi: u64::from_le_bytes(core::ptr::read_unaligned(p.add(8) as *const [u8; 8])) }
    }
    #[inline(always)]
    fn init(seed: u64, first_lane: usize) -> Self {
        <Soft as Blk>::xor(<Soft as Blk>::bcast(seed), Soft::from_bytes(&C[first_lane]))
    }
    #[inline(always)]
    fn round(self, k: Self) -> Self {
        round(self, k)
    }
    #[inline(always)]
    fn xor(self, o: Self) -> Self {
        <Soft as Blk>::xor(self, o)
    }
    #[inline(always)]
    fn splat(b: &[u8; 16]) -> Self {
        Soft::from_bytes(b)
    }
    #[inline(always)]
    fn zero() -> Self {
        Soft { lo: 0, hi: 0 }
    }
    #[inline(always)]
    unsafe fn diff_partial(x: Self, prev: Self, _p: *const u8, _count: usize) -> (Self, Self) {
        debug_assert!(false, "W == 1 never has a partial register");
        (x, prev)
    }
    #[inline(always)]
    fn add(self, o: Self) -> Self {
        Soft { lo: self.lo.wrapping_add(o.lo), hi: self.hi.wrapping_add(o.hi) }
    }
    #[inline(always)]
    fn fold(self) -> Self {
        self
    }
    #[inline(always)]
    unsafe fn round_partial(self, _p: *const u8, _count: usize) -> Self {
        debug_assert!(false, "W == 1 never has a partial register");
        self
    }
    #[inline(always)]
    fn store(self, out: &mut [[u8; 16]]) {
        out[0][..8].copy_from_slice(&self.lo.to_le_bytes());
        out[0][8..].copy_from_slice(&self.hi.to_le_bytes());
    }
    #[inline(always)]
    fn from_blocks(inp: &[[u8; 16]]) -> Self {
        Soft::from_bytes(&inp[0])
    }
}

/// Lane path, `len > 64`.
pub unsafe fn hash128_lanes(p: *const u8, len: usize, seed: u64) -> u128 {
    regimes!(Soft, 1, p, len, seed)
}

pub unsafe fn absorb16_soft(state: &mut [[u8; 16]; 16], p: *const u8, nblocks: usize) {
    let mut st = State::<Soft, 16, 16>::from_blocks(state);
    st.absorb(p, nblocks);
    st.store(state);
}
pub unsafe fn finish16_soft(state: &[[u8; 16]; 16], c: *const u8, len: usize) -> u128 {
    State::<Soft, 16, 16>::from_blocks(state).finish(c, len)
}
pub unsafe fn batch4_l4_soft(ps: &[*const u8; 4], lens: &[usize; 4], seed: u64) -> [u128; 4] {
    crate::aes::lanes::lanes_hash_batch4::<Soft, 4, 4>(ps, lens, seed)
}
pub unsafe fn batch2_l8_soft(ps: &[*const u8; 2], lens: &[usize; 2], seed: u64) -> [u128; 2] {
    crate::aes::lanes::lanes_hash_batch2::<Soft, 8, 8>(ps, lens, seed)
}

pub fn hash128_soft(bytes: &[u8], seed: u64) -> u128 {
    if bytes.len() <= crate::aes::SHORT_MAX {
        crate::aes::short::short128(bytes, seed)
    } else {
        unsafe { hash128_lanes(bytes.as_ptr(), bytes.len(), seed) }
    }
}
