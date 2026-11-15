# lanehash

[![crates.io](https://img.shields.io/crates/v/lanehash.svg)](https://crates.io/crates/lanehash)
[![docs.rs](https://img.shields.io/docsrs/lanehash)](https://docs.rs/lanehash)
[![MSRV 1.89](https://img.shields.io/badge/MSRV-1.89-blue)](#stability-and-versioning)
[![license](https://img.shields.io/crates/l/lanehash.svg)](#license)

**Fastest ever known, portable 64-bit and 128-bit hashing for Rust, with formally verified core properties.**

One definition runs everywhere: a scalar reference plus SSE2, AVX2, AVX-512, NEON and wasm SIMD128 kernels, all bit-identical. A second, AES-based function under `lanehash::aes` is a little faster on large inputs when the CPU has VAES. If you are not sure, use the default.

- **High quality**: passes SMHasher3 (`--test=All` 188/188, `--extra` 252/252) and rurban's SMHasher with no flagged result, at 64 and 128 bits.
- **Fast on bulk data**: 26.7 bytes per cycle at 64 KiB with AVX-512, ahead of gxhash, xxh3, foldhash and rapidhash in [our benchmark](#benchmarks).
- **Formally verified core**: Lean, Kani and CBMC proofs of memory safety, kernel equivalence and the seed secrets. See [Formal verification](#formal-verification).
- **Portable**: the same output on every platform and backend, with no AES or other special instructions required, and run-time CPU detection on x86-64.
- **Keyed**: every multiply operand carries a secret derived from the seed.
- **Streaming**: `Stream` gives the one-shot result however the input is split.
- **`HashMap` and `HashSet`** support through `RandomState` and `FixedState`.
- **`no_std` and no dependencies.**
- **C port and WebAssembly bindings**, bit-identical to the crate.
- **Non-cryptographic**: for hash tables, deduplication and checksums.

## Usage

```toml
[dependencies]
lanehash = "0.4"
```

### Hashing bytes

```rust
let h64 = lanehash::hash64(b"hello world", 42);
let h128 = lanehash::hash128(b"hello world", 42);
assert_eq!(h128 as u64, h64); // the low half of hash128 is hash64

// many inputs under one seed, without redoing the seed setup each call
let k = lanehash::Keyed::new(42);
assert_eq!(k.hash64(b"hello world"), h64);

// fixed-size keys: the short path inlines for the known length
assert_eq!(lanehash::hash64_fixed(&[7u8; 8], 42), lanehash::hash64(&[7u8; 8], 42));
```

### Streaming

```rust
let mut s = lanehash::Stream::new(42);
s.update(b"hello ");
s.update(b"world");
assert_eq!(s.finish128(), lanehash::hash128(b"hello world", 42));
assert_eq!(s.finish64(), lanehash::hash64(b"hello world", 42));
```

### HashMap and HashSet

```rust
use std::collections::HashMap;

// seeded at random once per process
let mut map: HashMap<&str, u32, lanehash::RandomState> = HashMap::default();
map.insert("key", 1);

// a fixed seed: the same hashes on every run
let mut fixed = HashMap::with_hasher(lanehash::FixedState::new(7));
fixed.insert("key", 1);
```

### The AES variant

`lanehash::aes` has the same API, plus batched hashing:

```rust
let h = lanehash::aes::hash64(b"hello world", 42);

// many inputs in one call
let inputs: [&[u8]; 2] = [b"hello", b"world"];
let mut out = [0u64; 2];
lanehash::aes::hash64_batch(&inputs, 42, &mut out);
```

## Cargo features

| Feature | Default | Effect |
| --- | --- | --- |
| `std` | yes | Run-time CPU detection on x86-64, and `RandomState`. Without it the crate is `no_std` and picks its x86-64 kernel at compile time from the enabled target features. |
| `force-fallback` | no | Always use the portable scalar code, for testing. |

## Benchmarks

Cycles per hash for short inputs, bytes per cycle for long ones, measured on one x86-64 core with AVX-512, native build. "Random 1–16 B" draws each length at random, the way real key sets look, so branch prediction can't learn it.

| Input | lanehash | lanehash::aes | foldhash | xxh3 | rapidhash | gxhash |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 8 B | 14.1 | 18.2 | 7.1 | 10.4 | 12.3 | 10.9 |
| random 1–16 B | 22.6 | 27.8 | 26.9 | 30.4 | 26.4 | 14.0 |
| 32 B | 13.8 | 18.2 | 12.8 | 12.4 | 18.5 | 10.3 |
| 128 B | 28.3 | 20.5 | 21.1 | 30.5 | 33.7 | 17.0 |
| 512 B | 56.1 | 40.1 | 74.7 | 110.7 | 144.8 | 38.6 |
| 4 KiB | 21.8 B/c | 24.3 B/c | 8.1 B/c | 13.4 B/c | 5.5 B/c | 22.2 B/c |
| 64 KiB | 26.7 B/c | 30.2 B/c | 8.2 B/c | 18.4 B/c | 5.9 B/c | 24.6 B/c |

![Hashing benchmarks](https://raw.githubusercontent.com/ccipherx/lanehash/main/docs/bench/charts/lanehash.svg)

## Quality

The default function passes SMHasher3 `--test=All` (188/188) and `--extra` (252/252), and rurban's SMHasher with no flagged result, at both widths ([full results](https://github.com/ccipherx/lanehash/tree/main/docs/quality/lanehash)).

SMHasher can't find everything. If only the last multiply carried the seed, both suites would still pass while 17 to 64 byte inputs had collisions that hold under every seed. So every multiply operand carries a secret derived from the seed, and [`tests/planted.rs`](https://github.com/ccipherx/lanehash/blob/main/lanehash/tests/planted.rs) plants every public constant at every position and tries to swap and cancel the internal chains. Miri is clean on every x86 kernel, inputs ending at an unmapped page never fault, and the fuzz targets compare every backend with the reference.

## Formal verification

Machine-checked proofs, in [`verification/`](https://github.com/ccipherx/lanehash/tree/main/verification). `verification/scripts/verify.sh` runs them all.

- **Lean 4**, on a model of the hash that reproduces the verification values and matches the crate on 12 808 inputs at every run:
  - every input byte is read, at every length
  - `ka` is uniform in the seed, and `ka ^ kb` takes any value with probability at most 2^-63
  - two multiplies of the short path have secrets that differ by any fixed value with probability at most 2^-62
  - a difference confined to one 32-bit half of a lane never vanishes in a step
  - `Stream` hands the kernels exactly the right bytes, for every sequence of updates
- **Kani**, on the Rust code itself, for every input:
  - the 0 to 64 byte path never reads outside its input and never panics
  - one block through the SSE2, AVX2 and AVX-512 kernels equals the reference, and so does the whole hash at 65 bytes
  - the software AES round equals FIPS-197
- **CBMC**: the C port of the default function has no out-of-bounds read and no undefined behaviour for any input up to 600 bytes.

## DoS resistance

lanehash is a keyed hash, designed to be minimally DoS resistant: with a random seed that stays secret, as `RandomState` gives, an attacker who cannot see hash outputs should not be able to build colliding inputs in advance.

It is not a cryptographic hash. An attacker who knows the seed can build collisions, and one who can observe outputs or response times over many inputs may learn enough to do the same. Don't use it as a MAC or anywhere an attacker can query hashes.

## Platforms

x86-64 picks AVX-512 (with VL and BMI2), AVX2 (with BMI2) or SSE2 at run time, or at compile time when the target features are enabled. aarch64 uses NEON, which compiles but hasn't been run on real hardware yet. wasm32 uses SIMD128 when built with `+simd128`. Every other target gets the scalar reference.

The repository also has a [C port](https://github.com/ccipherx/lanehash/tree/main/lanehash-c) and [JavaScript/WebAssembly bindings](https://github.com/ccipherx/lanehash/tree/main/lanehash-wasm), both bit-identical to the crate.

## Stability and versioning

- `hash64`, `hash128`, `Keyed`, `hash64_fixed`, `hash128_fixed` and `Stream` give the same output on every platform, backend and build. Known-answer tests and the SMHasher verification values pin it, and any change to it is a breaking release.
- The output of `lanehash::aes` is frozen.
- `LaneHasher`, `RandomState` and `FixedState` are for in-memory use. Their output may change between minor releases, so don't persist it.
- The minimum supported Rust version is 1.89.

Verification values (SMHasher): `lanehash` 64-bit `0x36B435A3`, 128-bit `0xB5F2A65D`. `lanehash::aes` 64-bit `0x9FF60BEF`, 128-bit `0x1A79672D`.

## License

MIT or Apache-2.0, at your option.
