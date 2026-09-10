//! Same-machine benchmark harness.
//! throughput: independent hashes over 64 buffers; latency: address+seed dependent chain.
use std::hint::black_box;
use std::time::Instant;

use bench::{aperf, hashes, rng, HashFn};

struct Sample {
    bytes_per_cycle: f64,
    gbs: f64,
    cycles_per_hash: f64,
}

/// Throughput: hash `nbuf` independent buffers round-robin for >= `min_ms`.
fn throughput(f: HashFn, bufs: &[&[u8]], size: usize, min_ms: f64) -> Sample {
    let nbuf = bufs.len();
    let mut sink = 0u64;
    let mut iters = 0u64;
    let mut best: Option<Sample> = None;
    // repeat a few short trials and keep the best (least interference), as SMHasher does
    for _trial in 0..5 {
        let mut n = 0u64;
        let t0 = Instant::now();
        let c0 = aperf();
        loop {
            for b in bufs {
                sink ^= f(&b[..size], n);
            }
            n += nbuf as u64;
            if t0.elapsed().as_secs_f64() * 1000.0 >= min_ms {
                break;
            }
        }
        let c1 = aperf();
        let secs = t0.elapsed().as_secs_f64();
        let cyc = (c1 - c0) as f64;
        let s = Sample { bytes_per_cycle: (n as f64 * size as f64) / cyc, gbs: n as f64 * size as f64 / secs / 1e9, cycles_per_hash: cyc / n as f64 };
        iters += n;
        if best.as_ref().map_or(true, |b| s.cycles_per_hash < b.cycles_per_hash) {
            best = Some(s);
        }
    }
    black_box((sink, iters));
    best.unwrap()
}

/// Latency: each call's address and seed depend on the previous result.
fn latency(f: HashFn, bufs: &[&[u8]], size: usize, min_ms: f64) -> Sample {
    let mask = bufs.len() - 1;
    let mut best: Option<Sample> = None;
    for _trial in 0..5 {
        let mut h = 0u64;
        let mut n = 0u64;
        let t0 = Instant::now();
        let c0 = aperf();
        loop {
            for _ in 0..256 {
                let b = &bufs[(h as usize) & mask];
                h = f(&b[..size], h);
            }
            n += 256;
            if t0.elapsed().as_secs_f64() * 1000.0 >= min_ms {
                break;
            }
        }
        let c1 = aperf();
        let secs = t0.elapsed().as_secs_f64();
        let cyc = (c1 - c0) as f64;
        black_box(h);
        let s = Sample { bytes_per_cycle: (n as f64 * size as f64) / cyc, gbs: n as f64 * size as f64 / secs / 1e9, cycles_per_hash: cyc / n as f64 };
        if best.as_ref().map_or(true, |b| s.cycles_per_hash < b.cycles_per_hash) {
            best = Some(s);
        }
    }
    best.unwrap()
}

/// Batch throughput (WP2): the same 64 buffers hashed through `hash64_batch` in
/// groups of `k` (k = 1 measures the API overhead of the ungrouped path).
fn batch_throughput(bufs: &[&[u8]], size: usize, k: usize, min_ms: f64) -> Sample {
    let nbuf = bufs.len();
    let views: Vec<&[u8]> = bufs.iter().map(|b| &b[..size]).collect();
    let mut out = vec![0u64; nbuf];
    let mut sink = 0u64;
    let mut best: Option<Sample> = None;
    for _trial in 0..5 {
        let mut n = 0u64;
        let t0 = Instant::now();
        let c0 = aperf();
        loop {
            for (chunk, o) in views.chunks(k).zip(out.chunks_mut(k)) {
                lanehash::hash64_batch(chunk, n, o);
            }
            sink ^= out[n as usize % nbuf];
            n += nbuf as u64;
            if t0.elapsed().as_secs_f64() * 1000.0 >= min_ms {
                break;
            }
        }
        let c1 = aperf();
        let secs = t0.elapsed().as_secs_f64();
        let cyc = (c1 - c0) as f64;
        let s = Sample { bytes_per_cycle: (n as f64 * size as f64) / cyc, gbs: n as f64 * size as f64 / secs / 1e9, cycles_per_hash: cyc / n as f64 };
        if best.as_ref().map_or(true, |b| s.cycles_per_hash < b.cycles_per_hash) {
            best = Some(s);
        }
    }
    black_box(sink);
    best.unwrap()
}

