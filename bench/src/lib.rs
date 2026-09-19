//! Shared by the harness binaries: cycle counters, the hash-function table, RNG.
use std::hash::{BuildHasher, Hasher};

/// Actual core cycles (APERF via user-mode RDPRU, Zen 2+).
#[inline(always)]
pub fn aperf() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe { core::arch::asm!("rdpru", in("ecx") 1u32, out("eax") lo, out("edx") hi, options(nomem, nostack)) };
    ((hi as u64) << 32) | lo as u64
}
/// Reference cycles at the nominal clock (MPERF via RDPRU).
#[inline(always)]
pub fn mperf() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe { core::arch::asm!("rdpru", in("ecx") 0u32, out("eax") lo, out("edx") hi, options(nomem, nostack)) };
    ((hi as u64) << 32) | lo as u64
}

/// CPU this thread runs on now (APERF is per core: an unpinned run is invalid).
pub fn current_cpu() -> i32 {
    unsafe { libc::sched_getcpu() }
}

/// Commit the binary was built from (from build.rs).
pub const COMMIT: &str = env!("BENCH_COMMIT");

/// Effective clock over a short spin: (APERF GHz, APERF/MPERF ratio). The
/// nominal clock of this part is 3.8 GHz; ratio * 3.8 is the boosted clock.
pub fn clock_check(ms: f64) -> (f64, f64) {
    let t0 = std::time::Instant::now();
    let (a0, m0) = (aperf(), mperf());
    let mut x = 1u64;
    while t0.elapsed().as_secs_f64() * 1000.0 < ms {
        for _ in 0..1000 {
            x = x.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(13);
        }
    }
    let (a1, m1) = (aperf(), mperf());
    std::hint::black_box(x);
    let secs = t0.elapsed().as_secs_f64();
    if std::env::var_os("BENCH_DEBUG_CLOCK").is_some() {
        eprintln!("clock_check: a0={a0} a1={a1} m0={m0} m1={m1} secs={secs}");
    }
    ((a1 - a0) as f64 / secs / 1e9, (a1 - a0) as f64 / (m1 - m0) as f64)
}

pub type HashFn = fn(&[u8], u64) -> u64;

pub fn h_lanehash(b: &[u8], s: u64) -> u64 { lanehash::hash64(b, s) }
pub fn h_lanehash_spec(b: &[u8], s: u64) -> u64 { lanehash::spec::hash128_spec(b, s) as u64 }
pub fn h_lanehash_soft(b: &[u8], s: u64) -> u64 { lanehash::soft::hash128_soft(b, s) as u64 }
pub fn h_lanehash_aesni(b: &[u8], s: u64) -> u64 { if b.len() <= 64 { lanehash::hash64(b, s) } else { unsafe { (lanehash::dispatch::AESNI.one_shot)(b.as_ptr(), b.len(), s) as u64 } } }
pub fn h_lanehash_vaes256(b: &[u8], s: u64) -> u64 { if b.len() <= 64 { lanehash::hash64(b, s) } else { unsafe { (lanehash::dispatch::VAES256.one_shot)(b.as_ptr(), b.len(), s) as u64 } } }
/// Streaming API, one `update` (WP7.5): should equal the one-shot rate for large inputs.
pub fn h_lanehash_stream(b: &[u8], s: u64) -> u64 {
    let mut st = lanehash::Stream::new(s);
    st.update(b);
    st.finish64()
}
/// Streaming API fed in 4 KiB pieces (a decompressor/parser-sized producer).
pub fn h_lanehash_stream4k(b: &[u8], s: u64) -> u64 {
    let mut st = lanehash::Stream::new(s);
    for c in b.chunks(4096) {
        st.update(c);
    }
    st.finish64()
}
/// Direct call to the EVEX backend (no atomic load, no indirect call): WP3.5 dispatch cost.
pub fn h_lanehash_direct(b: &[u8], s: u64) -> u64 { if b.len() <= 64 { lanehash::hash64(b, s) } else { unsafe { lanehash::x86::hash128_vaesvl(b.as_ptr(), b.len(), s) as u64 } } }
pub fn h_gxhash(b: &[u8], s: u64) -> u64 { gxhash::gxhash64(b, s as i64) }
pub fn h_rapidhash(b: &[u8], s: u64) -> u64 { rapidhash::v3::rapidhash_v3_seeded(b, &rapidhash::v3::RapidSecrets::seed_cpp(s)) }
pub fn h_xxh3(b: &[u8], s: u64) -> u64 { xxhash_rust::xxh3::xxh3_64_with_seed(b, s) }
pub fn h_foldhash(b: &[u8], s: u64) -> u64 {
    let mut h = foldhash::fast::FixedState::with_seed(s).build_hasher();
    h.write(b);
    h.finish()
}
pub fn h_ahash(b: &[u8], s: u64) -> u64 {
    let mut h = ahash::RandomState::with_seeds(s, s ^ 0x9E37_79B9_7F4A_7C15, s.rotate_left(17), !s).build_hasher();
    h.write(b);
    h.finish()
}

pub fn hashes() -> Vec<(&'static str, HashFn)> {
    vec![
        ("lanehash", h_lanehash),
        ("lanehash-spec", h_lanehash_spec),
        ("lanehash-soft", h_lanehash_soft),
        ("lanehash-aesni", h_lanehash_aesni),
        ("lanehash-vaes256", h_lanehash_vaes256),
        ("lanehash-direct", h_lanehash_direct),
        ("lanehash-stream", h_lanehash_stream),
        ("lanehash-stream4k", h_lanehash_stream4k),
        (if cfg!(feature = "hybrid") { "gxhash-hybrid" } else { "gxhash" }, h_gxhash),
        ("rapidhash-v3", h_rapidhash),
        ("xxh3", h_xxh3),
        ("foldhash", h_foldhash),
        ("ahash", h_ahash),
    ]
}

pub fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// Cache footprint of `nbuf` separately allocated buffers of `size` bytes: the
/// harness allocates `size + 128` bytes each (plus a 16-byte malloc header),
/// so the stride, not the payload, sets the footprint of small records.
pub fn footprint(nbuf: usize, size: usize) -> usize {
    nbuf * ((size + 144 + 63) & !63)
}
/// Residency class of a footprint on the benchmarking machine (32 KiB L1d, 1 MiB L2, 32 MiB L3).
pub fn residency(footprint: usize) -> &'static str {
    if footprint <= 24 << 10 { "L1" } else if footprint <= 768 << 10 { "L2" } else if footprint <= 24 << 20 { "L3" } else { "DRAM" }
}
