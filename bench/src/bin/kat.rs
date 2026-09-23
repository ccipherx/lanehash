//! Known-answer vectors for tests/vectors.rs, cross-checked against the dispatched backend.
//!
//! Input byte i is `(i * 31 + 7) as u8`.
fn main() {
    let lens = [65usize, 66, 96, 127, 128, 129, 191, 192, 255, 256, 257, 511, 512, 1023, 1024, 1025, 4096, 8191, 8192, 8193, 65535, 65536, 65537, (1 << 20) - 7, 1 << 20, (1 << 20) + 7];
    let seeds = [0u64, 1, 0x9E37_79B9_7F4A_7C15, u64::MAX];
    let buf: Vec<u8> = (0..(1 << 20) + 7).map(|i| (i * 31 + 7) as u8).collect();
    for &n in &lens {
        for &s in &seeds {
            let h = lanehash::spec::hash64(&buf[..n], s);
            assert_eq!(h, lanehash::hash64(&buf[..n], s), "spec != dispatched at n={n} seed={s:#x}");
            assert_eq!(h, lanehash::hash128(&buf[..n], s) as u64, "hash128 low half at n={n}");
            println!("    ({n}, {s:#x}, {:#018x}),", h);
        }
    }
    let (v64, v128) = verification();
    println!("// verification_LE: lanehash64 {v64:#010x}  lanehash128 {v128:#010x}");
}

/// SMHasher verification values of the spec (rurban `VerificationTest`): hash of the
/// concatenated hashes of key[..i] (key[i] = i) under seed 256 - i, then low 32 bits.
#[allow(dead_code)]
pub fn verification() -> (u32, u32) {
    let mut key = [0u8; 256];
    let mut h64 = [0u8; 8 * 256];
    let mut h128 = [0u8; 16 * 256];
    for i in 0..256 {
        key[i] = i as u8;
        let seed = 256 - i as u64;
        h64[8 * i..8 * i + 8].copy_from_slice(&lanehash::hash64(&key[..i], seed).to_le_bytes());
        h128[16 * i..16 * i + 16].copy_from_slice(&lanehash::hash128(&key[..i], seed).to_le_bytes());
    }
    (lanehash::hash64(&h64, 0) as u32, lanehash::hash128(&h128, 0) as u32)
}
