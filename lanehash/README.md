# lanehash – AES-lane hashing for Rust

A fast, non-cryptographic hash with 64-bit and 128-bit outputs. Inputs over 64 bytes use parallel AES lanes (AES-NI, VAES, or Armv8 AES); shorter inputs use a multiply-fold path.

* **Fast on bulk data** — 25–30 bytes/cycle for cache-resident inputs ≥4 KiB; 2–3.5× faster than rapidhash, foldhash, and ahash, and 1.5–3× faster than xxh3.
* **Fast in `HashMap`** — 36.6 cycles per `&str` lookup.
* **High quality** — passes all SMHasher and SMHasher3 tests.
* **Stable output** — all backends produce identical bits.
* **Runtime CPU dispatch** — VAES, AES-NI, Armv8 AES, or a T-table software fallback.
* **Streaming + batching** — streaming matches one-shot hashing; batch hashing interleaves records.
* **`std::hash` support** — `RandomState` and `FixedState`.
* **`no_std`**, zero dependencies.

## Usage

```toml
[dependencies]
lanehash = "0.1"
```

### One-shot

```rust
let h64 = lanehash::hash64(b"hello world", 42);
let h128 = lanehash::hash128(b"hello world", 42);
```

### Streaming

```rust
let mut s = lanehash::Stream::new(42);
s.update(b"hello ");
s.update(b"world");

assert_eq!(s.finish128(), lanehash::hash128(b"hello world", 42));
```

### Batched

```rust
let records: Vec<&[u8]> = vec![&[1u8; 200], &[2u8; 300], b"short"];
let mut out = vec![0u64; records.len()];

lanehash::hash64_batch(&records, 42, &mut out);
```

### `HashMap` / `HashSet`

```rust
use std::collections::HashMap;

let mut map: HashMap<&str, u32, lanehash::RandomState> = HashMap::default();
map.insert("key", 1);

let fixed: HashMap<u64, u64, lanehash::FixedState> =
    HashMap::with_hasher(lanehash::FixedState::new(7));
```

## Features

* `std` (default) — runtime CPU detection and `RandomState`.
* `force-fallback` — the T-table software backend on every target (testing).

Without `std`, the backend is selected at compile time from `-C target-feature`.

## Backends

| Target  | Backend   | Requires              |
| ------- | --------- | --------------------- |
| x86-64  | `vaesvl`  | VAES + AVX-512VL      |
| x86-64  | `vaes256` | VAES + AVX2           |
| x86-64  | `aesni`   | AES-NI + SSE2         |
| aarch64 | `neon`    | Armv8 AES             |
| any     | `soft`    | T-table software AES  |

All backends produce identical output.

## Benchmarks

Measured single-threaded on one pinned core with `-C target-cpu=native` and LTO. Throughput is **bytes per core cycle**; the numbers below are medians of 5 interleaved rounds.

### Throughput

| Size          |   lanehash | gxhash (AVX-512) |     gxhash |      xxh3 | rapidhash v3 | foldhash |    ahash |
| ------------- | ---------: | ---------------: | ---------: | --------: | -----------: | -------: | -------: |
| **L1 — 16 B** |   15.8 cyc |          9.8 cyc |    8.9 cyc |   9.4 cyc |     11.8 cyc |  7.1 cyc | 11.7 cyc |
| 256 B         |  11.2 (55) |        12.4 (61) |  11.6 (57) |  2.6 (14) |     4.2 (21) | 7.0 (35) | 7.3 (36) |
| 1 KiB         |  18.4 (92) |       21.0 (104) |  17.3 (86) |  6.9 (35) |     6.7 (33) | 7.6 (38) | 9.1 (46) |
| 4 KiB         | 24.7 (122) |       25.5 (129) |  19.8 (99) | 12.1 (60) |     7.9 (39) | 7.7 (39) | 8.7 (44) |
| 16 KiB        | 27.9 (138) |       25.0 (126) | 20.7 (104) | 15.2 (75) |     8.3 (41) | 7.8 (39) | 8.5 (42) |

**L2/L3**

| Size   |   lanehash | gxhash (AVX-512) |     gxhash |      xxh3 | rapidhash v3 | foldhash |    ahash |
| ------ | ---------: | ---------------: | ---------: | --------: | -----------: | -------: | -------: |
| 16 B   |   14.3 cyc |          8.3 cyc |    8.3 cyc |   8.4 cyc |     10.3 cyc |  7.1 cyc | 10.2 cyc |
| 256 B  |  11.2 (55) |        12.4 (61) |  11.4 (57) |  2.6 (13) |     3.7 (18) | 7.0 (35) | 7.2 (36) |
| 1 KiB  |  18.0 (89) |        19.9 (98) |  16.8 (83) |  6.5 (33) |     6.4 (32) | 7.9 (39) | 9.4 (47) |
| 4 KiB  | 25.2 (125) |       27.4 (136) | 22.3 (111) | 13.0 (64) |     8.1 (40) | 8.1 (40) | 9.1 (46) |
| 64 KiB | 30.3 (150) |       29.7 (147) | 24.9 (123) | 18.5 (91) |     8.8 (43) | 8.1 (40) | 8.9 (46) |
| 1 MiB  | 25.0 (124) |       26.4 (130) | 23.8 (118) | 18.1 (89) |     8.6 (42) | 8.1 (40) | 8.9 (45) |

