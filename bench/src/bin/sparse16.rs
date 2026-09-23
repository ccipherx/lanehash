//! rurban's sparse 16-bit keys (≤9 bits set), tested across many seeds.
//! Measures high/low 32-bit collisions in hash128/hash64 against the 0.298 Poisson expectation.
use std::collections::HashMap;
fn main() {
    let seeds: u64 = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(400);
    let keys: Vec<[u8; 2]> = (0u32..65536).filter(|k| k.count_ones() <= 9).map(|k| (k as u16).to_le_bytes()).collect();
    println!("keys {}", keys.len());
    let mut tot = [0usize; 4];
    let mut worst = [0usize; 4];
    for seed in 0..seeds {
        let mut maps: [HashMap<u32, u32>; 4] = Default::default();
        for k in &keys {
            let h = lanehash::hash128(k, seed);
            let v = [(h >> 96) as u32, (h >> 64) as u32, (h >> 32) as u32, h as u32];
            for i in 0..4 {
                *maps[i].entry(v[i]).or_insert(0) += 1;
            }
        }
        for i in 0..4 {
            let c: usize = maps[i].values().map(|&n| n as usize - 1).sum();
            tot[i] += c;
            worst[i] = worst[i].max(c);
        }
    }
    let n = keys.len() as f64;
    let expect = n * (n - 1.0) / 2.0 / 4294967296.0;
    println!("expected per seed {expect:.3}; {seeds} seeds");
    for (i, name) in ["hash128 high 32", "hash128 bits 64..96", "hash64 high 32", "hash64 low 32"].iter().enumerate() {
        println!("{name:<22} mean {:.3}  worst {}", tot[i] as f64 / seeds as f64, worst[i]);
    }
}
