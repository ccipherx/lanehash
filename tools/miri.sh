#!/bin/sh
# Miri over the tests of one function: tools/miri.sh lanehash|aes. Stacked and Tree
# Borrows, then with target features so the dispatcher takes the SIMD backends.
cd "$(dirname "$0")/../lanehash"
case ${1:-lanehash} in
  lanehash) T="--test vectors --test api"; TF="+avx2 +avx2,+avx512f" ;;
  aes) T="--test aes_vectors --test aes_round"; TF="+aes,+vaes,+avx2 +aes,+vaes,+avx2,+avx512f,+avx512vl" ;;
  *) echo "usage: $0 lanehash|aes"; exit 2 ;;
esac
G='^test |result:|error|Undefined|panicked|unsupported'
for bm in "" "-Zmiri-tree-borrows"; do
  echo "=== baseline features, MIRIFLAGS='$bm' ==="
  MIRIFLAGS="$bm" cargo +nightly miri test $T 2>&1 | grep -E "$G" | head -20
done
for tf in $TF; do
  echo "=== $tf ==="
  RUSTFLAGS="-C target-feature=$tf" cargo +nightly miri test $T 2>&1 | grep -E "$G" | head -20
done
echo MIRI_DONE
