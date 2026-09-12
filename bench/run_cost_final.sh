#!/bin/sh
# Final cost run: the WP1 baseline (every hash at 16 B .. 1 MiB in L1 / L2 / DRAM
# residency, exact instruction mix, dependent-chain latency), plus WP2 batch, WP7.5
# stream and checksum runs on the final binary. Core 3, quiet machine.
# Usage: bench/run_cost_final.sh <outdir>
cd "$(dirname "$0")/.."
O=${1:-docs/bench/cost-final}; mkdir -p $O
B=./target/release/bench; N=./target-nightly/release/bench
I=./target/release/icount; NI=./target-nightly/release/icount
H=lanehash,gxhash,rapidhash-v3,xxh3,foldhash,ahash
# L1-resident: 16 KiB working set (16 B: 64 lines)
for cfg in "16 64" "256 64" "1024 16" "4096 4" "16384 1"; do set -- $cfg
  taskset -c 3 $B throughput --rounds 5 --align 16 --nbuf $2 --hashes $H --sizes $1 >> $O/L1.csv 2>&1
  taskset -c 3 $N throughput --rounds 5 --align 16 --nbuf $2 --hashes gxhash-hybrid --sizes $1 >> $O/L1.csv 2>&1
done
# L2-resident: 64 buffers (256 B - 4 KiB), 4096 x 16 B, 8 x 64 KiB; 1 MiB x 4 = L3
taskset -c 3 $B throughput --rounds 5 --align 16 --nbuf 4096 --hashes $H --sizes 16 >> $O/L2.csv 2>&1
taskset -c 3 $N throughput --rounds 5 --align 16 --nbuf 4096 --hashes gxhash-hybrid --sizes 16 >> $O/L2.csv 2>&1
taskset -c 3 $B throughput --rounds 5 --align 16 --hashes $H --sizes 256,1024,4096,65536,1048576 >> $O/L2.csv 2>&1
taskset -c 3 $N throughput --rounds 5 --align 16 --hashes gxhash-hybrid --sizes 256,1024,4096,65536,1048576 >> $O/L2.csv 2>&1
# DRAM-streamed: 1 GiB arena
taskset -c 3 $B throughput --rounds 3 --align 16 --dram --hashes $H --sizes 16,256,1024,4096,65536,1048576 > $O/DRAM.csv 2>&1
taskset -c 3 $N throughput --rounds 3 --align 16 --dram --hashes gxhash-hybrid --sizes 16,256,1024,4096,65536,1048576 >> $O/DRAM.csv 2>&1
# dependent chain
taskset -c 3 $B latency --rounds 5 --align 16 --hashes $H --sizes 4,8,16,32,64,256,1024 > $O/latency.csv 2>&1
taskset -c 3 $N latency --rounds 5 --align 16 --hashes gxhash-hybrid --sizes 4,8,16,32,64,256,1024 >> $O/latency.csv 2>&1
# exact instruction mix per call
taskset -c 3 $I --hashes $H --sizes 4,8,16,32,64,128,256,1024,4096,65536,1048576 --dump $O/icount-dump > $O/icount.csv 2> $O/icount.log
taskset -c 3 $NI --hashes gxhash-hybrid --sizes 4,8,16,32,64,128,256,1024,4096,65536,1048576 --dump $O/icount-dump >> $O/icount.csv 2>> $O/icount.log
# workloads (maps, dedup, checksum)
taskset -c 3 ./target/release/workloads > $O/workloads.csv 2>&1

# batch API (WP2) and stream (WP7.5), final binary
taskset -c 3 $B batch --rounds 5 --align 16 --batch 1,2,4,8 --sizes 128,256,1024,4096 --hashes lanehash > $O/batch-L2.csv 2>&1
for cfg in "256 64" "1024 16" "4096 4"; do set -- $cfg
  taskset -c 3 $B batch --rounds 5 --align 16 --nbuf $2 --batch 1,2,4,8 --sizes $1 --hashes lanehash >> $O/batch-L1.csv 2>&1
done
taskset -c 3 $B throughput --rounds 5 --align 16 --hashes lanehash,lanehash-stream,lanehash-stream4k --sizes 4096,65536,1048576 > $O/stream.csv 2>&1
taskset -c 3 $B throughput --rounds 3 --align 16 --nbuf 4 --hashes lanehash,gxhash,xxh3,rapidhash-v3,foldhash,ahash --sizes 1048576 > $O/L3-1MiB.csv 2>&1
taskset -c 3 $N throughput --rounds 3 --align 16 --nbuf 4 --hashes gxhash-hybrid --sizes 1048576 >> $O/L3-1MiB.csv 2>&1
taskset -c 3 ./target/release/workloads --checksum-only --big-mib 512 --reverse > $O/checksum-512.txt 2>&1
taskset -c 3 ./target/release/workloads --checksum-only --reverse > $O/checksum-64-reversed.txt 2>&1

