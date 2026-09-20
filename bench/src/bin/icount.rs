//! icount: exact dynamic instruction mix of one hash call, by single-stepping a
//! forked child under ptrace (WP1 substitute for `perf stat`: perf_event_paranoid=4,
//! no root). Counts are per call and include the harness's call-through-pointer
//! loop (the same loop shape as `bench throughput`). Method: segment 1 runs the
//! call once, segment 2 three times; per-call = (seg2 - seg1) / 2, so every fixed
//! overhead (the SIGSTOP markers) cancels exactly. `rep`-prefixed instructions
//! trap once per iteration under TF and are reported separately.
//!
//! Usage: icount [--hashes a,b] [--sizes n,m] [--align a] [--dump DIR]
//! Output CSV: hash,size,instr,loads,stores,aes,branches,vec_stack,prefetch,rep_iters
use bench::{hashes, rng, HashFn};
use std::collections::HashMap;
use std::hint::black_box;
use std::io::Write;

struct Insn {
    sym: String,
    mnem: String,
    ops: String,
}

fn load_base() -> u64 {
    unsafe extern "C" fn cb(info: *mut libc::dl_phdr_info, _size: usize, data: *mut libc::c_void) -> libc::c_int {
        unsafe { *(data as *mut u64) = (*info).dlpi_addr as u64 };
        1
    }
    let mut base = 0u64;
    unsafe { libc::dl_iterate_phdr(Some(cb), &mut base as *mut u64 as *mut libc::c_void) };
    base
}

fn disasm() -> HashMap<u64, Insn> {
    let exe = std::env::current_exe().unwrap();
    let out = std::process::Command::new("objdump").args(["-d", "--no-show-raw-insn", "-M", "intel"]).arg(&exe).output().expect("objdump");
    let text = String::from_utf8_lossy(&out.stdout);
    let base = load_base();
    let mut m = HashMap::new();
    let mut sym = String::new();
    for line in text.lines() {
        if line.ends_with(">:") {
            if let Some(i) = line.find('<') {
                sym = line[i + 1..line.len() - 2].to_string();
            }
            continue;
        }
        let l = line.trim_start();
        let Some((a, rest)) = l.split_once(":\t") else { continue };
        let Ok(addr) = u64::from_str_radix(a, 16) else { continue };
        let rest = rest.trim();
        let (mut mnem, mut ops) = rest.split_once(' ').map(|(a, b)| (a.to_string(), b.trim().to_string())).unwrap_or((rest.to_string(), String::new()));
        // fold prefixes into the mnemonic
        while matches!(mnem.as_str(), "rep" | "repz" | "repnz" | "lock" | "data16" | "notrack" | "bnd") {
            let (m2, o2) = ops.split_once(' ').map(|(a, b)| (a.to_string(), b.trim().to_string())).unwrap_or((ops.clone(), String::new()));
            mnem = format!("{mnem} {m2}");
            ops = o2;
        }
        m.insert(base + addr, Insn { sym: sym.clone(), mnem, ops });
    }
    m
}

fn mix_of(hist: &HashMap<u64, i64>, dis: &HashMap<u64, Insn>) -> (Mix, i64, i64) {
    let mut mix = Mix::default();
    let (mut unknown, mut residue) = (0i64, 0i64);
    for (&addr, &d) in hist {
        if d < 0 {
            residue += d;
            continue;
        }
        if d == 0 {
            continue;
        }
        let n = d as u64 / 2;
        match dis.get(&addr) {
            Some(insn) => classify(insn, n, &mut mix),
            None => {
                mix.instr += n;
                unknown += n as i64;
            }
        }
    }
    (mix, unknown, residue)
}

impl Mix {
    fn sub(&self, o: &Mix) -> Mix {
        Mix { instr: self.instr - o.instr, ld: self.ld - o.ld, st: self.st - o.st, aes: self.aes - o.aes, br: self.br - o.br, vstack: self.vstack - o.vstack, pf: self.pf - o.pf, rep: self.rep - o.rep }
    }
}

#[derive(Default, Clone, Copy)]
struct Mix {
    instr: u64,
    ld: u64,
    st: u64,
    aes: u64,
    br: u64,
    vstack: u64,
    pf: u64,
    rep: u64,
}

