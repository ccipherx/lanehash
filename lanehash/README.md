# lanehash

**Fast, portable, non-cryptographic 64-bit and 128-bit hashing for Rust.**

`lanehash` provides deterministic 64-bit and 128-bit hashes with runtime-selected SIMD backends, a portable scalar implementation, streaming support, and `HashMap` / `HashSet` integration.

The crate root, `lanehash`, is the default algorithm. An independent AES-based variant is available under [`lanehash::aes`](#aes-variant) and is optimized for bulk workloads on CPUs with VAES.

## Highlights

* **Fast bulk hashing:** up to ~27 bytes/cycle for the default algorithm and ~30 bytes/cycle for the AES variant on Zen 4.
* **Competitive short-input performance:** optimized for the small keys common in hash tables.
* **Portable:** scalar fallback plus SSE2, AVX2, AVX-512, NEON, and wasm SIMD backends.
* **Deterministic:** every backend produces identical output for the same input and seed.
* **Runtime dispatch:** selects the fastest supported backend on x86-64 and aarch64.
* **Streaming:** incremental hashing through `Stream`.
* **HashMap / HashSet support:** `RandomState` and `FixedState`.
* **`no_std`:** no runtime dependencies.
* **Memory-safe:** no out-of-bounds reads; Miri-clean and guard-page tested.
* **Cross-language implementations:** C and WebAssembly bindings are included in the repository.
* **Statistically validated:** passes SMHasher3 and rurban's SMHasher suites, including BadSeeds.

## Which algorithm?

|                          | `lanehash`                  | `lanehash::aes`                         |
| ------------------------ | --------------------------- | --------------------------------------- |
| Hardware requirement     | None                        | Benefits from AES-NI / VAES / Armv8 AES |
| 64 KiB throughput, Zen 4 | ~27 B/cycle                 | ~30 B/cycle with VAES                   |
| 1 to 31 B keys           | ~17.2 cycles/hash           | ~18.6 cycles/hash                       |
| HashMap workload         | ~97.7 cycles/op             | ~97.3 cycles/op                         |
| wasm32                   | SIMD128 backend             | Software AES                            |
| SMHasher names           | `lanehash64`, `lanehash128` | `lanehash_aes64`, `lanehash_aes128`     |

**Use `lanehash` by default.**

The AES variant is primarily intended for bulk workloads on systems where VAES is available. The two algorithms are independent and produce unrelated outputs.

## Installation

```toml
[dependencies]
lanehash = "0.2"
```

## Usage

### One-shot hashing

```rust
let h64 = lanehash::hash64(b"hello world", 42);
let h128 = lanehash::hash128(b"hello world", 42);
```

The seed allows applications to select independent hash domains.

### Streaming

`Stream` produces the same result as one-shot hashing:

```rust
let mut stream = lanehash::Stream::new(42);

stream.update(b"hello ");
stream.update(b"world");

assert_eq!(
    stream.finish128(),
    lanehash::hash128(b"hello world", 42)
);
```

### `HashMap` and `HashSet`

`lanehash` provides `std::hash::BuildHasher` implementations for standard collections:

```rust
use std::collections::HashMap;

let mut map: HashMap<&str, u32, lanehash::RandomState> =
    HashMap::default();

map.insert("key", 1);
```

For deterministic hashing:

```rust
let fixed: HashMap<u64, u64, lanehash::FixedState> =
    HashMap::with_hasher(lanehash::FixedState::new(7));
```

`RandomState` and `FixedState` use a separate `std::hash::Hasher` implementation. Its output is an implementation detail and **must not be persisted or used as a stable serialization format**. It may change between minor releases.

## AES variant

The independent AES-based implementation is available under `lanehash::aes`:

```rust
let h = lanehash::aes::hash64(b"hello world", 42);

let mut stream = lanehash::aes::Stream::new(42);
stream.update(b"hello world");

assert_eq!(stream.finish64(), h);
```

The AES variant also provides batched hashing:

```rust
let records: Vec<&[u8]> = vec![
    &[1u8; 200],
    &[2u8; 300],
    b"short",
];

let mut output = vec![0u64; records.len()];

lanehash::aes::hash64_batch(
    &records,
    42,
    &mut output,
);
```

For short, latency-sensitive inputs, the default `lanehash` implementation is generally the intended choice. The AES variant is optimized primarily for throughput on sufficiently large buffers.

# Performance

Benchmarks are included to show the workloads `lanehash` is designed for. Results are hardware and configuration dependent and should not be treated as universal rankings.

Raw benchmark data is available under `docs/bench/lanehash/`.

## Default algorithm

| Input     |              lanehash |        xxh3 | rapidhash v3 |
| --------- | --------------------: | ----------: | -----------: |
| 4 to 16 B | 11.4 to 12.0 cyc/hash | 9.5 to 10.1 | 11.8 to 11.9 |
| 32 B      |         13.5 cyc/hash |        11.9 |         17.0 |
| 128 B     |     28 to 31 cyc/hash |        29.4 |         35.4 |
| 256 B     |     42 to 45 cyc/hash |         101 |           72 |
| 1 KiB     |          13.6 B/cycle |         6.3 |          6.4 |
| 4 KiB     |          21.8 B/cycle |        12.8 |          8.1 |
| 64 KiB    |      **27.0 B/cycle** |        18.5 |          8.8 |

SSE2-only throughput at 64 KiB is approximately **14.5 B/cycle**.

## AES variant benchmarks

Measured single-threaded on one pinned core with `-C target-cpu=native` and LTO.

Throughput is reported as **bytes per core cycle**, with the corresponding GB/s in parentheses. Values are medians across five interleaved rounds.

The AES variant is primarily optimized for cache-resident bulk workloads.

### L1

| Size   |            AES | gxhash AVX-512 |     gxhash |      xxh3 | rapidhash v3 | foldhash |    ahash |
| ------ | -------------: | -------------: | ---------: | --------: | -----------: | -------: | -------: |
| 16 B   |       15.8 cyc |        9.8 cyc |    8.9 cyc |   9.4 cyc |     11.8 cyc |  7.1 cyc | 11.7 cyc |
| 256 B  |      11.2 (55) |      12.4 (61) |  11.6 (57) |  2.6 (14) |     4.2 (21) | 7.0 (35) | 7.3 (36) |
| 1 KiB  |      18.4 (92) |     21.0 (104) |  17.3 (86) |  6.9 (35) |     6.7 (33) | 7.6 (38) | 9.1 (46) |
| 4 KiB  |     24.7 (122) |     25.5 (129) |  19.8 (99) | 12.1 (60) |     7.9 (39) | 7.7 (39) | 8.7 (44) |
| 16 KiB | **27.9 (138)** |     25.0 (126) | 20.7 (104) | 15.2 (75) |     8.3 (41) | 7.8 (39) | 8.5 (42) |

### L2 / L3

| Size   |            AES | gxhash AVX-512 |     gxhash |      xxh3 | rapidhash v3 | foldhash |    ahash |
| ------ | -------------: | -------------: | ---------: | --------: | -----------: | -------: | -------: |
| 16 B   |       14.3 cyc |        8.3 cyc |    8.3 cyc |   8.4 cyc |     10.3 cyc |  7.1 cyc | 10.2 cyc |
| 256 B  |      11.2 (55) |      12.4 (61) |  11.4 (57) |  2.6 (13) |     3.7 (18) | 7.0 (35) | 7.2 (36) |
| 1 KiB  |      18.0 (89) |      19.9 (98) |  16.8 (83) |  6.5 (33) |     6.4 (32) | 7.9 (39) | 9.4 (47) |
| 4 KiB  |     25.2 (125) |     27.4 (136) | 22.3 (111) | 13.0 (64) |     8.1 (40) | 8.1 (40) | 9.1 (46) |
| 64 KiB | **30.3 (150)** |     29.7 (147) | 24.9 (123) | 18.5 (91) |     8.8 (43) | 8.1 (40) | 8.9 (46) |
| 1 MiB  |     25.0 (124) |     26.4 (130) | 23.8 (118) | 18.1 (89) |     8.6 (42) | 8.1 (40) | 8.9 (45) |

### DRAM

At DRAM sizes, memory bandwidth increasingly dominates the hash function itself.

| Size   |      AES | gxhash AVX-512 |   gxhash |     xxh3 | rapidhash v3 | foldhash |    ahash |
| ------ | -------: | -------------: | -------: | -------: | -----------: | -------: | -------: |
| 16 B   | 54.4 cyc |           30.5 |     31.5 |     42.0 |         58.9 |     24.2 |     41.2 |
| 256 B  | 3.3 (17) |       3.0 (15) | 2.7 (14) | 2.1 (11) |     2.3 (12) | 4.5 (23) | 4.1 (21) |
| 1 KiB  | 5.4 (28) |       5.1 (26) | 5.5 (28) | 5.8 (29) |     5.2 (26) | 6.0 (30) | 3.4 (17) |
| 4 KiB  | 6.2 (30) |       6.2 (32) | 6.2 (32) | 6.4 (31) |     6.4 (32) | 6.4 (32) | 4.8 (24) |
| 64 KiB | 6.4 (32) |       6.4 (33) | 6.5 (33) | 6.7 (33) |     6.6 (33) | 6.6 (33) | 6.1 (30) |
| 1 MiB  | 6.7 (33) |       6.5 (33) | 6.5 (33) | 6.7 (33) |     6.6 (33) | 6.6 (33) | 6.6 (32) |

## Latency

Dependent-chain latency, measured in cycles per hash:

| Hash           |  4 B |  8 B | 16 B | 32 B | 64 B | 256 B | 1 KiB |
| -------------- | ---: | ---: | ---: | ---: | ---: | ----: | ----: |
| AES            | 27.4 | 27.4 | 26.4 | 27.7 | 36.2 |  69.6 | 104.3 |
| gxhash AVX-512 | 34.1 | 34.2 | 34.1 | 44.5 | 53.5 |  74.2 | 115.1 |
| gxhash         | 35.2 | 34.2 | 34.3 | 45.6 | 53.4 |  73.3 | 114.5 |
| xxh3           | 29.0 | 29.0 | 27.2 | 28.8 | 31.6 | 106.8 | 156.1 |
| rapidhash v3   | 23.4 | 23.4 | 23.6 | 30.1 | 40.8 |  69.2 | 158.2 |
| foldhash       | 18.4 | 18.6 | 18.6 | 27.9 | 34.4 |  57.3 | 159.5 |
| ahash          | 35.4 | 35.4 | 34.4 | 36.5 | 44.4 |  71.8 | 158.0 |

## HashMap workloads

Cycles per operation:

| Hasher              | `&str` insert | `&str` lookup | `u64` insert | `u64` lookup | struct insert | struct lookup | `String` insert | `&[u8]` lookup |
| ------------------- | ------------: | ------------: | -----------: | -----------: | ------------: | ------------: | --------------: | -------------: |
| AES                 |          48.9 |          36.6 |        136.1 |         47.2 |         101.3 |          30.9 |           123.5 |           70.9 |
| gxhash              |          56.5 |          37.1 |        116.6 |         46.3 |         154.2 |          35.3 |           167.8 |           57.1 |
| rapidhash (fast)    |          42.8 |          34.4 |        128.6 |         44.6 |         120.0 |          28.7 |           119.2 |           64.6 |
| rapidhash (quality) |          47.8 |          39.7 |        137.8 |         49.4 |         114.8 |          36.2 |           128.5 |           70.6 |
| foldhash (fast)     |          42.6 |          34.3 |        131.0 |         50.9 |         106.2 |          29.6 |           120.4 |           57.5 |
| ahash               |          50.8 |          39.5 |        126.2 |         46.1 |         115.3 |          30.5 |           142.2 |           58.4 |
| xxh3                |         150.7 |         162.9 |        229.8 |        123.0 |         192.6 |         179.2 |           167.2 |          180.8 |
| std SipHash-1-3     |          95.0 |          91.3 |        228.4 |        175.9 |         238.1 |         227.2 |           156.5 |          155.6 |

Additional workloads:

* **4 KiB page deduplication:** 526 cycles/page using `hash128` + `HashSet<u128>`; gxhash 495, xxh3-128 588, rapidhash v3 1055.
* **64 MiB checksum:** 45.8 GB/s one-shot and 47.2 GB/s through `Stream`; gxhash 49.9 GB/s, xxh3 50.9 GB/s, rapidhash v3 39.0 GB/s.

# Quality and validation

Both algorithms are tested against established non-cryptographic hash test suites and memory safety checks.

| Test                                | `lanehash` |     `aes` |
| ----------------------------------- | ---------: | --------: |
| SMHasher3 `--test=All`              |  188 / 188 | 188 / 188 |
| SMHasher3 `--extra --test=All`      |  252 / 252 | 252 / 252 |
| rurban SMHasher, including BadSeeds |      pass* |      pass |
| Miri                                |      clean |     clean |
| Guard-page tests                    |   no fault |  no fault |

Detailed logs are available under `docs/quality/`.

* The default 128-bit implementation has one statistical note in the rurban suite: sparse 16-bit keys, high 32 bits produced 3 collisions against an expected 0.3 at the suite's fixed seed. The corresponding rate was approximately 0.29 per seed over 400 seeds.

These tests measure statistical quality and implementation safety. They do not establish cryptographic security.

# CPU backends

The implementation automatically selects the fastest supported backend while preserving identical output across implementations.

| Platform     | `lanehash`           | `lanehash::aes`                       |
| ------------ | -------------------- | ------------------------------------- |
| x86-64       | AVX-512F, AVX2, SSE2 | VAES + AVX-512VL, VAES + AVX2, AES-NI |
| aarch64      | NEON                 | Armv8 AES                             |
| wasm32       | SIMD128 (`+simd128`) | Software AES                          |
| Any platform | Scalar reference     | T-table software AES                  |

Runtime dispatch is used on x86-64 and aarch64.

The repository also contains:

* `lanehash-c/`: C implementation
* `lanehash-wasm/`: JavaScript/WebAssembly bindings

Every optimized backend is verified against the portable reference implementation.

# Feature flags

| Feature          | Default | Description                                                                  |
| ---------------- | ------- | ---------------------------------------------------------------------------- |
| `std`            | Yes     | Runtime CPU detection and `RandomState` support                              |
| `force-fallback` | No      | Forces portable backends; useful for testing and cross-platform verification |

The core implementation supports `no_std`.

# Stability

The following APIs are frozen within the `0.2` series:

* `hash64`
* `hash128`
* `Stream`
* Corresponding AES APIs

The AES algorithm's output has remained unchanged since `0.1`, when it was the crate-root implementation.

## Verification values

| Algorithm  | 64-bit       | 128-bit      |
| ---------- | ------------ | ------------ |
| `lanehash` | `0xD048C22B` | `0xC2F39939` |
| `aes`      | `0x9FF60BEF` | `0x1A79672D` |

These values are implementation verification vectors, not cryptographic test vectors.

# Requirements

* **Rust:** 1.89 or newer
* **Platform:** any platform supported by Rust; optimized SIMD backends are selected automatically where available

# License

Licensed under either:

* MIT
* Apache-2.0

at your option.
