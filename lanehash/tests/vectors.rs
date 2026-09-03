//! Every backend must equal the portable spec for every length and seed.
use lanehash::spec::hash128_spec;

fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

fn lengths() -> Vec<usize> {
    if cfg!(miri) {
        // Miri is ~1000x slower: cover every regime boundary once.
        let mut v: Vec<usize> = (0..=80).collect();
        v.extend([100, 255, 256, 257, 300, 512, 1023, 1024, 1025, 1040, 1296, 1297, 2048, 2063]);
        return v;
    }
    let mut v: Vec<usize> = (0..=1100).collect();
    v.extend([2047, 2048, 2049, 4095, 4096, 4097, 65535, 65536, 65537, 1 << 20, (1 << 20) + 7]);
    v
}

#[test]
fn backends_match_spec() {
    let mut s = 0x1234_5678_9abc_def1u64;
    let seeds = [0u64, 1, u64::MAX, 0x8000_0000_0000_0000, rng(&mut s), rng(&mut s)];
    let mut buf = vec![0u8; if cfg!(miri) { 4096 } else { (1 << 20) + 64 }];
    for b in buf.iter_mut() {
        *b = rng(&mut s) as u8;
    }
    let backends: Vec<&lanehash::dispatch::Backend> = {
        let mut v = vec![&lanehash::dispatch::SPEC];
        #[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
        {
            if std::is_x86_feature_detected!("aes") {
                v.push(&lanehash::dispatch::AESNI);
            }
            if std::is_x86_feature_detected!("vaes") && std::is_x86_feature_detected!("avx2") {
                v.push(&lanehash::dispatch::VAES256);
            }
            if std::is_x86_feature_detected!("vaes") && std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512vl") {
                v.push(&lanehash::dispatch::VAESVL);
            }
        }
        #[cfg(all(target_arch = "aarch64", not(feature = "force-fallback")))]
        {
            if std::arch::is_aarch64_feature_detected!("aes") {
                v.push(&lanehash::dispatch::NEON);
            }
        }
        v
    };
    println!("backends: {:?}, dispatch: {}", backends.iter().map(|b| b.name).collect::<Vec<_>>(), lanehash::dispatch::backend().name);
    for &len in &lengths() {
        for &seed in &seeds {
            // random alignment offset 0..16 so unaligned loads are exercised
            let off = (rng(&mut s) % 16) as usize;
            let bytes = &buf[off..off + len];
            let want = hash128_spec(bytes, seed);
            assert_eq!(lanehash::hash128(bytes, seed), want, "dispatch len={len} seed={seed:#x}");
            assert_eq!(lanehash::hash64(bytes, seed), want as u64, "hash64 len={len} seed={seed:#x}");
            if len > lanehash::SHORT_MAX {
                for b in &backends {
                    let got = unsafe { (b.one_shot)(bytes.as_ptr(), len, seed) };
                    assert_eq!(got, want, "backend {} len={len} seed={seed:#x}", b.name);
                }
            }
        }
    }
}

#[test]
fn stream_matches_oneshot() {
    let mut s = 0xdead_beef_cafe_f00du64;
    let mut buf = vec![0u8; if cfg!(miri) { 4200 } else { 70_000 }];
    for b in buf.iter_mut() {
        *b = rng(&mut s) as u8;
    }
    let mut lens: Vec<usize> = (0..=1100).step_by(if cfg!(miri) { 97 } else { 7 }).collect();
    lens.extend([1024, 1025, 1039, 1040, 1041, 1279, 1280, 1281, 1296, 1297, 2047, 2048, 4096, 4097]);
    if !cfg!(miri) {
        lens.extend([10_000, 65_536, 65_537, 69_999]);
    }
    for &len in &lens {
        let bytes = &buf[..len];
        for seed in [0u64, 7, u64::MAX] {
            let want = lanehash::hash128(bytes, seed);
            // one chunk
            let mut st = lanehash::Stream::new(seed);
            st.update(bytes);
            assert_eq!(st.finish128(), want, "stream one-chunk len={len}");
            // random chunking
            for _ in 0..3 {
                let mut st = lanehash::Stream::new(seed);
                let mut i = 0;
                while i < len {
                    let c = (1 + rng(&mut s) as usize % 700).min(len - i);
                    st.update(&bytes[i..i + c]);
                    i += c;
                }
                assert_eq!(st.finish128(), want, "stream chunked len={len} seed={seed}");
            }
            // byte-at-a-time for small lengths
            if len <= 1300 && !cfg!(miri) {
                let mut st = lanehash::Stream::new(seed);
                for b in bytes {
                    st.update(core::slice::from_ref(b));
                }
                assert_eq!(st.finish128(), want, "stream bytewise len={len}");
            }
        }
    }
}

