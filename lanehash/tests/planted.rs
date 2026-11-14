//! Structured-input regression tests: the collisions that the first 0.2.0 draft had and
//! that no SMHasher test can see. A site is broken when two different inputs agree under
//! every seed tried.
//!
//! * planted constants: every public constant of the algorithm (and 0, 1, all ones),
//!   also XOR 1, XOR the length and complemented, at every word position of every short
//!   class, must not make any other byte irrelevant;
//! * the draft's three constructions (a multiplicand of one, a public value in the final
//!   fold, two equal multiplies) built from the draft's constants;
//! * chain swap and chain cancellation on the long path with the exact best guess of the
//!   correction between two chains;
//! * the same planted values through the map hasher.
use lanehash::constants::C;
use lanehash::spec::CT;
use lanehash::{hash128, hash64, FixedState};
use std::hash::BuildHasher;

fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
fn word(i: usize) -> u64 {
    u64::from_le_bytes(C[i / 2][8 * (i % 2)..8 * (i % 2) + 8].try_into().unwrap())
}
fn random(len: usize, s: &mut u64) -> Vec<u8> {
    (0..len).map(|_| rng(s) as u8).collect()
}
fn put(v: &mut [u8], at: usize, w: u64) {
    v[at..at + 8].copy_from_slice(&w.to_le_bytes());
}
fn get(v: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(v[at..at + 8].try_into().unwrap())
}
fn hasher(b: &[u8], seed: u64) -> u128 {
    FixedState::new(seed).hash_one(b) as u128
}
/// The draft's multiply constants (words of `C[48..52)`) and final-fold constants.
fn draft() -> ([u64; 8], [u64; 4]) {
    (core::array::from_fn(|j| word(96 + j)), core::array::from_fn(|j| word(104 + j)))
}