fn median(v: &mut Vec<f64>) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let m = v[v.len() / 2];
    let mut d: Vec<f64> = v.iter().map(|x| (x - m).abs()).collect();
    d.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (m, d[d.len() / 2])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("throughput");
    let rounds: usize = args.iter().position(|a| a == "--rounds").map(|i| args[i + 1].parse().unwrap()).unwrap_or(7);
    let only: Option<Vec<String>> = args.iter().position(|a| a == "--hashes").map(|i| args[i + 1].split(',').map(String::from).collect());
    let sizes: Vec<usize> = args
        .iter()
        .position(|a| a == "--sizes")
        .map(|i| args[i + 1].split(',').map(|s| s.parse().unwrap()).collect())
        .unwrap_or_else(|| vec![4, 8, 16, 32, 64, 128, 256, 1024, 4096, 16384, 65536, 1 << 20]);
    let dram = args.iter().any(|a| a == "--dram");
    let nbuf_opt: Option<usize> = args.iter().position(|a| a == "--nbuf").map(|i| args[i + 1].parse().unwrap());
    let align: Option<usize> = args.iter().position(|a| a == "--align").map(|i| args[i + 1].parse().unwrap());
    let batches: Vec<usize> = args.iter().position(|a| a == "--batch").map(|i| args[i + 1].split(',').map(|s| s.parse().unwrap()).collect()).unwrap_or_else(|| vec![1, 2, 4, 8, 16]);
    let min_ms = if dram { 200.0 } else { 30.0 };
    // --arena: one contiguous allocation for the DRAM pieces (2 MiB aligned); --thp: madvise(MADV_HUGEPAGE) on it (WP7.4)
    let arena = args.iter().any(|a| a == "--arena" || a == "--thp");
    let thp = args.iter().any(|a| a == "--thp");

    let mut hs = hashes();
    if let Some(o) = &only {
        hs.retain(|(n, _)| o.iter().any(|x| x == n));
    }
    if mode == "batch" {
        hs.clear();
    }
    let maxsize = *sizes.iter().max().unwrap();
    // buffers: 64 for throughput (fits L3 up to 64 KiB; 64 x 1 MiB would exceed L3, so fewer for big sizes)
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    let nbuf_for = |size: usize| -> usize {
        if let Some(n) = nbuf_opt {
            return n;
        }
        if dram {
            (1usize << 30) / size.max(4096)
        } else if size >= 1 << 20 {
            4
        } else if size >= 65536 {
            8
        } else {
            64
        }
    };
    let nb_max = nbuf_for(*sizes.iter().min().unwrap()).max(nbuf_for(maxsize));
    let _ = nb_max;
    let (ghz, ratio) = bench::clock_check(100.0);
    println!("# mode={mode} rounds={rounds} dram={dram} arena={arena} thp={thp} align={align:?} nbuf={nbuf_opt:?} counter=RDPRU-APERF core={} commit={} clock_check=APERF {ghz:.3} GHz, APERF/MPERF {ratio:.3} (boost active, governor not settable)", bench::current_cpu(), bench::COMMIT);
    println!("# hash,size,bytes_per_cycle_median,bpc_mad,GB_s_median,cycles_per_hash_median,cph_mad,samples,residency,footprint_bytes,eff_GHz");
    let mut results: std::collections::BTreeMap<(String, usize), (Vec<f64>, Vec<f64>, Vec<f64>)> = Default::default();
    let mut wset: std::collections::BTreeMap<usize, usize> = Default::default();
    for size in &sizes {
        let nbuf = nbuf_for(*size);
        wset.insert(*size, bench::footprint(nbuf, *size));
        // allocate with a random 0..64 offset each so alignment varies (page + offset)
        let bufs: Vec<Vec<u8>> = if arena {
            let stride = (size + 128 + 63) & !63;
            let mut v = vec![0u8; nbuf * stride + (2 << 20)];
            let off = (2usize << 20) - (v.as_ptr() as usize % (2 << 20));
            if thp {
                unsafe { libc::madvise(v.as_mut_ptr().add(off) as *mut libc::c_void, nbuf * stride, libc::MADV_HUGEPAGE) };
            }
            for b in v[off..off + nbuf * stride].iter_mut() {
                *b = rng(&mut s) as u8;
            }
            vec![v]
        } else {
            (0..nbuf)
                .map(|_| {
                    let mut v = vec![0u8; size + 128];
                    for b in v.iter_mut() {
                        *b = rng(&mut s) as u8;
                    }
                    v
                })
                .collect()
        };
        let views: Vec<&[u8]> = if arena {
            let stride = (size + 128 + 63) & !63;
            let v = &bufs[0];
            let off = (2usize << 20) - (v.as_ptr() as usize % (2 << 20));
            (0..nbuf).map(|i| {
                let base = v.as_ptr() as usize + off + i * stride;
                let o = match align { Some(a) => (a - base % a) % a + (rng(&mut s) as usize % (64 / a)) * a, None => rng(&mut s) as usize % 64 };
                &v[off + i * stride + o..off + i * stride + o + size]
            }).collect()
        } else {
            bufs.iter().map(|v| {
                let base = v.as_ptr() as usize;
                let o = match align { Some(a) => (a - base % a) % a + (rng(&mut s) as usize % (64 / a)) * a, None => rng(&mut s) as usize % 64 };
                &v[o..o + size]
            }).collect()
        };
        let bufs = views;
        for r in 0..rounds {
            // interleave: rotate function order each round
            let mut order: Vec<usize> = (0..hs.len()).collect();
            order.rotate_left(r % hs.len().max(1));
            for &i in &order {
                let (name, f) = hs[i];
                let smp = if mode == "latency" { latency(f, &bufs, *size, min_ms) } else { throughput(f, &bufs, *size, min_ms) };
                let e = results.entry((name.to_string(), *size)).or_default();
                e.0.push(smp.bytes_per_cycle);
                e.1.push(smp.gbs);
                e.2.push(smp.cycles_per_hash);
            }
            if mode == "batch" {
                let mut order: Vec<usize> = (0..batches.len()).collect();
                order.rotate_left(r % batches.len());
                for &i in &order {
                    let k = batches[i];
                    let smp = batch_throughput(&bufs, *size, k, min_ms);
                    let e = results.entry((format!("lanehash-batch{k}"), *size)).or_default();
                    e.0.push(smp.bytes_per_cycle);
                    e.1.push(smp.gbs);
                    e.2.push(smp.cycles_per_hash);
                }
            }
        }
    }
    for ((name, size), (bpc, gbs, cph)) in results.iter_mut() {
        let (bm, bd) = median(bpc);
        let (gm, _) = median(gbs);
        let (cm, cd) = median(cph);
        let ws = wset[size];
        println!("{name},{size},{bm:.3},{bd:.3},{gm:.2},{cm:.2},{cd:.2},{},{},{ws},{:.3}", bpc.len(), bench::residency(ws), gm / bm);
    }
}
