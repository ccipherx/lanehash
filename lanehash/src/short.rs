//! The 0–64 byte path. Secrets are fixed constants (`KS` for multiply operands, `KF`
//! for final folds). The seed enters once, mixed as `fold(seed ^ SEED0, SEED1)`, with
//! the length in the last protected multiply (a raw seed there cancels against equal
//! input bytes: exact collisions); the length also enters the final fold. One
//! out-of-line function per length class, so each gets its own register allocation and
//! the public functions are tail jumps. Scalar 64x64->128 multiplies on every build.
use super::spec::{fold, ks};
use crate::constants::C;

const fn lo64(c: &[u8; 16]) -> u64 {
    u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])
}
const fn hi64(c: &[u8; 16]) -> u64 {
    u64::from_le_bytes([c[8], c[9], c[10], c[11], c[12], c[13], c[14], c[15]])
}
/// Multiply secrets, 8 words from `C[48..52)`.
pub const KS: [u64; 8] = [lo64(&C[48]), hi64(&C[48]), lo64(&C[49]), hi64(&C[49]), lo64(&C[50]), hi64(&C[50]), lo64(&C[51]), hi64(&C[51])];
/// Final-fold secrets, 4 words from `C[52..54)`.
pub const KF: [u64; 4] = [lo64(&C[52]), hi64(&C[52]), lo64(&C[53]), hi64(&C[53])];

/// Protected mum (wyhash/rapidhash form): `(a ^ lo, b ^ hi)` of the 128-bit product.
/// A zero operand keeps the other operand's information.
#[inline(always)]
fn mum(a: u64, b: u64) -> (u64, u64) {
    let r = (a as u128).wrapping_mul(b as u128);
    (a ^ r as u64, b ^ (r >> 64) as u64)
}
#[inline(always)]
fn le64(p: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(p[i..i + 8].try_into().unwrap())
}
#[inline(always)]
fn le32(p: &[u8], i: usize) -> u64 {
    u32::from_le_bytes(p[i..i + 4].try_into().unwrap()) as u64
}

/// Final folds: `lo` always, `hi` only for the 128-bit output; the length sits in
/// different operands of the two.
#[inline(always)]
fn finish<const HI: bool>(x: u64, y: u64, ln: u64) -> u128 {
    let lo = fold(x ^ (KF[0] ^ ln), y ^ KF[1]);
    if !HI {
        return lo as u128;
    }
    let hi = fold(x ^ KF[2], y ^ (KF[3] ^ ln));
    lo as u128 | (hi as u128) << 64
}

#[inline(always)]
fn class_le16<const HI: bool>(p: &[u8], seed: u64) -> u128 {
    let len = p.len();
    debug_assert!(len <= 16);
    let ln = len as u64;
    let (w0, w1) = match len {
        0 => (0, 0),
        1..=3 => ((p[0] as u64) | ((p[len / 2] as u64) << 8) | ((p[len - 1] as u64) << 16), 0),
        4..=8 => (le32(p, 0) | (le32(p, len - 4) << 32), 0),
        _ => (le64(p, 0), le64(p, len - 8)),
    };
    let (x, y) = mum(w0 ^ KS[0], w1 ^ (KS[1] ^ ln ^ ks(seed)));
    finish::<HI>(x, y, ln)
}

#[inline(always)]
fn class_le32<const HI: bool>(p: &[u8], seed: u64) -> u128 {
    let len = p.len();
    debug_assert!(len > 16 && len <= 32);
    let ln = len as u64;
    let (w0, w1, w2, w3) = (le64(p, 0), le64(p, 8), le64(p, len - 16), le64(p, len - 8));
    let (a0, b0) = mum(w0 ^ KS[0], w1 ^ KS[1]);
    let (a1, b1) = mum(w2 ^ KS[2], w3 ^ (KS[3] ^ ln ^ ks(seed)));
    finish::<HI>(a0 ^ b1, b0 ^ a1, ln)
}

#[inline(always)]
fn class_le64<const HI: bool>(p: &[u8], seed: u64) -> u128 {
    let len = p.len();
    debug_assert!(len > 32 && len <= 64);
    let ln = len as u64;
    let (w0, w1, w2, w3) = (le64(p, 0), le64(p, 8), le64(p, 16), le64(p, 24));
    let (w4, w5, w6, w7) = (le64(p, len - 32), le64(p, len - 24), le64(p, len - 16), le64(p, len - 8));
    let (a0, b0) = mum(w0 ^ KS[0], w1 ^ KS[1]);
    let (a1, b1) = mum(w2 ^ KS[2], w3 ^ KS[3]);
    let (a2, b2) = mum(w4 ^ KS[4], w5 ^ KS[5]);
    let (a3, b3) = mum(w6 ^ KS[6], w7 ^ (KS[7] ^ ln ^ ks(seed)));
    finish::<HI>(a0 ^ b1 ^ a2 ^ b3, b0 ^ a1 ^ b2 ^ a3, ln)
}

#[inline(never)]
pub fn short64_le16(p: &[u8], seed: u64) -> u64 {
    class_le16::<false>(p, seed) as u64
}
#[inline(never)]
pub fn short64_le32(p: &[u8], seed: u64) -> u64 {
    class_le32::<false>(p, seed) as u64
}
#[inline(never)]
pub fn short64_le64(p: &[u8], seed: u64) -> u64 {
    class_le64::<false>(p, seed) as u64
}
#[inline(never)]
pub fn short128_le16(p: &[u8], seed: u64) -> u128 {
    class_le16::<true>(p, seed)
}
#[inline(never)]
pub fn short128_le32(p: &[u8], seed: u64) -> u128 {
    class_le32::<true>(p, seed)
}
#[inline(never)]
pub fn short128_le64(p: &[u8], seed: u64) -> u128 {
    class_le64::<true>(p, seed)
}

/// 64-bit hash of `p.len() <= 64` bytes.
#[inline(always)]
pub fn short64(p: &[u8], seed: u64) -> u64 {
    if p.len() <= 16 {
        short64_le16(p, seed)
    } else if p.len() <= 32 {
        short64_le32(p, seed)
    } else {
        short64_le64(p, seed)
    }
}
/// 128-bit hash of `p.len() <= 64` bytes; its low half is [`short64`].
#[inline(always)]
pub fn short128(p: &[u8], seed: u64) -> u128 {
    if p.len() <= 16 {
        short128_le16(p, seed)
    } else if p.len() <= 32 {
        short128_le32(p, seed)
    } else {
        short128_le64(p, seed)
    }
}
