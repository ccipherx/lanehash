// node run.js <pkg-dir>: ns/byte for each hash and size, best of 7 trials of >= 200 ms (after warm-up).
const path = process.argv[2] || "./pkg";
const w = require(path + "/bench_wasm.js");
const { performance } = require("perf_hooks");
const sizes = [256, 1024, 4096, 65536, 1048576];
const hashes = (process.argv[3] || "lanehash-scalar,aes,rapidhash,xxh3").split(",");
console.log(`# node ${process.version} ${w.features()} pkg=${path}`);
console.log("hash,size,ns_per_byte,ns_per_hash");
for (const h of hashes) {
  for (const s of sizes) {
    let iters = Math.max(4, Math.floor((1 << 22) / s));
    w.run(h, s, iters); // warm-up (JIT tiering)
    let best = Infinity;
    for (let t = 0; t < 7; t++) {
      let n = 0, t0 = performance.now(), el = 0;
      while (el < 200) { w.run(h, s, iters); n += iters; el = performance.now() - t0; }
      const nsph = (el * 1e6) / n;
      if (nsph < best) best = nsph;
    }
    console.log(`${h},${s},${(best / s).toFixed(4)},${best.toFixed(1)}`);
  }
}
