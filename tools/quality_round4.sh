#!/bin/sh
# Track B quality protocol in order, stopping at the first failure.
# Runs on cores 7-11 (bench core 3 and its sibling 9 are avoided for the timed tests; here quality only).
cd "$(dirname "$0")/.."
Q=docs/quality/round4; mkdir -p $Q
S3=third_party/smhasher3/build/SMHasher3; SR=third_party/smhasher/build/SMHasher
fail() { echo "FAILED at: $1" | tee -a $Q/STATUS; exit 1; }
echo "started $(date -Is) commit $(git rev-parse --short HEAD)" > $Q/STATUS
# quick subset, both widths
for h in lanehash_64 lanehash_128; do
  taskset -c 7-11 $S3 --test=Sparse,Zeroes,TwoBytes,Seed,SeedZeroes,SeedSparse,BIC $h > $Q/smhasher3-quick-$h.txt 2>&1
  grep -q 'FAIL' $Q/smhasher3-quick-$h.txt && fail "quick $h"
  echo "quick $h ok $(date -Is)" >> $Q/STATUS
done
for h in lanehash_64 lanehash_128; do
  taskset -c 7-11 $S3 --test=All $h > $Q/smhasher3-$h.txt 2>&1
  grep -q 'FAIL' $Q/smhasher3-$h.txt && fail "All $h"
  echo "All $h ok $(date -Is)" >> $Q/STATUS
done
for h in lanehash_64 lanehash_128; do
  taskset -c 7-11 $S3 --extra --test=All $h > $Q/smhasher3-extra-$h.txt 2>&1
  grep -q 'FAIL' $Q/smhasher3-extra-$h.txt && fail "extra $h"
  echo "extra $h ok $(date -Is)" >> $Q/STATUS
done
for h in lanehash64 lanehash128; do
  (cd third_party/smhasher/build && taskset -c 7-11 ./SMHasher --test=All $h) > $Q/rurban-$h.txt 2>&1
  grep -qE '^\s*FAIL|!!!!|FAILED' $Q/rurban-$h.txt && fail "rurban $h"
  echo "rurban $h ok $(date -Is)" >> $Q/STATUS
done
echo "PROTOCOL_DONE $(date -Is)" >> $Q/STATUS
