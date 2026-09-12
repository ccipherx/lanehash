#!/bin/sh
cd "$(dirname "$0")/../lanehash"
for bm in "" "-Zmiri-tree-borrows"; do
  echo "=== baseline features, MIRIFLAGS='$bm' ==="
  MIRIFLAGS="$bm" cargo +nightly miri test --test vectors --test aes_round 2>&1 | grep -E '^test |result:|error|Undefined|panicked|backends:' | head -20
done
echo "=== +aes,+vaes,+avx2 ==="
RUSTFLAGS="-C target-feature=+aes,+vaes,+avx2" cargo +nightly miri test --test vectors --test aes_round 2>&1 | grep -E '^test |result:|error|Undefined|panicked|backends:|unsupported' | head -20
echo "=== +avx512f,+avx512vl (vaesvl) ==="
RUSTFLAGS="-C target-feature=+aes,+vaes,+avx2,+avx512f,+avx512vl" cargo +nightly miri test --test vectors 2>&1 | grep -E '^test |result:|error|Undefined|panicked|backends:|unsupported' | head -20
echo MIRI_DONE
