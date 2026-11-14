# lanehash

Fast, portable 64-bit and 128-bit hashing for Rust. Not cryptographic.

There are two independent functions behind the same API. The crate root is the default and runs anywhere: one definition, with a scalar fallback and SSE2, AVX2, AVX-512, NEON and wasm SIMD128 kernels that all give the same output. `lanehash::aes` is an AES-based variant that is a little faster on large inputs when the CPU has VAES. If you are not sure, use the default.

```toml
[dependencies]
lanehash = "0.2"
```

## Usage

```rust
let h64 = lanehash::hash64(b"hello world", 42);
let h128 = lanehash::hash128(b"hello world", 42); // low half equals hash64

// many inputs under one seed, without redoing the seed setup each call
let k = lanehash::Keyed::new(42);
assert_eq!(k.hash64(b"hello world"), h64);

// fixed-size keys: the short path inlines for the known length
assert_eq!(lanehash::hash64_fixed(&[7u8; 8], 42), lanehash::hash64(&[7u8; 8], 42));

// streaming gives the same result as one-shot hashing
let mut s = lanehash::Stream::new(42);
s.update(b"hello ");
s.update(b"world");
assert_eq!(s.finish128(), h128);

// HashMap and HashSet
let mut map: std::collections::HashMap<&str, u32, lanehash::RandomState> = Default::default();
map.insert("key", 1);
```

`FixedState::new(seed)` gives deterministic maps. The `Hasher` behind `RandomState` and `FixedState` is a separate construction, and its output may change between minor releases, so don't persist it.

The AES variant has the same API under `lanehash::aes`, plus `hash64_batch` and `hash128_batch`. Its output has not changed since 0.1.

## Speed

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

Where it is weaker: fixed-length keys of 16 bytes or less (foldhash is about twice as fast there), and latency from 65 to 256 bytes (about 58 cycles against about 40 for foldhash and xxh3). With only SSE2 it does 14 bytes per cycle, and the scalar fallback 2. Without `-C target-cpu=native` short inputs cost a few cycles more. Your numbers will differ, so run the repository's `bench` harness on your own hardware.

## Quality

The default function passes SMHasher3 `--test=All` (188/188) and `--extra` (252/252), and rurban's SMHasher with no flagged result, for both widths.

SMHasher can't find everything. A draft of 0.2.0 passed both suites while 17 to 64 byte inputs had collisions that held under every seed, because only the last multiply carried the seed. Now every multiply operand carries a secret derived from the seed, and the tests in `tests/planted.rs` plant every public constant at every position and try to swap and cancel the internal chains. Miri is clean on every x86 kernel, inputs ending at an unmapped page never fault, and the fuzz targets compare every backend with the reference.

None of this makes it a cryptographic hash. Use it for hash tables, deduplication and checksums, not against attackers who can see outputs.

## Platforms

x86-64 picks AVX-512 (with VL and BMI2), AVX2 (with BMI2) or SSE2 at run time, or at compile time when the target features are enabled. aarch64 uses NEON. It compiles, but hasn't been run on real hardware yet. wasm32 uses SIMD128 when built with `+simd128`. Everything else gets the scalar reference. The crate is `no_std` with the `std` feature off, and has no dependencies. `force-fallback` forces the portable code for testing.

The repository also has a C port and JavaScript/WebAssembly bindings, both bit-identical to the crate.

Verification values (SMHasher): `lanehash` 64-bit `0x36B435A3`, 128-bit `0xB5F2A65D`. `lanehash::aes` 64-bit `0x9FF60BEF`, 128-bit `0x1A79672D`.

Rust 1.89 or newer. MIT or Apache-2.0, at your option.
