# Changelog

## 0.2.0 (unreleased)

* **Breaking:** The crate root now implements the new non-AES `lanehash` algorithm. `hash64`, `hash128`, `Stream`, `LaneHasher`, `FixedState`, and `RandomState` now use the new implementation and produce different output from 0.1.

  * SIMD backends: SSE2, AVX2, AVX-512, NEON, and wasm SIMD128
  * Portable scalar reference implementation
  * Multiply-fold path for inputs up to 64 bytes
  * Verification values: `0xD048C22B` (64-bit), `0xC2F39939` (128-bit)

* **0.1 compatibility:** The 0.1 AES-lane algorithm is unchanged bit-for-bit and is now available under `lanehash::aes`. Use the corresponding APIs under `lanehash::aes` to preserve 0.1 output.

  * Includes `hash64`, `hash128`, `Stream`, `LaneHasher`, `FixedState`, `RandomState`
  * Adds `hash64_batch` and `hash128_batch`

* **C port:** The new algorithm is exposed as `lanehash.c` / `lanehash64`. The unchanged AES implementation is exposed as `lanehash_aes.c` / `lanehash_aes64`.

* **SMHasher:** `lanehash64` now refers to the new algorithm. `lanehash_aes64` refers to the original 0.1 AES algorithm.

* **WebAssembly:** `hash64` / `Stream` use the new algorithm. `aes_hash64` / `AesStream` preserve the 0.1 AES implementation.

## 0.1.1 (2026-09-19)

* Added a T-table software AES fallback, improving portable AES performance by approximately 4x.
* Fixed the C port on i686.

## 0.1.0 (2026-09-18)

* Initial release of the AES-lane hash.
* Added streaming and batched hashing.
* Added `HashMap` support.
* Added C and WebAssembly bindings.
* Added fuzz targets.
