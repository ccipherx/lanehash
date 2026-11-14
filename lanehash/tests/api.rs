//! Every backend equals the reference on lengths 0–1100, the regime boundaries and
//! 1 MiB ± 7; `Stream` equals the one-shot function at every split; the map hasher is
//! deterministic per seed.
use lanehash::{dispatch, hash128, hash64, spec, Stream};

fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
fn data(n: usize) -> Vec<u8> {
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    (0..n).map(|_| rng(&mut s) as u8).collect()
}
fn lens() -> Vec<usize> {
    let mut v: Vec<usize> = (0..=1100).collect();
    // Miri: the megabyte lengths would take hours; lengths up to 4 KiB cover the block logic
    if cfg!(miri) {
        v.extend([4095, 4096, 4097]);
    } else {
        v.extend([4095, 4096, 4097, 65535, 65536, 65537, (1 << 20) - 7, 1 << 20, (1 << 20) + 7]);
    }
    v
}
fn backends() -> Vec<&'static dispatch::Backend> {
    let mut v: Vec<&'static dispatch::Backend> = vec![&dispatch::SPEC];
    #[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
    {
        v.push(&dispatch::SSE2);
        if dispatch::has_avx2() {
            v.push(&dispatch::AVX2);
        }
        if dispatch::has_avx512() {
            v.push(&dispatch::AVX512);
        }
    }
    v
}
fn reference128(b: &[u8], seed: u64) -> u128 {
    // the reference: `spec::hash128`
    spec::hash128(b, seed)
}

#[test]
fn every_backend_equals_the_reference() {
    let buf = data(if cfg!(miri) { 4097 } else { (1 << 20) + 7 });
    for &n in &lens() {
        let b = &buf[..n];
        for &seed in &[0u64, 1, 0x9E37_79B9_7F4A_7C15, u64::MAX] {
            let want = reference128(b, seed);
            assert_eq!(hash128(b, seed), want, "dispatched n={n} seed={seed:#x}");
            assert_eq!(hash64(b, seed) as u128, want & u64::MAX as u128, "hash64 n={n}");
            if n > 16 && n <= spec::SHORT_MAX {
                // the class functions of every backend (BMI2 builds of the same source)
                let (ka, kb, _) = spec::keys(seed);
                for be in backends() {
                    let got = unsafe { if n <= 32 { (be.short128_le32)(b, ka, kb) } else { (be.short128_le64)(b, ka, kb) } };
                    assert_eq!(got, want, "{} short n={n}", be.name);
                    let got = unsafe { if n <= 32 { (be.short64_le32)(b, ka, kb) } else { (be.short64_le64)(b, ka, kb) } };
                    assert_eq!(got as u128, want & u64::MAX as u128, "{} short64 n={n}", be.name);
                }
            }
            if n > spec::SHORT_MAX {
                let ks = spec::ks(seed);
                for be in backends() {
                    assert_eq!(unsafe { (be.hash128)(b.as_ptr(), n, ks) }, want, "{} n={n} seed={seed:#x}", be.name);
                    assert_eq!(unsafe { (be.hash64)(b.as_ptr(), n, ks) } as u128, want & u64::MAX as u128, "{} hash64 n={n}", be.name);
                }
            }
        }
    }
}