fn classify(insn: &Insn, n: u64, mix: &mut Mix) {
    let m = insn.mnem.as_str();
    let ops = insn.ops.as_str();
    mix.instr += n;
    if m.contains("aesenc") || m.contains("aesdec") {
        mix.aes += n;
    }
    if m.starts_with('j') || m == "call" || m == "ret" {
        mix.br += n;
    }
    if m.starts_with("rep") {
        mix.rep += n;
    }
    if m.starts_with("prefetch") {
        mix.pf += n;
        return;
    }
    match m {
        "push" | "call" => {
            mix.st += n;
            return;
        }
        "pop" | "ret" | "leave" => {
            mix.ld += n;
            return;
        }
        "lea" | "nop" | "endbr64" => return,
        _ => {}
    }
    let Some(lb) = ops.find('[') else { return };
    let first = ops.split_once(',').map(|(a, _)| a).unwrap_or(ops);
    let mem_first = first.contains('[');
    let mem_other = ops[lb..].contains(',') || !mem_first;
    let _ = mem_other;
    let stem = m.trim_start_matches('v');
    let read_only_first = stem.starts_with("cmp") || stem.starts_with("test") || stem.starts_with("ucomis") || stem.starts_with("comis") || stem.starts_with("bt");
    let rmw = matches!(stem, "add" | "sub" | "xor" | "or" | "and" | "inc" | "dec" | "adc" | "sbb" | "not" | "neg" | "shl" | "shr" | "sar" | "rol" | "ror" | "xchg" | "cmpxchg");
    if mem_first && !read_only_first {
        mix.st += n;
        if rmw {
            mix.ld += n;
        }
    } else {
        mix.ld += n;
    }
    let vec = m.starts_with("vmov") || m.starts_with("movdq") || m.starts_with("movap") || m.starts_with("movup") || m.starts_with("vextract") || m.starts_with("vinsert") || m.starts_with("vbroadcast");
    if vec && (ops.contains("[rsp") || ops.contains("[rbp")) {
        mix.vstack += n;
    }
}

fn write_dump(path: &str, what: &str, mut lines: Vec<(u64, i64)>, dis: &HashMap<u64, Insn>) {
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    lines.sort();
    let mut fh = std::fs::File::create(path).unwrap();
    writeln!(fh, "# per-call executed instructions (count = delta/2), {what}, commit {}", bench::COMMIT).unwrap();
    let mut cur = String::new();
    for (addr, d) in lines {
        let (sym, txt) = match dis.get(&addr) {
            Some(i) => (i.sym.as_str(), format!("{:<12} {}", i.mnem, i.ops)),
            None => ("?", String::new()),
        };
        if sym != cur {
            writeln!(fh, "\n<{sym}>:").unwrap();
            cur = sym.to_string();
        }
        writeln!(fh, "{:>8}  {:x}  {}", if d > 0 { d / 2 } else { d }, addr, txt).unwrap();
    }
}

/// Empty hash: its trace is the harness's own per-call overhead (loop, indirect call,
/// ret), subtracted from every reported row.
fn h_null(b: &[u8], s: u64) -> u64 {
    b.len() as u64 ^ s
}

fn call_single(f: HashFn, bufs: &[&[u8]], seed: u64) -> u64 {
    f(bufs[0], seed)
}
fn call_batch(_f: HashFn, bufs: &[&[u8]], seed: u64) -> u64 {
    let mut out = [0u64; 16];
    lanehash::aes::hash64_batch(bufs, seed, &mut out[..bufs.len()]);
    out[0] ^ out[bufs.len() - 1]
}

/// Single-step the child through one call (segment 1) and three calls (segment 2).
/// Returns per-address execution counts of segment 2 minus segment 1.
fn trace(call: fn(HashFn, &[&[u8]], u64) -> u64, f: HashFn, bufs: &[&[u8]]) -> HashMap<u64, i64> {
    trace_fn(&mut |i| call(f, bufs, i))
}

