//! Statistical sanity checks, deterministic (xorshift): single-bit avalanche, uniqueness
//! of near-identical inputs, length and seed sensitivity. Not a substitute for SMHasher
//! (logs in docs/quality/). Ignored under Miri.
use std::collections::HashSet;

fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

fn fill(buf: &mut [u8], s: &mut u64) {
    for b in buf.iter_mut() {
        *b = rng(s) as u8;
    }
}

/// Seeds: 0, a random one, and `K0` (`short::secrets` computes `fold(seed ^ K0, K1)`,
/// which is 0 at this seed: the secrets degenerate to their constants).
fn seeds() -> [u64; 3] {
    let k0 = u64::from_le_bytes(lanehash::constants::C[27][..8].try_into().unwrap());
    [0, k0, 0x9E37_79B9_7F4A_7C15]
}

/// Any single input bit flip changes the hash, flips at least 8 bits of each 64-bit
/// half and on average half of all 128 bits. Every bit position for lengths 1..=300,
/// sampled positions for larger lengths across the remaining regimes.
#[cfg_attr(miri, ignore)]
#[test]
fn flip_bit_trial() {
    let mut s = 0x6a09_e667_f3bc_c908u64;
    let mut lens: Vec<usize> = (1..=300).collect();
    lens.extend([512, 1023, 1024, 1025, 1296, 2048, 4097, 65_536, 65_537]);
    let (mut flips, mut trials) = (0u64, 0u64);
    for seed in seeds() {
        for &len in &lens {
            let mut data = vec![0u8; len];
            fill(&mut data, &mut s);
            let h = lanehash::aes::hash128(&data, seed);
            let positions: Vec<usize> = if len <= 300 {
                (0..len).collect()
            } else {
                (0..16).chain(len - 16..len).chain((0..16).map(|_| rng(&mut s) as usize % len)).collect()
            };
            for byte in positions {
                for bit in 0..8 {
                    data[byte] ^= 1 << bit;
                    let d = h ^ lanehash::aes::hash128(&data, seed);
                    data[byte] ^= 1 << bit;
                    let (lo, hi) = ((d as u64).count_ones(), (d >> 64).count_ones());
                    assert!(lo >= 8 && hi >= 8, "len={len} seed={seed:#x} bit {byte}:{bit} flipped only {lo}+{hi} bits");
                    flips += (lo + hi) as u64;
                    trials += 1;
                }
            }
        }
    }
    // std of the mean is 5.66 / sqrt(trials) ~ 0.005; 0.05 is ten sigma
    let avg = flips as f64 / trials as f64;
    assert!((avg - 64.0).abs() < 0.05, "average bits flipped {avg}, expected 64");
}

/// One bit flipped in a constant-pattern input (rapidhash's "ray cast"): over three
/// patterns, lengths 1..=384 and every bit position, no two inputs share a 64-bit hash
/// (1.8M values, birthday expectation 1e-7).
#[cfg_attr(miri, ignore)]
#[test]
fn single_bit_ray_cast_is_collision_free() {
    let mut seen = HashSet::with_capacity(3 * 384 * 385 * 4);
    for pattern in [0x00u8, 0xAA, 0x53] {
        for len in 1..=384 {
            let mut data = vec![pattern; len];
            for byte in 0..len {
                for bit in 0..8 {
                    data[byte] ^= 1 << bit;
                    let h = lanehash::aes::hash64(&data, 0);
                    data[byte] ^= 1 << bit;
                    assert!(seen.insert(h), "collision: pattern {pattern:#04x} len {len} bit {byte}:{bit}");
                }
            }
        }
    }
}

/// Length sensitivity: all-zero inputs of every length 0..=4096 and a few larger ones,
/// and every prefix of a random buffer, hash to distinct 64-bit values under each seed.
#[cfg_attr(miri, ignore)]
#[test]
fn lengths_are_distinguished() {
    let mut s = 0xbb67_ae85_84ca_a73bu64;
    let mut lens: Vec<usize> = (0..=4096).collect();
    lens.extend([65_535, 65_536, 65_537, 1 << 17]);
    let zeros = vec![0u8; 1 << 17];
    let mut random = vec![0u8; 1 << 17];
    fill(&mut random, &mut s);
    for seed in seeds() {
        let (mut seen_z, mut seen_r) = (HashSet::new(), HashSet::new());
        for &len in &lens {
            assert!(seen_z.insert(lanehash::aes::hash64(&zeros[..len], seed)), "zeros len={len} seed={seed:#x}");
            assert!(seen_r.insert(lanehash::aes::hash64(&random[..len], seed)), "prefix len={len} seed={seed:#x}");
        }
    }
}

/// Seed sensitivity: sequential, single-bit, all-but-one-bit and random seeds give
/// distinct 64-bit values for one input of each regime.
#[cfg_attr(miri, ignore)]
#[test]
fn seeds_are_distinguished() {
    let mut s = 0x3c6e_f372_fe94_f82bu64;
    let mut seeds: Vec<u64> = (0..1024).collect();
    seeds.extend((0..64).map(|i| 1u64 << i));
    seeds.extend((0..64).map(|i| u64::MAX ^ (1u64 << i)));
    seeds.extend((0..2048).map(|_| rng(&mut s)));
    seeds.push(u64::MAX);
    seeds.extend(self::seeds());
    seeds.sort_unstable();
    seeds.dedup();
    for len in [0usize, 5, 16, 24, 48, 100, 500, 2000] {
        let mut data = vec![0u8; len];
        fill(&mut data, &mut s);
        let mut seen = HashSet::new();
        for &seed in &seeds {
            assert!(seen.insert(lanehash::aes::hash64(&data, seed)), "len={len} seed={seed:#x}");
        }
    }
}
