// node check.js <pkg-dir> [hash]: hash64(buffer(size), 0) in the format of bench/src/bin/wasmcheck.rs.
const w = require((process.argv[2] || "./pkg") + "/bench_wasm.js");
const h = process.argv[3] || "lanehash";
for (const size of [0, 5, 64, 65, 256, 1024, 4096, 65536, 1048576]) {
  console.log(`${h === "lanehash-simd128" || h === "lanehash-scalar" ? "lanehash" : h},${size},${w.run(h, size, 1)}`);
}
