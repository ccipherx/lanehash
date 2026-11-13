//! The 0–64 byte path: protected 64x64->128 multiplies over the input words, one
//! unprotected fold per output half.
//!
//! Every multiply operand is an input word XORed with a secret derived from the seed
//! (`spec::keys`): `ka` for first operands, `kb` for second operands, rotated by a
//! different amount for each multiply of a class. Three rules, each from a collision that
//! existed while only the last multiply carried the seed:
//! * no operand is `word ^ public constant`: `mum(a, 1) = (0, 1)` for every `a`, so a
//!   public second operand let 8 chosen bytes erase 8 others under every seed;
//! * the secrets of two multiplies never differ by a public constant, or the two can be
//!   fed equal operands and their outputs cancel in the XOR that combines them;
//! * nothing the caller chooses reaches an operand of the final fold unmixed.
//! The seed is mixed by a multiply before it meets input (a raw seed cancels against
//! equal input bytes), and the length enters the last multiply and the final fold.
//! One out-of-line function per length class, so each gets its own register allocation
//! and the public functions are tail jumps. Scalar multiplies on every build.
use super::spec::{fold, keys};
use crate::constants::C;

const fn lo64(c: &[u8; 16]) -> u64 {
    u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])
}
const fn hi64(c: &[u8; 16]) -> u64 {
    u64::from_le_bytes([c[8], c[9], c[10], c[11], c[12], c[13], c[14], c[15]])
}
/// Constants XORed into the two seed secrets (`spec::keys`), from `C[48]`.
pub const KS: [u64; 2] = [lo64(&C[48]), hi64(&C[48])];
/// Final-fold constants, 4 words from `C[52..54)`.
pub const KF: [u64; 4] = [lo64(&C[52]), hi64(&C[52]), lo64(&C[53]), hi64(&C[53])];
/// Rotation of both secrets for multiply `j` of a class. Distinct, so no two operand
/// secrets differ by a constant: `rotl(k, r) ^ rotl(k, s)` is unknown without `k`.
pub const ROT: [u32; 4] = [0, 19, 38, 57];

/// Protected mum (wyhash/rapidhash form): `(a ^ lo, b ^ hi)` of the 128-bit product.
/// A zero operand keeps the other operand's information. `b == 1` erases `a`, which is
/// why `b` always carries a secret.
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

/// The two words that carry all of `p`, `p.len() <= 16`: three bytes spread (1 to 3
/// bytes), `le32(p, 0) | le32(p, len - 4) << 32` and zero (4 to 8), or
/// `le64(p, 0)` and `le64(p, len - 8)` (9 to 16). The last two cases share one
/// branch-free form of four 32-bit reads: key sets mix lengths around 8, and a branch
/// there mispredicts about every second key (6 cycles per hash on mixed 1 to 16 bytes).
#[inline(always)]
pub fn words16(p: &[u8]) -> (u64, u64) {
    let len = p.len();
    debug_assert!(len <= 16);
    if len >= 4 {
        // `over` is `len - 8` above 8 bytes and 0 otherwise
        let long = ((len > 8) as usize).wrapping_neg();
        let over = len.wrapping_sub(8) & long;
        // SAFETY: 4 <= len, and `over` is 0 or `len - 8`, so the four offsets
        // 0, len - 4 - over, over and len - 4 all leave 4 bytes inside `p`. Unchecked
        // because the compiler cannot see that through the mask: three bounds branches
        // and a frame for their panics cost 3 cycles at every fixed length.
        let rd = |at: usize| unsafe { u32::from_le(core::ptr::read_unaligned(p.as_ptr().add(at) as *const u32)) as u64 };
        debug_assert!(len - 4 - over + 4 <= len && over + 4 <= len);
        let w0 = rd(0) | rd(len - 4 - over) << 32;
        let w1 = (rd(over) | rd(len - 4) << 32) & long as u64;
        (w0, w1)
    } else if len > 0 {
        ((p[0] as u64) | ((p[len / 2] as u64) << 8) | ((p[len - 1] as u64) << 16), 0)
    } else {
        (0, 0)
    }
}

#[inline(always)]
fn class_le16<const HI: bool>(p: &[u8], ka: u64, kb: u64) -> u128 {
    let ln = p.len() as u64;
    let (w0, w1) = words16(p);
    let (x, y) = mum(w0 ^ ka, w1 ^ (kb ^ ln));
    finish::<HI>(x, y, ln)
}

