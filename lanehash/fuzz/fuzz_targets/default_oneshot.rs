//! The default function: `hash128`/`hash64`, `Keyed`, and every backend's one-shot
//! entry equal the portable reference. The input is copied to an exact-size allocation
//! so that ASan catches a read past either end. Run with `-max_len=8192` to reach the
//! chained path with several blocks.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let Some((seed, data)) = input.split_first_chunk::<8>() else { return };
    let seed = u64::from_le_bytes(*seed);
    let data = data.to_vec();
    let want = lanehash::spec::hash128(&data, seed);
    assert_eq!(lanehash::hash128(&data, seed), want, "hash128");
    assert_eq!(lanehash::hash64(&data, seed), want as u64, "hash64");
    let k = lanehash::Keyed::new(seed);
    assert_eq!(k.hash128(&data), want, "keyed");
    if data.len() > lanehash::spec::SHORT_MAX {
        let ks = lanehash::spec::ks(seed);
        for b in lanehash_fuzz::default_backends() {
            assert_eq!(unsafe { (b.hash128)(data.as_ptr(), data.len(), ks) }, want, "backend {}", b.name);
            assert_eq!(unsafe { (b.hash64)(data.as_ptr(), data.len(), ks) }, want as u64, "backend {} hash64", b.name);
        }
    }
});
