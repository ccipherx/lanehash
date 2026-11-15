#!/bin/sh
# verify_c.sh one length at a time, for the builds where a length range runs out of
# memory. Usage: verify_c_lengths.sh sse2|aes FROM TO [JOBS], or LENGTHS="a b c" to run
# exactly those lengths.
cd "$(dirname "$0")"
ulimit -v ${MEMLIMIT_KB:-6000000}
CBMC=${CBMC:-$(ls -d "$HOME"/.kani/kani-*/bin 2>/dev/null | tail -1)/cbmc}
case $1 in
  sse2) DEFS="" EXTRA="sse2_models.c" UNWIND=70 ;;
  aes) DEFS="-DLANEHASH_PORTABLE -DLANEHASH_AES_PORTABLE -DWITH_AES -DAES_ONLY" EXTRA="" UNWIND=40 ;;
  *) echo "usage: $0 sse2|aes FROM TO [JOBS]"; exit 2 ;;
esac
export CBMC DEFS EXTRA UNWIND
if [ -n "$LENGTHS" ]
then
  echo $LENGTHS | tr ' ' '\n'
else
  seq "$2" "$3"
fi | xargs -P "${4:-2}" -I{} sh -c '
  out=$("$CBMC" harness.c $EXTRA ../../lanehash-c/lanehash.c ../../lanehash-c/lanehash_aes.c $DEFS -DMINLEN={} -DMAXLEN={} \
    --bounds-check --pointer-check --pointer-overflow-check --signed-overflow-check --undefined-shift-check \
    --div-by-zero-check --memory-leak-check --slice-formula --object-bits 16 --unwind $UNWIND --unwinding-assertions 2>&1)
  if echo "$out" | grep -q "VERIFICATION SUCCESSFUL"
  then
    echo "ok {}"
  else
    echo "FAIL {} $(echo "$out" | grep -E "FAILURE|Out of memory" | head -2 | tr "\n" " ")"
  fi'
