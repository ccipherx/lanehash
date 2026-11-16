# Changelog

## 0.5.1 (2026-11-16)

- No code or output change. SMHasher3 (`--test=All` 188/188, `--extra` 252/252) and
  rurban's SMHasher re-run on the released function, at 64 and 128 bits: every test
  passes, nothing flagged.
- `lanehash-wasm` and `lanehash-ffi` now share the crate's version number.

## 0.5.0 (2026-11-15)

- **Fix:** `Stream` and `aes::Stream` on 32-bit targets. The byte count wrapped after
  4 GiB: the scalar backend then hashed the wrong length, the wasm simd128 backend read
  and wrote outside its state, and the AES stream read out of bounds. Both streams now
  panic once the input exceeds `usize::MAX` bytes (see `# Panics`). Output is unchanged
  for every shorter input.
- Formal verification in the repository's `verification/`: Lean proofs about a model of
  the hash that reproduces the verification values, Kani proofs on the 0–64 byte path,
  the `HashMap` hasher and the x86 kernels, and CBMC proofs for the C port.
- No output change: known answers and verification values are unchanged.

## 0.4.0 (2026-11-14)

The release after 0.1.1. Versions 0.2 and 0.3 were never published.

- **Breaking:** the crate root is a new function. `lanehash::hash64`, `hash128`,
  `Stream`, `LaneHasher`, `FixedState` and `RandomState` now compute the non-AES hash
  (chained NH-32 stripes on SSE2 / AVX2 / AVX-512 / NEON / wasm simd128, scalar
  reference, multiply-fold path for 0–64 bytes) and return different values than in 0.1.
  Every multiply operand of the 0–64 byte path carries a seed secret and the merge
  constants differ per chain: an unpublished draft keyed only the last multiply
  and let chosen inputs collide under every seed. Verification values 64-bit
  `0x36B435A3`, 128-bit `0xB5F2A65D`.
- New: `Keyed` (many inputs under one seed), `hash64_fixed` / `hash128_fixed` (arrays,
  inlined short classes). Same values as `hash64` / `hash128`.
- `HashMap` hasher: secrets in both operands of every fold, small integer writes packed
  into a 128-bit buffer, a different accumulator start per `RandomState`.
- 65–512 B 1.14–1.36x faster (latency 1.13–1.22x), variable-length 1–16 B keys 1.38x,
  `Stream` in 4 KiB pieces about 2x. Fixed-length 4–16 B keys through `hash64` about 10%
  slower, `hash64_fixed` faster than before. NEON compiles again (still untested on
  hardware).
- The 0.1 AES-lane function is unchanged, bit for bit, under `lanehash::aes` (same items
  plus `hash64_batch` / `hash128_batch`): replace `lanehash::` with `lanehash::aes::` to
  keep 0.1 output.
- C port: `lanehash.c` / `lanehash64` are the new function; the AES port is
  `lanehash_aes.c` / `lanehash_aes64`. SMHasher rows likewise: `lanehash64` is new,
  `lanehash_aes64` is the 0.1 function. `lanehash-wasm`: `hash64` / `Stream` are the new
  function, `aes_hash64` / `AesStream` the AES one.

## 0.1.1 (2026-09-19)

- T-table software AES fallback (4x faster portable path); C port i686 fix.

## 0.1.0 (2026-09-18)

- Initial release: the AES-lane hash, `Stream`, batched hashing, `HashMap` support, C
  port, wasm bindings, fuzz targets.
