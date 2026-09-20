//! Pins the frozen output: `aes_vectors.rs` only checks that the backends agree with
//! the spec, so a change to `constants.rs` or `lanes.rs` would pass there.

/// SMHasher's verification value: hash `key[..i]` (`key[i] = i`) under seed `256 - i`,
/// hash the concatenated results under seed 0, low 32 bits.
#[test]
fn smhasher_verification_values() {
    let mut key = [0u8; 256];
    let mut h64 = [0u8; 8 * 256];
    let mut h128 = [0u8; 16 * 256];
    for i in 0..256 {
        key[i] = i as u8;
        let seed = 256 - i as u64;
        h64[8 * i..8 * i + 8].copy_from_slice(&lanehash::aes::hash64(&key[..i], seed).to_le_bytes());
        h128[16 * i..16 * i + 16].copy_from_slice(&lanehash::aes::hash128(&key[..i], seed).to_le_bytes());
    }
    assert_eq!(lanehash::aes::hash64(&h64, 0) as u32, 0x9FF6_0BEF, "64-bit verification value");
    assert_eq!(lanehash::aes::hash128(&h128, 0) as u32, 0x1A79_672D, "128-bit verification value");
}

/// Known answers across every regime: the short classes, L4, L8, L16, the prefetch
/// threshold (64 KiB) and the EVEX sandwich threshold (512 KiB). Input byte `i` is
/// `(i * 31 + 7) as u8`. Generated at 0.1.0 (commit 7743982).
#[test]
fn known_answers() {
    const KAT: &[(usize, u64, u128)] = &[
        (0, 0x0, 0x5e201e25068f21a8efb65f27ba37854c),
        (1, 0x0, 0x0fed4f50ceee892d4bc3561e0e21b9a0),
        (3, 0x0, 0xe2ac3a05be512b8e0cad31fe1cdf9c8b),
        (4, 0x0, 0xc6f24c6f864fa587593ea31fc2afa1c6),
        (8, 0x0, 0xc07322567974984c105a5ef11f0c1980),
        (9, 0x0, 0x2fe82366983fc3a8d34cc6f32d08ab6a),
        (16, 0x0, 0x49be4932f88fa711d89885e74a874c74),
        (17, 0x0, 0x45630955d11e5cfcb984cc549b650a4d),
        (32, 0x0, 0x0ce1ccc33a737f314fc94a6c72c1978e),
        (33, 0x0, 0xaea7e799a9b926bf5204f95b3344f25b),
        (64, 0x0, 0xd8b50fcd5f39e98f7266500a9c41fd21),
        (65, 0x0, 0xb8a9c0c6da5cf1f32e910e7d56d36d80),
        (100, 0x0, 0x6026e3fd50024565844eac14a1bd524f),
        (256, 0x0, 0x3ee157fbf119bbcdad3ea30d1acdff17),
        (257, 0x0, 0xbb67ab0149f8da93231d4351c4be3a19),
        (777, 0x0, 0x24c4c8b67632b1bb41320d4ed2eb1f1d),
        (1024, 0x0, 0x6d04f06c65e12043a084ca2c7060e5ea),
        (1025, 0x0, 0xef217830aeb242a15ddd7efe15c60d37),
        (2048, 0x0, 0x27ece7a3c02cd744f24be0e5f60e1b3e),
        (4097, 0x0, 0xbffe21353311b94d9e9296f958ed57f7),
        (65_536, 0x0, 0x499899e610f584fcbde93b5ec6bd76b2),
        (65_537, 0x0, 0xd4d4b77333de57c8bc875251f3769499),
        (1 << 19, 0x0, 0x0c3bdea4b2b09186bef70c99323772ba),
        ((1 << 19) + 1, 0x0, 0x5500e34b604b58d69b4000cdba356857),
        (0, u64::MAX, 0x57abf8d393cf9dc593cdee2862a3c337),
        (7, u64::MAX, 0x458d869336d23179dd421ae92be1f9f1),
        (100, u64::MAX, 0x2119e58024762dfb39b63cfe91e8fdea),
        (777, u64::MAX, 0xe467db680efdc11106ac18104f8acbb1),
        (1025, u64::MAX, 0x482f59410309fe117601caaf69f06ba2),
        (70_000, u64::MAX, 0x970c2e7de5eeb7852516c4c5422a591d),
        (13, 0x9e3779b97f4a7c15, 0x628e715b2034842d49db70ee407f1aac),
        (200, 0x9e3779b97f4a7c15, 0x8ffb6b8ae8bc9556c2895d6edbcceb25),
        (999, 0x9e3779b97f4a7c15, 0x47949babebaa98b6ef4351198af04b30),
        (3000, 0x9e3779b97f4a7c15, 0xc4728add4cbf79c984a0f534f24caa85),
    ];
    let buf: Vec<u8> = (0..(1 << 19) + 1).map(|i| (i * 31 + 7) as u8).collect();
    for &(len, seed, want) in KAT {
        if cfg!(miri) && len >= 65_536 {
            continue; // Miri: L16 is still covered by 2048 and 4097
        }
        let bytes = &buf[..len];
        assert_eq!(lanehash::aes::hash128(bytes, seed), want, "hash128 len={len} seed={seed:#x}");
        assert_eq!(lanehash::aes::hash64(bytes, seed), want as u64, "hash64 len={len} seed={seed:#x}");
        assert_eq!(lanehash::aes::spec::hash128_spec(bytes, seed), want, "spec len={len} seed={seed:#x}");
        assert_eq!(lanehash::aes::soft::hash128_soft(bytes, seed), want, "soft len={len} seed={seed:#x}");
        let mut st = lanehash::aes::Stream::new(seed);
        st.update(bytes);
        assert_eq!(st.finish128(), want, "stream len={len} seed={seed:#x}");
    }
}

/// `short::secrets`: top bit forced (no secret is 0 or 1), all eight distinct, also at
/// the seed where the non-linear step is zero (`seed ^ K0 == 0`).
#[test]
fn short_secrets_are_well_formed() {
    let k0 = u64::from_le_bytes(lanehash::constants::C[27][..8].try_into().unwrap());
    let mut s = 0x2545_f491_4f6c_dd1du64;
    let mut seeds = vec![0u64, 1, u64::MAX, k0, k0 ^ 1, 1 << 63];
    seeds.extend((0..1000).map(|_| {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    }));
    for seed in seeds {
        let k = lanehash::aes::short::secrets(seed);
        for (j, &kj) in k.iter().enumerate() {
            assert!(kj >> 63 == 1, "seed={seed:#x} k[{j}]={kj:#x} top bit clear");
        }
        for i in 0..8 {
            for j in i + 1..8 {
                assert_ne!(k[i], k[j], "seed={seed:#x} k[{i}] == k[{j}]");
            }
        }
    }
}
