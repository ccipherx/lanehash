# Fuzzing

Differential fuzz targets for lanehash, run with
[cargo-fuzz](https://rust-fuzz.github.io/book/introduction.html) (`cargo install
cargo-fuzz`; needs nightly). Every target compares against an oracle, and cargo-fuzz
builds with AddressSanitizer and debug assertions, so an out-of-bounds read by a SIMD
backend or a wrong branch in `Stream::update` fails immediately.

| Target | Checks |
|---|---|
| `oneshot` | `hash128`, `hash64` and every backend on this CPU == `spec::hash128_spec` |
| `stream` | `Stream` under arbitrary chunking == one-shot; `finish128` after every update == hash of the prefix |
| `batch` | `hash128_batch`, `hash64_batch` and `batch_with` on every backend == one-shot per input |
| `hasher` | `LaneHasher` under arbitrary `Hasher` writes: no panic, deterministic |

From `lanehash/`:

```shell
cargo +nightly fuzz run oneshot
cargo +nightly fuzz run oneshot -- -max_len=1048576   # reach the 64 KiB prefetch and 512 KiB sandwich paths
cargo +nightly fuzz run stream
cargo +nightly fuzz run batch
cargo +nightly fuzz run hasher
```

Add `-- -max_total_time=60` for a bounded run. Crashing inputs land in
`fuzz/artifacts/<target>/`; reproduce one with `cargo +nightly fuzz run <target>
fuzz/artifacts/<target>/<file>`.
