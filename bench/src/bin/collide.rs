// Find 64-bit collisions among sparse keys (<= 2 bits set) of a given length.
use std::collections::HashMap;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let len: usize = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(96);
    let seed: u64 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(0);
    let hf: fn(&[u8], u64) -> u64 = if args.get(3).map_or(false, |s| s == "aes") { lanehash::aes::hash64 } else { lanehash::hash64 }; // third argument "aes": the AES function
    let nbits = len * 8;
    let mut seen: HashMap<u64, Vec<(usize, usize)>> = HashMap::new();
    let mut key = vec![0u8; len];
    let ins = |key: &[u8], a: usize, b: usize, seen: &mut HashMap<u64, Vec<(usize, usize)>>| {
        let h = hf(key, seed);
        seen.entry(h).or_default().push((a, b));
    };
    ins(&key, usize::MAX, usize::MAX, &mut seen);
    for i in 0..nbits {
        key[i / 8] ^= 1 << (i % 8);
        ins(&key, i, usize::MAX, &mut seen);
        for j in i + 1..nbits {
            key[j / 8] ^= 1 << (j % 8);
            ins(&key, i, j, &mut seen);
            key[j / 8] ^= 1 << (j % 8);
        }
        key[i / 8] ^= 1 << (i % 8);
    }
    let mut n = 0;
    for (h, v) in &seen {
        if v.len() > 1 {
            n += 1;
            if n <= 12 {
                let d: Vec<String> = v.iter().map(|(a, b)| format!("{{{}:{}}}", if *a == usize::MAX { "-".into() } else { format!("B{}b{}", a / 8, a % 8) }, if *b == usize::MAX { "-".into() } else { format!("B{}b{}", b / 8, b % 8) })).collect();
                println!("h={h:016x} keys={}", d.join(" "));
            }
        }
    }
    println!("len={len} seed={seed}: {} colliding hash values among {} keys", n, seen.values().map(|v| v.len()).sum::<usize>());
}
