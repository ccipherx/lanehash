//! Prints `len seed data_seed lo hi` per line: the hash of `len` bytes of xorshift64
//! output (low byte of each) started at `data_seed`. With an argument `N`, prints `N`
//! random cases instead of the fixed corpus.
fn xorshift(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

fn bytes(len: usize, data_seed: u64) -> Vec<u8> {
    let mut s = data_seed;
    (0..len).map(|_| xorshift(&mut s) as u8).collect()
}

fn emit(len: usize, seed: u64, data_seed: u64) {
    let h = lanehash::hash128(&bytes(len, data_seed), seed);
    println!("{len} {seed} {data_seed} {} {}", h as u64, (h >> 64) as u64);
}

fn main() {
    let seed0 = lanehash::spec::SEED0;
    let edge = [0u64, 1, u64::MAX, seed0, seed0 ^ 1, 1 << 63, 0x9e3779b97f4a7c15];
    if let Some(n) = std::env::args().nth(1) {
        let n: usize = n.parse().unwrap();
        let mut r = 0x1234_5678_9abc_def1u64;
        for _ in 0..n {
            let len = match xorshift(&mut r) % 4 {
                0 => (xorshift(&mut r) % 17) as usize,
                1 => 17 + (xorshift(&mut r) % 48) as usize,
                2 => 65 + (xorshift(&mut r) % 1024) as usize,
                _ => (xorshift(&mut r) % 5000) as usize,
            };
            emit(len, xorshift(&mut r), xorshift(&mut r) | 1);
        }
        return;
    }
    let mut ds = 1u64;
    for len in 0..=1100 {
        for &seed in &edge {
            ds += 2;
            emit(len, seed, ds);
        }
        // the all-zero input of every length too
        emit(len, 7, 0);
    }
    let mut r = 0x0dd_ba11u64;
    for _ in 0..4000 {
        let len = (xorshift(&mut r) % 5000) as usize;
        emit(len, xorshift(&mut r), xorshift(&mut r) | 1);
    }
}
