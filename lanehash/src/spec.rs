//! The definition of lanehash: byte-exact, no unsafe. Every backend equals it.
//!
//! Words are 8-byte little-endian; a stripe is 64 bytes = 8 words. State: eight chains
//! of eight 64-bit lanes, chain `c` starting at `CT[8 c + i] + ks`,
//! `ks = fold(seed ^ SEED0, SEED1)`. Stripe `s` goes to chain `s mod 8`; per lane
//! `x = A ^ w; A = x + lo32(x) * hi32(x)`.
//!
//! 0–64 bytes: `short.rs`. `n >= 65`: `m = (n - 1) / 64` leading stripes, then the
//! closing stripe `[n - 64, n)` with `w ^ n` into chain `m mod 8`. Merge: each chain that
//! received data takes one more step with `Q` (a plain XOR would let last-stripe
//! differences of different chains cancel), the chains are XORed lane-wise, four folds
//! with distinct secrets, then `fold(h ^ n, ks ^ S[8])`; the 128-bit output adds a
//! second merge over another lane pairing. Constants are consecutive words of `C`.
use crate::constants::C;

const fn lo64(c: &[u8; 16]) -> u64 {
    u64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])
}
const fn hi64(c: &[u8; 16]) -> u64 {
    u64::from_le_bytes([c[8], c[9], c[10], c[11], c[12], c[13], c[14], c[15]])
}
const fn words<const N: usize>(from: usize) -> [u64; N] {
    let mut t = [0u64; N];
    let mut j = 0;
    while j < N {
        t[j] = if j % 2 == 0 {
            lo64(&C[from + j / 2])
        } else {
            hi64(&C[from + j / 2])
        };
        j += 1;
    }
    t
}

pub const SEED0: u64 = lo64(&C[6]);
pub const SEED1: u64 = hi64(&C[6]) | 1;

/// Fold secrets: words of `C[1..6)` and `C[7..12)`.
pub const S: [u64; 19] = {
    let a: [u64; 10] = words(1);
    let b: [u64; 9] = words(7);
    let mut s = [0u64; 19];
    let mut j = 0;
    while j < 19 {
        s[j] = if j < 10 { a[j] } else { b[j - 10] };
        j += 1;
    }
    s
};

/// Chain start values, words of `C[12..44)`.
pub const CT: [u64; 64] = words(12);

/// Merge-step constants, words of `C[44..48)`.
pub const Q: [u64; 8] = words(44);

pub const STRIPE: usize = 64;
pub const CHAINS: usize = 8;
pub const SHORT_MAX: usize = 64;

/// `lo64(a * b) ^ hi64(a * b)`.
#[inline(always)]
pub fn fold(a: u64, b: u64) -> u64 {
    let r = (a as u128).wrapping_mul(b as u128);
    r as u64 ^ (r >> 64) as u64
}

/// `x = a ^ w; x + lo32(x) * hi32(x)`.
#[inline(always)]
pub fn step(a: u64, w: u64) -> u64 {
    let x = a ^ w;
    x.wrapping_add((x as u32 as u64).wrapping_mul(x >> 32))
}

#[inline(always)]
fn le64(c: &[u8; 64], i: usize) -> u64 {
    u64::from_le_bytes([
        c[8 * i],
        c[8 * i + 1],
        c[8 * i + 2],
        c[8 * i + 3],
        c[8 * i + 4],
        c[8 * i + 5],
        c[8 * i + 6],
        c[8 * i + 7],
    ])
}

/// Eight chains of eight lanes.
pub type State = [[u64; 8]; CHAINS];

/// SIMD/stream state: chain `c` occupies words `8 * c..8 * c + 8`.
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct Words(pub [u64; 64]);

impl Words {
    pub fn from_state(a: &State) -> Words {
        let mut w = Words([0u64; 64]);
        for c in 0..CHAINS {
            w.0[8 * c..8 * c + 8].copy_from_slice(&a[c]);
        }
        w
    }

    pub fn to_state(&self) -> State {
        core::array::from_fn(|c| self.0[8 * c..8 * c + 8].try_into().unwrap())
    }
}

/// One 512-byte block containing one stripe per chain.
pub const BLOCK: usize = CHAINS * STRIPE;

#[inline(always)]
pub fn ks(seed: u64) -> u64 {
    fold(seed ^ SEED0, SEED1)
}

pub fn init(ks: u64) -> State {
    core::array::from_fn(|c| {
        core::array::from_fn(|i| CT[8 * c + i].wrapping_add(ks))
    })
}

#[inline(always)]
fn stripe(ch: &mut [u64; 8], c: &[u8; 64], x: u64) {
    for i in 0..8 {
        ch[i] = step(ch[i], le64(c, i) ^ x);
    }
}