/// A11: batch == single == spec on random mixed-length batches, on every backend.
#[test]
fn batch_matches_single() {
    let mut s = 0x0bad_5eed_1234_5678u64;
    let mut buf = vec![0u8; 5000];
    for b in buf.iter_mut() {
        *b = rng(&mut s) as u8;
    }
    let backends: Vec<&lanehash::dispatch::Backend> = {
        let mut v = vec![&lanehash::dispatch::SPEC];
        #[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
        {
            if std::is_x86_feature_detected!("aes") {
                v.push(&lanehash::dispatch::AESNI);
            }
            if std::is_x86_feature_detected!("vaes") && std::is_x86_feature_detected!("avx2") {
                v.push(&lanehash::dispatch::VAES256);
            }
            if std::is_x86_feature_detected!("vaes") && std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512vl") {
                v.push(&lanehash::dispatch::VAESVL);
            }
        }
        #[cfg(all(target_arch = "aarch64", not(feature = "force-fallback")))]
        {
            if std::arch::is_aarch64_feature_detected!("aes") {
                v.push(&lanehash::dispatch::NEON);
            }
        }
        v
    };
    let rounds = if cfg!(miri) { 4 } else { 300 };
    for round in 0..rounds {
        let n = 1 + rng(&mut s) as usize % 40;
        // mixed regimes; every fourth round concentrates on one regime so groups fill
        let inputs: Vec<&[u8]> = (0..n)
            .map(|_| {
                let len = match round % 4 {
                    0 => rng(&mut s) as usize % 1300,
                    1 => 65 + rng(&mut s) as usize % 192,
                    2 => 257 + rng(&mut s) as usize % 768,
                    _ => rng(&mut s) as usize % 65,
                };
                let off = rng(&mut s) as usize % (buf.len() - len);
                &buf[off..off + len]
            })
            .collect();
        for &seed in &[0u64, 7, u64::MAX, (round as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)] {
            let want: Vec<u128> = inputs.iter().map(|b| hash128_spec(b, seed)).collect();
            let mut got = vec![0u128; n];
            lanehash::hash128_batch(&inputs, seed, &mut got);
            assert_eq!(got, want, "hash128_batch round={round} seed={seed:#x}");
            let mut got64 = vec![0u64; n];
            lanehash::hash64_batch(&inputs, seed, &mut got64);
            assert!(got64.iter().zip(&want).all(|(g, w)| *g == *w as u64), "hash64_batch round={round}");
            for b in &backends {
                let mut got = vec![0u128; n];
                lanehash::dispatch::batch_with(b, &inputs, seed, |i, v| got[i] = v);
                assert_eq!(got, want, "batch backend {} round={round} seed={seed:#x}", b.name);
            }
        }
    }
}

#[test]
fn hasher_basic() {
    use std::hash::{BuildHasher, Hash, Hasher};
    let bh = lanehash::FixedState::new(42);
    let mut h1 = bh.build_hasher();
    "hello".hash(&mut h1);
    let mut h2 = bh.build_hasher();
    "hello".hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
    let mut h3 = bh.build_hasher();
    "hellp".hash(&mut h3);
    assert_ne!(h1.finish(), h3.finish());
    let mut m: std::collections::HashMap<String, u32, lanehash::FixedState> = std::collections::HashMap::with_hasher(bh);
    for i in 0..1000 {
        m.insert(format!("k{i}"), i);
    }
    assert_eq!(m["k77"], 77);
}