#[test]
#[cfg_attr(miri, ignore)] // a million hashes
fn no_public_constant_erases_input() {
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    let mut cands: Vec<u64> = (0..2 * C.len()).map(word).collect();
    cands.extend([0, 1, !0]);
    let fns: [(&str, fn(&[u8], u64) -> u128); 2] = [("hash128", hash128), ("LaneHasher", hasher)];
    for (name, f) in fns {
        for len in [9usize, 12, 16, 17, 24, 32, 33, 48, 64] {
            for at in (0..len / 8).map(|j| 8 * j).chain([len - 8]) {
                for &c in &cands {
                    for v in [c, c ^ 1, c ^ len as u64, !c] {
                        for vary in (0..len).filter(|i| *i < at || *i >= at + 8) {
                            let mut all = true;
                            for _ in 0..8 {
                                let seed = rng(&mut s);
                                let mut a = random(len, &mut s);
                                put(&mut a, at, v);
                                let mut b = a.clone();
                                b[vary] ^= 1 + (rng(&mut s) as u8 % 255);
                                if f(&a, seed) != f(&b, seed) {
                                    all = false;
                                    break;
                                }
                            }
                            assert!(!all, "{name} len {len}: {v:#018x} at byte {at} makes byte {vary} irrelevant");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn the_draft_constructions_do_not_collide() {
    let (ks, kf) = draft();
    let mut s = 0x0123_4567_89AB_CDEFu64;
    for _ in 0..64 {
        let seed = rng(&mut s);
        // a multiplicand of one: word k = KS[k] ^ 1, the paired word varied
        for (len, at, k, free) in [(24usize, 8usize, 1usize, 0usize), (32, 8, 1, 0), (48, 8, 1, 0), (64, 8, 1, 0), (64, 24, 3, 16), (64, 40, 5, 32)] {
            let mut a = random(len, &mut s);
            put(&mut a, at, ks[k] ^ 1);
            let mut b = a.clone();
            put(&mut b, free, get(&a, free) ^ (rng(&mut s) | 1));
            assert_ne!(hash128(&a, seed), hash128(&b, seed), "multiplicand one, len {len} word {k}");
            assert_ne!(hash64(&a, seed), hash64(&b, seed), "multiplicand one, len {len} word {k}");
            assert_ne!(hasher(&a, seed), hasher(&b, seed), "hasher, multiplicand one, len {len}");
        }
        // a public value into the final fold: hash64 was 0 for every free word
        let mut a = vec![0u8; 32];
        put(&mut a, 0, ks[0]);
        put(&mut a, 8, ks[1] ^ kf[1]);
        put(&mut a, 16, ks[2]);
        put(&mut a, 24, rng(&mut s));
        let mut b = a.clone();
        put(&mut b, 24, rng(&mut s));
        assert_ne!(hash64(&a, seed), hash64(&b, seed), "final fold, len 32");
        assert_ne!(hash64(&a, seed), 0);
        // two equal multiplies: words 4, 5 = words 0, 1 ^ constants, varied together
        let base = random(64, &mut s);
        let mk = |s: &mut u64| {
            let mut a = base.clone();
            let (w0, w1) = (rng(s), rng(s));
            put(&mut a, 0, w0);
            put(&mut a, 8, w1);
            put(&mut a, 32, w0 ^ ks[0] ^ ks[4]);
            put(&mut a, 40, w1 ^ ks[1] ^ ks[5]);
            a
        };
        let (a, b) = (mk(&mut s), mk(&mut s));
        assert_ne!(hash128(&a, seed), hash128(&b, seed), "equal multiplies, len 64");
        // the map hasher: a write whose last word is KS[1] ^ len (operand zero), or its
        // complement (operand all ones), reset the accumulator
        for len in [9usize, 12, 16] {
            for v in [ks[1] ^ len as u64, !(ks[1] ^ len as u64), len as u64, !(len as u64), 0, !0] {
                let mut a = random(len, &mut s);
                put(&mut a, len - 8, v);
                let mut b = a.clone();
                b[0] ^= 1 + (rng(&mut s) as u8 % 255);
                assert_ne!(hasher(&a, seed), hasher(&b, seed), "hasher, len {len}, last word {v:#x}");
            }
        }
        let st = FixedState::new(seed);
        for hi in [ks[1] ^ 16, !(ks[1] ^ 16), 0, !0] {
            let (x, y) = ((hi as u128) << 64 | rng(&mut s) as u128, (hi as u128) << 64 | rng(&mut s) as u128);
            assert_ne!(st.hash_one(x), st.hash_one(y), "hasher, u128 with high half {hi:#x}");
        }
    }
}

/// Most likely value of `y ^ (y + d)` over uniform `y` and its probability `2^-cost`:
/// bit i is `d_i ^ carry_i`, and a carry step is free when `d_i == carry_i`, a coin flip
/// otherwise.
fn best_guess(d: u64) -> (u64, u32) {
    let mut cost = [0u32, u32::MAX / 2];
    let mut path = [0u64, 0u64];
    for i in 0..64 {
        let di = (d >> i) & 1;
        let mut nc = [u32::MAX / 2; 2];
        let mut np = [0u64; 2];
        for c in 0..2u64 {
            if cost[c as usize] >= u32::MAX / 2 {
                continue;
            }
            let p = path[c as usize] | (c << i);
            let nexts: &[(u64, u32)] = if di == c { &[(c, 0)] } else { &[(0, 1), (1, 1)] };
            for &(n, w) in nexts {
                let k = cost[c as usize] + if i == 63 { 0 } else { w };
                if k < nc[n as usize] {
                    nc[n as usize] = k;
                    np[n as usize] = p;
                }
            }
        }
        cost = nc;
        path = np;
    }
    let b = if cost[0] <= cost[1] { 0 } else { 1 };
    (d ^ path[b], cost[b])
}

#[test]
#[cfg_attr(miri, ignore)]
fn chains_cannot_be_swapped_or_cancelled() {
    // the most guessable correction between lane i of chains a and b
    let mut best = (u32::MAX, 0, 0, 0, 0u64);
    for a in 0..8 {
        for b in a + 1..8 {
            for i in 0..8 {
                let (g, cost) = best_guess(CT[8 * b + i].wrapping_sub(CT[8 * a + i]));
                if cost < best.0 {
                    best = (cost, a, b, i, g);
                }
            }
        }
    }
    let (cost, a, b, i, g) = best;
    assert!(cost <= 16, "the test needs a guess that holds often enough to be seen");
    // with a merge that treats chains alike, each loop below collided for 1 seed in 2^cost
    let seeds = 64u64 << cost;
    let mut s = 0xA5A5_5A5A_1234_5678u64;
    for len in [64 * (b.max(2) + 2), 64 * (8 * 9 + b.max(2) + 2)] {
        for _ in 0..seeds {
            let seed = rng(&mut s);
            let m = random(len, &mut s);
            let mut swapped = m.clone();
            let (pa, pb) = (64 * a + 8 * i, 64 * b + 8 * i);
            put(&mut swapped, pa, get(&m, pb) ^ g);
            put(&mut swapped, pb, get(&m, pa) ^ g);
            let mut st = 8;
            while 64 * (st + b + 1) < len - 64 {
                let (qa, qb) = (64 * (st + a) + 8 * i, 64 * (st + b) + 8 * i);
                put(&mut swapped, qa, get(&m, qb));
                put(&mut swapped, qb, get(&m, qa));
                st += 8;
            }
            assert_ne!(hash128(&m, seed), hash128(&swapped, seed), "chain swap, {len} bytes, seed {seed:#x}");
            if len < 1024 {
                // cancellation: the same lane state in both chains
                let mut c1 = m.clone();
                put(&mut c1, pb, get(&m, pa) ^ g);
                let mut c2 = c1.clone();
                let v = rng(&mut s);
                put(&mut c2, pa, v);
                put(&mut c2, pb, v ^ g);
                assert_ne!(hash128(&c1, seed), hash128(&c2, seed), "chain cancellation, seed {seed:#x}");
            }
        }
    }
}
