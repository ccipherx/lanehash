# Changelog

## 0.2.0 (unreleased)

- **Breaking:** the crate root is a new function. `lanehash::hash64`, `hash128`,
  `Stream`, `LaneHasher`, `FixedState` and `RandomState` now compute the non-AES hash
  (chained NH-32 stripes on SSE2 / AVX2 / AVX-512 / NEON / wasm simd128, scalar
  reference, multiply-fold path for 0–64 bytes) and return different values than in 0.1.
  Verification values 64-bit `0xD048C22B`, 128-bit `0xC2F39939`.
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
