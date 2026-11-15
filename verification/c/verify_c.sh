#!/bin/sh
# CBMC on the C port: memory safety and undefined behaviour for every length 0 to 600,
# every key and seed. Uses the CBMC from `cargo kani setup`.
# Usage: verify_c.sh [portable|noint128]
cd "$(dirname "$0")"
ulimit -v ${MEMLIMIT_KB:-8000000}
CBMC=${CBMC:-$(ls -d "$HOME"/.kani/kani-*/bin 2>/dev/null | tail -1)/cbmc}
CHECKS="--bounds-check --pointer-check --pointer-overflow-check --signed-overflow-check --undefined-shift-check --div-by-zero-check --memory-leak-check --slice-formula --object-bits 16"
SRC="harness.c ../../lanehash-c/lanehash.c ../../lanehash-c/lanehash_aes.c"
case ${1:-portable} in
  portable) DEFS="-DLANEHASH_PORTABLE -DLANEHASH_AES_PORTABLE" ;;
  noint128) DEFS="-DLANEHASH_PORTABLE -DLANEHASH_NO_INT128 -DLANEHASH_AES_PORTABLE -DLANEHASH_AES_NO_INT128" ;;
  *) echo "usage: $0 portable|noint128"; exit 2 ;;
esac
status=0
run() {
  echo "=== ${1:-portable} lengths $2..$3"
  out=$("$CBMC" $SRC $DEFS -DMINLEN=$2 -DMAXLEN=$3 $CHECKS --unwind 70 --unwinding-assertions 2>&1)
  rc=$?
  echo "$out" | grep -E 'VERIFICATION|FAILURE|\*\* [0-9]+ of|error' | head -20
  if [ $rc -ne 0 ]
  then
    status=1
  fi
}
run "$1" 0 64
run "$1" 65 600
exit $status
