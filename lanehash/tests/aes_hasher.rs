//! The map hasher is a different function from `hash64`: write-sequence
//! disambiguation, determinism and seeding, avalanche, spread of sequential keys, map
//! round trips.
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hasher};

fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// Same bytes, different write sequences, different hashes: every `write` mixes its
/// length and integer writes chain in order.
#[test]
fn write_sequences_are_disambiguated() {
    let bh = lanehash::aes::FixedState::new(1);
    let seq = |writes: &[&[u8]]| {
        let mut h = bh.build_hasher();
        for w in writes {
            h.write(w);
        }
        h.finish()
    };
    assert_ne!(seq(&[b"abc"]), seq(&[b"ab", b"c"]));
    assert_ne!(seq(&[b"ab", b"c"]), seq(&[b"a", b"bc"]));
    assert_ne!(seq(&[b"a", b"bc"]), seq(&[b"a", b"b", b"c"]));
    assert_ne!(seq(&[b"", b"abc"]), seq(&[b"abc", b""]));
    assert_ne!(seq(&[&[0u8; 16], &[0u8; 17]]), seq(&[&[0u8; 17], &[0u8; 16]]));
    assert_ne!(seq(&[&[0u8; 64], &[0u8; 65]]), seq(&[&[0u8; 65], &[0u8; 64]]));

    // through std's Hash impls (length prefixes, 0xff string terminators)
    assert_ne!(bh.hash_one([vec![1u8], vec![2, 3]]), bh.hash_one([vec![1u8, 2], vec![3]]));
    assert_ne!(bh.hash_one([vec![0u8; 0], vec![1]]), bh.hash_one([vec![1u8], vec![]]));
    assert_ne!(bh.hash_one(["abc", "def"]), bh.hash_one(["abcd", "ef"]));
    assert_ne!(bh.hash_one(["abc", "def"]), bh.hash_one(["def", "abc"]));
    assert_ne!(bh.hash_one([1u8, 2]), bh.hash_one([2u8, 1]));
    assert_ne!(bh.hash_one([1u16, 2]), bh.hash_one([2u16, 1]));
    assert_ne!(bh.hash_one([1u32, 2]), bh.hash_one([2u32, 1]));
    assert_ne!(bh.hash_one([1u64, 2]), bh.hash_one([2u64, 1]));
    assert_ne!(bh.hash_one([1u128, 2]), bh.hash_one([2u128, 1]));
    assert_ne!(bh.hash_one((1u64, 2u64)), bh.hash_one((2u64, 1u64)));
    assert_ne!(bh.hash_one(0u64), bh.hash_one(1u64));
    assert_ne!(bh.hash_one(0u128), bh.hash_one(1u128 << 64));
}

/// `FixedState`: same seed, same hash; `Default` is seed 0; distinct seeds give
/// distinct hashes; a cloned hasher continues identically. `RandomState`: one
/// seed per process, so every instance agrees.
#[test]
fn seeding_is_deterministic() {
    let key = "the quick brown fox";
    assert_eq!(lanehash::aes::FixedState::new(7).hash_one(key), lanehash::aes::FixedState::new(7).hash_one(key));
    assert_eq!(lanehash::aes::FixedState::default().hash_one(key), lanehash::aes::FixedState::new(0).hash_one(key));
    let mut seen = HashSet::new();
    for seed in (0..256u64).chain([u64::MAX, 1 << 63]) {
        assert!(seen.insert(lanehash::aes::FixedState::new(seed).hash_one(key)), "seed {seed} collides");
    }
    let bh = lanehash::aes::FixedState::new(3);
    let mut a = bh.build_hasher();
    a.write(b"part one, ");
    let mut b = a.clone();
    a.write(b"part two");
    b.write(b"part two");
    assert_eq!(a.finish(), b.finish());

    assert_eq!(lanehash::aes::RandomState::new().hash_one(key), lanehash::aes::RandomState::new().hash_one(key));
    assert_eq!(lanehash::aes::RandomState::default().hash_one(key), lanehash::aes::RandomState::new().hash_one(key));
}

