#!/bin/sh
# Runs every proof and check, with its time. Exits non-zero on any failure.
# Needs Kani, Lean (elan), nightly with Miri and the i686-unknown-linux-musl target.
# QUICK=1 skips Kani and CBMC. START_AT=<part of a step name> resumes there.
cd "$(dirname "$0")/../.."
fail=0
started=${START_AT:+no}
step() {
  name=$1
  shift
  if [ "$started" = no ]
  then
    case "$name" in
      *"$START_AT"*) started=yes ;;
      *) echo "SKIP $name" ; return ;;
    esac
  fi
  start=$(date +%s)
  if "$@"
  then
    echo "PASS $name ($(( $(date +%s) - start ))s)"
  else
    echo "FAIL $name ($(( $(date +%s) - start ))s)"
    fail=1
  fi
}

# 1. Output unchanged: known answers and verification values.
step "cargo test (output tripwire)" sh -c 'cargo test --release -p lanehash 2>&1 | grep -q "FAILED\|panicked" && exit 1 || exit 0'

# 2. The 32-bit stream length fix, natively and under Miri.
step "32-bit stream length tests (i686)" cargo test --release -q -p lanehash --target i686-unknown-linux-musl --lib usize_max
step "32-bit stream length tests (Miri i686)" cargo +nightly miri test -q --release -p lanehash --target i686-unknown-linux-gnu --lib usize_max

# 3. Lean: theorems and their axioms, then the model against the crate.
step "constants are pi" python3 verification/scripts/gen_constants.py
step "lean build" sh -c 'cd verification/lean && lake build > /tmp/lanehash-lean.log 2>&1 ; rc=$? ; grep -E "depends on axioms|does not depend|error" /tmp/lanehash-lean.log ; [ $rc -eq 0 ]'
step "lean model against the crate" sh -c 'cd verification/diff && cargo run --release -q > /tmp/lanehash-corpus.txt && cd ../lean && lake build diffcheck >/dev/null && .lake/build/bin/diffcheck /tmp/lanehash-corpus.txt'

# 4. Escape hatches.
echo "escape hatches (Lean):"
grep -rnE '\bsorry\b|\badmit\b|^axiom |native_decide' verification/lean/Lanehash || echo "  none of sorry, admit, axiom, native_decide"
echo "  bv_decide uses: $(grep -rc 'bv_decide$' verification/lean/Lanehash/*.lean | awk -F: '{s+=$2} END {print s}')"
echo "escape hatches (Kani):"
echo "  unwind bounds: $(grep -rc 'kani::unwind' lanehash/src/kani | awk -F: '{s+=$2} END {print s}'), stubs: $(grep -rc 'kani::stub(' lanehash/src/kani | awk -F: '{s+=$2} END {print s}'), assumptions: $(grep -rc 'kani::assume' lanehash/src/kani | awk -F: '{s+=$2} END {print s}')"

if [ -n "$QUICK" ]
then
  exit $fail
fi

# A proof that outgrows memory fails instead of exhausting the machine. Not set earlier:
# Lean cannot create its threads under this limit.
ulimit -v ${MEMLIMIT_KB:-9000000}

# 5. Kani. The kernel harnesses run one at a time, up to about 8.5 GB each.
step "kani (no stubs)" cargo kani -p lanehash -Z unstable-options --jobs 4 --output-format terse \
  --harness words16_ --harness mum_by_one --harness class_ --harness sponge_ --harness write_ \
  --harness aes_soft_round --harness lane_step_ --harness lane_extract_ --harness broadcast_
for h in absorb_avx2 absorb_sse2 absorb_avx512 hash_avx2_65 hash_sse2_65 hash_avx512_65
do
  step "kani $h (stubbed lane addition)" cargo kani -p lanehash -Z stubbing --harness $h
done

# 6. CBMC on the C port. The SSE2 build and the AES port go one length at a time.
for v in portable noint128
do
  step "cbmc C port ($v, lengths 0..600)" verification/c/verify_c.sh $v
done
step "cbmc C port (sse2, every length 0..600)" sh -c 'verification/c/verify_c_lengths.sh sse2 0 600 2 | tee /tmp/lanehash-sse2.log | grep -v "^ok" ; ! grep -q FAIL /tmp/lanehash-sse2.log'
step "cbmc C AES port (every length 0..64, and 65 100 128 192 256)" sh -c '{ verification/c/verify_c_lengths.sh aes 0 64 2 && LENGTHS="65 100 128 192 256" verification/c/verify_c_lengths.sh aes 0 0 1 ; } | tee /tmp/lanehash-aes.log | grep -v "^ok" ; ! grep -q FAIL /tmp/lanehash-aes.log'
exit $fail
