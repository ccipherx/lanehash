//! `hash128_batch`/`hash64_batch` and `batch_with` on every backend equal the
//! one-shot hash of each input, for arbitrary (overlapping, aliased, empty) spans
//! of the data in arbitrary order, so every group fills and every leftover occurs.
#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    seed: u64,
    spans: Vec<(u16, u16)>,
    data: &'a [u8],
}

fuzz_target!(|input: Input| {
    let Input { seed, spans, data } = input;
    let inputs: Vec<&[u8]> = spans
        .iter()
        .map(|&(off, len)| {
            let off = off as usize % (data.len() + 1);
            &data[off..off + (len as usize).min(data.len() - off)]
        })
        .collect();
    let want: Vec<u128> = inputs.iter().map(|b| lanehash::aes::hash128(b, seed)).collect();
    let mut got = vec![0u128; inputs.len()];
    lanehash::aes::hash128_batch(&inputs, seed, &mut got);
    assert_eq!(got, want, "hash128_batch");
    let mut got64 = vec![0u64; inputs.len()];
    lanehash::aes::hash64_batch(&inputs, seed, &mut got64);
    assert!(got64.iter().zip(&want).all(|(g, w)| *g == *w as u64), "hash64_batch");
    for b in lanehash_fuzz::backends() {
        let mut got = vec![0u128; inputs.len()];
        lanehash::aes::dispatch::batch_with(b, &inputs, seed, |i, v| got[i] = v);
        assert_eq!(got, want, "backend {}", b.name);
    }
});
