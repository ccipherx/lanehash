//! Realistic workloads: hashbrown-style maps, 4 KiB dedup, multi-MiB checksum.
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::hint::black_box;

fn aperf() -> u64 {
    let (lo, hi): (u32, u32);
    unsafe { core::arch::asm!("rdpru", in("ecx") 1u32, out("eax") lo, out("edx") hi, options(nomem, nostack)) };
    ((hi as u64) << 32) | lo as u64
}
fn rng(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct K {
    a: u32,
    b: u64,
    c: [u8; 12],
}

fn best_of<F: FnMut() -> u64>(mut f: F, reps: usize) -> u64 {
    (0..reps).map(|_| f()).min().unwrap()
}

fn map_workloads<S: BuildHasher + Clone>(name: &str, bh: S, words: &[String], ints: &[u64], structs: &[K], blobs: &[Vec<u8>]) {
    // words: insert then lookup
    let ins = best_of(|| { let c0 = aperf(); let mut m: HashMap<&str, u32, S> = HashMap::with_capacity_and_hasher(words.len(), bh.clone()); for (i, w) in words.iter().enumerate() { m.insert(w.as_str(), i as u32); } black_box(&m); aperf() - c0 }, 5) as f64 / words.len() as f64;
    let mut m: HashMap<&str, u32, S> = HashMap::with_capacity_and_hasher(words.len(), bh.clone());
    for (i, w) in words.iter().enumerate() { m.insert(w.as_str(), i as u32); }
    let look = best_of(|| { let c0 = aperf(); let mut s = 0u64; for w in words { s = s.wrapping_add(*m.get(w.as_str()).unwrap() as u64); } black_box(s); aperf() - c0 }, 5) as f64 / words.len() as f64;
    // u64 keys
    let ins_i = best_of(|| { let c0 = aperf(); let mut m: HashMap<u64, u64, S> = HashMap::with_capacity_and_hasher(ints.len(), bh.clone()); for &k in ints { m.insert(k, k ^ 1); } black_box(&m); aperf() - c0 }, 3) as f64 / ints.len() as f64;
    let mut mi: HashMap<u64, u64, S> = HashMap::with_capacity_and_hasher(ints.len(), bh.clone());
    for &k in ints { mi.insert(k, k ^ 1); }
    let look_i = best_of(|| { let c0 = aperf(); let mut s = 0u64; for k in ints { s = s.wrapping_add(*mi.get(k).unwrap()); } black_box(s); aperf() - c0 }, 3) as f64 / ints.len() as f64;
    // struct keys
    let ins_s = best_of(|| { let c0 = aperf(); let mut m: HashMap<K, u32, S> = HashMap::with_capacity_and_hasher(structs.len(), bh.clone()); for (i, k) in structs.iter().enumerate() { m.insert(k.clone(), i as u32); } black_box(&m); aperf() - c0 }, 3) as f64 / structs.len() as f64;
    let mut ms: HashMap<K, u32, S> = HashMap::with_capacity_and_hasher(structs.len(), bh.clone());
    for (i, k) in structs.iter().enumerate() { ms.insert(k.clone(), i as u32); }
    let look_s = best_of(|| { let c0 = aperf(); let mut s = 0u64; for k in structs { s = s.wrapping_add(*ms.get(k).unwrap() as u64); } black_box(s); aperf() - c0 }, 3) as f64 / structs.len() as f64;
    // String-keyed insert (owned keys: same hashing as &str, plus the allocation pattern of a real table build)
    let ins_o = best_of(|| { let c0 = aperf(); let mut m: HashMap<String, u32, S> = HashMap::with_capacity_and_hasher(words.len(), bh.clone()); for (i, w) in words.iter().enumerate() { m.insert(w.clone(), i as u32); } black_box(&m); aperf() - c0 }, 3) as f64 / words.len() as f64;
    // mixed-length &[u8] keys (1..=200 B, geometric-ish): lookup
    let mut mb: HashMap<&[u8], u32, S> = HashMap::with_capacity_and_hasher(blobs.len(), bh.clone());
    for (i, b) in blobs.iter().enumerate() { mb.insert(b.as_slice(), i as u32); }
    let look_b = best_of(|| { let c0 = aperf(); let mut s = 0u64; for b in blobs { s = s.wrapping_add(*mb.get(b.as_slice()).unwrap() as u64); } black_box(s); aperf() - c0 }, 5) as f64 / blobs.len() as f64;
    // the same word lookup through the hashbrown crate directly (std's map is hashbrown too, but pinned to a version)
    let mut mh: hashbrown::HashMap<&str, u32, S> = hashbrown::HashMap::with_capacity_and_hasher(words.len(), bh.clone());
    for (i, w) in words.iter().enumerate() { mh.insert(w.as_str(), i as u32); }
    let look_h = best_of(|| { let c0 = aperf(); let mut s = 0u64; for w in words { s = s.wrapping_add(*mh.get(w.as_str()).unwrap() as u64); } black_box(s); aperf() - c0 }, 5) as f64 / words.len() as f64;
    println!("{name},{ins:.1},{look:.1},{ins_i:.1},{look_i:.1},{ins_s:.1},{look_s:.1},{ins_o:.1},{look_b:.1},{look_h:.1}");
}

fn main() {
    let mut s = 0x1234_5678_9abc_def1u64;
    let checksum_only = std::env::args().any(|a| a == "--checksum-only");
    let words: Vec<String> = std::fs::read_to_string("/usr/share/dict/words").unwrap().lines().map(String::from).collect();
    let ints: Vec<u64> = (0..1_000_000).map(|_| rng(&mut s)).collect();
    let structs: Vec<K> = (0..200_000).map(|_| { let mut c = [0u8; 12]; for b in c.iter_mut() { *b = rng(&mut s) as u8; } K { a: rng(&mut s) as u32, b: rng(&mut s), c } }).collect();
    if !checksum_only {
    println!("# maps: cycles per operation (APERF, core 3), {} words, {} u64, {} structs; commit={}", words.len(), ints.len(), structs.len(), bench::COMMIT);
    let blobs: Vec<Vec<u8>> = (0..200_000).map(|_| { let r = rng(&mut s); let len = 1 + (r % 8) as usize * (1 + (r >> 8) % 25) as usize; (0..len).map(|_| rng(&mut s) as u8).collect() }).collect();
    println!("# hasher,words_insert,words_lookup,u64_insert,u64_lookup,struct_insert,struct_lookup,string_insert,bytes_mixed_lookup,hashbrown_words_lookup");
    map_workloads("lanehash", lanehash::FixedState::new(42), &words, &ints, &structs, &blobs);
    map_workloads("gxhash", gxhash::GxBuildHasher::with_seed(42), &words, &ints, &structs, &blobs);
    map_workloads("rapidhash-fast", rapidhash::fast::RandomState::default(), &words, &ints, &structs, &blobs);
    map_workloads("rapidhash-quality", rapidhash::quality::RandomState::default(), &words, &ints, &structs, &blobs);
    map_workloads("foldhash-fast", foldhash::fast::FixedState::with_seed(42), &words, &ints, &structs, &blobs);
    map_workloads("ahash", ahash::RandomState::with_seeds(1, 2, 3, 4), &words, &ints, &structs, &blobs);
    map_workloads("xxh3", xxhash_rust::xxh3::Xxh3DefaultBuilder, &words, &ints, &structs, &blobs);
    map_workloads("std-siphash", std::collections::hash_map::RandomState::new(), &words, &ints, &structs, &blobs);

    // 4 KiB dedup: 20k pages, 10% duplicates; set keyed by the 128-bit digest (set hasher fixed: foldhash)
    let npages = 20_000usize;
    let mut pages = vec![0u8; npages * 4096];
    for b in pages.iter_mut() { *b = rng(&mut s) as u8; }
    for i in 0..npages / 10 { let src = (rng(&mut s) as usize % (npages - 1)) * 4096; let dst = (npages - 1 - i) * 4096; pages.copy_within(src..src + 4096, dst); }
    println!("# dedup: cycles per 4 KiB page (hash128 + HashSet<u128> insert), {npages} pages");
    let dedup = |name: &str, f: &dyn Fn(&[u8]) -> u128| {
        let c = best_of(|| { let c0 = aperf(); let mut set: HashSet<u128, foldhash::fast::FixedState> = HashSet::with_capacity_and_hasher(npages, foldhash::fast::FixedState::with_seed(1)); let mut d = 0; for p in pages.chunks_exact(4096) { if !set.insert(f(p)) { d += 1; } } black_box(d); aperf() - c0 }, 3);
        println!("{name},{:.0}", c as f64 / npages as f64);
    };
    dedup("lanehash", &|p| lanehash::hash128(p, 7));
    dedup("gxhash", &|p| gxhash::gxhash128(p, 7));
    dedup("xxh3-128", &|p| xxhash_rust::xxh3::xxh3_128_with_seed(p, 7));
    dedup("rapidhash-v3(64x2)", &|p| { let a = rapidhash::v3::rapidhash_v3_seeded(p, &rapidhash::v3::RapidSecrets::seed_cpp(7)); let b = rapidhash::v3::rapidhash_v3_seeded(p, &rapidhash::v3::RapidSecrets::seed_cpp(8)); (a as u128) | ((b as u128) << 64) });
    }

    // checksum: 64 MiB in 1 MiB chunks (streams from DRAM/L3)
    let thp = std::env::args().any(|a| a == "--thp");
    let args: Vec<String> = std::env::args().collect();
    let big_mib: usize = args.iter().position(|a| a == "--big-mib").map(|i| args[i + 1].parse().unwrap()).unwrap_or(64);
    let mut big = vec![0u8; (big_mib << 20) + (2 << 20)];
    // 2 MiB-aligned view; with --thp, madvise(MADV_HUGEPAGE) so the kernel backs it with 2 MiB pages (WP7.4)
    let off = (2usize << 20) - (big.as_ptr() as usize % (2 << 20));
    if thp { unsafe { libc::madvise(big.as_mut_ptr().add(off) as *mut libc::c_void, big_mib << 20, libc::MADV_HUGEPAGE) }; }
    for (i, b) in big[off..off + (big_mib << 20)].iter_mut().enumerate() { *b = (i as u32).wrapping_mul(2654435761) as u8; }
    let big = &big[off..off + (big_mib << 20)];
    println!("# checksum: {big_mib} MiB in 1 MiB chunks: GB/s (wall) and B/cycle; thp={thp} commit={}", bench::COMMIT);
    let reverse = args.iter().any(|a| a == "--reverse");
    let mut order: Vec<(&str, &dyn Fn(&[u8]) -> u64)> = Vec::new();
    let chk = |name: &str, f: &dyn Fn(&[u8]) -> u64| {
        let mut best = f64::MAX; let mut bestc = 0u64;
        for _ in 0..3 { let t0 = std::time::Instant::now(); let c0 = aperf(); let mut x = 0u64; for ch in big.chunks_exact(1 << 20) { x ^= f(ch); } black_box(x); let c = aperf() - c0; let dt = t0.elapsed().as_secs_f64(); if dt < best { best = dt; bestc = c; } }
        println!("{name},{:.1},{:.1}", (big_mib << 20) as f64 / best / 1e9, (big_mib << 20) as f64 / bestc as f64);
    };
    let f_lh = |p: &[u8]| lanehash::hash64(p, 7);
    let f_st = |p: &[u8]| bench::h_lanehash_stream(p, 7);
    let f_v2 = |p: &[u8]| bench::h_lanehash_vaes256(p, 7);
    let f_ae = |p: &[u8]| bench::h_lanehash_aesni(p, 7);
    let f_gx = |p: &[u8]| gxhash::gxhash64(p, 7);
    let f_xx = |p: &[u8]| xxhash_rust::xxh3::xxh3_64_with_seed(p, 7);
    let f_rp = |p: &[u8]| rapidhash::v3::rapidhash_v3_seeded(p, &rapidhash::v3::RapidSecrets::seed_cpp(7));
    order.push(("lanehash", &f_lh));
    order.push(("lanehash-stream", &f_st));
    order.push(("lanehash-vaes256", &f_v2));
    order.push(("lanehash-aesni", &f_ae));
    order.push(("gxhash", &f_gx));
    order.push(("xxh3", &f_xx));
    order.push(("rapidhash-v3", &f_rp));
    if reverse { order.reverse(); }
    for (n, f) in &order { chk(n, *f); }

    // WP7.3 fusion: a producer writing 64 MiB of pseudo-random bytes, hashed (a) after producing
    // everything (two passes over DRAM) or (b) 64 KiB at a time from the producer's L1/L2-resident
    // buffer through the streaming API (one pass). Producer: 8 bytes per xorshift step.
    println!("# fusion: 64 MiB produced by a xorshift generator; cycles per byte for produce-all+hash-all vs produce-64KiB+hash-piece (Stream); commit={}", bench::COMMIT);
    let total = 64usize << 20;
    let piece = 64usize << 10;
    let gen = |dst: &mut [u8], st: &mut u64| { for w in dst.chunks_exact_mut(8) { w.copy_from_slice(&rng(st).to_le_bytes()); } };
    let mut best_two = u64::MAX; let mut best_one = u64::MAX; let mut best_gen = u64::MAX;
    let mut whole = vec![0u8; total];
    let mut small = vec![0u8; piece];
    for _ in 0..3 {
        let mut g = 1u64;
        let c0 = aperf(); gen(&mut whole, &mut g); let c1 = aperf();
        let h = lanehash::hash64(&whole, 7); let c2 = aperf();
        black_box(h); best_gen = best_gen.min(c1 - c0); best_two = best_two.min(c2 - c0);
        let mut g = 1u64;
        let c0 = aperf();
        let mut st = lanehash::Stream::new(7);
        for _ in 0..total / piece { gen(&mut small, &mut g); st.update(&small); }
        let h = st.finish64(); let c3 = aperf();
        black_box(h); best_one = best_one.min(c3 - c0);
    }
    let b = total as f64;
    println!("produce-only,{:.3}", best_gen as f64 / b);
    println!("produce-all+hash-all,{:.3},hash-share={:.3}", best_two as f64 / b, (best_two - best_gen) as f64 / b);
    println!("fused-64KiB-stream,{:.3},hash-share={:.3}", best_one as f64 / b, (best_one as f64 - best_gen as f64) / b);
}
