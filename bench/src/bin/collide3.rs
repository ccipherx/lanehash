// Enumerate all keys of `len` bytes with <= 3 bits set, report 64-bit collisions with bit positions.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let len: usize = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(96);
    let seed: u64 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(0);
    let hf: fn(&[u8], u64) -> u64 = if args.get(3).map_or(false, |s| s == "aes") { lanehash::aes::hash64 } else { lanehash::hash64 }; // third argument "aes": the AES function
    let nb = len * 8;
    let mut key = vec![0u8; len];
    let mut v: Vec<(u64, u32, u32, u32)> = Vec::with_capacity(nb * nb * nb / 6 + nb * nb + nb + 1);
    let flip = |key: &mut [u8], i: usize| key[i / 8] ^= 1 << (i % 8);
    v.push((hf(&key, seed), u32::MAX, u32::MAX, u32::MAX));
    for i in 0..nb {
        flip(&mut key, i);
        v.push((hf(&key, seed), i as u32, u32::MAX, u32::MAX));
        for j in i + 1..nb {
            flip(&mut key, j);
            v.push((hf(&key, seed), i as u32, j as u32, u32::MAX));
            for k in j + 1..nb {
                flip(&mut key, k);
                v.push((hf(&key, seed), i as u32, j as u32, k as u32));
                flip(&mut key, k);
            }
            flip(&mut key, j);
        }
        flip(&mut key, i);
    }
    v.sort_unstable();
    let fmt = |b: u32| if b == u32::MAX { "-".to_string() } else { format!("B{}b{}", b / 8, b % 8) };
    let mut n = 0;
    for w in v.windows(2) {
        if w[0].0 == w[1].0 {
            n += 1;
            if n <= 15 {
                println!("h={:016x}  {{{} {} {}}}  vs  {{{} {} {}}}", w[0].0, fmt(w[0].1), fmt(w[0].2), fmt(w[0].3), fmt(w[1].1), fmt(w[1].2), fmt(w[1].3));
            }
        }
    }
    println!("len={len} seed={seed}: {n} colliding pairs among {} keys", v.len());
}