#[inline(always)]
fn class_le32<const HI: bool>(p: &[u8], ka: u64, kb: u64) -> u128 {
    let len = p.len();
    debug_assert!(len > 16 && len <= 32);
    let ln = len as u64;
    let (w0, w1, w2, w3) = (le64(p, 0), le64(p, 8), le64(p, len - 16), le64(p, len - 8));
    let (x0, y0) = mum(w0 ^ ka, w1 ^ kb);
    let (x1, y1) = mum(w2 ^ ka.rotate_left(ROT[1]), w3 ^ (kb.rotate_left(ROT[1]) ^ ln));
    finish::<HI>(x0 ^ y1, y0 ^ x1, ln)
}

#[inline(always)]
fn class_le64<const HI: bool>(p: &[u8], ka: u64, kb: u64) -> u128 {
    let len = p.len();
    debug_assert!(len > 32 && len <= 64);
    let ln = len as u64;
    let (w0, w1, w2, w3) = (le64(p, 0), le64(p, 8), le64(p, 16), le64(p, 24));
    let (w4, w5, w6, w7) = (le64(p, len - 32), le64(p, len - 24), le64(p, len - 16), le64(p, len - 8));
    let (x0, y0) = mum(w0 ^ ka, w1 ^ kb);
    let (x1, y1) = mum(w2 ^ ka.rotate_left(ROT[1]), w3 ^ kb.rotate_left(ROT[1]));
    let (x2, y2) = mum(w4 ^ ka.rotate_left(ROT[2]), w5 ^ kb.rotate_left(ROT[2]));
    let (x3, y3) = mum(w6 ^ ka.rotate_left(ROT[3]), w7 ^ (kb.rotate_left(ROT[3]) ^ ln));
    finish::<HI>(x0 ^ y1 ^ x2 ^ y3, y0 ^ x1 ^ y2 ^ x3, ln)
}

/// The class functions inlined, for inputs whose length the caller knows at compile
/// time: the class and every load offset fold to constants.
#[inline(always)]
pub fn short128_inline<const HI: bool>(p: &[u8], ka: u64, kb: u64) -> u128 {
    if p.len() <= 16 {
        class_le16::<HI>(p, ka, kb)
    } else if p.len() <= 32 {
        class_le32::<HI>(p, ka, kb)
    } else {
        class_le64::<HI>(p, ka, kb)
    }
}

// Out of line per class and width, taking the secrets: callers that hold them (the map
// hasher, `Keyed`) skip the seed multiply.
#[inline(never)]
pub fn short64_le16(p: &[u8], ka: u64, kb: u64) -> u64 {
    class_le16::<false>(p, ka, kb) as u64
}
#[inline(never)]
pub fn short64_le32(p: &[u8], ka: u64, kb: u64) -> u64 {
    class_le32::<false>(p, ka, kb) as u64
}
#[inline(never)]
pub fn short64_le64(p: &[u8], ka: u64, kb: u64) -> u64 {
    class_le64::<false>(p, ka, kb) as u64
}
#[inline(never)]
pub fn short128_le16(p: &[u8], ka: u64, kb: u64) -> u128 {
    class_le16::<true>(p, ka, kb)
}
#[inline(never)]
pub fn short128_le32(p: &[u8], ka: u64, kb: u64) -> u128 {
    class_le32::<true>(p, ka, kb)
}
#[inline(never)]
pub fn short128_le64(p: &[u8], ka: u64, kb: u64) -> u128 {
    class_le64::<true>(p, ka, kb)
}

/// 64-bit hash of `p.len() <= 64` bytes under the secrets `(ka, kb)`.
#[inline(always)]
pub fn short64_keyed(p: &[u8], ka: u64, kb: u64) -> u64 {
    if p.len() <= 16 {
        short64_le16(p, ka, kb)
    } else if p.len() <= 32 {
        short64_le32(p, ka, kb)
    } else {
        short64_le64(p, ka, kb)
    }
}
/// 128-bit hash of `p.len() <= 64` bytes under the secrets; its low half is
/// [`short64_keyed`].
#[inline(always)]
pub fn short128_keyed(p: &[u8], ka: u64, kb: u64) -> u128 {
    if p.len() <= 16 {
        short128_le16(p, ka, kb)
    } else if p.len() <= 32 {
        short128_le32(p, ka, kb)
    } else {
        short128_le64(p, ka, kb)
    }
}

/// 64-bit hash of `p.len() <= 64` bytes.
#[inline(always)]
pub fn short64(p: &[u8], seed: u64) -> u64 {
    let (ka, kb, _) = keys(seed);
    short64_keyed(p, ka, kb)
}
/// 128-bit hash of `p.len() <= 64` bytes; its low half is [`short64`].
#[inline(always)]
pub fn short128(p: &[u8], seed: u64) -> u128 {
    let (ka, kb, _) = keys(seed);
    short128_keyed(p, ka, kb)
}
