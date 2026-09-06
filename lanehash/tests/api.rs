//! Contracts of the streaming and batched APIs beyond the random chunkings of
//! `tests/vectors.rs`: every chunk size at every buffer boundary, `finish` as a
//! checkpoint, empty updates, and batch groups that are aliased, exactly full or
//! left over.

fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// `Stream::update` absorbs whole 256-byte steps from its buffer, tops up a partial
/// step from the caller's slice, then absorbs straight from the slice; the buffer
/// (1296 bytes) must hold the rest. Every chunk size up to 300 and the sizes at the
/// step, buffer and regime boundaries, for lengths at those boundaries.
#[cfg_attr(miri, ignore)] // ~40 MB of hashing, ~1000x slower under Miri
#[test]
fn stream_every_chunk_size_at_boundaries() {
    let mut s = 0x510e_527f_ade6_82d1u64;
    let mut buf = vec![0u8; 5000];
    for b in buf.iter_mut() {
        *b = rng(&mut s) as u8;
    }
    let lens = [0usize, 1, 64, 65, 256, 257, 1024, 1025, 1039, 1040, 1041, 1279, 1280, 1281, 1295, 1296, 1297, 1535, 1536, 1537, 2048, 2049, 4096, 5000];
    let mut chunks: Vec<usize> = (1..=300).collect();
    chunks.extend([511, 512, 513, 767, 768, 769, 1023, 1024, 1025, 1295, 1296, 1297, 2000, 4095, 4096, 4097, 5000]);
    for &len in &lens {
        let bytes = &buf[..len];
        let want = lanehash::hash128(bytes, 11);
        for &c in &chunks {
            let mut st = lanehash::Stream::new(11);
            for chunk in bytes.chunks(c) {
                st.update(chunk);
            }
            assert_eq!(st.finish128(), want, "len={len} chunk={c}");
        }
    }
}

/// `finish128(&self)` does not consume the stream: after every update it equals the
/// one-shot hash of the bytes so far, the stream can go on, and empty updates
/// (before, between and after) change nothing.
#[test]
fn stream_checkpoints_and_empty_updates() {
    let mut s = 0x9b05_688c_2b3e_6c1fu64;
    let mut buf = vec![0u8; 6000];
    for b in buf.iter_mut() {
        *b = rng(&mut s) as u8;
    }
    for seed in [0u64, 42, u64::MAX] {
        let mut st = lanehash::Stream::new(seed);
        st.update(&[]);
        assert_eq!(st.finish128(), lanehash::hash128(&[], seed));
        let mut done = 0;
        while done < buf.len() {
            let c = (1 + rng(&mut s) as usize % 400).min(buf.len() - done);
            st.update(&buf[done..done + c]);
            st.update(&[]);
            done += c;
            let want = lanehash::hash128(&buf[..done], seed);
            assert_eq!(st.finish128(), want, "checkpoint at {done} seed={seed}");
            assert_eq!(st.finish128(), want, "second finish at {done} seed={seed}");
            assert_eq!(st.finish64(), want as u64, "finish64 at {done} seed={seed}");
        }
    }
}

/// Batch groups: the L4 group fills at four inputs, the L8 group at two; leftovers
/// go through the one-shot function; short and long inputs are hashed in place.
/// Aliased inputs (the same slice in every slot of a group), exactly full groups,
/// leftovers, empty inputs and an empty batch.
#[test]
fn batch_groups_and_aliasing() {
    let mut s = 0x1f83_d9ab_fb41_bd6bu64;
    let mut buf = vec![0u8; 3000];
    for b in buf.iter_mut() {
        *b = rng(&mut s) as u8;
    }
    let l4: &[u8] = &buf[1..201];
    let l8: &[u8] = &buf[3..503];
    let short: &[u8] = &buf[5..25];
    let long: &[u8] = &buf[7..2007];
    let cases: Vec<Vec<&[u8]>> = vec![
        vec![],
        vec![&[]],
        vec![&[], &[], &[], &[]],
        vec![l4; 4],
        vec![l4; 5],
        vec![l4; 8],
        vec![l8; 2],
        vec![l8; 3],
        vec![l8; 4],
        vec![short, l4, l8, long],
        vec![long, l8, l4, short, l4, l8, l4, l8, l4, l4, l8],
        vec![&buf[..65], &buf[..256], &buf[..257], &buf[..1024], &buf[..1025], &buf[..64]],
    ];
    for (ci, inputs) in cases.iter().enumerate() {
        for seed in [0u64, 9, u64::MAX] {
            let want: Vec<u128> = inputs.iter().map(|b| lanehash::hash128(b, seed)).collect();
            let mut got = vec![0u128; inputs.len()];
            lanehash::hash128_batch(inputs, seed, &mut got);
            assert_eq!(got, want, "case {ci} seed={seed}");
            let mut got64 = vec![0u64; inputs.len()];
            lanehash::hash64_batch(inputs, seed, &mut got64);
            assert_eq!(got64, want.iter().map(|&w| w as u64).collect::<Vec<_>>(), "hash64_batch case {ci} seed={seed}");
        }
    }
}
