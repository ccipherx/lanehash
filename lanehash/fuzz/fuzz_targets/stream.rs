//! `Stream` under arbitrary chunking (including empty chunks and chunks larger
//! than its buffer) equals the one-shot hash, and `finish128` after every update
//! equals the hash of the bytes so far.
#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    seed: u64,
    chunks: Vec<u16>,
    data: &'a [u8],
}

fuzz_target!(|input: Input| {
    let Input { seed, chunks, data } = input;
    let mut st = lanehash::aes::Stream::new(seed);
    let mut done = 0;
    for c in chunks {
        let c = (c as usize).min(data.len() - done);
        st.update(&data[done..done + c]);
        done += c;
        assert_eq!(st.finish128(), lanehash::aes::hash128(&data[..done], seed), "checkpoint at {done}");
    }
    st.update(&data[done..]);
    let want = lanehash::aes::hash128(data, seed);
    assert_eq!(st.finish128(), want, "final");
    assert_eq!(st.finish64(), want as u64, "finish64");
});
