//! The portable definition: software AES round and the lane machine on `[u8; 16]`.
use crate::constants::{C, SBOX};
use crate::aes::lanes::{Blk, Lanes};

pub type Block = [u8; 16];

#[inline(always)]
fn xt(a: u8) -> u8 {
    (a << 1) ^ (((a >> 7) & 1) * 0x1b)
}

/// One AES encryption round: `MixColumns(ShiftRows(SubBytes(x))) ^ k`
/// (FIPS-197; byte i of the array is row i%4, column i/4; identical to x86 AESENC).
pub fn aes_round(x: Block, k: Block) -> Block {
    let mut s = [0u8; 16];
    for c in 0..4 {
        for r in 0..4 {
            s[r + 4 * c] = SBOX[x[r + 4 * ((c + r) & 3)] as usize];
        }
    }
    let mut o = [0u8; 16];
    for c in 0..4 {
        let (a0, a1, a2, a3) = (s[4 * c], s[4 * c + 1], s[4 * c + 2], s[4 * c + 3]);
        o[4 * c] = xt(a0) ^ xt(a1) ^ a1 ^ a2 ^ a3;
        o[4 * c + 1] = a0 ^ xt(a1) ^ xt(a2) ^ a2 ^ a3;
        o[4 * c + 2] = a0 ^ a1 ^ xt(a2) ^ xt(a3) ^ a3;
        o[4 * c + 3] = xt(a0) ^ a0 ^ a1 ^ a2 ^ xt(a3);
    }
    for i in 0..16 {
        o[i] ^= k[i];
    }
    o
}

#[inline(always)]
pub fn bcast(v: u64) -> Block {
    let b = v.to_le_bytes();
    let mut o = [0u8; 16];
    o[..8].copy_from_slice(&b);
    o[8..].copy_from_slice(&b);
    o
}

#[inline(always)]
fn xor(a: Block, b: Block) -> Block {
    let mut o = [0u8; 16];
    for i in 0..16 {
        o[i] = a[i] ^ b[i];
    }
    o
}

#[inline(always)]
fn add64(a: Block, b: Block) -> Block {
    let lo = u64::from_le_bytes(a[..8].try_into().unwrap()).wrapping_add(u64::from_le_bytes(b[..8].try_into().unwrap()));
    let hi = u64::from_le_bytes(a[8..].try_into().unwrap()).wrapping_add(u64::from_le_bytes(b[8..].try_into().unwrap()));
    let mut o = [0u8; 16];
    o[..8].copy_from_slice(&lo.to_le_bytes());
    o[8..].copy_from_slice(&hi.to_le_bytes());
    o
}

impl Blk for Block {
    #[inline(always)]
    fn from_bytes(b: &[u8; 16]) -> Self {
        *b
    }
    #[inline(always)]
    fn bcast(v: u64) -> Self {
        bcast(v)
    }
    #[inline(always)]
    fn round(self, k: Self) -> Self {
        aes_round(self, k)
    }
    #[inline(always)]
    fn xor(self, o: Self) -> Self {
        xor(self, o)
    }
    #[inline(always)]
    fn to_u128(self) -> u128 {
        u128::from_le_bytes(self)
    }
}

impl Lanes for Block {
    type B = Block;
    const W: usize = 1;
    #[inline(always)]
    unsafe fn load(p: *const u8) -> Self {
        core::ptr::read_unaligned(p as *const Block)
    }
    #[inline(always)]
    fn init(seed: u64, first_lane: usize) -> Self {
        xor(bcast(seed), C[first_lane])
    }
    #[inline(always)]
    fn round(self, k: Self) -> Self {
        aes_round(self, k)
    }
    #[inline(always)]
    fn xor(self, o: Self) -> Self {
        xor(self, o)
    }
    #[inline(always)]
    fn splat(b: &[u8; 16]) -> Self {
        *b
    }
    #[inline(always)]
    fn zero() -> Self {
        [0u8; 16]
    }
    #[inline(always)]
    unsafe fn diff_partial(x: Self, prev: Self, _p: *const u8, _count: usize) -> (Self, Self) {
        debug_assert!(false, "W == 1 never has a partial register");
        (x, prev)
    }
    #[inline(always)]
    fn add(self, o: Self) -> Self {
        add64(self, o)
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
        out[0] = self;
    }
    #[inline(always)]
    fn from_blocks(inp: &[[u8; 16]]) -> Self {
        inp[0]
    }
}

/// Lane path on the portable backend, `len > 64`.
pub unsafe fn hash128_lanes(p: *const u8, len: usize, seed: u64) -> u128 {
    regimes!(Block, 1, p, len, seed)
}

pub unsafe fn batch4_l4_spec(ps: &[*const u8; 4], lens: &[usize; 4], seed: u64) -> [u128; 4] {
    crate::aes::lanes::lanes_hash_batch4::<Block, 4, 4>(ps, lens, seed)
}
pub unsafe fn batch2_l8_spec(ps: &[*const u8; 2], lens: &[usize; 2], seed: u64) -> [u128; 2] {
    crate::aes::lanes::lanes_hash_batch2::<Block, 8, 8>(ps, lens, seed)
}

/// The complete portable hash (the specification).
pub fn hash128_spec(bytes: &[u8], seed: u64) -> u128 {
    if bytes.len() <= crate::aes::SHORT_MAX {
        crate::aes::short::short128(bytes, seed)
    } else {
        unsafe { hash128_lanes(bytes.as_ptr(), bytes.len(), seed) }
    }
}
