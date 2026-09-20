//! The same hashes on wasm32, timed from JS (run.js).
use wasm_bindgen::prelude::*;

fn buffer(size: usize) -> Vec<u8> {
    let mut s = 0x1234_5678_9abc_def1u64;
    (0..size)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s as u8
        })
        .collect()
}

/// Hash `iters` times a `size`-byte buffer with `name`; returns a checksum so the work is kept.
#[wasm_bindgen]
pub fn run(name: &str, size: usize, iters: u32) -> u64 {
    let buf = buffer(size);
    let mut acc = 0u64;
    match name {
        "aes" => {
            for i in 0..iters {
                acc ^= lanehash::aes::hash64(&buf, i as u64);
            }
        }
        "rapidhash" => {
            for i in 0..iters {
                acc ^= rapidhash::v3::rapidhash_v3_seeded(&buf, &rapidhash::v3::RapidSecrets::seed_cpp(i as u64));
            }
        }
        "xxh3" => {
            for i in 0..iters {
                acc ^= xxhash_rust::xxh3::xxh3_64_with_seed(&buf, i as u64);
            }
        }
        // lanehash-scalar: the reference; lanehash-simd128: the dispatched function in a +simd128 build
        "lanehash-scalar" => {
            for i in 0..iters {
                acc ^= lanehash::spec::hash64(&buf, i as u64);
            }
        }
        #[cfg(target_feature = "simd128")]
        "lanehash-simd128" => {
            for i in 0..iters {
                acc ^= lanehash::hash64(&buf, i as u64);
            }
        }
        _ => panic!("unknown hash"),
    }
    acc
}

#[wasm_bindgen]
pub fn features() -> String {
    format!("simd128={}", cfg!(target_feature = "simd128"))
}