/// Every chunk size up to 130 and the block, carry and buffer boundaries, at lengths on
/// those boundaries; `finish` as a checkpoint; empty updates.
#[cfg_attr(miri, ignore)]
#[test]
fn stream_equals_one_shot_at_every_split() {
    let buf = data(6000);
    let lens = [0usize, 1, 63, 64, 65, 127, 128, 511, 512, 513, 575, 576, 577, 639, 640, 641, 1023, 1024, 1025, 1087, 1088, 1089, 1151, 1152, 1153, 2048, 2049, 4096, 4097, 6000];
    let mut chunks: Vec<usize> = (1..=130).collect();
    chunks.extend([255, 256, 511, 512, 513, 575, 576, 577, 639, 640, 641, 1023, 1024, 1025, 1151, 1152, 1153, 3000, 6000]);
    for &len in &lens {
        let bytes = &buf[..len];
        let want = hash128(bytes, 11);
        for &c in &chunks {
            let mut st = Stream::new(11);
            for chunk in bytes.chunks(c) {
                st.update(chunk);
                st.update(&[]);
            }
            assert_eq!(st.finish128(), want, "len={len} chunk={c}");
            assert_eq!(st.finish64(), want as u64, "finish64 len={len} chunk={c}");
        }
        // checkpoint: finish, then continue
        let mut st = Stream::new(11);
        st.update(&bytes[..len / 2]);
        assert_eq!(st.finish128(), hash128(&bytes[..len / 2], 11), "checkpoint len={len}");
        st.update(&bytes[len / 2..]);
        assert_eq!(st.finish128(), want, "after checkpoint len={len}");
    }
}

#[test]
fn keyed_and_fixed_entry_points_equal_the_one_shot_function() {
    let buf = data(4097);
    for &seed in &[0u64, 1, 0x9E37_79B9_7F4A_7C15, u64::MAX] {
        let k = lanehash::Keyed::new(seed);
        for &n in &lens() {
            if n > buf.len() {
                continue;
            }
            assert_eq!(k.hash64(&buf[..n]), hash64(&buf[..n], seed), "keyed n={n}");
            assert_eq!(k.hash128(&buf[..n]), hash128(&buf[..n], seed), "keyed n={n}");
        }
        macro_rules! fixed {
            ($($n:literal)*) => {$(
                let a: &[u8; $n] = buf[..$n].try_into().unwrap();
                assert_eq!(lanehash::hash64_fixed(a, seed), hash64(a, seed), "fixed {}", $n);
                assert_eq!(lanehash::hash128_fixed(a, seed), hash128(a, seed), "fixed {}", $n);
                assert_eq!(k.hash64_fixed(a), hash64(a, seed), "keyed fixed {}", $n);
            )*};
        }
        fixed!(0 1 2 3 4 5 7 8 9 12 15 16 17 20 24 31 32 33 40 48 63 64 65 100 128 513);
    }
}

/// The BMI2 builds of the 17–64 byte classes equal the plain ones.
#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
#[test]
fn bmi2_short_classes_equal_the_reference() {
    if !std::is_x86_feature_detected!("bmi2") {
        return;
    }
    let buf = data(64);
    for &seed in &[0u64, 1, 0x9E37_79B9_7F4A_7C15, u64::MAX] {
        let (ka, kb, _) = spec::keys(seed);
        for n in 17..=64 {
            let b = &buf[..n];
            let want = spec::hash128(b, seed);
            let got = unsafe { if n <= 32 { lanehash::short::bmi2::short128_le32(b, ka, kb) } else { lanehash::short::bmi2::short128_le64(b, ka, kb) } };
            assert_eq!(got, want, "n={n}");
            let got = unsafe { if n <= 32 { lanehash::short::bmi2::short64_le32(b, ka, kb) } else { lanehash::short::bmi2::short64_le64(b, ka, kb) } };
            assert_eq!(got, want as u64, "n={n}");
        }
    }
}

#[test]
fn hasher_is_deterministic_and_seeded() {
    use std::hash::{BuildHasher, Hasher};
    let a = lanehash::FixedState::new(1);
    let b = lanehash::FixedState::new(2);
    let key = b"the quick brown fox";
    assert_eq!(a.hash_one(key), a.hash_one(key));
    assert_ne!(a.hash_one(key), b.hash_one(key));
    let mut h = a.build_hasher();
    h.write(&data(1000));
    h.write_u64(7);
    assert_ne!(h.finish(), a.hash_one(key));
    let mut m = std::collections::HashMap::with_hasher(a);
    m.insert("x".to_string(), 1);
    assert_eq!(m.get("x"), Some(&1));
}
