# lanehash, C port

Self-contained C99 implementations of both functions, bit-identical to the Rust crate
(`../lanehash`). Written for the SMHasher submission, usable on their own.

| function | files | API |
|---|---|---|
| lanehash (default, no AES) | `lanehash.h`, `lanehash.c`, `lanehash_constants.h` | `lanehash64`, `lanehash128` |
| lanehash_aes | `lanehash_aes.h`, `lanehash_aes.c`, `lanehash_aes_constants.h`, `lanehash_aes_ttables.h` | `lanehash_aes64`, `lanehash_aes128` |

The constants headers are generated from `../lanehash/src/constants.rs`; the T-tables by
`gen_ttables.py`.

```c
#include "lanehash.h"
uint64_t h = lanehash64(buf, len, seed);
uint8_t h128[16];            /* 16 little-endian bytes, low 64 bits == lanehash64 */
lanehash128(buf, len, seed, h128);
```

Backends are chosen at compile time and all produce the same output:

| flags | lanehash | lanehash_aes |
|---|---|---|
| `-march=native` on a Zen 4 / Sapphire Rapids | AVX-512F | VAES, two lanes per ymm |
| `-mavx2` (+ `-mvaes -maes`) | AVX2 | VAES, two lanes per ymm |
| `-maes` / none | SSE2 | AES-NI / T-table software AES |
| `-DLANEHASH_PORTABLE` / `-DLANEHASH_AES_PORTABLE` | scalar | T-table software AES |
| `-DLANEHASH_NO_INT128` / `-DLANEHASH_AES_NO_INT128` | 32-bit multiplies for the folds | same |

Verification values (SMHasher): lanehash 0xD048C22B (64-bit), 0xC2F39939 (128-bit);
lanehash_aes 0x9FF60BEF (64-bit), 0x1A79672D (128-bit).

`make test` builds five variants (scalar, scalar without `__int128`, SSE2 + AES-NI, AVX2 +
VAES, AVX-512 + VAES) and compares each with the Rust crate through `../lanehash-ffi` on
12 678 (length, seed, alignment) cases per function. `make speed` prints cycles per hash of
the C port next to the Rust crate.
