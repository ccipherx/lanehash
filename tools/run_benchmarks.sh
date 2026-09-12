#!/bin/sh
# Reproduce the benchmark data end to end: build, run the pinned-core harness.
# One command in place of running bench/run_cost_final.sh by hand.
#
# Usage: tools/run_benchmarks.sh [outdir]   (default outdir: docs/bench/cost-final)
set -eu
case "${1:-}" in -h|--help)
  echo "Usage: tools/run_benchmarks.sh [outdir]   (default outdir: docs/bench/cost-final)"
  exit 0 ;;
esac
cd "$(dirname "$0")/.."
O=${1:-docs/bench/cost-final}

echo "Building release binaries (native)..."
RUSTFLAGS="-C target-cpu=native" cargo build --release -p bench

if rustup toolchain list 2>/dev/null | grep -q '^nightly'; then
  echo "Building the gxhash AVX-512 baseline (nightly)..."
  RUSTFLAGS="-C target-cpu=native" cargo +nightly build --release -p bench --features hybrid --target-dir target-nightly
else
  echo "No nightly toolchain (rustup toolchain install nightly): skipping the gxhash-hybrid baseline." >&2
fi

echo "Running the harness -> $O ..."
bench/run_cost_final.sh "$O"

echo "Done: $O"