/// Absorb whole 512-byte blocks (one stripe per chain).
pub fn absorb(a: &mut State, blocks: &[u8]) {
    debug_assert!(blocks.len() % BLOCK == 0);
    for blk in blocks.chunks_exact(BLOCK) {
        for (c, s) in blk.chunks_exact(STRIPE).enumerate() {
            stripe(&mut a[c], s.try_into().unwrap(), 0);
        }
    }
}

/// The `r < 8` leading stripes after the last whole block, the closing stripe, the merge.
pub fn finish(a: &State, leading: &[u8], closing: &[u8; 64], n: usize) -> Merged {
    debug_assert!(leading.len() % STRIPE == 0 && leading.len() <= BLOCK);

    let mut a = *a;
    let r = leading.len() / STRIPE;

    for (c, s) in leading.chunks_exact(STRIPE).enumerate() {
        stripe(&mut a[c], s.try_into().unwrap(), 0);
    }

    stripe(&mut a[r % CHAINS], closing, n as u64);

    let used = ((n - 1) / STRIPE + 1).min(CHAINS);
    let mut m = Merged([0u64; 8]);

    for ch in &a[..used] {
        for i in 0..8 {
            m.0[i] ^= step(ch[i], Q[i]);
        }
    }

    m
}

/// Merged lanes, returned by the kernels (64-byte aligned for the SIMD stores).
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct Merged(pub [u64; 8]);

/// Final folds; `HI` adds the high half of the 128-bit output.
#[inline(always)]
pub fn finish_lanes<const HI: bool>(m: &Merged, ks: u64, n: usize) -> u128 {
    let m = &m.0;
    let n = n as u64;

    let h = fold(m[0] ^ S[0], m[1] ^ S[1])
        ^ fold(m[2] ^ S[2], m[3] ^ S[3])
        ^ fold(m[4] ^ S[4], m[5] ^ S[5])
        ^ fold(m[6] ^ S[6], m[7] ^ S[7]);

    let lo = fold(h ^ n, ks ^ S[8]);
    if !HI {
        return lo as u128;
    }

    let h2 = fold(m[0] ^ S[10], m[2] ^ S[11])
        ^ fold(m[1] ^ S[12], m[3] ^ S[13])
        ^ fold(m[4] ^ S[14], m[6] ^ S[15])
        ^ fold(m[5] ^ S[16], m[7] ^ S[17]);

    let hi = fold(h2 ^ n, ks ^ S[18]);
    lo as u128 | (hi as u128) << 64
}

/// Merged lanes of `n >= 65` bytes.
pub fn lanes(bytes: &[u8], ks: u64) -> Merged {
    let n = bytes.len();
    debug_assert!(n >= 65);

    let m = (n - 1) / STRIPE;
    if m < CHAINS {
        // every chain sees at most one stripe: stripe step and merge step back to back, no state array
        let mut mv = Merged([0u64; 8]);

        for s in 0..=m {
            let (c, x): (&[u8; 64], u64) = if s < m {
                (
                    bytes[s * STRIPE..(s + 1) * STRIPE].try_into().unwrap(),
                    0,
                )
            } else {
                (
                    bytes[n - 64..].try_into().unwrap(),
                    n as u64,
                )
            };

            for i in 0..8 {
                mv.0[i] ^= step(
                    step(CT[8 * s + i].wrapping_add(ks), le64(c, i) ^ x),
                    Q[i],
                );
            }
        }

        return mv;
    }

    let mut a = init(ks);
    let nb = m / CHAINS;
    absorb(&mut a, &bytes[..nb * BLOCK]);

    finish(
        &a,
        &bytes[nb * BLOCK..m * STRIPE],
        bytes[n - 64..].try_into().unwrap(),
        n,
    )
}

/// Out of line so the short path keeps its own register allocation.
#[inline(never)]
fn long<const HI: bool>(bytes: &[u8], seed: u64) -> u128 {
    let ks = ks(seed);
    finish_lanes::<HI>(&lanes(bytes, ks), ks, bytes.len())
}

/// 64-bit lanehash of `bytes` under `seed`.
#[inline(always)]
pub fn hash64(bytes: &[u8], seed: u64) -> u64 {
    if bytes.len() <= SHORT_MAX {
        super::short::short64(bytes, seed)
    } else {
        long::<false>(bytes, seed) as u64
    }
}

/// 128-bit lanehash; its low half is [`hash64`].
#[inline(always)]
pub fn hash128(bytes: &[u8], seed: u64) -> u128 {
    if bytes.len() <= SHORT_MAX {
        super::short::short128(bytes, seed)
    } else {
        long::<true>(bytes, seed)
    }
}