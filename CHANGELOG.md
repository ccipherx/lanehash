# Changelog

## 0.4.0 (2026-11-14)

The release after 0.1.1. Versions 0.2 and 0.3 were never published.

* **Breaking:** The crate root now implements the new non-AES `lanehash` algorithm. `hash64`, `hash128`, `Stream`, `LaneHasher`, `FixedState`, and `RandomState` now use the new implementation and produce different output from 0.1.

  * SIMD backends: SSE2, AVX2, AVX-512, NEON, and wasm SIMD128
  * Portable scalar reference implementation
  * Multiply-fold path for inputs up to 64 bytes
  * Every multiply operand of the 0 to 64 byte path carries a secret derived from the seed. An earlier unpublished draft keyed only the last multiply, which let chosen inputs collide under every seed
  * Merge constants differ per chain, so no two chains can be exchanged or cancelled
  * Verification values: `0x36B435A3` (64-bit), `0xB5F2A65D` (128-bit)

* **New API:** `Keyed` hashes many inputs under one seed without the per-call seed multiply. `hash64_fixed` / `hash128_fixed` take arrays, so the short classes inline for a known length. All three return the same values as `hash64` / `hash128`.

* **`HashMap` hasher:** every fold has a secret in both operands, small integer writes are packed into a 128-bit buffer (one multiply for a `u64` key), and each `RandomState` starts from a different accumulator.

* **Speed** (against the unpublished draft): 65 to 512 byte inputs 1.14 to 1.36x faster in throughput and 1.13 to 1.22x in latency, variable-length keys of 1 to 16 bytes 1.38x faster, `Stream` fed in 4 KiB pieces about 2x faster. Fixed-length 4 to 16 byte keys through `hash64` are about 10% slower (keyed operands, branch-free load), while `hash64_fixed` on an 8-byte array is faster than before. Bulk speed is unchanged.

* **aarch64:** the NEON backend compiles again (it was type-checked only and had fallen out of sync). It has still not run on hardware.

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