fn trace_fn(call: &mut dyn FnMut(u64) -> u64) -> HashMap<u64, i64> {
    let n1 = black_box(1u64);
    let n2 = black_box(3u64);
    let mut hist: HashMap<u64, i64> = HashMap::new();
    unsafe {
        let pid = libc::fork();
        assert!(pid >= 0, "fork failed");
        if pid == 0 {
            libc::ptrace(libc::PTRACE_TRACEME, 0, 0, 0);
            let mut sink = 0u64;
            sink ^= call(0); // warm-up: dispatch cache, page faults
            libc::raise(libc::SIGSTOP);
            for i in 0..n1 {
                sink ^= call(i);
            }
            libc::raise(libc::SIGSTOP);
            for i in 0..n2 {
                sink ^= call(i);
            }
            libc::raise(libc::SIGSTOP);
            black_box(sink);
            libc::_exit(0);
        }
        let mut status = 0;
        libc::waitpid(pid, &mut status, 0);
        assert!(libc::WIFSTOPPED(status) && libc::WSTOPSIG(status) == libc::SIGSTOP, "no initial stop");
        for seg in 0..2 {
            let w: i64 = if seg == 0 { -1 } else { 1 };
            loop {
                let mut regs: libc::user_regs_struct = std::mem::zeroed();
                libc::ptrace(libc::PTRACE_GETREGS, pid, 0, &mut regs);
                libc::ptrace(libc::PTRACE_SINGLESTEP, pid, 0, 0);
                libc::waitpid(pid, &mut status, 0);
                assert!(libc::WIFSTOPPED(status), "child exited while tracing");
                let sig = libc::WSTOPSIG(status);
                *hist.entry(regs.rip).or_insert(0) += w;
                if sig == libc::SIGSTOP {
                    break;
                }
                assert_eq!(sig, libc::SIGTRAP, "unexpected signal {sig}");
            }
        }
        libc::kill(pid, libc::SIGKILL);
        libc::waitpid(pid, &mut status, 0);
    }
    hist
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let only: Option<Vec<String>> = args.iter().position(|a| a == "--hashes").map(|i| args[i + 1].split(',').map(String::from).collect());
    let sizes: Vec<usize> = args.iter().position(|a| a == "--sizes").map(|i| args[i + 1].split(',').map(|s| s.parse().unwrap()).collect()).unwrap_or_else(|| vec![16, 64, 128, 256, 1024, 4096, 65536, 1 << 20]);
    let align: usize = args.iter().position(|a| a == "--align").map(|i| args[i + 1].parse().unwrap()).unwrap_or(16);
    let dump: Option<String> = args.iter().position(|a| a == "--dump").map(|i| args[i + 1].clone());
    // --batch K: trace lanehash::aes::hash64_batch over K distinct buffers; counts are divided by K
    let batch: usize = args.iter().position(|a| a == "--batch").map(|i| args[i + 1].parse().unwrap()).unwrap_or(0);
    let mut hs = hashes();
    if let Some(o) = &only {
        hs.retain(|(n, _)| o.iter().any(|x| x == n));
    }
    let dis = disasm();
    let null_buf = [0u8; 16];
    let (overhead, _, _) = mix_of(&trace(call_single, h_null, &[&null_buf]), &dis);
    if args.iter().any(|a| a == "--map") {
        // instructions per HashMap<&str, u32> lookup (16 dictionary words of mixed length)
        use std::collections::HashMap as Map;
        use std::hash::BuildHasher;
        let words: Vec<String> = std::fs::read_to_string("/usr/share/dict/words").unwrap().lines().map(String::from).collect();
        let probe: Vec<&str> = words[5000..5016].iter().map(|s| s.as_str()).collect();
        fn run<S: BuildHasher + Clone>(name: &str, bh: S, words: &[String], probe: &[&str], dis: &HashMap<u64, Insn>, dump: &Option<String>, overhead: &Mix) {
            let mut m: Map<&str, u32, S> = Map::with_capacity_and_hasher(words.len(), bh);
            for (i, w) in words.iter().enumerate() {
                m.insert(w.as_str(), i as u32);
            }
            let hist = trace_fn(&mut |_| probe.iter().map(|w| *m.get(w).unwrap() as u64).sum());
            let (mix, _, _) = mix_of(&hist, dis);
            let mix = mix.sub(overhead);
            if let Some(dir) = dump {
                write_dump(&format!("{dir}/{name}-map-words.txt"), &format!("{name} map lookup x16"), hist.iter().filter(|(_, d)| **d != 0).map(|(a, d)| (*a, *d)).collect(), dis);
            }
            let n = probe.len() as f64;
            println!("{name}-map-words,16,{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},0,0", mix.instr as f64 / n, mix.ld as f64 / n, mix.st as f64 / n, mix.aes as f64 / n, mix.br as f64 / n, mix.vstack as f64 / n);
        }
        println!("# per-lookup instruction mix, HashMap<&str,u32>, 16 dictionary words (lengths {:?}); harness overhead of {} instructions subtracted once per 16 lookups", probe.iter().map(|w| w.len()).collect::<Vec<_>>(), overhead.instr);
        run("lanehash", lanehash::FixedState::new(42), &words, &probe, &dis, &dump, &overhead);
        run("aes", lanehash::aes::FixedState::new(42), &words, &probe, &dis, &dump, &overhead);
        run("foldhash-fast", foldhash::fast::FixedState::with_seed(42), &words, &probe, &dis, &dump, &overhead);
        run("gxhash", gxhash::GxBuildHasher::with_seed(42), &words, &probe, &dis, &dump, &overhead);
        run("rapidhash-fast", rapidhash::fast::RandomState::default(), &words, &probe, &dis, &dump, &overhead);
        run("ahash", ahash::RandomState::with_seeds(1, 2, 3, 4), &words, &probe, &dis, &dump, &overhead);
        return;
    }
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    let maxsize = *sizes.iter().max().unwrap();
    let nb = batch.max(1);
    let vs: Vec<Vec<u8>> = (0..nb).map(|_| (0..maxsize + 128).map(|_| rng(&mut s) as u8).collect()).collect();
    let offs: Vec<usize> = vs.iter().map(|v| (align - v.as_ptr() as usize % align) % align).collect();
    println!("# icount: exact per-call instruction mix by ptrace single-step (segment difference); commit={} align={align} core={} batch={batch}; harness overhead subtracted: {} instructions per call (empty function: loop, indirect call, ret)", bench::COMMIT, bench::current_cpu(), overhead.instr);
    println!("# hash,size,instr,loads,stores,aes,branches,vec_stack_movs,prefetch,rep_iters");
    if batch > 0 {
        hs = vec![(Box::leak(format!("aes-batch{batch}").into_boxed_str()), bench::h_aes as HashFn)];
    }
    for &size in &sizes {
        let bufs: Vec<&[u8]> = vs.iter().zip(&offs).map(|(v, &o)| &v[o..o + size]).collect();
        for (name, f) in &hs {
            let hist = trace(if batch > 0 { call_batch } else { call_single }, *f, &bufs);
            let (mix, unknown, residue) = mix_of(&hist, &dis);
            let mix = mix.sub(&overhead);
            let lines: Vec<(u64, i64)> = hist.iter().filter(|(_, d)| **d != 0).map(|(a, d)| (*a, *d)).collect();
            let k = nb as f64;
            if batch > 0 {
                println!("{name},{size},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1}", mix.instr as f64 / k, mix.ld as f64 / k, mix.st as f64 / k, mix.aes as f64 / k, mix.br as f64 / k, mix.vstack as f64 / k, mix.pf as f64 / k, mix.rep as f64 / k);
            } else {
                println!("{name},{size},{},{},{},{},{},{},{},{}", mix.instr, mix.ld, mix.st, mix.aes, mix.br, mix.vstack, mix.pf, mix.rep);
            }
            if unknown != 0 {
                eprintln!("# warning: {name} {size}: {unknown} executed instructions outside the disassembly");
            }
            if residue != 0 {
                eprintln!("# note: {name} {size}: segment residue {residue} (instructions executed more in the 1-call segment than the 3-call one; see dump)");
            }
            if let Some(dir) = &dump {
                write_dump(&format!("{dir}/{name}-{size}.txt"), &format!("{name} {size} B"), lines, &dis);
            }
        }
    }
}