/// Every input bit of a byte-string key changes the hash (all three write paths:
/// direct up to 16 bytes, 16-byte folds to 64, the one-shot hash above), at least
/// 8 output bits flip and half of them on average.
#[cfg_attr(miri, ignore)] // statistical, ~1000x slower under Miri
#[test]
fn flip_bit_trial() {
    let mut s = 0x5be0_cd19_137e_2179u64;
    let (mut flips, mut trials) = (0u64, 0u64);
    for seed in [0u64, 1, 0x9E37_79B9_7F4A_7C15] {
        let bh = lanehash::aes::FixedState::new(seed);
        for len in 0..=200usize {
            let mut data = vec![0u8; len];
            for b in data.iter_mut() {
                *b = rng(&mut s) as u8;
            }
            let h = bh.hash_one(&data[..]);
            for byte in 0..len {
                for bit in 0..8 {
                    data[byte] ^= 1 << bit;
                    let d = h ^ bh.hash_one(&data[..]);
                    data[byte] ^= 1 << bit;
                    let c = d.count_ones();
                    assert!(c >= 8, "seed={seed:#x} len={len} bit {byte}:{bit} flipped only {c} bits");
                    flips += c as u64;
                    trials += 1;
                }
            }
        }
    }
    // std of the mean is 4 / sqrt(trials) ~ 0.006
    let avg = flips as f64 / trials as f64;
    assert!((avg - 32.0).abs() < 0.05, "average bits flipped {avg}, expected 32");
}

/// Sequential integer and string keys: all 64-bit hashes distinct, and the low 16
/// bits (what a small table indexes by) cover at least 40 000 of 65 536 values
/// (a random function covers 41 427; a stuck bit would cap it at 32 768).
#[cfg_attr(miri, ignore)] // statistical, ~1000x slower under Miri
#[test]
fn sequential_keys_spread() {
    let bh = lanehash::aes::FixedState::new(0);
    let ints: Vec<u64> = (0..65_536u64).map(|i| bh.hash_one(i)).collect();
    let strs: Vec<u64> = (0..65_536u64).map(|i| bh.hash_one(format!("k{i}"))).collect();
    for (name, hs) in [("u64", &ints), ("str", &strs)] {
        assert_eq!(hs.iter().collect::<HashSet<_>>().len(), hs.len(), "{name}: 64-bit collision");
        let low = hs.iter().map(|h| (h & 0xffff) as u16).collect::<HashSet<_>>().len();
        assert!(low >= 40_000, "{name}: only {low} distinct low-16-bit values");
    }
}

#[cfg_attr(miri, ignore)] // 100k inserts, ~1000x slower under Miri
#[test]
fn maps_round_trip() {
    let mut m: HashMap<u64, u64, lanehash::aes::FixedState> = HashMap::default();
    for i in 0..100_000u64 {
        m.insert(i.wrapping_mul(0x9E37_79B9_7F4A_7C15), i);
    }
    assert_eq!(m.len(), 100_000);
    for i in 0..100_000u64 {
        assert_eq!(m.get(&i.wrapping_mul(0x9E37_79B9_7F4A_7C15)), Some(&i));
    }
    for i in 0..50_000u64 {
        assert_eq!(m.remove(&i.wrapping_mul(0x9E37_79B9_7F4A_7C15)), Some(i));
    }
    assert_eq!(m.len(), 50_000);

    let mut set: HashSet<Vec<u8>, lanehash::aes::RandomState> = HashSet::default();
    for len in 0..=2000usize {
        assert!(set.insert(vec![0u8; len]));
        assert!(set.insert(vec![0xffu8; len]) || len == 0);
    }
    assert_eq!(set.len(), 4001);
    assert!(set.contains(&vec![0u8; 1500]) && !set.contains(&vec![1u8; 1500]));
}

#[test]
fn types_are_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<lanehash::aes::Stream>();
    check::<lanehash::aes::LaneHasher>();
    check::<lanehash::aes::FixedState>();
    check::<lanehash::aes::RandomState>();
}
