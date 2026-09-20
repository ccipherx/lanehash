//! Native hash64 values for bench-wasm/check.js: same xorshift buffer, seed 0.
fn main() {
    let mut s: u64;
    for size in [0usize, 5, 64, 65, 256, 1024, 4096, 65536, 1_048_576] {
        s = 0x1234_5678_9abc_def1;
        let buf: Vec<u8> = (0..size).map(|_| bench::rng(&mut s) as u8).collect();
        println!("lanehash,{size},{}", lanehash::hash64(&buf, 0));
        println!("aes,{size},{}", lanehash::aes::hash64(&buf, 0));
    }
}
