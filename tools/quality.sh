#!/bin/sh
# Quality protocol for one function: tools/quality.sh lanehash|aes. Logs in docs/quality/<fn>/.
# Steps: cargo test, collision searches, SMHasher3 quick set, SMHasher3 All and extra
# (four runs at once), rurban SMHasher All (two runs at once). Stops at the first failure.
# Speed sections inside the parallel runs are taken under load; use docs/bench for timing.
cd "$(dirname "$0")/.."
F=${1:-lanehash}
case $F in
  lanehash) S3N="lanehash_64 lanehash_128"; RN="lanehash64 lanehash128"; ARG="" ;;
  aes) S3N="lanehash_aes_64 lanehash_aes_128"; RN="lanehash_aes64 lanehash_aes128"; ARG=aes ;;
  *) echo "usage: $0 lanehash|aes"; exit 2 ;;
esac
Q=docs/quality/$F; mkdir -p $Q
S3=third_party/smhasher3/build/SMHasher3
B=target/release
fail() { echo "FAILED at: $1 $(date -Is)" | tee -a $Q/STATUS; exit 1; }
echo "started $(date -Is) commit $(git rev-parse --short HEAD)" >> $Q/STATUS
cargo test --release -p lanehash > $Q/cargo-test.txt 2>&1 || fail "cargo test"
RUSTFLAGS="-C target-cpu=native" cargo build --release -p bench --bin collide --bin collide3 >> $Q/cargo-test.txt 2>&1 || fail "bench build"
# 1-2 and 1-3 bit flips over sparse keys: zero collisions required (1280 B needs 8.6 GB)
: > $Q/collide.txt
for run in "collide 96 0" "collide 128 0" "collide 512 0" "collide 1280 0" "collide3 96 0" "collide3 80 0"; do
  set -- $run
  echo "## $run $F" >> $Q/collide.txt
  taskset -c 8 $B/$1 $2 $3 $ARG >> $Q/collide.txt 2>&1 || fail "$run"
  tail -1 $Q/collide.txt | grep -q ': 0 colliding' || fail "$run (collisions)"
done
echo "collide ok $(date -Is)" >> $Q/STATUS
for h in $S3N; do
  taskset -c 1,2,7,8 $S3 --test=Sparse,Zeroes,TwoBytes,Seed,SeedZeroes,SeedSparse,BIC,PerlinNoise $h > $Q/smhasher3-quick-$h.txt 2>&1 &
done
wait
for h in $S3N; do
  grep -q 'FAIL' $Q/smhasher3-quick-$h.txt && fail "quick $h"
  echo "quick $h ok $(date -Is)" >> $Q/STATUS
done
for h in $S3N; do
  taskset -c 1,7 $S3 --test=All $h > $Q/smhasher3-$h.txt 2>&1 &
  taskset -c 2,8,4,10 $S3 --extra --test=All $h > $Q/smhasher3-extra-$h.txt 2>&1 &
done
wait
for h in $S3N; do
  for k in "" "extra-"; do
    f=$Q/smhasher3-$k$h.txt
    grep -q 'Testing took' $f || fail "smhasher3 $k$h incomplete"
    grep -q 'FAIL' $f && fail "smhasher3 $k$h"
    echo "smhasher3 $k$h ok $(date -Is)" >> $Q/STATUS
  done
done
for h in $RN; do
  (cd third_party/smhasher/build && taskset -c 1,2,7,8 ./SMHasher --test=All $h) > $Q/rurban-$h.txt 2>&1 &
done
wait
for h in $RN; do
  f=$Q/rurban-$h.txt
  grep -q 'Testing took' $f || fail "rurban $h incomplete"
  grep -qE '^\s*FAIL|FAILED' $f && fail "rurban $h"
  echo "rurban $h ok $(date -Is), flagged lines: $(grep -c '!!!!!' $f)" >> $Q/STATUS
done
echo "PROTOCOL_DONE $(date -Is)" >> $Q/STATUS