**DRAM**

| Size   | lanehash | gxhash (AVX-512) |   gxhash |     xxh3 | rapidhash v3 | foldhash |    ahash |
| ------ | -------: | ---------------: | -------: | -------: | -----------: | -------: | -------: |
| 16 B   | 54.4 cyc |         30.5 cyc | 31.5 cyc | 42.0 cyc |     58.9 cyc | 24.2 cyc | 41.2 cyc |
| 256 B  | 3.3 (17) |         3.0 (15) | 2.7 (14) | 2.1 (11) |     2.3 (12) | 4.5 (23) | 4.1 (21) |
| 1 KiB  | 5.4 (28) |         5.1 (26) | 5.5 (28) | 5.8 (29) |     5.2 (26) | 6.0 (30) | 3.4 (17) |
| 4 KiB  | 6.2 (30) |         6.2 (32) | 6.2 (32) | 6.4 (31) |     6.4 (32) | 6.4 (32) | 4.8 (24) |
| 64 KiB | 6.4 (32) |         6.4 (33) | 6.5 (33) | 6.7 (33) |     6.6 (33) | 6.6 (33) | 6.1 (30) |
| 1 MiB  | 6.7 (33) |         6.5 (33) | 6.5 (33) | 6.7 (33) |     6.6 (33) | 6.6 (33) | 6.6 (32) |

### Latency

Dependent chain, cycles per hash:

| Hash             |  4 B |  8 B | 16 B | 32 B | 64 B | 256 B | 1 KiB |
| ---------------- | ---: | ---: | ---: | ---: | ---: | ----: | ----: |
| lanehash         | 27.4 | 27.4 | 26.4 | 27.7 | 36.2 |  69.6 | 104.3 |
| gxhash (AVX-512) | 34.1 | 34.2 | 34.1 | 44.5 | 53.5 |  74.2 | 115.1 |
| gxhash           | 35.2 | 34.2 | 34.3 | 45.6 | 53.4 |  73.3 | 114.5 |
| xxh3             | 29.0 | 29.0 | 27.2 | 28.8 | 31.6 | 106.8 | 156.1 |
| rapidhash v3     | 23.4 | 23.4 | 23.6 | 30.1 | 40.8 |  69.2 | 158.2 |
| foldhash         | 18.4 | 18.6 | 18.6 | 27.9 | 34.4 |  57.3 | 159.5 |
| ahash            | 35.4 | 35.4 | 34.4 | 36.5 | 44.4 |  71.8 | 158.0 |

### `HashMap` workloads

Cycles per operation:

| Hasher              | `&str` insert | `&str` lookup | `u64` insert | `u64` lookup | struct insert | struct lookup | `String` insert | `&[u8]` lookup |
| ------------------- | ------------: | ------------: | -----------: | -----------: | ------------: | ------------: | --------------: | -------------: |
| lanehash            |          48.9 |          36.6 |        136.1 |         47.2 |         101.3 |          30.9 |           123.5 |           70.9 |
| gxhash              |          56.5 |          37.1 |        116.6 |         46.3 |         154.2 |          35.3 |           167.8 |           57.1 |
| rapidhash (fast)    |          42.8 |          34.4 |        128.6 |         44.6 |         120.0 |          28.7 |           119.2 |           64.6 |
| rapidhash (quality) |          47.8 |          39.7 |        137.8 |         49.4 |         114.8 |          36.2 |           128.5 |           70.6 |
| foldhash (fast)     |          42.6 |          34.3 |        131.0 |         50.9 |         106.2 |          29.6 |           120.4 |           57.5 |
| ahash               |          50.8 |          39.5 |        126.2 |         46.1 |         115.3 |          30.5 |           142.2 |           58.4 |
| xxh3                |         150.7 |         162.9 |        229.8 |        123.0 |         192.6 |         179.2 |           167.2 |          180.8 |
| std SipHash-1-3     |          95.0 |          91.3 |        228.4 |        175.9 |         238.1 |         227.2 |           156.5 |          155.6 |

Additional workloads:

* **4 KiB page dedup:** 526 cycles/page with `hash128` + `HashSet<u128>`; gxhash 495, xxh3-128 588, rapidhash v3 1055.
* **64 MiB checksum:** 45.8 GB/s one-shot, 47.2 GB/s through `Stream`; gxhash 49.9 GB/s, xxh3 50.9 GB/s, rapidhash v3 39.0 GB/s.

Full benchmark data is available in `docs/bench/TABLES.md`.

## Quality

| Suite                          |         Result |
| ------------------------------ | -------------: |
| SMHasher3 `--test=All`         | 188 / 188 pass |
| SMHasher3 `--extra --test=All` | 252 / 252 pass |
| SMHasher `--test=All`          |     0 failures |
| Miri                           |          Clean |
| Guard-page test                |       No fault |

## Stability

`hash64`, `hash128`, `Stream`, and batch functions are frozen at v0.1 and produce identical output across backends.

* 64-bit verification: `0x9FF60BEF`
* 128-bit verification: `0x1A79672D`

`RandomState` / `FixedState` output may change between versions and should not be persisted.

Minimum supported Rust version: **1.89**.

## License

MIT OR Apache-2.0