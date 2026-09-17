//! Differential: `hash128`/`hash64` and every backend's one-shot entry equal the
//! portable spec. The input is copied to an exact-size allocation so that ASan
//! catches a read past either end by the SIMD backends. Run with
//! `-max_len=1048576` to reach the prefetch (64 KiB) and sandwich (512 KiB) paths.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: &[u8]| {
    let Some((seed, data)) = input.split_first_chunk::<8>() else { return };
    let seed = u64::from_le_bytes(*seed);
    let data = data.to_vec();
    let want = lanehash::spec::hash128_spec(&data, seed);
    assert_eq!(lanehash::hash128(&data, seed), want, "hash128");
    assert_eq!(lanehash::hash64(&data, seed), want as u64, "hash64");
    if data.len() > lanehash::SHORT_MAX {
        for b in lanehash_fuzz::backends() {
            let got = unsafe { (b.one_shot)(data.as_ptr(), data.len(), seed) };
            assert_eq!(got, want, "backend {}", b.name);
        }
    }
});
