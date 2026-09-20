//! 0–64 byte paths: scalar 64x64->128 multiplies, shared by every build.
use crate::constants::C;

const fn lo64(c: &[u8; 16]) -> u64 {
    u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])
}
const K0: u64 = lo64(&C[27]);
const K1: u64 = lo64(&C[28]) | 1;
const KS: [u64; 8] = [lo64(&C[16]), lo64(&C[17]), lo64(&C[18]), lo64(&C[19]), lo64(&C[20]), lo64(&C[21]), lo64(&C[22]), lo64(&C[23])];
const TOP: u64 = 1 << 63;

/// 128-bit product split.
#[inline(always)]
fn mul128(a: u64, b: u64) -> (u64, u64) {
    let r = (a as u128).wrapping_mul(b as u128);
    (r as u64, (r >> 64) as u64)
}

/// Unprotected fold `lo ^ hi`, for the final step where no operand is raw input.
#[inline(always)]
pub fn fold(a: u64, b: u64) -> u64 {
    let (lo, hi) = mul128(a, b);
    lo ^ hi
}

/// Protected mum `(a ^ lo, b ^ hi)`: a zero operand keeps the other's information; the
/// final unprotected fold removes the linear terms again.
#[inline(always)]
pub fn mum(a: u64, b: u64) -> (u64, u64) {
    let (lo, hi) = mul128(a, b);
    (a ^ lo, b ^ hi)
}

/// Per-seed secrets: rotations of the mixed seed XORed with distinct constants, top bit
/// forced so none is 0 or 1.
#[inline(always)]
pub fn secrets(seed: u64) -> [u64; 8] {
    let ks = fold(seed ^ K0, K1);
    let mut k = [0u64; 8];
    let mut j = 0;
    while j < 8 {
        k[j] = (ks.rotate_left((9 * j) as u32) ^ KS[j]) | TOP;
        j += 1;
    }
    k
}

#[inline(always)]
fn le64(p: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(p[i..i + 8].try_into().unwrap())
}
#[inline(always)]
fn le32(p: &[u8], i: usize) -> u64 {
    u32::from_le_bytes(p[i..i + 4].try_into().unwrap()) as u64
}

/// Reference short path (`hash64` / `hash128` use the per-class functions below).
#[inline(always)]
pub fn short128(p: &[u8], seed: u64) -> u128 {
    short128_with(p, &secrets(seed))
}

/// `short128` with precomputed secrets (used by the batch API).
#[inline(always)]
pub fn short128_with(p: &[u8], k: &[u64; 8]) -> u128 {
    let len = p.len();
    if len <= 16 {
        class_le16::<true>(p, k)
    } else if len <= 32 {
        class_le32::<true>(p, k)
    } else {
        class_le64::<true>(p, k)
    }
}

// One out-of-line function per length class and width: its own register allocation
// (one inlined function needed six callee-saved registers on every path), tail jumps
// from `hash64` / `hash128`, only the secrets its class uses.
#[inline(never)]
pub fn short64_le16(p: &[u8], seed: u64) -> u64 {
    class_le16::<false>(p, &secrets(seed)) as u64
}
#[inline(never)]
pub fn short64_le32(p: &[u8], seed: u64) -> u64 {
    class_le32::<false>(p, &secrets(seed)) as u64
}
#[inline(never)]
pub fn short64_le64(p: &[u8], seed: u64) -> u64 {
    class_le64::<false>(p, &secrets(seed)) as u64
}
#[inline(never)]
pub fn short128_le16(p: &[u8], seed: u64) -> u128 {
    class_le16::<true>(p, &secrets(seed))
}
#[inline(never)]
pub fn short128_le32(p: &[u8], seed: u64) -> u128 {
    class_le32::<true>(p, &secrets(seed))
}
#[inline(never)]
pub fn short128_le64(p: &[u8], seed: u64) -> u128 {
    class_le64::<true>(p, &secrets(seed))
}

/// Final folds: `lo` always, `hi` only for the 128-bit output.
#[inline(always)]
fn finish<const HI: bool>(x: u64, y: u64, k: &[u64; 8], ln: u64) -> u128 {
    let lo = fold(x ^ k[2] ^ ln, y ^ k[3]);
    if !HI {
        return lo as u128;
    }
    let hi = fold(x ^ k[4], y ^ k[5] ^ ln);
    (lo as u128) | ((hi as u128) << 64)
}

#[inline(always)]
fn class_le16<const HI: bool>(p: &[u8], k: &[u64; 8]) -> u128 {
    let len = p.len();
    debug_assert!(len <= 16);
    let ln = len as u64;
    let (w0, w1) = match len {
        0 => (0, 0),
        1..=3 => ((p[0] as u64) | ((p[len / 2] as u64) << 8) | ((p[len - 1] as u64) << 16), 0),
        4..=8 => (le32(p, 0) | (le32(p, len - 4) << 32), 0),
        _ => (le64(p, 0), le64(p, len - 8)),
    };
    let (x, y) = mum(w0 ^ k[0], w1 ^ k[1] ^ ln);
    finish::<HI>(x, y, k, ln)
}

#[inline(always)]
fn class_le32<const HI: bool>(p: &[u8], k: &[u64; 8]) -> u128 {
    let len = p.len();
    debug_assert!(len > 16 && len <= 32);
    let ln = len as u64;
    let (w0, w1, w2, w3) = (le64(p, 0), le64(p, 8), le64(p, len - 16), le64(p, len - 8));
    let (a0, b0) = mum(w0 ^ k[0], w1 ^ k[1]);
    let (a1, b1) = mum(w2 ^ k[2], w3 ^ k[3] ^ ln);
    finish::<HI>(a0 ^ b1, b0 ^ a1, k, ln)
}

#[inline(always)]
fn class_le64<const HI: bool>(p: &[u8], k: &[u64; 8]) -> u128 {
    let len = p.len();
    debug_assert!(len > 32 && len <= crate::aes::SHORT_MAX);
    let ln = len as u64;
    let (w0, w1, w2, w3) = (le64(p, 0), le64(p, 8), le64(p, 16), le64(p, 24));
    let (w4, w5, w6, w7) = (le64(p, len - 32), le64(p, len - 24), le64(p, len - 16), le64(p, len - 8));
    let (a0, b0) = mum(w0 ^ k[0], w1 ^ k[1]);
    let (a1, b1) = mum(w2 ^ k[2], w3 ^ k[3]);
    let (a2, b2) = mum(w4 ^ k[4], w5 ^ k[5]);
    let (a3, b3) = mum(w6 ^ k[6], w7 ^ k[7] ^ ln);
    finish::<HI>(a0 ^ b1 ^ a2 ^ b3, b0 ^ a1 ^ b2 ^ a3, k, ln)
}
